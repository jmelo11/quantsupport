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
use std::collections::BTreeMap;

/// Stateless builder that constructs [`InterpolatedVolatilityCube`]
/// instances from [`VolatilityCubeConfiguration`] specs and a quote store.
/// Cubes are typically used for swaption volatilities.
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
    /// index also produces an error.
    pub fn build(
        &self,
        selector: &impl QuoteSelector,
        level: Level,
    ) -> Result<HashMap<MarketIndex, VolatilityCubeElement>> {
        let reference_date = selector.reference_date();
        let mut cubes = HashMap::new();

        for spec in &self.specs {
            let (cube, labels) = self.build_one(spec, selector, level, reference_date)?;
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

    #[allow(clippy::unused_self)]
    fn build_one(
        &self,
        spec: &VolatilityCubeConfiguration,
        selector: &impl QuoteSelector,
        level: Level,
        reference_date: Date,
    ) -> Result<(InterpolatedVolatilityCube<DualFwd>, Vec<String>)> {
        let mut points: BTreeMap<Period, BTreeMap<Period, BTreeMap<F64Key, DualFwd>>> =
            BTreeMap::new();
        let mut node_labels: BTreeMap<Period, BTreeMap<Period, BTreeMap<F64Key, String>>> =
            BTreeMap::new();

        for qid in spec.quotes() {
            let quote = selector
                .select(qid)
                .ok_or_else(|| QSError::NotFoundErr(format!("Quote not found: {qid}")))?;
            let val = quote.levels().value(level)?;
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

            let key = F64Key::new(strike.resolve(0.0));
            if node_labels
                .entry(expiry)
                .or_default()
                .entry(tenor)
                .or_default()
                .insert(key.clone(), qid.clone())
                .is_some()
            {
                return Err(QSError::InvalidValueErr(format!(
                    "Duplicate volatility cube node at expiry {expiry}, tenor {tenor}, key {}",
                    key.value()
                )));
            }
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
        );

        Ok((cube, labels))
    }
}
