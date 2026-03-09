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

/// # `TIBORIndex`
///
/// Details for TIBOR (Tokyo Interbank Offered Rate) term indices.
/// Covers 3m and 6m tenors.
#[derive(Copy, Clone, Serialize, Default, Deserialize)]
pub struct TIBORIndex;

impl MarketIndexDetails for TIBORIndex {
    fn quote_type(&self) -> QuoteType {
        QuoteType::Rate
    }

    fn name(&self) -> &'static str {
        "TIBOR"
    }
}

impl RateIndexDetails for TIBORIndex {
    fn calendar(&self) -> Calendar {
        Calendar::WeekendsOnly(WeekendsOnly::new())
    }

    fn currency(&self) -> Currency {
        Currency::JPY
    }

    fn fixing_lag(&self) -> i64 {
        2
    }

    fn rate_definition(&self) -> RateDefinition {
        RateDefinition::new(
            DayCounter::Actual365,
            Compounding::Simple,
            Frequency::Semiannual,
        )
    }

    fn market_index(&self) -> MarketIndex {
        MarketIndex::TIBOR3m
    }
}
