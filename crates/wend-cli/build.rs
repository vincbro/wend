//! Lists the files in the repo's `themes` folder as the built-in themes, so adding a theme is
//! adding a file.

use std::{
    env,
    error::Error,
    fmt::Write,
    fs,
    path::{Path, PathBuf},
};

fn main() -> Result<(), Box<dyn Error>> {
    let themes = Path::new(&env::var("CARGO_MANIFEST_DIR")?).join("../../themes");
    println!("cargo:rerun-if-changed={}", themes.display());

    let mut files: Vec<PathBuf> = fs::read_dir(&themes)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<_, _>>()?;
    files.retain(|file| {
        file.extension()
            .is_some_and(|extension| extension == "toml")
    });
    files.sort();

    let mut table = String::from("&[\n");
    for file in files {
        let name = file
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or("a theme file has a name that isn't text")?;
        let path = file.canonicalize()?;
        let path = path
            .to_str()
            .ok_or("a theme file has a path that isn't text")?;
        writeln!(table, "    ({name:?}, include_str!({path:?})),")?;
    }
    table.push_str("]\n");

    fs::write(Path::new(&env::var("OUT_DIR")?).join("themes.rs"), table)?;
    Ok(())
}
