//! File-by-file preview of what a restore changes: a unified diff for text
//! files, sizes for binary ones.

use similar::{ChangeTag, TextDiff};

/// Lines shown per file before the rest is summarised.
const MAX_LINES: usize = 200;
/// Characters shown per line; minified JSON can be one enormous line.
const MAX_LINE_CHARS: usize = 200;

#[derive(Debug, PartialEq, Eq)]
pub enum Preview {
    /// Nothing will change.
    Nothing,
    /// Only the line endings differ (CRLF and LF).
    LineEndingsOnly,
    /// Not text; only the sizes can be compared. `old` is `None` for a new file.
    Binary { old: Option<u64>, new: u64 },
    /// A unified diff, with `hidden` more lines left out.
    Text { lines: Vec<DiffLine>, hidden: usize },
}

#[derive(Debug, PartialEq, Eq)]
pub enum DiffLine {
    /// `@@ -1,3 +1,4 @@`
    Hunk(String),
    Context(String),
    Added(String),
    Removed(String),
}

/// Compares the current file (`None` if it doesn't exist) with what would
/// replace it.
pub fn diff(old: Option<&[u8]>, new: &[u8]) -> Preview {
    if old == Some(new) {
        return Preview::Nothing;
    }
    let (Some(old_text), Some(new_text)) = (as_text(old.unwrap_or_default()), as_text(new)) else {
        return Preview::Binary {
            old: old.map(|bytes| bytes.len() as u64),
            new: new.len() as u64,
        };
    };
    if old.is_some() && old_text.replace("\r\n", "\n") == new_text.replace("\r\n", "\n") {
        return Preview::LineEndingsOnly;
    }

    let diff = TextDiff::from_lines(old_text, new_text);
    let mut lines = Vec::new();
    let mut hidden = 0;
    for hunk in diff.unified_diff().context_radius(3).iter_hunks() {
        let mut push = |line: DiffLine| {
            if lines.len() < MAX_LINES {
                lines.push(line);
            } else {
                hidden += 1;
            }
        };
        push(DiffLine::Hunk(hunk.header().to_string()));
        for change in hunk.iter_changes() {
            let text = shorten(change.value().trim_end_matches(['\n', '\r']));
            push(match change.tag() {
                ChangeTag::Equal => DiffLine::Context(text),
                ChangeTag::Insert => DiffLine::Added(text),
                ChangeTag::Delete => DiffLine::Removed(text),
            });
        }
    }
    Preview::Text { lines, hidden }
}

/// The bytes as text, unless they aren't UTF-8 or contain NUL bytes.
pub(crate) fn as_text(bytes: &[u8]) -> Option<&str> {
    std::str::from_utf8(bytes)
        .ok()
        .filter(|text| !text.contains('\0'))
}

fn shorten(line: &str) -> String {
    let count = line.chars().count();
    if count <= MAX_LINE_CHARS {
        return line.to_string();
    }
    let kept: String = line.chars().take(MAX_LINE_CHARS).collect();
    format!("{kept}… ({} more characters)", count - MAX_LINE_CHARS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diffs_changed_lines_with_context() {
        let old = b"[user]\n\tname = Old\n\temail = a@example.com\n";
        let new = b"[user]\n\tname = New\n\temail = a@example.com\n";
        let Preview::Text { lines, hidden } = diff(Some(old), new) else {
            panic!("expected a text diff");
        };
        assert_eq!(hidden, 0);
        assert_eq!(
            lines,
            [
                DiffLine::Hunk("@@ -1,3 +1,3 @@".into()),
                DiffLine::Context("[user]".into()),
                DiffLine::Removed("\tname = Old".into()),
                DiffLine::Added("\tname = New".into()),
                DiffLine::Context("\temail = a@example.com".into()),
            ]
        );
    }

    #[test]
    fn new_files_show_every_line_added() {
        let Preview::Text { lines, .. } = diff(None, b"a\nb\n") else {
            panic!("expected a text diff");
        };
        assert_eq!(
            lines,
            [
                DiffLine::Hunk("@@ -0,0 +1,2 @@".into()),
                DiffLine::Added("a".into()),
                DiffLine::Added("b".into()),
            ]
        );
    }

    #[test]
    fn special_cases() {
        assert_eq!(diff(Some(b"same"), b"same"), Preview::Nothing);
        assert_eq!(
            diff(Some(b"a\r\nb\r\n"), b"a\nb\n"),
            Preview::LineEndingsOnly
        );
        assert_eq!(
            diff(Some(&[0, 1, 2]), &[0, 1]),
            Preview::Binary {
                old: Some(3),
                new: 2
            }
        );
        assert_eq!(
            diff(None, &[0xff, 0xfe]),
            Preview::Binary { old: None, new: 2 }
        );
    }

    #[test]
    fn long_diffs_and_lines_are_shortened() {
        let new: String = (0..500).map(|i| format!("line {i}\n")).collect();
        let Preview::Text { lines, hidden } = diff(None, new.as_bytes()) else {
            panic!("expected a text diff");
        };
        assert_eq!(lines.len(), MAX_LINES);
        assert_eq!(hidden, 501 - MAX_LINES);

        let long = "x".repeat(1000);
        let Preview::Text { lines, .. } = diff(None, long.as_bytes()) else {
            panic!("expected a text diff");
        };
        assert_eq!(
            lines[1],
            DiffLine::Added(format!("{}… (800 more characters)", "x".repeat(200)))
        );
    }
}
