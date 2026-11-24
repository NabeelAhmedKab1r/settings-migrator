//! The safety guard for tests that write fake settings: they run only
//! inside the folder SETTINGS_MIGRATOR_HOME names, which must be inside a
//! temp folder and must not be, or hold, a real home folder. Anything else
//! stops the test before it writes a single file. CI sets it to a folder in
//! the runner's temp folder (see the test job in .github/workflows/ci.yml).

#![allow(dead_code)]

use std::{
    env, fs,
    path::{Component, Path, PathBuf},
};

/// Folders whose direct children are people's home folders.
const HOME_PARENTS: &[&str] = &["/Users", "/home", "/var/home", "C:\\Users", "D:\\Users"];

/// The checked folder from SETTINGS_MIGRATOR_HOME, created if needed.
pub fn test_root() -> PathBuf {
    let raw = env::var_os("SETTINGS_MIGRATOR_HOME").unwrap_or_default();
    if raw.is_empty() {
        refuse("SETTINGS_MIGRATOR_HOME isn't set; set it to a new folder inside your temp folder");
    }
    let root = PathBuf::from(raw);
    if !root.is_absolute() {
        refuse(&format!(
            "SETTINGS_MIGRATOR_HOME must be an absolute path, not {}",
            root.display()
        ));
    }
    let target = norm(&root);
    let real_homes = [env::var_os("HOME"), env::var_os("USERPROFILE")];
    for home in real_homes
        .into_iter()
        .flatten()
        .filter(|home| !home.is_empty())
    {
        let home = norm(Path::new(&home));
        if target == home || home.starts_with(&target) {
            refuse(&format!(
                "{} is, or holds, the real home folder",
                root.display()
            ));
        }
    }
    for parent in HOME_PARENTS.iter().map(|p| norm(Path::new(p))) {
        if parent.starts_with(&target) || target.parent() == Some(parent.as_path()) {
            refuse(&format!(
                "{} is, holds, or looks like a real home folder",
                root.display()
            ));
        }
    }
    let temps = [
        Some(env::temp_dir().into_os_string()),
        env::var_os("RUNNER_TEMP"),
    ];
    let inside_temp = temps
        .into_iter()
        .flatten()
        .filter(|temp| !temp.is_empty())
        .map(|temp| norm(Path::new(&temp)))
        .any(|temp| target != temp && target.starts_with(&temp));
    if !inside_temp {
        refuse(&format!("{} must be inside a temp folder", root.display()));
    }
    fs::create_dir_all(&root).expect("create SETTINGS_MIGRATOR_HOME");
    root
}

/// A new, empty folder inside the checked one, removed when dropped.
pub fn fresh_dir(name: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(&format!("{name}-"))
        .tempdir_in(test_root())
        .expect("make a folder inside SETTINGS_MIGRATOR_HOME")
}

/// Absolute, `..` resolved, lower case (Windows and macOS ignore case by default).
fn norm(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str().to_string_lossy().to_lowercase()),
        }
    }
    out
}

fn refuse(why: &str) -> ! {
    panic!("Refusing to run: {why}.");
}
