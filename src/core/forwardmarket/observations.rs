//! Horizon spot observations and scenario fixing histories.

use std::collections::HashSet;

use crate::{
    core::forwardmarket::{
        advanceintime::AdvanceInTime,
        configuration::{FixingRoll, ForwardMarketConfiguration},
        source::ForwardMarketSource,
    },
    currencies::currency::Currency,
    indices::{fxpair::FxPair, marketindex::MarketIndex},
    quotes::{
        fixingstore::FixingStore,
        fxstore::{FxRateRecord, FxStore},
    },
    rates::{
        bootstrapping::bootstrapdiscountpolicy::BootstrapDiscountPolicy, interestrate::InterestRate,
    },
    time::{calendars::traits::IsCalendar, date::Date, enums::TimeUnit, period::Period},
    utils::errors::{QSError, Result},
};

impl ForwardMarketSource<'_> {
    /// Returns the collateral-consistent forward FX rate at a date.
    ///
    /// # Errors
    /// Requires source FX rates and collateral discount curves.
    pub fn fx_forward(&self, base: Currency, quote: Currency, date: Date) -> Result<f64> {
        let context = self.context();
        let policy =
            BootstrapDiscountPolicy::new(context.base_index().clone(), context.base_currency());
        let curve_factor = |currency| {
            let index = policy.discount_index_for_currency(currency)?;
            let curve = context
                .constructed_elements()
                .discount_curve(&index)
                .ok_or_else(|| {
                    QSError::NotFoundErr(format!(
                        "FX advancement requires collateral curve {index}"
                    ))
                })?;
            curve
                .curve()
                .discount_factor(date)
                .map(|value| value.value())
        };
        let value = context.fx_store().get_fx_rate(base, quote)?.value() * curve_factor(base)?
            / curve_factor(quote)?;
        if !value.is_finite() || value <= 0.0 {
            return Err(QSError::InvalidValueErr(
                "Forward FX must be finite and positive".into(),
            ));
        }
        Ok(value)
    }
}

impl AdvanceInTime for FxStore {
    type Output = Self;
    fn advance_in_time(
        &self,
        config: &ForwardMarketConfiguration,
        source: &ForwardMarketSource<'_>,
    ) -> Result<Self> {
        source.validate_date(config)?;
        if config.target_date() == source.context().evaluation_date() {
            return Ok(Self::from_records(self.records()));
        }
        let records = self
            .records()
            .into_iter()
            .map(|record| {
                Ok(FxRateRecord::new(
                    record.base(),
                    record.quote(),
                    source.fx_forward(record.base(), record.quote(), config.target_date())?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self::from_records(records))
    }
}

impl AdvanceInTime for FixingStore {
    type Output = Self;
    fn advance_in_time(
        &self,
        config: &ForwardMarketConfiguration,
        source: &ForwardMarketSource<'_>,
    ) -> Result<Self> {
        source.validate_date(config)?;
        let today = source.context().evaluation_date();
        let horizon = config.target_date();
        let mut result = Self::default();
        // A source dataset can contain later observations; do not introduce look-ahead.
        for (index, date, value) in self.observations().filter(|(_, date, _)| *date <= today) {
            result.add_fixing(index, date, value);
        }
        if horizon == today {
            return Ok(result);
        }
        if config.fixing_roll() == FixingRoll::Project {
            result.project_rate_observations(source, horizon)?;
        }
        let start = if config.fixing_roll() == FixingRoll::Project {
            today
        } else {
            horizon
        };
        result.project_equity_observations(self, source, start, horizon)?;
        result.project_fx_observations(self, source, start, horizon)?;
        Ok(result)
    }
}

impl FixingStore {
    fn project_rate_observations(
        &mut self,
        source: &ForwardMarketSource<'_>,
        horizon: Date,
    ) -> Result<()> {
        for (index, curve) in source.context().constructed_elements().discount_curves() {
            let Ok(details) = index.rate_index_details() else {
                continue;
            };
            let calendar = details.calendar();
            let definition = details.rate_definition();
            let tenor = Period::from_frequency(details.frequency()).ok_or_else(|| {
                QSError::InvalidValueErr(format!("No observation tenor for {index}"))
            })?;
            let mut date = source.context().evaluation_date();
            while date < horizon {
                if calendar.is_business_day(&date) {
                    let start = if details.is_in_arrears() {
                        date
                    } else {
                        let lag = i32::try_from(details.fixing_lag()).map_err(|_| {
                            QSError::InvalidValueErr("Fixing lag exceeds date range".into())
                        })?;
                        calendar.advance(date, Period::new(lag, TimeUnit::Days), None, false)?
                    };
                    let convention = details.swap_conventions(tenor).business_day_convention();
                    let end = calendar.advance(start, tenor, Some(convention), false)?;
                    let factor = curve.curve().discount_factor(start)?.value()
                        / curve.curve().discount_factor(end)?.value();
                    let rate = InterestRate::<f64>::implied_rate(
                        factor,
                        definition.day_counter(),
                        definition.compounding(),
                        definition.frequency(),
                        definition.day_counter().year_fraction(start, end),
                    )?
                    .rate();
                    self.add_fixing(index, date, rate);
                }
                date += 1;
            }
        }
        Ok(())
    }

    fn project_equity_observations(
        &mut self,
        history: &Self,
        source: &ForwardMarketSource<'_>,
        start: Date,
        horizon: Date,
    ) -> Result<()> {
        let equities = history
            .observations()
            .filter_map(|(index, _, _)| {
                matches!(index, MarketIndex::Equity(_)).then_some(index.clone())
            })
            .collect::<HashSet<_>>();
        for index in equities {
            let spot = history.fixing(&index, source.context().evaluation_date())?;
            let financing = source
                .context()
                .constructed_elements()
                .discount_curve(&index)
                .ok_or_else(|| QSError::NotFoundErr(format!("Financing curve for {index}")))?;
            let dividend = source
                .context()
                .constructed_elements()
                .dividend_curve(&index);
            let mut date = start;
            while date <= horizon {
                let dividend_factor = dividend.map_or(Ok(1.0), |curve| {
                    curve
                        .curve()
                        .discount_factor(date)
                        .map(|factor| factor.value())
                })?;
                let value =
                    spot * dividend_factor / financing.curve().discount_factor(date)?.value();
                self.add_fixing(&index, date, value);
                date += 1;
            }
        }
        Ok(())
    }

    fn project_fx_observations(
        &mut self,
        history: &Self,
        source: &ForwardMarketSource<'_>,
        start: Date,
        horizon: Date,
    ) -> Result<()> {
        let mut pairs = source
            .context()
            .fx_store()
            .records()
            .iter()
            .map(|record| FxPair::new(record.base(), record.quote()))
            .collect::<Result<HashSet<_>>>()?;
        pairs.extend(
            history
                .observations()
                .filter_map(|(index, _, _)| match index {
                    MarketIndex::FxPair(pair) => Some(*pair),
                    _ => None,
                }),
        );
        for pair in pairs {
            let index = MarketIndex::FxPair(pair);
            let mut date = start;
            while date <= horizon {
                self.add_fixing(
                    &index,
                    date,
                    source.fx_forward(pair.base(), pair.quote(), date)?,
                );
                date += 1;
            }
        }
        Ok(())
    }
}
