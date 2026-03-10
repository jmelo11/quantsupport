use serde::{Deserialize, Serialize};

use crate::{
    currencies::currency::Currency,
    indices::{
        marketindex::{MarketIndex, MarketIndexDetails},
        quotetype::QuoteType,
        rateindex::RateIndexDetails,
    },
    rates::{compounding::Compounding, interestrate::RateDefinition},
    time::{
        calendar::Calendar, calendars::weekendsonly::WeekendsOnly, daycounter::DayCounter,
        enums::Frequency,
    },
};

/// Details for SWESTR (Swedish krona Short-Term Rate).
/// Overnight unsecured rate published by the Riksbank.
#[derive(Copy, Clone, Serialize, Default, Deserialize)]
pub struct SWESTRIndex;

impl MarketIndexDetails for SWESTRIndex {
    fn quote_type(&self) -> QuoteType {
        QuoteType::Rate
    }

    fn name(&self) -> &'static str {
        "SWESTR"
    }
}

impl RateIndexDetails for SWESTRIndex {
    fn calendar(&self) -> Calendar {
        Calendar::WeekendsOnly(WeekendsOnly::new())
    }

    fn currency(&self) -> Currency {
        Currency::SEK
    }

    fn fixing_lag(&self) -> i64 {
        1
    }

    fn rate_definition(&self) -> RateDefinition {
        RateDefinition::new(
            DayCounter::Actual360,
            Compounding::Simple,
            Frequency::Annual,
        )
    }

    fn market_index(&self) -> MarketIndex {
        MarketIndex::SWESTR
    }
}
