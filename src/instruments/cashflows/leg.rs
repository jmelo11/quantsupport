use crate::{
    ad::{dual::DualFwd, scalar::Scalar},
    core::{collateral::Discountable, instrument::AssetClass, trade::Side},
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    instruments::cashflows::cashflowtype::CashflowType,
    rates::interestrate::InterestRate,
    time::date::Date,
};

/// A [`Leg`] represents a sequence of cashflows associated to a particular instrument.
#[derive(Clone)]
pub struct Leg<T: Scalar> {
    /// identifier for the leg, used for referencing in pricers and other components
    id: usize,
    /// list of cashflows associated with the leg
    cashflows: Vec<CashflowType<T>>,
    /// currency of the cashflows
    currency: Currency,
    /// discount index, if required
    discount_index: Option<MarketIndex>,
    /// forward rate index, if required
    forward_index: Option<MarketIndex>,
    /// spread of the floating leg, if any
    spread: Option<T>,
    /// rate associated with fixed-rate cashflows, if any
    interest_rate: Option<InterestRate<T>>,
    /// side of the leg (long or short)
    side: Side,
    /// whether the leg has a linear payoff structure (e.g., fixed payments) or non-linear (e.g., options)
    is_linear: bool,
    /// type of the underlying asset (e.g., interest rate, equity, commodity)
    asset_class: AssetClass,
    /// optional first and last payment dates for the leg, used for optimization and curve bootstrapping
    first_payment_date: Date,
    /// optional last payment date for the leg, used for optimization and curve bootstrapping
    last_payment_date: Date,
}

impl<T> Leg<T>
where
    T: Scalar,
{
    /// Creates a new [`Leg`] with the specified parameters.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        id: usize,
        cashflows: Vec<CashflowType<T>>,
        currency: Currency,
        discount_index: Option<MarketIndex>,
        forward_index: Option<MarketIndex>,
        spread: Option<T>,
        interest_rate: Option<InterestRate<T>>,
        side: Side,
        is_linear: bool,
        asset_class: AssetClass,
        first_payment_date: Date,
        last_payment_date: Date,
    ) -> Self {
        Self {
            id,
            cashflows,
            currency,
            discount_index,
            forward_index,
            spread,
            interest_rate,
            side,
            is_linear,
            asset_class,
            first_payment_date,
            last_payment_date,
        }
    }

    /// Returns the identifier of the leg.
    #[must_use]
    pub const fn id(&self) -> usize {
        self.id
    }

    /// Returns the cashflows associated with the leg.
    #[must_use]
    pub fn cashflows(&self) -> &[CashflowType<T>] {
        &self.cashflows
    }

    /// Returns the market index associated with the leg, if any.
    #[must_use]
    pub const fn forward_index(&self) -> Option<&MarketIndex> {
        self.forward_index.as_ref()
    }

    /// Returns the spread associated with the leg, if any.
    #[must_use]
    pub const fn spread(&self) -> Option<T> {
        self.spread
    }

    /// Returns the interest rate associated with the leg, if any.
    #[must_use]
    pub const fn interest_rate(&self) -> Option<InterestRate<T>> {
        self.interest_rate
    }

    /// Returns the side of the leg (long or short).
    #[must_use]
    pub const fn side(&self) -> Side {
        self.side
    }

    /// Returns whether the leg is linear (i.e., has a linear payoff structure) or non-linear.
    #[must_use]
    pub const fn is_linear(&self) -> bool {
        self.is_linear
    }

    /// Returns the asset class of the leg (e.g., interest rate, equity, commodity).
    #[must_use]
    pub const fn asset_class(&self) -> AssetClass {
        self.asset_class
    }

    /// Returns the first payment date of the leg.
    #[must_use]
    pub const fn first_payment_date(&self) -> Date {
        self.first_payment_date
    }

    /// Returns the last payment date of the leg.
    #[must_use]
    pub const fn last_payment_date(&self) -> Date {
        self.last_payment_date
    }
}

impl<T> Discountable for Leg<T>
where
    T: Scalar,
{
    fn currency(&self) -> Currency {
        self.currency
    }

    fn asset_class(&self) -> AssetClass {
        self.asset_class
    }

    fn discount_index(&self) -> Option<MarketIndex> {
        self.discount_index.clone()
    }
}

impl From<Leg<f64>> for Leg<DualFwd> {
    fn from(value: Leg<f64>) -> Self {
        Self::new(
            value.id,
            value.cashflows.into_iter().map(Into::into).collect(),
            value.currency,
            value.discount_index,
            value.forward_index,
            value.spread.map(DualFwd::new),
            value.interest_rate.map(Into::into),
            value.side,
            value.is_linear,
            value.asset_class,
            value.first_payment_date,
            value.last_payment_date,
        )
    }
}

impl From<Leg<DualFwd>> for Leg<f64> {
    fn from(value: Leg<DualFwd>) -> Self {
        Self::new(
            value.id,
            value.cashflows.into_iter().map(Into::into).collect(),
            value.currency,
            value.discount_index,
            value.forward_index,
            value.spread.map(|spread| spread.value()),
            value.interest_rate.map(Into::into),
            value.side,
            value.is_linear,
            value.asset_class,
            value.first_payment_date,
            value.last_payment_date,
        )
    }
}
