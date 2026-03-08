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
        calendars::unitedstates::{Market, UnitedStates},
        daycounter::DayCounter,
        enums::Frequency,
    },
};

/// # `SOFRIndex`
///
/// Details for the SOFR rate index.
#[derive(Copy, Clone, Serialize, Default, Deserialize)]
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
    fn rate_definition(&self) -> RateDefinition {
        RateDefinition::new(
            DayCounter::Actual360,
            Compounding::Simple,
            Frequency::Annual,
        )
    }

    fn market_index(&self) -> MarketIndex {
        MarketIndex::SOFR
    }
}
