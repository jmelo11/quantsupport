//! Export worksheet documentation on any host, including Linux and macOS.

#[path = "../src/documentation/catalog.rs"]
mod catalog;

#[path = "../src/documentation/intellisense.rs"]
mod intellisense;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/functions");
    let records = catalog::exports(&root)?;
    let mut arguments = std::env::args_os().skip(1);
    match arguments.next().as_deref() {
        None => println!("{}", serde_json::to_string_pretty(&records)?),
        Some(flag) if flag == "--intellisense" => {
            let destination = arguments.next();
            if arguments.next().is_some() {
                return Err("usage: function_catalog [--intellisense [OUTPUT]]".into());
            }
            let xml = intellisense::xml(&records);
            if let Some(path) = destination {
                std::fs::write(path, xml)?;
            } else {
                print!("{xml}");
            }
        }
        _ => return Err("usage: function_catalog [--intellisense [OUTPUT]]".into()),
    }
    Ok(())
}
