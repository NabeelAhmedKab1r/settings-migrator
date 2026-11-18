//! Flags settings that Windows and macOS usually set differently, such as
//! Git's `core.autocrlf`. Each recipe lists them per file as `section.key`;
//! the value is left as it was, since only the user knows which they want.

use std::collections::BTreeMap;

use super::Translation;
use crate::Os;

/// Looks through an INI-style file (Git config, Qt settings) for the
/// settings in `listed`. Section and key names match ignoring case, as Git
/// does, and a subsection (`[remote "origin"]`) counts as its section.
pub(super) fn check(text: &str, listed: &BTreeMap<String, String>, to: Os, out: &mut Translation) {
    if listed.is_empty() {
        return;
    }
    let listed: BTreeMap<String, &String> = listed
        .iter()
        .map(|(setting, note)| (setting.to_lowercase(), note))
        .collect();
    let mut section = String::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if let Some(header) = line.strip_prefix('[') {
            let header = header.split(']').next().unwrap_or_default();
            let name = header.split([' ', '\t', '"']).next().unwrap_or_default();
            section = name.to_lowercase();
            continue;
        }
        if line.is_empty() || line.starts_with(['#', ';']) {
            continue;
        }
        let (key, value) = match line.split_once('=') {
            Some((key, value)) => (key.trim(), Some(value.trim())),
            // In Git config a key on its own means true.
            None => (line, None),
        };
        let Some(note) = listed.get(&format!("{section}.{}", key.to_lowercase())) else {
            continue;
        };
        let shown = match value {
            Some(value) => format!("{key} = {value}"),
            None => key.to_string(),
        };
        out.flag(
            index + 1,
            format!(
                "`{shown}` {}; left as it was",
                note.replace("{to}", &to.to_string())
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flags(text: &str) -> Vec<(usize, String)> {
        let listed = BTreeMap::from([("core.autocrlf".to_string(), "differs on {to}".to_string())]);
        let mut out = Translation::unchanged(b"");
        check(text, &listed, Os::MacOs, &mut out);
        out.flags.into_iter().map(|f| (f.line, f.message)).collect()
    }

    #[test]
    fn finds_listed_settings_in_their_section() {
        let text = "[user]\n\tname = Sam\n[Core]\n\teditor = vim\n\tAutoCRLF = true\n";
        assert_eq!(
            flags(text),
            [(
                5,
                "`AutoCRLF = true` differs on macOS; left as it was".to_string()
            )]
        );
        assert_eq!(
            flags("[core]\n  autocrlf\n")[0].1,
            "`autocrlf` differs on macOS; left as it was"
        );
    }

    #[test]
    fn ignores_other_sections_and_comments() {
        let text = "[other]\nautocrlf = true\n[core]\n# autocrlf = true\n; autocrlf = true\n";
        assert!(flags(text).is_empty());
        assert!(flags("autocrlf = true\n").is_empty());
    }
}
