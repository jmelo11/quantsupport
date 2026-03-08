use crate::{
    ad::adreal::ADReal, core::trade::Side, currencies::currency::Currency,
    indices::marketindex::MarketIndex, instruments::cashflows::cashflowtype::CashflowType,
    rates::interestrate::InterestRate, time::date::Date,
};

/// A [`Leg`] represents a sequence of cashflows associated to a particular instrument.
pub struct Leg {
    leg_id: usize,
    cashflows: Vec<CashflowType>,
    /// currency of the cashflows
    currency: Currency,
    /// forward rate index, if required
    market_index: Option<MarketIndex>,
    /// spread of the floating leg, if any
    spread: Option<ADReal>,
    /// rate associated with fixed-rate cashflows, if any
    interest_rate: Option<InterestRate<ADReal>>,
    /// side of the leg (long or short)
    side: Side,
    /// whether the leg has a linear payoff structure (e.g., fixed payments) or non-linear (e.g., options)
    is_linear: bool,
    /// optional first and last payment dates for the leg, used for optimization and curve bootstrapping
    first_payment_date: Date,
    /// optional last payment date for the leg, used for optimization and curve bootstrapping
    last_payment_date: Date,
}

impl Leg {
    /// Creates a new [`Leg`] with the specified parameters.
    pub fn new(
        leg_id: usize,
        cashflows: Vec<CashflowType>,
        currency: Currency,
        market_index: Option<MarketIndex>,
        spread: Option<ADReal>,
        interest_rate: Option<InterestRate<ADReal>>,
        side: Side,
        is_linear: bool,
        first_payment_date: Date,
        last_payment_date: Date,
    ) -> Self {
        Self {
            leg_id,
            cashflows,
            currency,
            market_index,
            spread,
            interest_rate,
            side,
            is_linear,
            first_payment_date,
            last_payment_date,
        }
    }

    /// Returns the identifier of the leg.
    #[must_use]
    pub const fn leg_id(&self) -> usize {
        self.leg_id
    }

    /// Returns the cashflows associated with the leg.
    #[must_use]
    pub fn cashflows(&self) -> &[CashflowType] {
        &self.cashflows
    }

    /// Returns the currency of the leg.
    #[must_use]
    pub fn currency(&self) -> Currency {
        self.currency
    }

    /// Returns the market index associated with the leg, if any.
    #[must_use]
    pub fn market_index(&self) -> Option<&MarketIndex> {
        self.market_index.as_ref()
    }

    /// Returns the spread associated with the leg, if any.
    #[must_use]
    pub fn spread(&self) -> Option<ADReal> {
        self.spread
    }

    /// Returns the interest rate associated with the leg, if any.
    #[must_use]
    pub fn interest_rate(&self) -> Option<InterestRate<ADReal>> {
        self.interest_rate
    }

    /// Returns the side of the leg (long or short).
    #[must_use]
    pub fn side(&self) -> Side {
        self.side
    }

    /// Returns whether the leg is linear (i.e., has a linear payoff structure) or non-linear.
    #[must_use]
    pub fn is_linear(&self) -> bool {
        self.is_linear
    }

    /// Returns the first payment date of the leg.
    #[must_use]
    pub fn first_payment_date(&self) -> Date {
        self.first_payment_date
    }

    /// Returns the last payment date of the leg.
    #[must_use]
    pub fn last_payment_date(&self) -> Date {
        self.last_payment_date
    }
}
