//! The secret guard. Known secret files are never captured, whatever a
//! recipe says, and the contents of every captured file are scanned for
//! tokens, passwords and high-entropy strings so they can be reviewed.

use std::sync::OnceLock;

use regex::Regex;

/// File names (compared without case) that hold passwords, keys or cookies.
const DENIED_NAMES: &[&str] = &[
    ".git-credentials",
    ".netrc",
    "_netrc",
    "id_rsa",
    "id_dsa",
    "id_ecdsa",
    "id_ed25519",
    "cookies",
    "cookies-journal",
    "cookies.sqlite",
    "login data",
    "login data-journal",
    "logins.json",
    "key3.db",
    "key4.db",
    "web data",
];

/// File extensions (compared without case) used for private keys and certificates.
const DENIED_EXTENSIONS: &[&str] = &["pem", "key", "p12", "pfx", "keychain", "keychain-db"];

/// Whether a `/`-separated relative path names a known secret file.
pub fn is_denied(rel_path: &str) -> bool {
    let name = rel_path
        .rsplit('/')
        .next()
        .unwrap_or(rel_path)
        .to_lowercase();
    DENIED_NAMES.contains(&name.as_str())
        || name
            .rsplit_once('.')
            .is_some_and(|(_, ext)| DENIED_EXTENSIONS.contains(&ext))
}

/// Something in a file that looks like a secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// 1-based line number.
    pub line: usize,
    /// What it looks like, such as "GitHub token".
    pub kind: &'static str,
    /// The flagged value, shortened so the secret itself is never shown.
    pub excerpt: String,
}

/// Patterns for well-known token formats. The first capture group, if any,
/// is the secret; otherwise the whole match is.
const PATTERNS: &[(&str, &str)] = &[
    (
        "private key",
        r"-----BEGIN [A-Z ]*PRIVATE KEY(?: BLOCK)?-----",
    ),
    (
        "GitHub token",
        r"\b(?:gh[pousr]_[A-Za-z0-9]{36,}|github_pat_[A-Za-z0-9_]{40,})",
    ),
    ("GitLab token", r"\bglpat-[A-Za-z0-9_-]{20,}"),
    ("AWS access key", r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b"),
    ("Slack token", r"\bxox[abposr]-[A-Za-z0-9-]{10,}"),
    ("Stripe key", r"\b[sr]k_live_[0-9A-Za-z]{20,}"),
    ("API key", r"\bsk-[A-Za-z0-9_-]{20,}"),
    ("Google API key", r"\bAIza[0-9A-Za-z_-]{35}"),
    ("npm token", r"\bnpm_[A-Za-z0-9]{36}\b"),
    (
        "JSON web token",
        r"\beyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}",
    ),
    (
        "password in a URL",
        r"[A-Za-z][A-Za-z0-9+.-]*://[^\s/:@\x22']+:([^\s/@\x22']+)@",
    ),
    ("token in a URL", r"https?://([A-Za-z0-9_-]{20,})@"),
    (
        "authorization header",
        r"(?i)authorization:\s*(?:basic|bearer|token)\s+([A-Za-z0-9+/=._-]{8,})",
    ),
    (
        "secret setting",
        r"(?i)[\w.-]*(?:password|passwd|secret|token|api[_-]?key|access[_-]?key|private[_-]?key|credential)s?[\w.-]*[\x22']?[ \t]*[:=][ \t]*[\x22']?([^\s\x22'{}\[\],;$<%][^\s\x22'{}\[\],;]{7,})",
    ),
];

/// Strings at least this long are checked for randomness.
const ENTROPY_MIN_LEN: usize = 20;
/// Bits per character above which a mixed-case alphanumeric string looks
/// random rather than like a word, path or identifier.
const ENTROPY_THRESHOLD: f64 = 4.0;

fn patterns() -> &'static [(&'static str, Regex)] {
    static COMPILED: OnceLock<Vec<(&str, Regex)>> = OnceLock::new();
    COMPILED.get_or_init(|| {
        PATTERNS
            .iter()
            .map(|(kind, pattern)| (*kind, Regex::new(pattern).expect("valid secret pattern")))
            .collect()
    })
}

fn entropy_candidates() -> &'static Regex {
    static COMPILED: OnceLock<Regex> = OnceLock::new();
    COMPILED.get_or_init(|| Regex::new(r"[A-Za-z0-9+_=-]{20,}").unwrap())
}

/// Scans file contents for anything that looks like a secret. Binary files
/// are scanned as text with invalid bytes replaced.
pub fn scan_contents(bytes: &[u8]) -> Vec<Finding> {
    let text = String::from_utf8_lossy(bytes);
    // (start, end, kind) of each flagged value; earlier rules win overlaps.
    let mut spans: Vec<(usize, usize, &'static str)> = Vec::new();
    let mut add = |start: usize, end: usize, kind| {
        if !spans.iter().any(|&(s, e, _)| start < e && s < end) {
            spans.push((start, end, kind));
        }
    };

    for (kind, regex) in patterns() {
        for caps in regex.captures_iter(&text) {
            let m = caps.get(1).unwrap_or_else(|| caps.get(0).unwrap());
            if *kind == "secret setting" && !looks_like_secret_value(m.as_str()) {
                continue;
            }
            add(m.start(), m.end(), kind);
        }
    }
    for m in entropy_candidates().find_iter(&text) {
        if is_high_entropy(m.as_str()) {
            add(m.start(), m.end(), "random-looking string");
        }
    }

    spans.sort();
    spans
        .into_iter()
        .map(|(start, end, kind)| Finding {
            line: text[..start].matches('\n').count() + 1,
            kind,
            excerpt: redact(&text[start..end]),
        })
        .collect()
}

/// Filters "secret setting" values: plain lowercase words such as
/// `osxkeychain` or `manager-core` name a helper, not a secret, and pure
/// numbers are timeouts or sizes.
fn looks_like_secret_value(value: &str) -> bool {
    let has_letter = value.chars().any(|c| c.is_ascii_alphabetic());
    let plain_word = value
        .chars()
        .all(|c| c.is_ascii_lowercase() || matches!(c, '-' | '_' | '.'));
    has_letter && !plain_word && !matches!(value, "true" | "false" | "null")
}

fn is_high_entropy(candidate: &str) -> bool {
    candidate.len() >= ENTROPY_MIN_LEN
        && candidate.chars().any(|c| c.is_ascii_uppercase())
        && candidate.chars().any(|c| c.is_ascii_lowercase())
        && candidate.chars().any(|c| c.is_ascii_digit())
        && shannon_entropy(candidate) >= ENTROPY_THRESHOLD
}

fn shannon_entropy(text: &str) -> f64 {
    let mut counts = [0usize; 256];
    for b in text.bytes() {
        counts[b as usize] += 1;
    }
    let len = text.len() as f64;
    counts
        .iter()
        .filter(|&&n| n > 0)
        .map(|&n| {
            let p = n as f64 / len;
            -p * p.log2()
        })
        .sum()
}

/// Shows only the first few characters and the length.
fn redact(secret: &str) -> String {
    if secret.starts_with("-----BEGIN") {
        return secret.to_string();
    }
    let len = secret.chars().count();
    let shown: String = secret.chars().take(if len > 12 { 4 } else { 0 }).collect();
    format!("{shown}… ({len} characters)")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn denies_known_secret_files() {
        for path in [
            ".git-credentials",
            "ssh/id_ed25519",
            "Default/Cookies",
            "certs/server.PEM",
        ] {
            assert!(is_denied(path), "{path}");
        }
        for path in [
            ".gitconfig",
            "settings.json",
            "id_ed25519.pub",
            "keybindings.json",
        ] {
            assert!(!is_denied(path), "{path}");
        }
    }

    // Fake secrets, built at runtime so this source file never contains a
    // string that looks like a real token.
    fn fake(prefix: &str, body: &str, len: usize) -> String {
        let mut s = prefix.to_string();
        while s.len() < prefix.len() + len {
            s.push_str(body);
        }
        s.truncate(prefix.len() + len);
        s
    }

    fn kinds(text: &str) -> Vec<&'static str> {
        scan_contents(text.as_bytes())
            .into_iter()
            .map(|f| f.kind)
            .collect()
    }

    #[test]
    fn finds_known_token_formats() {
        let cases = [
            (fake("ghp_", "aB3dE5fG7h", 36), "GitHub token"),
            (fake("glpat-", "xY9zW8vU7t", 20), "GitLab token"),
            (fake("AKIA", "Q7R2S9T4", 16), "AWS access key"),
            (fake("xoxb-", "1234-abcd-", 30), "Slack token"),
            (fake("sk-", "Ab1Cd2Ef3G", 40), "API key"),
            (fake("npm_", "Zx8Yw7Vu6T", 36), "npm token"),
            (fake("AIza", "Sy-Ab1Cd2E", 35), "Google API key"),
            (
                "-----BEGIN OPENSSH PRIVATE KEY-----".to_string(),
                "private key",
            ),
        ];
        for (token, kind) in cases {
            assert_eq!(kinds(&format!("value = {token}\n")), [kind], "{token}");
        }
    }

    #[test]
    fn finds_secrets_in_settings_and_urls() {
        assert_eq!(
            kinds("[http]\n\tpassword = Hunter2Pass\n"),
            ["secret setting"]
        );
        assert_eq!(
            kinds(r#"{"env": {"MY_API_KEY": "abc123XYZ789"}}"#),
            ["secret setting"]
        );
        assert_eq!(
            kinds("url = https://me:S3cretPw@example.com/repo.git"),
            ["password in a URL"]
        );
        assert_eq!(
            kinds("extraheader = AUTHORIZATION: bearer abcDEF123456"),
            ["authorization header"]
        );
        let random = fake("", "Q2hhbmdlTWUtNjQ3OTEyZmFrZQ", 32);
        assert_eq!(kinds(&format!("blob={random}")), ["random-looking string"]);
    }

    #[test]
    fn reports_line_and_redacts() {
        let token = fake("ghp_", "aB3dE5fG7h", 36);
        let findings =
            scan_contents(format!("{{\n  \"a\": 1,\n  \"t\": \"{token}\"\n}}").as_bytes());
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, 3);
        assert_eq!(findings[0].excerpt, "ghp_… (40 characters)");
        assert!(!findings[0].excerpt.contains(&token[4..]));
    }

    #[test]
    fn ordinary_settings_are_not_flagged() {
        let ordinary = r##"
{
  "editor.fontFamily": "JetBrains Mono, Menlo",
  "editor.tokenColorCustomizations": { "comments": "#6A9955" },
  "editor.semanticTokenColorCustomizations": { "enabled": true },
  "git.useCredentialManager": true,
  "workbench.colorTheme": "Default Dark Modern",
  "terminal.integrated.defaultProfile.osx": "zsh",
  "files.exclude": { "**/node_modules": true },
  "http.proxy": "http://proxy.example.com:8080",
  "remote.SSH.configFile": "/Users/test/.ssh/config",
  "id": "3f2b8c1e-9a4d-4b7e-8f21-6c5d0e9a7b13",
  "hash": "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08",
  "sessionTokenTimeout": 30000000,
  "passwordStore": "basic"
}
[credential]
	helper = osxkeychain
[user]
	signingkey = 4AA5C7D2E1F0B3C9
[core]
volume-step=12.8
#http-password=
key-toggle-fullscreen=f
"##;
        assert_eq!(scan_contents(ordinary.as_bytes()), []);
    }
}
