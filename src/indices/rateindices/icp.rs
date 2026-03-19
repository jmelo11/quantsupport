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
        calendars::nullcalendar::NullCalendar,
        daycounter::DayCounter,
        enums::Frequency,
    },
};

/// Details for the ICP (Índice de Cámara Promedio) Chilean overnight rate index.
#[derive(Copy, Clone, Serialize, Default, Deserialize)]
pub struct ICPIndex;

impl MarketIndexDetails for ICPIndex {
    fn quote_type(&self) -> QuoteType {
        QuoteType::Rate
    }

    fn name(&self) -> &'static str {
        "ICP"
    }
}

impl RateIndexDetails for ICPIndex {
    fn calendar(&self) -> Calendar {
        Calendar::NullCalendar(NullCalendar::new())
    }

    fn currency(&self) -> Currency {
        Currency::CLP
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
        MarketIndex::ICP
    }
}
