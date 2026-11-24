//! Every recipe against its own sample settings in `recipes/samples/<id>/`
//! (see CONTRIBUTING.md). For each OS the recipe supports, this test:
//!
//! 1. puts the sample files where the app keeps its settings, in a fake home;
//! 2. saves them, and checks exactly the right files were saved: every
//!    sample file except those listed under `not_saved` or `flagged`;
//! 3. restores them on a fresh fake home with the same OS, and checks every
//!    file comes back byte for byte;
//! 4. for a cross-OS recipe, plans restoring them on the other OS, and checks
//!    nothing is skipped except files the recipe marks for one OS only.
//!
//! Fake homes go inside SETTINGS_MIGRATOR_HOME, which must be a folder in a
//! temp folder (see common/guard.rs); without it the test refuses to run.
//! Set SETTINGS_MIGRATOR_RECIPE=<id> to check one recipe.

#[path = "common/guard.rs"]
mod guard;

use std::{
    collections::BTreeSet,
    env, fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use settings_migrator_engine::{
    bundle::{self, Bundle, CaptureOptions},
    paths::join_relative,
    process::ProcessList,
    restore::{self, Action},
    Os, PathContext, Recipe, RecipeSet,
};

/// `recipes/samples/<id>/sample.yaml`.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct SampleInfo {
    /// Sample files that must never be saved: secrets, caches, excluded or
    /// older-version files. Paths as in the OS folders.
    #[serde(default)]
    not_saved: Vec<String>,
    /// Sample files with a fake secret in them, which the secret guard must
    /// flag. They're left out when saving, as the user would by default.
    /// In either list, an entry starting `windows/` or `macos/` applies to
    /// that OS's samples only.
    #[serde(default)]
    flagged: Vec<String>,
}

fn samples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../recipes/samples")
}

fn os_folder(os: Os) -> &'static str {
    match os {
        Os::Windows => "windows",
        Os::MacOs => "macos",
    }
}

fn other(os: Os) -> Os {
    match os {
        Os::Windows => Os::MacOs,
        Os::MacOs => Os::Windows,
    }
}

#[test]
fn every_recipe_saves_and_restores_its_samples() {
    let recipes = RecipeSet::builtin().unwrap();
    let only = env::var("SETTINGS_MIGRATOR_RECIPE")
        .ok()
        .filter(|id| !id.is_empty());
    if let Some(id) = &only {
        assert!(recipes.get(id).is_some(), "no recipe with id {id:?}");
    }
    let mut failures = Vec::new();
    for recipe in recipes.iter() {
        if only.as_ref().is_some_and(|id| *id != recipe.id) {
            continue;
        }
        if let Err(why) = check_recipe(&recipes, recipe) {
            failures.push(format!("{}: {why}", recipe.id));
        }
    }
    assert!(failures.is_empty(), "\n{}\n", failures.join("\n"));
}

#[test]
fn every_sample_folder_has_a_recipe() {
    let recipes = RecipeSet::builtin().unwrap();
    for entry in fs::read_dir(samples_dir()).unwrap() {
        let name = entry.unwrap().file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name.ends_with(".md") {
            continue;
        }
        assert!(
            recipes.get(&name).is_some(),
            "recipes/samples/{name} has no recipe"
        );
    }
}

fn check_recipe(recipes: &RecipeSet, recipe: &Recipe) -> Result<(), String> {
    check_samples(recipes, recipe, &samples_dir().join(&recipe.id))
}

/// The contributor kit's two templates, copied as they are, pass together,
/// so a new recipe starts from something that works.
#[test]
fn the_templates_pass_together() {
    let kit = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contributing");
    let dir = guard::fresh_dir("template-recipe");
    fs::copy(
        kit.join("recipe-template.yaml"),
        dir.path().join("example-app.yaml"),
    )
    .unwrap();
    let recipes = RecipeSet::load_dir(dir.path()).unwrap();
    let recipe = recipes
        .get("example-app")
        .expect("the template's id is example-app");
    check_samples(&recipes, recipe, &kit.join("sample-template")).unwrap();
}

fn check_samples(recipes: &RecipeSet, recipe: &Recipe, dir: &Path) -> Result<(), String> {
    if !dir.is_dir() {
        return Err(format!(
            "no samples; copy contributing/sample-template to recipes/samples/{}",
            recipe.id
        ));
    }
    let info: SampleInfo = match fs::read_to_string(dir.join("sample.yaml")) {
        Ok(yaml) => serde_yaml::from_str(&yaml).map_err(|e| format!("sample.yaml: {e}"))?,
        Err(_) => SampleInfo::default(),
    };
    for os in [Os::Windows, Os::MacOs] {
        let samples = dir.join(os_folder(os));
        match (recipe.location(os).is_some(), samples.is_dir()) {
            (true, true) => check_os(recipes, recipe, &info, os, &samples)
                .map_err(|why| format!("{os}: {why}"))?,
            (true, false) => {
                return Err(format!(
                    "the recipe supports {os} but there's no {} sample folder",
                    os_folder(os)
                ))
            }
            (false, true) => {
                return Err(format!(
                    "a {} sample folder, but the recipe has no {os} location",
                    os_folder(os)
                ))
            }
            (false, false) => {}
        }
    }
    Ok(())
}

fn check_os(
    recipes: &RecipeSet,
    recipe: &Recipe,
    info: &SampleInfo,
    os: Os,
    samples: &Path,
) -> Result<(), String> {
    let sample_files = files_under(samples);
    if sample_files.is_empty() {
        return Err(format!("{} is empty", samples.display()));
    }
    let not_saved = for_os(&info.not_saved, os);
    let want_flagged = for_os(&info.flagged, os);
    for (listed, one_os) in not_saved.iter().chain(&want_flagged) {
        let elsewhere = !one_os && other_os_has(samples, os, listed);
        if !sample_files.contains(listed) && !elsewhere {
            return Err(format!(
                "sample.yaml lists {listed}, which isn't a sample file"
            ));
        }
    }
    let not_saved: BTreeSet<String> = not_saved.into_iter().map(|(rel, _)| rel).collect();
    let want_flagged: BTreeSet<String> = want_flagged
        .into_iter()
        .map(|(rel, _)| rel)
        .filter(|rel| sample_files.contains(rel))
        .collect();

    // 1. The samples, where the app keeps its settings in a fake home.
    let source = guard::fresh_dir(&format!("{}-{}-saved", recipe.id, os_folder(os)));
    let ctx = PathContext::for_home(os, source.path());
    let root = ctx
        .resolve(recipe.location(os).unwrap())
        .map_err(|e| e.to_string())?;
    for rel in &sample_files {
        copy(
            &join_relative(samples, rel).unwrap(),
            &join_relative(&root, rel).unwrap(),
        );
    }

    // 2. Saving: every sample file except those that must be left out.
    let mut options = CaptureOptions::default();
    let plan = bundle::plan_capture(&ctx, &[recipe], &options).map_err(|e| e.to_string())?;
    let found: BTreeSet<String> = plan.files.iter().map(|file| file.rel.clone()).collect();
    let flagged: BTreeSet<String> = plan
        .files
        .iter()
        .filter(|file| !file.findings.is_empty())
        .map(|file| file.rel.clone())
        .collect();
    if flagged != want_flagged {
        return Err(format!(
            "the secret guard flagged {flagged:?}, but sample.yaml lists {want_flagged:?} under flagged \
             (sample files must not look like secrets unless listed there)"
        ));
    }
    let want: BTreeSet<String> = sample_files
        .iter()
        .filter(|rel| !not_saved.contains(*rel))
        .cloned()
        .collect();
    if found != want {
        let extra: Vec<_> = found.difference(&want).collect();
        let missing: Vec<_> = want.difference(&found).collect();
        return Err(format!(
            "saving found the wrong files; not expected: {extra:?} (add to not_saved if they must be \
             left out); missing: {missing:?} (does the recipe list them?)"
        ));
    }
    options.skip = want_flagged
        .iter()
        .map(|rel| format!("{}/{rel}", recipe.id))
        .collect();
    let work = guard::fresh_dir(&format!("{}-{}-bundle", recipe.id, os_folder(os)));
    let bundle_path = work.path().join("bundle.zip");
    bundle::capture(&ctx, &[recipe], &options, &bundle_path).map_err(|e| e.to_string())?;
    let bundle = Bundle::open(&bundle_path).map_err(|e| e.to_string())?;
    let saved: Vec<String> = bundle
        .apps
        .iter()
        .flat_map(|app| app.files.iter().map(|(rel, _)| rel.clone()))
        .collect();

    // 3. Restoring on the same OS brings every saved file back exactly.
    let target = guard::fresh_dir(&format!("{}-{}-restored", recipe.id, os_folder(os)));
    let target_ctx = PathContext::for_home(os, target.path());
    open_app_once(recipe, &target_ctx, &saved);
    let ids = [recipe.id.clone()];
    if !saved.is_empty() {
        let plan = restore::plan(&target_ctx, recipes, &bundle, &ids).map_err(|e| e.to_string())?;
        for item in &plan.items {
            if item.action != Action::Create {
                return Err(format!("restoring {} would {:?}", item.rel, item.action));
            }
        }
        let none_open = ProcessList::from_names(os, Vec::<String>::new());
        restore::apply(&target_ctx, recipes, &plan, "bundle.zip", &none_open)
            .map_err(|e| e.to_string())?;
        let target_root = target_ctx.resolve(recipe.location(os).unwrap()).unwrap();
        for rel in &saved {
            let want = fs::read(join_relative(&root, rel).unwrap()).unwrap();
            let got = fs::read(join_relative(&target_root, rel).unwrap())
                .map_err(|e| format!("{rel} wasn't restored: {e}"))?;
            if got != want {
                return Err(format!("{rel} came back different"));
            }
        }
    }

    // 4. Restoring on the other OS skips nothing it shouldn't.
    let to = other(os);
    if recipe.cross_os && recipe.location(to).is_some() && !saved.is_empty() {
        let target = guard::fresh_dir(&format!(
            "{}-{}-to-{}",
            recipe.id,
            os_folder(os),
            os_folder(to)
        ));
        let target_ctx = PathContext::for_home(to, target.path());
        open_app_once(recipe, &target_ctx, &saved);
        let plan = restore::plan(&target_ctx, recipes, &bundle, &ids).map_err(|e| e.to_string())?;
        for item in &plan.items {
            let os_only = recipe.entry(&item.rel).and_then(|entry| entry.os).is_some();
            match &item.action {
                Action::Create => {}
                Action::Skip(_) if os_only => {}
                other => return Err(format!("restoring {} on {to} would {other:?}", item.rel)),
            }
        }
    }
    Ok(())
}

/// Makes the folders an app creates on its first run that a recipe's `*`
/// folder names match, as if the app had been opened once on the new
/// computer: restore puts files only into a folder that already exists.
fn open_app_once(recipe: &Recipe, ctx: &PathContext, saved: &[String]) {
    let Some(location) = recipe.location(ctx.os()) else {
        return;
    };
    let root = ctx.resolve(location).unwrap();
    for rel in saved {
        if let Some(index) = recipe.entry(rel).and_then(|entry| entry.wildcard()) {
            let folder: Vec<&str> = rel.split('/').take(index + 1).collect();
            fs::create_dir_all(join_relative(&root, &folder.join("/")).unwrap()).unwrap();
        }
    }
}

/// The entries of a sample.yaml list that apply on `os`: all of them, but
/// an entry starting `windows/` or `macos/` only on that OS (with the
/// prefix removed, and `true` to say it's for one OS only).
fn for_os(list: &[String], os: Os) -> Vec<(String, bool)> {
    list.iter()
        .filter_map(|entry| match entry.split_once('/') {
            Some((prefix, rel)) if prefix == os_folder(os) => Some((rel.to_string(), true)),
            Some((prefix, _)) if prefix == os_folder(other(os)) => None,
            _ => Some((entry.clone(), false)),
        })
        .collect()
}

/// Whether the other OS's sample folder has `rel` (sample.yaml covers both).
fn other_os_has(samples: &Path, os: Os, rel: &str) -> bool {
    let other = samples.parent().unwrap().join(os_folder(other(os)));
    join_relative(&other, rel).is_ok_and(|path| path.is_file())
}

/// Every file under `dir`, as sorted `/`-separated relative paths.
fn files_under(dir: &Path) -> BTreeSet<String> {
    fn walk(dir: &Path, rel: &str, out: &mut BTreeSet<String>) {
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name().to_string_lossy().to_string();
            let child = if rel.is_empty() {
                name
            } else {
                format!("{rel}/{name}")
            };
            if entry.file_type().unwrap().is_dir() {
                walk(&entry.path(), &child, out);
            } else {
                out.insert(child);
            }
        }
    }
    let mut out = BTreeSet::new();
    if dir.is_dir() {
        walk(dir, "", &mut out);
    }
    out
}

fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to.parent().unwrap()).unwrap();
    fs::copy(from, to).unwrap();
}
