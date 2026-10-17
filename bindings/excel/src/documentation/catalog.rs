//! Reads worksheet metadata without linking the Windows-only Excel SDK.

use std::{collections::BTreeSet, error::Error, fs, path::Path};

use proc_macro2::{Group, TokenStream, TokenTree};
use quote::{quote, ToTokens};
use serde_json::{json, Value};
use syn::{punctuated::Punctuated, Expr, FnArg, Item, Lit, Meta, Pat, Token};

#[path = "function_help.rs"]
mod function_help;

fn string(expr: &Expr) -> Result<String, Box<dyn Error>> {
    if let Expr::Lit(value) = expr {
        if let Lit::Str(value) = &value.lit {
            return Ok(value.value());
        }
    }
    Err("worksheet metadata must contain literal strings".into())
}

fn substitute(tokens: TokenStream, bindings: &[(String, TokenStream)]) -> TokenStream {
    let mut result = TokenStream::new();
    let mut tokens = tokens.into_iter();
    while let Some(token) = tokens.next() {
        match token {
            TokenTree::Punct(ref value) if value.as_char() == '$' => {
                let parameter = tokens.next().expect("macro parameter after $").to_string();
                let replacement = &bindings.iter().find(|(name, _)| name == &parameter)
                    .expect("bound worksheet macro parameter").1;
                result.extend(replacement.clone());
            }
            TokenTree::Group(group) => result.extend([TokenTree::Group(Group::new(
                group.delimiter(), substitute(group.stream(), bindings),
            ))]),
            token => result.extend([token]),
        }
    }
    result
}

fn function(item: &syn::ItemFn, source: &str) -> Result<Option<Value>, Box<dyn Error>> {
    let Some(attr) = item.attrs.iter().find(|attr| attr.path().is_ident("xll_bindgen")) else {
        return Ok(None);
    };
    let mut record = json!({"aliases": [], "threadsafe": false, "volatile": false, "source": source});
    for meta in attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)? {
        match meta {
            Meta::NameValue(value) => {
                let key = value.path.get_ident().ok_or("invalid worksheet metadata key")?.to_string();
                record[&key] = if key == "aliases" || key == "arg_help" {
                    let Expr::Array(values) = value.value else { return Err("expected metadata array".into()); };
                    json!(values.elems.iter().map(string).collect::<Result<Vec<_>, _>>()?)
                } else { json!(string(&value.value)?) };
            }
            Meta::Path(path) => record[path.get_ident().ok_or("invalid worksheet flag")?.to_string()] = json!(true),
            _ => return Err("unsupported worksheet metadata".into()),
        }
    }
    for required in ["name", "category", "help"] {
        if record.get(required).and_then(Value::as_str).is_none() {
            return Err(format!("missing {required} in {}", item.sig.ident).into());
        }
    }
    let mut arguments = Vec::new();
    for arg in &item.sig.inputs {
        let FnArg::Typed(arg) = arg else { return Err("worksheet function has a receiver".into()); };
        let Pat::Ident(name) = &*arg.pat else { return Err("worksheet argument must have a name".into()); };
        arguments.push((name.ident.to_string(), arg.ty.to_token_stream().to_string()));
    }
    let names = arguments.iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>().join(",");
    let help = function_help::argument_help(record["name"].as_str().unwrap(), &names);
    let explicit_help = record.get("arg_help").and_then(Value::as_array);
    if explicit_help.is_some_and(|hints| hints.len() != arguments.len()) {
        return Err("worksheet argument help must match the signature".into());
    }
    record["arguments"] = json!(arguments.iter().enumerate().map(|(i, (name, ty))| json!({
        "name": name, "type": ty,
        "help": explicit_help.map_or(help[i], |hints| hints[i].as_str().unwrap()),
    })).collect::<Vec<_>>());
    Ok(Some(record))
}

/// Returns metadata for ordinary functions and functions produced by local macros.
pub(crate) fn exports(root: &Path) -> Result<Vec<Value>, Box<dyn Error>> {
    let mut records = Vec::new();
    let mut files = fs::read_dir(root)?.map(|entry| entry.map(|entry| entry.path())).collect::<Result<Vec<_>, _>>()?;
    files.sort();
    for file in files {
        if file.is_dir() { records.extend(exports(&file)?); continue; }
        if file.extension().and_then(|ext| ext.to_str()) != Some("rs") { continue; }
        let parsed = syn::parse_file(&fs::read_to_string(&file)?)?;
        let source = format!("bindings/excel/src/functions/{}", file.strip_prefix(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/functions"))?.display());
        let mut macros = Vec::new();
        for item in &parsed.items {
            if let Item::Macro(item) = item {
                if let Some(name) = &item.ident {
                    // Expand the same single-pattern macro used by worksheet registrations.
                    let groups: Vec<_> = item.mac.tokens.clone().into_iter().filter_map(|token| {
                        if let TokenTree::Group(group) = token { Some(group) } else { None }
                    }).collect();
                    if groups.len() == 2 && groups[1].stream().to_string().contains("xll_bindgen") {
                        let parameters: Vec<_> = groups[0].stream().into_iter().collect();
                        let names = parameters.windows(2).filter_map(|pair| match (&pair[0], &pair[1]) {
                            (TokenTree::Punct(p), TokenTree::Ident(name)) if p.as_char() == '$' => Some(name.to_string()),
                            _ => None,
                        }).collect::<Vec<_>>();
                        macros.push((name.to_string(), names, groups[1].stream()));
                    }
                }
            }
        }
        for item in &parsed.items {
            match item {
                Item::Fn(item) => if let Some(record) = function(item, &source)? { records.push(record); },
                Item::Macro(item) if item.ident.is_none() => {
                    let name = item.mac.path.to_token_stream().to_string();
                    if let Some((_, parameters, template)) = macros.iter().find(|(candidate, _, _)| candidate == &name) {
                        let tokens = &item.mac.tokens;
                        let args: syn::ExprTuple = syn::parse2(quote!((#tokens)))?;
                        if args.elems.len() != parameters.len() { return Err("worksheet macro argument mismatch".into()); }
                        let bindings = parameters.iter().cloned().zip(args.elems.iter().map(ToTokens::to_token_stream)).collect::<Vec<_>>();
                        for expanded in syn::parse2::<syn::File>(substitute(template.clone(), &bindings))?.items {
                            if let Item::Fn(item) = expanded {
                                if let Some(record) = function(&item, &source)? { records.push(record); }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    records.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    let mut names = BTreeSet::new();
    for record in &records {
        for name in std::iter::once(&record["name"]).chain(record["aliases"].as_array().unwrap()) {
            if !names.insert(name.as_str().unwrap().to_string()) { return Err(format!("duplicate worksheet name {name}").into()); }
        }
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    #[test]
    fn documents_macro_generated_functions_and_aliases() {
        let records = super::exports(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/functions")).unwrap();
        let scenario = records.iter().find(|record| record["name"] == "QS.SCENARIOS.FROM.JSON").unwrap();
        assert_eq!(scenario["arguments"][1]["name"], "json");
        assert!(records.iter().any(|record| record["aliases"].as_array().unwrap().iter().any(|alias| alias == "QS.STORE.RESET")));
        for record in records {
            assert!(!record["help"].as_str().unwrap().is_empty());
            assert!(record["arguments"].as_array().unwrap().iter().all(|arg| !arg["help"].as_str().unwrap().is_empty()));
        }
    }

    #[cfg(windows)]
    #[test]
    fn documented_catalog_matches_registered_inventory() {
        std::hint::black_box(quantsupport_xll::xlAutoOpen as extern "system" fn() -> i32);
        let records = super::exports(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/functions"),
        ).unwrap();
        let registered: Vec<_> = xll_rs::inventory::iter::<xll_rs::registry::XllExport>
            .into_iter().collect();
        assert_eq!(records.len(), registered.len());
        for record in records {
            let name = record["name"].as_str().unwrap();
            let export = registered.iter().find(|export| export.name == name)
                .unwrap_or_else(|| panic!("documented function is not registered: {name}"));
            assert_eq!(record["category"], export.category, "{name}");
            assert_eq!(record["help"], export.help, "{name}");
            assert_eq!(record["aliases"], serde_json::json!(export.aliases), "{name}");
            let arguments = record["arguments"].as_array().unwrap().iter()
                .map(|argument| argument["name"].as_str().unwrap())
                .collect::<Vec<_>>();
            let registered_arguments = export.arg_names.split(',').map(str::trim)
                .filter(|name| !name.is_empty()).collect::<Vec<_>>();
            assert_eq!(arguments, registered_arguments, "{name}");
        }
    }
}
