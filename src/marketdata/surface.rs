use std::collections::BTreeMap;
/// # `Strike`
///
/// Strike represented as absolute value
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Strike(pub f64); // absolute or rate

/// # `Delta`
///
/// Delta represented as absolute value between 0 and 1
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Delta(pub f64); // with convention metadata

/// # `Moneyness`
///
/// Moneyness represented as log-moneyness
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct LogMoneyness(pub f64);

/// # `Surface`
///     
/// A trait representing a generic surface with two axes and a quantity type.
///
/// ## Generics
/// - `A1`: The type of the first axis (e.g., Date).
/// - `A2`: The type of the second axis (e.g., Strike, Moneyness, Delta).
/// - `Q`: The type of the quantity (e.g., f64).
pub trait Surface<A1, A2, Q> {
    /// Returns a reference to the points of the surface.
    fn points(&self) -> &BTreeMap<A1, BTreeMap<A2, Q>>;
}
