//! Embeds every recipe in the workspace `recipes/` folder into the engine, so
//! the binary works without a recipes folder next to it.

use std::{env, fs, path::Path};

fn main() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../recipes");
    println!("cargo:rerun-if-changed={}", dir.display());

    let mut recipes: Vec<_> = fs::read_dir(&dir)
        .expect("recipes folder")
        .map(|entry| entry.expect("recipe entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "yaml"))
        .collect();
    recipes.sort();

    let mut out = String::from("&[\n");
    for path in recipes {
        let name = path.file_name().unwrap().to_string_lossy();
        out += &format!(
            "    ({name:?}, include_str!({:?})),\n",
            path.display().to_string()
        );
    }
    out += "]\n";

    let dest = Path::new(&env::var("OUT_DIR").unwrap()).join("builtin_recipes.rs");
    fs::write(dest, out).expect("write builtin_recipes.rs");
}
