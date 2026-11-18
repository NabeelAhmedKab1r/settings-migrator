//! Restores a bundle made on the other OS with the real binary, into a fake
//! home. In CI, the cross-OS jobs capture the sample home on one runner and
//! hand the bundle to a runner of the other OS through `CROSS_OS_BUNDLE`.
//! Without it, the bundle is made here from the other OS's sample home.

#[path = "../../engine/tests/common/cross_os.rs"]
mod cross_os_fixtures;

use std::{env, path::Path, path::PathBuf, process::Command};

use cross_os_fixtures::{any_settings, check_restored, expected_flags, other, source_home};
use settings_migrator_engine::{
    bundle::{self, Bundle, CaptureOptions},
    Os, PathContext, RecipeSet,
};
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

#[test]
fn restores_a_bundle_from_the_other_os() {
    let here = Os::current().unwrap();
    let recipes = RecipeSet::builtin().unwrap();
    let work = TempDir::new().unwrap();
    let bundle_path = match env::var_os("CROSS_OS_BUNDLE") {
        Some(path) => PathBuf::from(path),
        None => {
            let path = work.path().join("bundle.zip");
            let source = PathContext::for_home(other(here), source_home(other(here)));
            bundle::capture(
                &source,
                &recipes.select(&[]).unwrap(),
                &CaptureOptions::default(),
                &path,
            )
            .unwrap();
            path
        }
    };
    let opened = Bundle::open(&bundle_path).unwrap();
    assert_eq!(
        opened.manifest.source_os,
        other(here),
        "the bundle must come from the other OS"
    );
    assert_eq!(opened.apps.len(), recipes.iter().count());
    let bundle_arg = bundle_path.to_str().unwrap();

    // The preview says what's translated and flags what isn't, and writes nothing.
    let home = TempDir::new().unwrap();
    let ctx = PathContext::for_home(here, home.path());
    let (ok, out) = run(home.path(), &["restore", bundle_arg, "--dry-run"]);
    assert!(ok, "{out}");
    assert!(
        out.contains(&format!("This bundle was made on {}", other(here))),
        "{out}"
    );
    assert!(out.contains("translated: 3 shortcuts"), "{out}");
    for (_, _, line, part) in expected_flags(here) {
        assert!(out.contains(&format!("! line {line}: ")), "{out}");
        assert!(out.contains(part), "missing {part:?} in:\n{out}");
    }
    assert!(
        out.contains(&format!(
            "Not translated: {} items",
            expected_flags(here).len()
        )),
        "{out}"
    );
    assert!(!any_settings(&ctx, &recipes));

    let (ok, out) = run(home.path(), &["restore", bundle_arg, "--yes"]);
    assert!(ok, "{out}");
    check_restored(&ctx, &recipes);

    let (ok, out) = run(home.path(), &["undo"]);
    assert!(ok, "{out}");
    assert!(!any_settings(&ctx, &recipes));
}
