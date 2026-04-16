use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{
    core::collateral::Discountable,
    indices::marketindex::MarketIndex,
    math::interpolation::interpolator::Interpolator,
    quotes::{
        calibrationinstrument::CalibrationInstrument,
        quote::{CalibrationInstrumentType, Level},
        quoteselector::QuoteSelector,
    },
    rates::bootstrapping::bootstrapdiscountpolicy::BootstrapDiscountPolicy,
    time::{date::Date, daycounter::DayCounter},
    utils::errors::{QSError, Result},
};

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
        self.calibration_instruments.as_deref().ok_or_else(|| {
            QSError::InvalidValueErr(format!(
                "Calibration instruments of curve {} not constructed.",
                self.market_index
            ))
        })
    }

    /// Returns the valuation date captured during resolution.
    ///
    /// # Errors
    /// Returns an error if the configuration has not been resolved yet.
    pub fn reference_date(&self) -> Result<Date> {
        self.reference_date
            .ok_or_else(|| QSError::InvalidValueErr("Curve configuration not resolved".into()))
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

    /// Returns the full set of curve dependencies, including those implied
    /// by the discount policy (e.g. CSA and collateral curves).
    ///
    /// # Errors
    /// Returns an error if any instrument dependencies cannot be resolved via the policy or
    /// the curve has no constructed instruments.
    pub fn dependencies(&self, policy: &BootstrapDiscountPolicy) -> Result<HashSet<MarketIndex>> {
        let mut set = HashSet::new();
        set.insert(self.market_index.clone());
        let instruments = self.instruments()?;
        if instruments.is_empty() {
            return Err(QSError::NotFoundErr(format!(
                "Curve for index {} has no calibration instruments.",
                self.market_index
            )));
        };
        for instrument in instruments {
            match instrument.built() {
                CalibrationInstrumentType::FixedRateDeposit(deposit) => {
                    if let Some(discount_index) = deposit.discount_index() {
                        set.insert(discount_index);
                    }
                }
                CalibrationInstrumentType::Swap(swap) => {
                    if let Ok(idx) = policy.discount_index(swap.fixed_leg()) {
                        set.insert(idx);
                    }
                    set.insert(swap.forward_index());
                }
                CalibrationInstrumentType::BasisSwap(basis) => {
                    if let Ok(idx) = policy.discount_index(basis.pay_leg()) {
                        set.insert(idx);
                    }
                    set.insert(basis.pay_forward_index());
                    set.insert(basis.receive_forward_index());
                }
                CalibrationInstrumentType::FixFloatCrossCurrencySwap(xccy) => {
                    if let Ok(idx) = policy.discount_index(xccy.domestic_leg()) {
                        set.insert(idx);
                    }
                    if let Ok(idx) = policy.discount_index(xccy.foreign_leg()) {
                        set.insert(idx);
                    }
                    set.insert(xccy.forward_index());
                }
                CalibrationInstrumentType::FloatFloatCrossCurrencySwap(xccy) => {
                    if let Ok(idx) = policy.discount_index(xccy.domestic_leg()) {
                        set.insert(idx);
                    }
                    if let Ok(idx) = policy.discount_index(xccy.foreign_leg()) {
                        set.insert(idx);
                    }
                    set.insert(xccy.domestic_forward_index());
                    set.insert(xccy.foreign_forward_index());
                }
                CalibrationInstrumentType::FxForward(fwd) => {
                    if let Ok(idx) = policy.discount_index_for_currency(fwd.base_currency()) {
                        set.insert(idx);
                    }
                    if let Ok(idx) = policy.discount_index_for_currency(fwd.quote_currency()) {
                        set.insert(idx);
                    }
                }
                CalibrationInstrumentType::RateFutures(rf) => {
                    set.insert(rf.market_index());
                }
                _ => {}
            }
        }
        Ok(set)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{
        currencies::currency::Currency,
        indices::marketindex::MarketIndex,
        math::interpolation::interpolator::Interpolator,
        quotes::{
            quote::{Level, Quote, QuoteDetails, QuoteLevels},
            quoteselector::QuoteSelector,
        },
        rates::bootstrapping::bootstrapdiscountpolicy::BootstrapDiscountPolicy,
        time::{date::Date, daycounter::DayCounter},
        utils::errors::Result,
    };

    use super::CurveConfiguration;

    struct MapSelector {
        reference_date: Date,
        quotes: HashMap<String, f64>,
    }

    impl MapSelector {
        fn new(reference_date: Date) -> Self {
            Self {
                reference_date,
                quotes: HashMap::new(),
            }
        }
        fn add(&mut self, id: &str, rate: f64) {
            self.quotes.insert(id.to_string(), rate);
        }
    }

    impl QuoteSelector for MapSelector {
        fn select(&self, identifier: &str) -> Option<Quote> {
            let rate = self.quotes.get(identifier)?;
            let det: QuoteDetails = identifier.parse().ok()?;
            let q = Quote::new(det, QuoteLevels::with_mid(*rate));
            if q.build_instrument(self.reference_date, Level::Mid, None)
                .is_ok()
            {
                Some(q)
            } else {
                None
            }
        }
        fn reference_date(&self) -> Date {
            self.reference_date
        }
    }

    fn make_selector(quotes: &[(&str, f64)]) -> MapSelector {
        let mut sel = MapSelector::new(Date::new(2024, 1, 2));
        for (id, rate) in quotes {
            sel.add(id, *rate);
        }
        sel
    }

    fn resolve_config(
        index: MarketIndex,
        quote_ids: Vec<String>,
        selector: &MapSelector,
    ) -> Result<CurveConfiguration> {
        let mut cfg = CurveConfiguration::new(
            index,
            DayCounter::Actual360,
            Interpolator::Linear,
            true,
            quote_ids,
        );
        cfg.resolve(selector, Level::Mid, None)?;
        Ok(cfg)
    }

    #[test]
    fn dependencies_unresolved_errors() {
        let cfg = CurveConfiguration::new(
            MarketIndex::SOFR,
            DayCounter::Actual360,
            Interpolator::Linear,
            true,
            vec!["OIS_USD_SOFR_1Y".into()],
        );
        let policy = BootstrapDiscountPolicy::new(MarketIndex::SOFR, Currency::USD);
        assert!(cfg.dependencies(&policy).is_err());
    }

    #[test]
    fn dependencies_empty_instruments_errors() -> Result<()> {
        let selector = make_selector(&[]);
        let cfg = resolve_config(MarketIndex::SOFR, vec![], &selector)?;
        let policy = BootstrapDiscountPolicy::new(MarketIndex::SOFR, Currency::USD);
        assert!(cfg.dependencies(&policy).is_err());
        Ok(())
    }

    #[test]
    fn dependencies_deposit() -> Result<()> {
        let selector = make_selector(&[("FixedRateDeposit_USD_SOFR_6M", 0.05)]);
        let cfg = resolve_config(
            MarketIndex::SOFR,
            vec!["FixedRateDeposit_USD_SOFR_6M".into()],
            &selector,
        )?;
        let policy = BootstrapDiscountPolicy::new(MarketIndex::SOFR, Currency::USD);
        let deps = cfg.dependencies(&policy)?;

        assert!(deps.contains(&MarketIndex::SOFR));
        Ok(())
    }

    #[test]
    fn dependencies_ois_swap() -> Result<()> {
        let selector = make_selector(&[("OIS_USD_SOFR_1Y", 0.05)]);
        let cfg = resolve_config(MarketIndex::SOFR, vec!["OIS_USD_SOFR_1Y".into()], &selector)?;
        let policy = BootstrapDiscountPolicy::new(MarketIndex::SOFR, Currency::USD);
        let deps = cfg.dependencies(&policy)?;

        // OIS swap: self index + forward index + discount index
        assert!(deps.contains(&MarketIndex::SOFR));
        Ok(())
    }

    #[test]
    fn dependencies_ois_swap_cross_currency_discount() -> Result<()> {
        let selector = make_selector(&[("OIS_EUR_EURIBOR1m_1Y", 0.03)]);
        let cfg = resolve_config(
            MarketIndex::EURIBOR1m,
            vec!["OIS_EUR_EURIBOR1m_1Y".into()],
            &selector,
        )?;
        // CSA in USD, so EUR leg discount → Collateral(EUR, USD)
        let policy = BootstrapDiscountPolicy::new(MarketIndex::SOFR, Currency::USD);
        let deps = cfg.dependencies(&policy)?;

        assert!(deps.contains(&MarketIndex::EURIBOR1m));
        assert!(deps.contains(&MarketIndex::Collateral(Currency::EUR, Currency::USD)));
        Ok(())
    }

    #[test]
    fn dependencies_basis_swap() -> Result<()> {
        let selector = make_selector(&[("BasisSwap_USD_SOFR_TermSOFR3m_1Y", 0.001)]);
        let cfg = resolve_config(
            MarketIndex::TermSOFR3m,
            vec!["BasisSwap_USD_SOFR_TermSOFR3m_1Y".into()],
            &selector,
        )?;
        let policy = BootstrapDiscountPolicy::new(MarketIndex::SOFR, Currency::USD);
        let deps = cfg.dependencies(&policy)?;

        assert!(deps.contains(&MarketIndex::TermSOFR3m));
        assert!(deps.contains(&MarketIndex::SOFR));
        Ok(())
    }

    #[test]
    fn dependencies_fx_forward() -> Result<()> {
        let selector = make_selector(&[("FxForwardPoints_EURUSD_1M", 0.001)]);
        let cfg = resolve_config(
            MarketIndex::Collateral(Currency::EUR, Currency::USD),
            vec!["FxForwardPoints_EURUSD_1M".into()],
            &selector,
        )?;
        let policy = BootstrapDiscountPolicy::new(MarketIndex::SOFR, Currency::USD);
        let deps = cfg.dependencies(&policy)?;

        // FxForward: discount indices for both base and quote currencies
        assert!(deps.contains(&MarketIndex::Collateral(Currency::EUR, Currency::USD)));
        assert!(deps.contains(&MarketIndex::SOFR)); // USD discount
        Ok(())
    }

    #[test]
    fn dependencies_rate_futures() -> Result<()> {
        let selector = make_selector(&[("Future_USD_SOFR_H5", 95.0)]);
        let cfg = resolve_config(
            MarketIndex::SOFR,
            vec!["Future_USD_SOFR_H5".into()],
            &selector,
        )?;
        let policy = BootstrapDiscountPolicy::new(MarketIndex::SOFR, Currency::USD);
        let deps = cfg.dependencies(&policy)?;

        assert!(deps.contains(&MarketIndex::SOFR));
        Ok(())
    }

    #[test]
    fn dependencies_multiple_instruments() -> Result<()> {
        let selector = make_selector(&[
            ("FixedRateDeposit_USD_SOFR_3M", 0.04),
            ("OIS_USD_SOFR_1Y", 0.05),
            ("OIS_USD_SOFR_2Y", 0.05),
        ]);
        let cfg = resolve_config(
            MarketIndex::SOFR,
            vec![
                "FixedRateDeposit_USD_SOFR_3M".into(),
                "OIS_USD_SOFR_1Y".into(),
                "OIS_USD_SOFR_2Y".into(),
            ],
            &selector,
        )?;
        let policy = BootstrapDiscountPolicy::new(MarketIndex::SOFR, Currency::USD);
        let deps = cfg.dependencies(&policy)?;

        assert!(deps.contains(&MarketIndex::SOFR));
        // All instruments are USD/SOFR, so only SOFR should appear
        assert_eq!(deps.len(), 1);
        Ok(())
    }
}
