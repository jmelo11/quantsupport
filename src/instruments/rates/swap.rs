use crate::{
    ad::{dual::DualFwd, scalar::Scalar},
    core::{
        instrument::Instrument,
        request::LegsProvider,
        trade::{Side, Trade},
    },
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    instruments::cashflows::leg::Leg,
    time::date::Date,
    utils::errors::Result,
    xva::makecontigentclaim::IntoContingentClaims,
};

/// A [`Swap`] represents a vanilla fixed-float interest rate swap with two legs:
/// a fixed-rate leg and a floating-rate leg.
#[derive(Clone)]
pub struct Swap<T: Scalar> {
    identifier: String,
    legs: Vec<Leg<T>>,
    forward_index: MarketIndex,
    currency: Currency,
}

impl<T> Swap<T>
where
    T: Scalar,
{
    /// Creates a new [`Swap`].
    ///
    /// `legs[0]` is the fixed leg; `legs[1]` is the floating leg.
    #[must_use]
    pub fn new(
        identifier: String,
        fixed_leg: Leg<T>,
        floating_leg: Leg<T>,
        forward_index: MarketIndex,
        currency: Currency,
    ) -> Self {
        Self {
            identifier,
            legs: vec![fixed_leg, floating_leg],
            forward_index,
            currency,
        }
    }

    /// Returns a reference to the fixed leg (leg 0).
    #[must_use]
    pub fn fixed_leg(&self) -> &Leg<T> {
        &self.legs[0]
    }

    /// Returns a reference to the floating leg (leg 1).
    #[must_use]
    pub fn floating_leg(&self) -> &Leg<T> {
        &self.legs[1]
    }

    /// Returns the associated market index.
    #[must_use]
    pub fn forward_index(&self) -> MarketIndex {
        self.forward_index.clone()
    }

    /// Returns the currency of the swap.
    #[must_use]
    pub const fn currency(&self) -> Currency {
        self.currency
    }
}

impl<T> Instrument for Swap<T>
where
    T: Scalar,
{
    fn identifier(&self) -> String {
        self.identifier.clone()
    }
}

impl<T> LegsProvider<T> for Swap<T>
where
    T: Scalar,
{
    fn legs(&self) -> &[Leg<T>] {
        &self.legs
    }
}

/// Represents a trade of an interest rate swap.
pub struct SwapTrade<T: Scalar> {
    instrument: Swap<T>,
    trade_date: Date,
    notional: f64,
    side: Side,
}

impl<T> LegsProvider<T> for SwapTrade<T>
where
    T: Scalar,
{
    fn legs(&self) -> &[Leg<T>] {
        self.instrument.legs()
    }
}

impl<T> SwapTrade<T>
where
    T: Scalar,
{
    /// Creates a new [`SwapTrade`].
    #[must_use]
    pub const fn new(instrument: Swap<T>, trade_date: Date, notional: f64, side: Side) -> Self {
        Self {
            instrument,
            trade_date,
            notional,
            side,
        }
    }

    /// Returns the notional amount of the trade.
    #[must_use]
    pub const fn notional(&self) -> f64 {
        self.notional
    }
}

impl<T> Trade<Swap<T>> for SwapTrade<T>
where
    T: Scalar,
{
    fn instrument(&self) -> &Swap<T> {
        &self.instrument
    }

    fn trade_date(&self) -> Date {
        self.trade_date
    }

    fn side(&self) -> Side {
        self.side
    }
}

#[allow(clippy::expect_used)]
impl From<Swap<f64>> for Swap<DualFwd> {
    fn from(value: Swap<f64>) -> Self {
        let mut legs = value.legs.into_iter();
        Self::new(
            value.identifier,
            legs.next().expect("fixed leg must exist").into(),
            legs.next().expect("floating leg must exist").into(),
            value.forward_index,
            value.currency,
        )
    }
}

#[allow(clippy::expect_used)]
impl From<Swap<DualFwd>> for Swap<f64> {
    fn from(value: Swap<DualFwd>) -> Self {
        let mut legs = value.legs.into_iter();
        Self::new(
            value.identifier,
            legs.next().expect("fixed leg must exist").into(),
            legs.next().expect("floating leg must exist").into(),
            value.forward_index,
            value.currency,
        )
    }
}

impl From<SwapTrade<f64>> for SwapTrade<DualFwd> {
    fn from(value: SwapTrade<f64>) -> Self {
        Self::new(
            value.instrument.into(),
            value.trade_date,
            value.notional,
            value.side,
        )
    }
}

impl From<SwapTrade<DualFwd>> for SwapTrade<f64> {
    fn from(value: SwapTrade<DualFwd>) -> Self {
        Self::new(
            value.instrument.into(),
            value.trade_date,
            value.notional,
            value.side,
        )
    }
}

impl SwapTrade<f64> {
    /// Decomposes the swap trade into contingent claims using the
    /// instrument's own identifier as the trade id.
    ///
    /// # Errors
    /// Returns an error if claim construction fails.
    pub fn into_contingent_claims(
        &self,
    ) -> Result<Vec<crate::xva::contigentclaim::ContingentClaim>> {
        let trade_id = self.instrument().identifier();
        self.instrument()
            .legs()
            .to_vec()
            .into_contingent_claims(&trade_id)
    }
}
