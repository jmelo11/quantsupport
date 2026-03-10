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
        calendar::Calendar,
        calendars::weekendsonly::WeekendsOnly,
        daycounter::DayCounter,
        enums::Frequency,
    },
};

/// Details for NOWA (Norwegian Overnight Weighted Average).
/// Overnight unsecured rate published by Norges Bank.
#[derive(Copy, Clone, Serialize, Default, Deserialize)]
pub struct NOWAIndex;

impl MarketIndexDetails for NOWAIndex {
    fn quote_type(&self) -> QuoteType {
        QuoteType::Rate
    }

    fn name(&self) -> &'static str {
        "NOWA"
    }
}

impl RateIndexDetails for NOWAIndex {
    fn calendar(&self) -> Calendar {
        Calendar::WeekendsOnly(WeekendsOnly::new())
    }

    fn currency(&self) -> Currency {
        Currency::NOK
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
        MarketIndex::NOWA
    }
}
