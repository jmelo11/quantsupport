use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::{
    ad::dual::DualFwd,
    core::elements::volatilitysurfaceelement::VolatilitySurfaceElement,
    indices::marketindex::MarketIndex,
    quotes::{quote::Level, quoteselector::QuoteSelector},
    time::{date::Date, period::Period},
    utils::errors::{QSError, Result},
    volatility::{
        interpolatedvolatilitysurface::InterpolatedVolatilitySurface, volatilityindexing::F64Key,
        volatilitysurfaceconfiguration::VolatilitySurfaceConfiguration,
    },
};
use std::collections::BTreeMap;

/// Stateless builder that constructs [`InterpolatedVolatilitySurface`]
/// instances from [`VolatilitySurfaceConfiguration`] specs and a quote store.
pub struct VolatilitySurfaceBuilder {
    specs: Vec<VolatilitySurfaceConfiguration>,
}

impl VolatilitySurfaceBuilder {
    /// Creates a new builder from a list of surface specifications.
    #[must_use]
    pub const fn new(specs: Vec<VolatilitySurfaceConfiguration>) -> Self {
        Self { specs }
    }

    /// Builds all configured surfaces from the given quote source.
    ///
    /// # Errors
    /// Returns an error if a required quote is missing from the selector or
    /// if the quote details lack the expected fields.
    pub fn build(
        &self,
        selector: &impl QuoteSelector,
        level: Level,
    ) -> Result<HashMap<MarketIndex, VolatilitySurfaceElement>> {
        let reference_date = selector.reference_date();
        let mut surfaces = HashMap::new();

        for spec in &self.specs {
            let (surface, labels) = self.build_one(spec, selector, level, reference_date)?;
            let element = VolatilitySurfaceElement::new(
                spec.market_index().clone(),
                Rc::new(RefCell::new(surface.with_labels(&labels))),
            );
            surfaces.insert(spec.market_index().clone(), element);
        }

        Ok(surfaces)
    }

    #[allow(clippy::unused_self)]
    fn build_one(
        &self,
        spec: &VolatilitySurfaceConfiguration,
        selector: &impl QuoteSelector,
        level: Level,
        reference_date: Date,
    ) -> Result<(InterpolatedVolatilitySurface<DualFwd>, Vec<String>)> {
        let mut points: BTreeMap<Period, BTreeMap<F64Key, DualFwd>> = BTreeMap::new();
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
            let strike = details
                .strike()
                .ok_or_else(|| QSError::InvalidValueErr(format!("Quote {qid} missing strike")))?;

            points
                .entry(expiry)
                .or_default()
                .insert(F64Key::new(strike.resolve(0.0)), DualFwd::from(val));
            labels.push(qid.clone());
        }

        let surface = InterpolatedVolatilitySurface::new(
            reference_date,
            spec.market_index().clone(),
            points,
            spec.volatility_type().clone(),
            spec.smile_type(),
        );

        Ok((surface, labels))
    }
}
