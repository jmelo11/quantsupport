use crate::{
    core::{
        collateral::Discountable,
        instrument::{AssetClass, Instrument},
        trade::{Side, Trade},
    },
    currencies::currency::Currency,
    indices::{fxpair::FxPair, marketindex::MarketIndex},
    instruments::{cashflows::payoffops::PayoffOps, equity::equityeuropeanoption::EuroOptionType},
    time::{date::Date, daycounter::DayCounter},
    utils::errors::{QSError, Result},
    volatility::volatilityindexing::Strike,
    xva::{
        claimevaluationstrategy::ClaimEvaluationStrategy, contigentclaim::ContingentClaim,
        makecontigentclaim::MakeContingentClaim,
    },
};

/// A European FX option giving the holder the right (but not the obligation) to
/// exchange a notional amount of base currency for quote currency at a fixed
/// strike rate on the expiry date.
#[derive(Clone)]
pub struct FxEuropeanOption {
    identifier: String,
    /// Must be of type [`MarketIndex::FxPair`].
    market_index: MarketIndex,
    expiry_date: Date,
    strike: Strike,
    option_type: EuroOptionType,
    base_currency: Currency,
    quote_currency: Currency,
    day_counter: DayCounter,
}

impl FxEuropeanOption {
    /// Creates a new [`FxEuropeanOption`].
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        identifier: String,
        market_index: MarketIndex,
        expiry_date: Date,
        strike: Strike,
        option_type: EuroOptionType,
        base_currency: Currency,
        quote_currency: Currency,
        day_counter: DayCounter,
    ) -> Self {
        Self {
            identifier,
            market_index,
            expiry_date,
            strike,
            option_type,
            base_currency,
            quote_currency,
            day_counter,
        }
    }

    /// Returns the expiry date.
    #[must_use]
    pub const fn expiry_date(&self) -> Date {
        self.expiry_date
    }

    /// Returns the market index of this option.
    #[must_use]
    pub const fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }

    /// Returns the strike price.
    #[must_use]
    pub const fn strike(&self) -> Strike {
        self.strike
    }

    /// Returns the option type (Call or Put).
    #[must_use]
    pub const fn option_type(&self) -> EuroOptionType {
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

    /// Returns the FX pair represented by the option's market index.
    ///
    /// # Errors
    /// Returns an error if the market index is not an FX pair.
    pub fn pair(&self) -> Result<FxPair> {
        match self.market_index {
            MarketIndex::FxPair(pair) => Ok(pair),
            _ => Err(QSError::InvalidValueErr(
                "Invalid Market Index for FXEuropeanOption".into(),
            )),
        }
    }
}

impl Instrument for FxEuropeanOption {
    fn identifier(&self) -> String {
        self.identifier.clone()
    }
}

impl Discountable for FxEuropeanOption {
    fn currency(&self) -> Currency {
        self.quote_currency
    }

    fn asset_class(&self) -> AssetClass {
        AssetClass::Fx
    }
}

/// Represents a trade of an FX option.
pub struct FxEuropeanOptionTrade {
    instrument: FxEuropeanOption,
    trade_date: Date,
    notional: f64,
    side: Side,
}

impl FxEuropeanOptionTrade {
    /// Creates a new [`FxEuropeanOptionTrade`].
    ///
    /// `notional` is in base-currency terms.
    #[must_use]
    pub const fn new(
        instrument: FxEuropeanOption,
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

    /// Returns the notional amount in the base currency.
    #[must_use]
    pub const fn notional(&self) -> f64 {
        self.notional
    }

    /// Decomposes the FX option trade into contingent claims.
    ///
    /// Produces a single [`ContingentClaim`] with a [`SpotPayoff`](crate::xva::claimevaluationstrategy::ClaimEvaluationStrategy::SpotPayoff) strategy:
    /// - Call: `max(S − K, 0)`
    /// - Put:  `max(K − S, 0)`
    ///
    /// # Errors
    /// Returns an error if claim construction fails.
    pub fn into_contingent_claims(&self) -> Result<Vec<ContingentClaim>> {
        let opt = self.instrument();
        let trade_id = opt.identifier();
        let expiry = opt.expiry_date();
        let strike = opt.strike().resolve(0.0);

        let payoff = match opt.option_type() {
            EuroOptionType::Call => PayoffOps::Max(
                Box::new(PayoffOps::Minus(
                    Box::new(PayoffOps::Index),
                    Box::new(PayoffOps::Const(strike)),
                )),
                Box::new(PayoffOps::Const(0.0)),
            ),
            EuroOptionType::Put => PayoffOps::Max(
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
            .with_index(opt.market_index().clone())
            .with_evaluation_strategy(ClaimEvaluationStrategy::SpotPayoff {
                payoff_ops: payoff,
                strike,
                observation_date: expiry,
            })
            .build()?;

        Ok(vec![claim])
    }
}

impl Trade<FxEuropeanOption> for FxEuropeanOptionTrade {
    fn instrument(&self) -> &FxEuropeanOption {
        &self.instrument
    }

    fn trade_date(&self) -> Date {
        self.trade_date
    }

    fn side(&self) -> Side {
        self.side
    }
}
