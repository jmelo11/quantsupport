use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use quantsupport::prelude::*;
use serde::Serialize;

// ---------------------------------------------------------------------------
// Output data structures (serialised to JSON)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct OutputResults {
    pub reference_date: String,
    pub curves: Vec<CurveOutput>,
    pub products: Vec<ProductOutput>,
}

#[derive(Serialize)]
pub struct CurveOutput {
    pub name: String,
    pub nodes: Vec<CurveNode>,
}

#[derive(Serialize)]
pub struct CurveNode {
    pub date: String,
    pub year_fraction: f64,
    pub discount_factor: f64,
}

#[derive(Serialize)]
pub struct ProductOutput {
    pub label: String,
    pub npv: f64,
    pub sensitivities: Vec<SensitivityEntry>,
    pub cashflows: Vec<CashflowEntry>,
}

#[derive(Serialize)]
pub struct SensitivityEntry {
    pub pillar: String,
    pub exposure: f64,
    pub dv01: f64,
}

#[derive(Serialize)]
pub struct CashflowEntry {
    pub payment_date: String,
    pub accrual_start: String,
    pub accrual_end: String,
    pub cashflow_type: String,
    pub notional: f64,
    pub rate: Option<f64>,
    pub year_fraction: f64,
    pub amount: f64,
    pub currency: String,
    pub side: String,
    pub discount_factor: f64,
}

// ---------------------------------------------------------------------------
// Helpers to extract cashflow details from legs
// ---------------------------------------------------------------------------

pub fn extract_cashflows(
    legs: &[Leg<ADReal>],
    curve_lookup: &HashMap<MarketIndex, DiscountCurveElement>,
    csa_index: MarketIndex,
    csa_currency: Currency,
) -> Vec<CashflowEntry> {
    let policy = SingleCurveCSADiscountPolicy::new(csa_index, csa_currency);
    let mut entries = Vec::new();

    for leg in legs {
        let side_str = match leg.side() {
            Side::LongReceive => "Receive",
            Side::PayShort => "Pay",
        };
        let disc_index = policy.accept(leg).ok();

        for cf in leg.cashflows() {
            let entry = match cf {
                CashflowType::FixedRateCoupon(c) => {
                    let yf = c
                        .rate()
                        .day_counter()
                        .year_fraction(c.accrual_start_date(), c.accrual_end_date());
                    let df = disc_index
                        .as_ref()
                        .and_then(|idx| curve_lookup.get(idx))
                        .and_then(|elem| elem.curve().discount_factor(c.payment_date()).ok())
                        .map_or(f64::NAN, |v| v.value());
                    CashflowEntry {
                        payment_date: c.payment_date().to_string(),
                        accrual_start: c.accrual_start_date().to_string(),
                        accrual_end: c.accrual_end_date().to_string(),
                        cashflow_type: "FixedRateCoupon".into(),
                        notional: c.notional(),
                        rate: Some(c.rate().rate().value()),
                        year_fraction: yf,
                        amount: c.amount().map_or(f64::NAN, |a| a.value()),
                        currency: format!("{}", leg.currency()),
                        side: side_str.into(),
                        discount_factor: df,
                    }
                }
                CashflowType::FloatingRateCoupon(c) => {
                    let yf = c
                        .day_counter()
                        .year_fraction(c.accrual_start_date(), c.accrual_end_date());
                    let df = disc_index
                        .as_ref()
                        .and_then(|idx| curve_lookup.get(idx))
                        .and_then(|elem| elem.curve().discount_factor(c.payment_date()).ok())
                        .map_or(f64::NAN, |v| v.value());
                    CashflowEntry {
                        payment_date: c.payment_date().to_string(),
                        accrual_start: c.accrual_start_date().to_string(),
                        accrual_end: c.accrual_end_date().to_string(),
                        cashflow_type: "FloatingRateCoupon".into(),
                        notional: LinearCoupon::<ADReal>::notional(c),
                        rate: c.fixing().map(|f| f.value()),
                        year_fraction: yf,
                        amount: c.amount().map_or(f64::NAN, |a| a.value()),
                        currency: format!("{}", leg.currency()),
                        side: side_str.into(),
                        discount_factor: df,
                    }
                }
                CashflowType::Redemption(c) => {
                    let df = disc_index
                        .as_ref()
                        .and_then(|idx| curve_lookup.get(idx))
                        .and_then(|elem| elem.curve().discount_factor(c.payment_date()).ok())
                        .map_or(f64::NAN, |v| v.value());
                    CashflowEntry {
                        payment_date: c.payment_date().to_string(),
                        accrual_start: String::new(),
                        accrual_end: String::new(),
                        cashflow_type: "Redemption".into(),
                        notional: c.amount().unwrap_or(0.0),
                        rate: None,
                        year_fraction: 0.0,
                        amount: c.amount().unwrap_or(0.0),
                        currency: format!("{}", leg.currency()),
                        side: side_str.into(),
                        discount_factor: df,
                    }
                }
                CashflowType::Disbursement(c) => {
                    let df = disc_index
                        .as_ref()
                        .and_then(|idx| curve_lookup.get(idx))
                        .and_then(|elem| elem.curve().discount_factor(c.payment_date()).ok())
                        .map_or(f64::NAN, |v| v.value());
                    CashflowEntry {
                        payment_date: c.payment_date().to_string(),
                        accrual_start: String::new(),
                        accrual_end: String::new(),
                        cashflow_type: "Disbursement".into(),
                        notional: c.amount().unwrap_or(0.0),
                        rate: None,
                        year_fraction: 0.0,
                        amount: c.amount().unwrap_or(0.0),
                        currency: format!("{}", leg.currency()),
                        side: side_str.into(),
                        discount_factor: df,
                    }
                }
                _ => continue,
            };
            entries.push(entry);
        }
    }
    entries
}

// ---------------------------------------------------------------------------
// Curve node extraction
// ---------------------------------------------------------------------------

pub fn extract_curve_nodes(
    name: &str,
    elem: &DiscountCurveElement,
    rd: Date,
    dc: DayCounter,
) -> CurveOutput {
    let curve = elem.curve();
    let tenors = [
        "1D", "1M", "3M", "6M", "1Y", "2Y", "3Y", "4Y", "5Y", "7Y", "10Y", "15Y", "20Y", "30Y",
    ];
    let nodes: Vec<CurveNode> = tenors
        .iter()
        .filter_map(|t| {
            let period = Period::from_str(t).ok()?;
            let date = rd + period;
            let df = curve.discount_factor(date).ok()?.value();
            let yf = dc.year_fraction(rd, date);
            Some(CurveNode {
                date: date.to_string(),
                year_fraction: yf,
                discount_factor: df,
            })
        })
        .collect();
    CurveOutput {
        name: name.to_string(),
        nodes,
    }
}

// ---------------------------------------------------------------------------
// Write results to JSON file
// ---------------------------------------------------------------------------

pub fn write_results(
    output: &OutputResults,
    path: &Path,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::to_string_pretty(output)
        .map_err(|e| QSError::InvalidValueErr(e.to_string()))?;
    let mut file = File::create(path)?;
    file.write_all(json.as_bytes())?;
    println!("\n✓ Results written to {}", path.display());
    Ok(())
}
