//! Excel-DNA IntelliSense descriptions for native XLL worksheet functions.
//!
//! Protocol: https://github.com/Excel-DNA/IntelliSense/wiki/Usage-Instructions

use serde_json::Value;
use std::fmt::Write;

fn attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
        .replace('\n', "&#10;")
        .replace('\r', "&#13;")
        .replace('\t', "&#9;")
}

/// Generates the companion XML from the same catalog as the Function Wizard.
pub fn xml(records: &[Value]) -> String {
    let mut output = String::from(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\
         <IntelliSense xmlns=\"http://schemas.excel-dna.net/intellisense/1.0\">\n\
           <FunctionInfo>\n",
    );
    for record in records {
        let name = record["name"].as_str().unwrap();
        let page = name.to_ascii_lowercase().replace('.', "-");
        let help_topic =
            format!("https://jmelo11.github.io/quantsupport/api/excel/functions/{page}.html");
        let names = std::iter::once(name).chain(
            record["aliases"]
                .as_array()
                .unwrap()
                .iter()
                .map(|name| name.as_str().unwrap()),
        );
        for name in names {
            writeln!(
                output,
                "    <Function Name=\"{}\" Description=\"{}\" HelpTopic=\"{}\">",
                attribute(name),
                attribute(record["help"].as_str().unwrap()),
                attribute(&help_topic),
            )
            .unwrap();
            for argument in record["arguments"].as_array().unwrap() {
                writeln!(
                    output,
                    "      <Argument Name=\"{}\" Description=\"{}\" />",
                    attribute(argument["name"].as_str().unwrap()),
                    attribute(argument["help"].as_str().unwrap()),
                )
                .unwrap();
            }
            output.push_str("    </Function>\n");
        }
    }
    output.push_str("  </FunctionInfo>\n</IntelliSense>\n");
    output
}
