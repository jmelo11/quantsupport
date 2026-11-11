//! Component advancement contract.

use super::{configuration::ForwardMarketConfiguration, source::ForwardMarketSource};
use crate::utils::errors::Result;

/// A component that can produce independent market inputs at a future date.
pub trait AdvanceInTime {
    /// Advanced component, input definition, or pricing context.
    type Output;

    /// Advances this component using declarative assumptions and a source market.
    ///
    /// # Errors
    /// Returns an error for invalid dates, missing dependencies, unsupported
    /// quotation conventions, or failed construction.
    fn advance_in_time(
        &self,
        configuration: &ForwardMarketConfiguration,
        source: &ForwardMarketSource<'_>,
    ) -> Result<Self::Output>;
}
