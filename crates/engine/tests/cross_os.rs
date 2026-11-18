//! Moving settings between Windows and macOS: capture the sample home of one
//! OS, restore it into a fake home laid out for the other, and compare with
//! the expected files. Both directions run on every OS; CI also does it
//! across real Windows and macOS runners (see `crates/cli/tests/cross_os.rs`).

#[path = "common/cross_os.rs"]
mod cross_os_fixtures;

use cross_os_fixtures::{any_settings, check_restored, expected_flags, other, source_home};
use settings_migrator_engine::{
    bundle::{self, Bundle, CaptureOptions},
    process::ProcessList,
    restore::{self, Action},
    undo::UndoSet,
    Os, PathContext, RecipeSet,
};
use tempfile::TempDir;

#[test]
fn windows_to_macos() {
    move_settings(Os::Windows, Os::MacOs, "");
}

#[test]
fn macos_to_windows() {
    move_settings(Os::MacOs, Os::Windows, "");
}

/// On a Windows runner the stand-in Mac home is a Windows temp folder, with
/// backslashes in its path. This gives it backslashes on every OS, so a
/// mistake that only shows on Windows shows everywhere.
#[test]
fn windows_to_macos_with_backslashes_in_the_home_path() {
    move_settings(Os::Windows, Os::MacOs, r"Users\Sam");
}

/// Moves the sample settings from `from` to a fake home for `to`: a temp
/// folder, or the folder `home` inside it.
fn move_settings(from: Os, to: Os, home: &str) {
    let recipes = RecipeSet::builtin().unwrap();
    let source = PathContext::for_home(from, source_home(from));
    let work = TempDir::new().unwrap();
    let path = work.path().join("bundle.zip");
    // The samples hold no secrets, so nothing needs reviewing.
    bundle::capture(
        &source,
        &recipes.select(&[]).unwrap(),
        &CaptureOptions::default(),
        &path,
    )
    .unwrap();
    let opened = Bundle::open(&path).unwrap();
    assert_eq!(opened.manifest.source_os, from);
    assert_eq!(
        opened.apps.len(),
        recipes.iter().count(),
        "every recipe has samples"
    );

    let target = TempDir::new().unwrap();
    let home = match home {
        "" => target.path().to_path_buf(),
        name => target.path().join(name),
    };
    let ctx = PathContext::for_home(to, home);
    let plan = restore::plan(&ctx, &recipes, &opened, &[]).unwrap();
    assert!(plan.is_cross_os());

    // Everything untranslatable is flagged, and nothing else.
    let mut flags: Vec<_> = plan
        .writes()
        .flat_map(|item| {
            item.flags.iter().map(move |flag| {
                (
                    item.app_id.clone(),
                    item.rel.clone(),
                    flag.line,
                    flag.message.clone(),
                )
            })
        })
        .collect();
    flags.sort();
    let expected = expected_flags(to);
    assert_eq!(flags.len(), expected.len(), "{flags:#?}");
    for ((app, rel, line, message), (want_app, want_rel, want_line, part)) in
        flags.iter().zip(expected)
    {
        assert_eq!(
            (app.as_str(), rel.as_str(), *line),
            (*want_app, *want_rel, *want_line),
            "{message}"
        );
        assert!(
            message.contains(part),
            "{message:?} should contain {part:?}"
        );
    }

    // Windows-only files are skipped on a Mac, and say why.
    let skipped: Vec<_> = plan
        .items
        .iter()
        .filter_map(|item| match &item.action {
            Action::Skip(reason) => Some(format!("{}/{}: {reason}", item.app_id, item.rel)),
            _ => None,
        })
        .collect();
    match to {
        Os::MacOs => assert_eq!(skipped, ["vlc/vlc-qt-interface.ini: only used on Windows"]),
        Os::Windows => assert_eq!(skipped, Vec::<String>::new()),
    }

    // What was translated is listed per file.
    let changes = |app: &str, rel: &str| {
        plan.items
            .iter()
            .find(|item| item.app_id == app && item.rel == rel)
            .unwrap()
            .changes
            .clone()
    };
    let (ctrl, cmd) = match from {
        Os::Windows => ("Ctrl", "Cmd"),
        Os::MacOs => ("Cmd", "Ctrl"),
    };
    assert!(changes("vscode", "keybindings.json").contains(&format!("3 shortcuts: {ctrl} → {cmd}")));
    assert!(changes("obsidian", "obsidian.json")
        .contains(&format!("1 path moved into your {to} home folder")));
    assert_eq!(
        changes("obsidian", "Custom Dictionary.txt"),
        Vec::<String>::new()
    );

    // Restore, then compare with the expected files.
    let none_open = ProcessList::from_names(to, Vec::<String>::new());
    restore::apply(&ctx, &recipes, &plan, "bundle.zip", &none_open).unwrap();
    check_restored(&ctx, &recipes);

    // Undo works the same as for a same-OS restore.
    UndoSet::latest(&ctx)
        .unwrap()
        .unwrap()
        .apply(&recipes, &none_open)
        .unwrap();
    assert!(!any_settings(&ctx, &recipes));
}

#[test]
fn recipes_without_cross_os_are_skipped_on_the_other_os() {
    let recipes_dir = TempDir::new().unwrap();
    std::fs::write(
        recipes_dir.path().join("git.yaml"),
        "id: git\nname: Git\nlocations: {windows: '{home}', macos: '{home}'}\nfiles: [{path: .gitconfig}]\ncross_os: false\n",
    )
    .unwrap();
    let recipes = RecipeSet::load_dir(recipes_dir.path()).unwrap();

    let os = Os::current().unwrap();
    let source = PathContext::for_home(other(os), source_home(other(os)));
    let work = TempDir::new().unwrap();
    let path = work.path().join("bundle.zip");
    bundle::capture(
        &source,
        &recipes.select(&[]).unwrap(),
        &CaptureOptions::default(),
        &path,
    )
    .unwrap();

    let target = TempDir::new().unwrap();
    let ctx = PathContext::for_home(os, target.path());
    let plan = restore::plan(&ctx, &recipes, &Bundle::open(&path).unwrap(), &[]).unwrap();
    assert_eq!(plan.items.len(), 1);
    assert_eq!(
        plan.items[0].action,
        Action::Skip(format!(
            "the recipe doesn't support moving settings from {} to {os}",
            other(os)
        ))
    );
    assert_eq!(plan.writes().count(), 0);
}
