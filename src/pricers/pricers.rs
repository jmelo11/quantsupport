/// Closed-form pricer trait.
pub trait CloseFormPricer {}
/// Monte Carlo pricer trait.
pub trait MonteCarloPricer {}

/// PDE pricer trait.
pub trait PDEPricer {}
/// Backward evolution pricer trait.
pub trait BackwardEvolutionPricer {}

/// Black-Scholes closed-form pricer.
pub struct BlackClosedFormPricer;
impl CloseFormPricer for BlackClosedFormPricer {}

/// Normal (Bachelier) closed-form pricer.
pub struct NormalClosedFormPricer;
impl CloseFormPricer for NormalClosedFormPricer {}

/// Discounted cashflow pricer.
pub struct DiscountedCashflowPricer;
/// Hull-White closed-form pricer.
pub struct HullWhiteClosedFormPricer;
impl CloseFormPricer for HullWhiteClosedFormPricer {}

pub struct DiscountedCashflowsPricer;
impl CloseFormPricer for DiscountedCashflowsPricer {}
