pub trait CloseFormPricer {}
pub trait MonteCarloPricer {}

pub trait PDEPricer {}
pub trait BackwardEvolutionPricer {}

pub struct BlackClosedFormPricer;
impl CloseFormPricer for BlackClosedFormPricer {}

pub struct NormalClosedFormPricer;
impl CloseFormPricer for NormalClosedFormPricer {}

pub struct DiscountedCashflowPricer;
pub struct HullWhiteClosedFormPricer;
impl CloseFormPricer for HullWhiteClosedFormPricer {}
