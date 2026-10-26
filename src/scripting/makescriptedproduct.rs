//! Reusable scripted products with absolute event dates or event tenors.

use crate::{
    core::resolution::{ResolutionContext, Resolve},
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    scripting::{
        nodes::event::{CodedEvent, EventStream},
        product::ScriptedProduct,
    },
    time::{date::Date, period::Period},
    utils::errors::{QSError, Result},
};

/// Defines script events before a reference date and market are available.
///
/// Event tenors are measured from the resolution reference date. Event order
/// is preserved; the script compiler validates chronological execution order.
#[derive(Clone, Default)]
pub struct MakeScriptedProduct {
    identifier: Option<String>,
    reference_date: Option<Date>,
    currency: Option<Currency>,
    discount_index: Option<MarketIndex>,
    events: Vec<CodedEvent>,
    event_tenors: Vec<(Period, String)>,
}

impl MakeScriptedProduct {
    /// Sets the product identifier.
    #[must_use]
    pub fn with_identifier(mut self, identifier: String) -> Self {
        self.identifier = Some(identifier);
        self
    }

    /// Uses an explicit reference date instead of the context reference date.
    #[must_use]
    pub const fn with_reference_date(mut self, date: Date) -> Self {
        self.reference_date = Some(date);
        self
    }

    /// Sets the default payment currency.
    #[must_use]
    pub const fn with_currency(mut self, currency: Currency) -> Self {
        self.currency = Some(currency);
        self
    }

    /// Sets the default discount index for script payments.
    #[must_use]
    pub fn with_discount_index(mut self, index: MarketIndex) -> Self {
        self.discount_index = Some(index);
        self
    }

    /// Replaces the event schedule with explicitly dated source code.
    #[must_use]
    pub fn with_events(mut self, events: Vec<CodedEvent>) -> Self {
        self.events = events;
        self.event_tenors.clear();
        self
    }

    /// Replaces the event schedule with reference-relative tenors and source code.
    #[must_use]
    pub fn with_event_tenors(mut self, events: Vec<(Period, String)>) -> Self {
        self.event_tenors = events;
        self.events.clear();
        self
    }

    /// Compiles a script with an explicit reference date and event dates.
    ///
    /// # Errors
    /// Rejects missing terms, unresolved event tenors, or invalid script code.
    pub fn build(self) -> Result<ScriptedProduct> {
        if !self.event_tenors.is_empty() {
            return Err(QSError::InvalidValueErr(
                "Relative events require maker.resolve(&ResolutionContext)".into(),
            ));
        }
        ScriptedProduct::new(
            required(self.identifier, "Script identifier")?,
            EventStream::try_from(self.events)?,
            required(self.reference_date, "Script reference date")?,
            required(self.currency, "Script currency")?,
            required(self.discount_index, "Script discount index")?,
        )
    }
}

impl Resolve for MakeScriptedProduct {
    type Resolved = ScriptedProduct;

    fn resolve(&self, context: &ResolutionContext<'_>) -> Result<Self::Resolved> {
        let mut resolved = self.clone();
        let reference = self
            .reference_date
            .unwrap_or_else(|| context.reference_date());
        resolved.reference_date = Some(reference);
        if !self.event_tenors.is_empty() {
            resolved.events = self
                .event_tenors
                .iter()
                .map(|(tenor, source)| {
                    Ok(CodedEvent::new(
                        context.advance_date(reference, *tenor)?,
                        source.clone(),
                    ))
                })
                .collect::<Result<Vec<_>>>()?;
            resolved.event_tenors.clear();
        }
        resolved.build()
    }
}

fn required<T>(value: Option<T>, label: &str) -> Result<T> {
    value.ok_or_else(|| QSError::ValueNotSetErr(label.into()))
}

#[cfg(test)]
mod tests {
    use crate::{
        core::resolution::test_support::{empty_market, p, reference},
        prelude::*,
    };

    #[test]
    fn script_event_tenors_compile_against_each_context_reference_date() -> Result<()> {
        let market = empty_market(reference());
        let ctx = ResolutionContext::new(&market);
        let script = MakeScriptedProduct::default()
            .with_identifier("script".into())
            .with_currency(Currency::EUR)
            .with_discount_index(MarketIndex::ESTR)
            .with_event_tenors(vec![(p("1Y")?, "value = 0; value pays 100;".into())]);
        let claims = script.resolve(&ctx)?.contingent_claims()?;
        assert_eq!(claims.len(), 1);
        assert_eq!(claims[0].payment_date(), reference() + p("1Y")?);
        let later = ctx.with_reference_date(Date::new(2027, 3, 25));
        let claims = script.resolve(&later)?.contingent_claims()?;
        assert_eq!(claims[0].payment_date(), Date::new(2028, 3, 25));
        Ok(())
    }
}
