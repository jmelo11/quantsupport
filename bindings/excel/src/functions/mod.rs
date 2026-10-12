//! Worksheet-function modules.
//!
//! Each `#[xll_bindgen]` function submits its own registration metadata to an
//! inventory. `xlAutoOpen` discovers that inventory, so adding a function only
//! requires adding it to the appropriate module.

mod analytics;
mod curves;
mod helpers;
mod market;
mod objects;
mod pricing;
mod system;
mod time;
mod trades;
