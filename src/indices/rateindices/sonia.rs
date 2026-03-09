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

/// # `SONIAIndex`
///
/// Details for SONIA (Sterling Overnight Index Average).
/// Overnight unsecured rate administered by the Bank of England.
#[derive(Copy, Clone, Serialize, Default, Deserialize)]
pub struct SONIAIndex;

impl MarketIndexDetails for SONIAIndex {
    fn quote_type(&self) -> QuoteType {
        QuoteType::Rate
    }

    fn name(&self) -> &'static str {
        "SONIA"
    }
}

impl RateIndexDetails for SONIAIndex {
    fn calendar(&self) -> Calendar {
        Calendar::WeekendsOnly(WeekendsOnly::new())
    }

    fn currency(&self) -> Currency {
        Currency::GBP
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
        MarketIndex::SONIA
    }
}
