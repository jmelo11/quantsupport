use crate::{
    ad::dual::DualFwd,
    core::elements::curveelement::ADCurveElement,
    quotes::quote::{Level, Quote, QuoteInstrument},
    rates::{
        compounding::Compounding,
        interestrate::RateDefinition,
        yieldtermstructure::{
            discounttermstructure::DiscountTermStructure,
            zeroratetermstructure::ZeroRateTermStructure,
        },
    },
    time::{date::Date, enums::Frequency},
    utils::errors::{QSError, Result},
};

use super::{bootstrappedcurve::BootstrappedCurve, curveconfiguration::CurveConfiguration};

/// Resolved direct market observations, sorted together with their original labels.
#[derive(Clone)]
pub struct QuotedCurve {
    dates: Vec<Date>,
    values: Vec<f64>,
    labels: Vec<String>,
    rate_definition: Option<RateDefinition>,
}

impl QuotedCurve {
    /// Resolves discount-factor or zero-rate pillar quotes into a direct-quote curve definition.
    pub fn resolve(
        quotes: &[Quote],
        reference_date: Date,
        level: Level,
        spec: &CurveConfiguration,
    ) -> Result<Option<Self>> {
        let direct = |quote: &Quote| {
            matches!(
                quote.details().instrument(),
                QuoteInstrument::DiscountFactor | QuoteInstrument::ZeroRate
            )
        };
        if !quotes.iter().any(direct) {
            return Ok(None);
        }
        let first = quotes[0].details();
        let convention = |quote: &Quote| {
            let details = quote.details();
            (details.instrument() == &QuoteInstrument::ZeroRate).then(|| {
                RateDefinition::new(
                    details.day_counter().unwrap_or_else(|| spec.day_counter()),
                    details.compounding().unwrap_or(Compounding::Continuous),
                    details.frequency().unwrap_or(Frequency::Annual),
                )
            })
        };
        let rate_definition = convention(&quotes[0]);
        let mut nodes = Vec::with_capacity(quotes.len());
        for quote in quotes {
            let details = quote.details();
            let label = details.identifier();
            if !direct(quote)
                || details.instrument() != first.instrument()
                || convention(quote) != rate_definition
                || details.currency() != first.currency()
            {
                return Err(QSError::InvalidValueErr(format!(
                    "Curve {} requires one direct quote family with matching currency and rate conventions",
                    spec.market_index()
                )));
            }
            if details.market_index() != Some(spec.market_index()) {
                return Err(QSError::InvalidValueErr(format!(
                    "Direct quote '{label}' does not match curve index {}",
                    spec.market_index()
                )));
            }
            let date = details
                .maturity()
                .or_else(|| details.tenor().map(|tenor| reference_date + tenor))
                .ok_or_else(|| QSError::ValueNotSetErr(format!("Pillar for quote '{label}'")))?;
            let value = quote.levels().value(level).map_err(|error| {
                error.context(format!("reading {level:?} direct curve quote '{label}'"))
            })?;
            if !value.is_finite() || (rate_definition.is_none() && value <= 0.0) {
                return Err(QSError::InvalidValueErr(format!(
                    "Direct quote '{label}' must be finite; discount factors must be positive"
                )));
            }
            if rate_definition.is_none() && date == reference_date && (value - 1.0).abs() > 1e-12 {
                return Err(QSError::InvalidValueErr(format!(
                    "Reference-date discount factor '{label}' must equal 1"
                )));
            }
            nodes.push((date, value, label));
        }
        nodes.sort_by_key(|node| node.0);
        if nodes[0].0 < reference_date || nodes.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
            return Err(QSError::InvalidValueErr(
                "Direct curve pillars must be distinct dates on or after the reference date".into(),
            ));
        }
        let times: Vec<_> = nodes
            .iter()
            .map(|node| spec.day_counter().year_fraction(reference_date, node.0))
            .collect();
        if times.windows(2).any(|pair| pair[1] <= pair[0])
            || (rate_definition.is_none() && !times.iter().any(|time| *time > 0.0))
        {
            return Err(QSError::InvalidValueErr(
                "Direct curve pillar times must increase and discount curves need a future pillar"
                    .into(),
            ));
        }
        let mut result = Self {
            dates: Vec::with_capacity(nodes.len()),
            values: Vec::with_capacity(nodes.len()),
            labels: Vec::with_capacity(nodes.len()),
            rate_definition,
        };
        for (date, value, label) in nodes {
            result.dates.push(date);
            result.values.push(value);
            result.labels.push(label);
        }
        Ok(Some(result))
    }

    /// Constructs a zero-rate or discount-factor term structure from the resolved pillar values.
    pub fn construct(
        &self,
        spec: &CurveConfiguration,
    ) -> Result<(Box<dyn ADCurveElement>, BootstrappedCurve)> {
        let reference_date = spec.reference_date()?;
        if let Some(definition) = self.rate_definition {
            let curve = ZeroRateTermStructure::new(
                reference_date,
                self.dates.clone(),
                self.values.clone(),
                definition,
                spec.interpolator(),
                spec.enable_extrapolation(),
            )?
            .with_pillar_labels(self.labels.clone())?;
            let snapshot =
                BootstrappedCurve::from_zero_rates(spec.market_index().clone(), curve.clone())?;
            return Ok((Box::new(curve.to_dual()), snapshot));
        }

        // The origin is fixed at one; every future node is its own market input.
        // Keep an explicitly quoted origin as a source pillar with zero sensitivity.
        let mut dates = vec![reference_date];
        let mut factors = vec![1.0];
        let mut sensitivities = Vec::new();
        for (column, (&date, &value)) in self.dates.iter().zip(&self.values).enumerate() {
            if date == reference_date {
                continue;
            }
            dates.push(date);
            factors.push(value);
            let mut row = vec![0.0; self.values.len()];
            row[column] = 1.0;
            sensitivities.push(row);
        }
        let values: Vec<_> = self
            .values
            .iter()
            .map(|value| DualFwd::new(*value))
            .collect();
        let curve = DiscountTermStructure::<DualFwd>::new(
            dates.clone(),
            factors.iter().map(|value| DualFwd::new(*value)).collect(),
            spec.day_counter(),
            spec.interpolator(),
            spec.enable_extrapolation(),
        )?
        .with_pillar_values(values.clone())?
        .with_pillar_labels(self.labels.clone())?
        .with_ift_sensitivities(sensitivities.clone());
        let snapshot = BootstrappedCurve::new(
            spec.market_index().clone(),
            reference_date,
            dates
                .iter()
                .map(|date| spec.day_counter().year_fraction(reference_date, *date))
                .collect(),
            factors,
            spec.day_counter(),
            spec.interpolator(),
        )
        .with_pillar_values(values)
        .with_pillar_labels(self.labels.clone())
        .with_ift_sensitivities(sensitivities);
        Ok((Box::new(curve), snapshot))
    }
}

#[cfg(test)]
mod tests {
    use crate::prelude::*;

    fn inputs(observations: &[(&str, f64)]) -> Result<(QuoteStore, CurveConfiguration)> {
        let mut store = QuoteStore::new(Date::new(2026, 2, 24));
        for (label, value) in observations {
            store.add_quote(Quote::new(label.parse()?, QuoteLevels::with_mid(*value)));
        }
        let spec = CurveConfiguration::new(
            MarketIndex::ESTR,
            DayCounter::Actual365,
            Interpolator::Linear,
            false,
            observations
                .iter()
                .map(|(label, _)| (*label).into())
                .collect(),
        );
        Ok((store, spec))
    }

    #[test]
    fn direct_quotes_match_builtin_indices_in_any_case() -> Result<()> {
        let reference_date = Date::new(2026, 2, 24);
        let pillar_date = Date::new(2026, 3, 3);
        for (label, value, expected) in [
            ("DISCOUNTFACTOR_EUR_EURIBOR3M_1W", 0.999, 0.999),
            ("discountfactor_eur_euribor3m_1w", 0.999, 0.999),
            ("DiscountFactor_EUR_EuRiBoR3m_1W", 0.999, 0.999),
            (
                "ZERORATE_EUR_EURIBOR3M_1W",
                0.04,
                (-0.04_f64 * 7.0 / 365.0).exp(),
            ),
            (
                "zerorate_eur_euribor3m_1w",
                0.04,
                (-0.04_f64 * 7.0 / 365.0).exp(),
            ),
            (
                "ZeroRate_EUR_EuRiBoR3m_1W",
                0.04,
                (-0.04_f64 * 7.0 / 365.0).exp(),
            ),
        ] {
            let mut store = QuoteStore::new(reference_date);
            store.add_quote(Quote::new(label.parse()?, QuoteLevels::with_mid(value)));
            let spec = CurveConfiguration::new(
                MarketIndex::EURIBOR3m,
                DayCounter::Actual365,
                Interpolator::Linear,
                false,
                vec![label.into()],
            );
            let policy = BootstrapDiscountPolicy::new(MarketIndex::EURIBOR3m, Currency::EUR);
            let (curves, snapshots) = MultiCurveBootstrapper::new(vec![spec], policy)
                .bootstrap_with_snapshots(&store, Level::Mid)?;
            let factor = curves[&MarketIndex::EURIBOR3m]
                .curve()
                .discount_factor(pillar_date)?
                .value();
            assert!((factor - expected).abs() < 1e-12, "{label}");
            assert_eq!(
                snapshots[&MarketIndex::EURIBOR3m].pillar_labels().unwrap(),
                &[label]
            );
        }
        Ok(())
    }

    #[test]
    fn direct_quotes_preserve_labels_and_parent_bump_coordinates() -> Result<()> {
        for (observations, expected, bumped) in [
            (
                [
                    ("DiscountFactor_EUR_ESTR_2Y", 0.90),
                    ("DiscountFactor_EUR_ESTR_1Y", 0.96),
                ],
                0.96,
                0.97,
            ),
            (
                [
                    ("ZeroRate_EUR_ESTR_2Y", 0.05),
                    ("ZeroRate_EUR_ESTR_1Y", 0.04),
                ],
                (-0.04_f64).exp(),
                (-0.05_f64).exp(),
            ),
        ] {
            let (store, spec) = inputs(&observations)?;
            let policy = BootstrapDiscountPolicy::new(MarketIndex::ESTR, Currency::EUR);
            let (curves, mut snapshots) = MultiCurveBootstrapper::new(vec![spec], policy)
                .bootstrap_with_snapshots(&store, Level::Mid)?;
            let date = Date::new(2027, 2, 24);
            assert!(
                (curves[&MarketIndex::ESTR]
                    .curve()
                    .discount_factor(date)?
                    .value()
                    - expected)
                    .abs()
                    < 1e-12
            );
            let snapshot = snapshots.get_mut(&MarketIndex::ESTR).unwrap();
            assert_eq!(snapshot.pillar_labels().unwrap()[0], observations[1].0);
            snapshot.bump_pillar(0, 0.01)?;
            assert!((snapshot.discount_factor(date)? - bumped).abs() < 1e-12);
        }
        Ok(())
    }

    #[test]
    fn direct_quotes_reject_nonfinite_inputs_and_bad_identifiers() -> Result<()> {
        for (label, value) in [
            ("DiscountFactor_EUR_ESTR_1Y", f64::NAN),
            ("ZeroRate_EUR_ESTR_1Y", f64::INFINITY),
        ] {
            let (store, mut spec) = inputs(&[(label, value)])?;
            assert!(spec.resolve(&store, Level::Mid, None).is_err());
        }
        for label in [
            "DiscountFactor_EUR_ESTR",
            "DiscountFactor_EUR_ESTR_1Y_extra",
            "ZeroRate_EUR_ESTR_invalid",
            "ZeroRate_EUR_ESTR_1Y_invalid",
            "ZeroRate_EUR_ESTR_1Y_Continuous_invalid",
        ] {
            assert!(label.parse::<QuoteDetails>().is_err(), "{label}");
        }
        Ok(())
    }

    #[test]
    fn direct_quote_conventions_survive_serialization() -> Result<()> {
        let label = "ZeroRate_EUR_ESTR_2027-02-24_Compounded_Semiannual";
        let (store, spec) = inputs(&[(label, 0.04)])?;
        let store: QuoteStore =
            serde_json::from_str(&serde_json::to_string(&store).unwrap()).unwrap();
        let spec: CurveConfiguration =
            serde_json::from_str(&serde_json::to_string(&spec).unwrap()).unwrap();
        let quote = store.quote(label).unwrap();
        assert_eq!(quote.details().compounding(), Some(Compounding::Compounded));
        assert_eq!(quote.details().frequency(), Some(Frequency::Semiannual));
        let policy = BootstrapDiscountPolicy::new(MarketIndex::ESTR, Currency::EUR);
        let curves =
            MultiCurveBootstrapper::new(vec![spec], policy).bootstrap(&store, Level::Mid)?;
        let factor = curves[&MarketIndex::ESTR]
            .curve()
            .discount_factor(Date::new(2027, 2, 24))?
            .value();
        assert!((factor - 1.02_f64.powi(-2)).abs() < 1e-12);
        Ok(())
    }
}
