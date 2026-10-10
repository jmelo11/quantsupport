use quantsupport::prelude::{
    BasisSwap, BasisSwapTrade, BlackEuropeanOptionPricer, BondPricer, CdsPricer,
    ClosedFormBlackCapPricer, ClosedFormBlackCapletPricer, DiscountedCashflowPricer, DualFwd,
    EvaluationResults, FixFloatCrossCurrencySwap, FixFloatCrossCurrencySwapTrade,
    FixedIncomeDiscountPolicy, FixedRateBondTrade, FixedRateDeposit, FixedRateDepositTrade,
    FloatFloatCrossCurrencySwap, FloatFloatCrossCurrencySwapTrade, FloatingRateNote,
    FloatingRateNoteTrade, FxEuropeanOptionPricer, FxForwardPricer, Pricer, PricingContext,
    RateFuturesPricer, Request, SingleCurveCSADiscountPolicy, Swap, SwapTrade,
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
        QsObject::FixedRateBondTrade(trade) => evaluate_fixed_rate_bond(trade, context, requests),
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

fn evaluate_fixed_rate_bond(
    trade: &FixedRateBondTrade<DualFwd>,
    context: &PricingContext,
    requests: &[Request],
) -> Result<EvaluationResults, String> {
    let mut pricer = BondPricer::new();
    pricer.set_discount_policy(Box::new(
        FixedIncomeDiscountPolicy::new(true)
            .with_risk_free_index(context.base_currency(), context.base_index().clone()),
    ));
    pricer
        .evaluate(trade, requests, context)
        .map_err(|error| error.to_string())
}

fn evaluate_bond(
    registry: &Registry,
    trade_handle: &str,
    context_handle: &str,
    requests: &[Request],
) -> Result<EvaluationResults, String> {
    let context = registry
        .pricing_context(context_handle)
        .map_err(|error| error.to_string())?;
    let trade = match registry
        .object(trade_handle)
        .map_err(|error| error.to_string())?
    {
        QsObject::FixedRateBondTrade(trade) => trade,
        object => {
            return Err(format!(
                "object type '{}' is not a fixed-rate bond trade",
                object.kind()
            ))
        }
    };
    evaluate_fixed_rate_bond(trade, context, requests)
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
    name = "QS.BOND.PRICE",
    category = "QuantSupport - Fixed Income",
    help = "Prices a straight fixed-rate bond using the trade's pricing anchor"
)]
pub fn qs_bond_price(trade: &str, context: &str) -> Result<f64, XllError> {
    with_registry(|registry| evaluate_bond(registry, trade, context, &[Request::Value]))
        .map_err(value_error)?
        .price()
        .ok_or_else(|| not_found("the bond pricer did not return a price"))
}

#[xll_bindgen(
    name = "QS.BOND.MODIFIED.DURATION",
    category = "QuantSupport - Fixed Income",
    help = "Returns straight-bond yield-modified duration using the trade's pricing anchor"
)]
pub fn qs_bond_modified_duration(trade: &str, context: &str) -> Result<f64, XllError> {
    with_registry(|registry| evaluate_bond(registry, trade, context, &[Request::ModifiedDuration]))
        .map_err(value_error)?
        .modified_duration()
        .ok_or_else(|| not_found("the bond pricer did not return modified duration"))
}

#[xll_bindgen(
    name = "QS.BOND.Z.SPREAD",
    category = "QuantSupport - Fixed Income",
    help = "Returns the Z-spread of a straight bond using the trade's pricing anchor"
)]
pub fn qs_bond_z_spread(trade: &str, context: &str) -> Result<f64, XllError> {
    with_registry(|registry| evaluate_bond(registry, trade, context, &[Request::ZSpread]))
        .map_err(value_error)?
        .z_spread()
        .ok_or_else(|| not_found("the bond pricer did not return a Z-spread"))
}

#[xll_bindgen(
    name = "QS.BOND.OAS",
    category = "QuantSupport - Fixed Income",
    help = "Returns OAS for an issuer-call bond; straight fixed-rate bond handles are rejected"
)]
pub fn qs_bond_oas(trade: &str, context: &str) -> Result<f64, XllError> {
    with_registry(|registry| evaluate_bond(registry, trade, context, &[Request::OAS]))
        .map_err(value_error)?
        .oas()
        .ok_or_else(|| not_found("the bond pricer did not return an OAS"))
}

#[xll_bindgen(
    name = "QS.BOND.SENSITIVITIES",
    category = "QuantSupport - Fixed Income",
    help = "Spills curve and dPV/dZSpread risk in one table for a straight fixed-rate bond"
)]
pub fn qs_bond_sensitivities(trade: &str, context: &str) -> Result<*mut XLOPER12, XllError> {
    let results = with_registry(|registry| {
        evaluate_bond(
            registry,
            trade,
            context,
            &[Request::Value, Request::Sensitivities],
        )
    })
    .map_err(value_error)?;
    let sensitivities = results
        .sensitivities()
        .ok_or_else(|| not_found("the bond pricer did not return sensitivities"))?;
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
