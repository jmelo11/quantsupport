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
    pub fn new(specs: Vec<VolatilityCubeConfiguration>) -> Self {
        Self { specs }
    }

    /// Builds all configured cubes from the given quote source.
    ///
    /// # Errors
    /// Returns an error if a required quote is missing from the selector or
    /// if the quote details lack the expected fields.
    pub fn build(
        &self,
        selector: &impl QuoteSelector,
        level: Level,
    ) -> Result<HashMap<MarketIndex, VolatilityCubeElement>> {
        let reference_date = selector.reference_date();
        let mut cubes = HashMap::new();

        for spec in &self.specs {
            let (cube, labels) = self.build_one(spec, selector, level, reference_date)?;
            let element = VolatilityCubeElement::new(
                spec.market_index().clone(),
                Rc::new(RefCell::new(cube.with_labels(&labels))),
            );
            cubes.insert(spec.market_index().clone(), element);
        }

        Ok(cubes)
    }

    fn build_one(
        &self,
        spec: &VolatilityCubeConfiguration,
        selector: &impl QuoteSelector,
        level: Level,
        reference_date: Date,
    ) -> Result<(InterpolatedVolatilityCube<DualFwd>, Vec<String>)> {
        let mut points: BTreeMap<Period, BTreeMap<Period, BTreeMap<F64Key, DualFwd>>> =
            BTreeMap::new();
        let mut labels = Vec::new();

        for qid in spec.quotes() {
            let quote = selector
                .select(qid)
                .ok_or_else(|| QSError::NotFoundErr(format!("Quote not found: {qid}")))?;
            let val = quote.levels().value(level)?;
            let details = quote.details();

            let expiry = details.option_expiry().ok_or_else(|| {
                QSError::InvalidValueErr(format!("Quote {qid} missing option_expiry"))
            })?;
            let tenor = details
                .tenor()
                .ok_or_else(|| QSError::InvalidValueErr(format!("Quote {qid} missing tenor")))?;
            let strike = details
                .strike()
                .ok_or_else(|| QSError::InvalidValueErr(format!("Quote {qid} missing strike")))?;

            points
                .entry(expiry)
                .or_default()
                .entry(tenor)
                .or_default()
                .insert(F64Key::new(strike.resolve(0.0)), DualFwd::from(val));
            labels.push(qid.clone());
        }

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
