//! Source observations and dependencies for component advancement.

use super::configuration::ForwardMarketConfiguration;
use crate::{
    core::pricingcontext::PricingContext,
    utils::errors::{QSError, Result},
};

/// Borrowed source market for component-level advancement.
///
/// The context must have its required curves constructed. The high-level
/// [`PricingContext::advance_to`] method constructs these automatically without
/// generating source Monte Carlo paths.
pub struct ForwardMarketSource<'a> {
    context: &'a PricingContext,
    horizon: Option<&'a PricingContext>,
}

impl<'a> ForwardMarketSource<'a> {
    /// Binds component advancement to a source market.
    #[must_use]
    pub const fn new(context: &'a PricingContext) -> Self {
        Self {
            context,
            horizon: None,
        }
    }

    /// Supplies the temporary horizon curves used to generate native quotes.
    #[must_use]
    pub const fn with_horizon_market(mut self, market: &'a PricingContext) -> Self {
        self.horizon = Some(market);
        self
    }

    /// Validates assumptions and rejects horizons before the source market.
    ///
    /// # Errors
    /// Returns an error for invalid assumptions or a backward horizon.
    pub fn validate_date(&self, configuration: &ForwardMarketConfiguration) -> Result<()> {
        configuration.validate()?;
        if configuration.target_date() < self.context.evaluation_date() {
            return Err(QSError::InvalidValueErr(
                "Cannot advance market inputs backwards".into(),
            ));
        }
        Ok(())
    }

    /// Returns the temporary horizon market, when one is bound.
    #[must_use]
    pub const fn horizon_market(&self) -> Option<&'a PricingContext> {
        self.horizon
    }

    /// Returns the source pricing context.
    #[must_use]
    pub const fn context(&self) -> &'a PricingContext {
        self.context
    }
}
