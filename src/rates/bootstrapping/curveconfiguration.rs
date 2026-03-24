use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{
    core::collateral::Discountable,
    indices::marketindex::MarketIndex,
    math::interpolation::interpolator::Interpolator,
    quotes::quote::{CalibrationInstrumentType, Level, Quote},
    rates::bootstrapping::{
        bootstrapdiscountpolicy::BootstrapDiscountPolicy,
        calibrationinstrument::CalibrationInstrument,
    },
    time::{date::Date, daycounter::DayCounter},
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
#[derive(Serialize, Deserialize, Clone)]
pub struct CurveConfiguration {
    market_index: MarketIndex,
    #[serde(default = "default_day_counter")]
    day_counter: DayCounter,
    #[serde(default = "default_interpolator")]
    interpolator: Interpolator,
    #[serde(default = "default_enable_extrapolation")]
    enable_extrapolation: bool,
    /// Quote identifiers that define the pillars of this curve.
    #[serde(default)]
    quotes: Vec<String>,
    #[serde(skip)]
    reference_date: Option<Date>,
    #[serde(skip)]
    calibration_instruments: Option<Vec<CalibrationInstrument>>,
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

impl CurveConfiguration {
    /// Creates a curve specification.
    #[must_use]
    pub const fn new(
        market_index: MarketIndex,
        day_counter: DayCounter,
        interpolator: Interpolator,
        enable_extrapolation: bool,
        quotes: Vec<String>,
    ) -> Self {
        Self {
            market_index,
            day_counter,
            interpolator,
            enable_extrapolation,
            quotes,
            reference_date: None,
            calibration_instruments: None,
        }
    }

    /// Returns the market index for this spec.
    #[must_use]
    pub const fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }

    /// Resolves configured quote identifiers into concrete calibration
    /// instruments.
    ///
    /// # Errors
    /// Returns an error if a quote is not found, quote levels are missing,
    /// or a pillar date cannot be inferred.
    pub fn resolve(
        &mut self,
        selector: &impl QuoteSelector,
        level: Level,
        fx_spot: Option<f64>,
    ) -> Result<()> {
        let mut instruments = Vec::new();
        self.reference_date = Some(selector.reference_date());

        for id in &self.quotes {
            let quote = selector
                .select(id)
                .ok_or_else(|| QSError::NotFoundErr(format!("Quote {id} not found in quotes.")))?;

            let quote_value = quote.levels().value(level)?;
            let built = quote.build_instrument(selector.reference_date(), level, fx_spot)?;
            let pillar_date = built.pillar_date()?;

            instruments.push(CalibrationInstrument::new(
                quote,
                level,
                built,
                quote_value,
                pillar_date,
            ));
        }

        instruments.sort_by_key(CalibrationInstrument::pillar_date);
        self.calibration_instruments = Some(instruments);
        Ok(())
    }

    /// Returns the calibration instruments in pillar order.
    ///
    /// # Errors
    /// Returns an error if the configuration has not been resolved yet.
    pub fn instruments(&self) -> Result<&[CalibrationInstrument]> {
        self.calibration_instruments
            .as_deref()
            .ok_or_else(|| QSError::InvalidValueErr("Curve configuration not resolved".into()))
    }

    #[must_use]
    /// Returns the valuation date captured during resolution.
    #[allow(clippy::expect_used)]
    pub const fn reference_date(&self) -> Date {
        self.reference_date
            .expect("Curve configuration must be resolved before accessing reference_date")
    }

    #[must_use]
    /// Returns the day-count convention used for this curve.
    pub const fn day_counter(&self) -> DayCounter {
        self.day_counter
    }

    #[must_use]
    /// Returns the interpolation rule used between pillar dates.
    pub const fn interpolator(&self) -> Interpolator {
        self.interpolator
    }

    #[must_use]
    /// Returns whether extrapolation is enabled beyond the last pillar.
    pub const fn enable_extrapolation(&self) -> bool {
        self.enable_extrapolation
    }

    #[must_use]
    /// Returns the resolved pillar dates in calibration order.
    pub fn pillar_dates(&self) -> Vec<Date> {
        self.calibration_instruments
            .as_ref()
            .map(|instruments| {
                instruments
                    .iter()
                    .map(CalibrationInstrument::pillar_date)
                    .collect()
            })
            .unwrap_or_default()
    }

    #[must_use]
    /// Returns the resolved pillar labels in calibration order.
    pub fn pillar_labels(&self) -> Vec<String> {
        self.calibration_instruments
            .as_ref()
            .map(|instruments| {
                instruments
                    .iter()
                    .map(CalibrationInstrument::pillar_label)
                    .collect()
            })
            .unwrap_or_default()
    }

    #[must_use]
    /// Returns the resolved market quote values in calibration order.
    pub fn quote_values(&self) -> Vec<f64> {
        self.calibration_instruments
            .as_ref()
            .map(|instruments| {
                instruments
                    .iter()
                    .map(CalibrationInstrument::quote_value)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Returns the set of external curve dependencies implied by the calibration instruments.
    ///
    /// # Errors
    /// Returns an error if any instrument dependencies cannot be resolved.
    pub fn local_dependencies(&self) -> Result<HashSet<MarketIndex>> {
        let mut set = HashSet::new();
        set.insert(self.market_index.clone());
        let instruments = self.instruments()?;

        for instrument in instruments {
            match instrument.built() {
                CalibrationInstrumentType::FixedRateDeposit(deposit) => {
                    if let Some(discount_index) = deposit.discount_index() {
                        set.insert(discount_index);
                    }
                }
                CalibrationInstrumentType::Swap(swap) => {
                    set.insert(swap.forward_index());
                }
                CalibrationInstrumentType::FixFloatCrossCurrencySwap(xccy) => {
                    set.insert(xccy.forward_index());
                }
                CalibrationInstrumentType::FloatFloatCrossCurrencySwap(xccy) => {
                    set.insert(xccy.domestic_forward_index());
                    set.insert(xccy.foreign_forward_index());
                }
                CalibrationInstrumentType::BasisSwap(basis) => {
                    set.insert(basis.pay_forward_index());
                    set.insert(basis.receive_forward_index());
                }
                CalibrationInstrumentType::RateFutures(rf) => {
                    set.insert(rf.market_index());
                }
                // FxForward: dependencies are resolved via the discount policy.
                _ => {}
            }
        }
        Ok(set)
    }

    /// Returns the full set of curve dependencies, including those implied
    /// by the discount policy (e.g. CSA and collateral curves).
    ///
    /// # Errors
    /// Returns an error if any instrument dependencies cannot be resolved via the policy.
    pub fn dependencies(&self, policy: &BootstrapDiscountPolicy) -> Result<HashSet<MarketIndex>> {
        let mut deps = self.local_dependencies()?;
        let instruments = self.instruments()?;
        for instrument in instruments {
            match instrument.built() {
                CalibrationInstrumentType::Swap(swap) => {
                    if let Ok(idx) = policy.discount_index(swap.fixed_leg()) {
                        deps.insert(idx);
                    }
                }
                CalibrationInstrumentType::BasisSwap(basis) => {
                    if let Ok(idx) = policy.discount_index(basis.pay_leg()) {
                        deps.insert(idx);
                    }
                }
                CalibrationInstrumentType::FixFloatCrossCurrencySwap(xccy) => {
                    if let Ok(idx) = policy.discount_index(xccy.domestic_leg()) {
                        deps.insert(idx);
                    }
                    if let Ok(idx) = policy.discount_index(xccy.foreign_leg()) {
                        deps.insert(idx);
                    }
                }
                CalibrationInstrumentType::FloatFloatCrossCurrencySwap(xccy) => {
                    if let Ok(idx) = policy.discount_index(xccy.domestic_leg()) {
                        deps.insert(idx);
                    }
                    if let Ok(idx) = policy.discount_index(xccy.foreign_leg()) {
                        deps.insert(idx);
                    }
                }
                CalibrationInstrumentType::FxForward(fwd) => {
                    if let Ok(idx) = policy.discount_index_for_currency(fwd.base_currency()) {
                        deps.insert(idx);
                    }
                    if let Ok(idx) = policy.discount_index_for_currency(fwd.quote_currency()) {
                        deps.insert(idx);
                    }
                }
                _ => {}
            }
        }
        Ok(deps)
    }
}
