//! Runs the real binary end to end against fake home folders.

use std::{fs, path::Path, process::Command};

use settings_migrator_engine::{paths::join_relative, Os, PathContext, RecipeSet};
use tempfile::TempDir;

fn run(home: &Path, args: &[&str]) -> (bool, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_settings-migrator"))
        .arg("--home")
        .arg(home)
        .args(args)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&output.stdout).into_owned()
        + &String::from_utf8_lossy(&output.stderr);
    (output.status.success(), text)
}

fn vscode_settings(home: &Path) -> std::path::PathBuf {
    let ctx = PathContext::for_home(Os::current().unwrap(), home);
    let recipes = RecipeSet::builtin().unwrap();
    let location = recipes.get("vscode").unwrap().location(ctx.os()).unwrap();
    join_relative(&ctx.resolve(location).unwrap(), "settings.json").unwrap()
}

#[test]
fn capture_restore_undo_from_the_command_line() {
    let old = TempDir::new().unwrap();
    let new = TempDir::new().unwrap();
    let work = TempDir::new().unwrap();
    let bundle = work.path().join("settings.zip");
    let bundle = bundle.to_str().unwrap();

    let source = vscode_settings(old.path());
    fs::create_dir_all(source.parent().unwrap()).unwrap();
    fs::write(&source, b"{\"editor.fontSize\": 15}").unwrap();

    let (ok, out) = run(old.path(), &["scan"]);
    assert!(ok && out.contains("settings.json"), "{out}");

    let (ok, out) = run(old.path(), &["capture", "-o", bundle, "--dry-run"]);
    assert!(ok && out.contains("Nothing was written"), "{out}");
    assert!(!Path::new(bundle).exists());

    let (ok, out) = run(old.path(), &["capture", "-o", bundle]);
    assert!(ok && out.contains("Captured 1 file"), "{out}");

    let target = vscode_settings(new.path());
    let (ok, out) = run(new.path(), &["restore", bundle, "--dry-run"]);
    assert!(ok && out.contains("create"), "{out}");
    assert!(!target.exists());

    // Without a terminal to confirm on, restore shows the preview and refuses.
    let (ok, out) = run(new.path(), &["restore", bundle]);
    assert!(
        !ok && out.contains("+{\"editor.fontSize\": 15}") && out.contains("--yes"),
        "{out}"
    );
    assert!(!target.exists());

    let (ok, out) = run(new.path(), &["restore", bundle, "--yes"]);
    assert!(ok, "{out}");
    assert_eq!(fs::read(&target).unwrap(), b"{\"editor.fontSize\": 15}");

    let (ok, out) = run(new.path(), &["undo", "--dry-run"]);
    assert!(ok && out.contains("delete"), "{out}");
    assert!(target.exists());

    let (ok, out) = run(new.path(), &["undo"]);
    assert!(ok, "{out}");
    assert!(!target.exists());

    let (ok, out) = run(new.path(), &["undo"]);
    assert!(ok && out.contains("Nothing to undo"), "{out}");
}

#[test]
fn help_hides_the_testing_flag() {
    let output = Command::new(env!("CARGO_BIN_EXE_settings-migrator"))
        .arg("--help")
        .output()
        .unwrap();
    let help = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success() && help.contains("--recipes"),
        "{help}"
    );
    assert!(!help.contains("--home"), "{help}");
}

#[test]
fn flagged_secrets_block_capture_until_reviewed() {
    let home = TempDir::new().unwrap();
    let work = TempDir::new().unwrap();
    let bundle = work.path().join("settings.zip");
    let bundle = bundle.to_str().unwrap();

    // A fake token, built at runtime so this file never contains one.
    let token: String = std::iter::once("ghp_".to_string())
        .chain(std::iter::repeat("aB3dE5fG7h".to_string()).take(4))
        .collect();
    let settings = vscode_settings(home.path());
    fs::create_dir_all(settings.parent().unwrap()).unwrap();
    fs::write(
        &settings,
        format!("{{\"env\": {{\"GITHUB_TOKEN\": \"{token}\"}}}}"),
    )
    .unwrap();

    let (ok, out) = run(home.path(), &["capture", "-o", bundle]);
    assert!(!ok && out.contains("GitHub token"), "{out}");
    assert!(
        !out.contains(&token[4..]),
        "the review list must not print the secret"
    );
    assert!(!Path::new(bundle).exists());

    let (ok, out) = run(home.path(), &["capture", "-o", bundle, "--dry-run"]);
    assert!(ok && out.contains("FLAGGED"), "{out}");

    let (ok, out) = run(
        home.path(),
        &["capture", "-o", bundle, "--allow", "vscode/settings.json"],
    );
    assert!(ok && Path::new(bundle).exists(), "{out}");
}
