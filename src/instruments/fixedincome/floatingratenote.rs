use crate::{
    ad::adreal::{DualFwd, Scalar},
    core::{
        collateral::Discountable,
        instrument::{AssetClass, Instrument},
        request::LegsProvider,
        trade::{Side, Trade},
    },
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    instruments::cashflows::leg::Leg,
    time::date::Date,
};

/// A [`FloatingRateNote`] represents a bond that pays periodic floating-rate coupons
/// (typically referencing an interest rate index plus a spread) and repays its principal at maturity.
pub struct FloatingRateNote<T: Scalar> {
    identifier: String,
    units: f64,
    leg: Leg<T>,
    discount_index: Option<MarketIndex>,
    forward_index: Option<MarketIndex>,
    currency: Currency,
}

impl<T> FloatingRateNote<T>
where
    T: Scalar,
{
    /// Creates a new [`FloatingRateNote`].
    #[must_use]
    pub const fn new(
        identifier: String,
        units: f64,
        leg: Leg<T>,
        discount_index: Option<MarketIndex>,
        forward_index: Option<MarketIndex>,
        currency: Currency,
    ) -> Self {
        Self {
            identifier,
            units,
            leg,
            discount_index,
            forward_index,
            currency,
        }
    }

    /// Returns the units of the note.
    #[must_use]
    pub const fn units(&self) -> f64 {
        self.units
    }

    /// Returns a reference to the inner leg.
    #[must_use]
    pub const fn leg(&self) -> &Leg<T> {
        &self.leg
    }
}

impl<T> Discountable for FloatingRateNote<T>
where
    T: Scalar,
{
    fn currency(&self) -> Currency {
        self.currency
    }

    fn asset_class(&self) -> AssetClass {
        AssetClass::FixedIncome
    }

    fn discount_index(&self) -> Option<MarketIndex> {
        self.discount_index.clone()
    }
}

impl<T> Instrument for FloatingRateNote<T>
where
    T: Scalar,
{
    fn identifier(&self) -> String {
        self.identifier.clone()
    }
}

impl<T> LegsProvider<T> for FloatingRateNote<T>
where
    T: Scalar,
{
    fn legs(&self) -> &[Leg<T>] {
        std::slice::from_ref(&self.leg)
    }
}

/// Represents a trade of a floating rate note instrument.
pub struct FloatingRateNoteTrade<T: Scalar> {
    instrument: FloatingRateNote<T>,
    trade_date: Date,
    notional: f64,
    side: Side,
}

impl<T> FloatingRateNoteTrade<T>
where
    T: Scalar,
{
    /// Creates a new [`FloatingRateNoteTrade`].
    #[must_use]
    pub const fn new(
        instrument: FloatingRateNote<T>,
        trade_date: Date,
        notional: f64,
        side: Side,
    ) -> Self {
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

impl<T> Trade<FloatingRateNote<T>> for FloatingRateNoteTrade<T>
where
    T: Scalar,
{
    fn instrument(&self) -> &FloatingRateNote<T> {
        &self.instrument
    }

    fn trade_date(&self) -> Date {
        self.trade_date
    }

    fn side(&self) -> Side {
        self.side
    }
}

impl From<FloatingRateNote<f64>> for FloatingRateNote<DualFwd> {
    fn from(value: FloatingRateNote<f64>) -> Self {
        Self::new(
            value.identifier,
            value.units,
            value.leg.into(),
            value.discount_index,
            value.forward_index,
            value.currency,
        )
    }
}

impl From<FloatingRateNote<DualFwd>> for FloatingRateNote<f64> {
    fn from(value: FloatingRateNote<DualFwd>) -> Self {
        Self::new(
            value.identifier,
            value.units,
            value.leg.into(),
            value.discount_index,
            value.forward_index,
            value.currency,
        )
    }
}

impl From<FloatingRateNoteTrade<f64>> for FloatingRateNoteTrade<DualFwd> {
    fn from(value: FloatingRateNoteTrade<f64>) -> Self {
        Self::new(
            value.instrument.into(),
            value.trade_date,
            value.notional,
            value.side,
        )
    }
}

impl From<FloatingRateNoteTrade<DualFwd>> for FloatingRateNoteTrade<f64> {
    fn from(value: FloatingRateNoteTrade<DualFwd>) -> Self {
        Self::new(
            value.instrument.into(),
            value.trade_date,
            value.notional,
            value.side,
        )
    }
}
