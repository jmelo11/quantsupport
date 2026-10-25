use std::{env, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    // Maturin moves Python sources to the archive root for workspace sdists.
    let source = [
        manifest_dir.join("python/quantsupport/_error_support.py"),
        manifest_dir.join("../../python/quantsupport/_error_support.py"),
    ]
    .into_iter()
    .find(|path| path.is_file())
    .ok_or("could not find quantsupport/_error_support.py")?;
    println!("cargo:rerun-if-changed={}", source.display());
    fs::copy(
        source,
        PathBuf::from(env::var("OUT_DIR")?).join("_error_support.py"),
    )?;
    Ok(())
}
