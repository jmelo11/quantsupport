//! Portable registry operations for generated horizon pricing contexts.

use std::rc::Rc;

use quantsupport::core::forwardmarket::configuration::ForwardMarketConfiguration;

use super::registry::{QsObject, Registry};

/// Registry-bound operations for horizon configuration and context generation.
pub struct ForwardMarketRuntime<'a> {
    registry: &'a mut Registry,
}

impl<'a> ForwardMarketRuntime<'a> {
    /// Binds forward-market operations to a workbook registry.
    pub fn new(registry: &'a mut Registry) -> Self {
        Self { registry }
    }

    /// Stores declarative forward assumptions with revision tracking.
    pub fn configuration(
        &mut self,
        name: &str,
        configuration: ForwardMarketConfiguration,
    ) -> Result<String, String> {
        let key = serde_json::to_string(&configuration).map_err(|error| error.to_string())?;
        self.registry
            .upsert_if_changed(
                name,
                QsObject::ForwardMarketConfiguration(Box::new(configuration)),
                key,
            )
            .map_err(|error| error.to_string())
    }

    /// Generates a horizon context when the source or assumptions change.
    pub fn pricing_context(
        &mut self,
        name: &str,
        source: &str,
        configuration: &str,
    ) -> Result<String, String> {
        let settings = self
            .registry
            .forward_market_configuration(configuration)
            .map_err(|error| error.to_string())?
            .clone();
        let revision = super::context::revision_key(self.registry, source)?;
        let key = serde_json::to_string(&(source, revision, &settings))
            .map_err(|error| error.to_string())?;
        if let Some(handle) = self
            .registry
            .handle_if_unchanged(name, "PricingContext", &key)
            .map_err(|error| error.to_string())?
        {
            return Ok(handle);
        }
        let source = super::context::resolve(self.registry, source)?;
        let future = source
            .advance_to(&settings)
            .map_err(|error| error.to_string())?;
        self.registry
            .upsert_if_changed(name, QsObject::PricingContext(Rc::new(future)), key)
            .map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quantsupport::{
        core::pricingcontext::PricingContext, quotes::quotestore::QuoteStore, time::date::Date,
    };

    #[test]
    fn horizon_context_reuses_identical_inputs_and_tracks_configuration_changes() {
        let mut registry = Registry::default();
        let today = Date::new(2026, 1, 5);
        let source = registry
            .upsert_if_changed(
                "forward_source",
                QsObject::PricingContext(Rc::new(
                    PricingContext::new().with_quote_store(QuoteStore::new(today)),
                )),
                "source",
            )
            .unwrap();
        let settings = ForwardMarketRuntime::new(&mut registry)
            .configuration(
                "forward_settings",
                ForwardMarketConfiguration::new(today + 30),
            )
            .unwrap();
        let first = ForwardMarketRuntime::new(&mut registry)
            .pricing_context("forward_result", &source, &settings)
            .unwrap();
        let same = ForwardMarketRuntime::new(&mut registry)
            .pricing_context("forward_result", &source, &settings)
            .unwrap();
        assert_eq!(first, same);
        assert_eq!(
            registry.pricing_context(&first).unwrap().evaluation_date(),
            today + 30
        );
        let settings = ForwardMarketRuntime::new(&mut registry)
            .configuration(
                "forward_settings",
                ForwardMarketConfiguration::new(today + 60),
            )
            .unwrap();
        let changed = ForwardMarketRuntime::new(&mut registry)
            .pricing_context("forward_result", &source, &settings)
            .unwrap();
        assert_ne!(first, changed);
        assert_eq!(
            registry
                .pricing_context(&changed)
                .unwrap()
                .evaluation_date(),
            today + 60
        );
    }
}
