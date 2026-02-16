/// Closed-form pricer trait.
pub trait CloseFormPricer {}
/// Monte Carlo pricer trait.
pub trait MonteCarloPricer {}

/// PDE pricer trait.
pub trait PDEPricer {}
/// Backward evolution pricer trait.
pub trait BackwardEvolutionPricer {}

/// Black-Scholes closed-form pricer.
#[derive(Clone, Copy, Debug, Default)]
pub struct BlackClosedFormPricer;
impl CloseFormPricer for BlackClosedFormPricer {}

/// Black-Scholes Monte Carlo pricer.
#[derive(Clone, Copy, Debug, Default)]
pub struct BlackMonteCarloPricer;
impl MonteCarloPricer for BlackMonteCarloPricer {}

/// Normal (Bachelier) closed-form pricer.
pub struct NormalClosedFormPricer;
impl CloseFormPricer for NormalClosedFormPricer {}

/// Hull-White closed-form pricer.
pub struct HullWhiteClosedFormPricer;
impl CloseFormPricer for HullWhiteClosedFormPricer {}

pub struct DiscountedCashflowsPricer;
impl CloseFormPricer for DiscountedCashflowsPricer {}
