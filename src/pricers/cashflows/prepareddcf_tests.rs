use super::{DcfCapture, PreparedDcfValuation};
use crate::{
    ad::recording::{Recording, RecordingScope},
    prelude::*,
};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::Rc,
};

type SwapPricer = DiscountedCashflowPricer<Swap<Recorded>, SwapTrade<Recorded>>;
const REQUESTS: [Request; 2] = [Request::Value, Request::Sensitivities];

fn reference() -> Date {
    Date::new(2025, 1, 2)
}
fn years(n: i32) -> Period {
    Period::new(n, TimeUnit::Years)
}
fn market(date: Date, rate: f64, label: &str) -> PricingContext {
    let mut elements = ConstructedElementStore::default();
    for (index, value, name) in [
        (MarketIndex::SOFR, rate, label),
        (MarketIndex::ESTR, 0.02, "EUR.rate"),
        (
            MarketIndex::Collateral(Currency::EUR, Currency::USD),
            0.025,
            label,
        ),
    ] {
        let curve = FlatForwardTermStructure::new(
            reference(),
            Recorded::from(value),
            RateDefinition::new(
                DayCounter::Actual365,
                Compounding::Continuous,
                Frequency::Annual,
            ),
        )
        .with_pillar_label(name.into());
        elements.discount_curves_mut().insert(
            index.clone(),
            DiscountCurveElement::new(index, Rc::new(RefCell::new(curve))),
        );
    }
    let mut fx = FxStore::new();
    fx.add_fx_rate(Currency::EUR, Currency::USD, Recorded::from(1.1));
    PricingContext::new()
        .with_quote_store(QuoteStore::new(date))
        .with_constructed_elements(elements)
        .with_fx_store(fx)
        .with_base_currency(Currency::USD)
        .with_base_index(MarketIndex::SOFR)
}
fn swap(
    notional: f64,
    end: i32,
    index: MarketIndex,
    currency: Currency,
) -> Result<SwapTrade<Recorded>> {
    let instrument = MakeSwap::<Recorded>::default()
        .with_identifier("SWAP".into())
        .with_start_date(reference() + years(1))
        .with_maturity_date(reference() + years(end))
        .with_notional(notional)
        .with_currency(currency)
        .with_market_index(index)
        .with_fixed_rate(0.04)
        .with_rate_definition(RateDefinition::new(
            DayCounter::Actual360,
            Compounding::Simple,
            Frequency::Quarterly,
        ))
        .build()?;
    Ok(SwapTrade::new(
        instrument,
        reference(),
        notional,
        Side::LongReceive,
    ))
}
fn usd_swap(notional: f64, end: i32) -> Result<SwapTrade<Recorded>> {
    swap(notional, end, MarketIndex::SOFR, Currency::USD)
}
fn near(a: f64, b: f64) {
    assert!(
        (a - b).abs() <= 1e-11_f64.mul_add(a.abs().max(b.abs()), 1e-8),
        "{a} != {b}"
    );
}
fn same(a: &EvaluationResults, b: &EvaluationResults) {
    assert_eq!(a.price().is_some(), b.price().is_some());
    if let (Some(x), Some(y)) = (a.price(), b.price()) {
        near(x, y);
    }
    assert_eq!(a.sensitivities().is_some(), b.sensitivities().is_some());
    if let (Some(x), Some(y)) = (a.sensitivities(), b.sensitivities()) {
        let map = |s: &SensitivityMap| {
            s.instrument_keys()
                .iter()
                .cloned()
                .zip(s.exposure().iter().copied())
                .collect::<BTreeMap<_, _>>()
        };
        let x = map(x);
        let y = map(y);
        assert_eq!(x.keys().collect::<Vec<_>>(), y.keys().collect::<Vec<_>>());
        for (key, value) in x {
            near(value, y[&key]);
        }
    }
}

#[test]
fn prepared_requests_match_reference_without_accumulating() -> Result<()> {
    let context = market(reference(), 0.03, "USD.rate");
    let trade = usd_swap(1e6, 4)?;
    let pricer = SwapPricer::new();
    let mut prepared = pricer.prepare(&trade, &context)?;
    for requests in [
        vec![],
        vec![Request::Value],
        vec![Request::Sensitivities],
        REQUESTS.to_vec(),
        vec![
            Request::Sensitivities,
            Request::Value,
            Request::Sensitivities,
        ],
    ] {
        same(
            &prepared.evaluate(&requests)?,
            &pricer.evaluate(&trade, &requests, &context)?,
        );
    }
    assert!(prepared.evaluate(&[Request::Cashflows(false)]).is_err());
    same(
        &prepared.evaluate(&REQUESTS)?,
        &pricer.evaluate(&trade, &REQUESTS, &context)?,
    );
    Ok(())
}

#[test]
fn refresh_updates_numeric_inputs_and_labels_reuses_code() -> Result<()> {
    let pricer = SwapPricer::new();
    let trade = usd_swap(1e6, 4)?;
    let mut prepared = pricer.prepare(&trade, &market(reference(), 0.03, "USD.rate"))?;
    let old = prepared.evaluate(&REQUESTS)?;
    let old_code = prepared.clone();
    let updated = market(reference(), 0.05, "USD.new");
    same(&old, &prepared.evaluate(&REQUESTS)?);
    assert!(pricer.refresh(&mut prepared, &trade, &updated)?);
    assert!(prepared.shares_code_with(&old_code));
    let new = prepared.evaluate(&REQUESTS)?;
    assert_ne!(old.price(), new.price());
    same(&new, &pricer.evaluate(&trade, &REQUESTS, &updated)?);
    let changed = usd_swap(2e6, 4)?;
    assert!(pricer.refresh(&mut prepared, &changed, &updated)?);
    same(
        &prepared.evaluate(&REQUESTS)?,
        &pricer.evaluate(&changed, &REQUESTS, &updated)?,
    );
    let schedule = usd_swap(2e6, 5)?;
    assert!(!pricer.refresh(&mut prepared, &schedule, &updated)?);
    same(
        &prepared.evaluate(&REQUESTS)?,
        &pricer.evaluate(&schedule, &REQUESTS, &updated)?,
    );
    Ok(())
}

#[test]
fn failed_refresh_invalidates_and_successful_refresh_recovers() -> Result<()> {
    let pricer = SwapPricer::new();
    let trade = usd_swap(1e6, 4)?;
    let context = market(reference(), 0.03, "USD.rate");
    let mut prepared = pricer.prepare(&trade, &context)?;
    assert!(pricer
        .refresh(&mut prepared, &trade, &PricingContext::new())
        .is_err());
    assert!(prepared.evaluate(&REQUESTS).is_err());
    pricer.refresh(&mut prepared, &trade, &context)?;
    same(
        &prepared.evaluate(&REQUESTS)?,
        &pricer.evaluate(&trade, &REQUESTS, &context)?,
    );
    Ok(())
}

#[test]
fn prepare_and_warm_execution_preserve_parent_capture_and_coupon_cells() -> Result<()> {
    let context = market(reference(), 0.03, "USD.rate");
    let trade = usd_swap(1e6, 4)?;
    let pricer = SwapPricer::new();
    let fixings = || {
        trade
            .legs()
            .iter()
            .flat_map(Leg::cashflows)
            .filter_map(|cf| {
                if let CashflowType::FloatingRateCoupon(c) = cf {
                    Some(c.fixing())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    };
    let before_fixings = fixings();
    let _scope = RecordingScope::new();
    Recording::start();
    let input = Recorded::new(2.0);
    let before = Recording::state();
    let mut prepared = pricer.prepare(&trade, &context)?;
    assert_eq!(before_fixings, fixings());
    assert_eq!(before, Recording::state());
    prepared.evaluate(&REQUESTS)?;
    assert_eq!(before, Recording::state());
    pricer.evaluate(&trade, &[Request::Value], &context)?;
    assert_eq!(before, Recording::state());
    (input * input).differentiate()?;
    near(input.derivative()?.value(), 4.0);
    Ok(())
}

struct CountingMarket {
    context: PricingContext,
    calls: Cell<usize>,
}
impl MarketDataProvider for CountingMarket {
    fn handle_request(&self, request: &MarketDataRequest) -> Result<MarketData> {
        self.calls.set(self.calls.get() + 1);
        self.context.handle_request(request)
    }
    fn evaluation_date(&self) -> Date {
        self.context.evaluation_date()
    }
}
#[test]
fn warm_execution_and_thread_clones_need_no_provider_or_capture() -> Result<()> {
    let market = CountingMarket {
        context: market(reference(), 0.03, "USD.rate"),
        calls: Cell::new(0),
    };
    let pricer = SwapPricer::new();
    let trade = usd_swap(1e6, 4)?;
    let mut prepared = pricer.prepare(&trade, &market)?;
    let expected = prepared.evaluate(&REQUESTS)?;
    for _ in 0..10 {
        same(&prepared.evaluate(&REQUESTS)?, &expected);
    }
    assert_eq!(market.calls.get(), 1);
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let mut plan = prepared.clone();
                scope.spawn(move || plan.evaluate(&REQUESTS))
            })
            .collect();
        for handle in handles {
            let result = handle
                .join()
                .unwrap_or_else(|_| panic!("prepared worker panicked"))?;
            same(&result, &expected);
        }
        Ok(())
    })
}

#[test]
fn portfolio_matches_reference_keeps_multiplicity_and_invalidates_on_failure() -> Result<()> {
    let portfolio = Portfolio::new(
        "BOOK".into(),
        vec![usd_swap(1e6, 4)?, usd_swap(-1e6, 4)?, usd_swap(2e6, 5)?],
    )?;
    let market = CountingMarket {
        context: market(reference(), 0.03, "USD.rate"),
        calls: Cell::new(0),
    };
    let pricer = SwapPricer::new();
    let mut book = pricer.prepare_portfolio(&portfolio, &market)?;
    assert!(book.members()[0].shares_code_with(&book.members()[1]));
    let reference_pricer = PortfolioPricer::new(SwapPricer::new());
    let expected = reference_pricer.evaluate(&portfolio, &REQUESTS, &market.context)?;
    for _ in 0..3 {
        same(&book.evaluate(&REQUESTS)?, &expected);
    }
    assert_eq!(market.calls.get(), 3);
    assert!(pricer
        .refresh_portfolio(&mut book, &portfolio, &PricingContext::new())
        .is_err());
    assert!(book.evaluate(&REQUESTS).is_err());
    // Refresh must retain the previous member code even after the thread cache is cleared.
    super::CODE.with(|cache| cache.borrow_mut().clear());
    assert!(pricer.refresh_portfolio(&mut book, &portfolio, &market.context)?);
    same(&book.evaluate(&REQUESTS)?, &expected);
    Ok(())
}

#[test]
fn policy_and_fx_refresh_match_reference() -> Result<()> {
    let context = market(reference(), 0.03, "USD.rate");
    let trade = swap(1e6, 4, MarketIndex::ESTR, Currency::EUR)?;
    let mut pricer = SwapPricer::new();
    let mut prepared = pricer.prepare(&trade, &context)?;
    same(
        &prepared.evaluate(&REQUESTS)?,
        &pricer.evaluate(&trade, &REQUESTS, &context)?,
    );
    pricer.set_discount_policy(Box::new(SingleCurveCSADiscountPolicy::new(
        MarketIndex::SOFR,
        Currency::USD,
    )));
    pricer.refresh(&mut prepared, &trade, &context)?;
    same(
        &prepared.evaluate(&REQUESTS)?,
        &pricer.evaluate(&trade, &REQUESTS, &context)?,
    );
    let mut fx = FxStore::new();
    fx.add_fx_rate(Currency::USD, Currency::EUR, Recorded::from(0.8));
    let context = context.with_fx_store(fx);
    pricer.refresh(&mut prepared, &trade, &context)?;
    same(
        &prepared.evaluate(&REQUESTS)?,
        &pricer.evaluate(&trade, &REQUESTS, &context)?,
    );
    Ok(())
}

#[test]
fn dates_historical_fixings_and_projection_policy_refresh_match_reference() -> Result<()> {
    let trade = usd_swap(1e6, 4)?;
    let pricer = SwapPricer::new();
    let mut prepared = pricer.prepare(&trade, &market(reference(), 0.03, "USD.rate"))?;
    let rolled = reference() + years(2);
    let missing = market(rolled, 0.04, "USD.rate");
    assert!(pricer.refresh(&mut prepared, &trade, &missing).is_err());
    let mut fixings = FixingStore::default();
    for leg in trade.legs() {
        for cf in leg.cashflows() {
            if let CashflowType::FloatingRateCoupon(coupon) = cf {
                fixings.add_fixing(&MarketIndex::SOFR, coupon.fixing_date(), 0.06);
            }
        }
    }
    let anchored = missing.clone().with_fixing_store(fixings);
    pricer.refresh(&mut prepared, &trade, &anchored)?;
    same(
        &prepared.evaluate(&REQUESTS)?,
        &pricer.evaluate(&trade, &REQUESTS, &anchored)?,
    );
    let projected = missing.with_anchor_fixings(false);
    pricer.refresh(&mut prepared, &trade, &projected)?;
    same(
        &prepared.evaluate(&REQUESTS)?,
        &pricer.evaluate(&trade, &REQUESTS, &projected)?,
    );
    let expired = market(reference() + years(6), 0.04, "USD.rate").with_anchor_fixings(false);
    pricer.refresh(&mut prepared, &trade, &expired)?;
    let result = prepared.evaluate(&REQUESTS)?;
    near(result.price().unwrap_or(f64::NAN), 0.0);
    same(&result, &pricer.evaluate(&trade, &REQUESTS, &expired)?);
    Ok(())
}

fn calibrated(rate: f64) -> Result<PricingContext> {
    let ids = vec![
        "FixedRateDeposit_USD_SOFR_1Y".to_owned(),
        "FixedRateDeposit_USD_SOFR_5Y".to_owned(),
    ];
    let mut quotes = QuoteStore::new(reference());
    for (id, quote_rate) in ids.iter().zip([rate, rate + 0.005]) {
        quotes.add_quote(Quote::new(id.parse()?, QuoteLevels::with_mid(quote_rate)));
    }
    let config = CurveConfiguration::new(
        MarketIndex::SOFR,
        DayCounter::Actual365,
        Interpolator::LogLinear,
        true,
        ids,
    );
    let mut context = PricingContext::new()
        .with_quote_store(quotes)
        .with_curve_configurations(vec![config]);
    context.initialize()?;
    Ok(context)
}
#[test]
fn calibrated_quote_risk_is_rebound_and_matches_bumps() -> Result<()> {
    let pricer = SwapPricer::new();
    let trade = usd_swap(1e6, 4)?;
    let mut prepared = pricer.prepare(&trade, &calibrated(0.03)?)?;
    let changed = calibrated(0.05)?;
    pricer.refresh(&mut prepared, &trade, &changed)?;
    let result = prepared.evaluate(&REQUESTS)?;
    same(&result, &pricer.evaluate(&trade, &REQUESTS, &changed)?);
    let sensitivities = result
        .sensitivities()
        .ok_or_else(|| QSError::ValueNotSetErr("risk".into()))?;
    assert_eq!(sensitivities.exposure().len(), 2);
    let bump = 1e-6;
    let up = pricer
        .evaluate(&trade, &[Request::Value], &calibrated(0.05 + bump)?)?
        .price()
        .unwrap_or(f64::NAN);
    let down = pricer
        .evaluate(&trade, &[Request::Value], &calibrated(0.05 - bump)?)?
        .price()
        .unwrap_or(f64::NAN);
    let delta = sensitivities.exposure().iter().sum::<f64>();
    assert!((delta - (up - down) / (2.0 * bump)).abs() < 0.01);
    Ok(())
}

#[test]
fn excessive_prepared_graph_fails_before_native_compilation() -> Result<()> {
    let pricer = SwapPricer::new();
    let trade = usd_swap(1e6, 100)?;
    let Err(error) = pricer.prepare(&trade, &market(reference(), 0.03, "USD.rate")) else {
        panic!("oversized prototype graph should be rejected");
    };
    assert!(error.to_string().contains("2048"));
    Ok(())
}

#[test]
fn colliding_risk_labels_match_reference_aggregation() -> Result<()> {
    let context = market(reference(), 0.03, "EUR.rate");
    let trade = swap(1e6, 4, MarketIndex::ESTR, Currency::EUR)?;
    let mut pricer = SwapPricer::new();
    pricer.set_discount_policy(Box::new(SingleCurveCSADiscountPolicy::new(
        MarketIndex::SOFR,
        Currency::USD,
    )));
    let mut prepared = pricer.prepare(&trade, &context)?;
    assert!(prepared.positions.iter().any(|inputs| inputs.len() > 1));
    same(
        &prepared.evaluate(&REQUESTS)?,
        &pricer.evaluate(&trade, &REQUESTS, &context)?,
    );
    Ok(())
}

#[test]
fn overflowing_grouped_risk_is_rejected() -> Result<()> {
    let graph = crate::ad::graph::Graph::trace(&[0.0, 0.0], |x| x[0] * 1e308 + x[1] * 1e308)
        .map_err(|error| QSError::EvaluationErr(error.to_string()))?;
    let mut prepared = PreparedDcfValuation::from_capture(
        DcfCapture {
            date: reference(),
            identifier: "overflow".into(),
            graph,
            inputs: vec![0.0, 0.0],
            active: 2,
            risks: vec![("same".into(), 0), ("same".into(), 1)],
        },
        None,
    )?;
    assert!(prepared.evaluate(&REQUESTS).is_err());
    assert!(prepared.evaluate(&[Request::Value]).is_ok());
    Ok(())
}
