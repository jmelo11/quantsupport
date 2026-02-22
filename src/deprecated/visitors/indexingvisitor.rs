use std::cell::RefCell;

use super::traits::{HasCashflows, Visit};
use crate::{
    core::{meta::MarketRequest, traits::Registrable},
    utils::errors::Result,
};

/// # `IndexingVisitor`
/// `IndexingVisitor` is a visitor that registers the cashflows of an instrument
/// and generates a vector of market requests.
pub struct IndexingVisitor {
    request: RefCell<Vec<MarketRequest>>,
}

impl IndexingVisitor {
    /// Creates a new `IndexingVisitor` instance.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn new() -> Self {
        Self {
            request: RefCell::new(Vec::new()),
        }
    }

    /// Returns a clone of the collected market requests.
    #[must_use]
    pub fn request(&self) -> Vec<MarketRequest> {
        self.request.borrow().clone()
    }
}

impl Default for IndexingVisitor {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: HasCashflows> Visit<T> for IndexingVisitor {
    type Output = Result<()>;
    fn visit(&self, has_cashflows: &mut T) -> Self::Output {
        let mut requests = self.request.borrow_mut();
        has_cashflows
            .mut_cashflows()
            .iter_mut()
            .try_for_each(|cf| -> Result<()> {
                cf.set_id(requests.len());
                let request = cf.market_request()?;
                requests.push(request);
                Ok(())
            })?;
        Ok(())
    }
}
