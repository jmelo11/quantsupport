use crate::{
    core::{
        collateral::Discountable,
        instrument::{AssetClass, Instrument},
        trade::{Side, Trade},
    },
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    instruments::cashflows::payoffops::PayoffOps,
    time::{date::Date, daycounter::DayCounter},
    utils::errors::Result,
    xva::{
        claimevaluationstrategy::ClaimEvaluationStrategy, contigentclaim::ContingentClaim,
        makecontigentclaim::MakeContingentClaim,
    },
};

/// Represents the type of an FX option.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FxOptionType {
    /// Call option — right to buy the base currency at the strike price.
    Call,
    /// Put option — right to sell the base currency at the strike price.
    Put,
}

/// A European FX option giving the holder the right (but not the obligation) to
/// exchange a notional amount of base currency for quote currency at a fixed
/// strike rate on the expiry date.
#[derive(Clone)]
pub struct FxOption {
    identifier: String,
    expiry_date: Date,
    strike: f64,
    option_type: FxOptionType,
    base_currency: Currency,
    quote_currency: Currency,
    day_counter: DayCounter,
    underlying_index: MarketIndex,
}

impl FxOption {
    /// Creates a new [`FxOption`].
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        identifier: String,
        expiry_date: Date,
        strike: f64,
        option_type: FxOptionType,
        base_currency: Currency,
        quote_currency: Currency,
        day_counter: DayCounter,
        underlying_index: MarketIndex,
    ) -> Self {
        Self {
            identifier,
            expiry_date,
            strike,
            option_type,
            base_currency,
            quote_currency,
            day_counter,
            underlying_index,
        }
    }

    /// Returns the expiry date.
    #[must_use]
    pub const fn expiry_date(&self) -> Date {
        self.expiry_date
    }

    /// Returns the strike price.
    #[must_use]
    pub const fn strike(&self) -> f64 {
        self.strike
    }

    /// Returns the option type (Call or Put).
    #[must_use]
    pub const fn option_type(&self) -> FxOptionType {
        self.option_type
    }

    /// Returns the base currency (the currency being bought in a call).
    #[must_use]
    pub const fn base_currency(&self) -> Currency {
        self.base_currency
    }

    /// Returns the quote currency.
    #[must_use]
    pub const fn quote_currency(&self) -> Currency {
        self.quote_currency
    }

    /// Returns the day count convention.
    #[must_use]
    pub const fn day_counter(&self) -> &DayCounter {
        &self.day_counter
    }

    /// Returns the underlying spot index (e.g. `MarketIndex::Other("EURUSD")`).
    #[must_use]
    pub const fn underlying_index(&self) -> &MarketIndex {
        &self.underlying_index
    }
}

impl Instrument for FxOption {
    fn identifier(&self) -> String {
        self.identifier.clone()
    }
}

impl Discountable for FxOption {
    fn currency(&self) -> Currency {
        self.quote_currency
    }

    fn asset_class(&self) -> AssetClass {
        AssetClass::Fx
    }
}

/// Represents a trade of an FX option.
pub struct FxOptionTrade {
    instrument: FxOption,
    trade_date: Date,
    notional: f64,
    side: Side,
}

impl FxOptionTrade {
    /// Creates a new [`FxOptionTrade`].
    ///
    /// `notional` is in base-currency terms.
    #[must_use]
    pub const fn new(instrument: FxOption, trade_date: Date, notional: f64, side: Side) -> Self {
        Self {
            instrument,
            trade_date,
            notional,
            side,
        }
    }

    /// Returns the notional amount in the base currency.
    #[must_use]
    pub const fn notional(&self) -> f64 {
        self.notional
    }

    /// Decomposes the FX option trade into contingent claims.
    ///
    /// Produces a single [`ContingentClaim`] with a [`SpotPayoff`] strategy:
    /// - Call: `max(S − K, 0)`
    /// - Put:  `max(K − S, 0)`
    ///
    /// # Errors
    /// Returns an error if claim construction fails.
    pub fn into_contingent_claims(&self) -> Result<Vec<ContingentClaim>> {
        let opt = self.instrument();
        let trade_id = opt.identifier();
        let expiry = opt.expiry_date();
        let strike = opt.strike();

        let payoff = match opt.option_type() {
            FxOptionType::Call => PayoffOps::Max(
                Box::new(PayoffOps::Minus(
                    Box::new(PayoffOps::Index),
                    Box::new(PayoffOps::Const(strike)),
                )),
                Box::new(PayoffOps::Const(0.0)),
            ),
            FxOptionType::Put => PayoffOps::Max(
                Box::new(PayoffOps::Minus(
                    Box::new(PayoffOps::Const(strike)),
                    Box::new(PayoffOps::Index),
                )),
                Box::new(PayoffOps::Const(0.0)),
            ),
        };

        let claim = MakeContingentClaim::default()
            .with_trade_id(trade_id)
            .with_leg_id(0)
            .with_payment_date(expiry)
            .with_currency(opt.quote_currency())
            .with_notional(self.notional)
            .with_side(self.side)
            .with_index(opt.underlying_index().clone())
            .with_evaluation_strategy(ClaimEvaluationStrategy::SpotPayoff {
                payoff_ops: payoff,
                strike,
                observation_date: expiry,
            })
            .build()?;

        Ok(vec![claim])
    }
}

impl Trade<FxOption> for FxOptionTrade {
    fn instrument(&self) -> &FxOption {
        &self.instrument
    }

    fn trade_date(&self) -> Date {
        self.trade_date
    }

    fn side(&self) -> Side {
        self.side
    }
}
