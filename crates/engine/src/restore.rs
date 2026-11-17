//! Restores a bundle: first a plan (what dry-run shows), then applying it,
//! which backs up every original into an undo set before writing anything.

use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use crate::{
    bundle::Bundle,
    paths::join_relative,
    undo::{UndoEntry, UndoSet},
    Error, PathContext, RecipeSet, Result,
};

#[derive(Debug)]
pub struct RestorePlan {
    pub items: Vec<PlannedFile>,
}

#[derive(Debug)]
pub struct PlannedFile {
    pub app_id: String,
    pub app_name: String,
    /// Path relative to the app's settings folder.
    pub rel: String,
    /// Where it goes on this computer. `None` when the item is skipped
    /// before a target could be worked out.
    pub target: Option<PathBuf>,
    pub action: Action,
    contents: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// The file doesn't exist yet.
    Create,
    /// The file exists with different contents; it's backed up first.
    Update,
    /// The file already has exactly these contents.
    Unchanged,
    /// Not written, for the given reason.
    Skip(String),
}

impl RestorePlan {
    /// The items that would write a file.
    pub fn writes(&self) -> impl Iterator<Item = &PlannedFile> {
        self.items
            .iter()
            .filter(|item| matches!(item.action, Action::Create | Action::Update))
    }
}

/// Works out what restoring `bundle` would do, without writing anything.
/// `apps` limits it to those recipe ids; empty means every app in the bundle.
pub fn plan(
    ctx: &PathContext,
    recipes: &RecipeSet,
    bundle: &Bundle,
    apps: &[String],
) -> Result<RestorePlan> {
    if bundle.manifest.source_os != ctx.os() {
        return Err(Error::Other(format!(
            "this bundle was made on {}; restoring on {} isn't supported yet",
            bundle.manifest.source_os,
            ctx.os()
        )));
    }
    for id in apps {
        if !bundle.apps.iter().any(|app| &app.id == id) {
            return Err(Error::Other(format!(
                "the bundle has no settings for {id:?}"
            )));
        }
    }

    let mut items = Vec::new();
    for app in &bundle.apps {
        if !apps.is_empty() && !apps.contains(&app.id) {
            continue;
        }
        let root = match recipes.get(&app.id).map(|r| (r, r.location(ctx.os()))) {
            None => Err(format!("no recipe for {:?}", app.id)),
            Some((_, None)) => Err(format!("the recipe doesn't support {}", ctx.os())),
            Some((recipe, Some(location))) => Ok((recipe, ctx.resolve(location)?)),
        };
        for (rel, contents) in &app.files {
            let (target, action) = match &root {
                Err(reason) => (None, Action::Skip(reason.clone())),
                // The recipe on this computer decides what may be written, so a
                // bundle can never place files the recipe doesn't list.
                Ok((recipe, _)) if !recipe.allows(rel) => (
                    None,
                    Action::Skip("not a settings file in this app's recipe".into()),
                ),
                Ok((_, root)) => {
                    let target = join_relative(root, rel)?;
                    let action = action_for(&target, contents)?;
                    (Some(target), action)
                }
            };
            items.push(PlannedFile {
                app_id: app.id.clone(),
                app_name: app.name.clone(),
                rel: rel.clone(),
                target,
                action,
                contents: contents.clone(),
            });
        }
    }
    Ok(RestorePlan { items })
}

fn action_for(target: &Path, contents: &[u8]) -> Result<Action> {
    let meta = match fs::symlink_metadata(target) {
        Ok(meta) => meta,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Action::Create),
        Err(e) => return Err(Error::io(target, e)),
    };
    if meta.file_type().is_symlink() {
        // Writing would replace the link (often into a dotfiles repo) with a
        // plain file, and undo couldn't bring the link back.
        return Ok(Action::Skip("it's a symbolic link".into()));
    }
    if !meta.is_file() {
        return Ok(Action::Skip("a folder is in the way".into()));
    }
    let current = fs::read(target).map_err(|e| Error::io(target, e))?;
    Ok(if current == contents {
        Action::Unchanged
    } else {
        Action::Update
    })
}

/// Applies a plan. Every file it will touch is recorded in a new undo set,
/// with a backup of each original, before any settings file is written.
/// Returns `None` when there was nothing to write.
pub fn apply(ctx: &PathContext, plan: &RestorePlan, bundle_name: &str) -> Result<Option<UndoSet>> {
    let writes: Vec<_> = plan.writes().collect();
    if writes.is_empty() {
        return Ok(None);
    }

    let mut set = UndoSet::create(ctx, bundle_name)?;
    for item in &writes {
        let target = item.target.as_ref().expect("writes always have a target");
        let backup = match item.action {
            Action::Update => Some(set.back_up(target)?),
            _ => None,
        };
        let created_dirs = missing_ancestors(target);
        set.record(UndoEntry {
            app_id: item.app_id.clone(),
            rel: item.rel.clone(),
            target: target.clone(),
            backup,
            created_dirs,
        });
    }
    // The undo set is complete on disk before the first settings file changes.
    set.save()?;

    for item in writes {
        let target = item.target.as_ref().expect("writes always have a target");
        write_file(target, &item.contents).map_err(|e| {
            Error::Other(format!(
                "{e}\nSome files may already be restored; run `undo` to put everything back"
            ))
        })?;
    }
    Ok(Some(set))
}

/// Folders above `path` that don't exist yet, deepest first.
fn missing_ancestors(path: &Path) -> Vec<PathBuf> {
    path.ancestors()
        .skip(1)
        .take_while(|dir| !dir.as_os_str().is_empty() && !dir.exists())
        .map(Path::to_path_buf)
        .collect()
}

/// Writes through a temporary file in the same folder and renames it into
/// place, so a crash never leaves a half-written settings file.
pub(crate) fn write_file(target: &Path, contents: &[u8]) -> Result<()> {
    let dir = target
        .parent()
        .ok_or_else(|| Error::Other(format!("{} has no parent folder", target.display())))?;
    fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    let name = target.file_name().unwrap_or_default().to_string_lossy();
    let temp = dir.join(format!(".{name}.settings-migrator-tmp"));
    fs::write(&temp, contents).map_err(|e| Error::io(&temp, e))?;
    fs::rename(&temp, target).map_err(|e| {
        let _ = fs::remove_file(&temp);
        Error::io(target, e)
    })
}
