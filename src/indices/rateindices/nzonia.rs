use serde::{Deserialize, Serialize};

use crate::{
    currencies::currency::Currency,
    indices::{
        marketindex::{MarketIndex, MarketIndexDetails},
        quotetype::QuoteType,
        rateindex::{RateIndexDetails, SwapConventions},
    },
    rates::{compounding::Compounding, interestrate::RateDefinition},
    time::{
        calendar::Calendar, calendars::newzealand::NewZealand, daycounter::DayCounter,
        enums::Frequency,
    },
};

/// Details for NZONIA (New Zealand Overnight Index Average).
/// Overnight unsecured rate based on the RBNZ Official Cash Rate.
#[derive(Copy, Clone, Serialize, Default, Deserialize)]
pub struct NZONIAIndex;

impl MarketIndexDetails for NZONIAIndex {
    fn quote_type(&self) -> QuoteType {
        QuoteType::Rate
    }

    fn name(&self) -> &'static str {
        "NZONIA"
    }
}

impl RateIndexDetails for NZONIAIndex {
    fn calendar(&self) -> Calendar {
        Calendar::NewZealand(NewZealand::default())
    }

    fn currency(&self) -> Currency {
        Currency::NZD
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

    fn swap_conventions(&self, _tenor: crate::time::period::Period) -> SwapConventions {
        SwapConventions::new(
            self.rate_definition(),
            Frequency::Annual,
            Frequency::Annual,
            self.rate_definition().day_counter(),
            2,
            crate::time::enums::BusinessDayConvention::ModifiedFollowing,
        )
    }

    fn market_index(&self) -> MarketIndex {
        MarketIndex::NZONIA
    }
}
