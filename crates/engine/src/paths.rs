//! Path variables such as `{home}` and `{appdata}`, resolved per OS.

use std::{
    collections::BTreeMap,
    env,
    path::{Component, Path, PathBuf},
};

use crate::{Error, Os, Result};

/// Where the folders behind each path variable are on one computer.
#[derive(Debug, Clone)]
pub struct PathContext {
    os: Os,
    vars: BTreeMap<&'static str, PathBuf>,
    state_dir: PathBuf,
}

impl PathContext {
    /// The real folders of the current user on this computer.
    pub fn from_env() -> Result<PathContext> {
        let os = Os::current()
            .ok_or_else(|| Error::Other("only Windows and macOS are supported".into()))?;
        let home_var = match os {
            Os::Windows => "USERPROFILE",
            Os::MacOs => "HOME",
        };
        let home = env::var_os(home_var)
            .map(PathBuf::from)
            .ok_or_else(|| Error::Other(format!("{home_var} isn't set")))?;
        let mut ctx = PathContext::for_home(os, home);
        if os == Os::Windows {
            for (var, env_name) in [("appdata", "APPDATA"), ("localappdata", "LOCALAPPDATA")] {
                if let Some(dir) = env::var_os(env_name) {
                    ctx.vars.insert(var, dir.into());
                }
            }
            ctx.state_dir = ctx.vars["localappdata"].join("settings-migrator");
        }
        Ok(ctx)
    }

    /// Folders laid out under `home` the way `os` lays them out. Tests use
    /// this with a temp folder so they never touch real settings.
    pub fn for_home(os: Os, home: impl Into<PathBuf>) -> PathContext {
        let home = home.into();
        let mut vars = BTreeMap::new();
        let state_dir = match os {
            Os::Windows => {
                vars.insert("appdata", home.join("AppData").join("Roaming"));
                vars.insert("localappdata", home.join("AppData").join("Local"));
                vars["localappdata"].join("settings-migrator")
            }
            Os::MacOs => {
                let library = home.join("Library");
                vars.insert("app_support", library.join("Application Support"));
                vars.insert("preferences", library.join("Preferences"));
                vars["app_support"].join("settings-migrator")
            }
        };
        vars.insert("home", home);
        PathContext {
            os,
            vars,
            state_dir,
        }
    }

    pub fn os(&self) -> Os {
        self.os
    }

    /// The home folder, which paths inside settings values are moved into.
    pub fn home(&self) -> &Path {
        &self.vars["home"]
    }

    /// Where the app keeps its own data, such as undo backups.
    pub fn state_dir(&self) -> &Path {
        &self.state_dir
    }

    /// Expands a location like `{appdata}/Code/User`. The text must start
    /// with a variable, so a recipe can never point at an arbitrary folder.
    pub fn resolve(&self, location: &str) -> Result<PathBuf> {
        let rest = location.strip_prefix('{').ok_or_else(|| {
            Error::Other(format!(
                "location {location:?} must start with a variable such as {{home}}"
            ))
        })?;
        let (name, rest) = rest
            .split_once('}')
            .ok_or_else(|| Error::Other(format!("location {location:?} has an unclosed {{")))?;
        let base = self
            .vars
            .get(name)
            .ok_or_else(|| Error::UnknownVariable(name.to_string(), self.os))?;
        let rest = rest.trim_start_matches('/');
        if rest.contains('{') {
            return Err(Error::Other(format!(
                "location {location:?} may only use one variable, at the start"
            )));
        }
        join_relative(base, rest)
    }
}

/// Joins a `/`-separated relative path onto `base`, refusing anything that
/// could escape it (`..`, absolute paths, drive prefixes).
pub fn join_relative(base: &Path, rel: &str) -> Result<PathBuf> {
    if rel.starts_with('/') {
        return Err(Error::Other(format!("unsafe relative path {rel:?}")));
    }
    let mut path = base.to_path_buf();
    for part in rel.split('/').filter(|part| !part.is_empty()) {
        let mut components = Path::new(part).components();
        match (components.next(), components.next()) {
            (Some(Component::Normal(_)), None) if !part.contains('\\') && !part.contains(':') => {
                path.push(part)
            }
            _ => return Err(Error::Other(format!("unsafe relative path {rel:?}"))),
        }
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_variables_per_os() {
        let mac = PathContext::for_home(Os::MacOs, "/u");
        assert_eq!(
            mac.resolve("{app_support}/Code/User").unwrap(),
            Path::new("/u/Library/Application Support/Code/User")
        );
        assert_eq!(mac.resolve("{home}").unwrap(), Path::new("/u"));

        let win = PathContext::for_home(Os::Windows, "/u");
        assert_eq!(
            win.resolve("{appdata}/vlc").unwrap(),
            Path::new("/u").join("AppData").join("Roaming").join("vlc")
        );
    }

    #[test]
    fn rejects_bad_locations() {
        let mac = PathContext::for_home(Os::MacOs, "/u");
        assert!(matches!(
            mac.resolve("{appdata}/x"),
            Err(Error::UnknownVariable(..))
        ));
        assert!(mac.resolve("/etc").is_err());
        assert!(mac.resolve("{home}/../x").is_err());
        assert!(mac.resolve("{home}/{home}").is_err());
    }

    #[test]
    fn join_relative_refuses_escapes() {
        let base = Path::new("base");
        assert_eq!(
            join_relative(base, "a/b.json").unwrap(),
            base.join("a").join("b.json")
        );
        for bad in ["../x", "a/../../x", "/etc/passwd", "C:/x", "a\\..\\x"] {
            assert!(join_relative(base, bad).is_err(), "{bad}");
        }
    }
}
