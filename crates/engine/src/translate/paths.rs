//! Absolute paths inside settings values. A path in the home folder on the
//! old computer moves to the same place in the home folder here, renaming
//! the folders that differ (Videos ↔ Movies, AppData\Roaming ↔
//! Library/Application Support). Any other path is flagged, since there's
//! no telling where it should point on the other OS.
//!
//! On Windows, JSON gets native backslashes; other text gets forward slashes,
//! which Windows accepts too, because formats such as Git config and Qt INI
//! files treat a backslash as an escape character.

use std::{path::Path, sync::OnceLock};

use regex::Regex;

use super::{count, line_of, Translation};
use crate::Os;

/// Folders under `C:\Users` that aren't anyone's home folder.
const NOT_HOMES: &[&str] = &["Public", "Default", "Default User", "All Users"];

/// Home folder paths named differently on each OS, as (Windows, macOS).
const FOLDERS: &[(&[&str], &[&str])] = &[
    (&["Videos"], &["Movies"]),
    (&["AppData", "Roaming"], &["Library", "Application Support"]),
];

/// Where paths move: from one OS to the home folder on the other.
struct Move<'a> {
    from: Os,
    to: Os,
    /// The new home folder, written with `separator`.
    home: String,
    separator: &'a str,
}

/// A JSON string literal (group 1) or a comment, which is skipped.
fn json_strings() -> &'static Regex {
    static COMPILED: OnceLock<Regex> = OnceLock::new();
    COMPILED.get_or_init(|| Regex::new(r#""((?:[^"\\\n]|\\.)*)"|//[^\n]*|/\*(?s:.*?)\*/"#).unwrap())
}

/// Where an absolute path from `os` starts (group 1): at the start of the
/// text or after a space, quote, `=`, `,`, `;`, `(` or `[`.
fn path_start(os: Os) -> &'static Regex {
    static WINDOWS: OnceLock<Regex> = OnceLock::new();
    static MACOS: OnceLock<Regex> = OnceLock::new();
    match os {
        // A drive (C:\ or C:/) or a network share (\\server\).
        Os::Windows => WINDOWS.get_or_init(|| {
            Regex::new(r#"(?:^|[\s"'=,;(\[])((?:[A-Za-z]:|\\\\[^\\/\s"']+)[\\/])"#).unwrap()
        }),
        // Only the usual top-level folders, so values such as "/api" or
        // "/regex/" aren't taken for paths.
        Os::MacOs => MACOS.get_or_init(|| {
            Regex::new(
                r#"(?:^|[\s"'=,;(\[])(/(?:Users|Volumes|Applications|Library|System|opt|usr|private|tmp|var|etc|bin|sbin)(?:[/\s"']|$))"#,
            )
            .unwrap()
        }),
    }
}

pub(super) fn translate(
    text: &str,
    json: bool,
    from: Os,
    to: Os,
    home: &Path,
    out: &mut Translation,
) -> String {
    let separator = match (to, json) {
        (Os::Windows, true) => "\\",
        _ => "/",
    };
    let mut home = home.to_string_lossy().into_owned();
    if to == Os::Windows {
        home = home.replace(['\\', '/'], separator);
    }
    let target = Move {
        from,
        to,
        home: home.trim_end_matches(separator).to_string(),
        separator,
    };
    let mut moved = 0;
    let mut result = String::with_capacity(text.len());
    if json {
        // Paths in JSON are escaped (C:\\Users), so work on decoded strings.
        let mut copied = 0;
        for caps in json_strings().captures_iter(text) {
            let literal = caps.get(0).unwrap();
            if caps.get(1).is_none() {
                continue;
            }
            let Ok(value) = serde_json::from_str::<String>(literal.as_str()) else {
                continue;
            };
            let line = line_of(text, literal.start());
            if let Some(new) = translate_value(&value, &target, line, out, &mut moved) {
                result.push_str(&text[copied..literal.start()]);
                result.push_str(&serde_json::to_string(&new).expect("strings always serialize"));
                copied = literal.end();
            }
        }
        result.push_str(&text[copied..]);
    } else {
        for (i, line) in text.split_inclusive('\n').enumerate() {
            let body = line.trim_end_matches(['\n', '\r']);
            match translate_value(body, &target, i + 1, out, &mut moved) {
                Some(new) => {
                    result.push_str(&new);
                    result.push_str(&line[body.len()..]);
                }
                None => result.push_str(line),
            }
        }
    }
    if moved > 0 {
        out.changes.push(format!(
            "{} moved into your {to} home folder",
            count(moved, "path")
        ));
    }
    result
}

/// Moves the paths in one value or line. Returns the new text if any path
/// moved; flags the paths that can't.
fn translate_value(
    value: &str,
    target: &Move,
    line: usize,
    out: &mut Translation,
    moved: &mut usize,
) -> Option<String> {
    let mut result = String::new();
    let mut copied = 0;
    let mut checked = 0;
    for caps in path_start(target.from).captures_iter(value) {
        let start = caps.get(1).unwrap().start();
        if start < checked {
            continue;
        }
        // A quoted path ends at its closing quote; otherwise at the end of
        // the value, since paths may contain spaces.
        let quote = value[..start]
            .chars()
            .next_back()
            .filter(|c| *c == '"' || *c == '\'');
        let end = quote
            .and_then(|q| value[start..].find(q))
            .map_or(value.len(), |i| start + i);
        let path = value[start..end].trim_end();
        checked = start + path.len();
        match move_path(path, target) {
            Ok(new) => {
                result.push_str(&value[copied..start]);
                result.push_str(&new);
                copied = checked;
                *moved += 1;
            }
            Err(why) => out.flag(line, format!("`{path}` {why}; left as it was")),
        }
    }
    if copied == 0 {
        return None;
    }
    result.push_str(&value[copied..]);
    Some(result)
}

/// The path in `home` matching `path` from the old home folder, or why there
/// isn't one.
fn move_path(path: &str, target: &Move) -> Result<String, String> {
    let Move { from, to, .. } = *target;
    let parts: Vec<&str> = path
        .split(['\\', '/'])
        .filter(|part| !part.is_empty())
        .collect();
    let rest = match (from, parts.as_slice()) {
        (Os::Windows, [drive, users, name, rest @ ..])
            if drive.ends_with(':')
                && users.eq_ignore_ascii_case("Users")
                && !NOT_HOMES.iter().any(|n| n.eq_ignore_ascii_case(name)) =>
        {
            rest
        }
        (Os::MacOs, ["Users", name, rest @ ..]) if *name != "Shared" => rest,
        _ => {
            return Err(format!(
                "is outside your home folder, so there's no matching place on {to}"
            ))
        }
    };

    let mut moved: Vec<&str> = Vec::new();
    for (windows, macos) in FOLDERS {
        let (old, new) = match from {
            Os::Windows => (windows, macos),
            Os::MacOs => (macos, windows),
        };
        let matches = rest.len() >= old.len()
            && rest
                .iter()
                .zip(old.iter())
                .all(|(a, b)| a.eq_ignore_ascii_case(b));
        if matches {
            moved.extend(new.iter());
            moved.extend(&rest[old.len()..]);
            break;
        }
    }
    if moved.is_empty() {
        let app_data = match from {
            Os::Windows => "AppData",
            Os::MacOs => "Library",
        };
        if rest
            .first()
            .is_some_and(|first| first.eq_ignore_ascii_case(app_data))
        {
            return Err(format!(
                "is in an app data folder that {to} lays out differently"
            ));
        }
        moved.extend(rest);
    }

    let mut new = target.home.clone();
    for part in moved {
        new.push_str(target.separator);
        new.push_str(part);
    }
    Ok(new)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str, json: bool, from: Os) -> (String, Translation) {
        let (to, home) = match from {
            Os::Windows => (Os::MacOs, "/Users/alex"),
            Os::MacOs => (Os::Windows, r"C:\Users\Alex"),
        };
        let mut out = Translation::unchanged(b"");
        let text = translate(text, json, from, to, Path::new(home), &mut out);
        (text, out)
    }

    #[test]
    fn windows_home_paths_move_to_macos() {
        let json = r#"{
  // C:\Users\Sam\in-a-comment is left alone
  "vault": {"path": "C:\\Users\\Sam\\Documents\\My Notes", "open": true},
  "clip": "c:/users/sam/videos/intro.mp4",
  "roaming": "C:\\Users\\Sam\\AppData\\Roaming\\Code\\x.json",
  "url": "https://example.com/C:/Users/x"
}"#;
        let (text, out) = run(json, true, Os::Windows);
        assert_eq!(
            text,
            r#"{
  // C:\Users\Sam\in-a-comment is left alone
  "vault": {"path": "/Users/alex/Documents/My Notes", "open": true},
  "clip": "/Users/alex/Movies/intro.mp4",
  "roaming": "/Users/alex/Library/Application Support/Code/x.json",
  "url": "https://example.com/C:/Users/x"
}"#
        );
        assert_eq!(out.changes, ["3 paths moved into your macOS home folder"]);
        assert!(out.flags.is_empty(), "{:?}", out.flags);
    }

    #[test]
    fn macos_home_paths_move_to_windows() {
        let ini = "[SimpleOutput]\nFilePath=/Users/sam/Movies\nother=/Users/sam\n";
        let (text, out) = run(ini, false, Os::MacOs);
        assert_eq!(
            text,
            "[SimpleOutput]\nFilePath=C:/Users/Alex/Videos\nother=C:/Users/Alex\n"
        );
        assert_eq!(out.changes, ["2 paths moved into your Windows home folder"]);

        // In JSON the backslashes are escaped.
        let (text, _) = run(r#"{"path": "/Users/sam/Documents/Notes"}"#, true, Os::MacOs);
        assert_eq!(text, r#"{"path": "C:\\Users\\Alex\\Documents\\Notes"}"#);
    }

    #[test]
    fn paths_outside_the_home_folder_are_flagged() {
        let gitconfig = "[core]\n\teditor = 'C:/Program Files/Notepad++/notepad++.exe' -multiInst\n\texcludesfile = C:/Users/Sam/.gitignore_global\n";
        let (text, out) = run(gitconfig, false, Os::Windows);
        assert_eq!(
            text,
            "[core]\n\teditor = 'C:/Program Files/Notepad++/notepad++.exe' -multiInst\n\texcludesfile = /Users/alex/.gitignore_global\n"
        );
        assert_eq!(out.flags.len(), 1);
        assert_eq!(out.flags[0].line, 2);
        assert_eq!(
            out.flags[0].message,
            "`C:/Program Files/Notepad++/notepad++.exe` is outside your home folder, so there's no matching place on macOS; left as it was"
        );

        let flagged = [
            (r#"{"a": "D:\\Vaults\\Work"}"#, Os::Windows, "outside"),
            (r#"{"a": "\\\\server\\share\\x"}"#, Os::Windows, "outside"),
            (r#"{"a": "C:\\Users\\Public\\x"}"#, Os::Windows, "outside"),
            (
                r#"{"a": "C:\\Users\\Sam\\AppData\\Local\\x"}"#,
                Os::Windows,
                "app data",
            ),
            (r#"{"a": "/Volumes/Backup/Work"}"#, Os::MacOs, "outside"),
            (r#"{"a": "/opt/homebrew/bin/git"}"#, Os::MacOs, "outside"),
            (r#"{"a": "/Users/Shared/x"}"#, Os::MacOs, "outside"),
            (
                r#"{"a": "/Users/sam/Library/Caches/x"}"#,
                Os::MacOs,
                "app data",
            ),
        ];
        for (json, from, why) in flagged {
            let (text, out) = run(json, true, from);
            assert_eq!(text, json);
            assert_eq!(out.flags.len(), 1, "{json}");
            assert!(out.flags[0].message.contains(why), "{json}");
        }
    }

    #[test]
    fn things_that_merely_look_like_paths_are_ignored() {
        for (text, from) in [
            (
                r#"{"route": "/api/users", "glob": "**/node_modules", "url": "file:///Users/sam"}"#,
                Os::MacOs,
            ),
            (r#"{"ratio": "16:9", "time": "12:30/45"}"#, Os::Windows),
        ] {
            let (out_text, out) = run(text, true, from);
            assert_eq!(out_text, text);
            assert!(out.flags.is_empty() && out.changes.is_empty(), "{text}");
        }
    }
}
