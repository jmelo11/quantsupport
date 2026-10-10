//! Read-only scalar view of an AD discount curve.
use crate::{
    ad::dual::DualFwd,
    rates::{
        compounding::Compounding,
        yieldtermstructure::interestratestermstructure::InterestRatesTermStructure,
    },
    time::{date::Date, daycounter::DayCounter, enums::Frequency},
    utils::errors::Result,
};
pub struct PrimalCurve<'a>(pub(crate) &'a dyn InterestRatesTermStructure<DualFwd>);

impl InterestRatesTermStructure<f64> for PrimalCurve<'_> {
    fn reference_date(&self) -> Date {
        self.0.reference_date()
    }
    fn discount_factor(&self, date: Date) -> Result<f64> {
        Ok(self.0.discount_factor(date)?.value())
    }
    fn forward_rate(
        &self,
        start: Date,
        end: Date,
        comp: Compounding,
        freq: Frequency,
    ) -> Result<f64> {
        Ok(self.0.forward_rate(start, end, comp, freq)?.value())
    }
    fn nodes(&self) -> Option<Vec<(Date, f64)>> {
        self.0.nodes().map(|nodes| {
            nodes
                .into_iter()
                .map(|(date, value)| (date, value.value()))
                .collect()
        })
    }
    fn day_counter(&self) -> Option<DayCounter> {
        self.0.day_counter()
    }
    fn discount_factor_from_time(&self, time: f64) -> Result<f64> {
        Ok(self.0.discount_factor_from_time(time)?.value())
    }
    fn forward_rate_from_time(&self, start: f64, end: f64) -> Result<f64> {
        Ok(self.0.forward_rate_from_time(start, end)?.value())
    }
}
