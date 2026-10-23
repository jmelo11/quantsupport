//! Explicit ribbon queries return owned tables and never create registry objects.

use super::{
    browser::BrowserDetail,
    pricing,
    products::PositionTerms,
    registry::{QsObject, Registry},
};
use quantsupport::{core::elements::curveelement::ADCurveElement, prelude::*};
use serde::{Deserialize, Serialize};
use std::result::Result;

#[derive(Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct BrowserQuery {
    pub workbook: String,
    pub handle: String,
    pub view: String,
    pub curve: String,
    pub context: String,
    pub pricer: String,
    pub request: String,
    pub target: u32,
    pub trade_date: String,
    pub side: String,
    pub pricing_anchor: String,
    pub anchor_value: Option<f64>,
}

#[derive(Default, Serialize)]
pub struct BrowserResponse {
    pub title: String,
    pub message: String,
    pub curves: Vec<String>,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub numeric_columns: Vec<usize>,
    /// Parameter rows correspond to table rows (e.g. a selected calibration quote).
    pub parameters: Vec<Vec<BrowserDetail>>,
    pub error: String,
}

impl BrowserResponse {
    pub fn error(error: impl ToString) -> Self {
        Self {
            error: error.to_string(),
            ..Self::default()
        }
    }

    fn table(title: impl ToString, columns: &[&str]) -> Self {
        Self {
            title: title.to_string(),
            columns: columns.iter().map(|value| (*value).into()).collect(),
            ..Self::default()
        }
    }
}

pub fn curve_names(object: &QsObject) -> Vec<String> {
    let mut names: Vec<String> = match object {
        QsObject::DiscountCurve(curve) => vec![curve.market_index().to_string()],
        #[cfg(test)]
        QsObject::ConstructedElements(store) => store
            .discount_curves()
            .keys()
            .map(ToString::to_string)
            .collect(),
        #[cfg(test)]
        QsObject::CurveConfigurations(configs) => configs
            .iter()
            .map(|config| config.market_index().to_string())
            .collect(),
        QsObject::PricingContext(context) => context
            .constructed_elements()
            .discount_curves()
            .keys()
            .map(ToString::to_string)
            .chain(
                context
                    .curve_configurations()
                    .iter()
                    .map(|config| config.market_index().to_string()),
            )
            .collect(),
        _ => Vec::new(),
    };
    names.sort();
    names.dedup();
    names
}

pub fn execute(registry: &Registry, request: &BrowserQuery) -> Result<BrowserResponse, String> {
    let object = registry
        .browser_object(&request.handle)
        .map_err(|error| error.to_string())?;
    if request.view == "evaluate" {
        let context = match registry
            .browser_object(&request.context)
            .map_err(|error| error.to_string())?
        {
            QsObject::PricingContext(context) => context,
            _ => return Err("Select a pricing context.".into()),
        };
        let pricer = match registry
            .browser_object(&request.pricer)
            .map_err(|error| error.to_string())?
        {
            QsObject::Pricer(pricer) => pricer,
            _ => return Err("Select an explicit pricer.".into()),
        };
        let name = match request.request.as_str() {
            "Value" | "Cashflows" | "Sensitivities" => request.request.clone(),
            "FairRate" => format!("FairRate({})", request.target),
            _ => return Err("Choose PV, Cashflows, Fair rate or Sensitivities.".into()),
        };
        let requests = pricing::parse_requests(&[name])?;
        // Instrument previews use explicit position inputs and stay outside the
        // worksheet registry. Stored trades always retain their original terms.
        let preview = if let QsObject::Instrument(instrument) = object {
            let date = Date::from_str(request.trade_date.trim(), "%Y-%m-%d")
                .map_err(|_| "Choose a trade date for the instrument preview.".to_string())?;
            let side = match request.side.as_str() {
                "Receive" => Side::LongReceive,
                "Pay" => Side::PayShort,
                _ => return Err("Choose Receive or Pay for the instrument preview.".into()),
            };
            let anchor = match (request.pricing_anchor.as_str(), request.anchor_value) {
                ("", None) => None,
                ("CleanPrice", Some(value)) => Some(BondPricingAnchor::CleanPrice(value)),
                ("DirtyPrice", Some(value)) => Some(BondPricingAnchor::DirtyPrice(value)),
                ("ZSpread", Some(value)) => Some(BondPricingAnchor::ZSpread(value)),
                _ => return Err(
                    "Supply a bond anchor and value together: CleanPrice, DirtyPrice or ZSpread."
                        .into(),
                ),
            };
            Some(instrument.trade(
                date,
                side,
                &PositionTerms {
                    pricing_anchor: anchor,
                    ..Default::default()
                },
            )?)
        } else {
            if !request.trade_date.is_empty()
                || !request.side.is_empty()
                || !request.pricing_anchor.is_empty()
                || request.anchor_value.is_some()
            {
                return Err("Stored trades use their own position terms; select the instrument to preview different terms.".into());
            }
            None
        };
        let results = pricer.evaluate(preview.as_ref().unwrap_or(object), &requests, context)?;
        return evaluation_table(&results, &request.request);
    }

    let names = curve_names(object);
    if request.view == "curves" {
        return Ok(BrowserResponse {
            curves: names,
            ..Default::default()
        });
    }
    if !names.contains(&request.curve) {
        return Err("Select an available curve and refresh if its inputs have changed.".into());
    }
    let index = super::inputs::market_index(&request.curve)?;
    let context = match object {
        QsObject::PricingContext(context) => Some(context.as_ref()),
        _ if !request.context.is_empty() => match registry
            .browser_object(&request.context)
            .map_err(|error| error.to_string())?
        {
            QsObject::PricingContext(context) => Some(context.as_ref()),
            _ => return Err("Select a pricing context.".into()),
        },
        _ => None,
    };
    match request.view.as_str() {
        "nodes" => {
            let element = match object {
                QsObject::DiscountCurve(curve) => Some(curve.as_ref()),
                #[cfg(test)]
                QsObject::ConstructedElements(store) => store.discount_curves().get(&index),
                _ => context.and_then(|context| {
                    context.constructed_elements().discount_curves().get(&index)
                }),
            }
            .ok_or(
                "This curve has no constructed nodes here. Select an initialized pricing context.",
            )?;
            curve_table(&*element.curve(), &request.curve)
        }
        "instruments" => {
            let configs = match object {
                #[cfg(test)]
                QsObject::CurveConfigurations(configs) => Some(configs.as_slice()),
                _ => context.map(|context| context.curve_configurations().as_slice()),
            };
            let config = configs.and_then(|configs| {
                configs
                    .iter()
                    .find(|config| config.market_index() == &index)
            });
            if let Some(config) = config {
                configuration_table(config, context)
            } else {
                Ok(BrowserResponse {
                    title: format!("{} — curve inputs", request.curve),
                    message: "No calibration configuration is available here. For a supplied curve, inspect its defining cell; for a context-derived curve, select its pricing context.".into(),
                    ..Default::default()
                })
            }
        }
        _ => Err("Unknown browser view.".into()),
    }
}

fn curve_table(curve: &dyn ADCurveElement, name: &str) -> Result<BrowserResponse, String> {
    let reference = curve.reference_date();
    let mut table = BrowserResponse::table(
        format!("{name} — curve table"),
        &[
            "Pillar",
            "Date",
            "Discount factor",
            "Zero rate",
            "Forward from prior node",
        ],
    );
    table.numeric_columns = vec![2, 3, 4];
    let nodes = match curve.nodes() {
        Some(nodes) if !nodes.is_empty() => nodes,
        _ => {
            // Flat curves have no dated nodes. Show explicitly labelled annual samples.
            table.message = "Annual samples (this curve exposes no dated nodes). Rates are decimal, Simple/Annual, using the curve's day count.".into();
            [0, 1, 2, 3, 5, 10, 20, 30]
                .into_iter()
                .map(|years| {
                    let date = reference + Period::new(years, TimeUnit::Years);
                    curve
                        .discount_factor(date)
                        .map(|df| (date, df))
                        .map_err(|error| error.to_string())
                })
                .collect::<Result<Vec<_>, _>>()?
        }
    };
    if table.message.is_empty() {
        table.message = "Rates are decimal, Simple/Annual, using the curve's day count.".into();
    }
    let labels = curve.pillar_labels().unwrap_or_default();
    let mut pillar = 0;
    for (row, (date, df)) in nodes.iter().enumerate() {
        let label = if *date == reference {
            "Reference".into()
        } else {
            let label = labels.get(pillar).cloned().unwrap_or_default();
            pillar += 1;
            if table.message.starts_with("Annual samples") {
                "Sample".into()
            } else {
                label
            }
        };
        let rate = |start| {
            curve
                .forward_rate(start, *date, Compounding::Simple, Frequency::Annual)
                .map(|value| value.value().to_string())
                .map_err(|error| error.to_string())
        };
        table.rows.push(vec![
            label,
            date.to_string(),
            df.value().to_string(),
            if *date == reference {
                String::new()
            } else {
                rate(reference)?
            },
            if row == 0 {
                String::new()
            } else {
                rate(nodes[row - 1].0)?
            },
        ]);
    }
    Ok(table)
}

fn fields(value: &impl Serialize) -> Result<Vec<BrowserDetail>, String> {
    fn flatten(prefix: &str, value: &serde_json::Value, output: &mut Vec<BrowserDetail>) {
        match value {
            serde_json::Value::Null => {}
            serde_json::Value::Object(map) => {
                for (key, value) in map {
                    let label = key.replace('_', " ");
                    flatten(
                        &if prefix.is_empty() {
                            label
                        } else {
                            format!("{prefix}: {label}")
                        },
                        value,
                        output,
                    );
                }
            }
            value => output.push(BrowserDetail {
                label: prefix.into(),
                numeric: value.is_number(),
                value: match value {
                    serde_json::Value::String(text) => text.clone(),
                    _ => value.to_string(),
                },
            }),
        }
    }
    let value = serde_json::to_value(value).map_err(|error| error.to_string())?;
    let mut output = Vec::new();
    flatten("", &value, &mut output);
    Ok(output)
}

fn configuration_table(
    config: &CurveConfiguration,
    context: Option<&PricingContext>,
) -> Result<BrowserResponse, String> {
    let mut table = BrowserResponse::table(
        format!("{} — curve instruments & inputs", config.market_index()),
        &[
            "Identifier / pillar",
            "Instrument / input",
            "Quote side",
            "Value",
            "Pillar date",
        ],
    );
    let settings = fields(config)?;
    table.numeric_columns = vec![3];
    table.message = settings
        .iter()
        .filter(|field| field.label != "quotes")
        .map(|field| format!("{}: {}", field.label, field.value))
        .collect::<Vec<_>>()
        .join("; ");
    match config.construction() {
        CurveConstruction::ZeroRate { pillars, rates, .. } => {
            for (pillar, rate) in pillars.iter().zip(rates) {
                let label = serde_json::to_value(pillar).map_err(|error| error.to_string())?;
                table.rows.push(vec![
                    label
                        .as_str()
                        .map_or_else(|| label.to_string(), str::to_string),
                    "Zero rate".into(),
                    String::new(),
                    rate.to_string(),
                    context
                        .map(|context| pillar.date(context.evaluation_date()).to_string())
                        .unwrap_or_default(),
                ]);
                table.parameters.push(fields(config.construction())?);
            }
        }
        CurveConstruction::Composite {
            base_index,
            spread_index,
        } => {
            for (role, index) in [("Base curve", base_index), ("Spread curve", spread_index)] {
                table.rows.push(vec![
                    index.to_string(),
                    role.into(),
                    String::new(),
                    String::new(),
                    String::new(),
                ]);
                table.parameters.push(fields(config.construction())?);
            }
        }
        CurveConstruction::Bootstrapped => {
            for identifier in config.quotes() {
                let quote = context.and_then(|context| context.quote_store().quote(identifier));
                let details = match quote {
                    Some(quote) => quote.details().clone(),
                    None => identifier
                        .parse::<QuoteDetails>()
                        .map_err(|error| error.to_string())?,
                };
                let level = context.map(|context| context.quote_level());
                let value = quote
                    .zip(level)
                    .map(|(quote, level)| quote.levels().value(level))
                    .transpose()
                    .map_err(|error| error.to_string())?;
                let pillar = context
                    .and_then(|context| {
                        let element = context
                            .constructed_elements()
                            .discount_curves()
                            .get(config.market_index())?;
                        let curve = element.curve();
                        let labels = curve.pillar_labels()?;
                        let nodes = curve.nodes()?;
                        labels
                            .into_iter()
                            .zip(
                                nodes
                                    .into_iter()
                                    .filter(|(date, _)| *date != curve.reference_date()),
                            )
                            .find_map(|(label, (date, _))| (label == *identifier).then_some(date))
                    })
                    .or_else(|| {
                        quote.zip(context).and_then(|(quote, context)| {
                            if matches!(
                                details.instrument(),
                                QuoteInstrument::DiscountFactor | QuoteInstrument::ZeroRate
                            ) {
                                return details.maturity().or_else(|| {
                                    details
                                        .tenor()
                                        .map(|tenor| context.evaluation_date() + tenor)
                                });
                            }
                            quote
                                .build_instrument(
                                    context.evaluation_date(),
                                    context.quote_level(),
                                    None,
                                )
                                .and_then(|built| quote.calibration_pillar_date(&built))
                                .ok()
                        })
                    })
                    .map(|date| date.to_string())
                    .unwrap_or_default();
                table.rows.push(vec![
                    identifier.clone(),
                    format!("{:?}", details.instrument()),
                    level.map(|level| format!("{level:?}")).unwrap_or_default(),
                    value.map(|value| value.to_string()).unwrap_or_default(),
                    pillar,
                ]);
                let mut parameters = fields(&details)?;
                if let Some(quote) = quote {
                    parameters.extend(fields(quote.levels())?);
                }
                parameters.extend(settings.iter().filter(|field| field.label != "quotes").map(
                    |field| BrowserDetail {
                        label: format!("Curve: {}", field.label),
                        value: field.value.clone(),
                        numeric: field.numeric,
                    },
                ));
                table.parameters.push(parameters);
            }
            if context.is_none() {
                table.message.push_str(". Select this curve in a pricing context to see market levels and resolved pillar dates.");
            }
        }
    }
    Ok(table)
}

fn evaluation_table(results: &EvaluationResults, request: &str) -> Result<BrowserResponse, String> {
    if request == "Cashflows" {
        let flows = results
            .cashflows()
            .ok_or("This pricer did not return cashflows for this trade.")?;
        let mut table = BrowserResponse::table(
            "Trade cashflows",
            &[
                "Payment date",
                "Type",
                "Amount",
                "Currency",
                "Fixing",
                "Accrual period",
                "Floor strike",
                "Cap strike",
                "Leg index",
                "Accrual start",
                "Accrual end",
                "Fixing index",
                "Coupon index",
                "Signed notional",
                "Coupon rate",
                "Spread",
                "Day count",
                "Rate conventions",
                "Overnight conventions",
                "Payoff",
            ],
        );
        table.message = "Signed, undiscounted amounts in each row's currency. Rates are decimal; leg indices start at 0.".into();
        table.numeric_columns = vec![2, 4, 5, 6, 7, 8, 13, 14, 15];
        for row in 0..flows.payment_dates().len() {
            let details = flows.coupon_details()[row].as_ref();
            let optional =
                |value: Option<f64>| value.map(|value| value.to_string()).unwrap_or_default();
            table.rows.push(vec![
                flows.payment_dates()[row].to_string(),
                flows.cashflow_types()[row].to_string(),
                flows.amounts()[row].to_string(),
                flows.currencies()[row].to_string(),
                optional(flows.fixing()[row]),
                flows.accrual_periods()[row].to_string(),
                optional(flows.floorlet_strikes()[row]),
                optional(flows.caplet_strikes()[row]),
                flows.leg_indices()[row].to_string(),
                details
                    .map(|terms| terms.accrual_start().to_string())
                    .unwrap_or_default(),
                details
                    .map(|terms| terms.accrual_end().to_string())
                    .unwrap_or_default(),
                details
                    .and_then(CouponDetails::fixing_index)
                    .map(ToString::to_string)
                    .unwrap_or_default(),
                details
                    .and_then(|terms| terms.market_index())
                    .map(ToString::to_string)
                    .unwrap_or_default(),
                optional(details.map(|terms| terms.notional())),
                optional(details.and_then(|terms| terms.rate())),
                optional(details.and_then(|terms| terms.spread())),
                details
                    .map(|terms| format!("{:?}", terms.day_counter()))
                    .unwrap_or_default(),
                details
                    .and_then(|terms| terms.rate_definition())
                    .map(|definition| format!("{definition:?}"))
                    .unwrap_or_default(),
                details
                    .and_then(|terms| terms.overnight_conventions())
                    .map(|conventions| format!("{conventions:?}"))
                    .unwrap_or_default(),
                details
                    .and_then(|terms| terms.payoff())
                    .map(|payoff| format!("{payoff:?}"))
                    .unwrap_or_default(),
            ]);
        }
        Ok(table)
    } else if request == "Sensitivities" {
        let risk = results
            .sensitivities()
            .ok_or("This pricer did not return sensitivities for this trade.")?;
        if risk.instrument_keys().len() != risk.exposure().len() {
            return Err("Sensitivity risk factors and values have different lengths.".into());
        }
        let mut table = BrowserResponse::table("Sensitivities", &["Risk factor", "Sensitivity"]);
        table.numeric_columns = vec![1];
        table.message = "Raw derivatives per unit of each risk factor, in the pricer's value units. For decimal-rate factors, multiply by 0.0001 for a +1 bp estimate.".into();
        table.rows = risk
            .instrument_keys()
            .iter()
            .zip(risk.exposure())
            .map(|(key, value)| vec![key.clone(), value.to_string()])
            .collect();
        if table.rows.is_empty() {
            table.message.push_str(" No risk factors were returned.");
        }
        Ok(table)
    } else {
        let (label, value) = if request == "Value" {
            ("PV", results.price())
        } else {
            ("Fair rate", results.fair_rate())
        };
        let value =
            value.ok_or_else(|| format!("This pricer did not return {label} for this trade."))?;
        let mut table = BrowserResponse::table(label, &["Result", "Value"]);
        table.numeric_columns = vec![1];
        table.message = if request == "Value" {
            "Monetary PV under the selected pricer's conventions."
        } else {
            "Decimal rate for the selected zero-based leg or loan segment."
        }
        .into();
        table.rows.push(vec![label.into(), value.to_string()]);
        Ok(table)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        pricing::{PolicyObject, PricerObject},
        products::{InstrumentObject, NativeInstrument, PositionTerms},
    };
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    fn market() -> PricingContext {
        let date = Date::new(2026, 1, 15);
        let mut elements = ConstructedElementStore::default();
        for index in [MarketIndex::SOFR, MarketIndex::ESTR] {
            let label = format!("{index}.flat");
            elements.discount_curves_mut().insert(
                index.clone(),
                DiscountCurveElement::new(
                    index,
                    Rc::new(RefCell::new(
                        FlatForwardTermStructure::new(
                            date,
                            DualFwd::from(0.03),
                            RateDefinition::new(
                                DayCounter::Actual360,
                                Compounding::Continuous,
                                Frequency::Annual,
                            ),
                        )
                        .with_pillar_label(label),
                    )),
                ),
            );
        }
        PricingContext::new()
            .with_quote_store(QuoteStore::new(date))
            .with_constructed_elements(elements)
    }

    #[test]
    fn pricing_queries_cover_trades_and_instrument_previews_without_storing_results() {
        let date = Date::new(2026, 1, 15);
        let swap = MakeSwap::<DualFwd>::default()
            .with_identifier("swap".into())
            .with_start_date(date)
            .with_maturity_date(Date::new(2029, 1, 15))
            .with_currency(Currency::USD)
            .with_market_index(MarketIndex::SOFR)
            .with_notional(100_000.0)
            .with_fixed_rate(0.05)
            .with_spread(0.001)
            .with_rate_definition(RateDefinition::new(
                DayCounter::Actual360,
                Compounding::Simple,
                Frequency::Annual,
            ))
            .with_fixed_leg_frequency(Frequency::Annual)
            .with_floating_leg_frequency(Frequency::Annual)
            .with_side(Side::LongReceive)
            .build()
            .unwrap();
        let contract = InstrumentObject {
            native: NativeInstrument::Swap(swap),
            notional: 100_000.0,
            foreign_notional: None,
        };
        let policy = PolicyObject {
            native: std::sync::Arc::new(SingleCurveCSADiscountPolicy::new(
                MarketIndex::SOFR,
                Currency::USD,
            )),
            kind: "SingleCurveCSADiscountPolicy",
        };
        let pricer = PricerObject::new("DiscountedCashflowPricer", Some(&policy), &vec![]).unwrap();
        let mut registry = Registry::default();
        let trade = registry
            .upsert_if_changed(
                "swap",
                contract
                    .trade(date, Side::LongReceive, &PositionTerms::default())
                    .unwrap(),
                "trade",
            )
            .unwrap();
        let context = registry
            .upsert_if_changed(
                "market",
                QsObject::PricingContext(std::rc::Rc::new(market())),
                "context",
            )
            .unwrap();
        let pricer = registry
            .upsert_if_changed("pricer", QsObject::Pricer(Box::new(pricer)), "pricer")
            .unwrap();
        let instrument = registry
            .upsert_if_changed(
                "swap.contract",
                QsObject::Instrument(Box::new(contract)),
                "contract",
            )
            .unwrap();
        let count = registry.len();
        let mut query = BrowserQuery {
            handle: trade,
            context,
            pricer,
            view: "evaluate".into(),
            request: "Value".into(),
            ..Default::default()
        };
        let pv = execute(&registry, &query).unwrap().rows[0][1]
            .parse::<f64>()
            .unwrap();
        assert!(pv.is_finite() && pv.abs() > 1.0);
        let mut preview: BrowserQuery =
            serde_json::from_str(include_str!("../../ribbon/tests/query-instrument.json")).unwrap();
        preview.handle = instrument;
        preview.context.clone_from(&query.context);
        preview.pricer.clone_from(&query.pricer);
        let preview_pv = execute(&registry, &preview).unwrap().rows[0][1]
            .parse::<f64>()
            .unwrap();
        assert!((preview_pv - pv).abs() < 1e-10);
        preview.side = "Pay".into();
        let opposite_pv = execute(&registry, &preview).unwrap().rows[0][1]
            .parse::<f64>()
            .unwrap();
        assert!((opposite_pv + pv).abs() < 1e-10);
        preview.side.clear();
        assert!(execute(&registry, &preview)
            .err()
            .unwrap()
            .contains("Receive or Pay"));
        preview.side = "Receive".into();
        preview.trade_date.clear();
        assert!(execute(&registry, &preview)
            .err()
            .unwrap()
            .contains("trade date"));
        preview.trade_date = "2026-01-15".into();
        query.side = "Pay".into();
        assert!(execute(&registry, &query)
            .err()
            .unwrap()
            .contains("own position terms"));
        query.side.clear();
        query.request = "Cashflows".into();
        let flows = execute(&registry, &query).unwrap();
        preview.request.clone_from(&query.request);
        assert_eq!(execute(&registry, &preview).unwrap().rows, flows.rows);
        assert!(flows.rows.len() >= 6);
        assert!(flows.rows.iter().any(|row| row[8] == "0"));
        assert!(flows.rows.iter().any(|row| row[8] == "1"));
        assert!(flows
            .rows
            .iter()
            .any(|row| row[1] == "FloatingRateCoupon" && row[11] == "SOFR" && row[12] == "SOFR"));
        assert!(flows
            .rows
            .iter()
            .all(|row| row.len() == flows.columns.len()));
        for target in 0..2 {
            query.request = "FairRate".into();
            query.target = target;
            let value = execute(&registry, &query).unwrap().rows[0][1]
                .parse::<f64>()
                .unwrap();
            assert!(value.is_finite());
            preview.request.clone_from(&query.request);
            preview.target = target;
            let preview_rate = execute(&registry, &preview).unwrap().rows[0][1]
                .parse::<f64>()
                .unwrap();
            assert!((preview_rate - value).abs() < 1e-12);
        }
        query.target = 2;
        assert!(execute(&registry, &query).is_err());
        query.request = "Sensitivities".into();
        let risk = execute(&registry, &query).unwrap();
        assert_eq!(risk.columns, vec!["Risk factor", "Sensitivity"]);
        assert_eq!(risk.numeric_columns, vec![1]);
        assert_eq!(risk.rows.len(), 1);
        assert_eq!(risk.rows[0][0], "SOFR.flat");
        let derivative = risk.rows[0][1].parse::<f64>().unwrap();
        assert!(derivative.is_finite() && derivative.abs() > 1.0);
        preview.request.clone_from(&query.request);
        assert_eq!(execute(&registry, &preview).unwrap().rows, risk.rows);
        query.request = "Unknown".into();
        assert!(execute(&registry, &query).is_err());
        assert_eq!(registry.len(), count);
    }

    #[test]
    fn bond_instrument_previews_require_a_valid_explicit_anchor() {
        let date = Date::new(2026, 1, 15);
        let bond = MakeFixedRateBond::<DualFwd>::default()
            .with_identifier("bond".into())
            .with_start_date(date)
            .with_maturity_date(Date::new(2031, 1, 15))
            .with_currency(Currency::USD)
            .with_notional(100_000.0)
            .with_rate_definition(RateDefinition::default())
            .with_rate(0.05)
            .with_payment_frequency(Frequency::Semiannual)
            .build()
            .unwrap();
        let policy = pricing::fixed_income_policy(
            &vec![vec![
                super::super::inputs::Cell::Text("USD".into()),
                super::super::inputs::Cell::Text("SOFR".into()),
            ]],
            false,
        )
        .unwrap();
        let mut registry = Registry::default();
        let handle = registry
            .upsert_if_changed(
                "bond",
                QsObject::Instrument(Box::new(InstrumentObject {
                    native: NativeInstrument::FixedRateBond(bond),
                    notional: 100_000.0,
                    foreign_notional: None,
                })),
                "bond",
            )
            .unwrap();
        let context = registry
            .upsert_if_changed(
                "market",
                QsObject::PricingContext(std::rc::Rc::new(market())),
                "market",
            )
            .unwrap();
        let pricer = registry
            .upsert_if_changed(
                "bond.pricer",
                QsObject::Pricer(Box::new(
                    PricerObject::new("BondPricer", Some(&policy), &vec![]).unwrap(),
                )),
                "pricer",
            )
            .unwrap();
        let mut query = BrowserQuery {
            handle,
            context,
            pricer,
            view: "evaluate".into(),
            request: "Value".into(),
            trade_date: "2026-01-15".into(),
            side: "Receive".into(),
            ..Default::default()
        };
        assert!(execute(&registry, &query)
            .err()
            .unwrap()
            .contains("pricing_anchor"));
        query.pricing_anchor = "CleanPrice".into();
        query.anchor_value = Some(99.25);
        let pv = execute(&registry, &query).unwrap().rows[0][1]
            .parse::<f64>()
            .unwrap();
        assert!(pv.is_finite() && pv > 0.0);
        query.anchor_value = Some(0.0);
        assert!(execute(&registry, &query)
            .err()
            .unwrap()
            .contains("positive"));
        query.pricing_anchor = "ZSpread".into();
        assert!(execute(&registry, &query).is_ok());
        assert_eq!(
            registry.len(),
            3,
            "previews must not retain trades or results"
        );
    }

    #[test]
    fn context_curves_and_flat_curve_samples_are_selectable_and_not_registered() {
        let mut registry = Registry::default();
        let handle = registry
            .upsert_if_changed(
                "market",
                QsObject::PricingContext(std::rc::Rc::new(market())),
                "context",
            )
            .unwrap();
        let mut query = BrowserQuery {
            handle,
            view: "curves".into(),
            ..Default::default()
        };
        assert_eq!(
            execute(&registry, &query).unwrap().curves,
            vec!["ESTR", "SOFR"]
        );
        query.view = "nodes".into();
        query.curve = "SOFR".into();
        let table = execute(&registry, &query).unwrap();
        assert!(table.message.starts_with("Annual samples"));
        assert_eq!(table.rows[0], vec!["Reference", "2026-01-15", "1", "", ""]);
        assert_eq!(table.rows.len(), 8);
        let years =
            DayCounter::Actual360.year_fraction(Date::new(2026, 1, 15), Date::new(2027, 1, 15));
        assert!((table.rows[1][2].parse::<f64>().unwrap() - (-0.03 * years).exp()).abs() < 1e-12);
        query.curve = "Missing".into();
        assert!(execute(&registry, &query).is_err());
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn corrected_quote_identifiers_rebuild_queryable_curves_under_the_same_name() {
        use super::super::{inputs::Cell, market_inputs::import_quotes, registry::CellId};
        let date = Date::new(2026, 1, 15);
        let cell = |row| CellId {
            sheet: 42,
            row,
            column: 0,
        };
        let mut registry = Registry::default();
        let mut query = BrowserQuery {
            view: "nodes".into(),
            curve: "SOFR".into(),
            ..Default::default()
        };
        let mut previous_rows = Vec::new();
        for identifier in ["OIS_USD_SOFR_1Y", "OIS_USD_SOFR_2Y"] {
            registry.begin_cell(cell(0));
            let quotes = import_quotes(
                &mut registry,
                "quotes",
                date,
                &vec![vec![Cell::Text(identifier.into()), Cell::Number(0.03)]],
            )
            .unwrap();
            registry.finish_cell(true);
            registry.begin_cell(cell(1));
            let configurations = registry
                .upsert_if_changed(
                    "curves",
                    #[cfg(test)]
                    QsObject::CurveConfigurations(vec![CurveConfiguration::new(
                        MarketIndex::SOFR,
                        DayCounter::Actual360,
                        Interpolator::LogLinear,
                        true,
                        vec![identifier.into()],
                    )]),
                    identifier,
                )
                .unwrap();
            registry.finish_cell(true);
            if !query.handle.is_empty() {
                assert!(
                    execute(&registry, &query).is_err(),
                    "old market must be rebuilt"
                );
            }
            registry.begin_cell(cell(2));
            let mut context = PricingContext::new()
                .with_quote_store(registry.quote_store(&quotes).unwrap().clone())
                .with_curve_configurations(
                    registry
                        .curve_configurations(&configurations)
                        .unwrap()
                        .clone(),
                )
                .with_base_currency(Currency::USD)
                .with_base_index(MarketIndex::SOFR);
            context.initialize().unwrap();
            let handle = registry
                .upsert_if_changed(
                    "market",
                    QsObject::PricingContext(std::rc::Rc::new(context)),
                    format!("{quotes}|{configurations}"),
                )
                .unwrap();
            registry.finish_cell(true);
            assert_ne!(handle, query.handle);
            query.handle = handle;
            let table = execute(&registry, &query).unwrap();
            assert_ne!(table.rows, previous_rows);
            assert!(table.rows.iter().any(|row| row[0] == identifier));
            assert_eq!(table.numeric_columns, vec![2, 3, 4]);
            previous_rows = table.rows;
            let inputs = execute(
                &registry,
                &BrowserQuery {
                    handle: query.handle.clone(),
                    curve: query.curve.clone(),
                    view: "instruments".into(),
                    ..Default::default()
                },
            )
            .unwrap();
            assert_eq!(inputs.rows[0][0], identifier);
            assert_eq!(inputs.rows[0][3], "0.03");
            assert!(inputs.parameters[0]
                .iter()
                .any(|field| field.label == "mid" && field.numeric));
            assert_eq!(registry.len(), 3);
        }
    }

    #[test]
    fn calibration_quotes_expose_selected_market_levels_and_structured_parameters() {
        let id = "OIS_USD_SOFR_1Y";
        let config = CurveConfiguration::new(
            MarketIndex::SOFR,
            DayCounter::Actual360,
            Interpolator::LogLinear,
            true,
            vec![id.into()],
        );
        let mut quotes = QuoteStore::new(Date::new(2026, 1, 15));
        quotes.add_quote(Quote::new(
            id.parse().unwrap(),
            QuoteLevels::new(Some(0.03), Some(0.029), Some(0.031)),
        ));
        let context = market()
            .with_quote_store(quotes)
            .with_quote_level(Level::Ask)
            .with_curve_configurations(vec![config.clone()]);
        let table = configuration_table(&config, Some(&context)).unwrap();
        assert_eq!(&table.rows[0][..4], &[id, "OIS", "Ask", "0.031"]);
        assert!(!table.rows[0][4].is_empty());
        for field in [
            "instrument",
            "tenor",
            "market index",
            "mid",
            "bid",
            "ask",
            "Curve: day counter",
        ] {
            assert!(
                table.parameters[0]
                    .iter()
                    .any(|parameter| parameter.label == field),
                "missing {field}"
            );
        }
        let standalone = configuration_table(&config, None).unwrap();
        assert_eq!(standalone.rows[0][3], "");
        assert!(!standalone.parameters[0].is_empty());
    }

    #[test]
    fn explicit_zero_rates_composites_and_node_curves_are_reported() {
        let config: CurveConfiguration = serde_json::from_value(serde_json::json!({
            "market_index": "SOFR", "construction": { "type": "ZeroRate", "pillars": ["1Y", "2Y"], "rates": [0.03, 0.04], "compounding": "Continuous", "frequency": "Annual" }
        })).unwrap();
        let mut context = market().with_curve_configurations(vec![config.clone()]);
        context.initialize().unwrap();
        let table = configuration_table(&config, Some(&context)).unwrap();
        assert_eq!(table.rows[0][3], "0.03");
        assert_eq!(table.rows[0][4], "2027-01-15");
        let element = context
            .constructed_elements()
            .discount_curves()
            .get(&MarketIndex::SOFR)
            .unwrap();
        let table = curve_table(&*element.curve(), "SOFR").unwrap();
        assert_eq!(table.rows.len(), 2);
        assert_eq!(table.rows[0][1], "2027-01-15");
        assert!(!table.message.starts_with("Annual samples"));
        let config = config.with_construction(CurveConstruction::Composite {
            base_index: MarketIndex::SOFR,
            spread_index: MarketIndex::ESTR,
        });
        let table = configuration_table(&config, None).unwrap();
        assert_eq!(table.rows[0][0], "SOFR");
        assert_eq!(table.rows[1][0], "ESTR");
    }

    #[test]
    fn cashflow_json_matches_the_managed_contract_including_blanks_and_unicode() {
        let mut flows = CashflowsTable::new();
        flows.add_cashflow(
            Date::new(2027, 1, 15),
            "Coupon café".into(),
            -1234.5,
            None,
            1.0,
            Currency::EUR,
            None,
            None,
            1,
        );
        let result =
            EvaluationResults::new(Date::new(2026, 1, 15), "café".into()).with_cashflows(flows);
        let table = evaluation_table(&result, "Cashflows").unwrap();
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../ribbon/tests/query-response.json")).unwrap();
        assert_eq!(serde_json::to_value(table).unwrap(), fixture);
        let query: BrowserQuery =
            serde_json::from_str(include_str!("../../ribbon/tests/query.json")).unwrap();
        assert_eq!(query.workbook, "Café.xlsx");
        assert_eq!(query.target, 1);
        assert_eq!(query.request, "FairRate");
    }

    #[test]
    fn sensitivities_preserve_raw_values_labels_and_the_managed_contract() {
        let results = EvaluationResults::new(Date::new(2026, 1, 15), "swap".into())
            .with_sensitivities(
                SensitivityMap::default()
                    .with_instrument_keys(&["SOFR.flat".into(), "FX:USD/EUR.café".into()])
                    .with_exposure(&[-1234.5, 0.00001]),
            );
        let table = evaluation_table(&results, "Sensitivities").unwrap();
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../ribbon/tests/sensitivities-response.json"
        ))
        .unwrap();
        assert_eq!(serde_json::to_value(table).unwrap(), fixture);

        let results = EvaluationResults::new(Date::new(2026, 1, 15), "swap".into());
        assert!(evaluation_table(&results, "Sensitivities")
            .err()
            .unwrap()
            .contains("did not return sensitivities"));
        let empty = results.with_sensitivities(SensitivityMap::default());
        let table = evaluation_table(&empty, "Sensitivities").unwrap();
        assert!(table.rows.is_empty());
        assert!(table.message.contains("No risk factors were returned"));
        let inconsistent = empty.with_sensitivities(
            SensitivityMap::default().with_instrument_keys(&["SOFR.flat".into()]),
        );
        assert!(evaluation_table(&inconsistent, "Sensitivities").is_err());
    }
}
