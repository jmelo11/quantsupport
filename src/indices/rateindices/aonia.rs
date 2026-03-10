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


/// Details for AONIA (AUD Overnight Index Average).
/// Overnight unsecured rate published by the Reserve Bank of Australia.
#[derive(Copy, Clone, Serialize, Default, Deserialize)]
pub struct AONIAIndex;

impl MarketIndexDetails for AONIAIndex {
    fn quote_type(&self) -> QuoteType {
        QuoteType::Rate
    }

    fn name(&self) -> &'static str {
        "AONIA"
    }
}

impl RateIndexDetails for AONIAIndex {
    fn calendar(&self) -> Calendar {
        Calendar::WeekendsOnly(WeekendsOnly::new())
    }

    fn currency(&self) -> Currency {
        Currency::AUD
    }

    fn fixing_lag(&self) -> i64 {
        0
    }

    fn rate_definition(&self) -> RateDefinition {
        RateDefinition::new(
            DayCounter::Actual365,
            Compounding::Simple,
            Frequency::Annual,
        )
    }

    fn market_index(&self) -> MarketIndex {
        MarketIndex::AONIA
    }
}
