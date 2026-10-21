//! Revisioned handles for actual QuantSupport objects.
//!
//! Excel cells can only carry scalar values, so the registry owns library
//! components. A handle such as `QSObject:QuoteStore:usd:4` is both an object
//! reference and a dependency token; mutations return the next revision.

use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    error::Error,
    fmt,
};

use quantsupport::prelude::{
    BasisSwapTrade, CapFloorTrade, CapletFloorletTrade, CdsTrade, ConstructedElementStore,
    CreditCurveConfiguration, Currency, CurveConfiguration, Date, DiscountCurveElement, DualFwd,
    EquityEuropeanOptionTrade, EquityForwardTrade, EuropeanSwaptionTrade,
    FixFloatCrossCurrencySwapTrade, FixedRateBondTrade, FixedRateDepositTrade, FixingStore,
    FloatFloatCrossCurrencySwapTrade, FloatingRateNoteTrade, FxEuropeanOptionTrade, FxForwardTrade,
    FxStore, MarketIndex, MarketSnapshot, MarketSnapshotStore, PricerConfiguration, PricingContext,
    Quote, QuoteStore, RateFuturesTrade, Scenario, SimulationConfiguration, SnapshotSelector,
    SwapTrade, VolatilityCubeConfiguration, VolatilitySurfaceConfiguration,
};

const HANDLE_PREFIX: &str = "QSObject";
const MAX_OBJECT_NAME_UTF16: usize = 128;
const RESET_OPERATION_ID: &str = "__registry_reset__";

/// QuantSupport components and trades that can be referenced by an Excel cell.
pub enum QsObject {
    /// Native instrument, without position or pricing settings.
    Instrument(Box<super::products::InstrumentObject>),
    /// An explicit native discount policy.
    DiscountPolicy(Box<super::pricing::PolicyObject>),
    /// Configured native pricer dispatch.
    Pricer(Box<super::pricing::PricerObject>),
    /// Results of an explicit native valuation request.
    EvaluationResults(Box<quantsupport::prelude::EvaluationResults>),
    /// Native loan position, independent of pricer settings.
    LoanTrade(Box<quantsupport::prelude::LoanTrade<DualFwd>>),
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
    /// Instrument-level pricer selections and typed model assignments.
    PricerConfigurations(Vec<PricerConfiguration>),
    /// Quote scenarios.
    Scenarios(Vec<Scenario>),
    /// A constructed discount curve.
    DiscountCurve(Box<DiscountCurveElement>),
    /// A collection of constructed market-data elements.
    ConstructedElements(Box<ConstructedElementStore>),
    /// An initialized pricing context.
    PricingContext(Box<PricingContext>),
    /// Immutable historical market inputs.
    MarketSnapshot(Box<MarketSnapshot>),
    /// Complete historical markets indexed by observation timestamp.
    MarketSnapshotStore(Box<MarketSnapshotStore>),
    /// Timestamp selection rule and construction quote side.
    SnapshotSelector(Box<SnapshotSelector>),
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
    /// Equity-forward position.
    EquityForwardTrade(Box<EquityForwardTrade>),
    /// European swaption position.
    EuropeanSwaptionTrade(Box<EuropeanSwaptionTrade<DualFwd>>),
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
            Self::Instrument(value) => value.kind(),
            Self::DiscountPolicy(value) => value.kind,
            Self::Pricer(_) => "Pricer",
            Self::EvaluationResults(_) => "EvaluationResults",
            Self::LoanTrade(_) => "LoanTrade",
            Self::QuoteStore(_) => "QuoteStore",
            Self::FixingStore(_) => "FixingStore",
            Self::FxStore(_) => "FxStore",
            Self::CurveConfigurations(_) => "CurveConfigurations",
            Self::CreditCurveConfigurations(_) => "CreditCurveConfigurations",
            Self::VolatilitySurfaceConfigurations(_) => "VolatilitySurfaceConfigurations",
            Self::VolatilityCubeConfigurations(_) => "VolatilityCubeConfigurations",
            Self::SimulationConfigurations(_) => "SimulationConfigurations",
            Self::PricerConfigurations(_) => "PricerConfigurations",
            Self::Scenarios(_) => "Scenarios",
            Self::DiscountCurve(_) => "DiscountCurveElement",
            Self::ConstructedElements(_) => "ConstructedElementStore",
            Self::PricingContext(_) => "PricingContext",
            Self::MarketSnapshot(_) => "MarketSnapshot",
            Self::MarketSnapshotStore(_) => "MarketSnapshotStore",
            Self::SnapshotSelector(_) => "SnapshotSelector",
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
            Self::EquityForwardTrade(_) => "EquityForwardTrade",
            Self::EuropeanSwaptionTrade(_) => "EuropeanSwaptionTrade",
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
    definition_key: Option<String>,
    dependencies: HashMap<String, u64>,
    mutation_keys: HashMap<String, String>,
    undo: HashMap<String, MutationUndo>,
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
    /// The object is immutable or cannot be reconstructed without its inputs.
    ResetUnsupported(&'static str),
    /// Two worksheet cells attempted to own the same store update.
    DuplicateMutation(String),
    /// A cached object was built from an input that has since changed.
    StaleDependency { handle: String, dependency: String },
}

impl fmt::Display for RegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StaleDependency { handle, dependency } => write!(formatter, "object {handle} must be recalculated because dependency {dependency} changed"),
            Self::DuplicateMutation(operation) => write!(formatter, "store update {operation} is already defined by another cell"),
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
            Self::ResetUnsupported(actual) => write!(
                formatter,
                "object type {actual} cannot be reset; reset the mutable input store or configuration collection instead"
            ),
        }
    }
}

impl Error for RegistryError {}

/// Registry of live objects for the current Excel calculation thread.
#[derive(Default)]
pub struct Registry {
    objects: HashMap<String, RegistryEntry>,
    namespace: u64,
    revision_clock: u64,
    active_cell: Option<CellId>,
    reads: RefCell<HashSet<String>>,
    writes: RefCell<HashSet<String>>,
    mutations: HashSet<(String, String)>,
    cell_roots: HashMap<CellId, HashSet<String>>,
    cell_formulas: HashMap<CellId, String>,
    cell_mutations: HashMap<CellId, HashSet<(String, String)>>,
    references: HashMap<String, usize>,
    mutation_owners: HashMap<(String, String), CellId>,
}

/// Address of a calling worksheet cell within its workbook.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CellId {
    pub sheet: usize,
    pub row: i32,
    pub column: i32,
}

// Only the overwritten observation/configuration is retained, never a copy of
// the whole collection. This keeps storage linear in the number of worksheet inputs.
enum MutationUndo {
    Curve(Option<CurveConfiguration>),
    Credit(Option<CreditCurveConfiguration>),
    Quote {
        identifier: String,
        previous: Option<Quote>,
    },
    Fixing {
        index: MarketIndex,
        date: Date,
        previous: Option<f64>,
    },
    Fx {
        base: Currency,
        quote: Currency,
        previous: Option<DualFwd>,
    },
    Element {
        index: MarketIndex,
        previous: Option<DiscountCurveElement>,
    },
}

impl MutationUndo {
    fn capture(object: &QsObject, operation: &str) -> Option<Self> {
        match object {
            QsObject::QuoteStore(store) => {
                let identifier = operation.strip_prefix("quote:")?.to_string();
                let previous = store.quote(&identifier).cloned();
                Some(Self::Quote {
                    identifier,
                    previous,
                })
            }
            QsObject::FixingStore(store) => {
                let (index, date) = operation.strip_prefix("fixing:")?.rsplit_once(':')?;
                let index = super::inputs::market_index(index).ok()?;
                let date = Date::from_str(date, "%Y-%m-%d").ok()?;
                let previous = store.fixing(&index, date).ok();
                Some(Self::Fixing {
                    index,
                    date,
                    previous,
                })
            }
            QsObject::FxStore(store) => {
                let (base, quote) = operation.strip_prefix("fx:")?.split_once(':')?;
                let base = base.parse().ok()?;
                let quote = quote.parse().ok()?;
                let previous = store.direct_fx_rate(base, quote);
                Some(Self::Fx {
                    base,
                    quote,
                    previous,
                })
            }
            QsObject::ConstructedElements(store) => {
                let index =
                    super::inputs::market_index(operation.strip_prefix("discount_curve:")?).ok()?;
                let previous = store.discount_curves().get(&index).cloned();
                Some(Self::Element { index, previous })
            }
            QsObject::CurveConfigurations(values) => {
                let index = operation.strip_prefix("curve:")?;
                Some(Self::Curve(
                    values
                        .iter()
                        .find(|v| v.market_index().to_string() == index)
                        .cloned(),
                ))
            }
            QsObject::CreditCurveConfigurations(values) => {
                let index = operation.strip_prefix("credit:")?;
                Some(Self::Credit(
                    values
                        .iter()
                        .find(|v| v.market_index().to_string() == index)
                        .cloned(),
                ))
            }
            _ => None,
        }
    }

    fn restore(self, object: &mut QsObject, operation: &str) {
        match (self, object) {
            (
                Self::Quote {
                    identifier,
                    previous,
                },
                QsObject::QuoteStore(store),
            ) => {
                store.remove_quote(&identifier);
                if let Some(previous) = previous {
                    store.add_quote(previous);
                }
            }
            (
                Self::Fixing {
                    index,
                    date,
                    previous,
                },
                QsObject::FixingStore(store),
            ) => {
                store.remove_fixing(&index, date);
                if let Some(previous) = previous {
                    store.add_fixing(&index, date, previous);
                }
            }
            (
                Self::Fx {
                    base,
                    quote,
                    previous,
                },
                QsObject::FxStore(store),
            ) => {
                store.remove_fx_rate(base, quote);
                if let Some(previous) = previous {
                    store.add_fx_rate(base, quote, previous);
                }
            }
            (Self::Element { index, previous }, QsObject::ConstructedElements(store)) => {
                store.discount_curves_mut().remove(&index);
                if let Some(previous) = previous {
                    store.discount_curves_mut().insert(index, previous);
                }
            }
            (Self::Curve(previous), QsObject::CurveConfigurations(values)) => {
                values.retain(|v| format!("curve:{}", v.market_index()) != operation);
                values.extend(previous);
            }
            (Self::Credit(previous), QsObject::CreditCurveConfigurations(values)) => {
                values.retain(|v| format!("credit:{}", v.market_index()) != operation);
                values.extend(previous);
            }
            _ => {}
        }
    }
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
    fn in_workbook(namespace: u64) -> Self {
        Self {
            namespace,
            ..Self::default()
        }
    }

    fn named_identity(&self, kind: &str, name: &str) -> String {
        if self.namespace == 0 {
            identity(kind, name)
        } else {
            identity(kind, &format!("w{}~{name}", self.namespace))
        }
    }

    fn dependencies_current(&self, entry: &RegistryEntry) -> bool {
        entry.dependencies.iter().all(|(identity, revision)| {
            self.objects
                .get(identity)
                .is_some_and(|current| current.revision == *revision)
        })
    }

    fn record_write(&self, identity: &str) {
        if self.active_cell.is_some() {
            self.writes.borrow_mut().insert(identity.to_string());
        }
    }

    pub fn begin_cell(&mut self, cell: CellId) {
        self.active_cell = Some(cell);
        self.reads.borrow_mut().clear();
        self.writes.borrow_mut().clear();
        self.mutations.clear();
    }

    pub fn set_cell_formula(&mut self, cell: CellId, formula: Option<String>) {
        if let Some(formula) = formula {
            self.cell_formulas.insert(cell, formula);
        }
    }

    pub fn release_if_formula_changed(&mut self, cell: CellId, formula: &str, has_formula: bool) {
        if !has_formula
            || self
                .cell_formulas
                .get(&cell)
                .is_some_and(|previous| previous != formula)
        {
            self.release_cell(cell);
        }
    }

    pub fn finish_cell(&mut self, success: bool) {
        let Some(cell) = self.active_cell.take() else {
            return;
        };
        let mutations = if success {
            std::mem::take(&mut self.mutations)
        } else {
            HashSet::new()
        };
        let previous = self.cell_mutations.remove(&cell).unwrap_or_default();
        for key in previous.difference(&mutations) {
            self.undo_mutation(key);
        }
        // Also undo a partial operation if a later step of the formula failed.
        if !success {
            for key in std::mem::take(&mut self.mutations) {
                self.undo_mutation(&key);
            }
        }
        if !mutations.is_empty() {
            self.cell_mutations.insert(cell, mutations);
        }
        let mut roots = std::mem::take(self.reads.get_mut());
        let writes = std::mem::take(self.writes.get_mut());
        if success {
            roots.extend(writes.iter().cloned());
        } else {
            roots.clear();
        }
        let previous = self.cell_roots.remove(&cell).unwrap_or_default();
        for identity in roots.difference(&previous) {
            *self.references.entry(identity.clone()).or_default() += 1;
        }
        for identity in previous.difference(&roots) {
            self.release(identity);
        }
        for identity in writes.difference(&roots) {
            if !self.references.contains_key(identity) {
                self.objects.remove(identity);
            }
        }
        if !roots.is_empty() {
            self.cell_roots.insert(cell, roots);
        } else {
            self.cell_formulas.remove(&cell);
        }
    }

    fn undo_mutation(&mut self, key: &(String, String)) {
        self.mutation_owners.remove(key);
        if let Some(entry) = self.objects.get_mut(&key.0) {
            if let Some(undo) = entry.undo.remove(&key.1) {
                undo.restore(&mut entry.object, &key.1);
                entry.mutation_keys.remove(&key.1);
                // A revision change prevents old contexts/results from being reused.
                self.revision_clock = self.revision_clock.saturating_add(1);
                entry.revision = self.revision_clock;
            }
        }
    }

    fn release(&mut self, identity: &str) {
        if let Some(count) = self.references.get_mut(identity) {
            *count -= 1;
            if *count == 0 {
                self.references.remove(identity);
                self.objects.remove(identity);
            }
        }
    }

    pub fn release_cell(&mut self, cell: CellId) {
        self.cell_formulas.remove(&cell);
        if let Some(mutations) = self.cell_mutations.remove(&cell) {
            for key in mutations {
                self.undo_mutation(&key);
            }
        }
        if let Some(roots) = self.cell_roots.remove(&cell) {
            for identity in roots {
                self.release(&identity);
            }
        }
    }
}

impl Registry {
    /// Returns the current handle when a constructor's defining inputs match.
    pub fn handle_if_unchanged(
        &self,
        name: &str,
        kind: &str,
        definition_key: &str,
    ) -> Result<Option<String>, RegistryError> {
        let name = validate_name(name)?;
        let identity = self.named_identity(kind, name);
        Ok(self.objects.get(&identity).and_then(|entry| {
            if entry.definition_key.as_deref() == Some(definition_key)
                && self.dependencies_current(entry)
            {
                self.record_write(&identity);
                if self.active_cell.is_some() {
                    self.reads
                        .borrow_mut()
                        .extend(entry.dependencies.keys().cloned());
                }
                Some(format_handle(&identity, entry.revision))
            } else {
                None
            }
        }))
    }

    /// Inserts a named object only when its defining worksheet inputs changed.
    ///
    /// Mutations made after construction are retained when Excel recalculates
    /// the constructor with the same key.
    pub fn upsert_if_changed(
        &mut self,
        name: &str,
        object: QsObject,
        definition_key: impl Into<String>,
    ) -> Result<String, RegistryError> {
        let name = validate_name(name)?;
        let identity = self.named_identity(object.kind(), name);
        self.record_write(&identity);
        let definition_key = definition_key.into();
        if let Some(entry) = self.objects.get(&identity) {
            if entry.definition_key.as_deref() == Some(definition_key.as_str())
                && self.dependencies_current(entry)
            {
                if self.active_cell.is_some() {
                    self.reads
                        .borrow_mut()
                        .extend(entry.dependencies.keys().cloned());
                }
                return Ok(format_handle(&identity, entry.revision));
            }
        }

        self.revision_clock = next_revision(self.revision_clock, &identity)?;
        let revision = self.revision_clock;
        let dependencies = self
            .reads
            .borrow()
            .iter()
            .filter(|input| *input != &identity)
            .filter_map(|input| {
                self.objects
                    .get(input)
                    .map(|entry| (input.clone(), entry.revision))
            })
            .collect();
        self.objects.insert(
            identity.clone(),
            RegistryEntry {
                revision,
                object,
                definition_key: Some(definition_key),
                dependencies,
                mutation_keys: HashMap::new(),
                undo: HashMap::new(),
            },
        );
        Ok(format_handle(&identity, revision))
    }

    accessor!(quote_store, QuoteStore, QuoteStore, "QuoteStore");
    accessor!(
        instrument,
        Instrument,
        super::products::InstrumentObject,
        "Instrument"
    );
    accessor!(
        discount_policy,
        DiscountPolicy,
        super::pricing::PolicyObject,
        "DiscountPolicy"
    );
    accessor!(pricer, Pricer, super::pricing::PricerObject, "Pricer");
    accessor!(
        evaluation_results,
        EvaluationResults,
        quantsupport::prelude::EvaluationResults,
        "EvaluationResults"
    );
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
    collection_accessor!(
        pricer_configurations,
        PricerConfigurations,
        PricerConfiguration,
        "PricerConfigurations"
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
    accessor!(
        market_snapshot,
        MarketSnapshot,
        MarketSnapshot,
        "MarketSnapshot"
    );
    accessor!(
        market_snapshot_store,
        MarketSnapshotStore,
        MarketSnapshotStore,
        "MarketSnapshotStore"
    );
    accessor!(
        snapshot_selector,
        SnapshotSelector,
        SnapshotSelector,
        "SnapshotSelector"
    );

    /// Mutates a quote store only when this operation's worksheet inputs changed.
    pub fn update_quote_store_if_changed(
        &mut self,
        handle: &str,
        operation_id: &str,
        value_key: &str,
        operation: impl FnOnce(&mut QuoteStore),
    ) -> Result<String, RegistryError> {
        self.mutate_if_changed(handle, operation_id, value_key, |object| match object {
            QsObject::QuoteStore(store) => {
                operation(store.as_mut());
                Ok(())
            }
            other => Err(wrong_type("QuoteStore", other)),
        })
    }

    /// Mutates a fixing store only when this operation's worksheet inputs changed.
    pub fn update_fixing_store_if_changed(
        &mut self,
        handle: &str,
        operation_id: &str,
        value_key: &str,
        operation: impl FnOnce(&mut FixingStore),
    ) -> Result<String, RegistryError> {
        self.mutate_if_changed(handle, operation_id, value_key, |object| match object {
            QsObject::FixingStore(store) => {
                operation(store.as_mut());
                Ok(())
            }
            other => Err(wrong_type("FixingStore", other)),
        })
    }

    /// Mutates an FX store only when this operation's worksheet inputs changed.
    pub fn update_fx_store_if_changed(
        &mut self,
        handle: &str,
        operation_id: &str,
        value_key: &str,
        operation: impl FnOnce(&mut FxStore),
    ) -> Result<String, RegistryError> {
        self.mutate_if_changed(handle, operation_id, value_key, |object| match object {
            QsObject::FxStore(store) => {
                operation(store.as_mut());
                Ok(())
            }
            other => Err(wrong_type("FxStore", other)),
        })
    }

    /// Mutates curve configurations only when this operation's inputs changed.
    pub fn update_curve_configurations_if_changed(
        &mut self,
        handle: &str,
        operation_id: &str,
        value_key: &str,
        operation: impl FnOnce(&mut Vec<CurveConfiguration>),
    ) -> Result<String, RegistryError> {
        self.mutate_if_changed(handle, operation_id, value_key, |object| match object {
            QsObject::CurveConfigurations(configurations) => {
                operation(configurations);
                Ok(())
            }
            other => Err(wrong_type("CurveConfigurations", other)),
        })
    }

    /// Mutates credit configurations only when the worksheet inputs changed.
    pub fn update_credit_curve_configurations_if_changed(
        &mut self,
        handle: &str,
        operation_id: &str,
        value_key: &str,
        operation: impl FnOnce(&mut Vec<CreditCurveConfiguration>),
    ) -> Result<String, RegistryError> {
        self.mutate_if_changed(handle, operation_id, value_key, |object| match object {
            QsObject::CreditCurveConfigurations(configurations) => {
                operation(configurations);
                Ok(())
            }
            other => Err(wrong_type("CreditCurveConfigurations", other)),
        })
    }

    /// Mutates constructed elements only when this operation's inputs changed.
    pub fn update_constructed_elements_if_changed(
        &mut self,
        handle: &str,
        operation_id: &str,
        value_key: &str,
        operation: impl FnOnce(&mut ConstructedElementStore),
    ) -> Result<String, RegistryError> {
        self.mutate_if_changed(handle, operation_id, value_key, |object| match object {
            QsObject::ConstructedElements(store) => {
                operation(store.as_mut());
                Ok(())
            }
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

    /// Resets a mutable store or configuration collection when `reset_key`
    /// changes and returns its new revisioned handle.
    ///
    /// The reset also clears cached mutation keys so unchanged downstream
    /// worksheet formulas repopulate the fresh object. As with other
    /// mutators, an older revision of the same object identity is accepted;
    /// Excel mutation chains commonly retain their original constructor
    /// handle between recalculations.
    pub fn reset_if_changed(
        &mut self,
        handle: &str,
        reset_key: &str,
    ) -> Result<String, RegistryError> {
        let parsed = ParsedHandle::parse(handle)?;
        let identity = parsed.identity();
        self.record_write(&identity);
        if self.active_cell.is_some() {
            self.reads.borrow_mut().insert(identity.clone());
        }
        let entry = self
            .objects
            .get_mut(&identity)
            .ok_or_else(|| RegistryError::NotFound(handle.to_string()))?;
        if parsed.kind != entry.object.kind() {
            return Err(wrong_type(parsed.kind, &entry.object));
        }
        if entry
            .mutation_keys
            .get(RESET_OPERATION_ID)
            .map(String::as_str)
            == Some(reset_key)
        {
            return Ok(format_handle(&identity, entry.revision));
        }

        self.revision_clock = next_revision(self.revision_clock, &identity)?;
        let revision = self.revision_clock;
        reset_object(&mut entry.object)?;
        entry.revision = revision;
        entry.mutation_keys.clear();
        entry.undo.clear();
        entry
            .mutation_keys
            .insert(RESET_OPERATION_ID.to_string(), reset_key.to_string());
        Ok(format_handle(&identity, entry.revision))
    }

    /// Returns the number of live object identities.
    #[must_use]
    pub fn len(&self) -> usize {
        self.objects.len()
    }

    /// Drops every live object.
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.objects.clear();
        self.cell_roots.clear();
        self.cell_formulas.clear();
        self.cell_mutations.clear();
        self.references.clear();
        self.mutation_owners.clear();
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
        if let Some((dependency, _)) = entry.dependencies.iter().find(|(identity, revision)| {
            self.objects
                .get(*identity)
                .is_none_or(|current| current.revision != **revision)
        }) {
            return Err(RegistryError::StaleDependency {
                handle: handle.to_string(),
                dependency: dependency.clone(),
            });
        }
        if self.active_cell.is_some() {
            self.reads.borrow_mut().insert(identity);
        }
        Ok(entry)
    }

    fn mutate_if_changed(
        &mut self,
        handle: &str,
        operation_id: &str,
        value_key: &str,
        operation: impl FnOnce(&mut QsObject) -> Result<(), RegistryError>,
    ) -> Result<String, RegistryError> {
        let parsed = ParsedHandle::parse(handle)?;
        let identity = parsed.identity();
        self.record_write(&identity);
        if self.active_cell.is_some() {
            self.reads.borrow_mut().insert(identity.clone());
        }
        let reversible = self.objects.get(&identity).is_some_and(|entry| {
            matches!(
                entry.object,
                QsObject::QuoteStore(_)
                    | QsObject::FixingStore(_)
                    | QsObject::FxStore(_)
                    | QsObject::ConstructedElements(_)
                    | QsObject::CurveConfigurations(_)
                    | QsObject::CreditCurveConfigurations(_)
            )
        });
        if let Some(cell) = self.active_cell.filter(|_| reversible) {
            let key = (identity.clone(), operation_id.to_string());
            if self
                .mutation_owners
                .get(&key)
                .is_some_and(|owner| *owner != cell)
            {
                return Err(RegistryError::DuplicateMutation(operation_id.to_string()));
            }
            // A formula changing its curve/index replaces its previous contribution.
            let previous: Vec<_> = self
                .cell_mutations
                .get(&cell)
                .into_iter()
                .flatten()
                .filter(|(object, operation)| object == &identity && operation != operation_id)
                .cloned()
                .collect();
            for key in previous {
                self.undo_mutation(&key);
            }
            self.mutation_owners.insert(key.clone(), cell);
            self.mutations.insert(key);
        }
        let entry = self
            .objects
            .get_mut(&identity)
            .ok_or_else(|| RegistryError::NotFound(handle.to_string()))?;
        if parsed.kind != entry.object.kind() {
            return Err(wrong_type(parsed.kind, &entry.object));
        }
        if entry.mutation_keys.get(operation_id).map(String::as_str) == Some(value_key) {
            return Ok(format_handle(&identity, entry.revision));
        }

        self.revision_clock = next_revision(self.revision_clock, &identity)?;
        let revision = self.revision_clock;
        if self.active_cell.is_some() && !entry.undo.contains_key(operation_id) {
            if let Some(undo) = MutationUndo::capture(&entry.object, operation_id) {
                entry.undo.insert(operation_id.to_string(), undo);
            }
        }
        operation(&mut entry.object)?;
        entry.revision = revision;
        entry
            .mutation_keys
            .insert(operation_id.to_string(), value_key.to_string());
        Ok(format_handle(&identity, entry.revision))
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

fn reset_object(object: &mut QsObject) -> Result<(), RegistryError> {
    match object {
        QsObject::QuoteStore(store) => {
            let reference_date = store.reference_date();
            **store = QuoteStore::new(reference_date);
        }
        QsObject::FixingStore(store) => **store = FixingStore::default(),
        QsObject::FxStore(store) => **store = FxStore::new(),
        QsObject::CurveConfigurations(configurations) => configurations.clear(),
        QsObject::CreditCurveConfigurations(configurations) => configurations.clear(),
        QsObject::VolatilitySurfaceConfigurations(configurations) => configurations.clear(),
        QsObject::VolatilityCubeConfigurations(configurations) => configurations.clear(),
        QsObject::SimulationConfigurations(configurations) => configurations.clear(),
        QsObject::PricerConfigurations(configurations) => configurations.clear(),
        QsObject::Scenarios(scenarios) => scenarios.clear(),
        QsObject::ConstructedElements(store) => **store = ConstructedElementStore::default(),
        other => return Err(RegistryError::ResetUnsupported(other.kind())),
    }
    Ok(())
}

thread_local! {
    static REGISTRIES: RefCell<HashMap<u64, Registry>> = RefCell::new(HashMap::new());
    static ACTIVE_WORKBOOK: Cell<u64> = const { Cell::new(0) };
}

pub fn select_workbook(workbook: u64) -> u64 {
    ACTIVE_WORKBOOK.with(|active| active.replace(workbook))
}

/// Borrows the calling workbook's registry.
pub fn with_registry<R>(operation: impl FnOnce(&Registry) -> R) -> R {
    with_registry_mut(|registry| operation(registry))
}

/// Mutably borrows the calling workbook's registry.
pub fn with_registry_mut<R>(operation: impl FnOnce(&mut Registry) -> R) -> R {
    let workbook = ACTIVE_WORKBOOK.with(Cell::get);
    REGISTRIES.with(|registries| {
        operation(
            registries
                .borrow_mut()
                .entry(workbook)
                .or_insert_with(|| Registry::in_workbook(workbook)),
        )
    })
}

pub fn live_object_count(all_workbooks: bool) -> usize {
    if all_workbooks {
        REGISTRIES.with(|registries| registries.borrow().values().map(Registry::len).sum())
    } else {
        with_registry(Registry::len)
    }
}

pub fn close_workbook(workbook: u64) {
    REGISTRIES.with(|registries| {
        let mut registries = registries.borrow_mut();
        registries.remove(&workbook);
        if registries
            .values()
            .all(|registry| registry.objects.is_empty())
        {
            release_tapes();
        }
    });
}

pub fn clear_workbooks() {
    REGISTRIES.with(|registries| registries.borrow_mut().clear());
    release_tapes();
}

fn release_tapes() {
    use quantsupport::ad::{
        forward::ADForward,
        tape::{Tape, TapeHolder},
    };
    f64::with_tape(|tape| *tape = Tape::new());
    ADForward::with_tape(|tape| *tape = Tape::new());
}

pub fn workbook_cells_in_rectangles(
    workbook: u64,
    sheet: usize,
    rectangles: &[(i32, i32, i32, i32)],
) -> Vec<CellId> {
    REGISTRIES.with(|registries| {
        let registries = registries.borrow();
        let Some(registry) = registries.get(&workbook) else {
            return Vec::new();
        };
        let mut result = HashSet::new();
        for &(row, column, end_row, end_column) in rectangles {
            if i64::from(end_row - row) * i64::from(end_column - column) <= 512 {
                for row in row..end_row {
                    for column in column..end_column {
                        let cell = CellId { sheet, row, column };
                        if registry.cell_roots.contains_key(&cell) {
                            result.insert(cell);
                        }
                    }
                }
            } else {
                result.extend(
                    registry
                        .cell_roots
                        .keys()
                        .filter(|cell| {
                            cell.sheet == sheet
                                && cell.row >= row
                                && cell.row < end_row
                                && cell.column >= column
                                && cell.column < end_column
                        })
                        .copied(),
                );
            }
        }
        result.into_iter().collect()
    })
}

pub fn release_workbook_cell_if_changed(
    workbook: u64,
    cell: CellId,
    formula: &str,
    has_formula: bool,
) {
    REGISTRIES.with(|registries| {
        let mut registries = registries.borrow_mut();
        if let Some(registry) = registries.get_mut(&workbook) {
            registry.release_if_formula_changed(cell, formula, has_formula);
        }
        if registries
            .values()
            .all(|registry| registry.objects.is_empty())
        {
            release_tapes();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    use quantsupport::prelude::{Currency, Date, Quote, QuoteDetails, QuoteLevels};

    #[test]
    fn constructor_recalculation_reuses_revision_until_inputs_change() {
        let mut registry = Registry::default();
        let first = registry
            .upsert_if_changed("fx", QsObject::FxStore(Box::new(FxStore::new())), "empty")
            .expect("valid name");
        let recalculated = registry
            .upsert_if_changed("fx", QsObject::FxStore(Box::new(FxStore::new())), "empty")
            .expect("same constructor inputs");
        let changed = registry
            .upsert_if_changed(
                "fx",
                QsObject::FxStore(Box::new(FxStore::new())),
                "different inputs",
            )
            .expect("valid name");

        assert_eq!(first, "QSObject:FxStore:fx:1");
        assert_eq!(recalculated, first);
        assert_eq!(changed, "QSObject:FxStore:fx:2");
        assert_eq!(registry.len(), 1);
        assert!(matches!(
            registry.fx_store(&first),
            Err(RegistryError::StaleHandle { .. })
        ));
    }

    #[test]
    fn mutation_recalculation_reuses_revision_and_constructor_keeps_state() {
        let mut registry = Registry::default();
        let created = registry
            .upsert_if_changed("fx", QsObject::FxStore(Box::new(FxStore::new())), "empty")
            .expect("valid name");
        let first_update = registry
            .update_fx_store_if_changed(&created, "EUR/USD", "1.1", |store| {
                store.add_fx_rate(Currency::EUR, Currency::USD, DualFwd::from(1.1));
            })
            .expect("first update");
        let recalculated = registry
            .update_fx_store_if_changed(&created, "EUR/USD", "1.1", |_| {
                panic!("unchanged operation must not run")
            })
            .expect("same mutation inputs");
        let constructor_recalculated = registry
            .upsert_if_changed("fx", QsObject::FxStore(Box::new(FxStore::new())), "empty")
            .expect("same constructor inputs");
        let changed = registry
            .update_fx_store_if_changed(&created, "EUR/USD", "1.2", |store| {
                store.add_fx_rate(Currency::EUR, Currency::USD, DualFwd::from(1.2));
            })
            .expect("Excel may recalculate with the original handle");

        assert_eq!(first_update, "QSObject:FxStore:fx:2");
        assert_eq!(recalculated, first_update);
        assert_eq!(constructor_recalculated, first_update);
        assert_eq!(changed, "QSObject:FxStore:fx:3");
        assert!(registry.fx_store(&first_update).is_err());
        let rate = registry
            .fx_store(&changed)
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
            .upsert_if_changed(
                "quotes",
                QsObject::QuoteStore(Box::new(QuoteStore::new(Date::new(2026, 1, 2)))),
                "2026-01-02",
            )
            .expect("valid name");
        let details = QuoteDetails::from_str("OIS_USD_SOFR_1Y").expect("valid quote identifier");
        let first_update = registry
            .update_quote_store_if_changed(&created, "OIS_USD_SOFR_1Y", "0.03", |store| {
                store.add_quote(Quote::new(details.clone(), QuoteLevels::with_mid(0.03)));
            })
            .expect("first quote update");
        let second_update = registry
            .update_quote_store_if_changed(&created, "OIS_USD_SOFR_1Y", "0.04", |store| {
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
            .upsert_if_changed("", QsObject::FxStore(Box::new(FxStore::new())), "empty")
            .is_err());
        assert!(registry
            .upsert_if_changed(
                "bad:name",
                QsObject::FxStore(Box::new(FxStore::new())),
                "empty",
            )
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
            .upsert_if_changed("fx", QsObject::FxStore(Box::new(FxStore::new())), "first")
            .expect("valid name");
        let second = registry
            .upsert_if_changed("fx", QsObject::FxStore(Box::new(FxStore::new())), "second")
            .expect("valid name");

        assert!(registry.remove(&first).is_err());
        assert!(registry.remove(&second).expect("current handle"));
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn keyed_reset_clears_stale_quotes_and_reapplies_unchanged_mutations() {
        let mut registry = Registry::default();
        let created = registry
            .upsert_if_changed(
                "quotes",
                QsObject::QuoteStore(Box::new(QuoteStore::new(Date::new(2026, 1, 2)))),
                "2026-01-02",
            )
            .expect("quote store");
        let details = QuoteDetails::from_str("OIS_USD_SOFR_1Y").expect("quote identifier");
        registry
            .update_quote_store_if_changed(&created, "quote:1Y", "0.03", |store| {
                store.add_quote(Quote::new(details.clone(), QuoteLevels::with_mid(0.03)));
            })
            .expect("initial quote");

        let reset = registry
            .reset_if_changed(&created, "configuration-v2")
            .expect("reset accepts the constructor handle");
        assert!(registry
            .quote_store(&reset)
            .expect("reset store")
            .quote("OIS_USD_SOFR_1Y")
            .is_none());

        let reapplied = registry
            .update_quote_store_if_changed(&reset, "quote:1Y", "0.03", |store| {
                store.add_quote(Quote::new(details, QuoteLevels::with_mid(0.03)));
            })
            .expect("unchanged mutation must be reapplied after reset");
        let unchanged_reset = registry
            .reset_if_changed(&created, "configuration-v2")
            .expect("same reset key");

        assert_eq!(unchanged_reset, reapplied);
        assert!(registry
            .quote_store(&unchanged_reset)
            .expect("repopulated store")
            .quote("OIS_USD_SOFR_1Y")
            .is_some());
    }

    #[test]
    fn reset_rejects_immutable_objects() {
        let mut registry = Registry::default();
        let handle = registry
            .upsert_if_changed(
                "context",
                QsObject::PricingContext(Box::new(PricingContext::new())),
                "definition",
            )
            .expect("context handle");

        assert!(matches!(
            registry.reset_if_changed(&handle, "v2"),
            Err(RegistryError::ResetUnsupported("PricingContext"))
        ));
    }
}

#[cfg(test)]
mod ownership_tests {
    use super::*;
    use quantsupport::prelude::{DayCounter, Interpolator, MarketIndex};

    fn cell(row: i32) -> CellId {
        CellId {
            sheet: 42,
            row,
            column: 0,
        }
    }
    fn fx(registry: &mut Registry, row: i32, name: &str, key: &str) -> String {
        registry.begin_cell(cell(row));
        let handle = registry
            .upsert_if_changed(name, QsObject::FxStore(Box::default()), key)
            .unwrap();
        registry.finish_cell(true);
        handle
    }
    fn configurations(registry: &mut Registry) -> String {
        registry.begin_cell(cell(0));
        let handle = registry
            .upsert_if_changed("curves", QsObject::CurveConfigurations(Vec::new()), "empty")
            .unwrap();
        registry.finish_cell(true);
        handle
    }
    fn add(registry: &mut Registry, handle: &str, index: MarketIndex) -> String {
        let operation = format!("curve:{index}");
        registry
            .update_curve_configurations_if_changed(handle, &operation, &operation, |values| {
                values.push(CurveConfiguration::new(
                    index,
                    DayCounter::Actual360,
                    Interpolator::Linear,
                    true,
                    vec!["OIS_USD_SOFR_1Y".into()],
                ));
            })
            .unwrap()
    }

    #[test]
    fn replacing_failed_and_deleted_formulas_release_outputs_and_keep_other_readers() {
        let mut registry = Registry::in_workbook(7);
        let first = fx(&mut registry, 0, "first", "a");
        registry.begin_cell(cell(1));
        registry.fx_store(&first).unwrap();
        registry.finish_cell(true);
        let second = fx(&mut registry, 0, "second", "b");
        assert_eq!(
            registry.len(),
            2,
            "the downstream cell still references first"
        );
        registry.release_cell(cell(1));
        assert_eq!(registry.len(), 1);
        assert!(registry.fx_store(&first).is_err());
        assert!(registry.fx_store(&second).is_ok());
        registry.begin_cell(cell(0));
        registry.finish_cell(false);
        assert_eq!(
            registry.len(),
            0,
            "failed constructor releases its prior output"
        );
        assert!(registry.cell_roots.is_empty());
        assert!(registry.references.is_empty());
    }

    #[test]
    fn repeated_names_cannot_resurrect_a_deleted_handle_or_cached_result() {
        let mut registry = Registry::in_workbook(7);
        let previous = fx(&mut registry, 0, "same", "one");
        registry.release_cell(cell(0));
        let current = fx(&mut registry, 0, "same", "two");
        assert_ne!(previous, current);
        assert!(registry.fx_store(&previous).is_err());
        assert!(registry.fx_store(&current).is_ok());
        let mut other = Registry::in_workbook(8);
        let other_handle = fx(&mut other, 0, "same", "two");
        assert_ne!(current, other_handle);
        assert!(other.fx_store(&current).is_err());
    }

    #[test]
    fn changed_invalid_and_deleted_configurations_remove_their_previous_contribution() {
        let mut registry = Registry::default();
        let created = configurations(&mut registry);
        registry.begin_cell(cell(1));
        let first = add(&mut registry, &created, MarketIndex::SOFR);
        registry.finish_cell(true);
        assert_eq!(registry.curve_configurations(&first).unwrap().len(), 1);
        registry.begin_cell(cell(1));
        let second = add(&mut registry, &created, MarketIndex::TermSOFR3m);
        registry.finish_cell(true);
        let values = registry.curve_configurations(&second).unwrap();
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].market_index(), &MarketIndex::TermSOFR3m);
        registry.begin_cell(cell(1));
        registry.finish_cell(false); // parsing the edited formula failed
        assert!(registry.objects.values().next().unwrap().object.kind() == "CurveConfigurations");
        let QsObject::CurveConfigurations(values) =
            &registry.objects.values().next().unwrap().object
        else {
            unreachable!()
        };
        assert!(values.is_empty());
        registry.begin_cell(cell(1));
        let final_handle = add(&mut registry, &created, MarketIndex::SOFR);
        registry.finish_cell(true);
        assert_eq!(
            registry.curve_configurations(&final_handle).unwrap().len(),
            1
        );
        registry.release_cell(cell(1));
        let QsObject::CurveConfigurations(values) =
            &registry.objects.values().next().unwrap().object
        else {
            unreachable!()
        };
        assert!(values.is_empty());
    }

    #[test]
    fn configuration_undo_restores_json_baseline_and_rejects_duplicate_cell_owners() {
        let mut registry = Registry::default();
        let created = configurations(&mut registry);
        // Simulate a collection imported from native JSON before worksheet edits.
        let baseline = CurveConfiguration::new(
            MarketIndex::SOFR,
            DayCounter::Actual360,
            Interpolator::Linear,
            true,
            vec!["OIS_USD_SOFR_2Y".into()],
        );
        if let QsObject::CurveConfigurations(values) =
            &mut registry.objects.values_mut().next().unwrap().object
        {
            values.push(baseline);
        }
        registry.begin_cell(cell(1));
        let updated = registry
            .update_curve_configurations_if_changed(&created, "curve:SOFR", "changed", |values| {
                values.clear()
            })
            .unwrap();
        registry.finish_cell(true);
        assert!(registry.curve_configurations(&updated).unwrap().is_empty());
        registry.begin_cell(cell(2));
        assert!(matches!(
            registry.update_curve_configurations_if_changed(
                &created,
                "curve:SOFR",
                "other",
                |_| panic!("must reject before mutation")
            ),
            Err(RegistryError::DuplicateMutation(_))
        ));
        registry.finish_cell(false);
        registry.release_cell(cell(1));
        let QsObject::CurveConfigurations(values) =
            &registry.objects.values().next().unwrap().object
        else {
            unreachable!()
        };
        assert_eq!(values[0].quotes(), &["OIS_USD_SOFR_2Y"]);
    }

    #[test]
    fn ten_thousand_cells_can_be_renamed_and_deleted_without_accumulating_objects() {
        let mut registry = Registry::in_workbook(9);
        for pass in 0..3 {
            for row in 0..10_000 {
                fx(
                    &mut registry,
                    row,
                    &format!("position_{pass}_{row}"),
                    "unchanged",
                );
            }
            assert_eq!(registry.len(), 10_000);
            assert_eq!(registry.cell_roots.len(), 10_000);
            assert_eq!(registry.references.len(), 10_000);
        }
        for row in 0..10_000 {
            registry.release_cell(cell(row));
        }
        assert_eq!(registry.len(), 0);
        assert!(registry.cell_roots.is_empty());
        assert!(registry.references.is_empty());
        assert!(registry.cell_mutations.is_empty());
    }

    #[test]
    fn cached_outputs_keep_dependencies_and_reject_changed_upstream_objects() {
        let mut registry = Registry::in_workbook(3);
        let input = fx(&mut registry, 0, "input", "first");
        registry.begin_cell(cell(1));
        registry.fx_store(&input).unwrap();
        let output = registry
            .upsert_if_changed("output", QsObject::FxStore(Box::default()), "unchanged")
            .unwrap();
        registry.finish_cell(true);
        registry.begin_cell(cell(1));
        assert_eq!(
            registry
                .handle_if_unchanged("output", "FxStore", "unchanged")
                .unwrap(),
            Some(output.clone())
        );
        registry.finish_cell(true);
        registry.release_cell(cell(0));
        assert_eq!(
            registry.len(),
            2,
            "the cached constructor still observes its input"
        );
        fx(&mut registry, 0, "input", "changed");
        assert!(registry
            .handle_if_unchanged("output", "FxStore", "unchanged")
            .unwrap()
            .is_none());
        assert!(matches!(
            registry.fx_store(&output),
            Err(RegistryError::StaleDependency { .. })
        ));
        registry.release_cell(cell(1));
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn quote_fixing_and_fx_contributions_are_removed_or_restore_the_imported_baseline() {
        use quantsupport::prelude::{QuoteDetails, QuoteLevels};
        let mut registry = Registry::default();
        let date = Date::new(2026, 1, 15);
        let mut quotes = QuoteStore::new(date);
        quotes.add_quote(Quote::new(
            "OIS_USD_SOFR_1Y".parse::<QuoteDetails>().unwrap(),
            QuoteLevels::with_mid(0.03),
        ));
        registry.begin_cell(cell(0));
        let quotes = registry
            .upsert_if_changed("quotes", QsObject::QuoteStore(Box::new(quotes)), "imported")
            .unwrap();
        registry.finish_cell(true);
        registry.begin_cell(cell(1));
        registry
            .update_quote_store_if_changed(&quotes, "quote:ois_usd_sofr_1y", "0.04", |store| {
                store.add_quote(Quote::new(
                    "ois_usd_sofr_1y".parse().unwrap(),
                    QuoteLevels::with_mid(0.04),
                ));
            })
            .unwrap();
        registry.finish_cell(true);
        registry.release_cell(cell(1));
        let QsObject::QuoteStore(store) = &registry.objects.values().next().unwrap().object else {
            unreachable!()
        };
        assert_eq!(
            store.quote("OIS_USD_SOFR_1Y").unwrap().levels().mid(),
            Some(0.03)
        );

        registry.begin_cell(cell(2));
        let fixings = registry
            .upsert_if_changed("fixings", QsObject::FixingStore(Box::default()), "empty")
            .unwrap();
        registry.finish_cell(true);
        registry.begin_cell(cell(3));
        registry
            .update_fixing_store_if_changed(&fixings, "fixing:SOFR:2026-01-15", "0.03", |store| {
                store.add_fixing(&MarketIndex::SOFR, date, 0.03)
            })
            .unwrap();
        registry.finish_cell(true);
        registry.begin_cell(cell(3));
        registry.finish_cell(false);
        let entry = registry
            .objects
            .values()
            .find(|entry| matches!(entry.object, QsObject::FixingStore(_)))
            .unwrap();
        let QsObject::FixingStore(store) = &entry.object else {
            unreachable!()
        };
        assert!(store.fixing(&MarketIndex::SOFR, date).is_err());

        let fx_handle = fx(&mut registry, 4, "fx", "empty");
        registry.begin_cell(cell(5));
        registry
            .update_fx_store_if_changed(&fx_handle, "fx:EUR:USD", "1.1", |store| {
                store.add_fx_rate(Currency::EUR, Currency::USD, DualFwd::from(1.1))
            })
            .unwrap();
        registry.finish_cell(true);
        registry.release_cell(cell(5));
        let entry = registry
            .objects
            .values()
            .find(|entry| matches!(entry.object, QsObject::FxStore(_)))
            .unwrap();
        let QsObject::FxStore(store) = &entry.object else {
            unreachable!()
        };
        assert!(store.direct_fx_rate(Currency::EUR, Currency::USD).is_none());
    }

    #[test]
    fn edit_notifications_preserve_fresh_results_in_either_event_order() {
        let mut registry = Registry::in_workbook(4);
        let old = fx(&mut registry, 0, "old", "old");
        registry.set_cell_formula(cell(0), Some("=QS.FX.CREATE(\"old\")".into()));
        // Notification before evaluation releases the obsolete owner.
        registry.release_if_formula_changed(cell(0), "=QS.FX.CREATE(\"new\")", true);
        assert!(registry.fx_store(&old).is_err());
        let new = fx(&mut registry, 0, "new", "new");
        registry.set_cell_formula(cell(0), Some("=QS.FX.CREATE(\"new\")".into()));
        // Notification after evaluation must preserve the just-produced output.
        registry.release_if_formula_changed(cell(0), "=QS.FX.CREATE(\"new\")", true);
        assert!(registry.fx_store(&new).is_ok());
        // Replacing the formula with another Excel formula releases old ownership.
        registry.release_if_formula_changed(cell(0), "=1+2", true);
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn constructed_curve_edits_undo_their_store_entry_and_reset_cells_keep_a_reference() {
        use quantsupport::prelude::{
            Compounding, FlatForwardTermStructure, Frequency, RateDefinition,
        };
        use std::rc::Rc;
        let mut registry = Registry::default();
        registry.begin_cell(cell(0));
        let created = registry
            .upsert_if_changed(
                "elements",
                QsObject::ConstructedElements(Box::default()),
                "empty",
            )
            .unwrap();
        registry.finish_cell(true);
        registry.begin_cell(cell(1));
        let curve = DiscountCurveElement::new(
            MarketIndex::SOFR,
            Rc::new(RefCell::new(FlatForwardTermStructure::new(
                Date::new(2026, 1, 15),
                DualFwd::from(0.03),
                RateDefinition::new(
                    DayCounter::Actual360,
                    Compounding::Continuous,
                    Frequency::Annual,
                ),
            ))),
        );
        let updated = registry
            .update_constructed_elements_if_changed(
                &created,
                "discount_curve:SOFR",
                "0.03",
                |store| {
                    store.discount_curves_mut().insert(MarketIndex::SOFR, curve);
                },
            )
            .unwrap();
        registry.finish_cell(true);
        assert!(registry
            .constructed_elements(&updated)
            .unwrap()
            .discount_curves()
            .contains_key(&MarketIndex::SOFR));
        registry.release_cell(cell(1));
        let QsObject::ConstructedElements(store) =
            &registry.objects.values().next().unwrap().object
        else {
            unreachable!()
        };
        assert!(store.discount_curves().is_empty());
        registry.begin_cell(cell(2));
        let reset = registry.reset_if_changed(&created, "new inputs").unwrap();
        registry.finish_cell(true);
        registry.release_cell(cell(0));
        assert!(
            registry.constructed_elements(&reset).is_ok(),
            "reset output owns its returned handle"
        );
        registry.release_cell(cell(2));
        assert_eq!(registry.len(), 0);
    }
}
