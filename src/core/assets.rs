use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use crate::{
    ad::adreal::ADReal, core::contextmanager::ContextManager, indices::marketindex::MarketIndex,
};

/// AssetType
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
    assets: RwLock<HashMap<MarketIndex, AssetType>>, // or HashMap<MarketIndex, RwLock<AssetType>>?
}

/// Is an asset
pub trait Asset {
    /// Asset type
    fn asset_type(&self) -> AssetType;

    /// Inputs to the asset
    fn asset_inputs(&self) -> Vec<(String, ADReal)>;
}

/// Trait for any type of asset generator (bootstrapping/stripping engine)
pub trait AssetGenerator {
    /// generates related assets
    fn generate_assets(&self, ctx: &ContextManager) -> Vec<(MarketIndex, AssetType)>;
}
