use std::{
    any::Any,
    collections::HashMap,
    sync::{Arc, RwLock},
};

use crate::{
    ad::adreal::ADReal, core::contextmanager::ContextManager, indices::marketindex::MarketIndex,
};

/// AssetType
#[derive(Clone)]
pub enum AssetType {
    /// Curve
    InterestRateCurve(Arc<dyn Asset>),
    /// Vol Surface
    VolatilitySurface(Arc<dyn Asset>),
    /// Vol Cube
    VolatilityCube(Arc<dyn Asset>),
}

/// Generated Assets, like discount curves, stripped vol surfaces, etc.
#[derive(Default)]
pub struct Assets {
    assets: RwLock<HashMap<MarketIndex, Vec<AssetType>>>, // or HashMap<MarketIndex, RwLock<AssetType>>?
}

/// Is an asset
pub trait Asset: Any + Send + Sync {
    /// Asset type
    fn asset_type(&self) -> AssetType;

    /// Inputs to the asset
    fn asset_inputs(&self) -> Vec<(String, ADReal)>;

    /// Cast helper for downcasting.
    fn as_any(&self) -> &dyn Any;
}

/// Trait for any type of asset generator (bootstrapping/stripping engine)
pub trait AssetGenerator {
    /// generates related assets
    fn generate_assets(&self, ctx: &ContextManager) -> Vec<(MarketIndex, AssetType)>;
}

impl Assets {
    /// Retrieves an asset by market index.
    #[must_use]
    pub fn get(&self, market_index: &MarketIndex) -> Option<Vec<AssetType>> {
        self.assets
            .read()
            .ok()
            .and_then(|assets| assets.get(market_index).cloned())
    }

    /// Returns whether an asset exists for the market index.
    #[must_use]
    pub fn contains(&self, market_index: &MarketIndex) -> bool {
        self.assets
            .read()
            .map(|assets| assets.contains_key(market_index))
            .unwrap_or(false)
    }
}
