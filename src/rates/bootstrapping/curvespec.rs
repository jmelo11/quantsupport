use serde::{Deserialize, Serialize};

use crate::{
    ad::adreal::{ADReal, IsReal},
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    math::interpolation::interpolator::{Interpolate as _, Interpolator},
    quotes::quote::{BuiltInstrument, Level, Quote},
    rates::{
        bootstrapping::resolvedcurvespec::{ResolvedCurveSpec, ResolvedInstrument},
        compounding::Compounding,
        interestrate::InterestRate,
    },
    time::{date::Date, daycounter::DayCounter, enums::Frequency},
    utils::errors::{QSError, Result},
};

/// Selects market quotes by identifier.
pub trait QuoteSelector {
    /// Returns the quote with the given identifier.
    fn select(&self, identifier: &str) -> Option<Quote>;
    /// Returns the reference (valuation) date used for building instruments.
    fn reference_date(&self) -> Date;
}

/// User-defined bootstrap specification for a curve, carrying the quote
/// identifiers that should be used to calibrate it.
#[derive(Debug, Serialize, Deserialize)]
pub struct CurveSpec {
    market_index: MarketIndex,
    #[serde(default = "default_currency")]
    currency: Currency,
    #[serde(default = "default_day_counter")]
    day_counter: DayCounter,
    #[serde(default = "default_interpolator")]
    interpolator: Interpolator,
    #[serde(default = "default_enable_extrapolation")]
    enable_extrapolation: bool,
    /// Quote identifiers that define the pillars of this curve.
    #[serde(default)]
    quotes: Vec<String>,
}

const fn default_currency() -> Currency {
    Currency::USD
}
const fn default_day_counter() -> DayCounter {
    DayCounter::Actual360
}
const fn default_interpolator() -> Interpolator {
    Interpolator::LogLinear
}
const fn default_enable_extrapolation() -> bool {
    true
}

impl CurveSpec {
    /// Creates a curve specification.
    #[must_use]
    pub const fn new(
        market_index: MarketIndex,
        currency: Currency,
        day_counter: DayCounter,
        interpolator: Interpolator,
        enable_extrapolation: bool,
        quotes: Vec<String>,
    ) -> Self {
        Self {
            market_index,
            currency,
            day_counter,
            interpolator,
            enable_extrapolation,
            quotes,
        }
    }

    /// Returns the market index for this spec.
    #[must_use]
    pub const fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }

    /// Returns the currency of this spec.
    #[must_use]
    pub const fn currency(&self) -> Currency {
        self.currency
    }

    /// Resolves configured quote identifiers into concrete calibration
    /// instruments.
    ///
    /// # Errors
    /// Returns an error if a quote is not found, quote levels are missing,
    /// or a pillar date cannot be inferred.
    pub fn resolve(
        &self,
        selector: &impl QuoteSelector,
        level: Level,
    ) -> Result<ResolvedCurveSpec> {
        let mut instruments = Vec::new();

        for id in &self.quotes {
            let Some(quote) = selector.select(id) else {
                continue;
            };

            let quote_value = quote.levels().value(level)?;
            let built = quote.build_instrument(selector.reference_date(), level)?;
            let pillar_date = Self::resolve_pillar_dates(&built)?;

            instruments.push(ResolvedInstrument::new(
                quote,
                level,
                built,
                quote_value,
                pillar_date,
            ));
        }

        instruments.sort_by_key(super::resolvedcurvespec::ResolvedInstrument::pillar_date);

        Ok(ResolvedCurveSpec::new(
            self.market_index.clone(),
            self.currency,
            self.day_counter,
            self.interpolator,
            self.enable_extrapolation,
            selector.reference_date(),
            instruments,
        ))
    }

    /// Resolves the pillar date for a given built instrument.
    fn resolve_pillar_dates(built: &BuiltInstrument) -> Result<Date> {
        match built {
            BuiltInstrument::FixedRateDeposit(x) => Ok(x.leg().last_payment_date()),
            BuiltInstrument::Swap(x) => Ok(x.fixed_leg().last_payment_date().max(x.floating_leg().last_payment_date())),
            BuiltInstrument::BasisSwap(x) => Ok(x.pay_leg().last_payment_date().max(x.receive_leg().last_payment_date())),
            BuiltInstrument::CrossCurrencySwap(x) => Ok(x.domestic_leg().last_payment_date().max(x.foreign_leg().last_payment_date())),
            BuiltInstrument::RateFutures(x) => Ok(x.end_date()),
            BuiltInstrument::FxForward(x) => Ok(x.delivery_date()),
            _ => Err(QSError::InvalidValueErr("Instrument not supported".into())),
        }
    }
}

/// Curve state carrying current obtained values during bootstrapping.
#[derive(Clone)]
pub struct BootstrappedCurve {
    reference_date: Date,
    times: Vec<f64>,
    discount_factors: Vec<ADReal>,
    day_counter: DayCounter,
    interpolator: Interpolator,
}

impl BootstrappedCurve {
    /// Creates a curve with flat discount factors = 1.0 at every pillar.
    #[must_use]
    pub fn new(
        reference_date: Date,
        times: Vec<f64>,
        day_counter: DayCounter,
        interpolator: Interpolator,
    ) -> Self {
        let discount_factors = vec![ADReal::one(); times.len()];

        Self {
            reference_date,
            times,
            discount_factors,
            day_counter,
            interpolator,
        }
    }

    /// Creates a curve with explicit discount factors.
    ///
    /// `times` and `discount_factors` must have the same length.
    #[must_use]
    pub const fn new_with_dfs(
        reference_date: Date,
        times: Vec<f64>,
        discount_factors: Vec<ADReal>,
        day_counter: DayCounter,
        interpolator: Interpolator,
    ) -> Self {
        Self {
            reference_date,
            times,
            discount_factors,
            day_counter,
            interpolator,
        }
    }

    /// Returns the reference date.
    #[must_use]
    pub const fn reference_date(&self) -> Date {
        self.reference_date
    }

    /// Returns the day counter.
    #[must_use]
    pub const fn day_counter(&self) -> DayCounter {
        self.day_counter
    }

    /// Returns the interpolator.
    #[must_use]
    pub const fn interpolator(&self) -> Interpolator {
        self.interpolator
    }

    /// Returns the pillar times.
    #[must_use]
    pub fn times(&self) -> &[f64] {
        &self.times
    }

    /// Returns the discount factors.
    #[must_use]
    pub fn discount_factors(&self) -> &[ADReal] {
        &self.discount_factors
    }

    /// Replaces all discount factors.
    pub fn set_discount_factors(&mut self, discount_factors: &[ADReal]) {
        self.discount_factors.clear();
        self.discount_factors.extend_from_slice(discount_factors);
    }

    /// Computes the discount factor at `date` by interpolating the curve.
    ///
    /// # Errors
    /// Returns an error if interpolation fails.
    pub fn discount_factor(&self, date: Date) -> Result<ADReal> {
        let year_fraction = ADReal::new(self.day_counter.year_fraction(self.reference_date, date));

        let tmp_yfs = self
            .times
            .iter()
            .copied()
            .map(ADReal::new)
            .collect::<Vec<ADReal>>();

        let discount_factor =
            self.interpolator
                .interpolate(year_fraction, &tmp_yfs, &self.discount_factors, true)?;
        Ok(discount_factor)
    }

    /// Computes the simply-compounded forward rate between two dates.
    ///
    /// # Errors
    /// Returns an error if the underlying discount-factor interpolation fails.
    pub fn forward_rate(
        &self,
        start_date: Date,
        end_date: Date,
        comp: Compounding,
        freq: Frequency,
    ) -> Result<ADReal> {
        let discount_factor_to_star = self.discount_factor(start_date)?;
        let discount_factor_to_end = self.discount_factor(end_date)?;

        let comp_factor = discount_factor_to_star / discount_factor_to_end;
        let t = self.day_counter.year_fraction(start_date, end_date);

        Ok(InterestRate::<ADReal>::implied_rate(
            comp_factor.into(),
            self.day_counter,
            comp,
            freq,
            t,
        )?
        .rate())
    }
}
