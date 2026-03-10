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

/// Details for TONAR (Tokyo Overnight Average Rate).
/// Overnight unsecured rate published by the Bank of Japan.
#[derive(Copy, Clone, Serialize, Default, Deserialize)]
pub struct TONARIndex;

impl MarketIndexDetails for TONARIndex {
    fn quote_type(&self) -> QuoteType {
        QuoteType::Rate
    }

    fn name(&self) -> &'static str {
        "TONAR"
    }
}

impl RateIndexDetails for TONARIndex {
    fn calendar(&self) -> Calendar {
        Calendar::WeekendsOnly(WeekendsOnly::new())
    }

    fn currency(&self) -> Currency {
        Currency::JPY
    }

    fn fixing_lag(&self) -> i64 {
        1
    }

    fn rate_definition(&self) -> RateDefinition {
        RateDefinition::new(
            DayCounter::Actual365,
            Compounding::Simple,
            Frequency::Annual,
        )
    }

    fn market_index(&self) -> MarketIndex {
        MarketIndex::TONAR
    }
}
