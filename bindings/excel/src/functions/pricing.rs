use quantsupport::prelude::{
    BasisSwap, BasisSwapTrade, BlackEuropeanOptionPricer, CdsPricer, ClosedFormBlackCapPricer,
    ClosedFormBlackCapletPricer, DiscountedCashflowPricer, DualFwd, EvaluationResults,
    FixFloatCrossCurrencySwap, FixFloatCrossCurrencySwapTrade, FixedRateBond, FixedRateBondTrade,
    FixedRateDeposit, FixedRateDepositTrade, FloatFloatCrossCurrencySwap,
    FloatFloatCrossCurrencySwapTrade, FloatingRateNote, FloatingRateNoteTrade,
    FxEuropeanOptionPricer, FxForwardPricer, Pricer, PricingContext, RateFuturesPricer, Request,
    SingleCurveCSADiscountPolicy, Swap, SwapTrade,
};
use xll_rs::{
    convert::build_multi,
    types::{XllError, XLOPER12},
};
use xllgen::xll_bindgen;

use crate::registry::{with_registry, QsObject, Registry};

use super::helpers::{not_found, value_error};

fn evaluate(
    registry: &Registry,
    trade_handle: &str,
    context_handle: &str,
    requests: &[Request],
) -> Result<EvaluationResults, String> {
    let context = registry
        .pricing_context(context_handle)
        .map_err(|error| error.to_string())?;
    match registry
        .object(trade_handle)
        .map_err(|error| error.to_string())?
    {
        QsObject::FxForwardTrade(trade) => evaluate_fx_forward(trade, context, requests),
        QsObject::SwapTrade(trade) => {
            DiscountedCashflowPricer::<Swap<DualFwd>, SwapTrade<DualFwd>>::new()
                .evaluate(trade, requests, context)
                .map_err(|error| error.to_string())
        }
        QsObject::BasisSwapTrade(trade) => {
            DiscountedCashflowPricer::<BasisSwap<DualFwd>, BasisSwapTrade<DualFwd>>::new()
                .evaluate(trade, requests, context)
                .map_err(|error| error.to_string())
        }
        QsObject::FloatFloatCrossCurrencySwapTrade(trade) => DiscountedCashflowPricer::<
            FloatFloatCrossCurrencySwap<DualFwd>,
            FloatFloatCrossCurrencySwapTrade<DualFwd>,
        >::new()
        .evaluate(trade, requests, context)
        .map_err(|error| error.to_string()),
        QsObject::FixFloatCrossCurrencySwapTrade(trade) => DiscountedCashflowPricer::<
            FixFloatCrossCurrencySwap<DualFwd>,
            FixFloatCrossCurrencySwapTrade<DualFwd>,
        >::new()
        .evaluate(trade, requests, context)
        .map_err(|error| error.to_string()),
        QsObject::FixedRateBondTrade(trade) => {
            DiscountedCashflowPricer::<FixedRateBond<DualFwd>, FixedRateBondTrade<DualFwd>>::new()
                .evaluate(trade, requests, context)
                .map_err(|error| error.to_string())
        }
        QsObject::FloatingRateNoteTrade(trade) => DiscountedCashflowPricer::<
            FloatingRateNote<DualFwd>,
            FloatingRateNoteTrade<DualFwd>,
        >::new()
        .evaluate(trade, requests, context)
        .map_err(|error| error.to_string()),
        QsObject::FxEuropeanOptionTrade(trade) => {
            let mut pricer = FxEuropeanOptionPricer::new();
            pricer.set_discount_policy(Box::new(SingleCurveCSADiscountPolicy::new(
                context.base_index().clone(),
                context.base_currency(),
            )));
            pricer
                .evaluate(trade, requests, context)
                .map_err(|error| error.to_string())
        }
        QsObject::EquityEuropeanOptionTrade(trade) => {
            let mut pricer = BlackEuropeanOptionPricer::new();
            pricer.set_discount_policy(Box::new(SingleCurveCSADiscountPolicy::new(
                context.base_index().clone(),
                context.base_currency(),
            )));
            pricer
                .evaluate(trade, requests, context)
                .map_err(|error| error.to_string())
        }
        QsObject::CdsTrade(trade) => CdsPricer::new()
            .evaluate(trade, requests, context)
            .map_err(|error| error.to_string()),
        QsObject::CapFloorTrade(trade) => {
            let mut pricer = ClosedFormBlackCapPricer::new();
            pricer.set_discount_policy(Box::new(SingleCurveCSADiscountPolicy::new(
                context.base_index().clone(),
                context.base_currency(),
            )));
            pricer
                .evaluate(trade, requests, context)
                .map_err(|error| error.to_string())
        }
        QsObject::CapletFloorletTrade(trade) => {
            let mut pricer = ClosedFormBlackCapletPricer::new();
            pricer.set_discount_policy(Box::new(SingleCurveCSADiscountPolicy::new(
                context.base_index().clone(),
                context.base_currency(),
            )));
            pricer
                .evaluate(trade, requests, context)
                .map_err(|error| error.to_string())
        }
        QsObject::FixedRateDepositTrade(trade) => DiscountedCashflowPricer::<
            FixedRateDeposit<DualFwd>,
            FixedRateDepositTrade<DualFwd>,
        >::new()
        .evaluate(trade, requests, context)
        .map_err(|error| error.to_string()),
        QsObject::RateFuturesTrade(trade) => RateFuturesPricer::new()
            .evaluate(trade, requests, context)
            .map_err(|error| error.to_string()),
        object => Err(format!(
            "object type '{}' is not a supported trade",
            object.kind()
        )),
    }
}

fn evaluate_fx_forward(
    trade: &quantsupport::prelude::FxForwardTrade,
    context: &PricingContext,
    requests: &[Request],
) -> Result<EvaluationResults, String> {
    let mut pricer = FxForwardPricer::new();
    pricer.set_discount_policy(Box::new(SingleCurveCSADiscountPolicy::new(
        context.base_index().clone(),
        context.base_currency(),
    )));
    pricer
        .evaluate(trade, requests, context)
        .map_err(|error| error.to_string())
}

#[xll_bindgen(
    name = "QS.PRICE",
    category = "QuantSupport - Pricing",
    help = "Prices a stored trade with an initialized PricingContext"
)]
pub fn qs_price(trade: &str, context: &str) -> Result<f64, XllError> {
    with_registry(|registry| evaluate(registry, trade, context, &[Request::Value]))
        .map_err(value_error)?
        .price()
        .ok_or_else(|| not_found("the selected pricer did not return a price"))
}

#[xll_bindgen(
    name = "QS.FAIR.RATE",
    category = "QuantSupport - Pricing",
    help = "Returns the fair rate, par coupon, or fair forward from a supported pricer"
)]
pub fn qs_fair_rate(trade: &str, context: &str) -> Result<f64, XllError> {
    with_registry(|registry| evaluate(registry, trade, context, &[Request::FairRate]))
        .map_err(value_error)?
        .fair_rate()
        .ok_or_else(|| not_found("the selected pricer did not return a fair rate"))
}

#[xll_bindgen(
    name = "QS.SENSITIVITIES",
    category = "QuantSupport - Pricing",
    help = "Spills sensitivity keys and values for a supported trade/pricer"
)]
pub fn qs_sensitivities(trade: &str, context: &str) -> Result<*mut XLOPER12, XllError> {
    let results = with_registry(|registry| {
        evaluate(
            registry,
            trade,
            context,
            &[Request::Value, Request::Sensitivities],
        )
    })
    .map_err(value_error)?;
    let sensitivities = results
        .sensitivities()
        .ok_or_else(|| not_found("the selected pricer did not return sensitivities"))?;

    let rows = sensitivities.instrument_keys().len() + 1;
    let mut cells = Vec::with_capacity(rows * 2);
    cells.push(XLOPER12::from_str("Risk factor"));
    cells.push(XLOPER12::from_str("Sensitivity"));
    for (key, exposure) in sensitivities
        .instrument_keys()
        .iter()
        .zip(sensitivities.exposure())
    {
        cells.push(XLOPER12::from_str(key));
        cells.push(XLOPER12::from_f64(*exposure));
    }
    Ok(build_multi(cells, rows, 2))
}

#[xll_bindgen(
    name = "QS.CASHFLOWS",
    category = "QuantSupport - Pricing",
    help = "Spills the resolved cashflow table for a cashflow-based trade"
)]
pub fn qs_cashflows(trade: &str, context: &str) -> Result<*mut XLOPER12, XllError> {
    let results =
        with_registry(|registry| evaluate(registry, trade, context, &[Request::Cashflows]))
            .map_err(value_error)?;
    let table = results
        .cashflows()
        .ok_or_else(|| not_found("the selected pricer did not return cashflows"))?;

    const COLUMNS: usize = 9;
    let rows = table.payment_dates().len() + 1;
    let mut cells = Vec::with_capacity(rows * COLUMNS);
    for header in [
        "Payment date",
        "Type",
        "Amount",
        "Fixing",
        "Accrual",
        "Currency",
        "Floor",
        "Cap",
        "Leg",
    ] {
        cells.push(XLOPER12::from_str(header));
    }
    for row in 0..table.payment_dates().len() {
        cells.push(XLOPER12::from_str(&table.payment_dates()[row].to_string()));
        cells.push(XLOPER12::from_str(&table.cashflow_types()[row]));
        cells.push(XLOPER12::from_f64(table.amounts()[row]));
        cells.push(table.fixing()[row].map_or_else(XLOPER12::nil, XLOPER12::from_f64));
        cells.push(XLOPER12::from_f64(table.accrual_periods()[row]));
        cells.push(XLOPER12::from_str(table.currencies()[row].as_str()));
        cells.push(table.floorlet_strikes()[row].map_or_else(XLOPER12::nil, XLOPER12::from_f64));
        cells.push(table.caplet_strikes()[row].map_or_else(XLOPER12::nil, XLOPER12::from_f64));
        cells.push(XLOPER12::from_int(
            i32::try_from(table.leg_indices()[row]).unwrap_or(i32::MAX),
        ));
    }
    Ok(build_multi(cells, rows, COLUMNS))
}
