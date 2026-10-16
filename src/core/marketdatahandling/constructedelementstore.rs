use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::{
    core::elements::{
        curveelement::{CreditCurveElement, DiscountCurveElement, DividendCurveElement},
        montecarlosimulationelement::MonteCarloSimulationElement,
        volatilitycubelement::VolatilityCubeElement,
        volatilitysurfaceelement::VolatilitySurfaceElement,
    },
    indices::{fxpair::FxPair, marketindex::MarketIndex},
    rates::yieldtermstructure::compositetermstructure::CompositeTermStructure,
    utils::errors::{QSError, Result},
    volatility::orientedfxvolsurface::OrientedFxVolSurface,
};

/// Type alias for a shared element using reference counting and interior mutability.
pub type SharedElement<T> = Rc<RefCell<T>>;

/// Struct representing a store for constructed market data elements, including discount curves, dividend curves,
/// volatility surfaces, volatility cubes, and simulations.
#[derive(Clone, Default)]
pub struct ConstructedElementStore {
    discount_curves: HashMap<MarketIndex, DiscountCurveElement>,
    dividend_curves: HashMap<MarketIndex, DividendCurveElement>,
    credit_curves: HashMap<MarketIndex, CreditCurveElement>,
    volatility_surfaces: HashMap<MarketIndex, VolatilitySurfaceElement>,
    volatility_cubes: HashMap<MarketIndex, VolatilityCubeElement>,
    simulations: HashMap<MarketIndex, MonteCarloSimulationElement>,
}

impl ConstructedElementStore {
    /// Registers a composite curve whose discount factors multiply those of two
    /// existing curves. Components are owned snapshots; recompose after rebuilding
    /// either source curve.
    ///
    /// # Errors
    /// Rejects an existing destination index, missing or non-copyable components,
    /// and different reference dates or day-count conventions.
    pub fn add_composite_discount_curve(
        &mut self,
        index: MarketIndex,
        base_index: &MarketIndex,
        spread_index: &MarketIndex,
    ) -> Result<()> {
        if self.discount_curves.contains_key(&index) {
            return Err(QSError::InvalidValueErr(format!(
                "Discount curve {index} already exists"
            )));
        }
        let component = |source: &MarketIndex| {
            self.discount_curves
                .get(source)
                .ok_or_else(|| QSError::NotFoundErr(format!("Discount curve {source}")))?
                .curve()
                .clone_curve()
        };
        let base = component(base_index)?;
        let spread = component(spread_index)?;
        if base.reference_date() != spread.reference_date()
            || base.day_counter() != spread.day_counter()
        {
            return Err(QSError::InvalidValueErr(
                "Composite curves require matching reference dates and day counters".into(),
            ));
        }
        let curve = CompositeTermStructure::from_curves(spread, base);
        self.discount_curves.insert(
            index.clone(),
            DiscountCurveElement::new(index, Rc::new(RefCell::new(curve))),
        );
        Ok(())
    }

    /// Returns discount curves.
    #[must_use]
    pub const fn discount_curves(&self) -> &HashMap<MarketIndex, DiscountCurveElement> {
        &self.discount_curves
    }

    /// Checks if the store is empty (i.e., contains no constructed elements).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.discount_curves.is_empty()
            && self.dividend_curves.is_empty()
            && self.credit_curves.is_empty()
            && self.volatility_surfaces.is_empty()
            && self.volatility_cubes.is_empty()
            && self.simulations.is_empty()
    }

    /// Returns mutable discount curves map.
    #[must_use]
    pub const fn discount_curves_mut(&mut self) -> &mut HashMap<MarketIndex, DiscountCurveElement> {
        &mut self.discount_curves
    }

    /// Returns dividend curves.
    #[must_use]
    pub const fn dividend_curves(&self) -> &HashMap<MarketIndex, DividendCurveElement> {
        &self.dividend_curves
    }

    /// Returns mutable dividend curves map.
    #[must_use]
    pub const fn dividend_curves_mut(&mut self) -> &mut HashMap<MarketIndex, DividendCurveElement> {
        &mut self.dividend_curves
    }

    /// Returns credit (survival) curves.
    #[must_use]
    pub const fn credit_curves(&self) -> &HashMap<MarketIndex, CreditCurveElement> {
        &self.credit_curves
    }

    /// Returns mutable credit curves map.
    #[must_use]
    pub const fn credit_curves_mut(&mut self) -> &mut HashMap<MarketIndex, CreditCurveElement> {
        &mut self.credit_curves
    }

    /// Gets one credit curve by index.
    #[must_use]
    pub fn credit_curve(&self, index: &MarketIndex) -> Option<&CreditCurveElement> {
        self.credit_curves.get(index)
    }

    /// Returns volatility surfaces.
    #[must_use]
    pub const fn volatility_surfaces(&self) -> &HashMap<MarketIndex, VolatilitySurfaceElement> {
        &self.volatility_surfaces
    }

    /// Returns mutable volatility surfaces map.
    #[must_use]
    pub const fn volatility_surfaces_mut(
        &mut self,
    ) -> &mut HashMap<MarketIndex, VolatilitySurfaceElement> {
        &mut self.volatility_surfaces
    }

    /// Returns volatility cubes.
    #[must_use]
    pub const fn volatility_cubes(&self) -> &HashMap<MarketIndex, VolatilityCubeElement> {
        &self.volatility_cubes
    }

    /// Returns mutable volatility cubes map.
    #[must_use]
    pub const fn volatility_cubes_mut(
        &mut self,
    ) -> &mut HashMap<MarketIndex, VolatilityCubeElement> {
        &mut self.volatility_cubes
    }

    /// Returns simulations.
    #[must_use]
    pub const fn simulations(&self) -> &HashMap<MarketIndex, MonteCarloSimulationElement> {
        &self.simulations
    }

    /// Returns mutable simulations map.
    #[must_use]
    pub const fn simulations_mut(
        &mut self,
    ) -> &mut HashMap<MarketIndex, MonteCarloSimulationElement> {
        &mut self.simulations
    }

    /// Gets one discount curve by index.
    #[must_use]
    pub fn discount_curve(&self, index: &MarketIndex) -> Option<&DiscountCurveElement> {
        self.discount_curves.get(index)
    }

    /// Gets one dividend curve by index.
    #[must_use]
    pub fn dividend_curve(&self, index: &MarketIndex) -> Option<&DividendCurveElement> {
        self.dividend_curves.get(index)
    }

    /// Gets one volatility surface by index.
    #[must_use]
    pub fn volatility_surface(&self, index: &MarketIndex) -> Option<&VolatilitySurfaceElement> {
        self.volatility_surfaces.get(index)
    }

    /// Gets an FX volatility surface for the given pair, returning an oriented
    /// view that transparently handles parity inversion.
    ///
    /// Looks up first by the pair as given, then by its canonical form. If the
    /// surface is found under the inverted orientation the returned adapter
    /// applies the `K → 1/K` transform automatically.
    #[must_use]
    pub fn fx_volatility_surface(&self, pair: &FxPair) -> Option<OrientedFxVolSurface<'_>> {
        let key = MarketIndex::FxPair(*pair);
        if let Some(elem) = self.volatility_surfaces.get(&key) {
            return Some(OrientedFxVolSurface::new(elem, false));
        }
        // Try the inverted pair
        let inv_key = MarketIndex::FxPair(pair.inverted());
        if let Some(elem) = self.volatility_surfaces.get(&inv_key) {
            return Some(OrientedFxVolSurface::new(elem, true));
        }
        None
    }

    /// Gets one volatility cube by index.
    #[must_use]
    pub fn volatility_cube(&self, index: &MarketIndex) -> Option<&VolatilityCubeElement> {
        self.volatility_cubes.get(index)
    }
}
