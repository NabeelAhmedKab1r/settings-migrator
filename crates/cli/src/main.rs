use std::{
    io::{self, IsTerminal, Write},
    path::PathBuf,
};

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use settings_migrator_engine::{
    bundle::{self, Bundle, CaptureOptions, Review},
    preview::{DiffLine, Preview},
    process::ProcessList,
    restore::{self, Action},
    scan::{scan, ScanStatus},
    undo::{UndoSet, UndoStep},
    Os, PathContext, RecipeSet, VERSION,
};

/// Move your app settings to a new computer.
#[derive(Parser)]
#[command(name = "settings-migrator", version = VERSION)]
struct Cli {
    /// Act as if this folder were your home folder (for trying things out
    /// safely). Restore and undo then skip checking whether apps are open.
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
        /// Leave out a file flagged as a possible secret (such as vscode/settings.json).
        #[arg(long, value_name = "APP/FILE")]
        skip: Vec<String>,
        /// Include a flagged file after checking it holds no secret.
        #[arg(long, value_name = "APP/FILE")]
        allow: Vec<String>,
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
        /// Restore without asking for confirmation after the preview.
        #[arg(long, short)]
        yes: bool,
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
    // With --home the files are a sandbox no running app reads, so whether
    // an app is open doesn't matter.
    let open_apps = || match cli.home {
        Some(_) => Ok(ProcessList::from_names(ctx.os(), Vec::<String>::new())),
        None => ProcessList::current(),
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
            skip,
            allow,
        } => {
            let selected = recipes.select(&apps)?;
            let plan = bundle::plan_capture(&ctx, &selected, &CaptureOptions { skip, allow })?;
            if dry_run {
                println!("Dry run: would capture into {}", output.display());
            }
            let mut current_app = "";
            for file in &plan.files {
                if file.app_id != current_app {
                    current_app = &file.app_id;
                    println!("  {} [{}]", file.app_name, file.app_id);
                }
                let note = match (file.review, file.findings.is_empty()) {
                    (Review::Skipped, _) => "  skipped",
                    (Review::Allowed, _) => "  flagged, allowed",
                    (Review::None, false) => "  FLAGGED",
                    (Review::None, true) => "",
                };
                println!("     {}  ({}){note}", file.rel, human_size(file.size));
            }

            let blocking: Vec<_> = plan.blocking().collect();
            if !blocking.is_empty() {
                println!();
                println!("Possible secrets found. Check each one:");
                for file in &blocking {
                    println!("  {}", file.logical_path());
                    for finding in &file.findings {
                        println!(
                            "     line {}: {}  {}",
                            finding.line, finding.kind, finding.excerpt
                        );
                    }
                }
                println!();
                println!("Then run capture again, adding for each file:");
                println!("  --skip APP/FILE   to leave the file out of the bundle, or");
                println!("  --allow APP/FILE  if you're sure it holds no secret.");
                if !dry_run {
                    bail!("nothing was captured");
                }
            }

            let included = plan.included().count();
            if dry_run {
                println!(
                    "{} would be captured. Nothing was written.",
                    files(included)
                );
            } else {
                plan.write(&output)?;
                println!("Captured {} into {}", files(included), output.display());
            }
        }
        Command::Restore {
            bundle,
            apps,
            dry_run,
            yes,
        } => {
            let opened = Bundle::open(&bundle)?;
            let plan = restore::plan(&ctx, &recipes, &opened, &apps)?;
            if dry_run {
                println!("Dry run: nothing will be written.");
            }
            let color = use_color();
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
                print_preview(&item.preview(), color);
            }

            let writes = plan.writes().count();
            let running = plan.running_apps(&recipes, &open_apps()?);
            if !running.is_empty() {
                println!();
                println!("{} open. Quit it before restoring: an open app can overwrite its settings when it quits.", running.join(" and "));
            }
            if dry_run {
                println!("{} would be written.", files(writes));
                return Ok(());
            }
            if writes == 0 {
                println!("Nothing to restore; everything already matches.");
                return Ok(());
            }
            if !running.is_empty() {
                bail!("nothing was restored");
            }
            if !confirm(
                yes,
                &format!("Restore {}? Originals are backed up first.", files(writes)),
            )? {
                println!("Nothing was written.");
                return Ok(());
            }

            // Check again: an app may have been opened while the prompt waited.
            let name = bundle.file_name().unwrap_or_default().to_string_lossy();
            match restore::apply(&ctx, &recipes, &plan, &name, &open_apps()?)? {
                Some(set) => println!(
                    "Restored {}. Run `undo` to put the originals back ({}).",
                    files(writes),
                    set.id
                ),
                None => println!("Nothing to restore; everything already matches."),
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
            let running = set.running_apps(&recipes, &open_apps()?);
            if !running.is_empty() {
                println!("{} open. Quit it before undoing.", running.join(" and "));
            }
            if dry_run {
                println!("Nothing was changed.");
            } else {
                set.apply(&recipes, &open_apps()?)?;
                println!("Done.");
            }
        }
    }
    Ok(())
}

/// Asks a yes/no question; `--yes` answers it. Without a terminal to ask
/// on, refuses rather than guessing.
fn confirm(yes: bool, question: &str) -> Result<bool> {
    if yes {
        return Ok(true);
    }
    if !io::stdin().is_terminal() {
        bail!("no terminal to confirm on; review the preview with --dry-run, then pass --yes");
    }
    print!("{question} [y/N] ");
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

fn use_color() -> bool {
    // Older Windows consoles print escape codes literally.
    io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none() && !cfg!(windows)
}

fn print_preview(preview: &Preview, color: bool) {
    const INDENT: &str = "                ";
    let paint = |code: &str, text: &str| {
        if color {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    };
    match preview {
        Preview::Nothing => {}
        Preview::LineEndingsOnly => println!("{INDENT}(only line endings differ)"),
        Preview::Binary {
            old: Some(old),
            new,
        } => println!(
            "{INDENT}(binary file, {} → {})",
            human_size(*old),
            human_size(*new)
        ),
        Preview::Binary { old: None, new } => {
            println!("{INDENT}(binary file, {})", human_size(*new))
        }
        Preview::Text { lines, hidden } => {
            for line in lines {
                match line {
                    DiffLine::Hunk(text) => println!("{INDENT}{}", paint("36", text)),
                    DiffLine::Context(text) => println!("{INDENT} {text}"),
                    DiffLine::Added(text) => {
                        println!("{INDENT}{}", paint("32", &format!("+{text}")))
                    }
                    DiffLine::Removed(text) => {
                        println!("{INDENT}{}", paint("31", &format!("-{text}")))
                    }
                }
            }
            if *hidden > 0 {
                println!("{INDENT}… {hidden} more lines not shown");
            }
        }
    }
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
