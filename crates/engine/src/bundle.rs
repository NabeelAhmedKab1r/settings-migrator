//! The portable bundle: a zip holding `manifest.json` plus each settings file
//! under a logical path such as `vscode/settings.json`. No absolute paths are
//! stored, so a bundle doesn't depend on the computer that made it.

use std::{
    fs,
    io::{Read, Write},
    path::Path,
    time::SystemTime,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

use crate::{paths::join_relative, Error, Os, PathContext, Recipe, Result, VERSION};

pub const FORMAT_VERSION: u32 = 1;
const MANIFEST: &str = "manifest.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub format_version: u32,
    pub tool_version: String,
    pub source_os: Os,
    /// When the bundle was made, in RFC 3339 (UTC).
    pub created: String,
    pub apps: Vec<ManifestApp>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestApp {
    pub id: String,
    pub name: String,
    pub files: Vec<ManifestFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestFile {
    /// `/`-separated path relative to the app's settings folder.
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

/// A bundle read into memory, with every file's hash already checked.
#[derive(Debug)]
pub struct Bundle {
    pub manifest: Manifest,
    pub apps: Vec<BundleApp>,
}

#[derive(Debug)]
pub struct BundleApp {
    pub id: String,
    pub name: String,
    /// Relative paths and contents.
    pub files: Vec<(String, Vec<u8>)>,
}

/// Captures the settings of `recipes` found on this computer into a new
/// bundle at `out`. Apps with no settings files are left out. Never
/// overwrites an existing file.
pub fn capture(ctx: &PathContext, recipes: &[&Recipe], out: &Path) -> Result<Manifest> {
    if out.exists() {
        return Err(Error::Other(format!(
            "{} already exists; choose another name or delete it first",
            out.display()
        )));
    }

    let mut manifest = Manifest {
        format_version: FORMAT_VERSION,
        tool_version: VERSION.to_string(),
        source_os: ctx.os(),
        created: humantime::format_rfc3339_seconds(SystemTime::now()).to_string(),
        apps: Vec::new(),
    };
    let mut contents = Vec::new();
    for &recipe in recipes {
        let Some(location) = recipe.location(ctx.os()) else {
            continue;
        };
        let root = ctx.resolve(location)?;
        let mut app = ManifestApp {
            id: recipe.id.clone(),
            name: recipe.name.clone(),
            files: Vec::new(),
        };
        for rel in recipe.collect_files(&root)? {
            let path = join_relative(&root, &rel)?;
            let bytes = fs::read(&path).map_err(|e| Error::io(&path, e))?;
            app.files.push(ManifestFile {
                path: rel.clone(),
                size: bytes.len() as u64,
                sha256: sha256_hex(&bytes),
            });
            contents.push((format!("{}/{rel}", recipe.id), bytes));
        }
        if !app.files.is_empty() {
            manifest.apps.push(app);
        }
    }

    // Write next to the target and rename, so a failed capture never leaves
    // a half-written bundle behind under the real name.
    let partial = out.with_extension("partial");
    let result = write_zip(&partial, &manifest, &contents)
        .and_then(|()| fs::rename(&partial, out).map_err(|e| Error::io(out, e)));
    if result.is_err() {
        let _ = fs::remove_file(&partial);
    }
    result.map(|()| manifest)
}

fn write_zip(path: &Path, manifest: &Manifest, contents: &[(String, Vec<u8>)]) -> Result<()> {
    let file = fs::File::create(path).map_err(|e| Error::io(path, e))?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let zip_err = |e: zip::result::ZipError| Error::Other(format!("{}: {e}", path.display()));

    let json = serde_json::to_vec_pretty(manifest).map_err(|e| Error::Other(e.to_string()))?;
    zip.start_file(MANIFEST, options).map_err(zip_err)?;
    zip.write_all(&json).map_err(|e| Error::io(path, e))?;
    for (name, bytes) in contents {
        zip.start_file(name.as_str(), options).map_err(zip_err)?;
        zip.write_all(bytes).map_err(|e| Error::io(path, e))?;
    }
    zip.finish().map_err(zip_err)?;
    Ok(())
}

impl Bundle {
    /// Reads a bundle and checks it: known format, safe paths, and every
    /// file present with the size and hash the manifest records.
    pub fn open(path: &Path) -> Result<Bundle> {
        let bad = |message: String| Error::Other(format!("{}: {message}", path.display()));
        let file = fs::File::open(path).map_err(|e| Error::io(path, e))?;
        let mut zip = ZipArchive::new(file).map_err(|e| bad(format!("not a bundle ({e})")))?;

        let manifest: Manifest = {
            let entry = zip
                .by_name(MANIFEST)
                .map_err(|_| bad("no manifest.json".into()))?;
            serde_json::from_reader(entry).map_err(|e| bad(format!("manifest.json: {e}")))?
        };
        if manifest.format_version != FORMAT_VERSION {
            return Err(bad(format!(
                "bundle format {} isn't supported (this version reads format {FORMAT_VERSION})",
                manifest.format_version
            )));
        }

        let mut apps = Vec::new();
        for app in &manifest.apps {
            if app.id.is_empty() || app.id.contains(['/', '\\', '.']) {
                return Err(bad(format!("unsafe app id {:?}", app.id)));
            }
            let mut files = Vec::new();
            for file in &app.files {
                if file.path.ends_with('/') || join_relative(Path::new(""), &file.path).is_err() {
                    return Err(bad(format!("unsafe path {:?} for {}", file.path, app.id)));
                }
                let name = format!("{}/{}", app.id, file.path);
                let mut entry = zip
                    .by_name(&name)
                    .map_err(|_| bad(format!("{name} is missing")))?;
                let mut bytes = Vec::new();
                // Read one byte past the recorded size to catch oversized entries.
                (&mut entry)
                    .take(file.size + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|e| bad(format!("{name}: {e}")))?;
                if bytes.len() as u64 != file.size || sha256_hex(&bytes) != file.sha256 {
                    return Err(bad(format!(
                        "{name} doesn't match its hash; the bundle is damaged or was changed"
                    )));
                }
                files.push((file.path.clone(), bytes));
            }
            apps.push(BundleApp {
                id: app.id.clone(),
                name: app.name.clone(),
                files,
            });
        }
        Ok(Bundle { manifest, apps })
    }
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
