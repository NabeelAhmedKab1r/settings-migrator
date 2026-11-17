//! Settings Migrator engine: loads recipes, scans for apps, captures their
//! settings into a portable bundle and restores them with an undo backup.

use std::fmt;

mod error;
pub mod paths;
pub mod recipe;
pub mod scan;
pub mod secrets;

pub use error::{Error, Result};
pub use paths::PathContext;
pub use recipe::{Recipe, RecipeSet};

/// Version of the engine crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Operating systems the engine supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Windows,
    MacOs,
}

impl Os {
    /// The OS this binary was built for, or `None` if it isn't supported.
    pub fn current() -> Option<Os> {
        if cfg!(target_os = "windows") {
            Some(Os::Windows)
        } else if cfg!(target_os = "macos") {
            Some(Os::MacOs)
        } else {
            None
        }
    }
}

impl fmt::Display for Os {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Os::Windows => "Windows",
            Os::MacOs => "macOS",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    fn detects_current_os() {
        let expected = if cfg!(target_os = "windows") {
            Os::Windows
        } else {
            Os::MacOs
        };
        assert_eq!(Os::current(), Some(expected));
    }
}
