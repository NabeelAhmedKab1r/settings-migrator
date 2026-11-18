//! Recipe files: where an app keeps its settings and which of them to capture.

use std::{
    fs,
    path::{Path, PathBuf},
};

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use serde::Deserialize;

use crate::{paths::join_relative, secrets, Error, Os, Result};

const BUILTIN: &[(&str, &str)] = include!(concat!(env!("OUT_DIR"), "/builtin_recipes.rs"));

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub id: String,
    pub name: String,
    pub locations: Locations,
    pub files: Vec<FileEntry>,
    #[serde(default)]
    pub exclude: Vec<String>,
    /// Parsed now; listing and installing extensions comes in a later phase.
    #[serde(default)]
    pub extensions: Option<Extensions>,
    #[serde(default)]
    pub cross_os: bool,
    /// The app's programs, so restore can check the app is closed.
    #[serde(default)]
    pub processes: Processes,
    #[serde(skip)]
    excluded: GlobSet,
}

/// macOS app bundle names (`Visual Studio Code.app`) and Windows program
/// names (`Code.exe`).
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Processes {
    #[serde(default)]
    pub windows: Vec<String>,
    #[serde(default)]
    pub macos: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Locations {
    pub windows: Option<String>,
    pub macos: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileEntry {
    /// A file, or a folder when it ends in `/`.
    pub path: String,
    pub format: Option<Format>,
    /// Used by cross-OS translation in phase 4.
    #[serde(default)]
    pub translate: Vec<Translator>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Json,
    Ini,
    Yaml,
    Toml,
    Xml,
    Plist,
    Text,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Translator {
    ModifierKeys,
    AbsolutePaths,
    LineEndings,
    Fonts,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Extensions {
    pub list: String,
    pub install: String,
}

impl FileEntry {
    pub fn is_dir(&self) -> bool {
        self.path.ends_with('/')
    }
}

impl Recipe {
    /// Parses and checks one recipe. `source` names it in error messages.
    pub fn parse(source: &str, yaml: &str) -> Result<Recipe> {
        let err = |message: String| Error::Recipe {
            name: source.to_string(),
            message,
        };
        let mut recipe: Recipe = serde_yaml::from_str(yaml).map_err(|e| err(e.to_string()))?;

        let id_ok = !recipe.id.is_empty()
            && recipe
                .id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        if !id_ok {
            return Err(err(format!(
                "id {:?} must use only a-z, 0-9 and -",
                recipe.id
            )));
        }
        if recipe.files.is_empty() {
            return Err(err("files must list at least one path".into()));
        }
        if recipe.locations.windows.is_none() && recipe.locations.macos.is_none() {
            return Err(err("locations must name windows, macos or both".into()));
        }
        for entry in &recipe.files {
            if entry.path.trim_end_matches('/').is_empty()
                || join_relative(Path::new(""), &entry.path).is_err()
            {
                return Err(err(format!(
                    "file path {:?} must be relative, without ..",
                    entry.path
                )));
            }
        }

        for name in &recipe.processes.macos {
            if !name.ends_with(".app") || name.contains('/') {
                return Err(err(format!(
                    "macOS process {name:?} must be an app bundle name ending in .app"
                )));
            }
        }
        for name in &recipe.processes.windows {
            if !name.to_ascii_lowercase().ends_with(".exe") || name.contains(['/', '\\']) {
                return Err(err(format!(
                    "Windows process {name:?} must be a program name ending in .exe"
                )));
            }
        }

        let mut globs = GlobSetBuilder::new();
        for pattern in &recipe.exclude {
            // A trailing / excludes a whole folder.
            let pattern = match pattern.strip_suffix('/') {
                Some(dir) => format!("{dir}/**"),
                None => pattern.clone(),
            };
            let glob = GlobBuilder::new(&pattern)
                .literal_separator(true)
                .build()
                .map_err(|e| err(format!("exclude {pattern:?}: {e}")))?;
            globs.add(glob);
        }
        recipe.excluded = globs.build().map_err(|e| err(e.to_string()))?;
        Ok(recipe)
    }

    /// The settings location on `os`, still containing its path variable.
    pub fn location(&self, os: Os) -> Option<&str> {
        match os {
            Os::Windows => self.locations.windows.as_deref(),
            Os::MacOs => self.locations.macos.as_deref(),
        }
    }

    /// The app's process names on `os`; empty for tools that don't stay open.
    pub fn processes(&self, os: Os) -> &[String] {
        match os {
            Os::Windows => &self.processes.windows,
            Os::MacOs => &self.processes.macos,
        }
    }

    /// Whether a relative path may be captured or restored for this app:
    /// listed in `files`, not excluded, and not a known secret file.
    pub fn allows(&self, rel_path: &str) -> bool {
        let listed = self.files.iter().any(|entry| {
            if entry.is_dir() {
                rel_path.starts_with(&entry.path)
            } else {
                rel_path == entry.path
            }
        });
        listed && !self.excluded.is_match(rel_path) && !secrets::is_denied(rel_path)
    }

    /// The settings files that exist under `root`, as sorted `/`-separated
    /// relative paths. Symlinks inside listed folders are skipped so a
    /// capture never wanders outside the app's folder.
    pub fn collect_files(&self, root: &Path) -> Result<Vec<String>> {
        let mut found = Vec::new();
        for entry in &self.files {
            let path = join_relative(root, &entry.path)?;
            if entry.is_dir() {
                if path.is_dir() {
                    walk(&path, entry.path.trim_end_matches('/'), &mut found)?;
                }
            } else if path.is_file() {
                found.push(entry.path.clone());
            }
        }
        found.retain(|rel| self.allows(rel));
        found.sort();
        found.dedup();
        Ok(found)
    }
}

fn walk(dir: &Path, rel: &str, found: &mut Vec<String>) -> Result<()> {
    let entries = fs::read_dir(dir).map_err(|e| Error::io(dir, e))?;
    for entry in entries {
        let entry = entry.map_err(|e| Error::io(dir, e))?;
        let file_type = entry.file_type().map_err(|e| Error::io(entry.path(), e))?;
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue; // Bundles store UTF-8 paths only.
        };
        let child = format!("{rel}/{name}");
        if file_type.is_dir() {
            walk(&entry.path(), &child, found)?;
        } else if file_type.is_file() {
            found.push(child);
        }
    }
    Ok(())
}

/// All recipes in use, sorted by id.
#[derive(Debug)]
pub struct RecipeSet {
    recipes: Vec<Recipe>,
}

impl RecipeSet {
    /// The recipes built into the binary from the `recipes/` folder.
    pub fn builtin() -> Result<RecipeSet> {
        RecipeSet::from_sources(
            BUILTIN
                .iter()
                .map(|(name, yaml)| (name.to_string(), yaml.to_string())),
        )
    }

    /// Every `.yaml` recipe in `dir`.
    pub fn load_dir(dir: &Path) -> Result<RecipeSet> {
        let mut sources = Vec::new();
        for entry in fs::read_dir(dir).map_err(|e| Error::io(dir, e))? {
            let path: PathBuf = entry.map_err(|e| Error::io(dir, e))?.path();
            if path.extension().is_some_and(|ext| ext == "yaml") {
                let yaml = fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
                sources.push((path.display().to_string(), yaml));
            }
        }
        RecipeSet::from_sources(sources)
    }

    fn from_sources(sources: impl IntoIterator<Item = (String, String)>) -> Result<RecipeSet> {
        let mut recipes = Vec::new();
        for (name, yaml) in sources {
            recipes.push(Recipe::parse(&name, &yaml)?);
        }
        recipes.sort_by(|a, b| a.id.cmp(&b.id));
        if let Some(pair) = recipes.windows(2).find(|pair| pair[0].id == pair[1].id) {
            return Err(Error::Other(format!(
                "two recipes use the id {:?}",
                pair[0].id
            )));
        }
        Ok(RecipeSet { recipes })
    }

    pub fn iter(&self) -> impl Iterator<Item = &Recipe> {
        self.recipes.iter()
    }

    pub fn get(&self, id: &str) -> Option<&Recipe> {
        self.recipes.iter().find(|recipe| recipe.id == id)
    }

    /// The recipes named in `ids`, or all of them when `ids` is empty.
    pub fn select(&self, ids: &[String]) -> Result<Vec<&Recipe>> {
        if ids.is_empty() {
            return Ok(self.recipes.iter().collect());
        }
        ids.iter()
            .map(|id| {
                self.get(id)
                    .ok_or_else(|| Error::Other(format!("no recipe with id {id:?}")))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_recipes_load() {
        let set = RecipeSet::builtin().unwrap();
        let ids: Vec<_> = set.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["git", "obs", "obsidian", "vlc", "vscode"]);
        for recipe in set.iter() {
            assert!(recipe.location(Os::Windows).is_some() && recipe.location(Os::MacOs).is_some());
        }
    }

    #[test]
    fn excludes_and_denylist_apply() {
        let set = RecipeSet::builtin().unwrap();
        let vscode = set.get("vscode").unwrap();
        assert!(vscode.allows("settings.json"));
        assert!(vscode.allows("snippets/rust.json"));
        assert!(!vscode.allows("globalStorage/state.vscdb"));
        assert!(!vscode.allows("other.json"));

        let obs = set.get("obs").unwrap();
        assert!(obs.allows("basic/profiles/Main/basic.ini"));
        assert!(!obs.allows("basic/profiles/Main/service.json"));

        let git = set.get("git").unwrap();
        assert!(!git.allows(".git-credentials"));
    }

    #[test]
    fn rejects_bad_recipes() {
        let bad = [
            "id: Bad Id\nname: x\nlocations: {macos: '{home}'}\nfiles: [{path: a}]",
            "id: x\nname: x\nlocations: {macos: '{home}'}\nfiles: []",
            "id: x\nname: x\nlocations: {}\nfiles: [{path: a}]",
            "id: x\nname: x\nlocations: {macos: '{home}'}\nfiles: [{path: ../a}]",
            "id: x\nname: x\nlocations: {macos: '{home}'}\nfiles: [{path: a}]\nsurprise: 1",
            "id: x\nname: x\nlocations: {macos: '{home}'}\nfiles: [{path: a}]\nprocesses: {macos: [Code]}",
            "id: x\nname: x\nlocations: {macos: '{home}'}\nfiles: [{path: a}]\nprocesses: {windows: [C:/Code.exe]}",
        ];
        for yaml in bad {
            assert!(Recipe::parse("test", yaml).is_err(), "{yaml}");
        }
    }
}
