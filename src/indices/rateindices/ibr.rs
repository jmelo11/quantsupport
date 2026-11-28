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
        calendar::Calendar,
        calendars::colombia::Colombia,
        daycounter::DayCounter,
        enums::{BusinessDayConvention, Frequency, TimeUnit},
        period::Period,
    },
};

use super::macros::rate_index;

/// Swaps up to 18 months pay a single fixed coupon at maturity; longer swaps pay quarterly.
const fn ibr_fixed_frequency(tenor: Period) -> Frequency {
    let months = match tenor.units() {
        TimeUnit::Days => tenor.length() / 30,
        TimeUnit::Weeks => tenor.length() * 7 / 30,
        TimeUnit::Months => tenor.length(),
        TimeUnit::Years => tenor.length() * 12,
    };
    if months <= 18 {
        Frequency::Once
    } else {
        Frequency::Quarterly
    }
}

rate_index! {
    /// Details for the overnight IBR (Indicador Bancario de Referencia, COP) index.
    ///
    /// IBR overnight swaps compound daily fixings in arrears on the Colombian
    /// (BVC) business-day calendar.
    IBRIndex {
        market_index: IBR,
        currency: COP,
        calendar: Calendar::Colombia(Colombia::default()),
        fixing_lag: 0,
        day_counter: Actual360,
        compounding: Simple,
        compounding_frequency: Annual,
        frequency: Daily,
        is_in_arrears: true,
        swap_conventions: {
            fixed_day_counter: Actual360,
            fixed_frequency: ibr_fixed_frequency,
            floating_frequency: Quarterly,
            settlement_days: 2,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ibr_fixed_frequency_is_bullet_up_to_eighteen_months() {
        assert_eq!(
            ibr_fixed_frequency(Period::new(18, TimeUnit::Months)),
            Frequency::Once
        );
        assert_eq!(
            ibr_fixed_frequency(Period::new(1, TimeUnit::Years)),
            Frequency::Once
        );
        assert_eq!(
            ibr_fixed_frequency(Period::new(2, TimeUnit::Years)),
            Frequency::Quarterly
        );
    }

    #[test]
    fn ibr_is_a_cop_rate_index() -> crate::utils::errors::Result<()> {
        let index = MarketIndex::IBR;
        assert_eq!(index.to_string(), "IBR");
        assert_eq!("ibr".parse::<MarketIndex>()?, index);
        let details = index.rate_index_details()?;
        assert_eq!(details.market_index(), MarketIndex::IBR);
        assert_eq!(
            details
                .swap_conventions(Period::new(5, TimeUnit::Years))
                .fixed_frequency(),
            Frequency::Quarterly
        );
        assert_eq!(details.calendar(), Calendar::Colombia(Colombia::default()));
        Ok(())
    }
}
