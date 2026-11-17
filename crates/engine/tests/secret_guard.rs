//! The secret guard on fake settings seeded with fake tokens, in temp folders.

use std::{fs, path::Path};

use settings_migrator_engine::{
    bundle::{self, Bundle, CaptureOptions, Review},
    paths::join_relative,
    Os, PathContext, RecipeSet,
};
use tempfile::TempDir;

/// Builds a fake token at runtime so this file never contains one.
fn fake_token(prefix: &str) -> String {
    let body: String = "aB3dE5fG7hJ9kL2mN4pQ6rS8tU0vW1xY3zA5bC7d"
        .chars()
        .take(36)
        .collect();
    format!("{prefix}{body}")
}

fn write(ctx: &PathContext, recipes: &RecipeSet, app: &str, rel: &str, contents: &str) {
    let recipe = recipes.get(app).unwrap();
    let root = ctx.resolve(recipe.location(ctx.os()).unwrap()).unwrap();
    let path = join_relative(&root, rel).unwrap();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

/// A fake home whose settings hide three fake secrets, next to clean files.
fn seeded_home(recipes: &RecipeSet) -> (TempDir, PathContext) {
    let home = TempDir::new().unwrap();
    let ctx = PathContext::for_home(Os::current().unwrap(), home.path());
    let token = fake_token("ghp_");
    write(
        &ctx,
        recipes,
        "vscode",
        "settings.json",
        &format!(
            "{{\n  \"editor.fontSize\": 14,\n  \"terminal.integrated.env.osx\": {{ \"GITHUB_TOKEN\": \"{token}\" }}\n}}\n"
        ),
    );
    write(&ctx, recipes, "vscode", "keybindings.json", "[]");
    write(
        &ctx,
        recipes,
        "git",
        ".gitconfig",
        "[user]\n\tname = Test\n[remote \"origin\"]\n\turl = https://test:S3cretPassw0rd@example.com/r.git\n",
    );
    write(
        &ctx,
        recipes,
        "obs",
        "basic/profiles/Main/basic.ini",
        "[Output]\nMode=Simple\n[Stream]\nkey=live_8f3KzQ2mX9pL4vR7tY1wB6nC\n",
    );
    write(&ctx, recipes, "vlc", "vlcrc", "[core]\nvolume-step=12.8\n");
    (home, ctx)
}

fn flagged_paths(plan: &bundle::CapturePlan) -> Vec<String> {
    plan.blocking().map(|f| f.logical_path()).collect()
}

#[test]
fn seeded_tokens_block_the_capture() {
    let recipes = RecipeSet::builtin().unwrap();
    let (_home, ctx) = seeded_home(&recipes);
    let all = recipes.select(&[]).unwrap();

    let plan = bundle::plan_capture(&ctx, &all, &CaptureOptions::default()).unwrap();
    assert_eq!(
        flagged_paths(&plan),
        [
            "git/.gitconfig",
            "obs/basic/profiles/Main/basic.ini",
            "vscode/settings.json"
        ]
    );
    let vscode = plan
        .files
        .iter()
        .find(|f| f.logical_path() == "vscode/settings.json")
        .unwrap();
    assert_eq!(vscode.findings.len(), 1);
    assert_eq!(vscode.findings[0].line, 3);
    assert_eq!(vscode.findings[0].kind, "GitHub token");
    assert!(!vscode.findings[0]
        .excerpt
        .contains(&fake_token("ghp_")[4..]));

    // Blocked: no bundle, not even a partial one.
    let work = TempDir::new().unwrap();
    let out = work.path().join("bundle.zip");
    let err = plan.write(&out).unwrap_err().to_string();
    assert!(err.contains("possible secrets"), "{err}");
    assert_eq!(fs::read_dir(work.path()).unwrap().count(), 0);
}

#[test]
fn skipped_and_allowed_files_unblock_it() {
    let recipes = RecipeSet::builtin().unwrap();
    let (_home, ctx) = seeded_home(&recipes);
    let all = recipes.select(&[]).unwrap();
    let options = CaptureOptions {
        skip: vec![
            "vscode/settings.json".into(),
            "obs/basic/profiles/Main/basic.ini".into(),
        ],
        allow: vec!["git/.gitconfig".into()],
    };

    let plan = bundle::plan_capture(&ctx, &all, &options).unwrap();
    assert_eq!(flagged_paths(&plan), Vec::<String>::new());
    let git = plan.files.iter().find(|f| f.rel == ".gitconfig").unwrap();
    assert_eq!(git.review, Review::Allowed);

    let work = TempDir::new().unwrap();
    let out = work.path().join("bundle.zip");
    plan.write(&out).unwrap();
    let opened = Bundle::open(&out).unwrap();
    let paths: Vec<String> = opened
        .apps
        .iter()
        .flat_map(|app| {
            app.files
                .iter()
                .map(move |(rel, _)| format!("{}/{rel}", app.id))
        })
        .collect();
    assert!(paths.contains(&"git/.gitconfig".to_string()));
    assert!(paths.contains(&"vscode/keybindings.json".to_string()));
    assert!(!paths.contains(&"vscode/settings.json".to_string()));
    assert!(!paths.iter().any(|p| p.ends_with("basic.ini")));
}

#[test]
fn review_options_must_name_real_files() {
    let recipes = RecipeSet::builtin().unwrap();
    let (_home, ctx) = seeded_home(&recipes);
    let all = recipes.select(&[]).unwrap();
    let bad = [
        CaptureOptions {
            skip: vec!["vscode/setings.json".into()],
            ..Default::default()
        },
        // Nothing flagged in it, so there's nothing to allow.
        CaptureOptions {
            allow: vec!["vlc/vlcrc".into()],
            ..Default::default()
        },
        CaptureOptions {
            skip: vec!["git/.gitconfig".into()],
            allow: vec!["git/.gitconfig".into()],
        },
    ];
    for options in bad {
        assert!(
            bundle::plan_capture(&ctx, &all, &options).is_err(),
            "{options:?}"
        );
    }
}

#[test]
fn clean_settings_are_not_flagged() {
    // The ordinary fixtures from the round-trip tests must pass the guard.
    let recipes = RecipeSet::builtin().unwrap();
    let home = TempDir::new().unwrap();
    let ctx = PathContext::for_home(Os::current().unwrap(), home.path());
    write(
        &ctx,
        &recipes,
        "vscode",
        "settings.json",
        include_str!("fixtures/vscode-settings.json"),
    );
    write(
        &ctx,
        &recipes,
        "git",
        ".gitconfig",
        include_str!("fixtures/gitconfig"),
    );
    let plan = bundle::plan_capture(
        &ctx,
        &recipes.select(&[]).unwrap(),
        &CaptureOptions::default(),
    )
    .unwrap();
    assert_eq!(plan.files.len(), 2);
    assert_eq!(flagged_paths(&plan), Vec::<String>::new());
    assert!(!Path::new(&home.path().join("bundle.zip")).exists());
}
