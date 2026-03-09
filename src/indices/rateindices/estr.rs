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

/// # `ESTRIndex`
///
/// Details for the €STR (Euro Short-Term Rate) index.
/// Overnight unsecured rate published by the ECB.
#[derive(Copy, Clone, Serialize, Default, Deserialize)]
pub struct ESTRIndex;

impl MarketIndexDetails for ESTRIndex {
    fn quote_type(&self) -> QuoteType {
        QuoteType::Rate
    }

    fn name(&self) -> &'static str {
        "ESTR"
    }
}

impl RateIndexDetails for ESTRIndex {
    fn calendar(&self) -> Calendar {
        Calendar::TARGET(TARGET::new())
    }

    fn currency(&self) -> Currency {
        Currency::EUR
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
        MarketIndex::ESTR
    }
}
