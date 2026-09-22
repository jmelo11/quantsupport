//! Revisioned handles for actual QuantSupport objects.
//!
//! Excel cells can only carry scalar values, so the registry owns library
//! components. A handle such as `QSObject:QuoteStore:usd:4` is both an object
//! reference and a dependency token; mutations return the next revision.

use std::{cell::RefCell, collections::HashMap, error::Error, fmt};

use quantsupport::prelude::{
    BasisSwapTrade, CapFloorTrade, CapletFloorletTrade, CdsTrade, ConstructedElementStore,
    CreditCurveConfiguration, CurveConfiguration, DiscountCurveElement, DualFwd,
    EquityEuropeanOptionTrade, FixFloatCrossCurrencySwapTrade, FixedRateBondTrade,
    FixedRateDepositTrade, FixingStore, FloatFloatCrossCurrencySwapTrade, FloatingRateNoteTrade,
    FxEuropeanOptionTrade, FxForwardTrade, FxStore, PricingContext, QuoteStore, RateFuturesTrade,
    Scenario, SimulationConfiguration, SwapTrade, VolatilityCubeConfiguration,
    VolatilitySurfaceConfiguration,
};

const HANDLE_PREFIX: &str = "QSObject";
const MAX_OBJECT_NAME_UTF16: usize = 128;

/// QuantSupport components and trades that can be referenced by an Excel cell.
pub enum QsObject {
    /// Market quote collection.
    QuoteStore(Box<QuoteStore>),
    /// Historical fixing collection.
    FixingStore(Box<FixingStore>),
    /// FX spot-rate collection.
    FxStore(Box<FxStore>),
    /// Curve bootstrap configurations.
    CurveConfigurations(Vec<CurveConfiguration>),
    /// Credit-curve bootstrap configurations.
    CreditCurveConfigurations(Vec<CreditCurveConfiguration>),
    /// Volatility-surface configurations.
    VolatilitySurfaceConfigurations(Vec<VolatilitySurfaceConfiguration>),
    /// Volatility-cube configurations.
    VolatilityCubeConfigurations(Vec<VolatilityCubeConfiguration>),
    /// Monte Carlo simulation configurations.
    SimulationConfigurations(Vec<SimulationConfiguration>),
    /// Quote scenarios.
    Scenarios(Vec<Scenario>),
    /// A constructed discount curve.
    DiscountCurve(Box<DiscountCurveElement>),
    /// A collection of constructed market-data elements.
    ConstructedElements(Box<ConstructedElementStore>),
    /// An initialized pricing context.
    PricingContext(Box<PricingContext>),
    /// Deliverable or non-deliverable FX-forward trade.
    FxForwardTrade(Box<FxForwardTrade>),
    /// Vanilla fixed/floating swap trade.
    SwapTrade(Box<SwapTrade<DualFwd>>),
    /// Same-currency floating/floating basis swap trade.
    BasisSwapTrade(Box<BasisSwapTrade<DualFwd>>),
    /// Floating/floating cross-currency swap trade.
    FloatFloatCrossCurrencySwapTrade(Box<FloatFloatCrossCurrencySwapTrade<DualFwd>>),
    /// Fixed/floating cross-currency swap trade.
    FixFloatCrossCurrencySwapTrade(Box<FixFloatCrossCurrencySwapTrade<DualFwd>>),
    /// Fixed-rate bond trade.
    FixedRateBondTrade(Box<FixedRateBondTrade<DualFwd>>),
    /// Fixed-rate deposit trade.
    FixedRateDepositTrade(Box<FixedRateDepositTrade<DualFwd>>),
    /// Floating-rate note trade.
    FloatingRateNoteTrade(Box<FloatingRateNoteTrade<DualFwd>>),
    /// FX European option trade.
    FxEuropeanOptionTrade(Box<FxEuropeanOptionTrade>),
    /// Equity European option trade.
    EquityEuropeanOptionTrade(Box<EquityEuropeanOptionTrade>),
    /// Credit-default-swap trade.
    CdsTrade(Box<CdsTrade>),
    /// Cap/floor strip trade.
    CapFloorTrade(Box<CapFloorTrade>),
    /// Single-period caplet/floorlet trade.
    CapletFloorletTrade(Box<CapletFloorletTrade>),
    /// Exchange-traded interest-rate future.
    RateFuturesTrade(Box<RateFuturesTrade>),
}

impl QsObject {
    /// Stable type tag embedded in object handles.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::QuoteStore(_) => "QuoteStore",
            Self::FixingStore(_) => "FixingStore",
            Self::FxStore(_) => "FxStore",
            Self::CurveConfigurations(_) => "CurveConfigurations",
            Self::CreditCurveConfigurations(_) => "CreditCurveConfigurations",
            Self::VolatilitySurfaceConfigurations(_) => "VolatilitySurfaceConfigurations",
            Self::VolatilityCubeConfigurations(_) => "VolatilityCubeConfigurations",
            Self::SimulationConfigurations(_) => "SimulationConfigurations",
            Self::Scenarios(_) => "Scenarios",
            Self::DiscountCurve(_) => "DiscountCurveElement",
            Self::ConstructedElements(_) => "ConstructedElementStore",
            Self::PricingContext(_) => "PricingContext",
            Self::FxForwardTrade(_) => "FxForwardTrade",
            Self::SwapTrade(_) => "SwapTrade",
            Self::BasisSwapTrade(_) => "BasisSwapTrade",
            Self::FloatFloatCrossCurrencySwapTrade(_) => "FloatFloatCrossCurrencySwapTrade",
            Self::FixFloatCrossCurrencySwapTrade(_) => "FixFloatCrossCurrencySwapTrade",
            Self::FixedRateBondTrade(_) => "FixedRateBondTrade",
            Self::FixedRateDepositTrade(_) => "FixedRateDepositTrade",
            Self::FloatingRateNoteTrade(_) => "FloatingRateNoteTrade",
            Self::FxEuropeanOptionTrade(_) => "FxEuropeanOptionTrade",
            Self::EquityEuropeanOptionTrade(_) => "EquityEuropeanOptionTrade",
            Self::CdsTrade(_) => "CdsTrade",
            Self::CapFloorTrade(_) => "CapFloorTrade",
            Self::CapletFloorletTrade(_) => "CapletFloorletTrade",
            Self::RateFuturesTrade(_) => "RateFuturesTrade",
        }
    }
}

struct RegistryEntry {
    revision: u64,
    object: QsObject,
}

/// Error returned when an object handle cannot be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    /// The handle did not have the expected shape.
    InvalidHandle(String),
    /// The caller supplied an invalid object name.
    InvalidName(String),
    /// No live object exists for the handle identity.
    NotFound(String),
    /// The supplied revision is older than the stored object.
    StaleHandle {
        /// Handle supplied by the worksheet.
        supplied: String,
        /// Current handle to use instead.
        current: String,
    },
    /// The requested typed accessor does not match the object.
    WrongObjectType {
        /// Type required by the operation.
        expected: String,
        /// Actual stored type.
        actual: &'static str,
    },
    /// An object's revision counter could not be incremented.
    RevisionOverflow(String),
}

impl fmt::Display for RegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHandle(handle) => write!(
                formatter,
                "invalid object handle '{handle}'; expected QSObject:<type>:<name>:<revision>"
            ),
            Self::InvalidName(name) => write!(
                formatter,
                "invalid object name '{name}'; names must be non-empty, at most \
                 {MAX_OBJECT_NAME_UTF16} UTF-16 units, and cannot contain ':'"
            ),
            Self::NotFound(handle) => write!(formatter, "object '{handle}' does not exist"),
            Self::StaleHandle { supplied, current } => write!(
                formatter,
                "stale object handle '{supplied}'; current handle is '{current}'"
            ),
            Self::WrongObjectType { expected, actual } => {
                write!(formatter, "expected {expected}, found {actual}")
            }
            Self::RevisionOverflow(handle) => {
                write!(formatter, "revision counter overflow for '{handle}'")
            }
        }
    }
}

impl Error for RegistryError {}

/// Registry of live objects for the current Excel calculation thread.
#[derive(Default)]
pub struct Registry {
    objects: HashMap<String, RegistryEntry>,
}

macro_rules! accessor {
    ($name:ident, $variant:ident, $type:ty, $label:literal) => {
        pub fn $name(&self, handle: &str) -> Result<&$type, RegistryError> {
            match &self.entry(handle)?.object {
                QsObject::$variant(value) => Ok(value.as_ref()),
                object => Err(wrong_type($label, object)),
            }
        }
    };
}

macro_rules! collection_accessor {
    ($name:ident, $variant:ident, $type:ty, $label:literal) => {
        pub fn $name(&self, handle: &str) -> Result<&Vec<$type>, RegistryError> {
            match &self.entry(handle)?.object {
                QsObject::$variant(value) => Ok(value),
                object => Err(wrong_type($label, object)),
            }
        }
    };
}

impl Registry {
    /// Inserts or replaces a named object and returns its revisioned handle.
    pub fn upsert(&mut self, name: &str, object: QsObject) -> Result<String, RegistryError> {
        let name = validate_name(name)?;
        let identity = identity(object.kind(), name);
        let revision = self
            .objects
            .get(&identity)
            .map_or(Ok(1), |entry| next_revision(entry.revision, &identity))?;
        self.objects
            .insert(identity.clone(), RegistryEntry { revision, object });
        Ok(format_handle(&identity, revision))
    }

    accessor!(quote_store, QuoteStore, QuoteStore, "QuoteStore");
    accessor!(fixing_store, FixingStore, FixingStore, "FixingStore");
    accessor!(fx_store, FxStore, FxStore, "FxStore");
    collection_accessor!(
        curve_configurations,
        CurveConfigurations,
        CurveConfiguration,
        "CurveConfigurations"
    );
    collection_accessor!(
        credit_curve_configurations,
        CreditCurveConfigurations,
        CreditCurveConfiguration,
        "CreditCurveConfigurations"
    );
    collection_accessor!(
        volatility_surface_configurations,
        VolatilitySurfaceConfigurations,
        VolatilitySurfaceConfiguration,
        "VolatilitySurfaceConfigurations"
    );
    collection_accessor!(
        volatility_cube_configurations,
        VolatilityCubeConfigurations,
        VolatilityCubeConfiguration,
        "VolatilityCubeConfigurations"
    );
    collection_accessor!(
        simulation_configurations,
        SimulationConfigurations,
        SimulationConfiguration,
        "SimulationConfigurations"
    );
    collection_accessor!(scenarios, Scenarios, Scenario, "Scenarios");
    accessor!(
        discount_curve,
        DiscountCurve,
        DiscountCurveElement,
        "DiscountCurveElement"
    );
    accessor!(
        constructed_elements,
        ConstructedElements,
        ConstructedElementStore,
        "ConstructedElementStore"
    );
    accessor!(
        pricing_context,
        PricingContext,
        PricingContext,
        "PricingContext"
    );

    /// Mutates a quote store and returns the operation result and new handle.
    pub fn update_quote_store<R>(
        &mut self,
        handle: &str,
        operation: impl FnOnce(&mut QuoteStore) -> R,
    ) -> Result<(R, String), RegistryError> {
        self.mutate(handle, |object| match object {
            QsObject::QuoteStore(store) => Ok(operation(store.as_mut())),
            other => Err(wrong_type("QuoteStore", other)),
        })
    }

    /// Mutates a fixing store and returns the operation result and new handle.
    pub fn update_fixing_store<R>(
        &mut self,
        handle: &str,
        operation: impl FnOnce(&mut FixingStore) -> R,
    ) -> Result<(R, String), RegistryError> {
        self.mutate(handle, |object| match object {
            QsObject::FixingStore(store) => Ok(operation(store.as_mut())),
            other => Err(wrong_type("FixingStore", other)),
        })
    }

    /// Mutates an FX store and returns the operation result and new handle.
    pub fn update_fx_store<R>(
        &mut self,
        handle: &str,
        operation: impl FnOnce(&mut FxStore) -> R,
    ) -> Result<(R, String), RegistryError> {
        self.mutate(handle, |object| match object {
            QsObject::FxStore(store) => Ok(operation(store.as_mut())),
            other => Err(wrong_type("FxStore", other)),
        })
    }

    /// Mutates a constructed-element store and returns its new handle.
    pub fn update_constructed_elements<R>(
        &mut self,
        handle: &str,
        operation: impl FnOnce(&mut ConstructedElementStore) -> R,
    ) -> Result<(R, String), RegistryError> {
        self.mutate(handle, |object| match object {
            QsObject::ConstructedElements(store) => Ok(operation(store.as_mut())),
            other => Err(wrong_type("ConstructedElementStore", other)),
        })
    }

    /// Retrieves any object by a current handle for pricer dispatch.
    pub fn object(&self, handle: &str) -> Result<&QsObject, RegistryError> {
        Ok(&self.entry(handle)?.object)
    }

    /// Returns the type tag of a live object.
    pub fn object_type(&self, handle: &str) -> Result<&'static str, RegistryError> {
        Ok(self.entry(handle)?.object.kind())
    }

    /// Removes an object if the supplied handle is current.
    pub fn remove(&mut self, handle: &str) -> Result<bool, RegistryError> {
        let parsed = ParsedHandle::parse(handle)?;
        let identity = parsed.identity();
        let Some(entry) = self.objects.get(&identity) else {
            return Ok(false);
        };
        ensure_current(handle, &identity, parsed.revision, entry.revision)?;
        Ok(self.objects.remove(&identity).is_some())
    }

    /// Returns the number of live object identities.
    #[must_use]
    pub fn len(&self) -> usize {
        self.objects.len()
    }

    /// Drops every live object.
    pub fn clear(&mut self) {
        self.objects.clear();
    }

    fn entry(&self, handle: &str) -> Result<&RegistryEntry, RegistryError> {
        let parsed = ParsedHandle::parse(handle)?;
        let identity = parsed.identity();
        let entry = self
            .objects
            .get(&identity)
            .ok_or_else(|| RegistryError::NotFound(handle.to_string()))?;
        if parsed.kind != entry.object.kind() {
            return Err(wrong_type(parsed.kind, &entry.object));
        }
        ensure_current(handle, &identity, parsed.revision, entry.revision)?;
        Ok(entry)
    }

    fn mutate<R>(
        &mut self,
        handle: &str,
        operation: impl FnOnce(&mut QsObject) -> Result<R, RegistryError>,
    ) -> Result<(R, String), RegistryError> {
        let parsed = ParsedHandle::parse(handle)?;
        let identity = parsed.identity();
        let entry = self
            .objects
            .get_mut(&identity)
            .ok_or_else(|| RegistryError::NotFound(handle.to_string()))?;
        if parsed.kind != entry.object.kind() {
            return Err(wrong_type(parsed.kind, &entry.object));
        }

        // Excel can recalculate a mutation using its original upstream handle.
        // Apply it to the current identity and emit a fresh token. Reads remain
        // strict, so stale dependencies cannot silently price old state.
        let result = operation(&mut entry.object)?;
        entry.revision = next_revision(entry.revision, &identity)?;
        Ok((result, format_handle(&identity, entry.revision)))
    }
}

struct ParsedHandle<'handle> {
    kind: &'handle str,
    name: &'handle str,
    revision: u64,
}

impl<'handle> ParsedHandle<'handle> {
    fn parse(handle: &'handle str) -> Result<Self, RegistryError> {
        let mut parts = handle.split(':');
        let prefix = parts.next();
        let kind = parts.next();
        let name = parts.next();
        let revision = parts.next().and_then(|value| value.parse::<u64>().ok());
        let has_extra_parts = parts.next().is_some();

        match (prefix, kind, name, revision, has_extra_parts) {
            (Some(HANDLE_PREFIX), Some(kind), Some(name), Some(revision), false)
                if !kind.is_empty() && !name.is_empty() && revision > 0 =>
            {
                Ok(Self {
                    kind,
                    name,
                    revision,
                })
            }
            _ => Err(RegistryError::InvalidHandle(handle.to_string())),
        }
    }

    fn identity(&self) -> String {
        identity(self.kind, self.name)
    }
}

fn identity(kind: &str, name: &str) -> String {
    format!("{HANDLE_PREFIX}:{kind}:{name}")
}

fn format_handle(identity: &str, revision: u64) -> String {
    format!("{identity}:{revision}")
}

fn ensure_current(
    supplied: &str,
    identity: &str,
    supplied_revision: u64,
    current_revision: u64,
) -> Result<(), RegistryError> {
    if supplied_revision == current_revision {
        Ok(())
    } else {
        Err(RegistryError::StaleHandle {
            supplied: supplied.to_string(),
            current: format_handle(identity, current_revision),
        })
    }
}

fn next_revision(revision: u64, identity: &str) -> Result<u64, RegistryError> {
    revision
        .checked_add(1)
        .ok_or_else(|| RegistryError::RevisionOverflow(identity.to_string()))
}

fn validate_name(name: &str) -> Result<&str, RegistryError> {
    let name = name.trim();
    if name.is_empty() || name.contains(':') || name.encode_utf16().count() > MAX_OBJECT_NAME_UTF16
    {
        Err(RegistryError::InvalidName(name.to_string()))
    } else {
        Ok(name)
    }
}

fn wrong_type(expected: &str, object: &QsObject) -> RegistryError {
    RegistryError::WrongObjectType {
        expected: expected.to_string(),
        actual: object.kind(),
    }
}

thread_local! {
    static REGISTRY: RefCell<Registry> = RefCell::new(Registry::default());
}

/// Borrows the current thread's registry.
pub fn with_registry<R>(operation: impl FnOnce(&Registry) -> R) -> R {
    REGISTRY.with(|registry| operation(&registry.borrow()))
}

/// Mutably borrows the current thread's registry.
pub fn with_registry_mut<R>(operation: impl FnOnce(&mut Registry) -> R) -> R {
    REGISTRY.with(|registry| operation(&mut registry.borrow_mut()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    use quantsupport::prelude::{Currency, Date, Quote, QuoteDetails, QuoteLevels};

    #[test]
    fn named_upsert_reuses_identity_and_increments_revision() {
        let mut registry = Registry::default();
        let first = registry
            .upsert("fx", QsObject::FxStore(Box::new(FxStore::new())))
            .expect("valid name");
        let second = registry
            .upsert("fx", QsObject::FxStore(Box::new(FxStore::new())))
            .expect("valid name");

        assert_eq!(first, "QSObject:FxStore:fx:1");
        assert_eq!(second, "QSObject:FxStore:fx:2");
        assert_eq!(registry.len(), 1);
        assert!(matches!(
            registry.fx_store(&first),
            Err(RegistryError::StaleHandle { .. })
        ));
    }

    #[test]
    fn quote_recalculation_uses_old_input_and_returns_new_token() {
        let mut registry = Registry::default();
        let created = registry
            .upsert("fx", QsObject::FxStore(Box::new(FxStore::new())))
            .expect("valid name");
        let (_, first_update) = registry
            .update_fx_store(&created, |store| {
                store.add_fx_rate(Currency::EUR, Currency::USD, DualFwd::from(1.1));
            })
            .expect("first update");
        let (_, recalculated) = registry
            .update_fx_store(&created, |store| {
                store.add_fx_rate(Currency::EUR, Currency::USD, DualFwd::from(1.2));
            })
            .expect("Excel may recalculate with the original handle");

        assert_eq!(first_update, "QSObject:FxStore:fx:2");
        assert_eq!(recalculated, "QSObject:FxStore:fx:3");
        assert!(registry.fx_store(&first_update).is_err());
        let rate = registry
            .fx_store(&recalculated)
            .expect("new handle")
            .get_fx_rate(Currency::EUR, Currency::USD)
            .expect("updated rate")
            .value();
        assert!((rate - 1.2).abs() < f64::EPSILON);
    }

    #[test]
    fn quote_update_revises_the_real_quote_store() {
        let mut registry = Registry::default();
        let created = registry
            .upsert(
                "quotes",
                QsObject::QuoteStore(Box::new(QuoteStore::new(Date::new(2026, 1, 2)))),
            )
            .expect("valid name");
        let details = QuoteDetails::from_str("OIS_USD_SOFR_1Y").expect("valid quote identifier");
        let (_, first_update) = registry
            .update_quote_store(&created, |store| {
                store.add_quote(Quote::new(details.clone(), QuoteLevels::with_mid(0.03)));
            })
            .expect("first quote update");
        let (_, second_update) = registry
            .update_quote_store(&created, |store| {
                store.add_quote(Quote::new(details, QuoteLevels::with_mid(0.04)));
            })
            .expect("recalculated quote update");

        assert!(registry.quote_store(&first_update).is_err());
        let quote = registry
            .quote_store(&second_update)
            .expect("current quote store")
            .quote("OIS_USD_SOFR_1Y")
            .expect("updated quote");
        assert_eq!(quote.levels().mid(), Some(0.04));
    }

    #[test]
    fn rejects_malformed_handles_and_names() {
        let mut registry = Registry::default();
        assert!(registry
            .upsert("", QsObject::FxStore(Box::new(FxStore::new())))
            .is_err());
        assert!(registry
            .upsert("bad:name", QsObject::FxStore(Box::new(FxStore::new())))
            .is_err());
        assert!(matches!(
            registry.object_type("not-a-handle"),
            Err(RegistryError::InvalidHandle(_))
        ));
    }

    #[test]
    fn remove_requires_the_current_revision() {
        let mut registry = Registry::default();
        let first = registry
            .upsert("fx", QsObject::FxStore(Box::new(FxStore::new())))
            .expect("valid name");
        let second = registry
            .upsert("fx", QsObject::FxStore(Box::new(FxStore::new())))
            .expect("valid name");

        assert!(registry.remove(&first).is_err());
        assert!(registry.remove(&second).expect("current handle"));
        assert_eq!(registry.len(), 0);
    }
}
