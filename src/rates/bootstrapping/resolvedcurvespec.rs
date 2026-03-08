use std::collections::HashSet;

use crate::{
    ad::adreal::ADReal,
    core::request::LegsProvider,
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    math::interpolation::interpolator::Interpolator,
    quotes::quote::{BuiltInstrument, Level, Quote},
    rates::bootstrapping::bootstrapdiscountpolicy::BootstrapDiscountPolicy,
    time::{date::Date, daycounter::DayCounter},
};

// ---------------------------------------------------------------------------
// ResolvedInstrument
// ---------------------------------------------------------------------------

/// A resolved calibration instrument: a quote that has been turned into a
/// concrete `BuiltInstrument` with a known pillar date and AD-enabled quote
/// value.
pub struct ResolvedInstrument {
    quote: Quote,
    level: Level,
    built: BuiltInstrument,
    quote_value: ADReal,
    pillar_date: Date,
}

impl ResolvedInstrument {
    /// Creates a resolved calibration instrument.
    #[must_use]
    pub const fn new(
        quote: Quote,
        level: Level,
        built: BuiltInstrument,
        quote_value: ADReal,
        pillar_date: Date,
    ) -> Self {
        Self {
            quote,
            level,
            built,
            quote_value,
            pillar_date,
        }
    }

    /// Returns the source quote.
    #[must_use]
    pub const fn quote(&self) -> &Quote {
        &self.quote
    }

    /// Returns the quote level.
    #[must_use]
    pub const fn level(&self) -> Level {
        self.level
    }

    /// Returns the built instrument.
    #[must_use]
    pub const fn built(&self) -> &BuiltInstrument {
        &self.built
    }

    /// Returns the market input value as an `ADReal`.
    #[must_use]
    pub const fn quote_value(&self) -> &ADReal {
        &self.quote_value
    }

    /// Returns the pillar date.
    #[must_use]
    pub const fn pillar_date(&self) -> Date {
        self.pillar_date
    }

    /// Returns the reporting label associated with this calibration input.
    #[must_use]
    pub fn pillar_label(&self) -> String {
        self.quote.details().identifier()
    }
}

// ---------------------------------------------------------------------------
// ResolvedCurveSpec
// ---------------------------------------------------------------------------

/// Resolved calibration payload for one curve: all tenors have been turned
/// into concrete `ResolvedInstrument`s, sorted by pillar date.
pub struct ResolvedCurveSpec {
    market_index: MarketIndex,
    currency: Currency,
    day_counter: DayCounter,
    interpolator: Interpolator,
    enable_extrapolation: bool,
    reference_date: Date,
    instruments: Vec<ResolvedInstrument>,
}

impl ResolvedCurveSpec {
    /// Creates a resolved curve specification.
    #[must_use]
    pub const fn new(
        market_index: MarketIndex,
        currency: Currency,
        day_counter: DayCounter,
        interpolator: Interpolator,
        enable_extrapolation: bool,
        reference_date: Date,
        instruments: Vec<ResolvedInstrument>,
    ) -> Self {
        Self {
            market_index,
            currency,
            day_counter,
            interpolator,
            enable_extrapolation,
            reference_date,
            instruments,
        }
    }

    /// Returns the reference (valuation) date for this curve.
    #[must_use]
    pub const fn reference_date(&self) -> Date {
        self.reference_date
    }

    /// Returns the target market index.
    #[must_use]
    pub const fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }

    /// Returns the target curve currency.
    #[must_use]
    pub const fn currency(&self) -> Currency {
        self.currency
    }

    /// Returns the curve day counter.
    #[must_use]
    pub const fn day_counter(&self) -> DayCounter {
        self.day_counter
    }

    /// Returns the interpolator.
    #[must_use]
    pub const fn interpolator(&self) -> Interpolator {
        self.interpolator
    }

    /// Returns the extrapolation flag.
    #[must_use]
    pub const fn enable_extrapolation(&self) -> bool {
        self.enable_extrapolation
    }

    /// Returns the resolved instruments.
    #[must_use]
    pub fn instruments(&self) -> &[ResolvedInstrument] {
        &self.instruments
    }

    /// Collects all external curve dependencies induced by the instruments
    /// in this spec and the given discount policy.
    #[must_use]
    pub fn dependencies(&self, policy: &BootstrapDiscountPolicy) -> HashSet<MarketIndex> {
        let target = &self.market_index;
        let mut deps = HashSet::new();

        for instr in &self.instruments {
            for dep in policy.dependencies(instr.built(), target) {
                deps.insert(dep);
            }

            // Additionally collect any leg-level projection indices that
            // differ from the target (e.g. the floating leg of a swap
            // might reference a curve that is not the one being bootstrapped).
            match instr.built() {
                BuiltInstrument::Swap(s) => {
                    for leg in s.legs() {
                        if let Some(idx) = leg.market_index() {
                            if idx != target {
                                deps.insert(idx.clone());
                            }
                        }
                    }
                }
                BuiltInstrument::BasisSwap(bs) => {
                    for leg in bs.legs() {
                        if let Some(idx) = leg.market_index() {
                            if idx != target {
                                deps.insert(idx.clone());
                            }
                        }
                    }
                }
                BuiltInstrument::CrossCurrencySwap(xccy) => {
                    for leg in xccy.legs() {
                        if let Some(idx) = leg.market_index() {
                            if idx != target {
                                deps.insert(idx.clone());
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        deps
    }

    /// Returns the pillar dates (sorted, one per instrument).
    #[must_use]
    pub fn pillar_dates(&self) -> Vec<Date> {
        self.instruments
            .iter()
            .map(ResolvedInstrument::pillar_date)
            .collect()
    }

    /// Returns the pillar labels (one per instrument).
    #[must_use]
    pub fn pillar_labels(&self) -> Vec<String> {
        self.instruments
            .iter()
            .map(ResolvedInstrument::pillar_label)
            .collect()
    }

    /// Returns the market par-quote values (one per instrument) as `ADReal`.
    ///
    /// These are the original market inputs used to calibrate the curve.
    #[must_use]
    pub fn quote_values(&self) -> Vec<ADReal> {
        self.instruments.iter().map(|i| *i.quote_value()).collect()
    }
}
