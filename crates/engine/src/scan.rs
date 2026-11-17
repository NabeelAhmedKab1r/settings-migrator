//! Finds which apps with a recipe have settings on this computer.

use std::{fs, path::PathBuf};

use crate::{paths::join_relative, PathContext, Recipe, Result};

#[derive(Debug)]
pub struct AppScan<'a> {
    pub recipe: &'a Recipe,
    pub status: ScanStatus,
}

#[derive(Debug)]
pub enum ScanStatus {
    /// The recipe has no location for this OS.
    Unsupported,
    /// The app's settings folder doesn't exist.
    NotFound { root: PathBuf },
    /// Settings files that can be captured, with their sizes in bytes.
    Found {
        root: PathBuf,
        files: Vec<(String, u64)>,
    },
}

pub fn scan<'a>(ctx: &PathContext, recipes: &[&'a Recipe]) -> Result<Vec<AppScan<'a>>> {
    let mut results = Vec::new();
    for &recipe in recipes {
        let status = match recipe.location(ctx.os()) {
            None => ScanStatus::Unsupported,
            Some(location) => {
                let root = ctx.resolve(location)?;
                let files = recipe.collect_files(&root)?;
                if files.is_empty() {
                    ScanStatus::NotFound { root }
                } else {
                    let mut sized = Vec::new();
                    for rel in files {
                        let path = join_relative(&root, &rel)?;
                        let size = fs::metadata(&path)
                            .map_err(|e| crate::Error::io(&path, e))?
                            .len();
                        sized.push((rel, size));
                    }
                    ScanStatus::Found { root, files: sized }
                }
            }
        };
        results.push(AppScan { recipe, status });
    }
    Ok(results)
}
