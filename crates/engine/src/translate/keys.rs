//! Ctrl/Cmd in VS Code-style keybinding files, where each binding has a
//! `"key"` such as `"ctrl+shift+p"`, or a chord such as `"ctrl+k ctrl+c"`.
//!
//! Windows Ctrl becomes macOS Cmd and back. VS Code calls Alt and Option
//! both `alt`, so they keep their name; what can't move is flagged: the
//! Windows key (macOS has none), Cmd and Ctrl together (both are Ctrl on
//! Windows), and shortcuts the target OS keeps for itself.

use std::{collections::HashMap, sync::OnceLock};

use regex::Regex;

use super::{count, line_of, Translation};
use crate::Os;

/// Shortcuts macOS and Windows take before an app sees them.
const RESERVED_MACOS: &[&str] = &[
    "cmd+tab",
    "cmd+shift+tab",
    "cmd+space",
    "cmd+alt+space",
    "cmd+h",
    "cmd+alt+h",
    "cmd+m",
    "cmd+alt+escape",
    "cmd+shift+3",
    "cmd+shift+4",
    "cmd+shift+5",
    "cmd+ctrl+q",
];
const RESERVED_WINDOWS: &[&str] = &[
    "alt+tab",
    "alt+shift+tab",
    "alt+escape",
    "ctrl+escape",
    "ctrl+shift+escape",
    "ctrl+alt+delete",
];

/// A `"key"` or `"when"` value (groups 1 and 2). Other strings and comments
/// match too, without groups, so text inside them is never mistaken for a
/// binding.
fn key_pattern() -> &'static Regex {
    static COMPILED: OnceLock<Regex> = OnceLock::new();
    COMPILED.get_or_init(|| {
        Regex::new(concat!(
            r#""(key|when)"[ \t]*:[ \t]*"((?:[^"\\\n]|\\.)*)""#,
            r#"|"(?:[^"\\\n]|\\.)*"|//[^\n]*|/\*(?s:.*?)\*/"#,
        ))
        .unwrap()
    })
}

fn os_context() -> &'static Regex {
    static COMPILED: OnceLock<Regex> = OnceLock::new();
    COMPILED.get_or_init(|| Regex::new(r"\b(?:isWindows|isMac|isLinux)\b").unwrap())
}

/// Why a shortcut was left as it was.
enum Untranslatable {
    WindowsKey,
    CmdAndCtrl,
    Reserved(String),
}

pub(super) fn translate(text: &str, from: Os, to: Os, out: &mut Translation) -> String {
    let mut result = String::with_capacity(text.len());
    let mut copied = 0;
    let mut changed = 0;
    // Each shortcut after translation, with where it came from, to catch
    // two different shortcuts becoming the same one.
    let mut seen: HashMap<String, (String, usize)> = HashMap::new();

    for caps in key_pattern().captures_iter(text) {
        let Some(value) = caps.get(2) else {
            continue;
        };
        let line = line_of(text, value.start());
        let original = value.as_str();
        if &caps[1] == "when" {
            if let Some(m) = os_context().find(original) {
                out.flag(
                    line,
                    format!(
                        "a shortcut here depends on the OS (`{}`); check it does what you want on {to}",
                        m.as_str()
                    ),
                );
            }
            continue;
        }

        let translated = match translate_key(original, from, to) {
            Ok(translated) => translated,
            Err(reason) => {
                let message = match reason {
                    Untranslatable::WindowsKey => format!(
                        "`{original}` uses the Windows key, which macOS doesn't have; left as it was"
                    ),
                    Untranslatable::CmdAndCtrl => format!(
                        "`{original}` uses both Cmd and Ctrl, which are both Ctrl on Windows; left as it was"
                    ),
                    Untranslatable::Reserved(key) if key == original => format!(
                        "`{original}` is kept by {to} for itself, so VS Code won't see it"
                    ),
                    Untranslatable::Reserved(key) => format!(
                        "`{original}` would become `{key}`, which {to} keeps for itself; left as it was"
                    ),
                };
                out.flag(line, message);
                continue;
            }
        };

        match seen.get(&translated.to_ascii_lowercase()) {
            Some((first, first_line)) if !first.eq_ignore_ascii_case(original) => {
                let clash = if translated == original {
                    format!("`{original}` is what `{first}` on line {first_line} becomes")
                } else {
                    format!("`{original}` becomes `{translated}`, the same as `{first}` on line {first_line}")
                };
                out.flag(line, format!("{clash}; VS Code uses whichever comes last"));
            }
            _ => {
                seen.insert(
                    translated.to_ascii_lowercase(),
                    (original.to_string(), line),
                );
            }
        }
        if translated != original {
            changed += 1;
            result.push_str(&text[copied..value.start()]);
            result.push_str(&translated);
            copied = value.end();
        }
    }
    result.push_str(&text[copied..]);

    if changed > 0 {
        let (old, new) = match from {
            Os::Windows => ("Ctrl", "Cmd"),
            Os::MacOs => ("Cmd", "Ctrl"),
        };
        out.changes
            .push(format!("{}: {old} → {new}", count(changed, "shortcut")));
    }
    result
}

/// One shortcut, such as `ctrl+k ctrl+c`, as it should be on `to`.
fn translate_key(key: &str, from: Os, to: Os) -> Result<String, Untranslatable> {
    let mut chords = Vec::new();
    for chord in key.split(' ').filter(|chord| !chord.is_empty()) {
        let (modifiers, last) = split_chord(chord);
        let has = |name: &str| modifiers.iter().any(|m| m.eq_ignore_ascii_case(name));
        let mut mapped = Vec::new();
        for modifier in &modifiers {
            let lower = modifier.to_ascii_lowercase();
            mapped.push(match (from, lower.as_str()) {
                (Os::Windows, "ctrl") => "cmd",
                (Os::Windows, "win" | "meta") => return Err(Untranslatable::WindowsKey),
                (Os::MacOs, "ctrl") if has("cmd") || has("meta") => {
                    return Err(Untranslatable::CmdAndCtrl)
                }
                (Os::MacOs, "cmd" | "meta") => "ctrl",
                _ => modifier,
            });
        }
        mapped.push(last);
        let chord = mapped.join("+");
        if is_reserved(&chord, to) {
            return Err(Untranslatable::Reserved(chord));
        }
        chords.push(chord);
    }
    Ok(chords.join(" "))
}

/// Splits `ctrl+shift+p` into its modifiers and key. The key itself may be
/// `+`, as in `ctrl++`.
fn split_chord(chord: &str) -> (Vec<&str>, &str) {
    let (modifiers, key) = match chord.strip_suffix("++") {
        Some(modifiers) => (modifiers, "+"),
        None => chord.rsplit_once('+').unwrap_or(("", chord)),
    };
    let modifiers = modifiers.split('+').filter(|m| !m.is_empty()).collect();
    (modifiers, key)
}

fn is_reserved(chord: &str, os: Os) -> bool {
    let list = match os {
        Os::MacOs => RESERVED_MACOS,
        Os::Windows => RESERVED_WINDOWS,
    };
    let normal = normalize(chord);
    list.iter().any(|reserved| normalize(reserved) == normal)
}

/// Modifiers in a fixed order, lowercase, so `shift+cmd+Tab` equals `cmd+shift+tab`.
fn normalize(chord: &str) -> (Vec<String>, String) {
    let (modifiers, key) = split_chord(chord);
    let mut modifiers: Vec<String> = modifiers.iter().map(|m| m.to_ascii_lowercase()).collect();
    modifiers.sort();
    let key = match key.to_ascii_lowercase().as_str() {
        "esc" => "escape".to_string(),
        "del" => "delete".to_string(),
        other => other.to_string(),
    };
    (modifiers, key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str, from: Os) -> Translation {
        let to = match from {
            Os::Windows => Os::MacOs,
            Os::MacOs => Os::Windows,
        };
        let mut out = Translation::unchanged(b"");
        out.contents = translate(text, from, to, &mut out).into_bytes();
        out
    }

    fn text(t: &Translation) -> &str {
        std::str::from_utf8(&t.contents).unwrap()
    }

    #[test]
    fn ctrl_becomes_cmd_and_back() {
        let windows = r#"// A comment is left alone: "key": "ctrl+a"
[
    { "key": "ctrl+shift+alt+d", "command": "a" },
    { "key": "ctrl+k ctrl+t", "command": "b" },
    { "key": "alt+up", "command": "c", "when": "editorTextFocus" },
    {"key":"Ctrl+=","command":"d"},
    { "key": "ctrl++", "command": "e" },
    { "key": "f5", "command": "f", "args": "// not a comment", "when": "a" }, { "key": "ctrl+b" }
]"#;
        let t = run(windows, Os::Windows);
        assert!(text(&t).contains(r#""key": "cmd+shift+alt+d""#));
        assert!(text(&t).contains(r#""key": "cmd+k cmd+t""#));
        assert!(text(&t).contains(r#""key": "alt+up""#));
        assert!(text(&t).contains(r#""key":"cmd+=""#));
        assert!(text(&t).contains(r#""key": "cmd++""#));
        assert!(text(&t).contains(r#""key": "f5""#));
        assert!(text(&t).contains(r#""when": "editorTextFocus""#));
        assert!(text(&t).contains(r#""key": "cmd+b""#));
        assert!(text(&t).contains(r#""key": "ctrl+a""#));
        assert_eq!(t.changes, ["5 shortcuts: Ctrl → Cmd"]);
        assert!(t.flags.is_empty(), "{:?}", t.flags);

        let mac = r#"[{ "key": "cmd+shift+p", "command": "a" }, { "key": "ctrl+`", "command": "b" }, { "key": "meta+s", "command": "c" }]"#;
        let t = run(mac, Os::MacOs);
        assert_eq!(
            text(&t),
            r#"[{ "key": "ctrl+shift+p", "command": "a" }, { "key": "ctrl+`", "command": "b" }, { "key": "ctrl+s", "command": "c" }]"#
        );
        assert_eq!(t.changes, ["2 shortcuts: Cmd → Ctrl"]);
    }

    #[test]
    fn untranslatable_shortcuts_are_flagged_and_left_alone() {
        let windows = "[\n{ \"key\": \"win+e\", \"command\": \"a\" },\n{ \"key\": \"ctrl+tab\", \"command\": \"b\" },\n{ \"key\": \"ctrl+k ctrl+space\", \"command\": \"c\" }\n]";
        let t = run(windows, Os::Windows);
        assert_eq!(text(&t), windows);
        let lines: Vec<_> = t.flags.iter().map(|f| f.line).collect();
        assert_eq!(lines, [2, 3, 4]);
        assert!(t.flags[0].message.contains("Windows key"));
        assert!(t.flags[1]
            .message
            .contains("`cmd+tab`, which macOS keeps for itself"));
        assert!(t.flags[2].message.contains("`cmd+space`"));

        let mac = "[\n{ \"key\": \"cmd+ctrl+f\", \"command\": \"a\" },\n{ \"key\": \"alt+tab\", \"command\": \"b\" }\n]";
        let t = run(mac, Os::MacOs);
        assert_eq!(text(&t), mac);
        assert!(t.flags[0].message.contains("both Cmd and Ctrl"));
        assert!(t.flags[1].message.contains("is kept by Windows for itself"));
    }

    #[test]
    fn shortcuts_that_become_the_same_are_flagged() {
        let mac = "[\n{ \"key\": \"ctrl+w\", \"command\": \"a\" },\n{ \"key\": \"cmd+w\", \"command\": \"b\" }\n]";
        let t = run(mac, Os::MacOs);
        assert!(text(&t).contains(r#""key": "ctrl+w", "command": "b""#));
        assert_eq!(t.flags.len(), 1);
        assert_eq!(t.flags[0].line, 3);
        assert!(t.flags[0]
            .message
            .contains("the same as `ctrl+w` on line 2"));

        let reversed = "[\n{ \"key\": \"cmd+w\", \"command\": \"b\" },\n{ \"key\": \"ctrl+w\", \"command\": \"a\" }\n]";
        let t = run(reversed, Os::MacOs);
        assert_eq!(t.flags.len(), 1);
        assert!(t.flags[0]
            .message
            .starts_with("`ctrl+w` is what `cmd+w` on line 2 becomes"));

        // The same shortcut twice (such as removing a default, then adding
        // a new one) isn't a clash.
        let windows =
            r#"[{ "key": "ctrl+w", "command": "-a" }, { "key": "ctrl+w", "command": "b" }]"#;
        assert!(run(windows, Os::Windows).flags.is_empty());
    }

    #[test]
    fn os_specific_when_clauses_are_flagged() {
        let t = run(
            r#"[{ "key": "ctrl+r", "command": "a", "when": "isWindows && editorFocus" }]"#,
            Os::Windows,
        );
        assert_eq!(t.flags.len(), 1);
        assert!(t.flags[0].message.contains("`isWindows`"));
    }
}
