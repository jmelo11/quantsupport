use crate::{
    ad::adreal::ADReal,
    core::{
        instrument::{AssetClass, Instrument},
        trade::{Side, Trade},
    },
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    instruments::cashflows::{cashflow::SimpleCashflow, fixedratecoupon::FixedRateCoupon},
    rates::interestrate::InterestRate,
    time::date::Date,
};

/// A [`FixedRateDeposit`] represents a fixed-rate cash deposit with a single payment at the end (capital plus interest).
#[derive(Clone)]
pub struct FixedRateDeposit {
    identifier: String,
    units: f64,
    rate: InterestRate<ADReal>,
    coupon: FixedRateCoupon<ADReal>,
    redemption: SimpleCashflow<f64>,
    market_index: MarketIndex,
    currency: Currency,
}

impl FixedRateDeposit {
    /// Creates a new [`FixedRateDeposit`].

    #[must_use]
    pub const fn new(
        identifier: String,
        units: f64,
        rate: InterestRate<ADReal>,
        coupon: FixedRateCoupon<ADReal>,
        redemption: SimpleCashflow<f64>,
        market_index: MarketIndex,
        currency: Currency,
    ) -> Self {
        Self {
            identifier,
            units,
            rate,
            coupon,
            redemption,
            market_index,
            currency,
        }
    }

    /// Returns the units of the deposit.
    #[must_use]
    pub const fn units(&self) -> f64 {
        self.units
    }

    /// Returns the interest rate of the deposit.
    #[must_use]
    pub const fn rate(&self) -> InterestRate<ADReal> {
        self.rate
    }

    /// Returns a reference to the inner coupon.
    #[must_use]
    pub const fn coupon(&self) -> &FixedRateCoupon<ADReal> {
        &self.coupon
    }

    /// Returns a reference to the inner coupon.
    #[must_use]
    pub const fn redemption(&self) -> &SimpleCashflow<f64> {
        &self.redemption
    }

    /// Returns the associated market index.
    #[must_use]
    pub fn market_index(&self) -> MarketIndex {
        self.market_index.clone()
    }

    /// Returns the currency of payment.
    #[must_use]
    pub const fn currency(&self) -> Currency {
        self.currency
    }
}

impl Instrument for FixedRateDeposit {
    fn identifier(&self) -> String {
        self.identifier.clone()
    }

    fn asset_class(&self) -> AssetClass {
        AssetClass::FixedIncome
    }
}

/// Represents a trade of a deposit instrument.
pub struct FixedRateDepositTrade {
    instrument: FixedRateDeposit,
    trade_date: Date,
    notional: f64,
    side: Side,
}

impl FixedRateDepositTrade {
    /// Creates a new `DepositTrade`.
    #[must_use]
    pub const fn new(
        instrument: FixedRateDeposit,
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

impl Trade<FixedRateDeposit> for FixedRateDepositTrade {
    fn instrument(&self) -> &FixedRateDeposit {
        &self.instrument
    }

    fn trade_date(&self) -> Date {
        self.trade_date
    }

    fn side(&self) -> Side {
        self.side
    }
}
