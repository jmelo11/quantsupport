#[path = "src/documentation/catalog.rs"]
mod catalog;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    println!("cargo:rerun-if-changed=src/functions");
    println!("cargo:rerun-if-changed=src/documentation");
    let records = catalog::exports(&root.join("src/functions"))?;
    let mut names = Vec::new();
    for record in records {
        let arguments = record["arguments"]
            .as_array()
            .ok_or("missing worksheet arguments")?;
        let argument_text = arguments
            .iter()
            .map(|argument| argument["name"].as_str().unwrap())
            .collect::<Vec<_>>()
            .join(",");
        if argument_text.encode_utf16().count() > 255 {
            return Err(format!(
                "{}: argument names exceed Excel's 255-character registration limit",
                record["name"]
            )
            .into());
        }
        names.push(record["name"].as_str().unwrap().to_string());
        names.extend(
            record["aliases"]
                .as_array()
                .unwrap()
                .iter()
                .map(|alias| alias.as_str().unwrap().to_string()),
        );
    }
    let output = std::path::PathBuf::from(std::env::var("OUT_DIR")?).join("worksheet_exports.rs");
    std::fs::write(
        output,
        format!("const EXPECTED_WORKSHEET_EXPORTS: &[&str] = &{names:?};"),
    )?;
    Ok(())
}
