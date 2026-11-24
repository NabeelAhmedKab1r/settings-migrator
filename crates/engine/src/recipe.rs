//! Recipe files: where an app keeps its settings and which of them to capture.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use serde::Deserialize;

use crate::{
    paths::join_relative,
    secrets,
    wildcard::{self, Placement},
    Error, Os, Result,
};

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
    /// A file, or a folder when it ends in `/`. One folder name in it may
    /// use `*` for names that differ between computers (see `wildcard`).
    pub path: String,
    pub format: Option<Format>,
    /// What to translate when the settings move to the other OS.
    #[serde(default)]
    pub translate: Vec<Translator>,
    /// Only used on this OS; skipped when restoring on the other one.
    pub os: Option<Os>,
    /// Settings in an INI-style file that Windows and macOS usually set
    /// differently, as `section.key` and a note on why. Restoring on the
    /// other OS flags each one found, since only the user can say which
    /// value they want.
    #[serde(default)]
    pub os_specific: BTreeMap<String, String>,
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
    /// Ctrl/Cmd in VS Code-style keybinding files.
    ModifierKeys,
    /// Paths in the home folder inside settings values.
    AbsolutePaths,
    /// CRLF on Windows, LF on macOS. Only for files the app reads as plain
    /// text; some apps checksum their files, so this is never automatic.
    LineEndings,
    /// Fonts missing on the target. Parsed now; translated in a later phase.
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

    /// The path's folder and file names.
    fn segments(&self) -> Vec<&str> {
        self.path.trim_end_matches('/').split('/').collect()
    }

    /// Which folder name, if any, uses `*`.
    pub fn wildcard(&self) -> Option<usize> {
        self.segments().iter().position(|part| part.contains('*'))
    }

    /// Whether this entry lists `rel`: the file itself, or anything inside
    /// the folder, with a `*` folder name matching any name it allows.
    fn lists(&self, rel: &str) -> bool {
        let pattern = self.segments();
        let parts: Vec<&str> = rel.split('/').collect();
        let fits = if self.is_dir() {
            parts.len() > pattern.len()
        } else {
            parts.len() == pattern.len()
        };
        fits && pattern
            .iter()
            .zip(&parts)
            .all(|(pattern, part)| wildcard::matches(pattern, part))
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
            let segments = entry.segments();
            let starred = segments.iter().filter(|part| part.contains('*')).count();
            let last_is_file = !entry.is_dir() && entry.wildcard() == Some(segments.len() - 1);
            if starred > 1 || last_is_file || entry.path.contains("**") {
                return Err(err(format!(
                    "file path {:?} may use * in one folder name only, not in a file name",
                    entry.path
                )));
            }
            for setting in entry.os_specific.keys() {
                let ok = setting
                    .split_once('.')
                    .is_some_and(|(section, key)| !section.is_empty() && !key.is_empty());
                if !ok {
                    return Err(err(format!(
                        "os_specific setting {setting:?} must be written section.key"
                    )));
                }
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

    /// The `files` entry that lists a relative path: the file itself, or
    /// the deepest folder holding it.
    pub fn entry(&self, rel_path: &str) -> Option<&FileEntry> {
        self.files
            .iter()
            .filter(|entry| entry.lists(rel_path))
            .max_by_key(|entry| (!entry.is_dir(), entry.path.len()))
    }

    /// Whether a relative path may be captured or restored for this app:
    /// listed in `files`, not excluded, and not a known secret file.
    pub fn allows(&self, rel_path: &str) -> bool {
        self.entry(rel_path).is_some()
            && !self.excluded.is_match(rel_path)
            && !secrets::is_denied(rel_path)
    }

    /// The settings files that exist under `root`, as sorted `/`-separated
    /// relative paths. Symlinks inside listed folders are skipped so a
    /// capture never wanders outside the app's folder.
    pub fn collect_files(&self, root: &Path) -> Result<Vec<String>> {
        let mut found = Vec::new();
        for entry in &self.files {
            for rel in self.expand(root, entry)? {
                let path = join_relative(root, &rel)?;
                if entry.is_dir() {
                    if path.is_dir() {
                        walk(&path, rel.trim_end_matches('/'), &mut found)?;
                    }
                } else if path.is_file() {
                    found.push(rel);
                }
            }
        }
        found.retain(|rel| self.allows(rel));
        found.sort();
        found.dedup();
        Ok(found)
    }
}

impl Recipe {
    /// The entry's path with its `*` folder name, if any, replaced by each
    /// matching folder under `root` (the newest version of each product).
    fn expand(&self, root: &Path, entry: &FileEntry) -> Result<Vec<String>> {
        let Some(index) = entry.wildcard() else {
            return Ok(vec![entry.path.clone()]);
        };
        let segments = entry.segments();
        let parent = join_relative(root, &segments[..index].join("/"))?;
        let names =
            wildcard::newest_per_product(wildcard::matching_dirs(&parent, segments[index])?);
        let trailing = if entry.is_dir() { "/" } else { "" };
        Ok(names
            .into_iter()
            .map(|name| {
                let mut parts = segments.clone();
                parts[index] = &name;
                format!("{}{trailing}", parts.join("/"))
            })
            .collect())
    }

    /// Where a file saved as `rel` goes under `root` on this computer: the
    /// same path, unless its entry has a `*` folder whose name differs here.
    pub fn place(&self, root: &Path, rel: &str) -> Result<Placement> {
        let same = Placement::Into {
            rel: rel.to_string(),
            change: None,
            warning: None,
        };
        match self
            .entry(rel)
            .and_then(|entry| entry.wildcard().map(|i| (entry, i)))
        {
            Some((entry, index)) => wildcard::place(root, &entry.segments(), index, rel),
            None => Ok(same),
        }
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
        assert_eq!(set.iter().count(), 30);
        for recipe in set.iter() {
            let both =
                recipe.location(Os::Windows).is_some() && recipe.location(Os::MacOs).is_some();
            // Settings can only move between OSes when the app is on both.
            assert!(both || !recipe.cross_os, "{} is on one OS only", recipe.id);
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

        // A * folder matches one folder name, whatever it is.
        let jetbrains = set.get("jetbrains").unwrap();
        assert!(jetbrains.allows("IntelliJIdea2025.2/options/editor.xml"));
        assert!(!jetbrains.allows("IntelliJIdea2025.2/options/jdk.table.xml"));
        assert!(!jetbrains.allows("IntelliJIdea2025.2/c.kdbx"));
        assert!(!jetbrains.allows("a/b/options/editor.xml"));
        let firefox = set.get("firefox").unwrap();
        assert!(firefox.allows("Profiles/k3x9.default-release/user.js"));
        assert!(!firefox.allows("Profiles/k3x9.default-release/prefs.js"));
        assert!(!firefox.allows("Profiles/k3x9.default-release/logins.json"));
        assert!(!firefox.allows("Profiles/k3x9.default/user.js"));
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
            "id: x\nname: x\nlocations: {macos: '{home}'}\nfiles: [{path: a, os_specific: {autocrlf: x}}]",
            // * only in one folder name, never in a file name.
            "id: x\nname: x\nlocations: {macos: '{home}'}\nfiles: [{path: '*/*/a'}]",
            "id: x\nname: x\nlocations: {macos: '{home}'}\nfiles: [{path: 'a/*.json'}]",
            "id: x\nname: x\nlocations: {macos: '{home}'}\nfiles: [{path: '**/a'}]",
        ];
        for yaml in bad {
            assert!(Recipe::parse("test", yaml).is_err(), "{yaml}");
        }
    }
}
