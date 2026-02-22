use crate::{
    cashflows::cashflow::{Cashflow, Side},
    currencies::enums::Currency,
    rates::interestrate::RateDefinition,
};

use super::{instrument::RateType, traits::Structure};

/// # Leg
/// A financial leg. Contains a stream of cashflows. Instruments have one or more legs.
#[derive(Debug, Clone)]
pub struct Leg {
    structure: Structure,
    rate_type: RateType,
    rate_value: f64,
    rate_definition: RateDefinition,
    currency: Currency,
    side: Side,
    discount_curve_id: Option<usize>,
    forecast_curve_id: Option<usize>,
    cashflows: Vec<Cashflow>,
}

impl Leg {
    /// Creates a new `Leg` with the specified parameters.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    // allowed: high-arity API; refactor deferred
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        structure: Structure,
        rate_type: RateType,
        rate_value: f64,
        rate_definition: RateDefinition,
        currency: Currency,
        side: Side,
        discount_curve_id: Option<usize>,
        forecast_curve_id: Option<usize>,
        cashflows: Vec<Cashflow>,
    ) -> Self {
        Self {
            structure,
            rate_type,
            rate_value,
            rate_definition,
            currency,
            side,
            discount_curve_id,
            forecast_curve_id,
            cashflows,
        }
    }

    /// Returns a slice of the cashflows in this leg.
    #[must_use]
    pub fn cashflows(&self) -> &[Cashflow] {
        &self.cashflows
    }

    /// Returns the structure of this leg.
    #[must_use]
    pub const fn structure(&self) -> Structure {
        self.structure
    }

    /// Returns the rate type of this leg.
    #[must_use]
    pub const fn rate_type(&self) -> RateType {
        self.rate_type
    }

    /// Returns the rate value of this leg.
    #[must_use]
    pub const fn rate_value(&self) -> f64 {
        self.rate_value
    }

    /// Returns the rate definition of this leg.
    #[must_use]
    pub const fn rate_definition(&self) -> RateDefinition {
        self.rate_definition
    }

    /// Returns the currency of this leg.
    #[must_use]
    pub const fn currency(&self) -> Currency {
        self.currency
    }

    /// Returns the side of this leg.
    #[must_use]
    pub const fn side(&self) -> Side {
        self.side
    }

    /// Returns the discount curve ID of this leg, if set.
    #[must_use]
    pub const fn discount_curve_id(&self) -> Option<usize> {
        self.discount_curve_id
    }

    /// Returns the forecast curve ID of this leg, if set.
    #[must_use]
    pub const fn forecast_curve_id(&self) -> Option<usize> {
        self.forecast_curve_id
    }

    /// Clears all cashflows from this leg.
    pub fn clear(&mut self) {
        self.cashflows.clear();
    }
}
