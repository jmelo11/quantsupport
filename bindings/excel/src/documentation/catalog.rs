//! Reads worksheet metadata without linking the Windows-only Excel SDK.

use std::{collections::BTreeSet, error::Error, fs, path::Path};

use proc_macro2::{Group, TokenStream, TokenTree};
use quote::{quote, ToTokens};
use serde_json::{json, Value};
use syn::{punctuated::Punctuated, Expr, FnArg, Item, Lit, Meta, Pat, Token};

#[path = "function_help.rs"]
mod function_help;

#[cfg(all(test, not(windows)))]
#[path = "../functions/time/mod.rs"]
mod calendar_computation;

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
                let replacement = &bindings
                    .iter()
                    .find(|(name, _)| name == &parameter)
                    .expect("bound worksheet macro parameter")
                    .1;
                result.extend(replacement.clone());
            }
            TokenTree::Group(group) => result.extend([TokenTree::Group(Group::new(
                group.delimiter(),
                substitute(group.stream(), bindings),
            ))]),
            token => result.extend([token]),
        }
    }
    result
}

fn function(item: &syn::ItemFn, source: &str) -> Result<Option<Value>, Box<dyn Error>> {
    let Some(attr) = item
        .attrs
        .iter()
        .find(|attr| attr.path().is_ident("xll_bindgen"))
    else {
        return Ok(None);
    };
    let mut record =
        json!({"aliases": [], "threadsafe": false, "volatile": false, "source": source});
    for meta in attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)? {
        match meta {
            Meta::NameValue(value) => {
                let key = value
                    .path
                    .get_ident()
                    .ok_or("invalid worksheet metadata key")?
                    .to_string();
                record[&key] = if key == "aliases" || key == "arg_help" {
                    let Expr::Array(values) = value.value else {
                        return Err("expected metadata array".into());
                    };
                    json!(values
                        .elems
                        .iter()
                        .map(string)
                        .collect::<Result<Vec<_>, _>>()?)
                } else {
                    json!(string(&value.value)?)
                };
            }
            Meta::Path(path) => {
                record[path
                    .get_ident()
                    .ok_or("invalid worksheet flag")?
                    .to_string()] = json!(true)
            }
            _ => return Err("unsupported worksheet metadata".into()),
        }
    }
    for required in ["name", "category", "help"] {
        if record.get(required).and_then(Value::as_str).is_none() {
            return Err(format!("missing {required} in {}", item.sig.ident).into());
        }
    }
    record["return_type"] = json!(item.sig.output.to_token_stream().to_string());
    record["declaration"] = json!(item.sig.to_token_stream().to_string());
    let name = record["name"].as_str().unwrap();
    let (returns, errors) = function_help::result_help(name)
        .ok_or_else(|| format!("missing worksheet result help for {name}"))?;
    record["returns"] = json!(returns);
    record["errors"] = json!(errors);
    let mut arguments = Vec::new();
    for arg in &item.sig.inputs {
        let FnArg::Typed(arg) = arg else {
            return Err("worksheet function has a receiver".into());
        };
        let Pat::Ident(name) = &*arg.pat else {
            return Err("worksheet argument must have a name".into());
        };
        arguments.push((name.ident.to_string(), arg.ty.to_token_stream().to_string()));
    }
    let names = arguments
        .iter()
        .map(|(name, _)| name.as_str())
        .collect::<Vec<_>>()
        .join(",");
    let help = function_help::argument_help(record["name"].as_str().unwrap(), &names);
    let explicit_help = record.get("arg_help").and_then(Value::as_array);
    if explicit_help.is_some_and(|hints| hints.len() != arguments.len()) {
        return Err("worksheet argument help must match the signature".into());
    }
    record["arguments"] = json!(arguments
        .iter()
        .enumerate()
        .map(|(i, (name, ty))| json!({
            "name": name, "type": ty,
            "help": explicit_help.map_or(help[i], |hints| hints[i].as_str().unwrap()),
        }))
        .collect::<Vec<_>>());
    if arguments.iter().any(|(name, _)| name == "transpose") {
        let mut returns = record["returns"].as_str().unwrap().to_string();
        if arguments.iter().any(|(name, _)| name == "headers") {
            returns.push_str(" headers=FALSE removes the header row before transposing; omitted headers defaults to TRUE.");
        }
        returns.push_str(" transpose=TRUE exchanges rows and columns; omitted transpose defaults to FALSE. Empty output returns a blank cell.");
        record["returns"] = json!(returns);
        record["errors"] = json!(format!("{} Invalid transpose/headers flags return #VALUE!; use TRUE/FALSE or leave optional flags blank.", record["errors"].as_str().unwrap()));
    }
    Ok(Some(record))
}

/// Returns metadata for ordinary functions and functions produced by local macros.
pub fn exports(root: &Path) -> Result<Vec<Value>, Box<dyn Error>> {
    let mut records = Vec::new();
    let mut files = fs::read_dir(root)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()?;
    files.sort();
    for file in files {
        if file.is_dir() {
            records.extend(exports(&file)?);
            continue;
        }
        if file.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let parsed = syn::parse_file(&fs::read_to_string(&file)?)?;
        let source = format!(
            "bindings/excel/src/functions/{}",
            file.strip_prefix(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/functions"))?
                .to_string_lossy()
                .replace('\\', "/")
        );
        let mut macros = Vec::new();
        for item in &parsed.items {
            if let Item::Macro(item) = item {
                if let Some(name) = &item.ident {
                    // Expand the same single-pattern macro used by worksheet registrations.
                    let groups: Vec<_> = item
                        .mac
                        .tokens
                        .clone()
                        .into_iter()
                        .filter_map(|token| {
                            if let TokenTree::Group(group) = token {
                                Some(group)
                            } else {
                                None
                            }
                        })
                        .collect();
                    if groups.len() == 2 && groups[1].stream().to_string().contains("xll_bindgen") {
                        let parameters: Vec<_> = groups[0].stream().into_iter().collect();
                        let names = parameters
                            .windows(2)
                            .filter_map(|pair| match (&pair[0], &pair[1]) {
                                (TokenTree::Punct(p), TokenTree::Ident(name))
                                    if p.as_char() == '$' =>
                                {
                                    Some(name.to_string())
                                }
                                _ => None,
                            })
                            .collect::<Vec<_>>();
                        macros.push((name.to_string(), names, groups[1].stream()));
                    }
                }
            }
        }
        for item in &parsed.items {
            match item {
                Item::Fn(item) => {
                    if let Some(record) = function(item, &source)? {
                        records.push(record);
                    }
                }
                Item::Macro(item) if item.ident.is_none() => {
                    let name = item.mac.path.to_token_stream().to_string();
                    if let Some((_, parameters, template)) =
                        macros.iter().find(|(candidate, _, _)| candidate == &name)
                    {
                        let tokens = &item.mac.tokens;
                        let args: syn::ExprTuple = syn::parse2(quote!((#tokens)))?;
                        if args.elems.len() != parameters.len() {
                            return Err("worksheet macro argument mismatch".into());
                        }
                        let bindings = parameters
                            .iter()
                            .cloned()
                            .zip(args.elems.iter().map(ToTokens::to_token_stream))
                            .collect::<Vec<_>>();
                        for expanded in
                            syn::parse2::<syn::File>(substitute(template.clone(), &bindings))?.items
                        {
                            if let Item::Fn(item) = expanded {
                                if let Some(record) = function(&item, &source)? {
                                    records.push(record);
                                }
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
            if !names.insert(name.as_str().unwrap().to_string()) {
                return Err(format!("duplicate worksheet name {name}").into());
            }
        }
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    #[test]
    fn context_argument_names_fit_excel_registration_after_compacting() {
        let records =
            super::exports(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/functions"))
                .unwrap();
        let record = records
            .iter()
            .find(|record| record["name"] == "QS.CONTEXT.CREATE")
            .unwrap();
        let names: Vec<_> = record["arguments"]
            .as_array()
            .unwrap()
            .iter()
            .map(|argument| argument["name"].as_str().unwrap())
            .collect();
        assert!(names.join(", ").len() > 255);
        assert_eq!(names.join(",").len(), 252);
    }

    #[test]
    fn source_paths_use_forward_slashes() {
        let records =
            super::exports(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/functions"))
                .unwrap();
        assert!(!records.is_empty());
        for record in records {
            let source = record["source"].as_str().unwrap();
            assert!(
                source.starts_with("bindings/excel/src/functions/"),
                "{source}"
            );
            assert!(source.ends_with(".rs"), "{source}");
            assert!(!source.contains('\\'), "{source}");
        }
    }

    #[test]
    fn categories_follow_the_worksheet_domain_directories() {
        let records =
            super::exports(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/functions"))
                .unwrap();
        for record in records {
            let source = record["source"].as_str().unwrap();
            let domain = source
                .strip_prefix("bindings/excel/src/functions/")
                .unwrap()
                .split('/')
                .next()
                .unwrap();
            let title = match domain {
                "time" => "Time",
                "market" => "Market",
                "math" => "Math",
                "instruments" => "Instruments",
                "pricing" => "Pricing",
                "risk" => "Risk",
                "system" => "System",
                other => panic!("worksheet function outside a domain: {other}"),
            };
            assert_eq!(
                record["category"],
                format!("QuantSupport - {title}"),
                "{}",
                record["name"]
            );
        }
    }

    #[test]
    fn documents_macro_generated_functions_and_aliases() {
        let records =
            super::exports(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/functions"))
                .unwrap();
        assert_eq!(records.len(), 148);
        for (name, arguments) in [
            (
                "QS.QUOTES.FROM.RANGE",
                vec!["name", "reference_date", "data"],
            ),
            ("QS.FIXINGS.FROM.RANGE", vec!["name", "data"]),
        ] {
            let record = records
                .iter()
                .find(|record| record["name"] == name)
                .unwrap();
            let actual: Vec<_> = record["arguments"]
                .as_array()
                .unwrap()
                .iter()
                .map(|argument| argument["name"].as_str().unwrap())
                .collect();
            assert_eq!(actual, arguments);
            assert_eq!(record["volatile"], false);
        }
        let scenario = records
            .iter()
            .find(|record| record["name"] == "QS.SCENARIOS.FROM.JSON")
            .unwrap();
        assert_eq!(scenario["arguments"][1]["name"], "json");
        assert!(records.iter().any(|record| record["aliases"]
            .as_array()
            .unwrap()
            .iter()
            .any(|alias| alias == "QS.STORE.RESET")));
        for record in records {
            assert!(!record["help"].as_str().unwrap().is_empty());
            assert!(!record["return_type"].as_str().unwrap().is_empty());
            assert!(!record["declaration"].as_str().unwrap().is_empty());
            assert!(!record["returns"].as_str().unwrap().is_empty());
            assert!(!record["errors"].as_str().unwrap().is_empty());
            for argument in record["arguments"].as_array().unwrap() {
                let help = argument["help"].as_str().unwrap();
                assert!(!help.is_empty(), "{}: {}", record["name"], argument["name"]);
                assert_ne!(
                    help, "See the function description and QS.HELP for this argument",
                    "{}: {}",
                    record["name"], argument["name"]
                );
                if argument["type"].as_str().unwrap().contains("Option") {
                    assert!(
                        help.contains("blank"),
                        "{}: {}",
                        record["name"],
                        argument["name"]
                    );
                }
            }
        }
    }

    #[test]
    fn documents_native_interpolation_operations() {
        let records =
            super::exports(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/functions"))
                .unwrap();
        for (name, arguments) in [
            ("QS.INTERPOLATE", "point,x_values,y_values,method,enable_extrapolation"),
            ("QS.INTERPOLATE.LINEAR", "point,x_values,y_values,enable_extrapolation"),
            ("QS.INTERPOLATE.LOG.LINEAR", "point,x_values,y_values,enable_extrapolation"),
            ("QS.INTERPOLATE.CUBIC.SPLINE", "point,x_values,y_values,enable_extrapolation"),
            ("QS.INTERPOLATE.BILINEAR", "point_x,point_y,x_coordinates,y_coordinates,values,extrapolation"),
            ("QS.INTERPOLATE.TRILINEAR", "point_x,point_y,point_z,x_coordinates,y_coordinates,z_coordinates,values,extrapolation"),
        ] {
            let record = records.iter().find(|record| record["name"] == name).unwrap();
            assert_eq!(record["category"], "QuantSupport - Math");
            assert_eq!(record["source"], "bindings/excel/src/functions/math/mod.rs");
            assert_eq!(record["threadsafe"], true);
            assert_eq!(record["arguments"].as_array().unwrap().iter()
                .map(|argument| argument["name"].as_str().unwrap()).collect::<Vec<_>>(),
                arguments.split(',').collect::<Vec<_>>());
            assert!(record["errors"].as_str().unwrap().contains("#VALUE!"));
        }
    }

    #[test]
    fn documents_six_native_calendar_operations() {
        let records =
            super::exports(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/functions"))
                .unwrap();
        for (name, arguments) in [
            ("QS.CALENDAR.IS.HOLIDAY", &["date", "calendar"][..]),
            ("QS.CALENDAR.END.OF.MONTH", &["date", "calendar"][..]),
            ("QS.CALENDAR.IS.END.OF.MONTH", &["date", "calendar"][..]),
            (
                "QS.CALENDAR.BUSINESS.DAYS.BETWEEN",
                &["from", "to", "calendar", "include_first", "include_last"][..],
            ),
            (
                "QS.CALENDAR.HOLIDAYS",
                &["from", "to", "calendar", "include_weekends", "transpose"][..],
            ),
            (
                "QS.CALENDAR.BUSINESS.DAYS",
                &["from", "to", "calendar", "transpose"][..],
            ),
        ] {
            let record = records
                .iter()
                .find(|record| record["name"] == name)
                .unwrap();
            assert_eq!(record["category"], "QuantSupport - Time");
            assert_eq!(record["source"], "bindings/excel/src/functions/time/mod.rs");
            assert_eq!(record["threadsafe"], true);
            assert_eq!(
                record["arguments"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|argument| argument["name"].as_str().unwrap())
                    .collect::<Vec<_>>(),
                arguments
            );
            assert!(record["errors"]
                .as_str()
                .unwrap()
                .contains("1902-01-01..2198-12-31"));
        }
    }

    #[test]
    fn result_metadata_preserves_rust_signatures_and_worksheet_shapes() {
        let records =
            super::exports(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/functions"))
                .unwrap();
        for record in &records {
            let declaration = record["declaration"].as_str().unwrap();
            let function: syn::ItemFn = syn::parse_str(&format!("{declaration} {{}}")).unwrap();
            assert_eq!(
                record["return_type"],
                serde_json::json!(
                    quote::ToTokens::to_token_stream(&function.sig.output).to_string()
                )
            );
            assert_eq!(
                function.sig.inputs.len(),
                record["arguments"].as_array().unwrap().len()
            );
        }
        let forward = records
            .iter()
            .find(|record| record["name"] == "QS.CURVE.FORWARD")
            .unwrap();
        assert_eq!(
            forward["return_type"],
            quote::quote!(-> Result<f64, XllError>).to_string()
        );
        assert!(forward["declaration"]
            .as_str()
            .unwrap()
            .starts_with("fn qs_curve_forward"));
        let path = records
            .iter()
            .find(|record| record["name"] == "QS.SIMULATION.PATH")
            .unwrap();
        assert_eq!(
            path["return_type"],
            quote::quote!(-> Result<*mut xll_rs::types::XLOPER12, XllError>).to_string()
        );
        for (name, headers) in [
            (
                "QS.CURVE.TABLE",
                &[
                    "Pillar",
                    "Date",
                    "Discount factor",
                    "Zero rate",
                    "Forward from prior node",
                ][..],
            ),
            (
                "QS.CASHFLOWS",
                &[
                    "Payment date",
                    "Type",
                    "Amount",
                    "Fixing",
                    "Accrual",
                    "Currency",
                    "Floor",
                    "Cap",
                    "Leg",
                ][..],
            ),
            ("QS.SENSITIVITIES", &["Risk factor", "Sensitivity"][..]),
            ("QS.BOND.SENSITIVITIES", &["Risk factor", "Sensitivity"][..]),
            (
                "QS.FUNCTIONS.DETAILS",
                &[
                    "Function",
                    "Arguments",
                    "Category",
                    "Aliases",
                    "Description",
                ][..],
            ),
            (
                "QS.SIMULATION.CALIBRATION",
                &[
                    "identifier",
                    "expiry",
                    "t",
                    "big_t",
                    "market_vol",
                    "market_price",
                    "model_price",
                    "calibrated_sigma",
                    "forward_rate",
                    "effective_strike",
                    "price_error",
                ][..],
            ),
        ] {
            let record = records
                .iter()
                .find(|record| record["name"] == name)
                .unwrap();
            let source = std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
                    record["source"]
                        .as_str()
                        .unwrap()
                        .strip_prefix("bindings/excel/")
                        .unwrap(),
                ),
            )
            .unwrap();
            for header in headers {
                assert!(
                    record["returns"].as_str().unwrap().contains(header),
                    "{name}: {header}"
                );
                assert!(
                    source.contains(&format!("\"{header}\"")),
                    "{name}: {header}"
                );
            }
        }
    }

    #[cfg(windows)]
    #[test]
    fn documented_catalog_matches_registered_inventory() {
        std::hint::black_box(quantsupport_xll::xlAutoOpen as extern "system" fn() -> i32);
        let records =
            super::exports(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/functions"))
                .unwrap();
        let registered: Vec<_> = xll_rs::inventory::iter::<xll_rs::registry::XllExport>
            .into_iter()
            .collect();
        assert_eq!(records.len(), registered.len());
        for record in records {
            let name = record["name"].as_str().unwrap();
            let export = registered
                .iter()
                .find(|export| export.name == name)
                .unwrap_or_else(|| panic!("documented function is not registered: {name}"));
            assert_eq!(record["category"], export.category, "{name}");
            assert_eq!(record["help"], export.help, "{name}");
            assert_eq!(
                record["aliases"],
                serde_json::json!(export.aliases),
                "{name}"
            );
            let arguments = record["arguments"]
                .as_array()
                .unwrap()
                .iter()
                .map(|argument| argument["name"].as_str().unwrap())
                .collect::<Vec<_>>();
            let registered_arguments = export
                .arg_names
                .split(',')
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .collect::<Vec<_>>();
            assert_eq!(arguments, registered_arguments, "{name}");
        }
    }

    #[test]
    fn stored_objects_and_pricing_are_nonvolatile_and_direct_policy_arguments_are_visible() {
        let records =
            super::exports(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/functions"))
                .unwrap();
        for record in &records {
            assert_eq!(record["volatile"], false, "{}", record["name"]);
        }
        let policy = records
            .iter()
            .find(|record| record["name"] == "QS.DISCOUNT.POLICY.FIXED.INCOME")
            .unwrap();
        let arguments: Vec<_> = policy["arguments"]
            .as_array()
            .unwrap()
            .iter()
            .map(|argument| argument["name"].as_str().unwrap())
            .collect();
        assert_eq!(
            arguments,
            [
                "name",
                "currency",
                "market_index",
                "prefer_instrument_index"
            ]
        );
    }
}
