use quantsupport::prelude::{
    BasisSwap, BasisSwapTrade, BlackEuropeanOptionPricer, BondPricer, CdsPricer,
    ClosedFormBlackCapPricer, ClosedFormBlackCapletPricer, DiscountedCashflowPricer, DualFwd,
    EvaluationResults, FixFloatCrossCurrencySwap, FixFloatCrossCurrencySwapTrade,
    FixedIncomeDiscountPolicy, FixedRateBondTrade, FixedRateDeposit, FixedRateDepositTrade,
    FloatFloatCrossCurrencySwap, FloatFloatCrossCurrencySwapTrade, FloatingRateNote,
    FloatingRateNoteTrade, FxEuropeanOptionPricer, FxForwardPricer, Pricer, PricingContext,
    RateFuturesPricer, Request, SingleCurveCSADiscountPolicy, Swap, SwapTrade,
};
use xll_rs::types::{XllError, XLOPER12};
use xllgen::xll_bindgen;

use crate::runtime::registry::{with_registry, QsObject, Registry};

use crate::functions::helpers::{not_found, value_error};

/// Evaluates a stored trade using the selected pricing context and requests.
///
/// # Errors
/// Returns an error for missing handles, unsupported trades or pricing failures.
pub fn evaluate(
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
        QsObject::LoanTrade(loan) => loan.evaluate(context, requests),
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
        QsObject::EquityForwardTrade(trade) => {
            let mut pricer = quantsupport::prelude::EquityForwardPricer::new();
            pricer.set_discount_policy(Box::new(context.csa_policy()));
            pricer
                .evaluate(trade, requests, context)
                .map_err(|error| error.to_string())
        }
        QsObject::EuropeanSwaptionTrade(trade) => {
            use quantsupport::prelude::{InstrumentType, Trade};
            let configuration = context
                .pricer_configuration(InstrumentType::EuropeanSwaption)
                .ok_or_else(|| "EuropeanSwaption requires a pricer configuration".to_string())?;
            let mut pricer =
                quantsupport::prelude::ClosedFormHullWhiteSwaptionPricer::from_configuration(
                    configuration,
                    &trade.instrument().market_index(),
                )
                .map_err(|error| error.to_string())?;
            pricer.set_discount_policy(Box::new(context.csa_policy()));
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

/// Evaluates a stored fixed-rate bond using its pricing anchor.
///
/// # Errors
/// Returns an error for missing handles, a non-bond trade or pricing failures.
pub fn evaluate_bond(
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
    category = "QuantSupport - Pricing",
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
    category = "QuantSupport - Pricing",
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
    category = "QuantSupport - Pricing",
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
    category = "QuantSupport - Pricing",
    help = "Returns OAS for an issuer-call bond; straight fixed-rate bond handles are rejected"
)]
pub fn qs_bond_oas(trade: &str, context: &str) -> Result<f64, XllError> {
    with_registry(|registry| evaluate_bond(registry, trade, context, &[Request::OAS]))
        .map_err(value_error)?
        .oas()
        .ok_or_else(|| not_found("the bond pricer did not return an OAS"))
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
    name = "QS.CASHFLOWS",
    category = "QuantSupport - Pricing",
    help = "Spills the resolved cashflow table for a cashflow-based trade"
)]
pub fn qs_cashflows(
    trade: &str,
    context: &str,
    transpose: *const xll_rs::types::XLOPER12,
    headers: *const xll_rs::types::XLOPER12,
) -> Result<*mut XLOPER12, XllError> {
    let transpose = crate::functions::helpers::optional_bool(transpose, "transpose")?;
    let headers = crate::functions::helpers::optional_bool(headers, "headers")?;
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
    Ok(crate::functions::helpers::spill(
        cells, rows, COLUMNS, transpose, headers,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use quantsupport::prelude::*;
    use std::{cell::RefCell, rc::Rc};

    #[test]
    fn registry_swaptions_use_surface_and_cube_calibration() {
        let date = Date::new(2026, 1, 2);
        for cube in [false, true] {
            let mut quotes = QuoteStore::new(date);
            let mut identifiers = Vec::new();
            for (expiry, volatility) in [("1Y", 0.2), ("2Y", 0.3), ("3Y", 0.4)] {
                for strike in ["0.02", "0.06"] {
                    let identifier = if cube {
                        format!("Swaption_USD_SOFR_{expiry}_5Y_Absolute_{strike}_Black")
                    } else {
                        format!(
                            "CapletFloorlet_USD_SOFR_3M_{expiry}_Absolute_{strike}_Straddle_Black"
                        )
                    };
                    quotes.add_quote(Quote::new(
                        identifier.parse().unwrap(),
                        QuoteLevels::with_mid(volatility),
                    ));
                    identifiers.push(identifier);
                }
            }
            let source = if cube { "Cube" } else { "Surface" };
            let config: PricerConfiguration = serde_json::from_value(serde_json::json!({
                "instrument_type":"EuropeanSwaption", "pricer":"HullWhiteClosedForm",
                "models":[{"market_index":"SOFR","model":{"HullWhite":{"alpha":0.1,
                    "parameter_source":{"Calibrated":{"source":{(source):{"market_index":"SOFR"}},
                    "calibration_basket":{"strike":"Atm"}}}}}}]
            }))
            .unwrap();
            let curve = FlatForwardTermStructure::new(
                date,
                DualFwd::from(0.04),
                RateDefinition::new(
                    DayCounter::Actual365,
                    Compounding::Continuous,
                    Frequency::Annual,
                ),
            )
            .with_pillar_label("SOFR_flat".into());
            let mut elements = ConstructedElementStore::default();
            elements.discount_curves_mut().insert(
                MarketIndex::SOFR,
                DiscountCurveElement::new(MarketIndex::SOFR, Rc::new(RefCell::new(curve))),
            );
            let mut context = PricingContext::new()
                .with_quote_store(quotes)
                .with_constructed_elements(elements)
                .with_pricer_configurations(vec![config]);
            if cube {
                context = context.with_volatility_cube_configurations(vec![
                    VolatilityCubeConfiguration::new(
                        MarketIndex::SOFR,
                        VolatilityType::Black,
                        SmileType::Strike,
                        identifiers.clone(),
                    ),
                ]);
            } else {
                context = context.with_volatility_surface_configurations(vec![
                    VolatilitySurfaceConfiguration::new(
                        MarketIndex::SOFR,
                        VolatilityType::Black,
                        SmileType::Strike,
                        identifiers.clone(),
                    ),
                ]);
            }
            context.initialize().unwrap();
            let mut registry = Registry::default();
            let context_handle = registry
                .upsert_if_changed(
                    "CALIBRATION-CTX",
                    QsObject::PricingContext(Box::new(context)),
                    "context",
                )
                .unwrap();
            let expiry = date + Period::new(30, TimeUnit::Months);
            let option = MakeSwaption::<DualFwd>::default()
                .with_identifier("CALIBRATED".into())
                .with_expiry(expiry)
                .with_swap_tenor_date(expiry + Period::new(5, TimeUnit::Years))
                .with_strike(0.04)
                .with_notional(1.0)
                .with_currency(Currency::USD)
                .with_market_index(MarketIndex::SOFR)
                .with_rate_definition(RateDefinition::default())
                .build()
                .unwrap();
            let trade = EuropeanSwaptionTrade::new(option, date, 1_000_000.0, Side::LongReceive);
            let handle = registry
                .upsert_if_changed(
                    "CALIBRATED-SWPT",
                    QsObject::EuropeanSwaptionTrade(Box::new(trade)),
                    "option",
                )
                .unwrap();
            let results = evaluate(
                &registry,
                &handle,
                &context_handle,
                &[Request::Value, Request::Sensitivities],
            )
            .unwrap();
            assert!(results.price().unwrap() > 0.0);
            let risk = results.sensitivities().unwrap();
            assert!(risk
                .instrument_keys()
                .iter()
                .zip(risk.exposure())
                .any(|(label, value)| identifiers.contains(label) && value.abs() > 1.0));
        }
    }

    #[test]
    fn new_registry_trades_use_native_pricers() {
        let date = Date::new(2026, 1, 2);
        let equity = MarketIndex::Equity("SPX".into());
        let definition = RateDefinition::new(
            DayCounter::Actual365,
            Compounding::Continuous,
            Frequency::Annual,
        );
        let mut elements = ConstructedElementStore::default();
        for index in [equity.clone(), MarketIndex::SOFR] {
            let curve = FlatForwardTermStructure::new(date, DualFwd::from(0.04), definition)
                .with_pillar_label(format!("{index}_flat"));
            elements.discount_curves_mut().insert(
                index.clone(),
                DiscountCurveElement::new(index, Rc::new(RefCell::new(curve))),
            );
        }
        let config:PricerConfiguration=serde_json::from_value(serde_json::json!({
            "instrument_type":"EuropeanSwaption","pricer":"HullWhiteClosedForm",
            "models":[{"market_index":"SOFR","model":{"HullWhite":{"alpha":0.1,"parameter_source":{"Fixed":{"sigma":0.01}}}}}]
        })).unwrap();
        let mut fixings = FixingStore::default();
        fixings.add_fixing(&equity, date, 100.0);
        let context = PricingContext::new()
            .with_quote_store(QuoteStore::new(date))
            .with_constructed_elements(elements)
            .with_fixing_store(fixings)
            .with_pricer_configurations(vec![config]);
        let mut registry = Registry::default();
        let context = registry
            .upsert_if_changed(
                "TEST-CTX",
                QsObject::PricingContext(Box::new(context)),
                "context",
            )
            .unwrap();
        let forward = MakeEquityForward::default()
            .with_identifier("TEST".into())
            .with_market_index(equity)
            .with_delivery_date(Date::new(2027, 1, 2))
            .with_strike(100.0)
            .with_currency(Currency::USD)
            .build()
            .unwrap();
        let trade = EquityForwardTrade::new(forward, date, 10.0, Side::PayShort);
        let handle = registry
            .upsert_if_changed(
                "FWD",
                QsObject::EquityForwardTrade(Box::new(trade)),
                "forward",
            )
            .unwrap();
        let price = evaluate(&registry, &handle, &context, &[Request::Value])
            .unwrap()
            .price()
            .unwrap();
        assert!((price + 1000.0 * (1.0 - (-0.04_f64).exp())).abs() < 1e-10);
        let option = MakeSwaption::<DualFwd>::default()
            .with_identifier("SWPT".into())
            .with_expiry(Date::new(2027, 1, 2))
            .with_swap_tenor_date(Date::new(2032, 1, 2))
            .with_strike(0.04)
            .with_notional(1.0)
            .with_currency(Currency::USD)
            .with_market_index(MarketIndex::SOFR)
            .with_rate_definition(RateDefinition::default())
            .build()
            .unwrap();
        let trade = EuropeanSwaptionTrade::new(option, date, 1000000.0, Side::LongReceive);
        let handle = registry
            .upsert_if_changed(
                "SWPT",
                QsObject::EuropeanSwaptionTrade(Box::new(trade)),
                "option",
            )
            .unwrap();
        assert!(
            evaluate(
                &registry,
                &handle,
                &context,
                &[Request::Value, Request::Sensitivities]
            )
            .unwrap()
            .price()
            .unwrap()
                > 0.0
        );
    }
}
