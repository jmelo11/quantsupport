use std::{env, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    // Maturin moves Python sources to the archive root for workspace sdists.
    let source_dir = [
        manifest_dir.join("python/quantsupport"),
        manifest_dir.join("../../python/quantsupport"),
    ]
    .into_iter()
    .find(|path| path.join("_error_support.py").is_file())
    .ok_or("could not find quantsupport Python package sources")?;
    let output_dir = PathBuf::from(env::var("OUT_DIR")?);
    for filename in ["_error_support.py", "_maker_support.py"] {
        let source = source_dir.join(filename);
        println!("cargo:rerun-if-changed={}", source.display());
        fs::copy(source, output_dir.join(filename))?;
    }
    Ok(())
}
