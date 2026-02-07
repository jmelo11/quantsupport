use std::collections::HashMap;

use crate::{
    ad::adreal::ADReal,
    indices::marketindex::MarketIndex,
    marketdata::{
        fixingprovider::FixingProvider, marketdataprovider::MarketDataProvider, quote::Level,
    },
    rates::yieldtermstructure::discounttermstructure::DiscountTermStructure,
    time::date::Date,
    utils::errors::{AtlasError, Result},
};

/// # `MarketValue`
///
/// Holds a market index along with a typed value representation.
pub struct MarketValue<T> {
    market_index: MarketIndex,
    value: T,
}

impl<T: Copy> MarketValue<T> {
    /// Creates a new market value.
    #[must_use]
    pub const fn new(market_index: MarketIndex, value: T) -> Self {
        Self {
            market_index,
            value,
        }
    }

    /// Returns the market index for this value.
    #[must_use]
    pub const fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }

    /// Returns the stored value.
    #[must_use]
    pub const fn value(&self) -> T {
        self.value
    }
}

/// # `CurvePillar`
/// Represents a curve pillar date and its value.
pub struct CurvePillar {
    date: Date,
    value: ADReal,
}

impl CurvePillar {
    /// Creates a new curve pillar.
    #[must_use]
    pub const fn new(date: Date, value: ADReal) -> Self {
        Self { date, value }
    }

    /// Returns the pillar date.
    #[must_use]
    pub const fn date(&self) -> Date {
        self.date
    }

    /// Returns the pillar value.
    #[must_use]
    pub const fn value(&self) -> ADReal {
        self.value
    }
}

/// # `CurveInputs`
/// Holds an AD-ready curve and its pillar inputs.
pub struct CurveInputs {
    market_index: MarketIndex,
    curve: DiscountTermStructure<ADReal>,
    pillars: Vec<CurvePillar>,
}

impl CurveInputs {
    /// Creates a new curve inputs container.
    #[must_use]
    pub const fn new(
        market_index: MarketIndex,
        curve: DiscountTermStructure<ADReal>,
        pillars: Vec<CurvePillar>,
    ) -> Self {
        Self {
            market_index,
            curve,
            pillars,
        }
    }

    /// Returns the market index for the curve.
    #[must_use]
    pub const fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }

    /// Returns the AD-ready curve.
    #[must_use]
    pub const fn curve(&self) -> &DiscountTermStructure<ADReal> {
        &self.curve
    }

    /// Returns the curve pillars.
    #[must_use]
    pub const fn pillars(&self) -> &Vec<CurvePillar> {
        &self.pillars
    }
}

/// # `PricingDataContext`
pub struct PricingDataContext {
    market_data_provider: MarketDataProvider,
    fixings_provider: FixingProvider,
    model_configuration: usize, // Placeholder for model configuration identifier, WIP
    quote_level: Level,
    discount_curves: HashMap<MarketIndex, DiscountTermStructure<f64>>,
}

impl PricingDataContext {
    /// Creates a new pricing data context.
    #[must_use]
    pub fn new(
        market_data_provider: MarketDataProvider,
        fixings_provider: FixingProvider,
        model_configuration: usize,
    ) -> Self {
        Self {
            market_data_provider,
            fixings_provider,
            model_configuration,
            quote_level: Level::Mid,
            discount_curves: HashMap::new(),
        }
    }

    /// Sets the quote level used for market value extraction.
    #[must_use]
    pub fn with_quote_level(mut self, quote_level: Level) -> Self {
        self.quote_level = quote_level;
        self
    }

    /// Returns the market data provider.
    #[must_use]
    pub const fn market_data_provider(&self) -> &MarketDataProvider {
        &self.market_data_provider
    }

    /// Returns the fixings provider.
    #[must_use]
    pub const fn fixings_provider(&self) -> &FixingProvider {
        &self.fixings_provider
    }

    /// Returns the model configuration identifier.
    #[must_use]
    pub const fn model_configuration(&self) -> usize {
        self.model_configuration
    }

    /// Returns the current reference date.
    #[must_use]
    pub const fn evaluation_date(&self) -> Date {
        self.market_data_provider.reference_date()
    }

    /// Adds a discount curve to the context.
    pub fn add_discount_curve(
        &mut self,
        market_index: MarketIndex,
        curve: DiscountTermStructure<f64>,
    ) {
        self.discount_curves.insert(market_index, curve);
    }

    /// Returns a reference to the discount curve for a given market index.
    ///
    /// ## Errors
    /// Returns an error if the curve is not available.
    pub fn discount_curve(
        &self,
        market_index: &MarketIndex,
    ) -> Result<&DiscountTermStructure<f64>> {
        self.discount_curves
            .get(market_index)
            .ok_or(AtlasError::NotFoundErr(format!(
                "Discount curve not found for index {market_index}."
            )))
    }

    /// Returns an AD-ready curve and its pillar inputs for a given market index.
    ///
    /// ## Errors
    /// Returns an error if the curve is not available or cannot be rebuilt.
    pub fn discount_curve_inputs(&self, market_index: &MarketIndex) -> Result<CurveInputs> {
        let curve = self.discount_curve(market_index)?;
        let pillars = curve
            .dates()
            .iter()
            .zip(curve.discount_factors())
            .map(|(date, df)| CurvePillar::new(*date, ADReal::from(*df)))
            .collect::<Vec<_>>();
        let curve_ad = DiscountTermStructure::<ADReal>::new(
            curve.dates().clone(),
            pillars.iter().map(CurvePillar::value).collect(),
            curve.day_counter(),
            curve.interpolator(),
            curve.enable_extrapolation(),
        )?;
        Ok(CurveInputs::new(
            market_index.clone(),
            curve_ad,
            pillars,
        ))
    }

    /// Returns the market value for an index as a plain floating point value.
    ///
    /// ## Errors
    /// Returns an error if the market value cannot be found.
    pub fn market_value(&self, market_index: &MarketIndex) -> Result<MarketValue<f64>> {
        let value = self
            .market_data_provider
            .quote_value(market_index, &self.quote_level)?;
        Ok(MarketValue::new(market_index.clone(), value))
    }

    /// Returns the market value for an index as an automatic differentiation input.
    ///
    /// ## Errors
    /// Returns an error if the market value cannot be found.
    pub fn market_input(&self, market_index: &MarketIndex) -> Result<MarketValue<ADReal>> {
        let value = self
            .market_data_provider
            .quote_value(market_index, &self.quote_level)?;
        Ok(MarketValue::new(market_index.clone(), ADReal::from(value)))
    }
}
