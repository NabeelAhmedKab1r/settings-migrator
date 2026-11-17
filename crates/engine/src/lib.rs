//! Settings Migrator engine.
//!
//! Phase 1 only sets up the crate. Recipe loading, scan, capture and restore
//! arrive in phase 2.

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
