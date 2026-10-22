//! Borrowed market inputs with independent constructed objects for a scenario.

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

use crate::{
    core::{
        elements::{
            curveelement::{CreditCurveElement, DiscountCurveElement, DividendCurveElement},
            volatilitycubelement::VolatilityCubeElement,
            volatilitysurfaceelement::VolatilitySurfaceElement,
        },
        marketdatahandling::{
            constructedelementrequest::ConstructedElementRequest,
            constructedelementstore::ConstructedElementStore,
            marketdata::{MarketData, MarketDataProvider, MarketDataRequest},
        },
        pricingcontext::PricingContext,
    },
    indices::marketindex::MarketIndex,
    models::modelconfiguration::{ModelConfiguration, ParameterSource},
    quotes::{quote::Quote, quoteselector::QuoteSelector, scenario::Scenario},
    rates::bootstrapping::{
        bootstrapdiscountpolicy::BootstrapDiscountPolicy, bootstrappedcurve::BootstrappedCurve,
        creditcurvebootstrapper::CreditCurveBootstrapper,
        multicurvebootstrapper::MultiCurveBootstrapper,
    },
    simulations::simulationbuilder::SimulationBuilder,
    time::date::Date,
    utils::errors::{QSError, Result},
    volatility::{
        modelcalibration::{CalibrationSource, ModelCalibrationConfiguration},
        volatilitycubebuilder::VolatilityCubeBuilder,
        volatilitysurfacebuilder::VolatilitySurfaceBuilder,
    },
};

#[derive(Default)]
struct CachedMarket {
    elements: ConstructedElementStore,
    snapshots: HashMap<MarketIndex, Rc<BootstrappedCurve>>,
    affected_curves: HashMap<MarketIndex, bool>,
    affected_surfaces: HashMap<MarketIndex, bool>,
    affected_cubes: HashMap<MarketIndex, bool>,
}

/// Cache owned by one market view. Clones refer to the same constructed objects.
///
/// A cache must be discarded when its base inputs or shock definitions change.
#[derive(Clone, Default)]
pub struct MarketCache(Rc<RefCell<CachedMarket>>);

impl MarketCache {
    /// Releases all constructed objects and calibration snapshots in this cache.
    pub fn clear(&self) {
        *self.0.borrow_mut() = CachedMarket::default();
    }

    /// Replaces the cache with constructed objects and curve calibration snapshots.
    ///
    /// Supplied curves and volatility markets are marked as unaffected by shocks.
    /// The caller must ensure they match the view's base inputs and scenario.
    pub fn seed(
        &self,
        elements: ConstructedElementStore,
        snapshots: HashMap<MarketIndex, BootstrappedCurve>,
    ) {
        let affected_curves = elements
            .discount_curves()
            .keys()
            .map(|index| (index.clone(), false))
            .collect();
        let affected_surfaces = elements
            .volatility_surfaces()
            .keys()
            .map(|index| (index.clone(), false))
            .collect();
        let affected_cubes = elements
            .volatility_cubes()
            .keys()
            .map(|index| (index.clone(), false))
            .collect();
        *self.0.borrow_mut() = CachedMarket {
            elements,
            snapshots: snapshots
                .into_iter()
                .map(|(index, snapshot)| (index, Rc::new(snapshot)))
                .collect(),
            affected_curves,
            affected_surfaces,
            affected_cubes,
        };
    }
}

/// A market-data provider borrowing a base context and owning a scenario cache.
///
/// Quotes and historical fixings remain in the base context. Construction follows
/// the pricer's market-data request and the dependencies of its requested objects.
/// Unchanged curves reuse calibration results through independent AD wrappers.
pub struct MarketView<'a> {
    base: &'a PricingContext,
    scenario: Scenario,
    cache: MarketCache,
}

impl<'a> MarketView<'a> {
    /// Creates a view with an empty construction cache.
    ///
    /// This constructor does not validate the scenario. Prefer
    /// [`Scenario::apply`] when shocks must be checked against the base inputs.
    #[must_use]
    pub fn new(base: &'a PricingContext, scenario: Scenario) -> Self {
        Self {
            base,
            scenario,
            cache: MarketCache::default(),
        }
    }

    /// Reuses a cache belonging to the same base inputs and scenario definition.
    ///
    /// Bindings use this to retain requested objects across calls in a scope.
    #[must_use]
    pub fn with_cache(mut self, cache: MarketCache) -> Self {
        self.cache = cache;
        self
    }

    /// Creates a lazy, unshocked view using the context's shared request cache.
    #[must_use]
    pub fn base(context: &'a PricingContext) -> Self {
        Self::new(context, Scenario::default()).with_cache(context.request_cache().clone())
    }

    fn quote_changed(&self, identifiers: &[String]) -> bool {
        identifiers
            .iter()
            .any(|identifier| self.scenario.matches(identifier))
    }

    fn ensure_curve(
        &self,
        index: &MarketIndex,
        visiting: &mut HashSet<MarketIndex>,
    ) -> Result<bool> {
        if let Some(affected) = self.cache.0.borrow().affected_curves.get(index) {
            return Ok(*affected);
        }
        if !visiting.insert(index.clone()) {
            return Err(QSError::InvalidValueErr(format!(
                "Circular curve dependency at {index}"
            )));
        }
        let mut configuration = self
            .base
            .curve_configurations()
            .iter()
            .find(|configuration| configuration.market_index() == index)
            .cloned();
        let mut affected = false;
        let mut dependencies = HashSet::new();
        let policy =
            BootstrapDiscountPolicy::new(self.base.base_index().clone(), self.base.base_currency());
        if let Some(configuration) = &mut configuration {
            affected = self.quote_changed(configuration.quotes());
            for shock in self.scenario.shocks() {
                affected |= configuration.apply_shock(shock, self.base.evaluation_date()) > 0;
            }
            let fx = if let MarketIndex::Collateral(currency, collateral) = index {
                self.base
                    .fx_store()
                    .get_fx_rate(*collateral, *currency)
                    .ok()
                    .map(|value| value.value())
            } else {
                None
            };
            configuration.resolve(self, self.base.quote_level(), fx)?;
            dependencies = configuration.dependencies(&policy)?;
            dependencies.remove(index);
            for dependency in &dependencies {
                affected |= self.ensure_curve(dependency, visiting)?;
            }
        }
        visiting.remove(index);

        if !affected && self.copy_base_curve(index)? {
            return Ok(false);
        }
        let configuration = configuration.ok_or_else(|| {
            QSError::NotFoundErr(format!(
                "No discount curve or construction configuration for {index}"
            ))
        })?;
        let (curves, snapshots) = {
            let cache = self.cache.0.borrow();
            let curves = dependencies
                .iter()
                .filter_map(|dependency| {
                    cache
                        .elements
                        .discount_curve(dependency)
                        .map(|element| (dependency.clone(), element.clone()))
                })
                .collect();
            let snapshots = dependencies
                .iter()
                .filter_map(|dependency| {
                    cache
                        .snapshots
                        .get(dependency)
                        .map(|snapshot| (dependency.clone(), (**snapshot).clone()))
                })
                .collect();
            (curves, snapshots)
        };
        let (element, snapshot) = MultiCurveBootstrapper::new(Vec::new(), policy)
            .with_market_stores(self.base.fx_store(), self.base.fixing_store())
            .construct_curve(&configuration, &curves, &snapshots)?;
        let mut cache = self.cache.0.borrow_mut();
        cache
            .elements
            .discount_curves_mut()
            .insert(index.clone(), element);
        cache.snapshots.insert(index.clone(), Rc::new(snapshot));
        cache.affected_curves.insert(index.clone(), affected);
        Ok(affected)
    }

    fn copy_base_curve(&self, index: &MarketIndex) -> Result<bool> {
        let cached = self.base.request_cache().0.borrow();
        if let Some(element) = cached
            .elements
            .discount_curve(index)
            .or_else(|| self.base.constructed_elements().discount_curve(index))
        {
            let curve = element.curve().clone_curve()?;
            let element = DiscountCurveElement::new(index.clone(), Rc::new(RefCell::new(curve)));
            let snapshot = cached.snapshots.get(index).cloned();
            drop(cached);
            let mut cache = self.cache.0.borrow_mut();
            cache
                .elements
                .discount_curves_mut()
                .insert(index.clone(), element);
            if let Some(snapshot) = snapshot {
                cache.snapshots.insert(index.clone(), snapshot);
            }
            cache.affected_curves.insert(index.clone(), false);
            return Ok(true);
        }
        Ok(false)
    }

    fn ensure_credit(&self, index: &MarketIndex) -> Result<()> {
        if self.cache.0.borrow().elements.credit_curve(index).is_some() {
            return Ok(());
        }
        let configuration = self
            .base
            .credit_curve_configurations()
            .iter()
            .find(|configuration| configuration.market_index() == index);
        let affected = if let Some(configuration) = configuration {
            self.ensure_curve(configuration.discount_index(), &mut HashSet::new())?
                || self.quote_changed(configuration.quotes())
        } else {
            false
        };
        if !affected {
            let cached = self.base.request_cache().0.borrow();
            if let Some(element) = cached
                .elements
                .credit_curve(index)
                .or_else(|| self.base.constructed_elements().credit_curve(index))
            {
                let curve = element.curve().clone_curve()?;
                let element = CreditCurveElement::new(
                    index.clone(),
                    element.recovery(),
                    Rc::new(RefCell::new(curve)),
                );
                drop(cached);
                self.cache
                    .0
                    .borrow_mut()
                    .elements
                    .credit_curves_mut()
                    .insert(index.clone(), element);
                return Ok(());
            }
        }
        let configuration = configuration.ok_or_else(|| {
            QSError::NotFoundErr(format!("No credit curve configuration for {index}"))
        })?;
        let curves = CreditCurveBootstrapper::new(vec![configuration.clone()]).bootstrap(
            self,
            self.base.quote_level(),
            self.cache.0.borrow().elements.discount_curves(),
        )?;
        self.cache
            .0
            .borrow_mut()
            .elements
            .credit_curves_mut()
            .extend(curves);
        Ok(())
    }

    fn ensure_surface(&self, index: &MarketIndex) -> Result<bool> {
        if let Some(affected) = self.cache.0.borrow().affected_surfaces.get(index) {
            return Ok(*affected);
        }
        let configuration = self
            .base
            .volatility_surface_configurations()
            .iter()
            .find(|configuration| configuration.market_index() == index);
        let affected =
            configuration.is_some_and(|configuration| self.quote_changed(configuration.quotes()));
        if !affected {
            let cached = self.base.request_cache().0.borrow();
            if let Some(element) = cached
                .elements
                .volatility_surface(index)
                .or_else(|| self.base.constructed_elements().volatility_surface(index))
            {
                let surface = element.surface().clone_surface()?;
                let element = VolatilitySurfaceElement::new(index.clone(), surface);
                drop(cached);
                let mut cache = self.cache.0.borrow_mut();
                cache
                    .elements
                    .volatility_surfaces_mut()
                    .insert(index.clone(), element);
                cache.affected_surfaces.insert(index.clone(), false);
                return Ok(false);
            }
        }
        let configuration = configuration.ok_or_else(|| {
            QSError::NotFoundErr(format!("No volatility surface configuration for {index}"))
        })?;
        let surfaces = VolatilitySurfaceBuilder::new(vec![configuration.clone()])
            .build(self, self.base.quote_level())?;
        let mut cache = self.cache.0.borrow_mut();
        cache.elements.volatility_surfaces_mut().extend(surfaces);
        cache.affected_surfaces.insert(index.clone(), affected);
        Ok(affected)
    }

    fn ensure_cube(&self, index: &MarketIndex) -> Result<bool> {
        if let Some(affected) = self.cache.0.borrow().affected_cubes.get(index) {
            return Ok(*affected);
        }
        let configuration = self
            .base
            .volatility_cube_configurations()
            .iter()
            .find(|configuration| configuration.market_index() == index);
        let affected =
            configuration.is_some_and(|configuration| self.quote_changed(configuration.quotes()));
        if !affected {
            let cached = self.base.request_cache().0.borrow();
            if let Some(element) = cached
                .elements
                .volatility_cube(index)
                .or_else(|| self.base.constructed_elements().volatility_cube(index))
            {
                let cube = element.cube().clone_cube()?;
                let element = VolatilityCubeElement::new(index.clone(), cube);
                drop(cached);
                let mut cache = self.cache.0.borrow_mut();
                cache
                    .elements
                    .volatility_cubes_mut()
                    .insert(index.clone(), element);
                cache.affected_cubes.insert(index.clone(), false);
                return Ok(false);
            }
        }
        let configuration = configuration.ok_or_else(|| {
            QSError::NotFoundErr(format!("No volatility cube configuration for {index}"))
        })?;
        let cubes = VolatilityCubeBuilder::new(vec![configuration.clone()])
            .build(self, self.base.quote_level())?;
        let mut cache = self.cache.0.borrow_mut();
        cache.elements.volatility_cubes_mut().extend(cubes);
        cache.affected_cubes.insert(index.clone(), affected);
        Ok(affected)
    }

    fn ensure_calibration(&self, configuration: &ModelCalibrationConfiguration) -> Result<bool> {
        match configuration.source() {
            CalibrationSource::Surface { market_index } => self.ensure_surface(market_index),
            CalibrationSource::Cube { market_index } => self.ensure_cube(market_index),
        }
    }

    fn ensure_simulation(&self, index: &MarketIndex) -> Result<()> {
        if self
            .cache
            .0
            .borrow()
            .elements
            .simulations()
            .contains_key(index)
        {
            return Ok(());
        }
        if self
            .base
            .simulation_configurations()
            .iter()
            .all(|configuration| configuration.market_index() != index)
        {
            if let Some(element) = self.base.constructed_elements().simulations().get(index) {
                self.cache
                    .0
                    .borrow_mut()
                    .elements
                    .simulations_mut()
                    .insert(index.clone(), element.clone());
                return Ok(());
            }
        }
        let configuration = self
            .base
            .simulation_configurations()
            .iter()
            .find(|configuration| configuration.market_index() == index)
            .ok_or_else(|| {
                QSError::NotFoundErr(format!("No simulation configuration for {index}"))
            })?;
        let mut affected = self.ensure_curve(index, &mut HashSet::new())?;
        let calibration = match configuration.model() {
            ModelConfiguration::HullWhite {
                parameter_source, ..
            }
            | ModelConfiguration::Lgm {
                parameter_source, ..
            } => match parameter_source {
                ParameterSource::Calibrated(configuration) => Some(configuration),
                ParameterSource::Fixed(_) => None,
            },
            ModelConfiguration::BrownianMotion {
                parameter_source, ..
            } => match parameter_source {
                ParameterSource::Calibrated(configuration) => Some(configuration),
                ParameterSource::Fixed(_) => None,
            },
        };
        if let Some(calibration) = calibration {
            affected |= self.ensure_calibration(calibration)?;
        }
        if !affected {
            let cached = self.base.request_cache().0.borrow();
            if let Some(element) = cached
                .elements
                .simulations()
                .get(index)
                .or_else(|| self.base.constructed_elements().simulations().get(index))
            {
                let element = element.clone();
                drop(cached);
                self.cache
                    .0
                    .borrow_mut()
                    .elements
                    .simulations_mut()
                    .insert(index.clone(), element);
                return Ok(());
            }
        }
        let simulations = SimulationBuilder::new(vec![configuration.clone()]).build(
            &self.cache.0.borrow().elements,
            self,
            self.base.fixing_store(),
            self.base.quote_level(),
        )?;
        self.cache
            .0
            .borrow_mut()
            .elements
            .simulations_mut()
            .extend(simulations);
        Ok(())
    }

    fn ensure_dividend(&self, index: &MarketIndex, optional: bool) -> Result<()> {
        if self
            .cache
            .0
            .borrow()
            .elements
            .dividend_curve(index)
            .is_some()
        {
            return Ok(());
        }
        let element = self.base.constructed_elements().dividend_curve(index);
        if let Some(element) = element {
            let curve = element.curve().clone_curve()?;
            self.cache
                .0
                .borrow_mut()
                .elements
                .dividend_curves_mut()
                .insert(
                    index.clone(),
                    DividendCurveElement::new(index.clone(), Rc::new(RefCell::new(curve))),
                );
            Ok(())
        } else if optional {
            Ok(())
        } else {
            Err(QSError::NotFoundErr(format!(
                "No dividend curve for {index}"
            )))
        }
    }
}

impl QuoteSelector for MarketView<'_> {
    fn select(&self, identifier: &str) -> Option<Quote> {
        self.base
            .quote_store()
            .select(identifier)
            .map(|quote| self.scenario.shock_quote(&quote))
    }

    fn reference_date(&self) -> Date {
        self.base.evaluation_date()
    }
}

impl MarketDataProvider for MarketView<'_> {
    fn evaluation_date(&self) -> Date {
        self.base.evaluation_date()
    }

    fn handle_request(&self, request: &MarketDataRequest) -> Result<MarketData> {
        if let Some(elements) = request.constructed_elements_request() {
            for element in elements {
                match element {
                    ConstructedElementRequest::DiscountCurve { market_index } => {
                        self.ensure_curve(market_index, &mut HashSet::new())?;
                    }
                    ConstructedElementRequest::CreditCurve { market_index } => {
                        self.ensure_credit(market_index)?;
                    }
                    ConstructedElementRequest::DividendCurve { market_index } => {
                        self.ensure_dividend(market_index, false)?;
                    }
                    ConstructedElementRequest::OptionalDividendCurve { market_index } => {
                        self.ensure_dividend(market_index, true)?;
                    }
                    ConstructedElementRequest::VolatilitySurface { market_index } => {
                        self.ensure_surface(market_index)?;
                    }
                    ConstructedElementRequest::VolatilityCube { market_index } => {
                        self.ensure_cube(market_index)?;
                    }
                    ConstructedElementRequest::Simulation { market_index } => {
                        self.ensure_simulation(market_index)?;
                    }
                }
            }
        }
        self.base
            .resolve_request(request, &self.cache.0.borrow().elements, self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::{
        ADForward, CurveConfiguration, DualFwd, QuoteStore, ScenarioType, Shock, Tape,
    };

    fn context() -> Result<PricingContext> {
        let curves: Vec<CurveConfiguration> = serde_json::from_value(serde_json::json!([
            {"market_index": "ESTR", "interpolator": "Linear", "construction": {
                "type": "ZeroRate", "pillars": ["0D", "1Y", "5Y"],
                "rates": [0.03, 0.035, 0.04], "compounding": "Continuous", "frequency": "Annual"
            }},
            {"market_index": {"Other": "SPREAD"}, "interpolator": "Linear", "construction": {
                "type": "ZeroRate", "pillars": ["0D", "1Y", "5Y"],
                "rates": [0.005, 0.0075, 0.01], "compounding": "Continuous", "frequency": "Annual"
            }},
            {"market_index": {"Other": "FUNDING"}, "construction": {
                "type": "Composite", "base_index": "ESTR", "spread_index": {"Other": "SPREAD"}
            }}
        ]))
        .map_err(|error| QSError::InvalidValueErr(error.to_string()))?;
        Ok(PricingContext::new()
            .with_quote_store(QuoteStore::new(Date::new(2026, 2, 24)))
            .with_curve_configurations(curves))
    }

    fn request(index: MarketIndex) -> MarketDataRequest {
        MarketDataRequest::default().with_constructed_elements_request(vec![
            ConstructedElementRequest::DiscountCurve {
                market_index: index,
            },
        ])
    }

    #[test]
    fn requested_dependencies_are_cached_without_constructing_unrelated_markets() -> Result<()> {
        let mut context = context()?;
        let mut configurations = context.curve_configurations().clone();
        for number in 0..200 {
            let configuration = serde_json::from_value(serde_json::json!({
                "market_index": {"Other": format!("UNRELATED{number}")},
                "quotes": ["MISSING_QUOTE"]
            }))
            .map_err(|error| QSError::InvalidValueErr(error.to_string()))?;
            configurations.push(configuration);
        }
        let simulations = serde_json::from_value(serde_json::json!([{
            "market_index": {"Other": "MISSING_SIMULATION_CURVE"},
            "model": {"HullWhite": {"alpha": 0.1, "parameter_source": {"Fixed": {"sigma": 0.01}}}},
            "n_paths": 1000000, "horizon": "30Y"
        }]))
        .map_err(|error| QSError::InvalidValueErr(error.to_string()))?;
        context = context
            .with_curve_configurations(configurations)
            .with_simulation_configurations(simulations);
        let scenario = Scenario::new(vec![Shock::new("SPREAD", 0.005, ScenarioType::Absolute)]);
        let view = scenario.apply(&context)?;
        assert!(view.cache.0.borrow().elements.discount_curves().is_empty());
        let index = MarketIndex::Other("FUNDING".into());
        let first = view.handle_request(&request(index.clone()))?;
        let second = view.handle_request(&request(index.clone()))?;
        assert!(std::ptr::eq(
            &*first
                .constructed_elements()
                .discount_curve(&index)
                .ok_or_else(|| QSError::NotFoundErr("funding".into()))?
                .curve(),
            &*second
                .constructed_elements()
                .discount_curve(&index)
                .ok_or_else(|| QSError::NotFoundErr("funding".into()))?
                .curve(),
        ));
        assert_eq!(view.cache.0.borrow().elements.discount_curves().len(), 3);
        assert!(view.cache.0.borrow().elements.simulations().is_empty());
        assert!(context
            .request_cache()
            .0
            .borrow()
            .elements
            .discount_curves()
            .is_empty());
        assert!(context.constructed_elements().discount_curves().is_empty());
        Ok(())
    }

    #[test]
    fn unchanged_curve_copies_detach_tape_nodes_and_preserve_base_pillars() -> Result<()> {
        let mut context = context()?;
        context.initialize()?;
        Tape::<ADForward>::start_recording_fwd();
        let mut element = context
            .constructed_elements()
            .discount_curve(&MarketIndex::ESTR)
            .ok_or_else(|| QSError::NotFoundErr("ESTR".into()))?
            .clone();
        element.curve_mut().put_pillars_on_tape();
        let base_nodes: Vec<_> = element
            .curve()
            .pillars()
            .unwrap_or_default()
            .iter()
            .map(|(_, value)| value.node_ptr())
            .collect();
        assert!(base_nodes.iter().all(Option::is_some));
        let scenario = Scenario::new(vec![Shock::new("SPREAD", 0.005, ScenarioType::Absolute)]);
        let view = scenario.apply(&context)?;
        let mut market = view.handle_request(&request(MarketIndex::ESTR))?;
        let curve = market
            .constructed_elements_mut()
            .discount_curves_mut()
            .get_mut(&MarketIndex::ESTR)
            .ok_or_else(|| QSError::NotFoundErr("ESTR view".into()))?;
        assert!(curve
            .curve()
            .pillars()
            .unwrap_or_default()
            .iter()
            .all(|(_, value)| value.node_ptr().is_none()));
        curve.curve_mut().put_pillars_on_tape();
        let view_nodes: Vec<_> = curve
            .curve()
            .pillars()
            .unwrap_or_default()
            .iter()
            .map(|(_, value)| value.node_ptr())
            .collect();
        assert!(view_nodes
            .iter()
            .zip(&base_nodes)
            .all(|(view, base)| view != base));
        assert_eq!(
            element
                .curve()
                .pillars()
                .unwrap_or_default()
                .iter()
                .map(|(_, value)| value.node_ptr())
                .collect::<Vec<_>>(),
            base_nodes
        );
        let factor: DualFwd = curve.curve().discount_factor(Date::new(2027, 2, 24))?;
        factor.backward()?;
        assert!(element
            .curve()
            .pillars()
            .unwrap_or_default()
            .iter()
            .all(|(_, value)| value.adjoint().is_ok_and(|value| value.value() == 0.0)));
        Tape::<ADForward>::stop_recording_fwd();
        Ok(())
    }

    #[test]
    fn input_replacement_invalidates_lazy_market_cache() -> Result<()> {
        let context = context()?;
        let _ = context.handle_request(&request(MarketIndex::ESTR))?;
        assert_eq!(
            context
                .request_cache()
                .0
                .borrow()
                .elements
                .discount_curves()
                .len(),
            1
        );
        let changed = context.clone().with_curve_configurations(Vec::new());
        assert!(changed.handle_request(&request(MarketIndex::ESTR)).is_err());
        assert!(context.handle_request(&request(MarketIndex::ESTR)).is_ok());
        Ok(())
    }

    #[test]
    fn simulation_paths_are_reused_or_rebuilt_according_to_requested_dependencies() -> Result<()> {
        let configurations = serde_json::from_value(serde_json::json!([{
            "market_index": "ESTR",
            "model": {"HullWhite": {"alpha": 0.1, "parameter_source": {"Fixed": {"sigma": 0.01}}}},
            "n_paths": 4, "horizon": "1Y"
        }]))
        .map_err(|error| QSError::InvalidValueErr(error.to_string()))?;
        let mut context = context()?.with_simulation_configurations(configurations);
        context.initialize()?;
        let simulation_request =
            MarketDataRequest::default().with_constructed_elements_request(vec![
                ConstructedElementRequest::Simulation {
                    market_index: MarketIndex::ESTR,
                },
            ]);
        let base = context
            .constructed_elements()
            .simulations()
            .get(&MarketIndex::ESTR)
            .ok_or_else(|| QSError::NotFoundErr("base simulation".into()))?;
        for (target, value) in [("SPREAD", 0.005), ("ESTR", 0.0)] {
            let unchanged = Scenario::new(vec![Shock::new(target, value, ScenarioType::Absolute)])
                .apply(&context)?;
            let response = unchanged.handle_request(&simulation_request)?;
            let reused = response
                .constructed_elements()
                .simulations()
                .get(&MarketIndex::ESTR)
                .ok_or_else(|| QSError::NotFoundErr("reused simulation".into()))?;
            assert!(Rc::ptr_eq(base.simulation(), reused.simulation()));
        }

        let changed = Scenario::new(vec![Shock::new("ESTR", 0.01, ScenarioType::Absolute)])
            .apply(&context)?;
        let _ = changed.handle_request(&request(MarketIndex::ESTR))?;
        assert!(changed.cache.0.borrow().elements.simulations().is_empty());
        let response = changed.handle_request(&simulation_request)?;
        let rebuilt = response
            .constructed_elements()
            .simulations()
            .get(&MarketIndex::ESTR)
            .ok_or_else(|| QSError::NotFoundErr("rebuilt simulation".into()))?;
        assert!(!Rc::ptr_eq(base.simulation(), rebuilt.simulation()));
        Ok(())
    }

    #[test]
    fn case_variants_of_quote_ids_still_invalidate_scenario_curve_caches() -> Result<()> {
        use crate::prelude::{Currency, DayCounter, Interpolator, Quote, QuoteLevels};
        let date = Date::new(2026, 1, 15);
        let maturity = Date::new(2027, 1, 15);
        let mut quotes = QuoteStore::new(date);
        quotes.add_quote(Quote::new(
            "ZeroRate_EUR_ESTR_1Y_Continuous_Annual".parse()?,
            QuoteLevels::with_mid(0.05),
        ));
        let configuration = CurveConfiguration::new(
            MarketIndex::ESTR,
            DayCounter::Actual365,
            Interpolator::Linear,
            true,
            vec!["zerorate_eur_estr_1y_continuous_annual".into()],
        );
        let mut context = PricingContext::new()
            .with_quote_store(quotes)
            .with_base_currency(Currency::EUR)
            .with_base_index(MarketIndex::ESTR)
            .with_curve_configurations(vec![configuration]);
        context.initialize()?;
        let before = context
            .constructed_elements()
            .discount_curve(&MarketIndex::ESTR)
            .ok_or_else(|| QSError::NotFoundErr("ESTR".into()))?
            .curve()
            .discount_factor(maturity)?
            .value();
        let scenario = Scenario::new(vec![Shock::new(
            "ZeroRate_EUR_ESTR_1Y_Continuous_Annual",
            0.01,
            ScenarioType::Absolute,
        )]);
        let view = scenario.apply(&context)?;
        let shocked = view.handle_request(&request(MarketIndex::ESTR))?;
        let after = shocked
            .constructed_elements()
            .discount_curve(&MarketIndex::ESTR)
            .ok_or_else(|| QSError::NotFoundErr("ESTR".into()))?
            .curve()
            .discount_factor(maturity)?
            .value();
        assert!((before - (-0.05_f64).exp()).abs() < 1e-12);
        assert!((after - (-0.06_f64).exp()).abs() < 1e-12);
        Ok(())
    }
}
