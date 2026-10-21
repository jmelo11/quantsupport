use quantsupport_xll_macros::xll_bindgen;
use xll_rs::types::XllError;

#[xll_bindgen(
    name = "QS.VERSION",
    threadsafe,
    category = "QuantSupport - System",
    help = "Returns the linked QuantSupport library version"
)]
pub fn qs_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[xll_bindgen(
    name = "QS.FUNCTIONS",
    category = "QuantSupport - System",
    help = "Spills the names of all worksheet functions registered by this XLL"
)]
pub fn qs_functions(
    transpose: *const xll_rs::types::XLOPER12,
) -> Result<*mut xll_rs::types::XLOPER12, XllError> {
    let transpose = crate::functions::helpers::optional_bool(transpose, "transpose")?;
    let mut names: Vec<_> = xll_rs::inventory::iter::<xll_rs::registry::XllExport>
        .into_iter()
        .flat_map(|export| {
            std::iter::once(export.name.to_string())
                .chain(export.aliases.iter().map(|alias| (*alias).to_string()))
        })
        .collect();
    names.sort();
    names.dedup();
    Ok(crate::functions::helpers::string_column(names, transpose))
}

#[xll_bindgen(
    name = "QS.FUNCTIONS.DETAILS",
    category = "QuantSupport - System",
    help = "Spills the function catalog with arguments, category, aliases, and description"
)]
pub fn qs_functions_details(
    transpose: *const xll_rs::types::XLOPER12,
    headers: *const xll_rs::types::XLOPER12,
) -> Result<*mut xll_rs::types::XLOPER12, XllError> {
    let transpose = crate::functions::helpers::optional_bool(transpose, "transpose")?;
    let headers = crate::functions::helpers::optional_bool(headers, "headers")?;
    let mut exports: Vec<_> = xll_rs::inventory::iter::<xll_rs::registry::XllExport>
        .into_iter()
        .collect();
    exports.sort_by(|left, right| left.name.cmp(right.name));

    let mut rows = vec![vec![
        "Function".to_string(),
        "Arguments".to_string(),
        "Category".to_string(),
        "Aliases".to_string(),
        "Description".to_string(),
    ]];
    rows.extend(exports.into_iter().map(|export| {
        vec![
            export.name.to_string(),
            export.arg_names.to_string(),
            export.category.to_string(),
            export.aliases.join(", "),
            export.help.to_string(),
        ]
    }));
    Ok(crate::functions::helpers::string_matrix(
        rows, transpose, headers,
    ))
}

#[xll_bindgen(
    name = "QS.HELP",
    category = "QuantSupport - System",
    help = "Spills a worksheet function's description and parameter guidance"
)]
pub fn qs_help(
    function_name: &str,
    transpose: *const xll_rs::types::XLOPER12,
    headers: *const xll_rs::types::XLOPER12,
) -> Result<*mut xll_rs::types::XLOPER12, XllError> {
    let transpose = crate::functions::helpers::optional_bool(transpose, "transpose")?;
    let headers = crate::functions::helpers::optional_bool(headers, "headers")?;
    let requested_name = function_name.trim();
    let export = xll_rs::inventory::iter::<xll_rs::registry::XllExport>
        .into_iter()
        .find(|export| {
            export.name.eq_ignore_ascii_case(requested_name)
                || export
                    .aliases
                    .iter()
                    .any(|alias| alias.eq_ignore_ascii_case(requested_name))
        })
        .ok_or_else(|| {
            crate::functions::helpers::value_error(format!(
                "unknown worksheet function '{requested_name}'; use QS.FUNCTIONS() to list names"
            ))
        })?;

    let argument_names: Vec<_> = export
        .arg_names
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .collect();
    let generated_argument_help =
        crate::documentation::function_help::argument_help(export.name, export.arg_names);
    let argument_help = if export.arg_help.is_empty() {
        generated_argument_help.as_slice()
    } else {
        export.arg_help
    };
    let mut rows = vec![
        vec!["Function".to_string(), export.name.to_string()],
        vec!["Category".to_string(), export.category.to_string()],
        vec!["Description".to_string(), export.help.to_string()],
    ];
    if !export.aliases.is_empty() {
        rows.push(vec!["Aliases".to_string(), export.aliases.join(", ")]);
    }
    if headers.unwrap_or(true) {
        rows.push(vec!["Parameter".to_string(), "Guidance".to_string()]);
    }
    rows.extend(
        argument_names
            .into_iter()
            .zip(argument_help)
            .map(|(name, help)| vec![name.to_string(), help.to_string()]),
    );
    Ok(crate::functions::helpers::string_matrix(
        rows, transpose, None,
    ))
}

#[cfg(test)]
mod tests {
    #[test]
    fn curve_catalog_uses_the_unified_handle_api() {
        let names: Vec<_> = xll_rs::inventory::iter::<xll_rs::registry::XllExport>
            .into_iter()
            .flat_map(|export| std::iter::once(export.name).chain(export.aliases.iter().copied()))
            .collect();

        for expected in [
            "QS.CURVE.GET",
            "QS.CURVE.TABLE",
            "QS.CURVE.DISCOUNT",
            "QS.CURVE.FORWARD",
            "QS.CURVE.ADD",
        ] {
            assert!(names.contains(&expected), "missing {expected}");
        }
        for removed in [
            "QS.CONTEXT.CURVE.DISCOUNT",
            "QS.CONTEXT.CURVE.FORWARD",
            "QS.CURVE.ADD.FLAT",
            "QS.ELEMENTS.ADD.DISCOUNT.CURVE",
        ] {
            assert!(!names.contains(&removed), "obsolete export {removed}");
        }
    }
}

mod objects;
