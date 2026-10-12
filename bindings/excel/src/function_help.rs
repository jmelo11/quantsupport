//! Excel Function Arguments help for worksheet functions.
//!
//! `xllgen` exposes an `arg_help` field through `xll-rs`, but the version used
//! by this crate does not populate it.  The XLL therefore derives consistent
//! help text from the registered argument names when it calls `xlfRegister`.

pub fn argument_help(function_name: &str, argument_names: &str) -> Vec<&'static str> {
    argument_names
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|argument_name| describe_argument(function_name, argument_name))
        .collect()
}

fn describe_argument(function_name: &str, argument_name: &str) -> &'static str {
    match (function_name, argument_name) {
        ("QS.CURVE.CONFIGS.ADD", "configurations") => {
            "Current handle returned by QS.CURVE.CONFIGS.CREATE or a previous QS.CURVE.CONFIGS.ADD"
        }
        ("QS.CURVE.ADD", "elements") => {
            "ConstructedElementStore handle returned by QS.ELEMENTS.CREATE or a previous add"
        }
        ("QS.CURVE.ADD", "curve") => {
            "DiscountCurveElement handle returned by QS.CURVE.FLAT"
        }
        ("QS.CURVE.DISCOUNT" | "QS.CURVE.FORWARD" | "QS.CURVE.TABLE", "curve") => {
            "Discount-curve handle returned by QS.CURVE.GET or QS.CURVE.FLAT"
        }
        ("QS.HELP", "function_name") => {
            "Registered worksheet name, for example QS.CURVE.GET or QS.TRADE.SWAP"
        }
        ("QS.OBJECT.RESET", "reset_key") => {
            "Configuration version or signature; changing it clears the object exactly once"
        }
        (_, "convention") => "Business-day convention: Following, ModifiedFollowing, Preceding, ModifiedPreceding, or Unadjusted",
        (_, "calendar") => "Canonical calendar, e.g. Japan, UnitedKingdom(Settlement), or JointCalendar(JoinHolidays,TARGET,Japan)",
        (_, "name") => "Workbook-unique object name; unchanged recalculation reuses its current handle",
        (_, "handle") => "Current QuantSupport object handle returned by a QS constructor or mutator",
        (_, "store") => "Current store handle returned by the matching QS create, JSON, or set function",
        (_, "quotes") => "QuoteStore handle returned by QS.QUOTES.CREATE, FROM.JSON, or SET",
        (_, "curve_configurations") => {
            "Optional curve-configuration handle from QS.CURVE.CONFIGS.ADD or FROM.JSON; blank is allowed"
        }
        (_, "credit_curve_configurations") => {
            "Optional credit-curve handle from QS.CREDIT.CURVE.CONFIGS.FROM.JSON; blank is allowed"
        }
        (_, "fixings") => "Optional FixingStore handle; blank is allowed",
        (_, "fx") => "Optional FxStore handle; blank is allowed",
        (_, "volatility_surface_configurations") => {
            "Optional volatility-surface handle from QS.VOL.SURFACE.CONFIGS.FROM.JSON; blank is allowed"
        }
        (_, "volatility_cube_configurations") => {
            "Optional volatility-cube handle from QS.VOL.CUBE.CONFIGS.FROM.JSON; blank is allowed"
        }
        (_, "simulation_configurations") => {
            "Optional simulation handle from QS.SIMULATION.CONFIGS.FROM.JSON; blank is allowed"
        }
        (_, "constructed_elements") => {
            "Optional ConstructedElementStore handle containing flat/prebuilt curves; blank is allowed"
        }
        (_, "scenarios") => "Optional scenario handle from QS.SCENARIOS.FROM.JSON; blank is allowed",
        (_, "elements") => "Current ConstructedElementStore handle",
        (_, "context") => "Initialized PricingContext handle returned by QS.CONTEXT.CREATE",
        (_, "pricing_anchor") => "Bond trade anchor: CleanPrice, DirtyPrice, or ZSpread",
        (_, "anchor_value") => "Bond price per quote units or decimal spread, according to pricing_anchor",
        (_, "trade") => "Trade handle returned by a QS.TRADE.* function",
        (_, "market_index" | "base_index" | "forward_index" | "discount_index") => {
            "Market index, for example SOFR; prefixes include Equity:, Credit:, FX:, and Collateral:"
        }
        (_, "pay_index" | "receive_index" | "domestic_index" | "foreign_index") => {
            "Market index whose curve is present in the pricing context, for example SOFR or EURIBOR3M"
        }
        (_, "credit_index") => "Credit market index using the Credit:<entity> form",
        (_, "reference_date" | "trade_date" | "start_date" | "end_date" | "maturity_date") => {
            "ISO date text in YYYY-MM-DD format"
        }
        (_, "delivery_date" | "expiry_date" | "fixing_date" | "accrual_start" | "accrual_end") => {
            "ISO date text in YYYY-MM-DD format"
        }
        (_, "payment_date") => "ISO date text in YYYY-MM-DD format",
        (_, "date") => "ISO date text in YYYY-MM-DD format",
        (_, "currency" | "base_currency" | "quote_currency") => {
            "Three-letter ISO currency code, for example USD or EUR"
        }
        (_, "domestic_currency" | "foreign_currency") => {
            "Three-letter ISO currency code, for example USD or EUR"
        }
        (_, "rate" | "fixed_rate" | "coupon_rate" | "forward_rate") => {
            "Rate as a decimal, for example 0.03 for 3%"
        }
        (_, "spread" | "pay_spread" | "receive_spread") => {
            "Spread as a decimal, for example 0.001 for 10 basis points"
        }
        (_, "domestic_spread" | "foreign_spread") => {
            "Spread as a decimal, for example 0.001 for 10 basis points"
        }
        (_, "mid" | "bid" | "ask") => "Optional quote level; use a decimal number or leave blank",
        (_, "notional" | "domestic_notional" | "foreign_notional") => {
            "Trade notional in currency units"
        }
        (_, "day_counter") => "Day-count convention, for example Actual360, Actual365, or Thirty360",
        (_, "compounding") => "Compounding convention, for example Simple, Compounded, or Continuous",
        (
            _,
            "frequency"
            | "rate_frequency"
            | "fixed_leg_frequency"
            | "floating_leg_frequency"
            | "pay_frequency"
            | "receive_frequency"
            | "domestic_frequency"
            | "foreign_frequency"
            | "payment_frequency"
            | "premium_frequency",
        ) => "Frequency such as Annual, Semiannual, Quarterly, Monthly, or NoFrequency",
        (_, "side") => "Receive/Buy for long-receive, or Pay/Sell for pay-short",
        (_, "json") => "JSON text in the native QuantSupport serialization shape",
        (_, "interpolator") => "Curve interpolation: Linear, LogLinear, or CubicSpline",
        (_, "enable_extrapolation") => {
            "TRUE to allow dates beyond the final curve pillar; otherwise FALSE"
        }
        (_, "quote_identifiers") => {
            "One-column Excel range of quote identifiers to use as this curve's bootstrap pillars"
        }
        (_, "identifier") => "Quote identifier understood by QuantSupport, for example OIS_USD_SOFR_1Y",
        (_, "level") => "Quote level: Mid, Bid, or Ask",
        (_, "quote_level") => "Quote level for construction and calibration: Mid (default), Bid, or Ask",
        (_, "value") => "Numeric fixing value",
        (_, "length" | "maturity_length") => "Whole-number tenor length",
        (_, "unit" | "maturity_unit") => "Tenor unit: Days, Weeks, Months, or Years",
        (_, "key") => "Surface/cube lookup key, such as strike, moneyness, or delta per its configuration",
        (_, "path_index") => "Zero-based simulation path number",
        (_, "strike") => "Option strike as a decimal price or rate",
        (_, "option_type") => "Option type: Call or Put",
        (_, "cap_floor_type") => "Instrument type: Cap or Floor",
        (_, "caplet_floorlet_type") => "Instrument type: Caplet or Floorlet",
        (_, "recovery") => "Recovery rate as a decimal between 0 and 1",
        (_, "futures_price") => "Quoted futures price, for example 95.25",
        (_, "contract_size") => "Currency value of one futures contract",
        (_, "number_of_contracts") => "Signed or unsigned number of futures contracts",
        _ => "See the function description and QS.HELP for this argument",
    }
}

#[cfg(test)]
mod tests {
    use super::argument_help;

    #[test]
    fn returns_one_hint_per_registered_argument() {
        let help = argument_help("QS.CURVE.TABLE", "curve,compounding,frequency");
        assert_eq!(help.len(), 3);
        assert!(help[0].contains("QS.CURVE.GET"));
        assert!(help[1].contains("Compounding"));
    }
}
