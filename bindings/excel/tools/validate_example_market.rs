//! Bootstrap inputs extracted from generated worksheets without Windows or Excel.
//! JSON is a test transport over stdin, never a worksheet input format.

use quantsupport::prelude::{
    Currency, CurveConfiguration, Date, DayCounter, Interpolator, Level, MarketIndex,
    PricingContext, QuoteStore, QuoteStoreRecords,
};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
struct CurveInput {
    market_index: MarketIndex,
    day_counter: DayCounter,
    interpolator: Interpolator,
    enable_extrapolation: bool,
    quotes: Vec<String>,
}

impl From<CurveInput> for CurveConfiguration {
    fn from(input: CurveInput) -> Self {
        Self::new(
            input.market_index,
            input.day_counter,
            input.interpolator,
            input.enable_extrapolation,
            input.quotes,
        )
    }
}

#[derive(Deserialize)]
struct ContextInput {
    name: String,
    quotes: QuoteStoreRecords,
    curves: Vec<CurveInput>,
    base_currency: Currency,
    base_index: MarketIndex,
    quote_level: Level,
    query_date: Date,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let inputs: Vec<ContextInput> = serde_json::from_reader(std::io::stdin().lock())?;
    let mut reports = Vec::new();
    for input in inputs {
        let configurations = input
            .curves
            .into_iter()
            .map(CurveConfiguration::from)
            .collect();
        let mut context = PricingContext::new()
            .with_quote_store(QuoteStore::try_from(input.quotes)?)
            .with_curve_configurations(configurations)
            .with_base_currency(input.base_currency)
            .with_base_index(input.base_index)
            .with_quote_level(input.quote_level);
        context
            .initialize()
            .map_err(|error| format!("{}: {error}", input.name))?;
        let mut curves = Vec::new();
        for (index, curve) in context.constructed_elements().discount_curves() {
            let factor = curve.curve().discount_factor(input.query_date)?.value();
            if !factor.is_finite() || factor <= 0.0 {
                return Err(format!("{}: invalid discount factor for {index}", input.name).into());
            }
            curves.push(json!({"market_index": index, "discount_factor": factor}));
        }
        reports.push(json!({"name": input.name, "curves": curves}));
    }
    println!("{}", serde_json::to_string(&reports)?);
    Ok(())
}
