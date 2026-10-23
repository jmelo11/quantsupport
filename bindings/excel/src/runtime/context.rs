//! Declarative context inputs. Every input refers to the same root; the price
//! formula supplies the complete input range, making Excel dependencies explicit.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    rc::{Rc, Weak},
};

use quantsupport::prelude::*;
use std::result::Result;

use super::{
    inputs::Table,
    market_inputs,
    pricing::PricerObject,
    products::PositionTerms,
    registry::{QsObject, Registry},
};

/// Worksheet-owned contribution, independent of every other contribution.
pub struct ContextInput {
    pub context: String,
    pub data: InputData,
}

pub enum InputData {
    Quotes(QuoteStore),
    Fixings(FixingStore),
    Fx(Vec<FxRateRecord>),
    Curve {
        configuration: CurveConfiguration,
        quotes: QuoteStore,
    },
    Credit {
        configuration: CreditCurveConfiguration,
        quotes: QuoteStore,
    },
    Surface {
        configuration: VolatilitySurfaceConfiguration,
        quotes: QuoteStore,
    },
    Cube {
        configuration: VolatilityCubeConfiguration,
        quotes: QuoteStore,
    },
    Pricer(PricerConfiguration),
    Simulation(SimulationConfiguration),
}

/// Composed contexts have bounded residency. Historical contexts are shared for
/// as long as a worksheet object (or a batch) owns them, without pinning history.
#[derive(Default)]
pub struct ContextCache {
    prepared: VecDeque<(String, Rc<PricingContext>)>,
    historical: HashMap<String, Weak<PricingContext>>,
    historical_insertions: usize,
}

impl ContextCache {
    pub fn clear(&mut self) {
        self.prepared.clear();
        self.historical.clear();
        self.historical_insertions = 0;
    }

    pub fn historical(&self, key: &str) -> Option<Rc<PricingContext>> {
        self.historical.get(key).and_then(Weak::upgrade)
    }

    pub fn remember_historical(&mut self, key: String, context: &Rc<PricingContext>) {
        self.historical_insertions += 1;
        if self.historical_insertions == 1024 {
            self.historical.retain(|_, value| value.strong_count() > 0);
            self.historical_insertions = 0;
        }
        self.historical.insert(key, Rc::downgrade(context));
    }
}

/// Historical pricing can reuse a context's fixing input directly.
pub fn fixing_history<'a>(registry: &'a Registry, handle: &str) -> Result<&'a FixingStore, String> {
    match registry.object(handle).map_err(|e| e.to_string())? {
        #[cfg(test)]
        QsObject::FixingStore(store) => Ok(store),
        QsObject::PricingContext(context) => Ok(context.fixing_store()),
        QsObject::ContextInput(input) => match &input.data {
            InputData::Fixings(store) => Ok(store),
            _ => Err("expected a fixing input".into()),
        },
        _ => Err("expected a prepared context or CONTEXT.ADD.FIXINGS input".into()),
    }
}

pub fn create(
    registry: &mut Registry,
    name: &str,
    date: Date,
    currency: Currency,
    index: MarketIndex,
    level: Level,
) -> Result<String, String> {
    let key = serde_json::to_string(&(date, currency, &index, level)).map_err(|e| e.to_string())?;
    if let Some(handle) = registry
        .handle_if_unchanged(name, "PricingContext", &key)
        .map_err(|e| e.to_string())?
    {
        return Ok(handle);
    }
    let context = PricingContext::new()
        .with_quote_store(QuoteStore::new(date))
        .with_base_currency(currency)
        .with_base_index(index)
        .with_quote_level(level);
    registry
        .upsert_if_changed(name, QsObject::PricingContext(Rc::new(context)), key)
        .map_err(|e| e.to_string())
}

pub fn add(
    registry: &mut Registry,
    name: &str,
    context: &str,
    data: InputData,
    key: &str,
) -> Result<String, String> {
    registry
        .pricing_context(context)
        .map_err(|e| e.to_string())?;
    let key = serde_json::to_string(&(context, key)).map_err(|e| e.to_string())?;
    registry
        .upsert_if_changed(
            name,
            QsObject::ContextInput(Box::new(ContextInput {
                context: context.into(),
                data,
            })),
            key,
        )
        .map_err(|e| e.to_string())
}

/// Defines a curve from normalized array fields, or from identifiers selecting
/// existing context quotes. Empty identifiers select matching market indices.
pub fn curve(
    date: Date,
    index: MarketIndex,
    table: &Table,
    day_counter: DayCounter,
    interpolator: Interpolator,
    extrapolate: bool,
) -> Result<InputData, String> {
    let (identifiers, quotes) = market_inputs::definition_quotes(date, table)?;
    Ok(InputData::Curve {
        configuration: CurveConfiguration::new(
            index,
            day_counter,
            interpolator,
            extrapolate,
            identifiers,
        ),
        quotes,
    })
}

pub fn prepare(
    registry: &Registry,
    context: &str,
    inputs: &[String],
) -> Result<Rc<PricingContext>, String> {
    let base = match registry.object(context).map_err(|e| e.to_string())? {
        QsObject::PricingContext(value) => value,
        other => {
            return Err(format!(
                "expected PricingContext, received {}",
                other.kind()
            ))
        }
    };
    let mut handles = inputs.to_vec();
    handles.sort();
    if handles.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err("the input range contains the same context input more than once".into());
    }
    // Always validate dependencies, including on cache hits.
    let inputs = handles
        .iter()
        .map(
            |handle| match registry.object(handle).map_err(|e| e.to_string())? {
                QsObject::ContextInput(input) if input.context == context => Ok(input.as_ref()),
                QsObject::ContextInput(_) => Err("context input belongs to another context".into()),
                other => Err(format!("expected ContextInput, received {}", other.kind())),
            },
        )
        .collect::<Result<Vec<_>, String>>()?;
    if inputs.is_empty() {
        return Ok(Rc::clone(base));
    }
    let key = serde_json::to_string(&(context, handles)).map_err(|e| e.to_string())?;
    {
        let mut cache = registry.context_cache.borrow_mut();
        if let Some(position) = cache.prepared.iter().position(|(stored, _)| stored == &key) {
            let entry = cache.prepared.remove(position).unwrap();
            let value = Rc::clone(&entry.1);
            cache.prepared.push_back(entry);
            return Ok(value);
        }
    }
    let mut quotes = base.quote_store().clone();
    for input in &inputs {
        let store = match &input.data {
            InputData::Quotes(store)
            | InputData::Curve { quotes: store, .. }
            | InputData::Credit { quotes: store, .. }
            | InputData::Surface { quotes: store, .. }
            | InputData::Cube { quotes: store, .. } => store,
            _ => continue,
        };
        for (id, quote) in store.quotes() {
            if let Some(previous) = quotes.quote(id) {
                let a = previous.levels();
                let b = quote.levels();
                if (a.mid().is_some() && b.mid().is_some())
                    || (a.bid().is_some() && b.bid().is_some())
                    || (a.ask().is_some() && b.ask().is_some())
                {
                    return Err(format!("duplicate context quote level: {id}"));
                }
                quotes.add_quote(Quote::new(
                    quote.details().clone(),
                    QuoteLevels::new(
                        a.mid().or(b.mid()),
                        a.bid().or(b.bid()),
                        a.ask().or(b.ask()),
                    ),
                ));
            } else {
                quotes.add_quote(quote.clone());
            }
        }
    }
    let mut curves = base.curve_configurations().clone();
    let mut credit = base.credit_curve_configurations().clone();
    let mut surfaces = base.volatility_surface_configurations().clone();
    let mut cubes = base.volatility_cube_configurations().clone();
    let mut pricers = base.pricer_configurations().clone();
    let mut simulations = base.simulation_configurations().clone();
    let mut indices: HashSet<_> = curves.iter().map(|c| c.market_index().clone()).collect();
    let mut fixings: Option<FixingStore> = if base.fixing_store().observations().next().is_some() {
        Some(base.fixing_store().clone())
    } else {
        None
    };
    let mut fx = base.fx_store().clone();
    let mut fx_pairs = HashSet::new();
    for input in &inputs {
        match &input.data {
            InputData::Curve { configuration, .. } => {
                let index = configuration.market_index();
                if !indices.insert(index.clone()) {
                    return Err(format!("duplicate context curve: {index}"));
                }
                let configuration = if configuration.quotes().is_empty()
                    && matches!(
                        configuration.construction(),
                        CurveConstruction::Bootstrapped
                    ) {
                    let mut ids: Vec<_> = quotes
                        .quotes()
                        .iter()
                        .filter(|(_, quote)| {
                            quote.details().market_index() == Some(index)
                                && quote.details().vol_type().is_none()
                        })
                        .map(|(id, _)| id.clone())
                        .collect();
                    ids.sort();
                    if ids.is_empty() {
                        return Err(format!("no quotes match curve {index}"));
                    }
                    CurveConfiguration::new(
                        index.clone(),
                        configuration.day_counter(),
                        configuration.interpolator(),
                        configuration.enable_extrapolation(),
                        ids,
                    )
                } else {
                    configuration.clone()
                };
                curves.push(configuration);
            }
            InputData::Fixings(store) => {
                if let Some(combined) = &mut fixings {
                    for (index, date, value) in store.observations() {
                        if combined.fixing(index, date).is_ok() {
                            return Err(format!("duplicate context fixing: {index} on {date}"));
                        }
                        combined.add_fixing(index, date, value);
                    }
                } else {
                    fixings = Some(store.clone());
                }
            }
            InputData::Fx(records) => {
                for record in records {
                    if !fx_pairs.insert((record.base, record.quote)) {
                        return Err("duplicate context FX pair".into());
                    }
                    fx.add_fx_rate(record.base, record.quote, DualFwd::from(record.rate));
                }
            }
            InputData::Credit { configuration, .. } => credit.push(configuration.clone()),
            InputData::Surface { configuration, .. } => surfaces.push(configuration.clone()),
            InputData::Cube { configuration, .. } => cubes.push(configuration.clone()),
            InputData::Pricer(configuration) => pricers.push(configuration.clone()),
            InputData::Simulation(configuration) => simulations.push(configuration.clone()),
            InputData::Quotes(_) => {}
        }
    }
    let mut result = base
        .as_ref()
        .clone()
        .with_quote_store(quotes)
        .with_fx_store(fx)
        .with_curve_configurations(curves)
        .with_credit_curve_configurations(credit)
        .with_volatility_surface_configurations(surfaces)
        .with_volatility_cube_configurations(cubes)
        .with_pricer_configurations(pricers)
        .with_simulation_configurations(simulations);
    if let Some(fixings) = fixings {
        result = result.with_fixing_store(fixings);
    }
    result.initialize().map_err(|e| e.to_string())?;
    let result = Rc::new(result);
    let mut cache = registry.context_cache.borrow_mut();
    if cache.prepared.len() == 128 {
        cache.prepared.pop_front();
    }
    cache.prepared.push_back((key, Rc::clone(&result)));
    Ok(result)
}

/// Direct contract pricing uses a Receive position at the context date. Pass a
/// trade for another side/date, notional override, or bond anchor.
pub fn evaluate(
    registry: &Registry,
    context: &PricingContext,
    instrument: &str,
    request: &Request,
    pricer: Option<&str>,
) -> Result<EvaluationResults, String> {
    evaluate_requests(
        registry,
        context,
        instrument,
        std::slice::from_ref(request),
        pricer,
        None,
    )
}

/// Common dispatch for direct values, stored results and historical batches.
pub fn evaluate_requests(
    registry: &Registry,
    context: &PricingContext,
    instrument: &str,
    requests: &[Request],
    pricer: Option<&str>,
    policy: Option<&str>,
) -> Result<EvaluationResults, String> {
    Request::validate(requests).map_err(|e| e.to_string())?;
    if pricer.is_some() && policy.is_some() {
        return Err(
            "the explicit pricer already owns its discount policy; leave policy blank".into(),
        );
    }
    let object = registry.object(instrument).map_err(|e| e.to_string())?;
    let position;
    let trade = if let QsObject::Instrument(instrument) = object {
        position = instrument.trade(
            context.evaluation_date(),
            Side::LongReceive,
            &PositionTerms::default(),
        )?;
        &position
    } else {
        object
    };
    let default;
    let pricer = if let Some(handle) = pricer {
        registry.pricer(handle).map_err(|e| e.to_string())?
    } else {
        let implementation = match trade {
            QsObject::LoanTrade(_) => "LoanPricer",
            QsObject::FixedRateBondTrade(_) => "BondPricer",
            QsObject::FxForwardTrade(_) => "FxForwardPricer",
            QsObject::FxEuropeanOptionTrade(_) => "FxEuropeanOptionPricer",
            QsObject::EquityEuropeanOptionTrade(_) => "BlackEuropeanOptionPricer",
            QsObject::EquityForwardTrade(_) => "EquityForwardPricer",
            QsObject::CdsTrade(_) => "CdsPricer",
            QsObject::CapFloorTrade(_) => "ClosedFormBlackCapPricer",
            QsObject::CapletFloorletTrade(_) => "ClosedFormBlackCapletPricer",
            QsObject::RateFuturesTrade(_) => "RateFuturesPricer",
            QsObject::EuropeanSwaptionTrade(_) => {
                return Err("supply an explicit swaption pricer with model parameters".into())
            }
            _ => "DiscountedCashflowPricer",
        };
        let default_policy =
            super::pricing::PolicyObject {
                native: if matches!(
                    trade,
                    QsObject::LoanTrade(_) | QsObject::FixedRateBondTrade(_)
                ) {
                    std::sync::Arc::new(FixedIncomeDiscountPolicy::new(false).with_risk_free_index(
                        context.base_currency(),
                        context.base_index().clone(),
                    ))
                } else {
                    std::sync::Arc::new(context.csa_policy())
                },
                kind: "ContextDiscountPolicy",
            };
        default = PricerObject::new(
            implementation,
            if implementation == "RateFuturesPricer" {
                None
            } else {
                Some(match policy {
                    Some(handle) => registry
                        .discount_policy(handle)
                        .map_err(|e| e.to_string())?,
                    None => &default_policy,
                })
            },
            &vec![],
        )?;
        &default
    };
    if policy.is_some() && matches!(trade, QsObject::RateFuturesTrade(_)) {
        return Err("RateFuturesPricer does not use a discount policy".into());
    }
    pricer.evaluate(trade, requests, context)
}

pub fn scalar(results: &EvaluationResults, request: &Request) -> Result<f64, String> {
    let value = match request {
        Request::Value => results.price(),
        Request::FairRate(_) => results.fair_rate(),
        Request::YieldToMaturity => results.ytm(),
        Request::ModifiedDuration => results.modified_duration(),
        Request::ZSpread => results.z_spread(),
        Request::OAS => results.oas(),
        _ => return Err("a batch requires a scalar request".into()),
    };
    value.ok_or_else(|| "the selected pricer did not supply the requested measure".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{
        inputs::Cell,
        products::{InstrumentObject, NativeInstrument},
        registry::CellId,
    };

    fn root(registry: &mut Registry) -> String {
        create(
            registry,
            "root",
            Date::new(2025, 1, 1),
            Currency::USD,
            MarketIndex::SOFR,
            Level::Mid,
        )
        .unwrap()
    }

    fn quotes(rate: f64) -> Table {
        vec![vec![
            Cell::Text("FixedRateDeposit_USD_SOFR_6M".into()),
            Cell::Number(rate),
        ]]
    }

    fn add_curve(registry: &mut Registry, root: &str, rate: f64) -> String {
        let data = curve(
            Date::new(2025, 1, 1),
            MarketIndex::SOFR,
            &quotes(rate),
            DayCounter::Actual360,
            Interpolator::LogLinear,
            true,
        )
        .unwrap();
        add(registry, "curve", root, data, &rate.to_string()).unwrap()
    }

    fn instrument(registry: &mut Registry) -> String {
        let deposit = MakeFixedRateDeposit::<DualFwd>::default()
            .with_identifier("deposit".into())
            .with_start_date(Date::new(2025, 1, 1))
            .with_maturity_date(Date::new(2025, 7, 1))
            .with_currency(Currency::USD)
            .with_notional(1000.0)
            .with_rate(0.05)
            .with_rate_definition(RateDefinition::new(
                DayCounter::Actual360,
                Compounding::Simple,
                Frequency::Annual,
            ))
            .build()
            .unwrap();
        registry
            .upsert_if_changed(
                "deposit",
                QsObject::Instrument(Box::new(InstrumentObject {
                    native: NativeInstrument::FixedRateDeposit(deposit),
                    notional: 1000.0,
                    foreign_notional: None,
                })),
                "deposit",
            )
            .unwrap()
    }

    #[test]
    fn direct_contract_value_matches_an_explicit_native_position_and_returns_risk_and_cashflows() {
        let mut registry = Registry::default();
        let root = root(&mut registry);
        let input = add_curve(&mut registry, &root, 0.04);
        let instrument = instrument(&mut registry);
        let context = prepare(&registry, &root, &[input]).unwrap();
        let pv = evaluate(&registry, &context, &instrument, &Request::Value, None)
            .unwrap()
            .price()
            .unwrap();
        let trade = registry
            .instrument(&instrument)
            .unwrap()
            .trade(
                context.evaluation_date(),
                Side::LongReceive,
                &PositionTerms::default(),
            )
            .unwrap();
        let mut pricer = DiscountedCashflowPricer::<
            FixedRateDeposit<DualFwd>,
            FixedRateDepositTrade<DualFwd>,
        >::new();
        pricer.set_discount_policy(Box::new(context.csa_policy()));
        let QsObject::FixedRateDepositTrade(trade) = trade else {
            panic!("deposit position");
        };
        let expected = pricer
            .evaluate(trade.as_ref(), &[Request::Value], context.as_ref())
            .unwrap()
            .price()
            .unwrap();
        assert!((pv - expected).abs() < 1e-12);
        assert!(
            evaluate(&registry, &context, &instrument, &Request::Cashflows, None)
                .unwrap()
                .cashflows()
                .is_some()
        );
        let risk = evaluate(
            &registry,
            &context,
            &instrument,
            &Request::Sensitivities,
            None,
        )
        .unwrap();
        assert!(risk
            .sensitivities()
            .unwrap()
            .exposure()
            .iter()
            .any(|value| value.abs() > 0.0));
        assert_eq!(
            registry.len(),
            3,
            "direct valuation stores no result or preview trade"
        );
    }

    #[test]
    fn unchanged_inputs_reuse_calibration_and_edits_or_removed_inputs_change_the_market() {
        let mut registry = Registry::default();
        let root = root(&mut registry);
        let input = add_curve(&mut registry, &root, 0.04);
        let first = prepare(&registry, &root, std::slice::from_ref(&input)).unwrap();
        assert!(Rc::ptr_eq(
            &first,
            &prepare(&registry, &root, std::slice::from_ref(&input)).unwrap()
        ));
        let changed = add_curve(&mut registry, &root, 0.08);
        assert!(
            prepare(&registry, &root, &[input]).is_err(),
            "stale inputs cannot hit the cache"
        );
        let second = prepare(&registry, &root, &[changed]).unwrap();
        let df = |context: &PricingContext| {
            context
                .constructed_elements()
                .discount_curve(&MarketIndex::SOFR)
                .unwrap()
                .curve()
                .discount_factor(Date::new(2025, 7, 1))
                .unwrap()
                .value()
        };
        assert!(df(&second) < df(&first));
        assert!(prepare(&registry, &root, &[])
            .unwrap()
            .constructed_elements()
            .discount_curves()
            .is_empty());
    }

    #[test]
    fn snapshots_capture_the_complete_input_range_and_fixing_inputs_can_be_shared_with_history() {
        let mut registry = Registry::default();
        let root = root(&mut registry);
        let curve = add_curve(&mut registry, &root, 0.04);
        let mut fixings = FixingStore::default();
        fixings.add_fixing(&MarketIndex::SOFR, Date::new(2024, 12, 31), 0.03);
        let fixings = add(
            &mut registry,
            "history",
            &root,
            InputData::Fixings(fixings),
            "fixings",
        )
        .unwrap();
        let snapshot = crate::runtime::historical::capture_inputs(
            &mut registry,
            "snapshot",
            "2025-01-01T16:00:00Z".parse().unwrap(),
            &root,
            &[curve, fixings.clone()],
        )
        .unwrap();
        let archive =
            crate::runtime::historical::archive(&mut registry, "archive", &[snapshot]).unwrap();
        let selector =
            crate::runtime::historical::selector("2025-01-01T17:00:00Z", None, None).unwrap();
        let restored = crate::runtime::historical::shared_context(
            &registry,
            &archive,
            &selector,
            Some(&fixings),
        )
        .unwrap();
        assert!(restored
            .constructed_elements()
            .discount_curve(&MarketIndex::SOFR)
            .is_some());
        assert_eq!(restored.quote_store().quotes().len(), 1);
        assert_eq!(
            restored
                .fixing_store()
                .fixing(&MarketIndex::SOFR, Date::new(2024, 12, 31))
                .unwrap(),
            0.03
        );
    }

    #[test]
    fn independent_quotes_are_matched_by_index_without_an_ordered_handle_chain() {
        let mut registry = Registry::default();
        let root = root(&mut registry);
        let quotes = add(
            &mut registry,
            "quotes",
            &root,
            InputData::Quotes(market_inputs::quotes(Date::new(2025, 1, 1), &quotes(0.04)).unwrap()),
            "quotes",
        )
        .unwrap();
        let curve = add(
            &mut registry,
            "definition",
            &root,
            curve(
                Date::new(2025, 1, 1),
                MarketIndex::SOFR,
                &vec![],
                DayCounter::Actual360,
                Interpolator::LogLinear,
                false,
            )
            .unwrap(),
            "curve",
        )
        .unwrap();
        let context = prepare(&registry, &root, &[curve.clone(), quotes.clone()]).unwrap();
        assert!(context
            .constructed_elements()
            .discount_curve(&MarketIndex::SOFR)
            .is_some());
        assert!(Rc::ptr_eq(
            &context,
            &prepare(&registry, &root, &[quotes, curve]).unwrap()
        ));
    }

    #[test]
    fn duplicate_or_foreign_inputs_fail_instead_of_overwriting_each_other() {
        let mut registry = Registry::default();
        let root = root(&mut registry);
        let curve = add_curve(&mut registry, &root, 0.04);
        assert!(prepare(&registry, &root, &[curve.clone(), curve.clone()]).is_err());
        let other = create(
            &mut registry,
            "other",
            Date::new(2025, 1, 1),
            Currency::USD,
            MarketIndex::SOFR,
            Level::Mid,
        )
        .unwrap();
        assert!(prepare(&registry, &other, std::slice::from_ref(&curve)).is_err());
        let duplicate = add(
            &mut registry,
            "duplicate",
            &root,
            InputData::Quotes(market_inputs::quotes(Date::new(2025, 1, 1), &quotes(0.05)).unwrap()),
            "duplicate",
        )
        .unwrap();
        assert!(prepare(&registry, &root, &[curve, duplicate])
            .err()
            .unwrap()
            .contains("duplicate context quote"));
    }

    #[test]
    fn cell_cleanup_releases_contributions_and_cached_native_state_cannot_resurrect_them() {
        let mut registry = Registry::default();
        let root = root(&mut registry);
        let cell = CellId {
            sheet: 1,
            row: 2,
            column: 1,
        };
        registry.begin_cell(cell);
        let input = add_curve(&mut registry, &root, 0.04);
        registry.finish_cell(true);
        let prepared = prepare(&registry, &root, std::slice::from_ref(&input)).unwrap();
        assert!(!prepared.constructed_elements().discount_curves().is_empty());
        registry.release_cell(cell);
        assert!(prepare(&registry, &root, &[input]).is_err());
    }
}

#[cfg(test)]
mod addition_tests {
    use super::*;
    #[test]
    fn quote_sides_merge_but_conflicting_levels_fail() {
        let mut registry = Registry::default();
        let date = Date::new(2026, 1, 15);
        let root = create(
            &mut registry,
            "root",
            date,
            Currency::USD,
            MarketIndex::SOFR,
            Level::Mid,
        )
        .unwrap();
        let mut input = |name, levels| {
            let mut quotes = QuoteStore::new(date);
            quotes.add_quote(Quote::new(
                "FixedRateDeposit_USD_SOFR_6M".parse().unwrap(),
                levels,
            ));
            add(&mut registry, name, &root, InputData::Quotes(quotes), name).unwrap()
        };
        let mid = input("mid", QuoteLevels::with_mid(0.04));
        let ask = input("ask", QuoteLevels::new(None, None, Some(0.05)));
        let duplicate = input("duplicate", QuoteLevels::with_mid(0.03));
        let context = prepare(&registry, &root, &[mid.clone(), ask]).unwrap();
        let levels = context
            .quote_store()
            .quote("FixedRateDeposit_USD_SOFR_6M")
            .unwrap()
            .levels();
        assert_eq!(levels.mid(), Some(0.04));
        assert_eq!(levels.ask(), Some(0.05));
        assert_eq!(levels.bid(), None);
        assert!(prepare(&registry, &root, &[mid, duplicate])
            .err()
            .unwrap()
            .contains("duplicate context quote"));
    }
    #[test]
    fn fixing_additions_merge_and_duplicate_observations_fail() {
        let mut registry = Registry::default();
        let date = Date::new(2026, 1, 15);
        let root = create(
            &mut registry,
            "root",
            date,
            Currency::USD,
            MarketIndex::SOFR,
            Level::Mid,
        )
        .unwrap();
        let mut input = |name, day, value| {
            let mut fixings = FixingStore::default();
            fixings.add_fixing(&MarketIndex::SOFR, Date::new(2026, 1, day), value);
            add(
                &mut registry,
                name,
                &root,
                InputData::Fixings(fixings),
                name,
            )
            .unwrap()
        };
        let first = input("first", 13, 0.03);
        let second = input("second", 14, 0.04);
        let duplicate = input("duplicate", 13, 0.05);
        let context = prepare(&registry, &root, &[first.clone(), second]).unwrap();
        assert_eq!(context.fixing_store().observations().count(), 2);
        assert_eq!(
            context
                .fixing_store()
                .fixing(&MarketIndex::SOFR, Date::new(2026, 1, 13))
                .unwrap(),
            0.03
        );
        assert!(prepare(&registry, &root, &[first, duplicate])
            .err()
            .unwrap()
            .contains("duplicate context fixing"));
    }
}
