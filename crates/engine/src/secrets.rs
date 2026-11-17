//! Files that are never captured, whatever a recipe says. The full secret
//! guard (value scanning and a review list) comes in phase 3.

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
}
