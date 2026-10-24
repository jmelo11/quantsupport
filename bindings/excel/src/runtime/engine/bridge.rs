//! Worksheet adapter and versioned, nonblocking native/managed engine ABI.
//! Definitions are normalized worksheet values and replay functions, never Rc,
//! RefCell, COM pointers, AD nodes, or Excel references.
use super::scheduler::{Cancellation, Job, Scheduler};
use crate::runtime::{
    context,
    registry::{self, CellId},
};
use serde::Serialize;
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet, VecDeque},
    ffi::{c_char, CString},
    hash::{Hash, Hasher},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use windows::Win32::{
    Foundation::HANDLE,
    System::{
        LibraryLoader::{
            GetModuleHandleExW, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
            GET_MODULE_HANDLE_EX_FLAG_PIN,
        },
        Threading::{CreateEventW, GetCurrentThreadId, SetEvent},
    },
};
use xll_rs::{memory::xlAutoFree12, types::*};

#[derive(Clone, Serialize)]
pub enum Value {
    Missing,
    Blank,
    Number(f64),
    Text(String),
    Boolean(bool),
    Error(i32),
    Array {
        rows: usize,
        columns: usize,
        values: Vec<Value>,
    },
    Browser(Box<crate::runtime::browser_queries::BrowserResponse>),
}
impl Value {
    pub fn capture(value: *const XLOPER12) -> Result<Self, String> {
        let value = unsafe { value.as_ref() }.ok_or("null worksheet argument")?;
        Ok(match value.base_type() {
            XLTYPE_MISSING => Self::Missing,
            XLTYPE_NIL => Self::Blank,
            XLTYPE_NUM => Self::Number(unsafe { value.val.num }),
            XLTYPE_INT => Self::Number(f64::from(unsafe { value.val.w })),
            XLTYPE_STR => Self::Text(value.as_string().ok_or("invalid worksheet text")?),
            XLTYPE_BOOL => Self::Boolean(unsafe { value.val.xbool } != 0),
            XLTYPE_ERR => Self::Error(unsafe { value.val.err }),
            XLTYPE_MULTI => {
                let array = unsafe { &*std::ptr::addr_of!(value.val.array) };
                if array.rows < 1 || array.columns < 1 || array.lparray.is_null() {
                    return Err("invalid worksheet array".into());
                }
                let (rows, columns) = (array.rows as usize, array.columns as usize);
                let length = rows
                    .checked_mul(columns)
                    .filter(|n| *n <= 16_777_216)
                    .ok_or("worksheet array is too large")?;
                Self::Array {
                    rows,
                    columns,
                    values: (0..length)
                        .map(|i| Self::capture(unsafe { array.lparray.add(i) }))
                        .collect::<Result<_, _>>()?,
                }
            }
            _ => return Err("engine arguments must be detached worksheet values".into()),
        })
    }
    pub fn oper(&self) -> XLOPER12 {
        match self {
            Self::Missing | Self::Blank => XLOPER12::nil(),
            Self::Number(n) => XLOPER12::from_f64(*n),
            Self::Text(s) => XLOPER12::from_str(s),
            Self::Boolean(v) => XLOPER12::from_bool(*v),
            Self::Error(e) => XLOPER12::from_err(*e),
            Self::Browser(_) => XLOPER12::from_err(XLERR_VALUE),
            Self::Array {
                rows,
                columns,
                values,
            } => {
                let pointer = crate::functions::helpers::spill(
                    values.iter().map(Self::oper).collect(),
                    *rows,
                    *columns,
                    None,
                    None,
                );
                // Transfer the outer allocation, retaining the array's owned children.
                unsafe { *Box::from_raw(pointer) }
            }
        }
    }
    pub fn into_raw(&self) -> *mut XLOPER12 {
        let mut value = self.oper();
        value.xltype |= XLBIT_DLL_FREE;
        Box::into_raw(Box::new(value))
    }
    fn handles(&self, output: &mut Vec<String>) {
        match self {
            Self::Text(s) if s.starts_with("QSObject:") || s.starts_with("QSEngine:") => {
                output.push(s.clone())
            }
            Self::Array { values, .. } => {
                for v in values {
                    v.handles(output);
                }
            }
            _ => {}
        }
    }
    fn remap(&self, mappings: &HashMap<String, Value>) -> Self {
        match self {
            Self::Text(s) => mappings.get(s).cloned().unwrap_or_else(|| self.clone()),
            Self::Array {
                rows,
                columns,
                values,
            } => Self::Array {
                rows: *rows,
                columns: *columns,
                values: values.iter().map(|v| v.remap(mappings)).collect(),
            },
            _ => self.clone(),
        }
    }
    fn input_error(&self) -> Option<i32> {
        match self {
            Self::Error(e) => Some(*e),
            Self::Array { values, .. } => values.iter().find_map(Self::input_error),
            _ => None,
        }
    }
}
pub trait Detach {
    fn detach(self) -> Result<Value, String>;
}
impl Detach for String {
    fn detach(self) -> Result<Value, String> {
        Ok(Value::Text(self))
    }
}
impl Detach for f64 {
    fn detach(self) -> Result<Value, String> {
        Ok(Value::Number(self))
    }
}
impl Detach for i32 {
    fn detach(self) -> Result<Value, String> {
        Ok(Value::Number(f64::from(self)))
    }
}
impl Detach for bool {
    fn detach(self) -> Result<Value, String> {
        Ok(Value::Boolean(self))
    }
}
impl Detach for *mut XLOPER12 {
    fn detach(self) -> Result<Value, String> {
        let result = Value::capture(self);
        xlAutoFree12(self);
        result
    }
}
pub struct Replay {
    pub name: &'static str,
    pub invoke: fn(&[Value]) -> Result<Value, String>,
}
xll_rs::inventory::collect!(Replay);
pub struct Operands(pub Vec<*mut XLOPER12>);
impl Operands {
    pub fn new(args: &[Value]) -> Self {
        Self(args.iter().map(Value::into_raw).collect())
    }
}
impl Drop for Operands {
    fn drop(&mut self) {
        for p in self.0.drain(..) {
            xlAutoFree12(p);
        }
    }
}

#[derive(Clone)]
struct Definition {
    id: u64,
    epoch: u64,
    invocation: usize,
    function: String,
    args: Arc<Vec<Value>>,
    cell: CellId,
    outputs: Vec<String>,
    key: String,
    sealed: Option<Vec<(String, u64)>>,
}
#[derive(Clone)]
struct Spec {
    definitions: Vec<Arc<Definition>>,
    commits: HashMap<usize, Vec<(String, Vec<String>)>>,
    query: Arc<Definition>,
    retirement: Option<u64>,
}
struct Worker {
    materialized: HashMap<u64, Value>,
    mappings: HashMap<String, Value>,
    definitions_built: u64,
    contexts_built: u64,
    curves_built: u64,
    owners: HashMap<u64, u64>,
}
struct WorkResult {
    result: Result<Value, String>,
    definitions: u64,
    contexts: u64,
    curves: u64,
    object: Option<crate::runtime::browser::BrowserObject>,
}
thread_local! { static WORKER: Cell<bool> = const { Cell::new(false) }; }
pub fn on_worker() -> bool {
    WORKER.with(Cell::get)
}
#[cfg_attr(test, allow(dead_code))]
pub fn attached() -> bool {
    ATTACHED.load(Ordering::Acquire)
}
pub fn status() -> String {
    STATE.with(|s| {
    let s = s.borrow(); let mut result = serde_json::to_value(&s.metrics).unwrap();
    result["lifecycle_error"] = serde_json::json!(crate::runtime::lifecycle::event_error());
    result["pending"] = serde_json::json!(s.consumers.values().filter(|c| c.value.is_none() || c.validated_at != s.revision).count());
    result["settled"] = serde_json::json!(!s.changed && s.dirty.is_empty() && !registry::definition_pass_pending());
    result["pending_delivery"] = serde_json::json!(s.consumers.iter().filter(|(_, c)| c.definition.cell.sheet != 0 && c.value.is_some() && c.delivered_generation != c.generation).count());
    result["waiting_inputs"] = serde_json::json!(s.consumers.values().filter(|c| c.waiting_input).count());
    result["notification_groups"] = serde_json::json!(s.consumers.values().filter(|c| c.definition.cell.sheet != 0).map(|c| (c.epoch, c.definition.cell.sheet, c.definition.cell.column, c.definition.cell.row / 64)).collect::<HashSet<_>>().len());
    result["errors"] = serde_json::json!(s.consumers.values().filter(|c| !c.error.is_empty()).count());
    result["diagnostics"] = serde_json::json!(s.consumers.values().filter(|c| !c.error.is_empty()).take(10).map(|c| &c.error).collect::<Vec<_>>());
    if s.consumers.len() <= 32 {
        result["jobs"] = serde_json::json!(s.consumers.values().map(|c| {
            let value = match c.value.as_deref() { Some(Value::Number(n)) => n.to_string(), Some(Value::Text(t)) => t.clone(), Some(Value::Error(e)) => format!("Excel error {e}"), Some(_) => "table".into(), None => "pending".into() };
            serde_json::json!({"function":c.definition.function,"row":c.definition.cell.row+1,"column":c.definition.cell.column+1,"value":value,"current":c.validated_at==s.revision,"delivered":c.delivered_generation==c.generation})
        }).collect::<Vec<_>>());
    }
    result.to_string()
})
}
pub fn opaque_kind(handle: &str) -> Option<String> {
    STATE.with(|s| s.borrow().objects.get(handle).map(|(_, o)| o.kind.clone()))
}
pub fn browser_objects(epoch: u64) -> Vec<crate::runtime::browser::BrowserObject> {
    STATE.with(|s| {
        let s = s.borrow();
        s.objects
            .iter()
            .filter(|(_, (e, _))| *e == epoch)
            .map(|(h, (_, o))| {
                let mut o = o.clone();
                o.current_inputs = s
                    .definitions
                    .get(h)
                    .and_then(|d| d.sealed.as_ref())
                    .is_some_and(|versions| {
                        versions
                            .iter()
                            .all(|(h, id)| s.definitions.get(h).is_some_and(|d| d.id == *id))
                    });
                o
            })
            .collect()
    })
}
pub fn root_curves(handle: &str) -> Vec<String> {
    STATE.with(|s| {
        let s = s.borrow();
        let mut curves: Vec<_> = s
            .contributions
            .get(handle)
            .into_iter()
            .flatten()
            .filter_map(|h| s.definitions.get(h))
            .filter(|d| {
                matches!(
                    d.function.as_str(),
                    "qs_context_add_curve"
                        | "qs_context_add_zero_curve"
                        | "qs_context_add_composite_curve"
                )
            })
            .filter_map(|d| match d.args.get(1) {
                Some(Value::Text(index)) => Some(index.clone()),
                _ => None,
            })
            .collect();
        curves.sort();
        curves.dedup();
        curves
    })
}
pub fn root_valid(handle: &str) -> bool {
    STATE.with(|s| {
        let s = s.borrow();
        !s.invalid_roots.contains_key(handle) && !s.churn_invalid.contains_key(handle)
    })
}
impl Worker {
    fn invoke(&mut self, definition: &Definition) -> Result<Value, String> {
        if let Some(value) = self.materialized.get(&definition.id) {
            for output in &definition.outputs {
                self.mappings.insert(output.clone(), value.clone());
            }
            return Ok(value.clone());
        }
        let replay = xll_rs::inventory::iter::<Replay>
            .into_iter()
            .find(|r| r.name == definition.function)
            .ok_or("unknown engine function")?;
        let args: Vec<_> = definition
            .args
            .iter()
            .map(|v| v.remap(&self.mappings))
            .collect();
        registry::select_workbook(definition.epoch);
        registry::with_registry_mut(|r| {
            r.begin_cell(definition.cell);
            r.set_invocation(&definition.function, definition.invocation);
        });
        let result = (replay.invoke)(&args);
        registry::with_registry_mut(|r| r.finish_cell(result.is_ok()));
        let value = result?;
        for output in &definition.outputs {
            self.mappings.insert(output.clone(), value.clone());
        }
        self.materialized.insert(definition.id, value.clone());
        self.owners.insert(definition.id, definition.epoch);
        self.definitions_built += 1;
        Ok(value)
    }
    fn execute(&mut self, spec: Spec, cancel: &Cancellation) -> Result<Value, String> {
        if let Some(epoch) = spec.retirement {
            registry::close_workbook(epoch);
            self.materialized
                .retain(|id, _| self.owners.get(id) != Some(&epoch));
            self.owners.retain(|_, owner| *owner != epoch);
            self.mappings.retain(|h, _| {
                !h.contains(&format!("w{epoch}~"))
                    && !(h.starts_with("QSEngine:")
                        && h.split(':').nth(2) == Some(epoch.to_string().as_str()))
            });
            return Ok(Value::Blank);
        }
        if self.materialized.len() > 131072 {
            self.materialized.clear();
            self.mappings.clear();
            self.owners.clear();
            registry::clear_workbooks();
        }
        struct TapeGuard;
        impl Drop for TapeGuard {
            fn drop(&mut self) {
                use quantsupport::prelude::Tape;
                Tape::stop_recording();
                Tape::stop_recording_fwd();
                // Retained contexts contain detached DualFwd inputs, never reverse tape nodes.
                Tape::rewind_to_init();
                Tape::<quantsupport::prelude::ADForward>::rewind_to_init_for();
            }
        }
        let _guard = TapeGuard;
        let mut markets = Vec::new();
        for (position, definition) in spec.definitions.iter().enumerate() {
            if cancel.canceled() {
                return Err("canceled".into());
            }
            self.invoke(definition)?;
            for (root, inputs) in spec.commits.get(&position).into_iter().flatten() {
                let text = |key: &String| match self.mappings.get(key) {
                    Some(Value::Text(s)) => Ok(s.clone()),
                    _ => Err(format!("missing materialized definition {key}")),
                };
                let root = text(root)?;
                let inputs = inputs.iter().map(text).collect::<Result<Vec<_>, _>>()?;
                let (market, builds) = registry::with_registry(|r| {
                    let before = r.context_cache.borrow().compositions;
                    let prepared = context::prepare(r, &root, &inputs)?;
                    let key = serde_json::to_string(&(
                        r.context_revision(&root).map_err(|e| e.to_string())?,
                        &inputs,
                    ))
                    .map_err(|e| e.to_string())?;
                    let mut cache = r.context_cache.borrow_mut();
                    cache.committed_keys.insert(root.clone(), key);
                    cache.committed.insert(root, prepared.clone());
                    Ok::<_, String>((prepared, cache.compositions - before))
                })?;
                self.contexts_built += builds;
                let before = market.request_cache().discount_curve_count();
                markets.push((market, before));
            }
        }
        if cancel.canceled() {
            return Err("canceled".into());
        }
        let result = self.invoke(&spec.query);
        for (market, before) in markets {
            self.curves_built += market
                .request_cache()
                .discount_curve_count()
                .saturating_sub(before) as u64;
        }
        result
    }
}
#[derive(Default, Serialize)]
struct Metrics {
    submitted: u64,
    completed: u64,
    canceled: u64,
    reused: u64,
    definitions: usize,
    subscriptions: usize,
    workers: usize,
    pending: usize,
    queue_ms: f64,
    native_ms: f64,
    commit_ms: f64,
    max_dispatch_ms: f64,
    definitions_built: u64,
    context_builds: u64,
    curve_builds: u64,
    worksheet_calls: u64,
    worksheet_ms: f64,
    caller_ms: f64,
    subscription_ms: f64,
    delivery_ms: f64,
    delivery_passes: u64,
    max_delivery_ms: f64,
}
struct Consumer {
    definition: Arc<Definition>,
    epoch: u64,
    generation: u64,
    fingerprint: u64,
    job: Option<u64>,
    value: Option<Arc<Value>>,
    error: String,
    affinity: u64,
    validated_at: u64,
    delivered_generation: u64,
    waiting_input: bool,
    sources: Vec<CellId>,
}
struct Flight {
    cancel: Cancellation,
    subscribers: Vec<(String, u64)>,
    spec: Spec,
    epoch: u64,
    fingerprint: u64,
}
struct Batch {
    query: Arc<Definition>,
    children: Vec<String>,
    count: usize,
    revision: u64,
    transpose: bool,
}
struct State {
    next: u64,
    definitions: HashMap<String, Arc<Definition>>,
    consumers: HashMap<String, Consumer>,
    dependents: HashMap<CellId, HashSet<String>>,
    batches: HashMap<String, Batch>,
    dirty: VecDeque<String>,
    dirty_set: HashSet<String>,
    flights: HashMap<u64, Flight>,
    dedup: HashMap<u64, u64>,
    pool: Option<Scheduler<Spec, WorkResult>>,
    metrics: Metrics,
    invalid: HashMap<CellId, String>,
    changed: bool,
    revision: u64,
    contributions: HashMap<String, Vec<String>>,
    invalid_roots: HashMap<String, (CellId, String)>,
    ready: HashMap<u64, (Arc<Value>, String)>,
    ready_order: VecDeque<u64>,
    objects: HashMap<String, (u64, crate::runtime::browser::BrowserObject)>,
    churn: HashMap<String, (u64, usize)>,
    churn_invalid: HashMap<String, (CellId, String)>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            next: 0,
            definitions: HashMap::new(),
            consumers: HashMap::new(),
            dependents: HashMap::new(),
            batches: HashMap::new(),
            dirty: VecDeque::new(),
            dirty_set: HashSet::new(),
            flights: HashMap::new(),
            dedup: HashMap::new(),
            pool: None,
            metrics: Metrics::default(),
            invalid: HashMap::new(),
            changed: false,
            revision: 0,
            contributions: HashMap::new(),
            invalid_roots: HashMap::new(),
            ready: HashMap::new(),
            ready_order: VecDeque::new(),
            objects: HashMap::new(),
            churn: HashMap::new(),
            churn_invalid: HashMap::new(),
        }
    }
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
static EVENT: AtomicUsize = AtomicUsize::new(0);
static MAIN: AtomicUsize = AtomicUsize::new(0);
static ATTACHED: AtomicBool = AtomicBool::new(false);
fn wake() {
    let handle = EVENT.load(Ordering::Acquire);
    if handle != 0 {
        unsafe {
            let _ = SetEvent(HANDLE(handle as _));
        }
    }
}
pub fn schedule_commit() {
    if !on_worker() {
        wake();
    }
}
fn hash(value: impl Hash) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut h);
    h.finish()
}
fn notification_key(epoch: u64, cell: CellId) -> String {
    format!(
        "cells:{epoch}:{}:{}:{}",
        cell.sheet,
        cell.column,
        cell.row / 64
    )
}
fn error(value: impl ToString) -> XllError {
    crate::functions::helpers::value_error(value.to_string())
}
fn normalize(function: &str, mut args: Vec<Value>) -> Vec<Value> {
    if function == "qs_evaluate" && args.len() == 7 {
        if matches!(&args[2], Value::Missing | Value::Blank)
            || matches!(&args[2], Value::Text(v) if v.is_empty())
        {
            args[2] = Value::Text("Value".into());
        }
        for index in [3, 4] {
            if matches!(&args[index], Value::Blank)
                || matches!(&args[index], Value::Text(v) if v.is_empty())
            {
                args[index] = Value::Missing;
            }
        }
        if matches!(args[5], Value::Boolean(false)) {
            args[5] = Value::Missing;
        }
        if matches!(args[6], Value::Boolean(true)) {
            args[6] = Value::Missing;
        }
    }
    args
}

impl State {
    fn batch_step(&mut self, key: &str, query: Arc<Definition>) -> Result<(), String> {
        let mut batch = if let Some(batch) = self.batches.remove(key) {
            batch
        } else {
            let entries = |value: &Value| match value {
                Value::Array { values, .. } => values.len(),
                _ => 1,
            };
            let count = entries(&query.args[0]);
            if count != entries(&query.args[1]) {
                return Err(
                    "context must be one handle or an array aligned with instruments".into(),
                );
            }
            for arg in &query.args[..2] {
                if !matches!(arg, Value::Array { values, .. } if values.iter().all(|v| matches!(v, Value::Text(s) if !s.is_empty())))
                {
                    return Err(
                        "paired contexts and instruments must be nonblank handle arrays".into(),
                    );
                }
            }
            let request = match &query.args[2] {
                Value::Missing | Value::Blank => "Value",
                Value::Text(s) if s.is_empty() => "Value",
                Value::Text(s) => s.as_str(),
                _ => return Err("request_name must be text".into()),
            };
            let requests = crate::runtime::pricing::parse_requests(&[request.into()])?;
            use quantsupport::prelude::Request;
            if !matches!(
                requests.first(),
                Some(
                    Request::Value
                        | Request::FairRate(_)
                        | Request::YieldToMaturity
                        | Request::ModifiedDuration
                        | Request::ZSpread
                        | Request::OAS
                )
            ) {
                return Err("a batch requires a scalar request".into());
            }
            let operands = Operands::new(&[query.args[5].clone()]);
            let transpose = crate::functions::helpers::optional_bool(operands.0[0], "transpose")
                .map_err(|e| e.message().unwrap_or("invalid transpose").to_string())?
                .unwrap_or(false);
            Batch {
                query,
                children: Vec::with_capacity(count),
                count,
                revision: u64::MAX,
                transpose,
            }
        };
        if batch.revision != self.revision {
            batch.revision = self.revision;
            let c = self.consumers.get_mut(key).unwrap();
            c.generation += 1;
            c.value = None;
            c.error.clear();
            c.validated_at = self.revision;
        }
        for _ in 0..128 {
            let index = batch.children.len();
            if index == batch.count {
                break;
            }
            let at = |argument: usize| match &batch.query.args[argument] {
                Value::Array { values, .. } => values[index].clone(),
                _ => unreachable!(),
            };
            let args = normalize(
                "qs_evaluate",
                vec![
                    at(0),
                    at(1),
                    batch.query.args[2].clone(),
                    batch.query.args[3].clone(),
                    batch.query.args[4].clone(),
                    Value::Missing,
                    Value::Missing,
                ],
            );
            let child = format!("batch:{}:{index}", batch.query.id);
            let definition = Arc::new(Definition {
                id: self.id(),
                epoch: batch.query.epoch,
                invocation: 0,
                function: "qs_evaluate".into(),
                key: serde_json::to_string(&args).map_err(|e| e.to_string())?,
                args: Arc::new(args),
                cell: CellId {
                    sheet: 0,
                    row: index as i32,
                    column: 0,
                },
                outputs: Vec::new(),
                sealed: None,
            });
            self.subscribe(
                child.clone(),
                Consumer {
                    definition,
                    epoch: batch.query.epoch,
                    generation: 0,
                    fingerprint: 0,
                    job: None,
                    value: None,
                    error: String::new(),
                    affinity: 0,
                    validated_at: 0,
                    delivered_generation: 0,
                    waiting_input: false,
                    sources: Vec::new(),
                },
            );
            self.mark(child.clone());
            batch.children.push(child);
        }
        if batch.children.len() < batch.count {
            self.mark(key.into());
        }
        self.batches.insert(key.into(), batch);
        Ok(())
    }

    fn finish_batches(&mut self, changed: &mut Vec<String>) {
        for (key, batch) in &self.batches {
            if batch.children.len() != batch.count
                || self
                    .consumers
                    .get(key)
                    .is_none_or(|c| c.value.is_some() || c.validated_at != self.revision)
            {
                continue;
            }
            if batch.children.iter().any(|key| {
                self.consumers
                    .get(key)
                    .is_none_or(|c| c.value.is_none() || c.validated_at != self.revision)
                    || self.dirty_set.contains(key)
            }) {
                continue;
            }
            let mut values = Vec::with_capacity(batch.count);
            let mut error = String::new();
            for (index, child) in batch.children.iter().enumerate() {
                let c = &self.consumers[child];
                if !c.error.is_empty() || !matches!(c.value.as_deref(), Some(Value::Number(_))) {
                    error = format!(
                        "Valuation row {}: {}",
                        index + 1,
                        if c.error.is_empty() {
                            "a batch requires numeric scalar results"
                        } else {
                            &c.error
                        }
                    );
                    break;
                }
                values.push(c.value.as_deref().unwrap().clone());
            }
            let result = if error.is_empty() {
                Value::Array {
                    rows: if batch.transpose { 1 } else { batch.count },
                    columns: if batch.transpose { batch.count } else { 1 },
                    values,
                }
            } else {
                Value::Error(XLERR_VALUE)
            };
            let c = self.consumers.get_mut(key).unwrap();
            c.value = Some(Arc::new(result));
            c.error = error;
            changed.push(key.clone());
        }
    }

    fn subscribe(&mut self, key: String, mut consumer: Consumer) {
        let mut handles = Vec::new();
        for arg in consumer.definition.args.iter() {
            arg.handles(&mut handles);
        }
        consumer.sources = handles
            .iter()
            .filter(|h| h.starts_with("QSEngine:"))
            .filter_map(|h| self.definitions.get(h).map(|d| d.cell))
            .filter(|cell| cell.sheet != 0)
            .collect();
        consumer
            .sources
            .sort_unstable_by_key(|cell| (cell.sheet, cell.row, cell.column));
        consumer.sources.dedup();
        for source in &consumer.sources {
            self.dependents
                .entry(*source)
                .or_default()
                .insert(key.clone());
        }
        self.consumers.insert(key, consumer);
    }
    fn remove_consumer(&mut self, key: &str) {
        if let Some(batch) = self.batches.remove(key) {
            for child in batch.children {
                self.remove_consumer(&child);
            }
        }
        if let Some(c) = self.consumers.remove(key) {
            for source in &c.sources {
                if let Some(keys) = self.dependents.get_mut(source) {
                    keys.remove(key);
                    if keys.is_empty() {
                        self.dependents.remove(source);
                    }
                }
            }
            if let Some(id) = c.job {
                if let Some(f) = self.flights.get_mut(&id) {
                    f.subscribers.retain(|(k, _)| k != key);
                    if f.subscribers.is_empty() {
                        f.cancel.cancel();
                        let fingerprint = f.fingerprint;
                        if self.dedup.get(&fingerprint) == Some(&id) {
                            self.dedup.remove(&fingerprint);
                        }
                        self.flights.remove(&id);
                        self.metrics.canceled += 1;
                    }
                }
            }
        }
    }
    fn id(&mut self) -> u64 {
        self.next += 1;
        self.next
    }
    fn mark(&mut self, key: String) {
        if self.dirty_set.insert(key.clone()) {
            self.dirty.push_back(key);
        }
    }
    fn pool(&mut self) -> &Scheduler<Spec, WorkResult> {
        self.pool.get_or_insert_with(|| {
            let workers = std::env::var("QS_ENGINE_WORKERS")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or_else(|| {
                    std::thread::available_parallelism().map_or(1, |n| n.get().min(8))
                })
                .clamp(1, 32);
            Scheduler::new(
                workers,
                4096,
                || {
                    WORKER.with(|v| v.set(true));
                    let mut worker = Worker {
                        materialized: HashMap::new(),
                        mappings: HashMap::new(),
                        owners: HashMap::new(),
                        definitions_built: 0,
                        contexts_built: 0,
                        curves_built: 0,
                    };
                    move |spec, cancel: &Cancellation| {
                        let before = (
                            worker.definitions_built,
                            worker.contexts_built,
                            worker.curves_built,
                        );
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            worker.execute(spec, cancel)
                        }))
                        .unwrap_or_else(|_| Err("native worker panicked".into()));
                        let object = if let Ok(Value::Text(handle)) = &result {
                            if handle.starts_with("QSObject:") {
                                registry::with_registry(|r| {
                                    r.browser_object(handle)
                                        .ok()
                                        .map(crate::runtime::browser::BrowserObject::from_object)
                                })
                            } else {
                                None
                            }
                        } else {
                            None
                        };
                        WorkResult {
                            result,
                            definitions: worker.definitions_built - before.0,
                            contexts: worker.contexts_built - before.1,
                            curves: worker.curves_built - before.2,
                            object,
                        }
                    }
                },
                Arc::new(wake),
            )
        })
    }
    fn snapshot(&self, query: Arc<Definition>) -> Result<(Spec, u64, u64), String> {
        let mut definitions = Vec::new();
        let mut seen = HashSet::new();
        let mut visiting = HashSet::new();
        let mut roots = Vec::new();
        fn visit(
            s: &State,
            handle: &str,
            definitions: &mut Vec<Arc<Definition>>,
            seen: &mut HashSet<u64>,
            visiting: &mut HashSet<u64>,
            roots: &mut Vec<(String, Vec<String>)>,
        ) -> Result<(), String> {
            let d = s.definitions.get(handle).ok_or_else(|| {
                format!("definition is unavailable or its source cell failed: {handle}")
            })?;
            if visiting.contains(&d.id) {
                return Err(format!("cyclic market dependency at {:?}", d.cell));
            }
            if seen.contains(&d.id) {
                return Ok(());
            }
            if !visiting.insert(d.id) {
                return Err(format!("cyclic market dependency at {:?}", d.cell));
            }
            if let Some(versions) = &d.sealed {
                if versions
                    .iter()
                    .any(|(h, id)| s.definitions.get(h).is_none_or(|d| d.id != *id))
                {
                    return Err("Stored result inputs changed; await its current evaluation before reading the result.".into());
                }
            }
            let mut dependencies = Vec::new();
            for (index, arg) in d.args.iter().enumerate() {
                if index == 0 && d.function.starts_with("qs_context_add_") {
                    if let Value::Text(root) = arg {
                        if s.definitions
                            .get(root)
                            .is_some_and(|root| visiting.contains(&root.id))
                        {
                            continue;
                        }
                    }
                }
                arg.handles(&mut dependencies);
            }
            for dependency in dependencies {
                visit(s, &dependency, definitions, seen, visiting, roots)?;
            }
            definitions.push(d.clone());
            if d.function == "qs_context_new" {
                if let Some((cell, e)) = s.invalid_roots.get(handle) {
                    return Err(format!("context input at {cell:?}: {e}"));
                }
                if let Some((cell, e)) = s.churn_invalid.get(handle) {
                    return Err(format!("context input at {cell:?}: {e}"));
                }
                let mut contributions = s.contributions.get(handle).cloned().unwrap_or_default();
                contributions.sort();
                for input in &contributions {
                    visit(s, input, definitions, seen, visiting, roots)?;
                }
                roots.push((handle.into(), contributions));
            }
            visiting.remove(&d.id);
            seen.insert(d.id);
            Ok(())
        }
        let mut handles = Vec::new();
        for arg in query.args.iter() {
            arg.handles(&mut handles);
        }
        for handle in handles {
            visit(
                self,
                &handle,
                &mut definitions,
                &mut seen,
                &mut visiting,
                &mut roots,
            )?;
        }
        for d in &definitions {
            if let Some(e) = self.invalid.get(&d.cell) {
                return Err(format!("{:?}: {e}", d.cell));
            }
        }
        let ids: Vec<_> = definitions.iter().map(|d| d.id).collect();
        let key = hash((&query.function, &query.key, ids));
        let affinity = roots
            .first()
            .map_or_else(|| definitions.first().map_or(key, |d| d.id), |r| hash(&r.0));
        let positions: HashMap<_, _> = definitions
            .iter()
            .enumerate()
            .flat_map(|(i, d)| d.outputs.iter().map(move |h| (h.clone(), i)))
            .collect();
        let mut commits: HashMap<_, Vec<_>> = HashMap::new();
        for (root, inputs) in roots {
            let after = std::iter::once(&root)
                .chain(&inputs)
                .filter_map(|h| positions.get(h))
                .max()
                .copied()
                .unwrap_or(0);
            commits.entry(after).or_default().push((root, inputs));
        }
        Ok((
            Spec {
                definitions,
                commits,
                query,
                retirement: None,
            },
            key,
            affinity,
        ))
    }
}

/// Capture only changed constructor outputs; unchanged recalculation sends no definitions.
pub fn record(function: &str, args: &[*const XLOPER12]) -> Result<(), XllError> {
    if on_worker() {
        return Ok(());
    }
    let (cell, writes, epoch, invocation) = registry::with_registry(|r| {
        (
            r.engine_cell(),
            r.engine_writes(),
            r.engine_epoch(),
            r.invocation(),
        )
    });
    let Some(cell) = cell else {
        return Ok(());
    };
    if writes.is_empty() {
        return Ok(());
    }
    let args = args
        .iter()
        .map(|p| Value::capture(*p).map_err(error))
        .collect::<Result<Vec<_>, _>>()?;
    let key = serde_json::to_string(&args).map_err(error)?;
    STATE.with(|state| {
        let mut s = state.borrow_mut();
        if writes
            .iter()
            .all(|h| s.definitions.get(h).is_some_and(|d| d.key == key))
        {
            return;
        }
        let d = Arc::new(Definition {
            id: s.id(),
            epoch,
            invocation,
            function: function.into(),
            args: Arc::new(args),
            cell,
            outputs: writes.clone(),
            key,
            sealed: None,
        });
        for handle in writes {
            s.definitions.insert(handle, d.clone());
        }
        s.changed = true;
        s.revision += 1;
        wake();
    });
    Ok(())
}
pub fn failed(cell: CellId, diagnostic: String) {
    if on_worker() {
        return;
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let roots: Vec<_> = s
            .definitions
            .values()
            .filter(|d| d.cell == cell && d.function.starts_with("qs_context_add_"))
            .filter_map(|d| match d.args.first() {
                Some(Value::Text(root)) => Some(root.clone()),
                _ => None,
            })
            .collect();
        for root in roots {
            s.invalid_roots.insert(root, (cell, diagnostic.clone()));
        }
        s.invalid.insert(cell, diagnostic);
        s.changed = true;
        s.revision += 1;
    });
    wake();
}
pub fn begin_source_pass(cell: CellId) {
    if on_worker() {
        return;
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.invalid.remove(&cell).is_some() {
            s.invalid_roots.retain(|_, (source, _)| *source != cell);
            s.changed = true;
            s.revision += 1;
            wake();
        }
    });
}
pub fn source_error(cell: CellId) -> Option<String> {
    STATE.with(|s| s.borrow().invalid.get(&cell).cloned())
}
pub fn release_cell(cell: CellId) {
    if on_worker() {
        return;
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.definitions
            .retain(|h, d| h.starts_with("QSEngine:") || d.cell != cell);
        s.invalid.remove(&cell);
        s.invalid_roots.retain(|_, (source, _)| *source != cell);
        let keys: Vec<_> = s
            .consumers
            .iter()
            .filter(|(_, c)| c.definition.cell == cell)
            .map(|(k, _)| k.clone())
            .collect();
        for key in keys {
            s.remove_consumer(&key);
        }
        s.changed = true;
        s.revision += 1;
    });
    wake();
}

pub fn worksheet(function: &str, args: &[*const XLOPER12]) -> Result<*mut XLOPER12, XllError> {
    if !ATTACHED.load(Ordering::Acquire) {
        return Err(error("Load the matching QuantSupport.Ribbon64.xll companion for background pricing (RTD bridge unavailable)."));
    }
    let args = normalize(
        function,
        args.iter()
            .map(|p| Value::capture(*p).map_err(error))
            .collect::<Result<Vec<_>, _>>()?,
    );
    if let Some(code) = args
        .iter()
        .find_map(Value::input_error)
        .filter(|e| *e != XLERR_NA)
    {
        registry::with_registry_mut(|r| {
            r.record_diagnostic(Some(
                if code == XLERR_NA {
                    "Calculating…"
                } else {
                    "A formula input contains an Excel error."
                }
                .into(),
            ))
        });
        return Ok(Value::Error(code).into_raw());
    }
    let (epoch, cell, invocation) =
        registry::with_registry(|r| (r.engine_epoch(), r.engine_cell(), r.invocation()));
    let cell = cell.ok_or_else(|| error("asynchronous pricing requires a worksheet caller"))?;
    let key = serde_json::to_string(&args).map_err(error)?;
    let topic = format!(
        "{epoch}:{}:{}:{}:{function}:{invocation}",
        cell.sheet, cell.row, cell.column
    );
    // Register the nested RTD dependency even for a ready value.
    let mut helper = XLOPER12::from_str("QS._ENGINE.SUBSCRIBE");
    // One RTD dependency per small column block. Logical consumers and worker
    // generations remain per invocation; only the change notification is shared.
    let mut token = XLOPER12::from_str(&notification_key(epoch, cell));
    let subscribe_began = Instant::now();
    let (status, mut result) = xll_rs::entrypoint::excel12(255, &mut [&mut helper, &mut token]);
    STATE.with(|s| {
        s.borrow_mut().metrics.subscription_ms += subscribe_began.elapsed().as_secs_f64() * 1000.
    });
    let connected =
        status == 0 && (result.base_type() != XLTYPE_ERR || unsafe { result.val.err } == XLERR_NA);
    let delivery_ready = status == 0
        && ((result.base_type() == XLTYPE_NUM && unsafe { result.val.num } >= 0.)
            || (result.base_type() == XLTYPE_INT && unsafe { result.val.w } >= 0));
    xll_rs::entrypoint::excel_free(&mut result);
    xlAutoFree12(Box::into_raw(Box::new(helper)));
    xlAutoFree12(Box::into_raw(Box::new(token)));
    if !connected {
        return Err(error("Load the matching QuantSupport.Ribbon64.xll companion for background pricing (RTD bridge unavailable)."));
    }
    let (value, diagnostic) = STATE.with(|state| {
        let mut s = state.borrow_mut();
        let replaced = s
            .consumers
            .get(&topic)
            .is_some_and(|c| c.definition.key != key);
        let generation = s.consumers.get(&topic).map_or(0, |c| c.generation + 1);
        if replaced {
            s.remove_consumer(&topic);
        }
        if !s.consumers.contains_key(&topic) {
            let definition = Arc::new(Definition {
                id: s.id(),
                epoch,
                invocation,
                function: function.into(),
                args: Arc::new(args),
                cell,
                outputs: Vec::new(),
                key,
                sealed: None,
            });
            let waiting_input = definition
                .args
                .iter()
                .any(|v| v.input_error() == Some(XLERR_NA));
            s.subscribe(
                topic.clone(),
                Consumer {
                    definition,
                    epoch,
                    generation,
                    fingerprint: 0,
                    job: None,
                    value: None,
                    error: String::new(),
                    affinity: 0,
                    validated_at: 0,
                    delivered_generation: u64::MAX,
                    waiting_input,
                    sources: Vec::new(),
                },
            );
            s.mark(topic.clone());
            wake();
        }
        let revision = s.revision;
        let dirty = s.dirty_set.contains(&topic);
        let c = s.consumers.get_mut(&topic).unwrap();
        let value = (delivery_ready && c.validated_at == revision && !dirty)
            .then(|| c.value.clone())
            .flatten();
        if value.is_some() {
            c.delivered_generation = c.generation;
        }
        (
            value,
            if c.error.is_empty() {
                "Calculating…".into()
            } else {
                c.error.clone()
            },
        )
    });
    registry::with_registry_mut(|r| {
        r.record_diagnostic(
            (value.is_none() || matches!(value.as_deref(), Some(Value::Error(_))))
                .then_some(diagnostic),
        )
    });
    Ok(value
        .unwrap_or_else(|| Arc::new(Value::Error(XLERR_NA)))
        .into_raw())
}

#[derive(Default, Serialize)]
struct Pulse {
    changed: Vec<String>,
    pending: usize,
    pending_delivery: usize,
    settled: bool,
    submitted: u64,
    completed: u64,
    error: String,
}
fn pulse() -> Pulse {
    let began = Instant::now();
    let ownership_changed = registry::commit_definition_pass();
    STATE.with(|state| {
        let mut s = state.borrow_mut(); let mut result = Pulse::default();
        if ownership_changed { s.changed = true; s.revision += 1; }
        if s.changed {
            s.changed = false;
            // A settled calculation pass is the commit barrier. Scan live definitions once.
            s.definitions.retain(|handle, _| handle.starts_with("QSEngine:") || registry::engine_handle_live(handle));
            let mut contributions: HashMap<String, Vec<String>> = HashMap::new();
            for (handle, definition) in &s.definitions {
                if definition.function.starts_with("qs_context_add_") {
                    if let Some(Value::Text(root)) = definition.args.first() { contributions.entry(root.clone()).or_default().push(handle.clone()); }
                }
            }
            s.contributions = contributions;
            registry::browser_changed();
            let roots: Vec<_> = s.definitions.iter().filter(|(_, d)| d.function == "qs_context_new").map(|(h, d)| (h.clone(), d.clone())).collect();
            for (root, definition) in roots {
                let mut ids: Vec<_> = s.contributions.get(&root).into_iter().flatten().filter_map(|h| s.definitions.get(h)).map(|d| d.id).collect(); ids.sort();
                let signature = hash((definition.id, ids));
                let counter = s.churn.entry(root.clone()).or_insert((signature, 0));
                if counter.0 != signature { counter.0 = signature; counter.1 += 1; }
                if counter.1 > 8 {
                    let source = s.contributions.get(&root).into_iter().flatten().filter_map(|h| s.definitions.get(h)).max_by_key(|d| d.id).map_or(definition.cell, |d| d.cell);
                    s.churn_invalid.insert(root, (source, "Market inputs changed repeatedly without an edit; a price-derived input may be circular. Remove that dependency or edit its source to retry.".into()));
                }
            }
            let keys: Vec<_> = s.consumers.keys().cloned().collect(); for key in keys { s.mark(key); }
        }
        for _ in 0..128 {
            if began.elapsed() > Duration::from_millis(12) { break; }
            let Some(key) = s.dirty.pop_front() else { break; }; s.dirty_set.remove(&key);
            let Some(c) = s.consumers.get(&key) else { continue; };
            if c.waiting_input { let revision = s.revision; s.consumers.get_mut(&key).unwrap().validated_at = revision; continue; }
            let (query, epoch) = (c.definition.clone(), c.epoch);
            if query.function == "qs_evaluate_range" && matches!(&query.args[0], Value::Array { values, .. } if values.len() > 1) {
                if let Err(error) = s.batch_step(&key, query) {
                    let revision = s.revision; let c = s.consumers.get_mut(&key).unwrap(); c.value = Some(Arc::new(Value::Error(XLERR_VALUE))); c.error = error; c.validated_at = revision; c.generation += 1; result.changed.push(key);
                }
                continue;
            }
            let (mut spec, fingerprint, affinity) = match s.snapshot(query) {
                Ok(v) => v,
                Err(e) => { let revision = s.revision; let c = s.consumers.get_mut(&key).unwrap(); c.generation += 1; c.value = Some(Arc::new(Value::Error(XLERR_VALUE))); c.error = e; c.validated_at = revision; c.fingerprint = 0; result.changed.push(key); continue; }
            };
            let revision = s.revision;
            s.consumers.get_mut(&key).unwrap().validated_at = revision;
            if s.consumers[&key].fingerprint == fingerprint { s.metrics.reused += 1; result.changed.push(key); continue; }
            if let Some((value, error)) = s.ready.get(&fingerprint).cloned() {
                let c = s.consumers.get_mut(&key).unwrap(); c.generation += 1; c.value = Some(value); c.error = error; c.fingerprint = fingerprint; c.job = None;
                s.metrics.reused += 1; result.changed.push(key); continue;
            }
            let old_job = s.consumers[&key].job;
            if let Some(old) = old_job {
                if let Some(f) = s.flights.get_mut(&old) {
                    f.subscribers.retain(|(k, _)| k != &key);
                    if f.subscribers.is_empty() { f.cancel.cancel(); s.metrics.canceled += 1; }
                }
            }
            let generation = s.consumers[&key].generation + 1;
            let existing = s.dedup.get(&fingerprint).copied().filter(|id| s.flights.get(id).is_some_and(|f| !f.cancel.canceled()));
            let id = if let Some(id) = existing { s.flights.get_mut(&id).unwrap().subscribers.push((key.clone(), generation)); id } else {
                let id = s.id(); let cancel = Cancellation::default();
                let mut query = (*spec.query).clone(); query.id = id; spec.query = Arc::new(query);
                if s.pool().submit(affinity, Job { id, epoch, generation: fingerprint, interactive: key.starts_with("ribbon:"), cancel: cancel.clone(), payload: spec.clone(), submitted: Instant::now() }).is_err() {
                    s.mark(key); break;
                }
                s.metrics.submitted += 1;
                s.dedup.insert(fingerprint, id);
                s.flights.insert(id, Flight { cancel, subscribers: vec![(key.clone(), generation)], spec, epoch, fingerprint }); id
            };
            let c = s.consumers.get_mut(&key).unwrap(); c.generation = generation; c.fingerprint = fingerprint; c.affinity = affinity;
            let was_ready = c.value.is_some();
            c.job = Some(id); c.value = None; c.error.clear(); if was_ready { result.changed.push(key); }
        }
        for completion_index in 0..256 {
            let Some(completion) = s.pool.as_ref().and_then(Scheduler::take) else { break; };
            if completion_index == 255 { wake(); }
            let Some(flight) = s.flights.remove(&completion.id) else { continue; };
            if s.dedup.get(&flight.fingerprint) == Some(&completion.id) { s.dedup.remove(&flight.fingerprint); }
            if flight.cancel.canceled() || flight.epoch != completion.epoch || flight.fingerprint != completion.generation { continue; }
            s.metrics.completed += 1; s.metrics.queue_ms += completion.queue_time.as_secs_f64()*1000.; s.metrics.native_ms += completion.run_time.as_secs_f64()*1000.;
            s.metrics.definitions_built += completion.result.definitions; s.metrics.context_builds += completion.result.contexts; s.metrics.curve_builds += completion.result.curves;
            let (mut value, failure) = match completion.result.result { Ok(v) => (Some(Arc::new(v)), String::new()), Err(e) => (None, e) };
            if value.as_deref().is_some_and(|v| matches!(v, Value::Text(h) if h.starts_with("QSObject:"))) {
                let kind = completion.result.object.as_ref().map_or("Result", |o| o.kind.as_str());
                let opaque = format!("QSEngine:{kind}:{}:{}", completion.epoch, completion.id);
                let mut definition = (*flight.spec.query).clone(); definition.outputs = vec![opaque.clone()];
                definition.sealed = Some(flight.spec.definitions.iter().flat_map(|d| d.outputs.iter().map(move |h| (h.clone(), d.id))).collect());
                s.definitions.insert(opaque.clone(), Arc::new(definition)); value = Some(Arc::new(Value::Text(opaque)));
                // A completed producer can replace a handle already read by a
                // dependent cell during this Manual-mode calculation. Revisit
                // those cells after delivering the new producer value.
                let dependents = s.dependents.get(&flight.spec.query.cell).cloned().unwrap_or_default();
                for key in dependents { if let Some(c) = s.consumers.get_mut(&key) { c.delivered_generation = u64::MAX; } }
                if let Some(mut object) = completion.result.object {
                    let handle = match value.as_deref() { Some(Value::Text(handle)) => handle.clone(), _ => unreachable!() };
                    object.handle = handle.clone(); object.name = format!("result.{}", completion.id); object.revision = completion.id; object.current_inputs = true;
                    let source = flight.spec.query.cell;
                    if source.sheet != 0 { object.locations.push(crate::runtime::browser::BrowserLocation { sheet_id: source.sheet as u64, sheet: String::new(), row: source.row + 1, column: source.column + 1, formula: String::new() }); }
                    for arg in flight.spec.query.args.iter() { arg.handles(&mut object.dependencies); }
                    s.objects.insert(handle, (completion.epoch, object));
                    registry::browser_changed();
                }
            }
            if let Some(value) = &value {
                s.ready.insert(flight.fingerprint, (value.clone(), String::new())); s.ready_order.push_back(flight.fingerprint);
                if s.ready_order.len() > 32768 { if let Some(old) = s.ready_order.pop_front() { s.ready.remove(&old); } }
            }
            let revision = s.revision;
            for (key, generation) in flight.subscribers {
                if let Some(c) = s.consumers.get_mut(&key) {
                    if c.validated_at != revision { c.fingerprint = 0; c.job = None; continue; }
                    if c.epoch == completion.epoch && c.generation == generation && c.job == Some(completion.id) {
                        c.value = value.clone().or_else(|| Some(Arc::new(Value::Error(XLERR_VALUE)))); c.error = failure.clone();
                        c.job = None; result.changed.push(key);
                    }
                }
            }
        }
        s.finish_batches(&mut result.changed);
        s.metrics.definitions = s.definitions.len(); s.metrics.subscriptions = s.consumers.keys().filter(|key| !key.starts_with("batch:")).count(); s.metrics.workers = s.pool.as_ref().map_or(0, Scheduler::workers);
        s.metrics.pending = s.consumers.values().filter(|c| c.value.is_none()).count();
        let elapsed_ms = began.elapsed().as_secs_f64()*1000.; s.metrics.commit_ms += elapsed_ms; s.metrics.max_dispatch_ms = s.metrics.max_dispatch_ms.max(elapsed_ms);
        result.pending = s.consumers.values().filter(|c| c.value.is_none() || c.validated_at != s.revision).count(); result.settled = !s.changed && s.dirty.is_empty();
        result.pending_delivery = s.consumers.iter().filter(|(_, c)| c.definition.cell.sheet != 0 && c.value.is_some() && c.delivered_generation != c.generation).count();
        result.changed = result.changed.into_iter().filter_map(|key| s.consumers.get(&key).filter(|c| c.definition.cell.sheet != 0).map(|c| notification_key(c.epoch, c.definition.cell))).collect();
        result.changed.sort_unstable(); result.changed.dedup();
        result.submitted = s.metrics.submitted; result.completed = s.metrics.completed;
        if !s.dirty.is_empty() { wake(); } result
    })
}
pub fn retire(epoch: u64) {
    if on_worker() {
        return;
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let keys: Vec<_> = s
            .consumers
            .iter()
            .filter(|(_, c)| c.epoch == epoch)
            .map(|(key, _)| key.clone())
            .collect();
        for key in keys {
            s.remove_consumer(&key);
        }
        s.flights.retain(|_, f| {
            if f.epoch == epoch {
                f.cancel.cancel();
                false
            } else {
                true
            }
        });
        let live: HashSet<_> = s.flights.keys().copied().collect();
        s.dedup.retain(|_, id| live.contains(id));
        s.definitions.retain(|_, d| d.epoch != epoch);
        s.objects.retain(|_, (e, _)| *e != epoch);
        s.ready.clear();
        s.ready_order.clear();
        if let Some(pool) = &s.pool {
            let query = Arc::new(Definition {
                id: 0,
                epoch,
                invocation: 0,
                function: String::new(),
                args: Arc::new(Vec::new()),
                cell: CellId {
                    sheet: 0,
                    row: 0,
                    column: 0,
                },
                outputs: Vec::new(),
                key: String::new(),
                sealed: None,
            });
            for worker in 0..pool.workers() {
                pool.submit_control(
                    worker as u64,
                    Job {
                        id: 0,
                        epoch,
                        generation: 0,
                        interactive: true,
                        cancel: Cancellation::default(),
                        payload: Spec {
                            definitions: Vec::new(),
                            commits: HashMap::new(),
                            query: query.clone(),
                            retirement: Some(epoch),
                        },
                        submitted: Instant::now(),
                    },
                );
            }
        }
        s.changed = true;
    });
    wake();
}
pub fn start() {
    let _ = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build_global();
    MAIN.store(unsafe { GetCurrentThreadId() } as usize, Ordering::Release);
    // Workers can finish after xlAutoClose. Keep their executing DLL mapped until process exit.
    unsafe {
        let mut module = windows::Win32::Foundation::HMODULE::default();
        let _ = GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
            windows::core::PCWSTR(start as *const () as *const u16),
            &mut module,
        );
        if EVENT.load(Ordering::Acquire) == 0 {
            if let Ok(event) = CreateEventW(None, false, false, None) {
                EVENT.store(event.0 as usize, Ordering::Release);
            }
        }
    }
}
pub fn reset() {
    if on_worker() {
        return;
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        for f in s.flights.values() {
            f.cancel.cancel();
        }
        let next = s.next;
        *s = State::default();
        s.next = next;
    });
}
pub fn user_edit() {
    if on_worker() {
        return;
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.churn.clear();
        if !s.churn_invalid.is_empty() {
            s.churn_invalid.clear();
            s.changed = true;
            s.revision += 1;
            wake();
        }
    });
}
pub fn stop() {
    MAIN.store(0, Ordering::Release);
    reset();
}
fn json(value: &impl Serialize) -> *mut c_char {
    CString::new(serde_json::to_string(value).unwrap_or_else(|_| "{}".into()))
        .unwrap()
        .into_raw()
}

fn browser_replay(args: &[Value]) -> Result<Value, String> {
    let text = |i| match &args[i] {
        Value::Text(s) => Ok(s.as_str()),
        _ => Err("invalid browser job".to_string()),
    };
    let mut query: crate::runtime::browser_queries::BrowserQuery =
        serde_json::from_str(text(3)?).map_err(|e| e.to_string())?;
    query.handle = text(0)?.into();
    query.context = text(1)?.into();
    query.pricer = text(2)?.into();
    registry::with_registry(|r| crate::runtime::browser_queries::execute(r, &query))
        .map(|v| Value::Browser(Box::new(v)))
}
xll_rs::inventory::submit! { Replay { name: "__browser", invoke: browser_replay } }

#[derive(Serialize)]
struct Ticket {
    job: u64,
    pending: bool,
    result: String,
    error: String,
    total_rows: usize,
    revision: u64,
}
fn browser_table(value: &Value, offset: usize, limit: usize) -> (String, usize) {
    use crate::runtime::browser_queries::BrowserResponse;
    let text = |v: &Value| match v {
        Value::Text(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Boolean(b) => b.to_string(),
        _ => String::new(),
    };
    let total = match value {
        Value::Browser(table) => table.rows.len(),
        Value::Array { rows, .. } => rows.saturating_sub(1),
        _ => 1,
    };
    let result = match value {
        Value::Browser(table) => BrowserResponse {
            title: table.title.clone(),
            message: table.message.clone(),
            curves: table.curves.clone(),
            columns: table.columns.clone(),
            numeric_columns: table.numeric_columns.clone(),
            error: table.error.clone(),
            rows: table
                .rows
                .iter()
                .skip(offset)
                .take(limit)
                .cloned()
                .collect(),
            parameters: table
                .parameters
                .iter()
                .skip(offset)
                .take(limit)
                .cloned()
                .collect(),
        },
        Value::Array {
            columns, values, ..
        } if *columns > 0 => BrowserResponse {
            title: "Pricing results".into(),
            columns: values.iter().take(*columns).map(text).collect(),
            rows: values[*columns..]
                .chunks(*columns)
                .skip(offset)
                .take(limit)
                .map(|row| row.iter().map(text).collect())
                .collect(),
            numeric_columns: (0..*columns)
                .filter(|col| {
                    values
                        .iter()
                        .skip(*columns + col)
                        .step_by(*columns)
                        .take(256)
                        .any(|v| matches!(v, Value::Number(_)))
                })
                .collect(),
            ..BrowserResponse::default()
        },
        Value::Number(n) => BrowserResponse {
            title: "Pricing results".into(),
            columns: vec!["Value".into()],
            rows: vec![vec![n.to_string()]],
            numeric_columns: vec![0],
            ..BrowserResponse::default()
        },
        _ => BrowserResponse::error("The requested result is unavailable."),
    };
    (serde_json::to_string(&result).unwrap_or_default(), total)
}
fn cancel(key: &str) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if key.starts_with("cells:") {
            let group: Vec<_> = key
                .split(':')
                .skip(1)
                .filter_map(|n| n.parse::<u64>().ok())
                .collect();
            if group.len() == 4 {
                let keys: Vec<_> = s
                    .consumers
                    .iter()
                    .filter(|(_, c)| {
                        c.epoch == group[0]
                            && c.definition.cell.sheet as u64 == group[1]
                            && c.definition.cell.column as u64 == group[2]
                            && (c.definition.cell.row / 64) as u64 == group[3]
                    })
                    .map(|(key, _)| key.clone())
                    .collect();
                for key in keys {
                    s.remove_consumer(&key);
                }
            }
        } else {
            s.remove_consumer(key);
        }
    });
}
#[no_mangle]
pub unsafe extern "system" fn qs_engine_submit(request: *const u16, length: usize) -> *mut c_char {
    let result = std::panic::catch_unwind(|| -> Result<u64, String> {
        if MAIN.load(Ordering::Acquire) != unsafe { GetCurrentThreadId() } as usize {
            return Err("submit on Excel's main thread".into());
        }
        if request.is_null() || length > 1_048_576 {
            return Err("invalid engine request".into());
        }
        let text = String::from_utf16(unsafe { std::slice::from_raw_parts(request, length) })
            .map_err(|e| e.to_string())?;
        let query: crate::runtime::browser_queries::BrowserQuery =
            serde_json::from_str(&text).map_err(|e| e.to_string())?;
        let epoch = crate::runtime::lifecycle::workbook_id(&query.workbook)
            .ok_or("workbook session is closed")?;
        let shared = query.view == "evaluate"
            && query.trade_date.is_empty()
            && query.side.is_empty()
            && query.pricing_anchor.is_empty()
            && query.anchor_value.is_none();
        let request = if query.request == "FairRate" {
            format!("FairRate({})", query.target)
        } else {
            query.request.clone()
        };
        let args = if shared {
            normalize(
                "qs_evaluate",
                vec![
                    Value::Text(query.context.clone()),
                    Value::Text(query.handle.clone()),
                    Value::Text(request),
                    Value::Text(query.pricer.clone()),
                    Value::Missing,
                    Value::Missing,
                    Value::Missing,
                ],
            )
        } else {
            vec![
                Value::Text(query.handle.clone()),
                Value::Text(query.context.clone()),
                Value::Text(query.pricer.clone()),
                Value::Text(text.clone()),
            ]
        };
        let definition_key = serde_json::to_string(&args).map_err(|e| e.to_string())?;
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            let id = s.id();
            let key = format!("ribbon:{id}");
            let definition = Arc::new(Definition {
                id,
                epoch,
                invocation: 0,
                function: if shared { "qs_evaluate" } else { "__browser" }.into(),
                args: Arc::new(args),
                cell: CellId {
                    sheet: 0,
                    row: id as i32,
                    column: 0,
                },
                outputs: Vec::new(),
                key: definition_key,
                sealed: None,
            });
            s.subscribe(
                key.clone(),
                Consumer {
                    definition,
                    epoch,
                    generation: 0,
                    fingerprint: 0,
                    job: None,
                    value: None,
                    error: String::new(),
                    affinity: 0,
                    validated_at: 0,
                    delivered_generation: 0,
                    waiting_input: false,
                    sources: Vec::new(),
                },
            );
            s.mark(key);
            wake();
            Ok(id)
        })
    })
    .unwrap_or_else(|_| Err("engine submission failed".into()));
    json(&match result {
        Ok(job) => Ticket {
            job,
            pending: true,
            result: String::new(),
            error: String::new(),
            total_rows: 0,
            revision: 0,
        },
        Err(error) => Ticket {
            job: 0,
            pending: false,
            result: String::new(),
            error,
            total_rows: 0,
            revision: 0,
        },
    })
}
#[no_mangle]
pub extern "system" fn qs_engine_take(job: u64) -> *mut c_char {
    qs_engine_take_page(job, 0, 256)
}
#[no_mangle]
pub extern "system" fn qs_engine_take_page(job: u64, offset: usize, limit: usize) -> *mut c_char {
    if MAIN.load(Ordering::Acquire) != unsafe { GetCurrentThreadId() } as usize {
        return json(&Ticket {
            job,
            pending: false,
            result: String::new(),
            error: "read results on Excel's main thread".into(),
            total_rows: 0,
            revision: 0,
        });
    }
    STATE.with(|s| {
        let s = s.borrow();
        let key = format!("ribbon:{job}");
        json(&if let Some(c) = s.consumers.get(&key) {
            let (result, total_rows) = c
                .value
                .as_deref()
                .map(|v| browser_table(v, offset, limit.clamp(1, 256)))
                .unwrap_or_default();
            Ticket {
                job,
                pending: c.value.is_none() || c.validated_at != s.revision,
                result,
                total_rows,
                revision: c.generation,
                error: c.error.clone(),
            }
        } else {
            Ticket {
                job,
                pending: false,
                result: String::new(),
                error: "job was canceled or workbook closed".into(),
                total_rows: 0,
                revision: 0,
            }
        })
    })
}
#[no_mangle]
pub extern "system" fn qs_engine_cancel(job: u64) {
    if MAIN.load(Ordering::Acquire) == unsafe { GetCurrentThreadId() } as usize {
        cancel(&format!("ribbon:{job}"));
    }
}
#[no_mangle]
pub extern "system" fn qs_engine_api_version() -> u32 {
    1
}
#[no_mangle]
pub extern "system" fn qs_engine_attach(version: u32) -> i32 {
    if MAIN.load(Ordering::Acquire) != unsafe { GetCurrentThreadId() } as usize {
        return 0;
    }
    ATTACHED.store(version == 1, Ordering::Release);
    i32::from(version == 1)
}
#[no_mangle]
pub unsafe extern "system" fn qs_engine_unsubscribe(topic: *const u16, length: usize) {
    if MAIN.load(Ordering::Acquire) != unsafe { GetCurrentThreadId() } as usize
        || topic.is_null()
        || length > 32767
    {
        return;
    }
    if let Ok(key) = String::from_utf16(unsafe { std::slice::from_raw_parts(topic, length) }) {
        cancel(&key);
    }
}
#[no_mangle]
pub extern "system" fn qs_engine_event() -> usize {
    EVENT.load(Ordering::Acquire)
}
#[no_mangle]
pub extern "system" fn qs_engine_pulse() -> *mut c_char {
    if MAIN.load(Ordering::Acquire) != unsafe { GetCurrentThreadId() } as usize {
        return json(&Pulse {
            error: "engine dispatch requires Excel's main thread".into(),
            ..Pulse::default()
        });
    }
    json(&std::panic::catch_unwind(pulse).unwrap_or_else(|_| Pulse {
        error: "engine dispatch failed".into(),
        ..Pulse::default()
    }))
}
#[no_mangle]
pub extern "system" fn qs_engine_metrics() -> *mut c_char {
    STATE.with(|s| json(&s.borrow().metrics))
}
pub fn record_call(caller_ms: f64, total_ms: f64) {
    if on_worker() {
        return;
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.metrics.worksheet_calls += 1;
        s.metrics.caller_ms += caller_ms;
        s.metrics.worksheet_ms += total_ms;
    });
}
#[no_mangle]
pub extern "system" fn qs_engine_record_delivery(milliseconds: f64) {
    if MAIN.load(Ordering::Acquire) != unsafe { GetCurrentThreadId() } as usize
        || !milliseconds.is_finite()
    {
        return;
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.metrics.delivery_ms += milliseconds;
        s.metrics.delivery_passes += 1;
        s.metrics.max_delivery_ms = s.metrics.max_delivery_ms.max(milliseconds);
    });
}
#[derive(Serialize)]
struct DeliveryTarget {
    sheet: usize,
    row: i32,
    column: i32,
}
#[no_mangle]
pub extern "system" fn qs_engine_refresh_targets() -> *mut c_char {
    STATE.with(|s| {
        let s = s.borrow();
        let mut seen = HashSet::new();
        let mut targets: Vec<_> = s
            .consumers
            .iter()
            .filter(|(_, c)| {
                c.definition.cell.sheet != 0
                    && (c.waiting_input
                        || (c.value.is_some() && c.delivered_generation != c.generation))
            })
            .map(|(_, c)| (c.waiting_input, c.definition.cell))
            .filter(|(_, cell)| seen.insert(*cell))
            .collect();
        // Ready parents precede waiting dependents. Ordered cells become short
        // contiguous Excel ranges rather than a long union of random addresses.
        targets
            .sort_unstable_by_key(|(waiting, cell)| (*waiting, cell.sheet, cell.column, cell.row));
        let targets: Vec<_> = targets
            .into_iter()
            .take(128)
            .map(|(_, cell)| DeliveryTarget {
                sheet: cell.sheet,
                row: cell.row,
                column: cell.column,
            })
            .collect();
        json(&targets)
    })
}
#[no_mangle]
pub unsafe extern "system" fn qs_engine_free(value: *mut c_char) {
    if !value.is_null() {
        drop(unsafe { CString::from_raw(value) });
    }
}
