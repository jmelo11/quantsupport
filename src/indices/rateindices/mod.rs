use crate::indices::{marketindex::MarketIndex, rateindex::RateIndexDetails};

/// Sofr index implementation.
pub mod sofr;

/// Returns rate index details for the given market index.
#[must_use]
pub fn rate_index_details(market_index: &MarketIndex) -> Option<Box<dyn RateIndexDetails>> {
    match market_index {
        MarketIndex::SOFR => Some(Box::new(sofr::SOFRIndex)),
        _ => None,
    }
}
