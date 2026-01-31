use crate::{
    indices::{
        marketindex::{MarketIndex, MarketIndexDetails},
        quotetype::QuoteType,
        rateindex::RateIndexDetails,
    },
    prelude::{
        Calendar, Compounding, Currency, DayCounter, Frequency, Market, RateDefinition,
        UnitedStates,
    },
};

/// # SOFRIndex
pub struct SOFRIndex;
impl MarketIndexDetails for SOFRIndex {
    fn quote_type(&self) -> QuoteType {
        QuoteType::Rate
    }

    fn name(&self) -> &'static str {
        "SOFR"
    }
}

impl RateIndexDetails for SOFRIndex {
    fn calendar(&self) -> Calendar {
        Calendar::UnitedStates(UnitedStates::new(Market::Sofr))
    }
    fn currency(&self) -> Currency {
        Currency::USD
    }
    fn fixing_lag(&self) -> i64 {
        1
    }
    fn rate_definition(&self) -> Option<crate::prelude::RateDefinition> {
        Some(RateDefinition::new(
            DayCounter::Actual360,
            Compounding::Simple,
            Frequency::Annual,
        ))
    }

    fn market_index(&self) -> MarketIndex {
        MarketIndex::SOFR
    }
}
