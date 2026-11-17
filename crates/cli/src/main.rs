use std::path::PathBuf;

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use settings_migrator_engine::{
    bundle::{self, Bundle},
    restore::{self, Action},
    scan::{scan, ScanStatus},
    undo::{UndoSet, UndoStep},
    Os, PathContext, RecipeSet, VERSION,
};

/// Move your app settings to a new computer.
#[derive(Parser)]
#[command(name = "settings-migrator", version = VERSION)]
struct Cli {
    /// Act as if this folder were your home folder (for trying things out safely).
    #[arg(long, global = true, value_name = "DIR")]
    home: Option<PathBuf>,

    /// Load recipes from this folder instead of the built-in ones.
    #[arg(long, global = true, value_name = "DIR")]
    recipes: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List apps with a recipe and the settings files that can be captured.
    Scan {
        /// Only these apps (recipe ids, such as vscode).
        #[arg(long = "app", value_name = "ID")]
        apps: Vec<String>,
    },
    /// Save settings into a bundle file you can move to another computer.
    Capture {
        /// Bundle file to create (it must not exist yet).
        #[arg(short, long, value_name = "FILE")]
        output: PathBuf,
        /// Only these apps (recipe ids, such as vscode).
        #[arg(long = "app", value_name = "ID")]
        apps: Vec<String>,
        /// Show what would be captured without writing the bundle.
        #[arg(long)]
        dry_run: bool,
    },
    /// Put the settings from a bundle onto this computer, backing up originals first.
    Restore {
        /// Bundle file made by `capture`.
        bundle: PathBuf,
        /// Only these apps (recipe ids, such as vscode).
        #[arg(long = "app", value_name = "ID")]
        apps: Vec<String>,
        /// Show what would change without writing anything.
        #[arg(long)]
        dry_run: bool,
    },
    /// Put back the files changed by the most recent restore.
    Undo {
        /// Show what would be put back without changing anything.
        #[arg(long)]
        dry_run: bool,
    },
}

fn main() {
    if let Err(err) = run(Cli::parse()) {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    let ctx = match &cli.home {
        Some(home) => {
            let Some(os) = Os::current() else {
                bail!("only Windows and macOS are supported");
            };
            PathContext::for_home(os, home)
        }
        None => PathContext::from_env()?,
    };
    let recipes = match &cli.recipes {
        Some(dir) => RecipeSet::load_dir(dir)?,
        None => RecipeSet::builtin()?,
    };

    match cli.command {
        Command::Scan { apps } => {
            for app in scan(&ctx, &recipes.select(&apps)?)? {
                let name = &app.recipe.name;
                match app.status {
                    ScanStatus::Unsupported => println!("-  {name}: no recipe for {}", ctx.os()),
                    ScanStatus::NotFound { root } => {
                        println!("-  {name}: not found ({})", root.display())
                    }
                    ScanStatus::Found { root, files } => {
                        println!("✓  {name} ({}) [{}]", root.display(), app.recipe.id);
                        for (rel, size) in files {
                            println!("     {rel}  ({})", human_size(size));
                        }
                    }
                }
            }
        }
        Command::Capture {
            output,
            apps,
            dry_run,
        } => {
            let selected = recipes.select(&apps)?;
            if dry_run {
                println!("Dry run: would capture into {}", output.display());
                let mut total = 0;
                for app in scan(&ctx, &selected)? {
                    if let ScanStatus::Found { files, .. } = app.status {
                        println!("  {} [{}]", app.recipe.name, app.recipe.id);
                        for (rel, size) in files {
                            println!("     {rel}  ({})", human_size(size));
                            total += 1;
                        }
                    }
                }
                println!("{}. Nothing was written.", files(total));
            } else {
                let manifest = bundle::capture(&ctx, &selected, &output)?;
                for app in &manifest.apps {
                    println!("  {} [{}]: {}", app.name, app.id, files(app.files.len()));
                }
                let total: usize = manifest.apps.iter().map(|app| app.files.len()).sum();
                println!("Captured {} into {}", files(total), output.display());
            }
        }
        Command::Restore {
            bundle,
            apps,
            dry_run,
        } => {
            let opened = Bundle::open(&bundle)?;
            let plan = restore::plan(&ctx, &recipes, &opened, &apps)?;
            if dry_run {
                println!("Dry run: nothing will be written.");
            }
            let mut current_app = "";
            for item in &plan.items {
                if item.app_id != current_app {
                    current_app = &item.app_id;
                    println!("  {} [{}]", item.app_name, item.app_id);
                }
                let shown = item
                    .target
                    .as_ref()
                    .map_or(item.rel.clone(), |target| target.display().to_string());
                let label = match &item.action {
                    Action::Create => "create   ".to_string(),
                    Action::Update => "update   ".to_string(),
                    Action::Unchanged => "unchanged".to_string(),
                    Action::Skip(reason) => format!("skip ({reason})"),
                };
                println!("     {label}  {shown}");
            }
            let writes = plan.writes().count();
            if dry_run {
                println!("{} would be written.", files(writes));
            } else {
                let name = bundle.file_name().unwrap_or_default().to_string_lossy();
                match restore::apply(&ctx, &plan, &name)? {
                    Some(set) => println!(
                        "Restored {}. Originals are backed up; run `undo` to put them back ({}).",
                        files(writes),
                        set.id
                    ),
                    None => println!("Nothing to restore; everything already matches."),
                }
            }
        }
        Command::Undo { dry_run } => {
            let Some(mut set) = UndoSet::latest(&ctx)? else {
                println!("Nothing to undo.");
                return Ok(());
            };
            println!(
                "{}restore of {} ({}):",
                if dry_run {
                    "Dry run: would undo the "
                } else {
                    "Undoing the "
                },
                set.record.bundle,
                set.id
            );
            for step in set.steps() {
                match step {
                    UndoStep::Restore(path) => println!("     put back  {}", path.display()),
                    UndoStep::Delete(path) => println!("     delete    {}", path.display()),
                }
            }
            if dry_run {
                println!("Nothing was changed.");
            } else {
                set.apply()?;
                println!("Done.");
            }
        }
    }
    Ok(())
}

fn files(n: usize) -> String {
    if n == 1 {
        "1 file".into()
    } else {
        format!("{n} files")
    }
}

fn human_size(bytes: u64) -> String {
    match bytes {
        0..1024 => format!("{bytes} B"),
        1024..1_048_576 => format!("{:.1} KB", bytes as f64 / 1024.0),
        _ => format!("{:.1} MB", bytes as f64 / 1_048_576.0),
    }
}
