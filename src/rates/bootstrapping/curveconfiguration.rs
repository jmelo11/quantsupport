use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{
    core::collateral::Discountable,
    indices::marketindex::MarketIndex,
    math::interpolation::interpolator::Interpolator,
    quotes::quote::{BuiltInstrument, Level, Quote},
    rates::bootstrapping::{
        bootstrapdiscountpolicy::BootstrapDiscountPolicy, resolvedinstrument::ResolvedInstrument,
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
    resolved_instruments: Option<Vec<ResolvedInstrument>>,
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
            resolved_instruments: None,
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
            let Some(quote) = selector.select(id) else {
                continue;
            };

            let quote_value = quote.levels().value(level)?;
            let built = quote.build_instrument(selector.reference_date(), level, fx_spot)?;
            let pillar_date = built.pillar_date()?;

            instruments.push(ResolvedInstrument::new(
                quote,
                level,
                built,
                quote_value,
                pillar_date,
            ));
        }

        instruments.sort_by_key(ResolvedInstrument::pillar_date);
        self.resolved_instruments = Some(instruments);
        Ok(())
    }

    /// Returns the resolved instruments in pillar order.
    ///
    /// # Errors
    /// Returns an error if the configuration has not been resolved yet.
    pub fn instruments(&self) -> Result<&[ResolvedInstrument]> {
        self.resolved_instruments
            .as_ref()
            .map(|instruments| instruments.as_slice())
            .ok_or_else(|| QSError::InvalidValueErr("Curve configuration not resolved".into()))
    }

    #[must_use]
    /// Returns the valuation date captured during resolution.
    pub fn reference_date(&self) -> Date {
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
        self.resolved_instruments
            .as_ref()
            .map(|instruments| {
                instruments
                    .iter()
                    .map(ResolvedInstrument::pillar_date)
                    .collect()
            })
            .unwrap_or_default()
    }

    #[must_use]
    /// Returns the resolved pillar labels in calibration order.
    pub fn pillar_labels(&self) -> Vec<String> {
        self.resolved_instruments
            .as_ref()
            .map(|instruments| {
                instruments
                    .iter()
                    .map(ResolvedInstrument::pillar_label)
                    .collect()
            })
            .unwrap_or_default()
    }

    #[must_use]
    /// Returns the resolved market quote values in calibration order.
    pub fn quote_values(&self) -> Vec<f64> {
        self.resolved_instruments
            .as_ref()
            .map(|instruments| {
                instruments
                    .iter()
                    .map(ResolvedInstrument::quote_value)
                    .collect()
            })
            .unwrap_or_default()
    }

    #[must_use]
    /// Returns the set of external curve dependencies implied by the resolved instruments.
    pub fn local_dependencies(&self) -> Result<HashSet<MarketIndex>> {
        let mut set = HashSet::new();
        set.insert(self.market_index.clone());
        let instruments = self.instruments()?;

        for instrument in instruments {
            match instrument.built() {
                BuiltInstrument::FixedRateDeposit(deposit) => {
                    if let Some(discount_index) = deposit.discount_index() {
                        set.insert(discount_index);
                    }
                }
                BuiltInstrument::Swap(swap) => {
                    set.insert(swap.forward_index());
                }
                BuiltInstrument::FixFloatCrossCurrencySwap(xccy) => {
                    set.insert(xccy.forward_index());
                }
                BuiltInstrument::FloatFloatCrossCurrencySwap(xccy) => {
                    set.insert(xccy.domestic_forward_index());
                    set.insert(xccy.foreign_forward_index());
                }
                BuiltInstrument::BasisSwap(basis) => {
                    set.insert(basis.pay_forward_index());
                    set.insert(basis.receive_forward_index());
                }
                BuiltInstrument::RateFutures(rf) => {
                    set.insert(rf.market_index());
                }
                BuiltInstrument::FxForward(_) => {
                    // Dependencies are resolved via the discount policy.
                }
                _ => {}
            };
        }
        Ok(set)
    }

    /// Returns the full set of curve dependencies, including those implied
    /// by the discount policy (e.g. CSA and collateral curves).
    pub fn dependencies(&self, policy: &BootstrapDiscountPolicy) -> Result<HashSet<MarketIndex>> {
        let mut deps = self.local_dependencies()?;
        let instruments = self.instruments()?;
        for instrument in instruments {
            match instrument.built() {
                BuiltInstrument::Swap(swap) => {
                    if let Ok(idx) = policy.discount_index(swap.fixed_leg()) {
                        deps.insert(idx);
                    }
                }
                BuiltInstrument::BasisSwap(basis) => {
                    if let Ok(idx) = policy.discount_index(basis.pay_leg()) {
                        deps.insert(idx);
                    }
                }
                BuiltInstrument::FixFloatCrossCurrencySwap(xccy) => {
                    if let Ok(idx) = policy.discount_index(xccy.domestic_leg()) {
                        deps.insert(idx);
                    }
                    if let Ok(idx) = policy.discount_index(xccy.foreign_leg()) {
                        deps.insert(idx);
                    }
                }
                BuiltInstrument::FloatFloatCrossCurrencySwap(xccy) => {
                    if let Ok(idx) = policy.discount_index(xccy.domestic_leg()) {
                        deps.insert(idx);
                    }
                    if let Ok(idx) = policy.discount_index(xccy.foreign_leg()) {
                        deps.insert(idx);
                    }
                }
                BuiltInstrument::FxForward(fwd) => {
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
