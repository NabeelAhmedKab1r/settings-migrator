//! Cross-OS translation: fixes what differs between Windows and macOS in a
//! settings file (shortcuts, paths inside values, line endings), and flags
//! anything it can't translate so the restore preview shows it. Nothing is
//! ever dropped: an untranslatable item is restored as it was.

mod keys;
mod paths;

use std::path::Path;

use crate::{
    preview::as_text,
    recipe::{FileEntry, Format, Translator},
    Os,
};

/// A settings file made ready for the target OS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Translation {
    pub contents: Vec<u8>,
    /// What was translated, such as "3 shortcuts: Ctrl → Cmd".
    pub changes: Vec<String>,
    /// What couldn't be translated and was left as it was.
    pub flags: Vec<Flag>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flag {
    /// Line in the file, counting from 1; 0 means the whole file.
    pub line: usize,
    pub message: String,
}

impl Translation {
    /// The file as it is, with nothing translated.
    pub fn unchanged(contents: &[u8]) -> Translation {
        Translation {
            contents: contents.to_vec(),
            changes: Vec::new(),
            flags: Vec::new(),
        }
    }

    fn flag(&mut self, line: usize, message: String) {
        self.flags.push(Flag { line, message });
    }
}

/// Translates one file captured on `from` so it works on `to`. `entry` is
/// the recipe's entry for the file, which says what to translate; `home` is
/// the home folder on this computer, where paths in the old home folder move.
pub fn translate(
    rel: &str,
    entry: Option<&FileEntry>,
    contents: &[u8],
    from: Os,
    to: Os,
    home: &Path,
) -> Translation {
    let mut out = Translation::unchanged(contents);
    if from == to {
        return out;
    }
    let translators = entry.map_or(&[][..], |entry| &entry.translate[..]);
    let Some(text) = as_text(contents) else {
        if translators.iter().any(|t| *t != Translator::Fonts) {
            out.flag(
                0,
                "not UTF-8 text, so it couldn't be checked for paths or shortcuts; restored as it was"
                    .into(),
            );
        }
        return out;
    };

    let json = entry.and_then(|entry| entry.format) == Some(Format::Json) || rel.ends_with(".json");
    let mut text = text.to_string();
    if translators.contains(&Translator::ModifierKeys) {
        text = keys::translate(&text, from, to, &mut out);
    }
    if translators.contains(&Translator::AbsolutePaths) {
        text = paths::translate(&text, json, from, to, home, &mut out);
    }
    if translators.contains(&Translator::LineEndings) {
        text = line_endings(text, to, &mut out);
    }
    out.flags.sort_by_key(|flag| flag.line);
    out.contents = text.into_bytes();
    out
}

/// Converts a file that consistently uses one OS's line endings to the
/// other's. Files that mix both are left alone.
fn line_endings(text: String, to: Os, out: &mut Translation) -> String {
    let crlf = text.matches("\r\n").count();
    let lf = text.matches('\n').count() - crlf;
    match to {
        Os::MacOs if crlf > 0 && lf == 0 => {
            out.changes
                .push("line endings: Windows (CRLF) → macOS (LF)".into());
            text.replace("\r\n", "\n")
        }
        Os::Windows if lf > 0 && crlf == 0 => {
            out.changes
                .push("line endings: macOS (LF) → Windows (CRLF)".into());
            text.replace('\n', "\r\n")
        }
        _ => text,
    }
}

/// The line holding byte `offset` of `text`, counting from 1.
fn line_of(text: &str, offset: usize) -> usize {
    text[..offset].matches('\n').count() + 1
}

/// "1 path" or "2 paths".
fn count(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("1 {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(yaml: &str) -> FileEntry {
        serde_yaml::from_str(yaml).unwrap()
    }

    fn run(entry: Option<&FileEntry>, contents: &[u8], from: Os, to: Os) -> Translation {
        translate("x.ini", entry, contents, from, to, Path::new("/h"))
    }

    #[test]
    fn same_os_is_byte_for_byte() {
        let all = entry("{path: x.ini, translate: [absolute-paths, line-endings]}");
        let contents = b"a=C:\\Users\\Sam\r\nb\r\n";
        let t = run(Some(&all), contents, Os::Windows, Os::Windows);
        assert_eq!(t, Translation::unchanged(contents));
    }

    #[test]
    fn line_endings_follow_the_target_when_the_recipe_asks() {
        let eol = entry("{path: x.ini, translate: [line-endings]}");
        let to_mac = run(Some(&eol), b"a\r\nb\r\n", Os::Windows, Os::MacOs);
        assert_eq!(to_mac.contents, b"a\nb\n");
        assert_eq!(
            to_mac.changes,
            ["line endings: Windows (CRLF) → macOS (LF)"]
        );
        let to_windows = run(Some(&eol), b"a\nb\n", Os::MacOs, Os::Windows);
        assert_eq!(to_windows.contents, b"a\r\nb\r\n");

        // Already right, mixed, or a single line: left alone.
        for text in [&b"a\nb\n"[..], b"a\r\nb\n", b"one line"] {
            let t = run(Some(&eol), text, Os::Windows, Os::MacOs);
            assert_eq!(t.contents, text);
            assert!(t.changes.is_empty());
        }

        // Without the translator the file is copied byte for byte, since
        // some apps checksum their files.
        let plain = run(None, b"a\r\nb\r\n", Os::Windows, Os::MacOs);
        assert_eq!(plain, Translation::unchanged(b"a\r\nb\r\n"));
    }

    #[test]
    fn binary_files_are_flagged_only_when_they_need_translating() {
        let bytes = [0, 159, 146, 150];
        assert!(run(None, &bytes, Os::Windows, Os::MacOs).flags.is_empty());

        let paths = entry("{path: a.bin, translate: [absolute-paths]}");
        let t = run(Some(&paths), &bytes, Os::Windows, Os::MacOs);
        assert_eq!(t.contents, bytes);
        assert_eq!(t.flags.len(), 1);
        assert_eq!(t.flags[0].line, 0);
    }
}
