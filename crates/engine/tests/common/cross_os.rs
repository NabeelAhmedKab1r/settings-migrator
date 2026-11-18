//! The cross-OS sample settings in `tests/fixtures/cross-os`: a fake Windows
//! home and a fake macOS home with settings for every recipe, and what each
//! app's files should look like after restoring on the other OS. Shared by
//! the engine and CLI tests.

#![allow(dead_code)]

use std::{
    fs,
    path::{Path, PathBuf},
};

use settings_migrator_engine::{paths::join_relative, Os, PathContext, RecipeSet};

/// Works from either crate, since both sit in `crates/`.
pub fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../engine/tests/fixtures/cross-os")
}

/// The sample home folder laid out the way `os` lays it out.
pub fn source_home(os: Os) -> PathBuf {
    fixtures().join(match os {
        Os::Windows => "windows-home",
        Os::MacOs => "macos-home",
    })
}

pub fn other(os: Os) -> Os {
    match os {
        Os::Windows => Os::MacOs,
        Os::MacOs => Os::Windows,
    }
}

/// What restoring the other OS's sample home on `target` can't translate,
/// as (app, file, line, part of the message).
pub fn expected_flags(target: Os) -> &'static [(&'static str, &'static str, usize, &'static str)] {
    match target {
        Os::MacOs => &[
            (
                "git",
                ".gitconfig",
                5,
                "`autocrlf = true` sets how Git changes line endings",
            ),
            (
                "git",
                ".gitconfig",
                6,
                "`C:/Program Files/Notepad++/notepad++.exe` is outside your home folder",
            ),
            (
                "obs",
                "basic/scenes/Main.json",
                1,
                "`D:/Stream Assets/logo.png` is outside",
            ),
            (
                "obsidian",
                "obsidian.json",
                1,
                r"`D:\Vaults\Work` is outside",
            ),
            (
                "vscode",
                "keybindings.json",
                6,
                "`ctrl+tab` would become `cmd+tab`, which macOS keeps for itself",
            ),
            (
                "vscode",
                "keybindings.json",
                7,
                "`win+e` uses the Windows key",
            ),
            ("vscode", "keybindings.json", 8, "(`isWindows`)"),
            (
                "vscode",
                "settings.json",
                4,
                r"`C:\Program Files\Git\bin\git.exe` is outside",
            ),
        ],
        Os::Windows => &[
            (
                "git",
                ".gitconfig",
                5,
                "`autocrlf = input` sets how Git changes line endings",
            ),
            (
                "git",
                ".gitconfig",
                9,
                "`/opt/homebrew/etc/gitconfig` is outside your home folder",
            ),
            (
                "obs",
                "basic/scenes/Main.json",
                1,
                "`/Volumes/Assets/logo.png` is outside",
            ),
            (
                "obsidian",
                "obsidian.json",
                1,
                "`/Volumes/Backup/Work` is outside",
            ),
            (
                "vscode",
                "keybindings.json",
                7,
                "`cmd+ctrl+f` uses both Cmd and Ctrl",
            ),
            (
                "vscode",
                "keybindings.json",
                8,
                "`alt+tab` is kept by Windows for itself",
            ),
            (
                "vscode",
                "keybindings.json",
                10,
                "`ctrl+w` is what `cmd+w` on line 9 becomes",
            ),
            (
                "vscode",
                "settings.json",
                4,
                "`/opt/homebrew/bin/git` is outside",
            ),
        ],
    }
}

/// Every file under `dir`, as sorted `/`-separated relative paths.
fn files_under(dir: &Path) -> Vec<String> {
    fn walk(dir: &Path, rel: &str, out: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries {
            let entry = entry.unwrap();
            let name = entry.file_name().into_string().unwrap();
            let child = if rel.is_empty() {
                name
            } else {
                format!("{rel}/{name}")
            };
            if entry.file_type().unwrap().is_dir() {
                walk(&entry.path(), &child, out);
            } else {
                out.push(child);
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, "", &mut out);
    out.sort();
    out
}

/// Checks that each app's settings on `ctx` are exactly the expected files
/// for its OS: the same files, byte for byte, with `{home}` and
/// `{home_json}` standing for the home folder.
pub fn check_restored(ctx: &PathContext, recipes: &RecipeSet) {
    let mut home = ctx.home().to_string_lossy().into_owned();
    if ctx.os() == Os::Windows {
        // Also when the Windows layout is tried out on a Mac.
        home = home.replace('/', "\\");
    }
    let home_json = serde_json::to_string(&home).unwrap();
    let home_json = &home_json[1..home_json.len() - 1];
    let home = home.replace('\\', "/");
    let expected_dir = fixtures().join(match ctx.os() {
        Os::Windows => "expected-on-windows",
        Os::MacOs => "expected-on-macos",
    });

    for recipe in recipes.iter() {
        let root = ctx.resolve(recipe.location(ctx.os()).unwrap()).unwrap();
        let restored = recipe.collect_files(&root).unwrap();
        let expected = files_under(&expected_dir.join(&recipe.id));
        assert!(!expected.is_empty(), "no expected files for {}", recipe.id);
        assert_eq!(restored, expected, "files restored for {}", recipe.id);
        for rel in expected {
            let want =
                fs::read_to_string(join_relative(&expected_dir.join(&recipe.id), &rel).unwrap())
                    .unwrap()
                    .replace("{home_json}", home_json)
                    .replace("{home}", &home);
            let got = fs::read(join_relative(&root, &rel).unwrap()).unwrap();
            assert_eq!(
                String::from_utf8_lossy(&got),
                want,
                "{}/{rel} on {}",
                recipe.id,
                ctx.os()
            );
        }
    }
}

/// Whether any app has settings left on `ctx`.
pub fn any_settings(ctx: &PathContext, recipes: &RecipeSet) -> bool {
    recipes.iter().any(|recipe| {
        let root = ctx.resolve(recipe.location(ctx.os()).unwrap()).unwrap();
        !recipe.collect_files(&root).unwrap().is_empty()
    })
}
