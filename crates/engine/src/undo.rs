//! Undo sets: a record of every file a restore touched, with a backup of
//! each original, kept under the app's state folder until undone.

use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    time::SystemTime,
};

use serde::{Deserialize, Serialize};

use crate::{restore::write_file, Error, PathContext, Result};

const RECORD: &str = "undo.json";
const BACKUPS: &str = "backups";

#[derive(Debug)]
pub struct UndoSet {
    pub id: String,
    dir: PathBuf,
    pub record: UndoRecord,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UndoRecord {
    pub created: String,
    /// File name of the bundle that was restored.
    pub bundle: String,
    pub entries: Vec<UndoEntry>,
    pub undone: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UndoEntry {
    pub app_id: String,
    pub rel: String,
    pub target: PathBuf,
    /// Name of the backup of the original under `backups/`, or `None` if the
    /// restore created the file (undo then deletes it).
    pub backup: Option<String>,
    /// Folders the restore created for this file, deepest first.
    #[serde(default)]
    pub created_dirs: Vec<PathBuf>,
}

/// One step of an undo, for dry-run output.
#[derive(Debug, PartialEq, Eq)]
pub enum UndoStep {
    /// Put the original contents back.
    Restore(PathBuf),
    /// Delete a file the restore created.
    Delete(PathBuf),
}

fn undo_root(ctx: &PathContext) -> PathBuf {
    ctx.state_dir().join("undo")
}

impl UndoSet {
    pub(crate) fn create(ctx: &PathContext, bundle: &str) -> Result<UndoSet> {
        let created = humantime::format_rfc3339_seconds(SystemTime::now()).to_string();
        let base = created.replace(':', "-");
        let root = undo_root(ctx);
        fs::create_dir_all(&root).map_err(|e| Error::io(&root, e))?;

        // Two restores in the same second get -02, -03, ...
        let mut n = 1;
        let (id, dir) = loop {
            let id = if n == 1 {
                base.clone()
            } else {
                format!("{base}-{n:02}")
            };
            let dir = root.join(&id);
            match fs::create_dir(&dir) {
                Ok(()) => break (id, dir),
                Err(e) if e.kind() == ErrorKind::AlreadyExists => n += 1,
                Err(e) => return Err(Error::io(&dir, e)),
            }
        };
        let backups = dir.join(BACKUPS);
        fs::create_dir(&backups).map_err(|e| Error::io(&backups, e))?;
        Ok(UndoSet {
            id,
            dir,
            record: UndoRecord {
                created,
                bundle: bundle.to_string(),
                entries: Vec::new(),
                undone: false,
            },
        })
    }

    /// Copies the current contents of `target` into the set; returns the
    /// backup's name.
    pub(crate) fn back_up(&mut self, target: &Path) -> Result<String> {
        let name = format!("{:04}", self.record.entries.len() + 1);
        let backup = self.dir.join(BACKUPS).join(&name);
        fs::copy(target, &backup).map_err(|e| Error::io(target, e))?;
        Ok(name)
    }

    pub(crate) fn record(&mut self, entry: UndoEntry) {
        self.record.entries.push(entry);
    }

    pub(crate) fn save(&self) -> Result<()> {
        let json =
            serde_json::to_vec_pretty(&self.record).map_err(|e| Error::Other(e.to_string()))?;
        write_file(&self.dir.join(RECORD), &json)
    }

    /// The most recent restore that hasn't been undone.
    pub fn latest(ctx: &PathContext) -> Result<Option<UndoSet>> {
        Ok(UndoSet::list(ctx)?
            .into_iter()
            .rev()
            .find(|set| !set.record.undone))
    }

    /// Every undo set, oldest first.
    pub fn list(ctx: &PathContext) -> Result<Vec<UndoSet>> {
        let root = undo_root(ctx);
        let entries = match fs::read_dir(&root) {
            Ok(entries) => entries,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(Error::io(&root, e)),
        };
        let mut sets = Vec::new();
        for entry in entries {
            let dir = entry.map_err(|e| Error::io(&root, e))?.path();
            let record_path = dir.join(RECORD);
            // A set without a record never got as far as writing any settings.
            let Ok(json) = fs::read(&record_path) else {
                continue;
            };
            let record = serde_json::from_slice(&json)
                .map_err(|e| Error::Other(format!("{}: {e}", record_path.display())))?;
            let id = dir
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            sets.push(UndoSet { id, dir, record });
        }
        sets.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(sets)
    }

    /// What undoing would do, without changing anything.
    pub fn steps(&self) -> Vec<UndoStep> {
        self.record
            .entries
            .iter()
            .map(|entry| match entry.backup {
                Some(_) => UndoStep::Restore(entry.target.clone()),
                None => UndoStep::Delete(entry.target.clone()),
            })
            .collect()
    }

    /// Puts every file back as it was before the restore, then marks the set
    /// as undone so it isn't applied twice.
    pub fn apply(&mut self) -> Result<()> {
        if self.record.undone {
            return Err(Error::Other(format!("{} was already undone", self.id)));
        }
        for entry in self.record.entries.iter().rev() {
            match &entry.backup {
                Some(name) => {
                    let backup = self.dir.join(BACKUPS).join(name);
                    let original = fs::read(&backup).map_err(|e| Error::io(&backup, e))?;
                    write_file(&entry.target, &original)?;
                }
                None => match fs::remove_file(&entry.target) {
                    Ok(()) => {}
                    // A failed restore may never have written it.
                    Err(e) if e.kind() == ErrorKind::NotFound => {}
                    Err(e) => return Err(Error::io(&entry.target, e)),
                },
            }
            for dir in &entry.created_dirs {
                // Only removes empty folders; anything added since stays.
                let _ = fs::remove_dir(dir);
            }
        }
        self.record.undone = true;
        self.save()
    }
}
