use std::path::PathBuf;

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use settings_migrator_engine::{
    scan::{scan, ScanStatus},
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
    }
    Ok(())
}

fn human_size(bytes: u64) -> String {
    match bytes {
        0..1024 => format!("{bytes} B"),
        1024..1_048_576 => format!("{:.1} KB", bytes as f64 / 1024.0),
        _ => format!("{:.1} MB", bytes as f64 / 1_048_576.0),
    }
}
