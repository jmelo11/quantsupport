use xllgen::xll_bindgen;

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
    name = "QS.ADD",
    threadsafe,
    category = "QuantSupport - System",
    help = "Small scalar example showing how to register a worksheet function"
)]
pub fn qs_add(left: f64, right: f64) -> f64 {
    left + right
}

#[xll_bindgen(
    name = "QS.FUNCTIONS",
    category = "QuantSupport - System",
    help = "Spills the names of all worksheet functions registered by this XLL"
)]
pub fn qs_functions() -> *mut xll_rs::types::XLOPER12 {
    let mut names: Vec<_> = xll_rs::inventory::iter::<xll_rs::registry::XllExport>
        .into_iter()
        .flat_map(|export| {
            std::iter::once(export.name.to_string())
                .chain(export.aliases.iter().map(|alias| (*alias).to_string()))
        })
        .collect();
    names.sort();
    names.dedup();
    super::helpers::string_column(names)
}
