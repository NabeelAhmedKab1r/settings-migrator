//! `*` in one folder name of a recipe path, for apps that keep settings in
//! folders whose names differ between computers: a random Firefox profile
//! name, a version such as `IntelliJIdea2025.2` or Blender's `4.3`.
//!
//! Capturing keeps each matching folder's real name, but only the newest
//! version of each product (older version folders are usually left behind
//! by updates). Restoring puts files into the matching folder that exists
//! on this computer; when there isn't a clear one, the file is skipped with
//! the reason, never put somewhere the app won't look.

use std::{cmp::Ordering, fs, path::Path};

use crate::{paths::join_relative, Error, Result};

/// Whether `name` matches `pattern`, where `*` stands for any run of
/// characters (possibly none). Case matters, as folder names are compared
/// as written.
pub fn matches(pattern: &str, name: &str) -> bool {
    let mut parts = pattern.split('*');
    let first = parts.next().unwrap_or("");
    let Some(mut rest) = name.strip_prefix(first) else {
        return false;
    };
    let parts: Vec<&str> = parts.collect();
    let Some((last, middle)) = parts.split_last() else {
        return rest.is_empty(); // No `*` at all: an exact match.
    };
    for part in middle {
        match rest.find(part) {
            Some(at) => rest = &rest[at + part.len()..],
            None => return false,
        }
    }
    rest.len() >= last.len() && rest.ends_with(last)
}

/// The folders directly in `dir` whose names match `pattern`, sorted.
/// Symbolic links are left out, so a capture never wanders elsewhere.
pub fn matching_dirs(dir: &Path, pattern: &str) -> Result<Vec<String>> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(Error::io(dir, e)),
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| Error::io(dir, e))?;
        let is_dir = entry
            .file_type()
            .map_err(|e| Error::io(entry.path(), e))?
            .is_dir();
        if let Some(name) = entry.file_name().to_str() {
            if is_dir && matches(pattern, name) {
                names.push(name.to_string());
            }
        }
    }
    names.sort();
    Ok(names)
}

/// A folder name without its trailing version: `IntelliJIdea` for
/// `IntelliJIdea2025.2`, empty for Blender's `4.3`.
fn product(name: &str) -> &str {
    name.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.')
}

/// Compares the trailing versions of two folder names, number by number,
/// so `2025.10` is newer than `2025.9`.
fn compare_versions(a: &str, b: &str) -> Ordering {
    let numbers = |name: &str| -> Vec<u64> {
        name[product(name).len()..]
            .split('.')
            .filter(|part| !part.is_empty())
            .map(|part| part.parse().unwrap_or(u64::MAX))
            .collect()
    };
    numbers(a).cmp(&numbers(b)).then_with(|| a.cmp(b))
}

/// The newest of `names`.
fn newest<'a>(names: impl IntoIterator<Item = &'a String>) -> Option<&'a String> {
    names.into_iter().max_by(|a, b| compare_versions(a, b))
}

/// Of the folders found while capturing, the newest version of each product.
pub fn newest_per_product(names: Vec<String>) -> Vec<String> {
    let mut kept: Vec<String> = Vec::new();
    for name in &names {
        let same: Vec<&String> = names
            .iter()
            .filter(|other| product(other) == product(name))
            .collect();
        if newest(same) == Some(name) {
            kept.push(name.clone());
        }
    }
    kept
}

/// Where a file from a bundle goes when its path has a `*` folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Placement {
    /// The relative path to write, with a note when the folder differs
    /// from the one it was saved from, and a warning when it's older.
    Into {
        rel: String,
        change: Option<String>,
        warning: Option<String>,
    },
    /// Not restored, for this reason.
    Skip(String),
}

/// Works out where `rel` (saved from a folder matching `segments[index]`)
/// goes under `root` on this computer.
pub fn place(root: &Path, segments: &[&str], index: usize, rel: &str) -> Result<Placement> {
    let parts: Vec<&str> = rel.split('/').collect();
    let saved = parts[index];
    let pattern = segments[index];
    let parent = join_relative(root, &parts[..index].join("/"))?;
    let here = matching_dirs(&parent, pattern)?;
    let with_folder = |name: &str| {
        let mut parts = parts.clone();
        parts[index] = name;
        parts.join("/")
    };

    if here.iter().any(|name| name == saved) {
        return Ok(Placement::Into {
            rel: rel.to_string(),
            change: None,
            warning: None,
        });
    }
    let same_product: Vec<&String> = here
        .iter()
        .filter(|name| product(name) == product(saved))
        .collect();
    let chosen = match (newest(same_product), here.as_slice()) {
        (Some(name), _) => name,
        (None, [only]) => only,
        (None, []) => {
            return Ok(Placement::Skip(format!(
                "there's no {pattern} folder here yet (saved from {saved}); open the app once, then restore again"
            )))
        }
        (None, several) => {
            return Ok(Placement::Skip(format!(
                "several folders could hold it ({}), so it's unclear which one; saved from {saved}",
                several.join(", ")
            )))
        }
    };
    let older =
        product(chosen) == product(saved) && compare_versions(chosen, saved) == Ordering::Less;
    Ok(Placement::Into {
        rel: with_folder(chosen),
        change: Some(format!("put in {chosen}, the folder on this computer (saved from {saved})")),
        warning: older.then(|| {
            format!("saved from a newer version ({saved}) than the one here ({chosen}); it may not read every setting")
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn star_matches_any_run_of_characters() {
        assert!(matches("*", "IntelliJIdea2025.2"));
        assert!(matches("*.default-release", "k3x9.default-release"));
        assert!(!matches("*.default-release", "k3x9.default"));
        assert!(matches("PyCharm*", "PyCharm2025.1"));
        assert!(matches("a*b*c", "abc"));
        assert!(!matches("a*b*c", "acb"));
        assert!(matches("exact", "exact"));
        assert!(!matches("exact", "exactly"));
        assert!(!matches("ab*ba", "aba"));
    }

    #[test]
    fn keeps_the_newest_version_of_each_product() {
        let found = [
            "IntelliJIdea2024.3",
            "IntelliJIdea2025.10",
            "IntelliJIdea2025.9",
            "PyCharm2025.1",
        ];
        let kept = newest_per_product(found.iter().map(|s| s.to_string()).collect());
        assert_eq!(kept, ["IntelliJIdea2025.10", "PyCharm2025.1"]);
        let blender = newest_per_product(vec!["4.2".into(), "4.3".into()]);
        assert_eq!(blender, ["4.3"]);
    }

    fn home(dirs: &[&str]) -> TempDir {
        let temp = TempDir::new().unwrap();
        for dir in dirs {
            fs::create_dir_all(temp.path().join(dir)).unwrap();
        }
        temp
    }

    fn placed(rel: &str) -> Placement {
        Placement::Into {
            rel: rel.into(),
            change: None,
            warning: None,
        }
    }

    #[test]
    fn restores_into_the_same_folder_when_it_exists() {
        let temp = home(&["JetBrains/PyCharm2025.1", "JetBrains/IntelliJIdea2025.2"]);
        let got = place(
            temp.path(),
            &["JetBrains", "*", "options/"],
            1,
            "JetBrains/PyCharm2025.1/options/ui.xml",
        );
        assert_eq!(
            got.unwrap(),
            placed("JetBrains/PyCharm2025.1/options/ui.xml")
        );
    }

    #[test]
    fn restores_into_the_newest_version_of_the_same_product() {
        let temp = home(&[
            "JetBrains/IntelliJIdea2025.2",
            "JetBrains/IntelliJIdea2025.3",
            "JetBrains/PyCharm2025.3",
        ]);
        let got = place(
            temp.path(),
            &["JetBrains", "*", "options/"],
            1,
            "JetBrains/IntelliJIdea2025.1/options/ui.xml",
        );
        let Placement::Into {
            rel,
            change,
            warning,
        } = got.unwrap()
        else {
            panic!()
        };
        assert_eq!(rel, "JetBrains/IntelliJIdea2025.3/options/ui.xml");
        assert!(change.unwrap().contains("saved from IntelliJIdea2025.1"));
        assert_eq!(warning, None);
    }

    #[test]
    fn warns_when_the_version_here_is_older() {
        let temp = home(&["4.2/config"]);
        let got = place(
            temp.path(),
            &["*", "config/"],
            0,
            "4.3/config/userpref.blend",
        );
        let Placement::Into { rel, warning, .. } = got.unwrap() else {
            panic!()
        };
        assert_eq!(rel, "4.2/config/userpref.blend");
        assert!(warning.unwrap().contains("newer version (4.3)"));
    }

    #[test]
    fn uses_the_only_matching_folder() {
        let temp = home(&["Profiles/ab12.default-release"]);
        let got = place(
            temp.path(),
            &["Profiles", "*.default-release", "chrome/"],
            1,
            "Profiles/k3x9.default-release/chrome/userChrome.css",
        );
        let Placement::Into { rel, .. } = got.unwrap() else {
            panic!()
        };
        assert_eq!(rel, "Profiles/ab12.default-release/chrome/userChrome.css");
    }

    #[test]
    fn skips_when_there_is_no_clear_folder() {
        let none = home(&[]);
        let got = place(
            none.path(),
            &["Profiles", "*.default-release", "user.js"],
            1,
            "Profiles/k3x9.default-release/user.js",
        );
        assert!(matches!(got.unwrap(), Placement::Skip(why) if why.contains("open the app once")));

        let two = home(&["Profiles/a.default-release", "Profiles/b.default-release"]);
        let got = place(
            two.path(),
            &["Profiles", "*.default-release", "user.js"],
            1,
            "Profiles/k3x9.default-release/user.js",
        );
        assert!(matches!(got.unwrap(), Placement::Skip(why) if why.contains("several folders")));
    }
}
