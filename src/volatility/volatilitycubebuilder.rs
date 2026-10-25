use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::{
    ad::dual::DualFwd,
    core::elements::volatilitycubelement::VolatilityCubeElement,
    indices::marketindex::MarketIndex,
    quotes::{quote::Level, quoteselector::QuoteSelector},
    time::{date::Date, period::Period},
    utils::errors::{QSError, Result},
    volatility::{
        interpolatedvolatilitycube::InterpolatedVolatilityCube,
        volatilitycubeconfiguration::VolatilityCubeConfiguration, volatilityindexing::F64Key,
    },
};
use std::collections::{BTreeMap, BTreeSet};

/// Stateless builder that constructs [`InterpolatedVolatilityCube`]
/// instances from [`VolatilityCubeConfiguration`] specs and a quote store.
///
/// Cubes are typically used for swaption volatilities.
/// Node keys use `strike.resolve(0.0)`; smile metadata does not perform
/// forward-dependent strike, delta, or log-moneyness conversion.
pub struct VolatilityCubeBuilder {
    specs: Vec<VolatilityCubeConfiguration>,
}

impl VolatilityCubeBuilder {
    /// Creates a new builder from a list of cube specifications.
    #[must_use]
    pub const fn new(specs: Vec<VolatilityCubeConfiguration>) -> Self {
        Self { specs }
    }

    /// Builds all configured cubes from the given quote source.
    ///
    /// # Errors
    /// Returns an error when a quote is missing, belongs to another market
    /// index, lacks an expiry, tenor, or strike, or duplicates an
    /// expiry-tenor-strike node. A second configuration for the same market
    /// index also produces an error. Empty quote lists and invalid node
    /// coordinates or volatilities are rejected.
    pub fn build(
        &self,
        selector: &impl QuoteSelector,
        level: Level,
    ) -> Result<HashMap<MarketIndex, VolatilityCubeElement>> {
        let reference_date = selector.reference_date();
        let mut cubes = HashMap::new();

        for spec in &self.specs {
            let (cube, labels) = self
                .build_one(spec, selector, level, reference_date)
                .map_err(|error| {
                    error.context(format!("building volatility cube {}", spec.market_index()))
                })?;
            let cube = cube
                .with_labels(&labels)
                .with_calibration_instrument_ids(&labels);
            let element = VolatilityCubeElement::new(
                spec.market_index().clone(),
                Rc::new(RefCell::new(cube)),
            );
            if cubes.insert(spec.market_index().clone(), element).is_some() {
                return Err(QSError::InvalidValueErr(format!(
                    "Duplicate volatility cube configuration for index {}",
                    spec.market_index()
                )));
            }
        }

        Ok(cubes)
    }

    /// Builds one volatility cube from its configured expiry, tenor and smile quotes.
    #[allow(clippy::unused_self)]
    fn build_one(
        &self,
        spec: &VolatilityCubeConfiguration,
        selector: &impl QuoteSelector,
        level: Level,
        reference_date: Date,
    ) -> Result<(InterpolatedVolatilityCube<DualFwd>, Vec<String>)> {
        if spec.quotes().is_empty() {
            return Err(QSError::InvalidValueErr(format!(
                "Volatility cube for {} requires at least one quote",
                spec.market_index(),
            )));
        }
        let mut coordinates = BTreeSet::new();
        let mut points: BTreeMap<Period, BTreeMap<Period, BTreeMap<F64Key, DualFwd>>> =
            BTreeMap::new();
        let mut node_labels: BTreeMap<Period, BTreeMap<Period, BTreeMap<F64Key, String>>> =
            BTreeMap::new();

        for qid in spec.quotes() {
            let quote = selector
                .select(qid)
                .ok_or_else(|| QSError::NotFoundErr(format!("Quote not found: {qid}")))?;
            let val = quote.levels().value(level).map_err(|error| {
                error.context(format!("reading {level:?} volatility quote '{qid}'"))
            })?;
            let details = quote.details();
            if details.market_index() != Some(spec.market_index()) {
                return Err(QSError::InvalidValueErr(format!(
                    "Quote {qid} belongs to {:?}, expected cube index {}",
                    details.market_index(),
                    spec.market_index()
                )));
            }

            let expiry = details.option_expiry().ok_or_else(|| {
                QSError::InvalidValueErr(format!("Quote {qid} missing option_expiry"))
            })?;
            let tenor = details
                .tenor()
                .ok_or_else(|| QSError::InvalidValueErr(format!("Quote {qid} missing tenor")))?;
            let strike = details
                .strike()
                .ok_or_else(|| QSError::InvalidValueErr(format!("Quote {qid} missing strike")))?;

            let coordinate = strike.resolve(0.0);
            let key = F64Key::new(if coordinate == 0.0 { 0.0 } else { coordinate });
            if !val.is_finite()
                || val < 0.0
                || !key.value().is_finite()
                || expiry.period_in_year() <= 0.0
                || tenor.period_in_year() <= 0.0
            {
                return Err(QSError::InvalidValueErr(format!(
                    "Invalid volatility cube node {qid}: require positive periods, a finite key, and finite non-negative volatility"
                )));
            }
            if !coordinates.insert((
                F64Key::new(expiry.period_in_year()),
                F64Key::new(tenor.period_in_year()),
                key.clone(),
            )) {
                return Err(QSError::InvalidValueErr(format!(
                    "Duplicate volatility cube coordinates for quote {qid}"
                )));
            }
            node_labels
                .entry(expiry)
                .or_default()
                .entry(tenor)
                .or_default()
                .insert(key.clone(), qid.clone());
            points
                .entry(expiry)
                .or_default()
                .entry(tenor)
                .or_default()
                .insert(key, DualFwd::from(val));
        }

        // Keep labels in exactly the same canonical coordinate order used by
        // `InterpolatedVolatilityCube::pillars`.
        let labels = node_labels
            .values()
            .flat_map(|tenors| tenors.values().flat_map(|smile| smile.values().cloned()))
            .collect();

        let cube = InterpolatedVolatilityCube::new(
            reference_date,
            spec.market_index().clone(),
            points,
            spec.volatility_type().clone(),
            spec.smile_type(),
        )
        .with_extrapolation_policy(spec.extrapolation_policy());

        Ok((cube, labels))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::{
        ExtrapolationPolicy, Quote, QuoteLevels, QuoteStore, SmileType, TimeUnit, VolatilityType,
    };

    const QUOTE: &str = "Swaption_USD_SOFR_1Y_5Y_Absolute_0.03_Black";

    fn build(
        nodes: &[(&str, f64)],
        policy: ExtrapolationPolicy,
    ) -> Result<HashMap<MarketIndex, VolatilityCubeElement>> {
        let mut store = QuoteStore::new(Date::new(2025, 1, 1));
        for &(identifier, value) in nodes {
            store.add_quote(Quote::new(
                identifier.parse()?,
                QuoteLevels::with_mid(value),
            ));
        }
        let configuration = VolatilityCubeConfiguration::new(
            MarketIndex::SOFR,
            VolatilityType::Black,
            SmileType::Strike,
            nodes
                .iter()
                .map(|&(identifier, _)| identifier.to_string())
                .collect(),
        )
        .with_extrapolation_policy(policy);
        VolatilityCubeBuilder::new(vec![configuration]).build(&store, Level::Mid)
    }

    #[test]
    fn builder_supports_single_quote_and_propagates_extrapolation_policy() -> Result<()> {
        for policy in [ExtrapolationPolicy::Reject, ExtrapolationPolicy::Flat] {
            let elements = build(&[(QUOTE, 0.20)], policy)?;
            let element = elements.get(&MarketIndex::SOFR).expect("built cube").cube();
            assert!(
                (element
                    .volatility_from_period(
                        Period::new(1, TimeUnit::Years),
                        Period::new(5, TimeUnit::Years),
                        0.03
                    )?
                    .value()
                    - 0.20)
                    .abs()
                    < 1e-12
            );
            let query = element.volatility_from_period(
                Period::new(2, TimeUnit::Years),
                Period::new(10, TimeUnit::Years),
                0.04,
            );
            if policy == ExtrapolationPolicy::Reject {
                assert!(query.is_err());
            } else {
                assert!((query?.value() - 0.20).abs() < 1e-12);
            }
        }
        Ok(())
    }

    #[test]
    fn builder_rejects_empty_invalid_and_aliased_nodes() {
        for policy in [ExtrapolationPolicy::Reject, ExtrapolationPolicy::Flat] {
            assert!(build(&[], policy).is_err());
            for value in [-0.01, f64::NAN, f64::INFINITY] {
                assert!(build(&[(QUOTE, value)], policy).is_err());
            }
            assert!(build(
                &[("Swaption_USD_SOFR_1Y_0Y_Absolute_0.03_Black", 0.20)],
                policy
            )
            .is_err());
            for nodes in [
                vec![
                    (QUOTE, 0.20),
                    ("Swaption_USD_SOFR_12M_60M_Absolute_0.03_Black", 0.30),
                ],
                vec![
                    ("Swaption_USD_SOFR_1Y_5Y_Absolute_0_Black", 0.20),
                    ("Swaption_USD_SOFR_1Y_5Y_Absolute_-0_Black", 0.30),
                ],
            ] {
                let error = build(&nodes, policy)
                    .err()
                    .expect("aliased coordinates must fail");
                assert!(error.to_string().contains("Duplicate"));
            }
        }
    }
}
