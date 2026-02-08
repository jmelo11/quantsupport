use serde::{Deserialize, Serialize};

use crate::{
    core::{contextmanager::ContextManager, instrument::Instrument, trade::Trade},
    indices::marketindex::MarketIndex,
    time::{date::Date, daycounter::DayCounter, period::Period, schedule::MakeSchedule, schedule::Schedule},
    utils::errors::{AtlasError, Result},
};

/// Defines swap direction for fixed leg payments.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum SwapDirection {
    /// Pay fixed, receive floating.
    PayFixed,
    /// Receive fixed, pay floating.
    ReceiveFixed,
}

/// Simple fixed-float interest rate swap.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InterestRateSwap {
    name: String,
    effective_date: Date,
    maturity_date: Date,
    fixed_rate: f64,
    fixed_leg_tenor: Period,
    float_leg_tenor: Period,
    day_counter: DayCounter,
    direction: SwapDirection,
    market_index: MarketIndex,
    discount_curve_index: Option<MarketIndex>,
}

impl InterestRateSwap {
    /// Creates a new interest rate swap.
    #[must_use]
    pub fn new(
        name: String,
        effective_date: Date,
        maturity_date: Date,
        fixed_rate: f64,
        fixed_leg_tenor: Period,
        float_leg_tenor: Period,
        day_counter: DayCounter,
        direction: SwapDirection,
        market_index: MarketIndex,
    ) -> Self {
        Self {
            name,
            effective_date,
            maturity_date,
            fixed_rate,
            fixed_leg_tenor,
            float_leg_tenor,
            day_counter,
            direction,
            market_index,
            discount_curve_index: None,
        }
    }

    /// Sets the discount curve index for dual-curve discounting.
    #[must_use]
    pub fn with_discount_curve_index(mut self, discount_curve_index: MarketIndex) -> Self {
        self.discount_curve_index = Some(discount_curve_index);
        self
    }

    /// Returns the effective date.
    #[must_use]
    pub const fn effective_date(&self) -> Date {
        self.effective_date
    }

    /// Returns the maturity date.
    #[must_use]
    pub const fn maturity_date(&self) -> Date {
        self.maturity_date
    }

    /// Returns the fixed rate.
    #[must_use]
    pub const fn fixed_rate(&self) -> f64 {
        self.fixed_rate
    }

    /// Returns the fixed leg tenor.
    #[must_use]
    pub const fn fixed_leg_tenor(&self) -> Period {
        self.fixed_leg_tenor
    }

    /// Returns the floating leg tenor.
    #[must_use]
    pub const fn float_leg_tenor(&self) -> Period {
        self.float_leg_tenor
    }

    /// Returns the day counter.
    #[must_use]
    pub const fn day_counter(&self) -> DayCounter {
        self.day_counter
    }

    /// Returns the swap direction.
    #[must_use]
    pub const fn direction(&self) -> SwapDirection {
        self.direction
    }

    /// Returns the market index.
    #[must_use]
    pub fn market_index(&self) -> MarketIndex {
        self.market_index.clone()
    }

    /// Returns the discount curve index (defaults to the market index).
    #[must_use]
    pub fn discount_curve_index(&self) -> MarketIndex {
        self.discount_curve_index
            .clone()
            .unwrap_or_else(|| self.market_index.clone())
    }

    /// Builds the fixed leg schedule.
    pub fn fixed_schedule(&self) -> Result<Schedule> {
        MakeSchedule::new(self.effective_date, self.maturity_date)
            .with_tenor(self.fixed_leg_tenor)
            .build()
    }

    /// Builds the floating leg schedule.
    pub fn float_schedule(&self) -> Result<Schedule> {
        MakeSchedule::new(self.effective_date, self.maturity_date)
            .with_tenor(self.float_leg_tenor)
            .build()
    }
}

impl Instrument for InterestRateSwap {
    fn identifier(&self) -> String {
        self.name.clone()
    }

    fn resolve(&self, _: &ContextManager) -> Result<Self> {
        if self.effective_date > self.maturity_date {
            return Err(AtlasError::InvalidValueErr(
                "Swap effective date after maturity.".into(),
            ));
        }
        Ok(self.clone())
    }
}

/// Trade representation for interest rate swaps.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InterestRateSwapTrade {
    swap: InterestRateSwap,
    trade_date: Date,
    notional: f64,
    trade_price: Option<f64>,
}

impl InterestRateSwapTrade {
    /// Creates a new swap trade.
    #[must_use]
    pub fn new(swap: InterestRateSwap, trade_date: Date, notional: f64) -> Self {
        Self {
            swap,
            trade_date,
            notional,
            trade_price: None,
        }
    }

    /// Sets the trade price.
    #[must_use]
    pub fn with_trade_price(mut self, trade_price: f64) -> Self {
        self.trade_price = Some(trade_price);
        self
    }

    /// Returns the trade date.
    #[must_use]
    pub const fn trade_date(&self) -> Date {
        self.trade_date
    }

    /// Returns the trade notional.
    #[must_use]
    pub const fn notional(&self) -> f64 {
        self.notional
    }
}

impl Trade<InterestRateSwap> for InterestRateSwapTrade {
    fn instrument(&self) -> InterestRateSwap {
        self.swap.clone()
    }
}
