/// Provides the yield curve composed from multiple term structures.
pub mod compositetermstructure;
/// Provides the discount-factor term structure.
pub mod discounttermstructure;
/// Provides the flat-forward term structure.
pub mod flatforwardtermstructure;
/// Provides the common interest-rate term-structure interface.
pub mod interestratestermstructure;
/// Provides the spreaded term structure.
pub mod spreadtermstructure;
/// Provides the tenor-based zero-rate term structure.
pub mod tenorbasedzeroratetermstructure;
/// Provides the zero-rate term structure.
pub mod zeroratetermstructure;

/// Provides a numerical view of a recorded curve for calculations that do not record sensitivities.
pub mod primalcurve;
