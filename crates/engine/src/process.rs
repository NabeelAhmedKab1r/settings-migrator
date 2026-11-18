//! Checks whether an app is open, so settings aren't changed underneath it
//! (an open app often writes its settings back when it quits).

use std::process::Command;

use crate::{Error, Os, Recipe, RecipeSet, Result};

/// The programs running on a computer.
#[derive(Debug, Clone)]
pub struct ProcessList {
    os: Os,
    /// Full executable paths on macOS, image names (`Code.exe`) on Windows.
    names: Vec<String>,
}

impl ProcessList {
    /// The programs running on this computer now.
    pub fn current() -> Result<ProcessList> {
        let os = Os::current()
            .ok_or_else(|| Error::Other("only Windows and macOS are supported".into()))?;
        let (program, args): (&str, &[&str]) = match os {
            Os::MacOs => ("/bin/ps", &["-axo", "comm="]),
            Os::Windows => ("tasklist", &["/FO", "CSV", "/NH"]),
        };
        let failed = |detail: String| {
            Error::Other(format!(
                "couldn't check which apps are open ({program}: {detail})"
            ))
        };
        let output = Command::new(program)
            .args(args)
            .output()
            .map_err(|e| failed(e.to_string()))?;
        if !output.status.success() {
            return Err(failed(
                String::from_utf8_lossy(&output.stderr).trim().to_string(),
            ));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let names = stdout
            .lines()
            .filter_map(|line| match os {
                Os::MacOs => Some(line.trim().to_string()),
                // "Code.exe","1234","Console","1","123,456 K"
                Os::Windows => line
                    .strip_prefix('"')
                    .and_then(|rest| rest.split('"').next())
                    .map(str::to_string),
            })
            .filter(|name| !name.is_empty())
            .collect();
        Ok(ProcessList { os, names })
    }

    /// A made-up list, for tests.
    pub fn from_names(os: Os, names: impl IntoIterator<Item = impl Into<String>>) -> ProcessList {
        ProcessList {
            os,
            names: names.into_iter().map(Into::into).collect(),
        }
    }

    /// Whether any of a recipe's processes is running. On macOS a process
    /// matches an app bundle name such as `Visual Studio Code.app` when it
    /// runs from that bundle's `Contents/MacOS` folder; on Windows it
    /// matches an image name such as `Code.exe`, ignoring case.
    pub fn is_running(&self, recipe: &Recipe) -> bool {
        recipe
            .processes(self.os)
            .iter()
            .any(|pattern| match self.os {
                Os::MacOs => {
                    let marker = format!("/{pattern}/Contents/MacOS/");
                    self.names.iter().any(|name| name.contains(&marker))
                }
                Os::Windows => self
                    .names
                    .iter()
                    .any(|name| name.eq_ignore_ascii_case(pattern)),
            })
    }

    /// Names of the apps among `ids` that are open, in recipe order.
    pub fn running_apps<'a>(
        &self,
        recipes: &RecipeSet,
        ids: impl IntoIterator<Item = &'a str>,
    ) -> Vec<String> {
        let ids: Vec<&str> = ids.into_iter().collect();
        recipes
            .iter()
            .filter(|recipe| ids.contains(&recipe.id.as_str()) && self.is_running(recipe))
            .map(|recipe| recipe.name.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_app_bundles_and_exe_names() {
        let recipes = RecipeSet::builtin().unwrap();
        let mac = ProcessList::from_names(
            Os::MacOs,
            [
                "/Applications/Visual Studio Code.app/Contents/MacOS/Electron",
                // A helper inside another app's bundle doesn't count.
                "/Applications/Other.app/Contents/Frameworks/OBS.app/Contents/Helpers/x",
            ],
        );
        assert_eq!(
            mac.running_apps(&recipes, ["vscode", "obs", "git"]),
            ["Visual Studio Code"]
        );

        let win = ProcessList::from_names(Os::Windows, ["obs64.exe", "explorer.exe"]);
        assert_eq!(
            win.running_apps(&recipes, ["vscode", "obs"]),
            ["OBS Studio"]
        );
        assert!(win.running_apps(&recipes, ["vscode"]).is_empty());
    }

    #[test]
    fn lists_real_processes_including_this_one() {
        let list = ProcessList::current().unwrap();
        let exe = std::env::current_exe().unwrap();
        let own = exe.file_name().unwrap().to_string_lossy().to_string();
        assert!(
            list.names.iter().any(|name| name.ends_with(&own)),
            "{own} not in {} processes",
            list.names.len()
        );
    }
}
