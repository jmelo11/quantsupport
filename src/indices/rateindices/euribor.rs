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
        calendars::target::TARGET,
        daycounter::DayCounter,
        enums::Frequency,
    },
};

/// # `EuriborIndex`
///
/// Details for EURIBOR (Euro Interbank Offered Rate) term indices.
/// Covers 1m, 3m, 6m, and 12m tenors.
#[derive(Copy, Clone, Serialize, Default, Deserialize)]
pub struct EuriborIndex;

impl MarketIndexDetails for EuriborIndex {
    fn quote_type(&self) -> QuoteType {
        QuoteType::Rate
    }

    fn name(&self) -> &'static str {
        "EURIBOR"
    }
}

impl RateIndexDetails for EuriborIndex {
    fn calendar(&self) -> Calendar {
        Calendar::TARGET(TARGET::new())
    }

    fn currency(&self) -> Currency {
        Currency::EUR
    }

    fn fixing_lag(&self) -> i64 {
        2
    }

    fn rate_definition(&self) -> RateDefinition {
        RateDefinition::new(
            DayCounter::Actual360,
            Compounding::Simple,
            Frequency::Annual,
        )
    }

    fn market_index(&self) -> MarketIndex {
        MarketIndex::EURIBOR3m
    }
}
