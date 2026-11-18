//! What the screens ask the engine for. Each function here is a Tauri
//! command and returns plain data the React side can show; nothing here
//! decides what to say to the user beyond what the engine already says.
//!
//! Every command that writes has a preview command before it that writes
//! nothing: `plan_capture` before `save_bundle`, `preview_restore` before
//! `restore`, `preview_undo` before `undo`.

use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

use serde::Serialize;
use settings_migrator_engine::{
    bundle::{self, Bundle, CaptureOptions},
    preview::{DiffLine, Preview},
    process::ProcessList,
    restore::{self, Action},
    scan::{scan, ScanStatus},
    undo::{UndoSet, UndoStep},
    Os, PathContext, RecipeSet,
};
use tauri::State;

/// Everything the commands share.
pub struct AppState {
    ctx: PathContext,
    recipes: RecipeSet,
    /// True when using a stand-in home folder (see `AppState::from_env`).
    /// No running app reads those files, so open apps are ignored.
    sandbox: bool,
    /// The restore preview last shown, so `restore` can check nothing
    /// changed between the preview and the click.
    shown: Mutex<Option<ShownRestore>>,
}

struct ShownRestore {
    bundle: PathBuf,
    files: Vec<RestoreFile>,
}

/// Errors reach the screens as plain text.
type Result<T> = std::result::Result<T, String>;

fn text(err: impl std::fmt::Display) -> String {
    err.to_string()
}

impl AppState {
    /// The real home folder, or the one in `SETTINGS_MIGRATOR_HOME` when it
    /// is set. That stand-in is for trying the app out and for tests, the
    /// same as `--home` on the command line.
    pub fn from_env() -> std::result::Result<AppState, String> {
        let recipes = RecipeSet::builtin().map_err(text)?;
        match std::env::var_os("SETTINGS_MIGRATOR_HOME") {
            Some(home) => {
                let os = Os::current().ok_or("only Windows and macOS are supported")?;
                Ok(AppState::new(
                    PathContext::for_home(os, home),
                    recipes,
                    true,
                ))
            }
            None => Ok(AppState::new(
                PathContext::from_env().map_err(text)?,
                recipes,
                false,
            )),
        }
    }

    pub fn new(ctx: PathContext, recipes: RecipeSet, sandbox: bool) -> AppState {
        AppState {
            ctx,
            recipes,
            sandbox,
            shown: Mutex::new(None),
        }
    }

    fn open_apps(&self) -> Result<ProcessList> {
        if self.sandbox {
            Ok(ProcessList::from_names(self.ctx.os(), Vec::<String>::new()))
        } else {
            ProcessList::current().map_err(text)
        }
    }
}

// ---- About this computer ----------------------------------------------

#[derive(Debug, Serialize)]
pub struct AppInfo {
    pub os: String,
    /// Set when using a stand-in home folder, so the screens can say so.
    pub test_home: Option<String>,
}

#[tauri::command]
pub fn app_info(state: State<'_, AppState>) -> AppInfo {
    AppInfo {
        os: state.ctx.os().to_string(),
        test_home: state.sandbox.then(|| shown(state.ctx.home())),
    }
}

// ---- Scan -------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct ScanResult {
    pub apps: Vec<ScannedApp>,
}

#[derive(Debug, Serialize)]
pub struct ScannedApp {
    pub id: String,
    pub name: String,
    /// "found", "not_found" or "unsupported" (no recipe for this OS).
    pub status: &'static str,
    pub folder: Option<String>,
    pub files: Vec<FileSize>,
}

#[derive(Debug, Serialize)]
pub struct FileSize {
    pub path: String,
    pub size: u64,
}

#[tauri::command]
pub fn scan_apps(state: State<'_, AppState>) -> Result<ScanResult> {
    scan_apps_in(&state)
}

pub fn scan_apps_in(state: &AppState) -> Result<ScanResult> {
    let all = state.recipes.select(&[]).map_err(text)?;
    let apps = scan(&state.ctx, &all)
        .map_err(text)?
        .into_iter()
        .map(|app| {
            let (status, folder, files) = match app.status {
                ScanStatus::Unsupported => ("unsupported", None, Vec::new()),
                ScanStatus::NotFound { root } => ("not_found", Some(shown(&root)), Vec::new()),
                ScanStatus::Found { root, files } => (
                    "found",
                    Some(shown(&root)),
                    files
                        .into_iter()
                        .map(|(path, size)| FileSize { path, size })
                        .collect(),
                ),
            };
            ScannedApp {
                id: app.recipe.id.clone(),
                name: app.recipe.name.clone(),
                status,
                folder,
                files,
            }
        })
        .collect();
    Ok(ScanResult { apps })
}

fn shown(path: &Path) -> String {
    path.display().to_string()
}

// ---- Capture ----------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct CapturePreview {
    pub files: Vec<CaptureFile>,
}

#[derive(Debug, Serialize)]
pub struct CaptureFile {
    pub app_id: String,
    pub app_name: String,
    pub path: String,
    /// `app/path`, the name used to skip or allow a flagged file.
    pub key: String,
    pub size: u64,
    /// Possible secrets the guard found. Excerpts are shortened so the
    /// secret itself is never shown.
    pub findings: Vec<SecretFinding>,
}

#[derive(Debug, Serialize)]
pub struct SecretFinding {
    pub line: usize,
    pub kind: String,
    pub excerpt: String,
}

/// What capturing `apps` would put in the bundle, and which files need a
/// decision first. Writes nothing.
#[tauri::command]
pub fn plan_capture(state: State<'_, AppState>, apps: Vec<String>) -> Result<CapturePreview> {
    plan_capture_in(&state, &apps)
}

pub fn plan_capture_in(state: &AppState, apps: &[String]) -> Result<CapturePreview> {
    if apps.is_empty() {
        return Err("Choose at least one app.".into());
    }
    let selected = state.recipes.select(apps).map_err(text)?;
    let plan =
        bundle::plan_capture(&state.ctx, &selected, &CaptureOptions::default()).map_err(text)?;
    Ok(CapturePreview {
        files: plan
            .files
            .iter()
            .map(|file| CaptureFile {
                app_id: file.app_id.clone(),
                app_name: file.app_name.clone(),
                path: file.rel.clone(),
                key: file.logical_path(),
                size: file.size,
                findings: file
                    .findings
                    .iter()
                    .map(|finding| SecretFinding {
                        line: finding.line,
                        kind: finding.kind.to_string(),
                        excerpt: finding.excerpt.clone(),
                    })
                    .collect(),
            })
            .collect(),
    })
}

#[derive(Debug, Serialize)]
pub struct SavedBundle {
    pub path: String,
    pub files: usize,
}

/// Writes the bundle. `skip` and `allow` hold the user's decision for each
/// flagged file; the engine refuses if any flagged file has none, and
/// never overwrites an existing file.
#[tauri::command]
pub fn save_bundle(
    state: State<'_, AppState>,
    apps: Vec<String>,
    skip: Vec<String>,
    allow: Vec<String>,
    path: PathBuf,
) -> Result<SavedBundle> {
    save_bundle_in(&state, &apps, skip, allow, &path)
}

pub fn save_bundle_in(
    state: &AppState,
    apps: &[String],
    skip: Vec<String>,
    allow: Vec<String>,
    path: &Path,
) -> Result<SavedBundle> {
    if path.exists() {
        return Err(format!(
            "There's already a file called {}. Choose another name, so it isn't replaced.",
            path.file_name().unwrap_or_default().to_string_lossy()
        ));
    }
    let selected = state.recipes.select(apps).map_err(text)?;
    let plan = bundle::plan_capture(&state.ctx, &selected, &CaptureOptions { skip, allow })
        .map_err(text)?;
    if plan.blocking().next().is_some() {
        return Err(
            "A file was flagged as a possible secret since you checked. Go back and check again."
                .into(),
        );
    }
    plan.write(path).map_err(text)?;
    Ok(SavedBundle {
        path: shown(path),
        files: plan.included().count(),
    })
}

// ---- Restore ----------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct RestorePreview {
    pub bundle_name: String,
    pub created: String,
    pub source_os: String,
    pub target_os: String,
    pub cross_os: bool,
    pub apps: Vec<RestoreApp>,
    /// Names of open apps that need quitting before restoring.
    pub running: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct RestoreApp {
    pub id: String,
    pub name: String,
    pub files: Vec<RestoreFile>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RestoreFile {
    pub app_id: String,
    pub path: String,
    /// Where it goes on this computer.
    pub target: Option<String>,
    /// "create", "update", "unchanged" or "skip".
    pub action: &'static str,
    pub skip_reason: Option<String>,
    /// What was translated for this OS.
    pub changes: Vec<String>,
    /// What couldn't be translated and is restored as it was.
    pub flags: Vec<FlagItem>,
    pub diff: Diff,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FlagItem {
    /// 0 means the whole file.
    pub line: usize,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Diff {
    Nothing,
    LineEndingsOnly,
    Binary { old: Option<u64>, new: u64 },
    Text { lines: Vec<Line>, hidden: usize },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Line {
    /// "hunk", "context", "added" or "removed".
    pub kind: &'static str,
    pub text: String,
}

impl From<Preview> for Diff {
    fn from(preview: Preview) -> Diff {
        match preview {
            Preview::Nothing => Diff::Nothing,
            Preview::LineEndingsOnly => Diff::LineEndingsOnly,
            Preview::Binary { old, new } => Diff::Binary { old, new },
            Preview::Text { lines, hidden } => Diff::Text {
                lines: lines
                    .into_iter()
                    .map(|line| {
                        let (kind, text) = match line {
                            DiffLine::Hunk(text) => ("hunk", text),
                            DiffLine::Context(text) => ("context", text),
                            DiffLine::Added(text) => ("added", text),
                            DiffLine::Removed(text) => ("removed", text),
                        };
                        Line { kind, text }
                    })
                    .collect(),
                hidden,
            },
        }
    }
}

/// Plans restoring `apps` from the bundle (every app when empty) and turns
/// it into what the screen shows.
fn restore_files(
    state: &AppState,
    opened: &Bundle,
    apps: &[String],
) -> Result<(restore::RestorePlan, Vec<RestoreFile>)> {
    let plan = restore::plan(&state.ctx, &state.recipes, opened, apps).map_err(text)?;
    let files = plan
        .items
        .iter()
        .map(|item| {
            let (action, skip_reason) = match &item.action {
                Action::Create => ("create", None),
                Action::Update => ("update", None),
                Action::Unchanged => ("unchanged", None),
                Action::Skip(reason) => ("skip", Some(reason.clone())),
            };
            let writes = matches!(item.action, Action::Create | Action::Update);
            RestoreFile {
                app_id: item.app_id.clone(),
                path: item.rel.clone(),
                target: item.target.as_deref().map(shown),
                action,
                skip_reason,
                changes: if writes {
                    item.changes.clone()
                } else {
                    Vec::new()
                },
                flags: if writes {
                    item.flags
                        .iter()
                        .map(|flag| FlagItem {
                            line: flag.line,
                            message: flag.message.clone(),
                        })
                        .collect()
                } else {
                    Vec::new()
                },
                diff: item.preview().into(),
            }
        })
        .collect();
    Ok((plan, files))
}

/// What restoring the bundle would change, file by file. Writes nothing.
#[tauri::command]
pub fn preview_restore(state: State<'_, AppState>, bundle: PathBuf) -> Result<RestorePreview> {
    preview_restore_in(&state, &bundle)
}

pub fn preview_restore_in(state: &AppState, bundle_path: &Path) -> Result<RestorePreview> {
    let opened = Bundle::open(bundle_path)
        .map_err(|err| format!("This file can't be read as a settings bundle. ({err})"))?;
    let (plan, files) = restore_files(state, &opened, &[])?;
    let running = plan.running_apps(&state.recipes, &state.open_apps()?);
    let apps = opened
        .apps
        .iter()
        .map(|app| RestoreApp {
            id: app.id.clone(),
            name: app.name.clone(),
            files: files
                .iter()
                .filter(|file| file.app_id == app.id)
                .cloned()
                .collect(),
        })
        .collect();
    *state.shown.lock().unwrap() = Some(ShownRestore {
        bundle: bundle_path.to_path_buf(),
        files,
    });
    Ok(RestorePreview {
        bundle_name: file_name(bundle_path),
        created: opened.manifest.created.clone(),
        source_os: plan.source_os.to_string(),
        target_os: plan.target_os.to_string(),
        cross_os: plan.is_cross_os(),
        apps,
        running,
    })
}

#[derive(Debug, Serialize)]
pub struct Restored {
    pub files: usize,
}

/// Restores the chosen apps, backing up every original first. Refuses if
/// anything differs from the preview the user saw, or an app is open.
#[tauri::command]
pub fn restore(state: State<'_, AppState>, bundle: PathBuf, apps: Vec<String>) -> Result<Restored> {
    restore_in(&state, &bundle, &apps)
}

pub fn restore_in(state: &AppState, bundle_path: &Path, apps: &[String]) -> Result<Restored> {
    if apps.is_empty() {
        return Err("Choose at least one app.".into());
    }
    let opened = Bundle::open(bundle_path).map_err(text)?;
    let (plan, files) = restore_files(state, &opened, apps)?;
    let matches_preview = state.shown.lock().unwrap().as_ref().is_some_and(|shown| {
        let seen: Vec<_> = shown
            .files
            .iter()
            .filter(|file| apps.contains(&file.app_id))
            .cloned()
            .collect();
        shown.bundle == bundle_path && seen == files
    });
    if !matches_preview {
        return Err(
            "Something changed since the preview was made. Look over the new preview, then restore again."
                .into(),
        );
    }
    let set = restore::apply(
        &state.ctx,
        &state.recipes,
        &plan,
        &file_name(bundle_path),
        &state.open_apps()?,
    )
    .map_err(text)?;
    *state.shown.lock().unwrap() = None;
    Ok(Restored {
        files: set.map_or(0, |set| set.record.entries.len()),
    })
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

// ---- Undo -------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct UndoPreview {
    pub id: String,
    pub bundle_name: String,
    pub created: String,
    pub steps: Vec<UndoItem>,
    pub running: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct UndoItem {
    pub app_name: String,
    /// "put_back" (the original returns) or "delete" (the restore made it).
    pub kind: &'static str,
    pub path: String,
}

/// What undoing the latest restore would do, or `None` if there's nothing
/// to undo. Changes nothing.
#[tauri::command]
pub fn preview_undo(state: State<'_, AppState>) -> Result<Option<UndoPreview>> {
    preview_undo_in(&state)
}

pub fn preview_undo_in(state: &AppState) -> Result<Option<UndoPreview>> {
    let Some(set) = UndoSet::latest(&state.ctx).map_err(text)? else {
        return Ok(None);
    };
    let steps = set
        .steps()
        .into_iter()
        .zip(&set.record.entries)
        .map(|(step, entry)| {
            let app_name = state
                .recipes
                .get(&entry.app_id)
                .map_or(entry.app_id.clone(), |recipe| recipe.name.clone());
            let (kind, path) = match step {
                UndoStep::Restore(path) => ("put_back", path),
                UndoStep::Delete(path) => ("delete", path),
            };
            UndoItem {
                app_name,
                kind,
                path: shown(&path),
            }
        })
        .collect();
    Ok(Some(UndoPreview {
        running: set.running_apps(&state.recipes, &state.open_apps()?),
        id: set.id.clone(),
        bundle_name: set.record.bundle.clone(),
        created: set.record.created.clone(),
        steps,
    }))
}

/// Undoes the restore `id`, which must still be the latest one, so the
/// screen can't undo something other than what it showed.
#[tauri::command]
pub fn undo(state: State<'_, AppState>, id: String) -> Result<()> {
    undo_in(&state, &id)
}

pub fn undo_in(state: &AppState, id: &str) -> Result<()> {
    let mut set = UndoSet::latest(&state.ctx)
        .map_err(text)?
        .filter(|set| set.id == id)
        .ok_or("Something changed since this was shown. Go back and look again.")?;
    set.apply(&state.recipes, &state.open_apps()?).map_err(text)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use settings_migrator_engine::paths::join_relative;
    use tempfile::TempDir;

    use super::*;

    /// A fake home with VS Code settings, one holding a fake token.
    fn fake_home() -> (TempDir, AppState) {
        let home = TempDir::new().unwrap();
        let os = Os::current().unwrap();
        let ctx = PathContext::for_home(os, home.path());
        let recipes = RecipeSet::builtin().unwrap();
        let root = ctx
            .resolve(recipes.get("vscode").unwrap().location(os).unwrap())
            .unwrap();
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("settings.json"), "{\"editor.fontSize\": 14}\n").unwrap();
        fs::write(
            root.join("keybindings.json"),
            "// token: ghp_aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789\n[]\n",
        )
        .unwrap();
        (home, AppState::new(ctx, recipes, true))
    }

    #[test]
    fn scan_lists_every_app() {
        let (_home, state) = fake_home();
        let result = scan_apps_in(&state).unwrap();
        assert_eq!(result.apps.len(), 5);
        let vscode = result.apps.iter().find(|app| app.id == "vscode").unwrap();
        assert_eq!(vscode.status, "found");
        assert_eq!(vscode.files.len(), 2);
        let git = result.apps.iter().find(|app| app.id == "git").unwrap();
        assert_eq!(git.status, "not_found");
    }

    #[test]
    fn capture_needs_a_decision_on_each_flagged_file() {
        let (home, state) = fake_home();
        let apps = vec!["vscode".to_string()];
        let preview = plan_capture_in(&state, &apps).unwrap();
        let flagged: Vec<_> = preview
            .files
            .iter()
            .filter(|file| !file.findings.is_empty())
            .collect();
        assert_eq!(flagged.len(), 1);
        assert_eq!(flagged[0].key, "vscode/keybindings.json");
        assert!(!flagged[0].findings[0]
            .excerpt
            .contains("aBcDeFgHiJkLmNoPqRsTuVwXyZ"));

        let out = home.path().join("bundle.zip");
        assert!(save_bundle_in(&state, &apps, vec![], vec![], &out).is_err());
        assert!(!out.exists());

        let saved = save_bundle_in(
            &state,
            &apps,
            vec!["vscode/keybindings.json".into()],
            vec![],
            &out,
        )
        .unwrap();
        assert_eq!(saved.files, 1);
        let opened = Bundle::open(&out).unwrap();
        assert_eq!(opened.apps[0].files.len(), 1);

        // An existing file is never replaced.
        let again = save_bundle_in(
            &state,
            &apps,
            vec!["vscode/keybindings.json".into()],
            vec![],
            &out,
        );
        assert!(again.unwrap_err().contains("already a file"));
    }

    #[test]
    fn restore_matches_the_preview_and_can_be_undone() {
        let (home, state) = fake_home();
        let apps = vec!["vscode".to_string()];
        let out = home.path().join("bundle.zip");
        save_bundle_in(
            &state,
            &apps,
            vec!["vscode/keybindings.json".into()],
            vec![],
            &out,
        )
        .unwrap();

        // Change the setting after capturing, so restoring has work to do.
        let os = state.ctx.os();
        let root = state
            .ctx
            .resolve(state.recipes.get("vscode").unwrap().location(os).unwrap())
            .unwrap();
        let settings = join_relative(&root, "settings.json").unwrap();
        fs::write(&settings, "{\"editor.fontSize\": 20}\n").unwrap();

        // Restoring without a preview is refused.
        assert!(restore_in(&state, &out, &apps).is_err());

        let preview = preview_restore_in(&state, &out).unwrap();
        assert!(!preview.cross_os);
        let file = &preview.apps[0].files[0];
        assert_eq!(
            (file.path.as_str(), file.action),
            ("settings.json", "update")
        );
        let Diff::Text { lines, .. } = &file.diff else {
            panic!("expected a text diff, got {:?}", file.diff);
        };
        assert!(lines
            .iter()
            .any(|l| l.kind == "added" && l.text.contains("14")));

        // A change after the preview is caught.
        fs::write(&settings, "{\"editor.fontSize\": 30}\n").unwrap();
        let err = restore_in(&state, &out, &apps).unwrap_err();
        assert!(err.contains("changed since the preview"), "{err}");
        assert!(fs::read_to_string(&settings).unwrap().contains("30"));

        preview_restore_in(&state, &out).unwrap();
        assert_eq!(restore_in(&state, &out, &apps).unwrap().files, 1);
        assert!(fs::read_to_string(&settings).unwrap().contains("14"));

        let undo = preview_undo_in(&state).unwrap().unwrap();
        assert_eq!(undo.steps.len(), 1);
        assert_eq!(undo.steps[0].kind, "put_back");
        assert_eq!(undo.steps[0].app_name, "Visual Studio Code");
        assert!(undo_in(&state, "not-the-latest").is_err());
        undo_in(&state, &undo.id).unwrap();
        assert!(fs::read_to_string(&settings).unwrap().contains("30"));
        assert!(preview_undo_in(&state).unwrap().is_none());
    }
}
