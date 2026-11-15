//! Credit default swap (CDS) instrument.
//!
//! A [`CreditDefaultSwap`](crate::instruments::credit::creditdefaultswap::CreditDefaultSwap) exchanges a running premium (spread) paid on a
//! regular schedule against a protection payment of `(1 − recovery)` on
//! default of the reference entity. Survival probabilities are sourced from
//! the credit curve keyed by [`MarketIndex::Credit`](crate::indices::marketindex::MarketIndex::Credit), while cash flows are
//! discounted with the curve keyed by `discount_index` (or a CSA discount
//! policy when one is supplied to the pricer).

use crate::{
    core::{
        collateral::Discountable,
        instrument::{AssetClass, Instrument},
        trade::{Side, Trade},
    },
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    time::{date::Date, daycounter::DayCounter, enums::Frequency},
    utils::errors::{QSError, Result},
};

/// A single-name credit default swap.
#[derive(Clone, Debug)]
pub struct CreditDefaultSwap {
    identifier: String,
    /// Credit curve index of the reference entity (`MarketIndex::Credit`).
    credit_index: MarketIndex,
    /// Discounting curve index for the premium and protection legs.
    discount_index: MarketIndex,
    currency: Currency,
    start_date: Date,
    maturity_date: Date,
    /// Running premium (decimal, e.g. `0.01` = 100 bp).
    spread: f64,
    /// Assumed recovery rate of the reference entity.
    recovery: f64,
    premium_frequency: Frequency,
    day_counter: DayCounter,
}

impl CreditDefaultSwap {
    /// Creates a new credit default swap.
    ///
    /// # Errors
    /// Returns an error if `credit_index` is not a [`MarketIndex::Credit`],
    /// if `maturity_date <= start_date`, or if `recovery` is outside `[0, 1)`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        identifier: String,
        credit_index: MarketIndex,
        discount_index: MarketIndex,
        currency: Currency,
        start_date: Date,
        maturity_date: Date,
        spread: f64,
        recovery: f64,
        premium_frequency: Frequency,
        day_counter: DayCounter,
    ) -> Result<Self> {
        if !matches!(credit_index, MarketIndex::Credit(_)) {
            return Err(QSError::InvalidValueErr(format!(
                "CDS credit index must be MarketIndex::Credit, got {credit_index}"
            )));
        }
        if maturity_date <= start_date {
            return Err(QSError::InvalidValueErr(
                "CDS maturity date must be after start date".into(),
            ));
        }
        if !(0.0..1.0).contains(&recovery) {
            return Err(QSError::InvalidValueErr(format!(
                "CDS recovery must be in [0, 1), got {recovery}"
            )));
        }
        Ok(Self {
            identifier,
            credit_index,
            discount_index,
            currency,
            start_date,
            maturity_date,
            spread,
            recovery,
            premium_frequency,
            day_counter,
        })
    }

    /// Credit curve index of the reference entity.
    #[must_use]
    pub const fn credit_index(&self) -> &MarketIndex {
        &self.credit_index
    }

    /// Discount curve index.
    #[must_use]
    pub const fn discount_index(&self) -> &MarketIndex {
        &self.discount_index
    }

    /// Instrument currency.
    #[must_use]
    pub const fn currency(&self) -> Currency {
        self.currency
    }

    /// Protection/premium start date.
    #[must_use]
    pub const fn start_date(&self) -> Date {
        self.start_date
    }

    /// Protection end / last premium date.
    #[must_use]
    pub const fn maturity_date(&self) -> Date {
        self.maturity_date
    }

    /// Running premium (decimal).
    #[must_use]
    pub const fn spread(&self) -> f64 {
        self.spread
    }

    /// Assumed recovery rate.
    #[must_use]
    pub const fn recovery(&self) -> f64 {
        self.recovery
    }

    /// Premium payment frequency.
    #[must_use]
    pub const fn premium_frequency(&self) -> Frequency {
        self.premium_frequency
    }

    /// Accrual day counter.
    #[must_use]
    pub const fn day_counter(&self) -> DayCounter {
        self.day_counter
    }
}

impl Instrument for CreditDefaultSwap {
    fn identifier(&self) -> String {
        self.identifier.clone()
    }
}

impl Discountable for CreditDefaultSwap {
    fn asset_class(&self) -> AssetClass {
        AssetClass::Credit
    }

    fn discount_index(&self) -> Option<MarketIndex> {
        Some(self.discount_index.clone())
    }

    fn currency(&self) -> Currency {
        self.currency
    }
}

/// A position on a [`CreditDefaultSwap`].
///
/// [`Side::LongReceive`] represents the protection buyer (pays the premium,
/// receives protection); [`Side::PayShort`] the protection seller.
pub struct CdsTrade {
    instrument: CreditDefaultSwap,
    trade_date: Date,
    notional: f64,
    side: Side,
}

impl CdsTrade {
    /// Creates a new CDS trade.
    #[must_use]
    pub const fn new(
        instrument: CreditDefaultSwap,
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

    /// Trade notional.
    #[must_use]
    pub const fn notional(&self) -> f64 {
        self.notional
    }
}

impl Trade<CreditDefaultSwap> for CdsTrade {
    fn instrument(&self) -> &CreditDefaultSwap {
        &self.instrument
    }

    fn trade_date(&self) -> Date {
        self.trade_date
    }

    fn side(&self) -> Side {
        self.side
    }
}
