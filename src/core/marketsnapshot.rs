//! Serializable market inputs for reproducible historical construction.

use serde::{Deserialize, Serialize};

use crate::{
    core::{pricerconfiguration::PricerConfiguration, pricingcontext::PricingContext},
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    models::modelconfiguration::SimulationConfiguration,
    quotes::{
        fixingstore::FixingStore,
        fxstore::{FxRateRecord, FxStore},
        quote::Level,
        quotestore::QuoteStore,
    },
    rates::bootstrapping::{
        creditcurveconfiguration::CreditCurveConfiguration, curveconfiguration::CurveConfiguration,
    },
    time::datetime::DateTime,
    utils::errors::{QSError, Result},
    volatility::{
        volatilitycubeconfiguration::VolatilityCubeConfiguration,
        volatilitysurfaceconfiguration::VolatilitySurfaceConfiguration,
    },
};

/// One timestamped, coherent set of market observations and construction inputs.
///
/// Curves are saved as configurations referencing quote identifiers or composite
/// components, and are rebuilt for the requested quote side. Runtime AD objects
/// and caches are not serialized. FX records are side-independent; instrument,
/// zero-rate and discount-factor quotes all use the selected level.
/// Historical fixings are supplied separately when creating a pricing context.
#[derive(Clone, Serialize, Deserialize)]
pub struct MarketSnapshot {
    timestamp: DateTime,
    quotes: QuoteStore,
    #[serde(default)]
    fx: Vec<FxRateRecord>,
    #[serde(default)]
    curves: Vec<CurveConfiguration>,
    #[serde(default)]
    credit_curves: Vec<CreditCurveConfiguration>,
    #[serde(default)]
    volatility_surfaces: Vec<VolatilitySurfaceConfiguration>,
    #[serde(default)]
    volatility_cubes: Vec<VolatilityCubeConfiguration>,
    #[serde(default)]
    simulations: Vec<SimulationConfiguration>,
    #[serde(default)]
    pricers: Vec<PricerConfiguration>,
    base_currency: Currency,
    base_index: MarketIndex,
}

impl MarketSnapshot {
    /// Captures the base market's serializable inputs, not temporary scenarios.
    ///
    /// The timestamp identifies the observation instant. The quote reference date
    /// remains the business date used for pricing, even across UTC day boundaries.
    ///
    /// # Errors
    /// Returns an error if externally supplied market objects would be lost.
    pub fn from_context(timestamp: DateTime, context: &PricingContext) -> Result<Self> {
        if context.has_supplied_elements() {
            return Err(QSError::InvalidValueErr(
                "Snapshots require construction configurations instead of supplied runtime market objects".into(),
            ));
        }
        Ok(Self {
            timestamp,
            quotes: context.quote_store().clone(),
            fx: context.fx_store().records(),
            curves: context.curve_configurations().clone(),
            credit_curves: context.credit_curve_configurations().clone(),
            volatility_surfaces: context.volatility_surface_configurations().clone(),
            volatility_cubes: context.volatility_cube_configurations().clone(),
            simulations: context.simulation_configurations().clone(),
            pricers: context.pricer_configurations().clone(),
            base_currency: context.base_currency(),
            base_index: context.base_index().clone(),
        })
    }

    /// Returns the observation timestamp, normalized to UTC.
    #[must_use]
    pub const fn timestamp(&self) -> DateTime {
        self.timestamp
    }

    /// Returns the observations and their business reference date.
    #[must_use]
    pub const fn quotes(&self) -> &QuoteStore {
        &self.quotes
    }

    /// Creates an independent context, without constructed objects or AD state.
    ///
    /// Its evaluation date is the selected market's business date. Fixings are
    /// not publication-vintage filtered; callers must supply the appropriate
    /// historical fixing dataset if revisions or publication times matter.
    #[must_use]
    pub fn pricing_context(&self, fixings: FixingStore, level: Level) -> PricingContext {
        PricingContext::new()
            .with_quote_store(self.quotes.clone())
            .with_quote_level(level)
            .with_fx_store(FxStore::from_records(self.fx.clone()))
            .with_fixing_store(fixings)
            .with_curve_configurations(self.curves.clone())
            .with_credit_curve_configurations(self.credit_curves.clone())
            .with_volatility_surface_configurations(self.volatility_surfaces.clone())
            .with_volatility_cube_configurations(self.volatility_cubes.clone())
            .with_simulation_configurations(self.simulations.clone())
            .with_pricer_configurations(self.pricers.clone())
            .with_base_currency(self.base_currency)
            .with_base_index(self.base_index.clone())
            .with_snapshot_timestamp(self.timestamp)
    }
}
