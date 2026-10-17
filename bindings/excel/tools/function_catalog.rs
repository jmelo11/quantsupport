//! Export worksheet documentation on any host, including Linux and macOS.

#[path = "../src/documentation/catalog.rs"]
mod catalog;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/functions");
    println!(
        "{}",
        serde_json::to_string_pretty(&catalog::exports(&root)?)?
    );
    Ok(())
}
