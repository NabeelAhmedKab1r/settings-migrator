//! Capture-then-restore round trips on fake settings in temp folders. These
//! run on whichever OS the tests run on, using that OS's folder layout.

use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use settings_migrator_engine::{
    bundle::{self, Bundle, CaptureOptions},
    paths::join_relative,
    preview::{DiffLine, Preview},
    process::ProcessList,
    restore::{self, Action},
    undo::UndoSet,
    Os, PathContext, RecipeSet,
};
use tempfile::TempDir;

/// Settings files written for every recipe, as (app id, relative path, contents).
const SETTINGS: &[(&str, &str, &[u8])] = &[
    (
        "vscode",
        "settings.json",
        b"{\n  \"editor.fontSize\": 14\n}\n",
    ),
    (
        "vscode",
        "keybindings.json",
        b"[{\"key\": \"ctrl+k\", \"command\": \"x\"}]",
    ),
    (
        "vscode",
        "snippets/rust.json",
        b"{\"fn\": {\"body\": \"fn $1() {}\"}}",
    ),
    (
        "git",
        ".gitconfig",
        b"[user]\n\tname = Test User\n\temail = test@example.com\n",
    ),
    ("git", ".config/git/ignore", b".DS_Store\nThumbs.db\n"),
    ("obsidian", "obsidian.json", b"{\"vaults\": {}}"),
    (
        "obsidian",
        "Preferences",
        b"{\"spellcheck\": {\"dictionaries\": [\"en-GB\"]}}",
    ),
    // CRLF line endings must survive byte for byte.
    ("vlc", "vlcrc", b"[core]\r\nvolume-step=12.8\r\n"),
    ("obs", "global.ini", b"[General]\nLanguage=en-GB\n"),
    (
        "obs",
        "basic/profiles/Main/basic.ini",
        b"[Output]\nMode=Simple\n",
    ),
    (
        "obs",
        "basic/scenes/Untitled.json",
        b"{\"name\": \"Untitled\"}",
    ),
    // Not UTF-8: binary files must be copied as is.
    ("obs", "basic/scenes/thumb.bin", &[0, 159, 146, 150, 255]),
];

/// Files that sit in settings folders but must never be captured.
const NEVER_CAPTURED: &[(&str, &str)] = &[
    ("vscode", "globalStorage/state.vscdb"),
    ("vscode", "workspaceStorage/abc/state.vscdb"),
    ("git", ".git-credentials"),
    ("obs", "basic/profiles/Main/service.json"),
    ("obs", "basic/profiles/Main/basic.ini.bak"),
    ("obs", "logs/2026-11-17.txt"),
];

fn no_apps_open(os: Os) -> ProcessList {
    ProcessList::from_names(os, Vec::<String>::new())
}

/// VS Code's main program as the process list shows it on `os`.
fn vscode_open(os: Os) -> ProcessList {
    let name = match os {
        Os::MacOs => "/Applications/Visual Studio Code.app/Contents/MacOS/Electron",
        Os::Windows => "Code.exe",
    };
    ProcessList::from_names(os, [name])
}

fn os() -> Os {
    Os::current().expect("tests run on Windows or macOS")
}

fn settings_path(ctx: &PathContext, recipes: &RecipeSet, app: &str, rel: &str) -> PathBuf {
    let recipe = recipes.get(app).unwrap();
    let root = ctx.resolve(recipe.location(ctx.os()).unwrap()).unwrap();
    join_relative(&root, rel).unwrap()
}

fn write(path: &Path, contents: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

/// A fake home with settings for all five apps, plus files that must be left out.
fn source_home(recipes: &RecipeSet) -> (TempDir, PathContext) {
    source_home_for(os(), recipes)
}

fn source_home_for(os: Os, recipes: &RecipeSet) -> (TempDir, PathContext) {
    let home = TempDir::new().unwrap();
    let ctx = PathContext::for_home(os, home.path());
    for (app, rel, contents) in SETTINGS {
        write(&settings_path(&ctx, recipes, app, rel), contents);
    }
    for (app, rel) in NEVER_CAPTURED {
        write(&settings_path(&ctx, recipes, app, rel), b"SECRET-or-cache");
    }
    (home, ctx)
}

/// Every file and folder under `root`, with file contents.
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, Option<Vec<u8>>>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let rel = path.strip_prefix(root).unwrap().to_path_buf();
            if path.is_dir() {
                out.insert(rel, None);
                walk(root, &path, out);
            } else {
                out.insert(rel, Some(fs::read(&path).unwrap()));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

/// Like `snapshot`, but leaving out the app's own undo folder and the folders
/// above it, which creating the undo folder may add (`AppData/Local` on Windows).
fn settings_snapshot(ctx: &PathContext, root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    let state = ctx.state_dir().strip_prefix(root).unwrap().to_path_buf();
    snapshot(root)
        .into_iter()
        .filter(|(rel, _)| !rel.starts_with(&state) && !state.starts_with(rel))
        .collect()
}

fn capture_all(ctx: &PathContext, recipes: &RecipeSet, dir: &Path) -> PathBuf {
    let out = dir.join("bundle.zip");
    bundle::capture(
        ctx,
        &recipes.select(&[]).unwrap(),
        &CaptureOptions::default(),
        &out,
    )
    .unwrap();
    out
}

// Both folder layouts run on every OS; the real OS's file system is what
// differs between the CI runners.
#[test]
fn round_trip_macos_layout() {
    capture_then_restore_then_undo(Os::MacOs);
}

#[test]
fn round_trip_windows_layout() {
    capture_then_restore_then_undo(Os::Windows);
}

fn capture_then_restore_then_undo(os: Os) {
    let recipes = RecipeSet::builtin().unwrap();
    let (_source, source_ctx) = source_home_for(os, &recipes);
    let work = TempDir::new().unwrap();
    let bundle_path = capture_all(&source_ctx, &recipes, work.path());

    // The bundle holds exactly the settings files, and nothing excluded.
    let opened = Bundle::open(&bundle_path).unwrap();
    assert_eq!(opened.manifest.source_os, os);
    let mut captured: Vec<_> = opened
        .apps
        .iter()
        .flat_map(|app| {
            app.files
                .iter()
                .map(|(rel, _)| (app.id.clone(), rel.clone()))
        })
        .collect();
    let mut expected: Vec<_> = SETTINGS
        .iter()
        .map(|(app, rel, _)| (app.to_string(), rel.to_string()))
        .collect();
    captured.sort();
    expected.sort();
    assert_eq!(captured, expected);

    // A new computer that already has some settings: one file differs, one matches.
    let target = TempDir::new().unwrap();
    let ctx = PathContext::for_home(os, target.path());
    write(
        &settings_path(&ctx, &recipes, "git", ".gitconfig"),
        b"[user]\n\tname = Old\n",
    );
    write(
        &settings_path(&ctx, &recipes, "obs", "global.ini"),
        b"[General]\nLanguage=en-GB\n",
    );
    let before = settings_snapshot(&ctx, target.path());

    // Dry run: a full plan, and nothing written (not even the undo folder).
    let plan = restore::plan(&ctx, &recipes, &opened, &[]).unwrap();
    let action = |app: &str, rel: &str| {
        plan.items
            .iter()
            .find(|item| item.app_id == app && item.rel == rel)
            .map(|item| item.action.clone())
            .unwrap()
    };
    assert_eq!(action("git", ".gitconfig"), Action::Update);
    assert_eq!(action("obs", "global.ini"), Action::Unchanged);
    assert_eq!(action("vscode", "settings.json"), Action::Create);
    assert_eq!(plan.writes().count(), SETTINGS.len() - 1);

    // Every change has a preview: a diff for text, sizes for binary files.
    let preview = |app: &str, rel: &str| {
        plan.items
            .iter()
            .find(|item| item.app_id == app && item.rel == rel)
            .unwrap()
            .preview()
    };
    let Preview::Text { lines, .. } = preview("git", ".gitconfig") else {
        panic!("expected a text diff");
    };
    assert!(lines.contains(&DiffLine::Removed("\tname = Old".into())));
    assert!(lines.contains(&DiffLine::Added("\tname = Test User".into())));
    assert!(matches!(
        preview("obs", "basic/scenes/thumb.bin"),
        Preview::Binary { old: None, new: 5 }
    ));
    assert_eq!(preview("obs", "global.ini"), Preview::Nothing);

    assert_eq!(settings_snapshot(&ctx, target.path()), before);
    assert!(!ctx.state_dir().exists());

    // Restore: every file arrives byte for byte.
    let set = restore::apply(&ctx, &recipes, &plan, "bundle.zip", &no_apps_open(ctx.os()))
        .unwrap()
        .unwrap();
    for (app, rel, contents) in SETTINGS {
        let path = settings_path(&ctx, &recipes, app, rel);
        assert_eq!(fs::read(&path).unwrap(), *contents, "{app}/{rel}");
    }
    for (app, rel) in NEVER_CAPTURED {
        assert!(
            !settings_path(&ctx, &recipes, app, rel).exists(),
            "{app}/{rel}"
        );
    }
    assert_eq!(UndoSet::latest(&ctx).unwrap().unwrap().id, set.id);

    // Restoring the same bundle again changes nothing and needs no undo set.
    let again = restore::plan(&ctx, &recipes, &opened, &[]).unwrap();
    assert_eq!(again.writes().count(), 0);
    assert!(restore::apply(
        &ctx,
        &recipes,
        &again,
        "bundle.zip",
        &no_apps_open(ctx.os())
    )
    .unwrap()
    .is_none());

    // Undo: the settings folders are exactly as they were, created folders included.
    let mut set = UndoSet::latest(&ctx).unwrap().unwrap();
    set.apply(&recipes, &no_apps_open(os)).unwrap();
    assert_eq!(settings_snapshot(&ctx, target.path()), before);
    assert!(UndoSet::latest(&ctx).unwrap().is_none());
    assert!(
        set.apply(&recipes, &no_apps_open(os)).is_err(),
        "an undo set can't be applied twice"
    );
}

#[test]
fn restore_can_pick_apps() {
    let recipes = RecipeSet::builtin().unwrap();
    let (_source, source_ctx) = source_home(&recipes);
    let work = TempDir::new().unwrap();
    let opened = Bundle::open(&capture_all(&source_ctx, &recipes, work.path())).unwrap();

    let target = TempDir::new().unwrap();
    let ctx = PathContext::for_home(os(), target.path());
    let plan = restore::plan(&ctx, &recipes, &opened, &["vlc".into()]).unwrap();
    assert!(plan.items.iter().all(|item| item.app_id == "vlc"));
    restore::apply(&ctx, &recipes, &plan, "bundle.zip", &no_apps_open(ctx.os())).unwrap();
    assert!(settings_path(&ctx, &recipes, "vlc", "vlcrc").exists());
    assert!(!settings_path(&ctx, &recipes, "git", ".gitconfig").exists());

    assert!(restore::plan(&ctx, &recipes, &opened, &["zed".into()]).is_err());
}

#[test]
fn capture_never_overwrites() {
    let recipes = RecipeSet::builtin().unwrap();
    let (_source, ctx) = source_home(&recipes);
    let work = TempDir::new().unwrap();
    let out = work.path().join("bundle.zip");
    fs::write(&out, b"keep me").unwrap();
    assert!(bundle::capture(
        &ctx,
        &recipes.select(&[]).unwrap(),
        &CaptureOptions::default(),
        &out
    )
    .is_err());
    assert_eq!(fs::read(&out).unwrap(), b"keep me");
}

/// Rewrites a bundle's entries through `edit`, which gets (name, contents).
fn rewrite_bundle(path: &Path, edit: impl Fn(&str, Vec<u8>) -> Option<(String, Vec<u8>)>) {
    let mut entries = Vec::new();
    {
        let mut zip = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).unwrap();
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).unwrap();
            if let Some(edited) = edit(entry.name(), bytes) {
                entries.push(edited);
            }
        }
    }
    let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
    for (name, bytes) in entries {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(&bytes).unwrap();
    }
    zip.finish().unwrap();
}

fn edit_manifest(path: &Path, edit: impl Fn(&mut serde_json::Value)) {
    rewrite_bundle(path, |name, bytes| {
        if name != "manifest.json" {
            return Some((name.to_string(), bytes));
        }
        let mut manifest: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        edit(&mut manifest);
        Some((name.to_string(), serde_json::to_vec(&manifest).unwrap()))
    });
}

#[test]
fn damaged_bundles_are_rejected() {
    let recipes = RecipeSet::builtin().unwrap();
    let (_source, ctx) = source_home(&recipes);
    let work = TempDir::new().unwrap();
    let path = capture_all(&ctx, &recipes, work.path());

    // A changed file no longer matches its hash.
    rewrite_bundle(&path, |name, bytes| {
        let bytes = if name == "git/.gitconfig" {
            b"[user]\n\tname = Tampered\n".to_vec()
        } else {
            bytes
        };
        Some((name.to_string(), bytes))
    });
    let err = Bundle::open(&path).unwrap_err().to_string();
    assert!(err.contains("hash"), "{err}");
}

#[test]
fn bundle_paths_cannot_escape() {
    let recipes = RecipeSet::builtin().unwrap();
    let (_source, ctx) = source_home(&recipes);
    let work = TempDir::new().unwrap();

    for bad in ["../../evil", "/etc/evil", "a/../../evil"] {
        let path = work.path().join(format!("{}.zip", bad.len()));
        let _ = fs::remove_file(&path);
        bundle::capture(
            &ctx,
            &recipes.select(&["git".into()]).unwrap(),
            &CaptureOptions::default(),
            &path,
        )
        .unwrap();
        edit_manifest(&path, |m| m["apps"][0]["files"][0]["path"] = bad.into());
        assert!(Bundle::open(&path).is_err(), "{bad}");
    }
}

#[test]
fn restore_only_writes_files_the_recipe_lists() {
    let recipes = RecipeSet::builtin().unwrap();
    let (_source, source_ctx) = source_home(&recipes);
    let work = TempDir::new().unwrap();
    let path = work.path().join("bundle.zip");
    bundle::capture(
        &source_ctx,
        &recipes.select(&["git".into()]).unwrap(),
        &CaptureOptions::default(),
        &path,
    )
    .unwrap();

    // Rename .gitconfig in the bundle to a file the git recipe doesn't list.
    rewrite_bundle(&path, |name, bytes| {
        let name = name.replace("git/.gitconfig", "git/.ssh/config");
        let bytes = if name == "manifest.json" {
            String::from_utf8(bytes)
                .unwrap()
                .replace("\".gitconfig\"", "\".ssh/config\"")
                .into_bytes()
        } else {
            bytes
        };
        Some((name, bytes))
    });

    let target = TempDir::new().unwrap();
    let ctx = PathContext::for_home(os(), target.path());
    let plan = restore::plan(&ctx, &recipes, &Bundle::open(&path).unwrap(), &[]).unwrap();
    let item = plan.items.iter().find(|i| i.rel == ".ssh/config").unwrap();
    assert!(matches!(item.action, Action::Skip(_)));
    restore::apply(&ctx, &recipes, &plan, "bundle.zip", &no_apps_open(ctx.os())).unwrap();
    assert!(!target.path().join(".ssh").exists());
}

#[cfg(unix)]
#[test]
fn symlinked_settings_are_skipped() {
    let recipes = RecipeSet::builtin().unwrap();
    let (_source, source_ctx) = source_home(&recipes);
    let work = TempDir::new().unwrap();
    let opened = Bundle::open(&capture_all(&source_ctx, &recipes, work.path())).unwrap();

    // A .gitconfig managed by a dotfiles repo.
    let target = TempDir::new().unwrap();
    let ctx = PathContext::for_home(os(), target.path());
    let real = target.path().join("dotfiles-gitconfig");
    fs::write(&real, b"[user]\n\tname = Dotfiles\n").unwrap();
    std::os::unix::fs::symlink(&real, target.path().join(".gitconfig")).unwrap();

    let plan = restore::plan(&ctx, &recipes, &opened, &["git".into()]).unwrap();
    restore::apply(&ctx, &recipes, &plan, "bundle.zip", &no_apps_open(ctx.os())).unwrap();
    assert!(fs::symlink_metadata(target.path().join(".gitconfig"))
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(fs::read(&real).unwrap(), b"[user]\n\tname = Dotfiles\n");
}

#[test]
fn restore_and_undo_wait_for_open_apps_to_quit() {
    for os in [Os::MacOs, Os::Windows] {
        let recipes = RecipeSet::builtin().unwrap();
        let (_source, source_ctx) = source_home_for(os, &recipes);
        let work = TempDir::new().unwrap();
        let opened = Bundle::open(&capture_all(&source_ctx, &recipes, work.path())).unwrap();

        let target = TempDir::new().unwrap();
        let ctx = PathContext::for_home(os, target.path());
        let plan = restore::plan(&ctx, &recipes, &opened, &[]).unwrap();
        assert_eq!(
            plan.running_apps(&recipes, &vscode_open(os)),
            ["Visual Studio Code"]
        );

        // VS Code is open: nothing is written, not even an undo set.
        let err =
            restore::apply(&ctx, &recipes, &plan, "bundle.zip", &vscode_open(os)).unwrap_err();
        assert!(
            err.to_string().contains("quit Visual Studio Code first"),
            "{err}"
        );
        assert_eq!(snapshot(target.path()), BTreeMap::new());

        // Restoring only other apps doesn't need VS Code closed.
        let git_only = restore::plan(&ctx, &recipes, &opened, &["git".into()]).unwrap();
        restore::apply(&ctx, &recipes, &git_only, "bundle.zip", &vscode_open(os)).unwrap();

        // Undo waits too, once the restore touched VS Code.
        restore::apply(&ctx, &recipes, &plan, "bundle.zip", &no_apps_open(os)).unwrap();
        let mut set = UndoSet::latest(&ctx).unwrap().unwrap();
        assert!(set.apply(&recipes, &vscode_open(os)).is_err());
        assert!(settings_path(&ctx, &recipes, "vscode", "settings.json").exists());
        set.apply(&recipes, &no_apps_open(os)).unwrap();
        assert!(!settings_path(&ctx, &recipes, "vscode", "settings.json").exists());
    }
}
