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
        ("QS.EVALUATE.RANGE", "context") => return "One committed context handle for the whole range, or an independent handle array aligned one-for-one with instruments; paired contexts price across workers",
        ("QS.ENGINE.PROBE", "seconds") => return "Synthetic background delay in seconds, from 0 through 30; used to check Excel responsiveness",
        ("QS.ENGINE.PROBE", "rows") => return "Number of result rows from 1 through 10000; one row returns 42, multiple rows spill consecutive values",
        (_, "policy") => return "Optional discount-policy handle for the default native pricer; omitted or blank uses the context policy; leave blank when passing an explicit configured pricer",
        ("QS.SNAPSHOT.FROM.RANGE", "contexts") => return "Independent array of complete prepared context handles; aligned one-for-one with observation timestamps; no header row",
        ("QS.SNAPSHOT.FROM.RANGE", "timestamps") => return "Independent offset-aware RFC 3339 observation timestamp array aligned with contexts; duplicates and interior blanks are rejected",
        ("QS.CONTEXT.QUOTE" | "QS.CONTEXT.ADD.QUOTE", "level") => return "Optional Mid, Bid or Ask; omitted or blank uses Mid; missing selected sides raise an error",

        (name, "identifiers") if name.starts_with("QS.CONTEXT.ADD.") || name == "QS.QUOTES.FROM.RANGE" => return "Independent quote-identifier array: scalar, row, column or spilled array; may be nonadjacent to quote sides; all supplied arrays must have the same entry count; omit for automatic matching in CONTEXT.ADD.CURVE",
        (name, "mid" | "bid" | "ask") if name.starts_with("QS.CONTEXT.ADD.") || name == "QS.QUOTES.FROM.RANGE" => return "Optional independent quote-side array; scalar, row, column or spill; blank omits this side; interior blanks preserve missing observations; supplied arrays match identifier count regardless of orientation",
        ("QS.CONTEXT.ADD.FIXINGS" | "QS.FIXINGS.FROM.RANGE", "market_indices") => return "Independent market-index array, one entry per fixing date and value; scalar, row, column or spill; no implicit broadcasting",
        ("QS.CONTEXT.ADD.FIXINGS" | "QS.FIXINGS.FROM.RANGE", "dates") => return "Independent ISO fixing-date array, aligned by position with indices and values; either row or column orientation",
        ("QS.CONTEXT.ADD.FIXINGS" | "QS.FIXINGS.FROM.RANGE", "values") => return "Independent finite numeric fixing-value array, aligned with indices and dates; interior blanks are not dropped",
        ("QS.CONTEXT.ADD.FX", "base_currencies" | "quote_currencies") => return "Independent currency array, one entry per FX rate; scalar, row, column or spill; supplied arrays must have equal entry counts",
        ("QS.CONTEXT.ADD.FX", "rates") => return "Independent positive FX-rate array, one quote-currency amount per base unit; either row or column orientation",
        ("QS.DISCOUNT.POLICY.FIXED.INCOME", "currency" | "market_index") => return "Independent currency or curve-index array; scalar, row, column or spill; one index per currency; arrays may have different orientations and locations",
        ("QS.DISCOUNT.POLICY.FIXED.INCOME.FROM.MAPPINGS", "currencies" | "market_indices") => return "Independent currency or curve-index array; one index per currency; scalar, row, column or spill; no combined mapping table",
        ("QS.PRICER.CREATE", "setting_names" | "setting_values") => return "Optional independent setting-name or typed setting-value array; omitted or blank uses native defaults; arrays align by position and may have different orientations",
        ("QS.TRADE.LOAN" | "QS.INSTRUMENT.LOAN", "coupon_periods") => return "Optional payment counts per coupon segment, e.g. 12 for 12 monthly payments; align with coupon_rates; only the final count may be blank to fill maturity; counts must cover the whole loan",
        ("QS.TRADE.LOAN" | "QS.INSTRUMENT.LOAN", "coupon_rates") => return "Optional successive annual decimal coupons/spreads, e.g. 0.05 = 5% fixed or 0.01 = 100 bp spread; overrides scalar rate and market_index; align with coupon_periods and coupon_indices; blank uses scalar terms",
        ("QS.TRADE.LOAN" | "QS.INSTRUMENT.LOAN", "coupon_indices") => return "Optional projection index per coupon segment, e.g. EURIBOR12m; blank entry = fixed coupon, otherwise coupon_rates is a spread; omitted array makes ALL segments fixed, regardless of scalar market_index",
        ("QS.TRADE.LOAN" | "QS.INSTRUMENT.LOAN", "prepayment_dates") => return "Optional YYYY-MM-DD observation dates for dated annual CPRs; align with prepayment_cprs; do not combine with constant annual_cpr or refinancing settings",
        ("QS.TRADE.LOAN" | "QS.INSTRUMENT.LOAN", "prepayment_cprs") => return "Optional dated annual prepayment fractions in [0,1], e.g. 0.10 = 10% CPR per year, not per payment; align with prepayment_dates; no prepayment model when all prepayment inputs are blank",
        ("QS.CONTEXT.CREATE" | "QS.EVALUATE.FROM.SNAPSHOT", "quote_level") => return "Optional Mid, Bid or Ask; omitted or blank defaults to Mid; a missing selected side raises an error",
        ("QS.EVALUATE.FROM.SNAPSHOT" | "QS.CONTEXT.FROM.SNAPSHOT", "fixings") => return "Optional prepared context or CONTEXT.ADD.FIXINGS input handle; omitted or blank uses an empty history; histories are not automatically filtered by publication vintage",
        ("QS.CONTEXT.ADD.CURVE", "interpolator") => return "Optional Linear, LogLinear or CubicSpline; omitted or blank defaults to LogLinear",
        ("QS.CONTEXT.ADD.VOL.SURFACE" | "QS.CONTEXT.ADD.VOL.CUBE", "volatility_type") => return "Required native volatility convention: Black or Normal, matching the quote identifiers",
        ("QS.CONTEXT.ADD.VOL.SURFACE" | "QS.CONTEXT.ADD.VOL.CUBE", "smile_type") => return "Required smile coordinate convention: Strike or Delta",
        ("QS.LOAN.SETTINGS", "credit_index") => return "Optional Credit:<entity> curve index; blank leaves borrower credit unset",
        ("QS.LOAN.SETTINGS", "recovery") => return "Optional fraction of opening principal recovered on borrower default, in [0,1], e.g. 0.40 = 40%; blank defaults to 0.40; used with credit_index",
        ("QS.LOAN.SETTINGS", "calendar") => return "Optional payment/fixing calendar, e.g. TARGET; blank defaults to NullCalendar (every day is a business day); contractual accrual boundaries remain unadjusted",
        ("QS.LOAN.SETTINGS", "business_day_convention") => return "Optional payment-date adjustment: Following, ModifiedFollowing, Preceding, ModifiedPreceding or Unadjusted (blank default); contractual accrual dates remain unadjusted",
        ("QS.LOAN.SETTINGS", "floor") => return "Optional floor on the all-in floating coupon (index + spread), annual decimal, e.g. 0 = 0%; blank = no floor; requires scalar market_index; incompatible with coupon arrays and FairRate solving",
        ("QS.LOAN.SETTINGS", "cap") => return "Optional cap on the all-in floating coupon (index + spread), annual decimal, e.g. 0.06 = 6%; blank = no cap; requires scalar market_index; incompatible with coupon arrays and FairRate solving",
        (name, "context") if name.starts_with("QS.CONTEXT.ADD.") => return "The same root context cell from QS.CONTEXT.CREATE for every addition; never reference the previous addition",
        ("QS.EVALUATE" | "QS.EVALUATE.RANGE" | "QS.EVALUATE.RESULTS", "pricer") => return "Optional explicit pricer handle; blank selects the product's native pricer and the context base discount curve; swaptions require an explicit configured pricer",
        ("QS.EVALUATE" | "QS.EVALUATE.RANGE" | "QS.EVALUATE.FROM.SNAPSHOT", "request_name") => return "One request: Value (omitted or blank default), FairRate(index), YieldToMaturity, ModifiedDuration, ZSpread, OAS; QS.EVALUATE also returns Cashflows or Sensitivities tables",
        ("QS.EVALUATE" | "QS.EVALUATE.RANGE", "instrument" | "instruments") => return "Contract or trade handle(s); contracts use Receive at the context date and contractual notional; use a trade for explicit side/date/quantity or bond anchor",
        ("QS.EVALUATE.FROM.SNAPSHOT", "timestamps") => return "Offset-aware origination timestamps aligned one-for-one with instruments in row-major order; blanks are rejected; Previous selects the latest complete market at or before each timestamp",
        ("QS.EVALUATE.FROM.SNAPSHOT", "store") => return "Historical market archive handle; each distinct matched snapshot is constructed once per batch, using its actual market reference date",
        ("QS.CONTEXT.ADD.CURVE" | "QS.CONTEXT.ADD.ZERO.CURVE", "extrapolate") => return "Optional TRUE/FALSE, default FALSE; permits curve evaluation outside its calibrated pillars",
        ("QS.EVALUATE.FROM.SNAPSHOT", "instruments") => return "Contract or trade handles aligned one-for-one with origination timestamps; blanks are rejected; contracts use Receive and contractual notional, while trades preserve position terms",
        ("QS.TRADE.LOAN", "settings") | ("QS.INSTRUMENT.LOAN", "settings") => return "Optional handle returned by QS.LOAN.SETTINGS, not a range or JSON; blank = no credit/prepayment/bounds, 40% recovery, zero fixing lag, NullCalendar and Unadjusted payments",
        ("QS.LOAN.SETTINGS", "balloon_fraction") => return "Principal fraction left for the final balloon, e.g. 0.25 = 25% of original principal; required only for Balloon repayment; invalid with French or Bullet; blank = unset",
        ("QS.LOAN.SETTINGS", "amortization_rate") => return "Annual decimal all-in rate used to build principal instalments, e.g. 0.05 = 5%; required for floating-first French/Balloon loans; blank otherwise uses the first fixed coupon; does not set the floating spread",
        ("QS.LOAN.SETTINGS", "fixing_lag") => return "Whole business days to move backwards from each reset date to its fixing, e.g. 2; nonnegative; blank defaults to 0; uses the selected calendar",
        ("QS.LOAN.SETTINGS", "fixing_frequency") => return "Optional floating reset frequency, e.g. Quarterly or Annual; reset dates must align with payment boundaries; blank uses index metadata (payment_frequency for daily indices); distinct from index_tenor",
        ("QS.LOAN.SETTINGS", "index_tenor") => return "Optional floating index observation tenor, e.g. 3M or 12M; distinct from payment/reset frequency; blank uses index metadata; must be positive",
        ("QS.LOAN.SETTINGS", "introductory_periods") => return "Optional positive count of initial fixed payments, e.g. 12 = first year for Monthly; supply introductory_rate too; must leave a floating payment; incompatible with coupon arrays; blank = no introduction",
        ("QS.LOAN.SETTINGS", "introductory_rate") => return "Annual decimal fixed coupon during introductory_periods, e.g. 0.03 = 3%; afterwards scalar rate is the spread over market_index; supply both introductory fields on a floating loan; blank = no introduction",
        ("QS.LOAN.SETTINGS", "annual_cpr") => return "Optional constant annual prepayment fraction in [0,1], e.g. 0.10 = 10% per year, not per payment; omit for dated CPRs or refinancing settings; all prepayment fields blank = no prepayment",
        ("QS.LOAN.SETTINGS", "base_cpr") => return "Refinancing base annual CPR in [0,1], e.g. 0.02 = 2%; supply all three refinancing fields and a QS.LOAN.PRICER benchmark; excludes annual_cpr/dated CPRs; all prepayment fields blank = no prepayment",
        ("QS.LOAN.SETTINGS", "incentive_multiplier") => return "Nonnegative CPR increase per unit of positive coupon-minus-benchmark incentive, e.g. 4 adds 0.04 CPR for a 0.01 incentive; supply base_cpr and maximum_cpr too; all prepayment fields blank = no prepayment",
        ("QS.LOAN.SETTINGS", "maximum_cpr") => return "Refinancing annual CPR ceiling in [0,1], at least base_cpr, e.g. 0.30 = 30% per year; supply base_cpr and incentive_multiplier too; blank with all prepayment inputs = no prepayment",
        ("QS.LOAN.PRICER", "refinancing_index") => return "Optional benchmark curve for refinancing incentives, e.g. EURIBOR12m; required by the refinancing prepayment model; blank supplies no benchmark",
        ("QS.LOAN.PRICER", "rebuild_amortization") => return "Optional TRUE/FALSE, default FALSE; rebuild the contractual amortization schedule during fair-rate solving",
        _ => {},
    }
    if let Some(product) = function_name.strip_prefix("QS.INSTRUMENT.") {
        let product = match product {
            "FIXED.RATE.BOND" => "FIXED.BOND",
            "FIXED.RATE.DEPOSIT" => "FIXED.DEPOSIT",
            "FLOATING.RATE.NOTE" => "FLOATING.NOTE",
            "RATE.FUTURES" => "RATE.FUTURE",
            other => other,
        };
        if argument_name == "discount_index" {
            return "Optional instrument discount index; blank leaves policy selection to the pricer; a fixed-income policy may prefer or override this index";
        }
        return describe_argument(&format!("QS.TRADE.{product}"), argument_name);
    }
    match (function_name, argument_name) {
        ("QS.PORTFOLIO.CREATE", "trades") => "Cell or range of live trade handles in row-major order; repeated handles count as repeated positions",
        ("QS.EVALUATE.PORTFOLIO", "portfolio") => "Portfolio handle from QS.PORTFOLIO.CREATE",
        ("QS.EVALUATE.PORTFOLIO", "pricers") => "One explicit pricer handle for all members, or a range with one pricer per member in trade-list order",
        ("QS.EVALUATE.PORTFOLIO", "value_currency") => "Common monetary PV currency of the selected pricers, such as USD; blank is allowed for cashflow-only runs; required for returned Value or Sensitivities; not an FX conversion",
        ("QS.EVALUATE.PORTFOLIO", "requests") => "Value, Cashflows and Sensitivities are aggregated; other measures remain absent; a measure omitted by any member remains absent for the portfolio",
        ("QS.QUOTES.FROM.RANGE", "bid") => "Optional Bid row or column array with one entry per identifier; omitted argument or blank cells preserve absent Bid levels",
        ("QS.QUOTES.FROM.RANGE", "ask") => "Optional Ask row or column array with one entry per identifier; omitted argument or blank cells preserve absent Ask levels",
        (_, "all_workbooks") => "Optional TRUE counts live objects across all workbooks in this Excel instance; omitted or FALSE counts the calling workbook",
        (_, "trades") => "Text cell or range of native trade handles; values follow row-major input order; all trades use the supplied explicit pricer and context",
        (_, "result") => "EvaluationResults handle from QS.EVALUATE or QS.EVALUATE.PORTFOLIO; the requested measure must have been returned by the evaluation",
        (_, "target") | ("QS.REQUEST.FAIR.RATE", "index") => "Zero-based leg or loan coupon/spread segment index; omitted or blank target defaults to zero; single-quote products accept only zero",
        (_, "mappings") => "Two-column Currency/Index range selecting ordinary discount curves; optional headers; blank is allowed only when prefer_instrument_index is TRUE",
        (_, "prefer_instrument_index") => "Optional TRUE prefers the instrument's discount index; omitted or blank defaults to FALSE and uses currency/index mappings",
        (_, "collateral_currency") => "Native CSA collateral currency, such as USD",
        (_, "collateral_index") => "Ordinary market index for the native CSA collateral curve, such as SOFR",
        (_, "discount_policy") => "Native discount-policy handle; blank is supported only for RateFuturesPricer, which does not discount cashflows",
        (_, "implementation") => "Native pricer class name, e.g. LoanPricer, DiscountedCashflowPricer, BondPricer or FxForwardPricer",
        (_, "pricer") => "Pricer handle from QS.PRICER.CREATE; must support the native trade type",
        (_, "requests") => "Text cell or range of native request names: Value, Cashflows, Sensitivities, YieldToMaturity, ModifiedDuration, ZSpread, OAS or FairRate(index); QS.REQUEST.FAIR.RATE(index) builds the latter; one fair-rate target per evaluation",
        ("QS.TRADE.CREATE", "instrument") => "Native contract handle from QS.INSTRUMENT.*; trade direction, quantity and bond pricing anchor are defined here",
        ("QS.TRADE.CREATE", "notional") => "Optional position notional; blank uses the contract principal or one unit; loan principal is contractual and cannot be overridden",
        ("QS.TRADE.CREATE", "foreign_notional") => "Optional foreign position principal for cross-currency swaps; blank uses the contract's foreign principal",
        ("QS.TRADE.CREATE", "pricing_anchor") => "Bond anchor type CleanPrice, DirtyPrice or ZSpread; supply anchor_value together; blank for other products",
        ("QS.TRADE.CREATE", "anchor_value") => "Numeric bond price or decimal spread corresponding to pricing_anchor; blank for non-bond trades",
        ("QS.TRADE.CREATE", "settlement_date") => "Optional ISO bond settlement date; blank uses the native trade default; unsupported for other products",
        ("QS.TRADE.CREATE", "spread_day_counter") => "Optional bond spread day counter; blank uses Actual365; unsupported for other products",

        (_, "transpose") => "Optional TRUE transposes the entire spill; omitted or blank defaults to FALSE; table headers move with their fields",
        (_, "headers") => "Optional TRUE includes table headers (default); FALSE removes the first header row before transposing; omitted or blank defaults to TRUE",
        ("QS.TRADE.LOAN", "rate") => "Annual decimal fixed coupon when market_index is blank; otherwise the floating spread, e.g. 0.01 for 100 bp",
        ("QS.TRADE.LOAN", "start_date") => "Contractual advance/accrual start as YYYY-MM-DD text, e.g. 2026-02-24; required; distinct from the position's trade date",
        ("QS.TRADE.LOAN", "maturity_date") => "Final contractual accrual date as YYYY-MM-DD text; required and after start_date; payment date may be adjusted by Loan Settings",
        ("QS.TRADE.LOAN", "notional") => "Positive original principal in currency units, e.g. 10000 = EUR 10,000 for EUR; contractual principal, not a price or percentage",
        ("QS.TRADE.LOAN", "market_index") => "Optional floating projection index such as EURIBOR12m; omitted or blank creates a fixed-rate loan",
        ("QS.TRADE.LOAN", "repayment") => "Optional French = level initial instalments (blank default); Bullet = all principal at maturity; Balloon = amortization plus final residual, requiring balloon_fraction in QS.LOAN.SETTINGS",
        ("QS.TRADE.LOAN", "payment_frequency") => "Optional contractual payment frequency; omitted or blank defaults to Monthly; floating resets independently use index metadata unless settings override it",
        ("QS.TRADE.LOAN", "day_counter") => "Optional interest and amortization day count; omitted or blank defaults to Actual365",
        ("QS.TRADE.LOAN", "side") => "Optional Receive/Buy for lender receipts (default) or Pay/Sell for opposite signs; omitted or blank defaults to Receive",
        ("QS.CREDIT.CURVE.CONFIGS.ADD", "configurations") => "CreditCurveConfigurations handle from QS.CREDIT.CURVE.CONFIGS.CREATE or an earlier ADD; replaces the same credit index or appends a new one",
        ("QS.CREDIT.CURVE.CONFIGS.ADD", "market_index") => "Borrower credit index text, for example Credit:BORROWER",
        ("QS.CREDIT.CURVE.CONFIGS.ADD", "quotes") => "Required worksheet range of CDS quote identifiers already in the quote store; blank cells are skipped",
        ("QS.CREDIT.CURVE.CONFIGS.ADD", "day_counter") => "Optional CDS premium day counter; omitted or blank defaults to Actual360",
        ("QS.CREDIT.CURVE.CONFIGS.ADD", "premium_frequency") => "Optional CDS premium frequency; omitted or blank defaults to Quarterly",
        (name, "point" | "point_x" | "point_y" | "point_z") if name.starts_with("QS.INTERPOLATE") => "Finite numeric query coordinate on the corresponding axis",
        (name, "x_values") if name.starts_with("QS.INTERPOLATE") => "Required numeric range of at least two strictly increasing finite x coordinates; duplicate coordinates, blanks, text, and booleans are rejected; ranges are read in row-major order",
        (name, "y_values") if name.starts_with("QS.INTERPOLATE") => "Required finite numeric values aligned with x_values, with the same cell count; log-linear interpolation requires strictly positive values; blanks, text, and booleans are rejected",
        (name, "x_coordinates" | "y_coordinates" | "z_coordinates") if name.starts_with("QS.INTERPOLATE") => "Required finite numeric coordinate range aligned point-by-point with values in row-major order; all ranges must have the same nonzero cell count; input order may be arbitrary; duplicate full coordinate tuples are rejected",
        (name, "values") if name.starts_with("QS.INTERPOLATE") => "Required finite numeric point values aligned with every coordinate range; blanks, text, and booleans are rejected; rectangular grids, sparse slices, and singleton axes are supported",
        ("QS.INTERPOLATE", "method") => "Optional Linear, LogLinear, or CubicSpline; omitted, blank, or empty text defaults to Linear; CubicSpline uses natural boundary conditions",
        (name, "enable_extrapolation") if name.starts_with("QS.INTERPOLATE") => "Optional TRUE/FALSE or finite numeric flag (nonzero is TRUE); omitted or blank defaults to FALSE; TRUE extends the native end interval (linear, log-linear, or cubic polynomial)",
        (name, "extrapolation") if name.starts_with("QS.INTERPOLATE") => "Optional Reject or Flat; omitted, blank, or empty text defaults to Reject; Flat clamps each axis within each native slice, including singleton axes",
        (_, "timestamp") => "Required offset-aware RFC 3339 text, e.g. 2026-01-15T16:00:00Z or 2026-01-15T17:00:00+01:00; normalized to UTC; date-only and naive timestamps are rejected",
        (_, "snapshot") => "Current MarketSnapshot handle from QS.SNAPSHOT.FROM.CONTEXT or QS.SNAPSHOT.SELECT",
        (_, "snapshots") => "Range of current MarketSnapshot handles; blank range cells are ignored; an omitted or blank scalar creates an empty archive; duplicate UTC timestamps are rejected",
        (_, "selector") => "Current SnapshotSelector handle from QS.SNAPSHOT.SELECTOR; selects one complete market, without rolling its valuation date",
        (_, "rule") => "Optional Exact, Previous, or Closest matching rule; omitted, blank, or empty text defaults to Previous; Closest may select future data, with ties preferring the earlier snapshot",
        ("QS.SNAPSHOT.SELECTOR", "quote_level") => "Optional Mid, Bid, or Ask; omitted, blank, or empty text defaults to Mid; missing quote sides raise errors; explicit zero rates and FX spots are side-independent",
        ("QS.SNAPSHOT.SELECT" | "QS.SNAPSHOT.TIMESTAMPS" | "QS.SNAPSHOT.TO.JSON" | "QS.CONTEXT.FROM.SNAPSHOT", "store") => "Current MarketSnapshotStore handle from QS.SNAPSHOT.ARCHIVE or QS.SNAPSHOT.FROM.JSON",
        (_, "pillars") => "Required ordered range of ISO date text (YYYY-MM-DD) or tenors (0D, 1Y, 10Y); each pillar must have one numeric rate; dates must resolve in increasing order",
        (_, "rates") => "Required numeric range of finite annual decimal zero rates, one per pillar (0.04 means 4%); blanks and text are rejected",
        (_, "spread_index") => "Market index of the configured spread curve; the composite multiplies its discount factors by the base curve's discount factors",
        ("QS.CALENDAR.IS.HOLIDAY" | "QS.CALENDAR.END.OF.MONTH" | "QS.CALENDAR.IS.END.OF.MONTH", "date") => {
            "ISO date text in YYYY-MM-DD format within 1902-01-01..2198-12-31"
        }
        ("QS.CALENDAR.BUSINESS.DAYS.BETWEEN", "from" | "to") => {
            "ISO date bound in YYYY-MM-DD format within 1902-01-01..2198-12-31; reversed bounds produce a negative count with endpoint flags swapped"
        }
        ("QS.CALENDAR.HOLIDAYS" | "QS.CALENDAR.BUSINESS.DAYS", "from" | "to") => {
            "Inclusive ISO date bound in YYYY-MM-DD format within 1902-01-01..2198-12-31; from must be <= to"
        }
        ("QS.CALENDAR.BUSINESS.DAYS.BETWEEN", "include_first") => {
            "Optional TRUE/FALSE or numeric flag (nonzero is TRUE) including the from bound when it is a business day; omitted or blank defaults to TRUE"
        }
        ("QS.CALENDAR.BUSINESS.DAYS.BETWEEN", "include_last") => {
            "Optional TRUE/FALSE or numeric flag (nonzero is TRUE) including the to bound when it is a business day; omitted or blank defaults to FALSE"
        }
        ("QS.CALENDAR.HOLIDAYS", "include_weekends") => {
            "Optional TRUE/FALSE or numeric flag (nonzero is TRUE) including weekend holidays; omitted or blank defaults to FALSE; WeekendsOnly still returns an empty native list"
        }
        ("QS.CURVE.FORWARD" | "QS.CURVE.TABLE" | "QS.CURVE.ZERO.RATE", "compounding") => {
            "Compounding convention; omitted or blank input defaults to Simple"
        }
        ("QS.CURVE.FORWARD" | "QS.CURVE.TABLE" | "QS.CURVE.ZERO.RATE", "frequency") => {
            "Frequency for the rate convention; omitted or blank input defaults to Annual"
        }
        ("QS.CONTEXT.FROM.COMPONENTS", "pricer_configurations") => {
            "Optional PricerConfigurations handle from QS.PRICER.CONFIGS.FROM.JSON; omitted or blank means an empty configuration collection (no explicit pricer/model selections)"
        }
        ("QS.CONTEXT.FROM.COMPONENTS", "scenarios") => {
            "Optional Scenarios handle from QS.SCENARIOS.FROM.JSON; blank applies no shocks; supplied scenario shocks are combined after initialization"
        }
        ("QS.CONTEXT.FROM.COMPONENTS", "quote_level") => {
            "Construction/calibration quote level: Mid, Bid, or Ask; omitted, blank, or empty text defaults to Mid"
        }
        ("QS.TRADE.EUROPEAN.SWAPTION", "calendar") => {
            "Canonical calendar; empty text leaves the underlying swap calendar unset (None), otherwise the supplied calendar is parsed without trimming"
        }
        ("QS.TRADE.EUROPEAN.SWAPTION", "swaption_type") => {
            "Required exact exercise direction: Payer (pay fixed) or Receiver (receive fixed); no blank-input default"
        }
        ("QS.TRADE.EQUITY.FORWARD" | "QS.TRADE.EUROPEAN.SWAPTION", "notional") => {
            "Required finite, non-negative trade notional in currency units; no blank-input default"
        }
        ("QS.TRADE.FX.FORWARD", "forward_rate") => {
            "Contracted FX rate in quote-currency units per base-currency unit"
        }
        ("QS.TRADE.SWAP", "side") => {
            "Required trade direction: Receive/Buy receives the fixed leg, Pay/Sell pays the fixed leg; no blank-input default"
        }
        ("QS.TRADE.FX.OPTION", "strike") => {
            "Absolute FX strike in quote-currency units per base-currency unit"
        }
        ("QS.TRADE.EQUITY.OPTION" | "QS.TRADE.EQUITY.FORWARD", "strike") => {
            "Absolute equity strike in the instrument currency per underlying unit"
        }
        ("QS.TRADE.FIXED.BOND", "pricing_anchor") => {
            "Required exact bond anchor: CleanPrice, DirtyPrice, or ZSpread; no blank-input default; the bond has bullet principal repayment"
        }
        ("QS.TRADE.FLOATING.NOTE", "payment_frequency") => {
            "Required coupon frequency such as Annual, Semiannual, Quarterly, or Monthly; the note has bullet principal repayment"
        }
        ("QS.CURVE.CONFIGS.ADD" | "QS.CURVE.CONFIGS.ADD.ZERO" | "QS.CURVE.CONFIGS.ADD.COMPOSITE", "configurations") => {
            "Current handle returned by QS.CURVE.CONFIGS.CREATE or a previous QS.CURVE.CONFIGS.ADD"
        }
        ("QS.CURVE.ADD", "elements") => {
            "ConstructedElementStore handle returned by QS.ELEMENTS.CREATE or a previous add"
        }
        ("QS.CURVE.ADD", "curve") => {
            "DiscountCurveElement handle returned by QS.CURVE.FLAT"
        }
        ("QS.CURVE.DISCOUNT" | "QS.CURVE.FORWARD" | "QS.CURVE.TABLE" | "QS.CURVE.ZERO.RATE", "curve") => {
            "Discount-curve handle returned by QS.CURVE.GET or QS.CURVE.FLAT"
        }
        ("QS.HELP", "function_name") => {
            "Registered worksheet name, for example QS.CURVE.GET or QS.TRADE.SWAP"
        }
        ("QS.OBJECT.RESET", "reset_key") => {
            "Configuration version or signature; changing it clears the object exactly once"
        }
        (_, "swaption_type") => "Swaption exercise direction: Payer (pay fixed) or Receiver (receive fixed)",
        (_, "convention") => "Business-day convention: Following, ModifiedFollowing, Preceding, ModifiedPreceding, or Unadjusted",
        (_, "calendar") => "Canonical calendar, e.g. Canada(Settlement), Switzerland, China(IB), or JointCalendar(JoinHolidays,TARGET,Japan)",
        (_, "name") => "Workbook-unique object name; unchanged recalculation reuses its current handle",
        (_, "handle") => "Current QuantSupport object handle returned by a QS constructor or mutator",
        (_, "store") => "Current store handle returned by the matching QS create, JSON, or set function",
        (_, "quotes") => "QuoteStore handle returned by QS.QUOTES.CREATE, FROM.JSON, or SET",
        (_, "curve_configurations") => {
            "Optional CurveConfigurations handle from QS.CURVE.CONFIGS.ADD or FROM.JSON; omitted or blank means an empty configuration collection"
        }
        (_, "credit_curve_configurations") => {
            "Optional CreditCurveConfigurations handle from QS.CREDIT.CURVE.CONFIGS.CREATE/ADD; omitted or blank means an empty configuration collection"
        }
        (_, "fixings") => "Optional FixingStore handle; omitted or blank leaves the default empty fixing store",
        (_, "fx") => "Optional FxStore handle; omitted or blank leaves the default empty FX store",
        (_, "volatility_surface_configurations") => {
            "Optional VolatilitySurfaceConfigurations handle from QS.VOL.SURFACE.CONFIGS.FROM.JSON; omitted or blank means an empty configuration collection"
        }
        (_, "volatility_cube_configurations") => {
            "Optional VolatilityCubeConfigurations handle from QS.VOL.CUBE.CONFIGS.FROM.JSON; omitted or blank means an empty configuration collection"
        }
        (_, "simulation_configurations") => {
            "Optional SimulationConfigurations handle from QS.SIMULATION.CONFIGS.FROM.JSON; omitted or blank means an empty configuration collection"
        }
        (_, "constructed_elements") => {
            "Optional ConstructedElementStore handle containing prebuilt elements; omitted or blank starts with an empty store before context initialization"
        }
        (_, "scenarios") => "Optional scenario handle from QS.SCENARIOS.FROM.JSON; blank is allowed",
        (_, "elements") => "Current ConstructedElementStore handle",
        (_, "context") => "Stable workbook-scoped root from QS.CONTEXT.CREATE or revisioned historical context; additions belong to the context and commit after the definition pass",
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
        (_, "mid" | "bid" | "ask") => "Optional numeric quote level in the quote's native units; omitted or blank leaves this level absent, not zero or copied from another level",
        (_, "notional" | "domestic_notional" | "foreign_notional") => {
            "Trade notional in currency units"
        }
        (_, "day_counter") => "Required day-count convention, for example Actual360, Actual365, or Thirty360; no blank-input default",
        (_, "compounding") => "Required compounding convention, for example Simple, Compounded, or Continuous; no blank-input default",
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
        ) => "Required frequency such as Annual, Semiannual, Quarterly, Monthly, or NoFrequency; no blank-input default",
        (_, "side") => "Required Receive/Buy for long-receive, or Pay/Sell for pay-short; no blank-input default",
        (_, "json") => "JSON text in the native QuantSupport serialization shape",
        (_, "interpolator") => "Curve interpolation: Linear, LogLinear, or CubicSpline",
        (_, "enable_extrapolation") => {
            "Required TRUE/FALSE or numeric flag (nonzero is TRUE) allowing dates beyond the final pillar; blank is rejected"
        }
        (_, "quote_identifiers") => {
            "Required quote identifier text or Excel range of identifiers for bootstrap pillars; blank cells in a range are skipped, but at least one identifier is required"
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
        (_, "contract_size") => "Currency value per one futures price point for one contract; 2500 per price point is 25 per 0.01-point move, not 2500 per basis point",
        (_, "number_of_contracts") => "Signed or unsigned number of futures contracts",
        _ => "See the function description and QS.HELP for this argument",
    }
}

pub fn result_help(function_name: &str) -> Option<(&'static str, &'static str)> {
    if function_name == "QS.ENGINE.STATUS" {
        return Some(("JSON job counts and timing metrics. pending == 0, pending_delivery == 0 and settled == true denote delivered worksheet results after the definition pass. Explicitly dirty a nonvolatile status cell when polling from automation.", "#VALUE! if the native engine is unavailable."));
    }
    if function_name == "QS.ENGINE.PROBE" {
        return Some(("42 or a column starting at 42 after the requested background delay.", "#N/A while pending; #VALUE! for invalid arguments or a missing matching RTD companion."));
    }
    match function_name {
        "QS.CONTEXT.QUOTE" => return Some(("Selected quote level in its native units; missing sides do not fall back.", "#VALUE! for stale contexts, absent identifiers or quote sides, or invalid levels.")),
        "QS.CONTEXT.FIXING" => return Some(("Exact historical fixing in the index's native units, with no unit conversion, interpolation or vintage filtering.", "#VALUE! for stale contexts, invalid index/date or absent observations.")),
        "QS.SNAPSHOT.FROM.RANGE" => return Some(("MarketSnapshotStore archive capturing the prepared contexts at aligned observation timestamps; removing entries rebuilds the archive.", "#VALUE! for unequal array lengths, blanks, stale contexts, invalid or duplicate timestamps or runtime objects that cannot be captured.")),
        "QS.EVALUATE.RESULTS" => return Some(("Revisioned opaque handle to worker-owned EvaluationResults; read with asynchronous QS.RESULT functions. #N/A means pending.", "#VALUE! for stale handles, duplicate or foreign context inputs, conflicting policy/pricer, unsupported requests or pricing errors.")),

        "QS.LOAN.PRICER" => return Some(("Pricer handle with the selected currency discount curve and explicit loan pricing settings; pass it to QS.EVALUATE or QS.EVALUATE.FROM.SNAPSHOT.", "#VALUE! for invalid discount mapping, refinancing index, amortization flag or registry failure.")),
        "QS.INSTRUMENT.LOAN" => return Some(("Loan contract handle. QS.EVALUATE prices lender receipts at the context date by default; QS.TRADE.CREATE sets side/date explicitly. Value excludes the original advance; FairRate(index) selects a zero-based coupon/spread segment.", "#VALUE! for invalid dates/principal, missing Balloon fraction or floating-first French/Balloon amortization rate, incomplete coupon segments, conflicting settings/prepayment models or invalid reset grids.")),
        "QS.CONTEXT.CREATE" => return Some(("Stable root context identity containing date, base currency/index and quote side. CONTEXT.ADD cells own contributions on this context; evaluation commits them and builds markets lazily on workers.", "#VALUE! for invalid name, date, currency, index or quote side.")),
        "QS.EVALUATE" => return Some(("Requested scalar or spilled cashflow/sensitivity table directly, without an EvaluationResults handle. Committed market dependencies and identical computations are reused; #N/A means background work is pending.", "#VALUE! for stale, wrong-context or duplicate inputs, missing market data, unsupported request/product or pricing failure; #N/A for an absent table result.")),
        "QS.EVALUATE.RANGE" | "QS.EVALUATE.FROM.SNAPSHOT" => return Some(("One scalar per instrument in input order, with no stored evaluation objects. Historical batches reuse each distinct selected market and preserve its actual valuation date.", "#VALUE! for invalid or misaligned inputs, blank historical timestamps/instruments, stale handles, unsupported scalar requests, missing markets or pricing failure. Historical errors identify the input row.")),
        "QS.LOAN.SETTINGS" => return Some(("LoanSettings handle for QS.INSTRUMENT.LOAN. Blank fields mean no credit, prepayment, bounds or introductory coupon; 40% recovery, zero fixing lag, NullCalendar and Unadjusted payments. Floating-first French/Balloon loans require amortization_rate.", "#VALUE! for invalid settings or conflicting prepayment models; repayment-specific validation occurs when the loan is built.")),
        name if name.starts_with("QS.CONTEXT.ADD.") => return Some(("Context contribution owned by its source cell. All additions reference the same stable root; edits, removals and failed inputs invalidate registered consumers after the definition pass.", "#VALUE! for invalid root context, malformed arrays or conventions, invalid configuration, duplicate observations or registry errors. Market-dependent validation runs at pricing.")),
        _ => {},
    }
    if function_name.starts_with("QS.INSTRUMENT.") {
        return Some(("Native instrument contract handle. Pass it directly to QS.EVALUATE, or create a position with QS.TRADE.CREATE for explicit side, date, quantity or bond anchor.", "#VALUE! for invalid contract terms, missing dependencies, native builder errors or registry failures."));
    }
    let object = match function_name {
        "QS.PORTFOLIO.CREATE" => Some("Revisioned Portfolio handle retaining the ordered list of live trade handles."),
        "QS.EVALUATE.PORTFOLIO" => Some("Standard EvaluationResults handle; cashflow compression preserves coupon types, fixing indices, accrual dates, conventions and pay/receive direction, with notional-weighted compatible rates or spreads. Unsupported or incomplete measures remain absent."),
        "QS.RESULT.COUPONS" => Some("Eighteen-column cashflow spill including original coupon type, fixing and contractual indices, accrual dates, signed notional, weighted rate/spread, day count and overnight/payoff conventions. Optional transpose and headers control layout."),
        "QS.RESULT.CASHFLOW.SUMMARY" | "QS.PORTFOLIO.RESULT.CASHFLOWS" => Some("Six-column cashflow spill: Payment date, Currency, Interest, Capital, Other, Total. Signed undiscounted payments, with optional transpose and headers; no accrual allocation or FX conversion."),
        "QS.TRADE.CREATE" => Some("Revisioned text handle to a native Trade on the supplied instrument, with explicit position direction, quantity and any bond pricing anchor."),
        "QS.DISCOUNT.POLICY.FIXED.INCOME" | "QS.DISCOUNT.POLICY.FIXED.INCOME.FROM.MAPPINGS" | "QS.DISCOUNT.POLICY.CSA" => Some("Revisioned text handle to a native discount policy selecting ordinary market curves; no instrument or trade is modified."),
        "QS.PRICER.CREATE" => Some("Revisioned text handle to an explicit native pricer configured with its discount policy and typed worksheet settings."),
        "QS.EVALUATE.VALUE" => Some("Monetary PV as a number. Excel caches the scalar; no EvaluationResults object is retained.") ,
        "QS.EVALUATE.VALUES" => Some("A numeric PV list, one per trade in input order; vertical by default or transposed when TRUE; no header row or EvaluationResults objects.") ,
        "QS.EVALUATE" => Some("Revisioned EvaluationResults handle containing the requested native measures; each evaluation selects at most one fair-rate target."),
        "QS.REQUEST.FAIR.RATE" => Some("Text request FairRate(index) selecting a zero-based swap leg or loan rate segment; pass to QS.EVALUATE."),
        "QS.RESULT.CASHFLOWS" => Some("Signed undiscounted amounts, positive for receipts and negative for payments. Nine-column cashflow spill: Payment date, Type, Amount, Fixing, Accrual, Currency, Floor, Cap, Leg. Optional transpose and headers control layout."),
        "QS.RESULT.SENSITIVITIES" => Some("Two-column Risk factor/Sensitivity spill of raw value derivatives. Optional transpose and headers control layout."),
        name if name.starts_with("QS.RESULT.") => Some("Scalar native measure from stored evaluation results. Interest rates and spreads are decimals; fair rate corresponds to the target selected in the request."),
        _ => None,
    };
    if let Some(output) = object {
        return Some((output, "#VALUE! for invalid inputs, stale or wrong-type handles, unsupported target/pricer/trade combinations or native evaluation errors; #N/A when the result does not contain the requested measure."));
    }
    let returns = match function_name {
        "QS.INTERPOLATE" | "QS.INTERPOLATE.LINEAR" | "QS.INTERPOLATE.LOG.LINEAR" | "QS.INTERPOLATE.CUBIC.SPLINE" => "Finite numeric interpolated value in the input values' units. One-dimensional data requires at least two strictly increasing coordinates; extrapolation is disabled by default. CubicSpline uses a natural spline and falls back to linear interpolation with two points.",
        "QS.INTERPOLATE.BILINEAR" => "Finite numeric value interpolated along y within each x slice, then along x. Supports rectangular or sparse point ranges and singleton axes; no grid completion or output spill is performed.",
        "QS.INTERPOLATE.TRILINEAR" => "Finite numeric value interpolated along z within each (x,y) slice, then along y and x. Supports rectangular or sparse point ranges and singleton axes; no grid completion or output spill is performed.",
        "QS.DATETIME.PARSE" => "Offset-aware RFC 3339 timestamp text normalized to UTC, preserving nanoseconds; not an Excel serial datetime.",
        "QS.DATETIME.DATE" => "ISO date text for the timestamp's UTC calendar day; this is not a market business or valuation date.",
        "QS.SNAPSHOT.FROM.CONTEXT" => "Revisioned MarketSnapshot handle capturing serializable base observations and configurations. Fixings are excluded. Supplied runtime elements and materialized scenarios cannot be archived; use construction configurations.",
        "QS.SNAPSHOT.ARCHIVE" => "Revisioned MarketSnapshotStore handle rebuilt from all supplied snapshot handles. Removed rows disappear; duplicate UTC instants are rejected. Omitted or blank input creates an empty archive.",
        "QS.SNAPSHOT.SELECTOR" => "Revisioned SnapshotSelector handle. Defaults to Previous and Mid. Closest may choose future data and prefers the earlier snapshot on ties. Selection never rolls the market valuation date.",
        "QS.SNAPSHOT.SELECT" => "Revisioned MarketSnapshot handle for one coherent matched market. All curves, FX, volatility, and model configurations come from that single snapshot.",
        "QS.SNAPSHOT.TIMESTAMP" => "Actual matched observation timestamp as UTC RFC 3339 text, which may differ from the requested timestamp.",
        "QS.SNAPSHOT.REFERENCE.DATE" => "Market business reference date as ISO text; contexts selected from this snapshot use this valuation date.",
        "QS.SNAPSHOT.TIMESTAMPS" => "One-column spill of observation timestamps in ascending UTC order; no header row; an empty archive returns a blank cell.",
        "QS.SNAPSHOT.FROM.JSON" => "Revisioned MarketSnapshotStore handle restored from the native JSON snapshot array shared with Python and Rust. Duplicate observation instants are rejected.",
        "QS.SNAPSHOT.TO.JSON" => "Native snapshot-array JSON text for Python/Rust interoperability. Includes construction configurations and explicit zero-rate pillars, excluding fixing histories, runtime curves, and AD state.",
        "QS.CONTEXT.FROM.SNAPSHOT" => "Revisioned initialized PricingContext handle built from the matched complete market and requested quote side. Its valuation date is the matched market business date. Fixings are supplied separately without publication-vintage filtering.",
        "QS.CONTEXT.EVALUATION.DATE" => "Valuation date as ISO text, from the context quote store reference date.",
        "QS.CONTEXT.SNAPSHOT.TIMESTAMP" => "Actual matched observation timestamp as UTC RFC 3339 text; empty text for contexts without snapshot provenance.",
        "QS.CONTEXT.QUOTE.LEVEL" => "Text naming the context construction quote side: Mid, Bid, or Ask.",
        "QS.CURVE.CONFIGS.ADD.ZERO" => "Current revisioned CurveConfigurations handle after adding or replacing an explicit zero-rate curve. Snapshot-compatible native construction inputs; rate order and curve ranges are validated when constructing the context.",
        "QS.CURVE.CONFIGS.ADD.COMPOSITE" => "Current revisioned CurveConfigurations handle after adding or replacing a composite curve. Its discount factors multiply those of the configured base and spread curves. Dependencies and cycles are validated during context construction.",
        "QS.CURVE.ZERO.RATE" => "Numeric zero rate from the curve reference date to the requested date, as a decimal annual rate under the requested compounding and frequency; blank conventions default to Simple and Annual.",
        "QS.VERSION" => "Text containing the linked XLL package version.",
        "QS.FUNCTIONS" => "One-column spill of sorted, distinct registered worksheet names, including aliases; no header row.",
        "QS.FUNCTIONS.DETAILS" => "Five-column spill with headers Function, Arguments, Category, Aliases, Description; one row per primary registered function, sorted by name. headers=FALSE removes headers; transpose=TRUE exchanges rows and columns.",
        "QS.HELP" => "Two-column help spill of labels and values: Function, Category, Description, optional Aliases, then Parameter/Guidance and argument rows. Names and aliases are matched case-insensitively. headers=FALSE omits Parameter/Guidance while retaining the function metadata; transpose=TRUE exchanges rows and columns.",
        "QS.OBJECT.TYPE" => "Text type tag of the live object, for example QuoteStore or SwapTrade; not an object handle.",
        "QS.OBJECT.COUNT" => "Integer count of live object identities in the calling workbook (or all workbooks when TRUE), capped at i32 maximum; revisions are not counted separately.",
        "QS.OBJECT.REMOVE" => "TRUE when the current object is removed; FALSE when its identity is already absent from the thread's registry.",
        "QS.OBJECT.RESET" => "Current revisioned text handle of the cleared mutable store or configuration collection. An unchanged reset key reuses the handle; older revisions of the same identity are accepted.",
        "QS.DATE.ADVANCE" => "ISO date text (YYYY-MM-DD) advanced by the requested period, without business-day adjustment.",
        "QS.YEAR.FRACTION" => "Numeric native year fraction. ActualActual omits one intervening year when endpoint years differ by at least two; Thirty360US independently adjusts February month-ends after its 31st-day rules and is not generally US/NASD/BondBasis-equivalent.",
        "QS.SCHEDULE" => "One-column spill of unadjusted schedule dates as ISO text (YYYY-MM-DD), without a header row.",
        "QS.CALENDAR.IS.BUSINESS.DAY" => "TRUE if the date is a business day in the supplied calendar; otherwise FALSE.",
        "QS.CALENDAR.ADJUST" => "Business-day-adjusted date as ISO text (YYYY-MM-DD).",
        "QS.CALENDAR.IS.HOLIDAY" => "TRUE if the native calendar considers the date a holiday (the complement of is_business_day); otherwise FALSE.",
        "QS.CALENDAR.END.OF.MONTH" => "Last business day of the input date's month as ISO text (YYYY-MM-DD), using Preceding adjustment of the calendar month end; not necessarily the final civil day.",
        "QS.CALENDAR.IS.END.OF.MONTH" => "TRUE when Following adjustment of date + 1 crosses into another month. This native predicate can also be TRUE on non-business days after the last business day; it is not an equality check against END.OF.MONTH.",
        "QS.CALENDAR.BUSINESS.DAYS.BETWEEN" => "Signed integer business-day count with optional endpoint inclusion (first TRUE, last FALSE by default). Reversed bounds negate the forward count with endpoint flags swapped. Equal bounds count one only when both flags are TRUE and the date is a business day.",
        "QS.CALENDAR.HOLIDAYS" => "One-column spill of native holiday-list ISO dates (YYYY-MM-DD), ascending with inclusive bounds and no header row; weekends are excluded by default. Native WeekendsOnly and NullCalendar return empty lists even when include_weekends is TRUE.",
        "QS.CALENDAR.BUSINESS.DAYS" => "One-column spill of native business-day-list ISO dates (YYYY-MM-DD), ascending with inclusive bounds and no header row. Native WeekendsOnly and NullCalendar return empty lists rather than enumerating dates, even though their business-day predicates may return TRUE.",
        "QS.SCHEDULE.ADJUSTED" => "One-column spill of business-day-adjusted schedule dates as ISO text (YYYY-MM-DD), without a header row.",
        "QS.CURVE.FLAT" => "Revisioned text handle to a flat DiscountCurveElement; usable directly for curve queries or with QS.CURVE.ADD, not a discount-factor scalar.",
        "QS.CURVE.GET" => "Reusable revisioned text handle to the requested DiscountCurveElement from the initialized PricingContext.",
        "QS.CURVE.DISCOUNT" => "Dimensionless discount factor at the requested date, as a numeric scalar.",
        "QS.CURVE.FORWARD" => "Forward interest rate between the two dates as a decimal scalar (0.03 means 3%), using the curve's day count and requested compounding/frequency.",
        "QS.CURVE.TABLE" => "Five-column spill with headers Pillar, Date, Discount factor, Zero rate, Forward from prior node. Dates are ISO text, discount factors dimensionless, and rates decimal. The reference-date zero rate and first-node forward are blank.",
        "QS.ELEMENTS.CREATE" => "Revisioned text handle to an initially empty ConstructedElementStore, ready for QS.CURVE.ADD or QS.CONTEXT.FROM.COMPONENTS.",
        "QS.CURVE.ADD" => "Current revisioned text handle to the ConstructedElementStore after adding or replacing the discount curve by market index; unchanged inputs reuse its revision.",
        "QS.QUOTES.CREATE" => "Revisioned text handle to an empty QuoteStore with the supplied reference date.",
        "QS.QUOTES.FROM.JSON" => "Revisioned text handle to a QuoteStore populated from native QuoteStoreRecords JSON.",
        "QS.QUOTES.FROM.RANGE" => "Revisioned text handle to a QuoteStore populated from independent field arrays; changing or removing entries replaces prior inputs; unchanged inputs reuse the handle.",
        "QS.QUOTES.SET" => "Current revisioned text handle to the QuoteStore after adding or replacing the quote; unchanged inputs reuse its revision.",
        "QS.QUOTES.GET" => "Numeric Mid, Bid, or Ask quote level in the quote's native units; no percentage or basis-point conversion is applied.",
        "QS.FIXINGS.CREATE" => "Revisioned text handle to an initially empty FixingStore.",
        "QS.FIXINGS.FROM.JSON" => "Revisioned text handle to a FixingStore populated from its native JSON representation.",
        "QS.FIXINGS.FROM.RANGE" => "Revisioned text handle to a FixingStore populated from independent field arrays; changing or removing entries replaces prior inputs; unchanged inputs reuse the handle.",
        "QS.FIXINGS.SET" => "Current revisioned text handle to the FixingStore after adding or replacing the index/date fixing; unchanged inputs reuse its revision.",
        "QS.FIXINGS.GET" => "Numeric stored historical fixing for the index/date in the index's native units; no unit conversion, interpolation, forecasting, or missing-observation fallback is applied.",
        "QS.FX.CREATE" => "Revisioned text handle to an initially empty FxStore.",
        "QS.FX.FROM.JSON" => "Revisioned text handle to an FxStore populated from native FxRateRecord JSON values.",
        "QS.FX.SET" => "Current revisioned text handle to the FxStore after adding or replacing the currency-pair rate; unchanged inputs reuse its revision.",
        "QS.FX.GET" => "Numeric FX rate in quote-currency units per base-currency unit, using a direct, inverse, or triangulated rate.",
        "QS.CURVE.CONFIGS.CREATE" => "Revisioned text handle to an initially empty CurveConfigurations collection; no curves are built yet.",
        "QS.CURVE.CONFIGS.ADD" => "Current revisioned text handle to CurveConfigurations after adding or replacing the market index's bootstrap configuration; curves are built by QS.CONTEXT.FROM.COMPONENTS.",
        "QS.CURVE.CONFIGS.FROM.JSON" => "Revisioned text handle to a collection of CurveConfiguration values parsed from JSON; no curves are built yet.",
        "QS.CREDIT.CURVE.CONFIGS.FROM.JSON" => "Revisioned text handle to a collection of CreditCurveConfiguration values parsed from JSON; no credit curves are built yet.",
        "QS.CREDIT.CURVE.CONFIGS.CREATE" => "Revisioned text handle to an empty credit-curve configuration collection. Use QS.CREDIT.CURVE.CONFIGS.ADD with worksheet arguments and a CDS quote range.",
        "QS.CREDIT.CURVE.CONFIGS.ADD" => "Revisioned text handle to credit-curve configurations with the supplied credit index replaced or appended. Build curves by supplying the returned handle to QS.CONTEXT.FROM.COMPONENTS.",
        "QS.VOL.SURFACE.CONFIGS.FROM.JSON" => "Revisioned text handle to a collection of VolatilitySurfaceConfiguration values parsed from JSON; no surfaces are built yet.",
        "QS.VOL.CUBE.CONFIGS.FROM.JSON" => "Revisioned text handle to a collection of VolatilityCubeConfiguration values parsed from JSON; no cubes are built yet.",
        "QS.PRICER.CONFIGS.FROM.JSON" => "Revisioned text handle to a collection of PricerConfiguration values selecting instrument pricers and typed model assignments; not a PricingContext handle.",
        "QS.SIMULATION.CONFIGS.FROM.JSON" => "Revisioned text handle to a collection of SimulationConfiguration values parsed from JSON; no paths are generated yet.",
        "QS.SCENARIOS.FROM.JSON" => "Revisioned text handle to a collection of Scenario definitions parsed from JSON; shocks are not applied until context construction.",
        "QS.CONTEXT.FROM.COMPONENTS" => "Revisioned text handle to an initialized PricingContext containing the supplied market data and constructed/calibrated elements, with any supplied scenario shocks applied to its worksheet snapshot.",
        "QS.CONTEXT.CURVES" => "One-column spill of sorted discount-curve market-index text from the context, without a header row; these are index names, not curve handles.",
        "QS.CONTEXT.VOL.SURFACES" => "One-column spill of sorted volatility-surface market-index text from the context, without a header row; not surface handles.",
        "QS.CONTEXT.VOL.CUBES" => "One-column spill of sorted volatility-cube market-index text from the context, without a header row; not cube handles.",
        "QS.CONTEXT.SIMULATIONS" => "One-column spill of sorted simulation market-index text from the context, without a header row; not simulation handles.",
        "QS.VOL.SURFACE" => "Numeric volatility at the expiry date and lookup key, in the constructed surface's configured volatility convention; not a surface handle.",
        "QS.VOL.CUBE" => "Numeric volatility at the expiry date, underlying maturity tenor, and lookup key, in the constructed cube's configured volatility convention; not a cube handle.",
        "QS.SIMULATION.NPATHS" => "Integer number of simulated paths for the requested market index.",
        "QS.SIMULATION.DT" => "Numeric simulation time step in years, as reported by the simulation (an average step for generated date grids).",
        "QS.SIMULATION.DATES" => "One-column spill of simulation dates in path order as ISO text (YYYY-MM-DD), without a header row.",
        "QS.SIMULATION.PATH" => "Numeric vector spill for one zero-based simulation path, in simulation-date order. Values use the simulated state's native units and align with QS.SIMULATION.DATES; no header is included.",
        "QS.SIMULATION.CALIBRATION" => "Eleven-column Hull-White diagnostics spill with headers identifier, expiry, t, big_t, market_vol, market_price, model_price, calibrated_sigma, forward_rate, effective_strike, price_error. Expiry is tenor text such as 1Y, not an ISO date; price_error is model_price minus market_price, not an absolute error.",
        "QS.TRADE.FX.FORWARD" => "Revisioned text handle to a deliverable FxForwardTrade, ready for pricing; not the forward rate or present value.",
        "QS.TRADE.SWAP" => "Revisioned text handle to a vanilla fixed-versus-floating SwapTrade; not a par rate or present value.",
        "QS.TRADE.LOAN" => "Compatibility constructor returning a native LoanTrade handle. Evaluate with an explicit LoanPricer and discount policy through QS.EVALUATE; use QS.INSTRUMENT.LOAN and QS.TRADE.CREATE to keep contract and position separate. Value is signed future receipt PV excluding the advance; FairRate(index) selects one coupon/spread segment.",
        "QS.TRADE.BASIS.SWAP" => "Revisioned text handle to a floating-versus-floating BasisSwapTrade; not a basis spread or present value.",
        "QS.TRADE.XCCY.FLOAT.FLOAT" => "Revisioned text handle to a FloatFloatCrossCurrencySwapTrade with the supplied domestic and foreign legs.",
        "QS.TRADE.XCCY.FIX.FLOAT" => "Revisioned text handle to a FixFloatCrossCurrencySwapTrade with a domestic fixed leg and foreign floating leg.",
        "QS.TRADE.FIXED.BOND" => "Revisioned text handle to a bullet FixedRateBondTrade with the supplied CleanPrice, DirtyPrice, or ZSpread anchor; not a quoted bond price.",
        "QS.TRADE.FIXED.DEPOSIT" => "Revisioned text handle to a FixedRateDepositTrade; not a deposit rate or present value.",
        "QS.TRADE.FLOATING.NOTE" => "Revisioned text handle to a bullet FloatingRateNoteTrade; not a coupon or present value.",
        "QS.TRADE.FX.OPTION" => "Revisioned text handle to an FxEuropeanOptionTrade for the supplied currency pair, absolute strike, and Call/Put direction.",
        "QS.TRADE.EQUITY.OPTION" => "Revisioned text handle to an EquityEuropeanOptionTrade with the supplied absolute strike and Call/Put direction.",
        "QS.TRADE.CDS" => "Revisioned text handle to a CdsTrade referencing the credit entity and discount index; not a CDS spread or present value.",
        "QS.TRADE.CAP.FLOOR" => "Revisioned text handle to a CapFloorTrade for the supplied index and cap/floor strike.",
        "QS.TRADE.CAPLET.FLOORLET" => "Revisioned text handle to a CapletFloorletTrade with the supplied fixing, accrual, and payment dates and absolute strike.",
        "QS.TRADE.RATE.FUTURE" => "Revisioned text handle to a RateFuturesTrade with the supplied contract size and number of contracts; not a quoted futures price.",
        "QS.TRADE.EQUITY.FORWARD" => "Revisioned text handle to an EquityForwardTrade with the supplied absolute strike, notional, and delivery date.",
        "QS.TRADE.EUROPEAN.SWAPTION" => "Revisioned text handle to a EuropeanSwaptionTrade; its underlying swap has unit notional and the trade carries the supplied notional. Pricing requires a configured Hull-White model.",
        "QS.PRICE" => "Numeric present value returned by the selected trade pricer, in its valuation currency and trade-size conventions; not an object handle or a clean/dirty bond price quote.",
        "QS.BOND.PRICE" => "Numeric fixed-rate bond present value using the trade's price or Z-spread anchor, not the clean/dirty price quoted per bond quote units.",
        "QS.BOND.MODIFIED.DURATION" => "Numeric yield-modified duration in years, computed from the anchored straight-bond cashflows and yield convention.",
        "QS.BOND.Z.SPREAD" => "Numeric straight-bond Z-spread as a decimal rate (0.001 means 10 basis points), resolved from the trade's anchor.",
        "QS.BOND.OAS" => "Intended scalar option-adjusted spread as a decimal rate; the current worksheet path accepts only straight fixed-rate bonds and rejects their OAS request, so no successful OAS result is available through the current constructors.",
        "QS.BOND.SENSITIVITIES" => "Two-column spill with headers Risk factor, Sensitivity, including curve risk and the bond's dPV/dZSpread node. Values are raw derivatives per unit risk-factor change, not automatically scaled to one basis point.",
        "QS.FAIR.RATE" => "Scalar fair coupon, floating spread or forward quote for the selected zero-based target; use QS.EVALUATE with an explicit pricer for loans and policy selection.",
        "QS.SENSITIVITIES" => "Two-column spill with headers Risk factor, Sensitivity. Values are raw derivatives of the pricer value per unit risk-factor change, in the pricer's valuation units; no automatic basis-point scaling.",
        "QS.CASHFLOWS" => "Nine-column spill with headers Payment date, Type, Amount, Fixing, Accrual, Currency, Floor, Cap, Leg. Dates are ISO text; Amount is in the row currency, Accrual is a year fraction, and absent Fixing/Floor/Cap values are blank. Generic discounted-cashflow reports contain signed amounts, positive for receipts and negative for payments, including supplied principal exchanges and reversed disbursements; there is no PV column or automatic reporting-currency conversion.",
        _ => return None,
    };
    let errors = match function_name {
        "QS.INTERPOLATE" | "QS.INTERPOLATE.LINEAR" | "QS.INTERPOLATE.LOG.LINEAR" | "QS.INTERPOLATE.CUBIC.SPLINE" => "#VALUE! for invalid methods or flags, blank/text/boolean/nonfinite range cells, mismatched ranges, fewer than two points, unordered or duplicate x coordinates, nonpositive log-linear values, forbidden extrapolation, or a nonfinite result.",
        "QS.INTERPOLATE.BILINEAR" | "QS.INTERPOLATE.TRILINEAR" => "#VALUE! for an unknown extrapolation policy, empty or mismatched coordinate/value ranges, blank/text/boolean/nonfinite range cells or queries, duplicate coordinate tuples, a query outside any required slice under Reject, or a nonfinite result.",
        "QS.DATETIME.PARSE" => "#VALUE! for invalid, naive, or date-only input; an explicit RFC 3339 timezone offset is required.",
        "QS.DATETIME.DATE" => "#VALUE! for invalid, naive, or date-only input; an explicit RFC 3339 timezone offset is required.",
        "QS.SNAPSHOT.FROM.CONTEXT" => "#VALUE! for invalid timestamps, stale/wrong-type context handles, supplied runtime market elements that cannot be serialized, or registry failures.",
        "QS.SNAPSHOT.ARCHIVE" => "#VALUE! for nontext snapshot inputs, stale/wrong-type snapshot handles, duplicate UTC timestamps, or registry failures.",
        "QS.SNAPSHOT.SELECTOR" => "#VALUE! for invalid or naive timestamps, unknown matching rules or quote levels, or registry failures.",
        "QS.SNAPSHOT.SELECT" => "#VALUE! for stale/wrong-type archive or selector handles, no snapshot satisfying the rule, or registry failures.",
        "QS.SNAPSHOT.TIMESTAMP" => "#VALUE! for a malformed, missing, stale, or wrong-type MarketSnapshot handle.",
        "QS.SNAPSHOT.REFERENCE.DATE" => "#VALUE! for a malformed, missing, stale, or wrong-type MarketSnapshot handle.",
        "QS.SNAPSHOT.TIMESTAMPS" => "#VALUE! for a malformed, missing, stale, or wrong-type MarketSnapshotStore handle.",
        "QS.SNAPSHOT.FROM.JSON" => "#VALUE! for invalid native snapshot JSON, duplicate UTC instants, or registry failures.",
        "QS.SNAPSHOT.TO.JSON" => "#VALUE! for malformed, missing, stale, or wrong-type archive handles, serialization failures, or JSON exceeding Excel's 32767 UTF-16-unit cell limit.",
        "QS.CONTEXT.FROM.SNAPSHOT" => "#VALUE! for stale/wrong-type dependencies, no matching snapshot, missing consumed quote sides, market construction/calibration failures, or registry failures. Missing Bid/Ask inputs never fall back to Mid.",
        "QS.CONTEXT.EVALUATION.DATE" => "#VALUE! for a malformed, missing, stale, or wrong-type PricingContext handle.",
        "QS.CONTEXT.SNAPSHOT.TIMESTAMP" => "#VALUE! for a malformed, missing, stale, or wrong-type PricingContext handle.",
        "QS.CONTEXT.QUOTE.LEVEL" => "#VALUE! for a malformed, missing, stale, or wrong-type PricingContext handle.",
        "QS.CURVE.CONFIGS.ADD.ZERO" => "#VALUE! for invalid index/conventions/pillars, empty or mismatched ranges, nonnumeric/blank/nonfinite rates, or registry failures. Invalid resolved pillar ordering or range constraints fail during context construction.",
        "QS.CURVE.CONFIGS.ADD.COMPOSITE" => "#VALUE! for invalid indices or configuration handles or registry failures; missing dependencies and cycles fail during context construction.",
        "QS.CURVE.ZERO.RATE" => "#VALUE! for invalid date/conventions, a stale or wrong-type curve handle, or native zero-rate evaluation failures.",
        "QS.OBJECT.COUNT" => "#VALUE! for an invalid all_workbooks flag; use TRUE/FALSE or leave it blank.",
        "QS.VERSION" =>
            "No worksheet-level Result error is returned.",
        "QS.FUNCTIONS" | "QS.FUNCTIONS.DETAILS" => "#VALUE! for invalid layout flags; use TRUE/FALSE or leave the argument blank.",
        "QS.HELP" => "#VALUE! if the requested name or alias is not registered.",
        "QS.OBJECT.TYPE" => "#VALUE! for a malformed, missing, wrong-type, or stale handle.",
        "QS.OBJECT.REMOVE" => "#N/A for a malformed or stale handle; an already-absent identity returns FALSE rather than an error.",
        "QS.OBJECT.RESET" => "#VALUE! for malformed or missing handles, a type mismatch, an object that cannot be reset, or revision overflow; an older revision of the same identity is allowed.",
        "QS.DATE.ADVANCE" => "#VALUE! if the ISO date or time unit cannot be parsed.",
        "QS.YEAR.FRACTION" => "#VALUE! if either ISO date or the day-count convention cannot be parsed.",
        "QS.SCHEDULE" => "#VALUE! for invalid dates/frequency or a schedule-builder failure.",
        "QS.CALENDAR.IS.BUSINESS.DAY" => "#VALUE! if the ISO date or calendar cannot be parsed.",
        "QS.CALENDAR.ADJUST" => "#VALUE! if the ISO date, calendar, or business-day convention cannot be parsed.",
        "QS.CALENDAR.IS.HOLIDAY" | "QS.CALENDAR.END.OF.MONTH" | "QS.CALENDAR.IS.END.OF.MONTH" =>
            "#VALUE! for an invalid canonical calendar or ISO date, or a date outside 1902-01-01..2198-12-31.",
        "QS.CALENDAR.BUSINESS.DAYS.BETWEEN" =>
            "#VALUE! for an invalid canonical calendar or ISO date, a date outside 1902-01-01..2198-12-31, or a non-boolean/nonnumeric endpoint flag. Reversed bounds are supported.",
        "QS.CALENDAR.HOLIDAYS" =>
            "#VALUE! for an invalid canonical calendar or ISO date, a date outside 1902-01-01..2198-12-31, from > to, or a non-boolean/nonnumeric include_weekends flag.",
        "QS.CALENDAR.BUSINESS.DAYS" =>
            "#VALUE! for an invalid canonical calendar or ISO date, a date outside 1902-01-01..2198-12-31, or from > to.",
        "QS.SCHEDULE.ADJUSTED" => "#VALUE! for invalid dates, frequency, calendar, business-day convention, or a schedule-builder failure.",
        "QS.CURVE.FLAT" => "#VALUE! for invalid index/date/rate-convention text or registry/fingerprint failures.",
        "QS.CURVE.GET" => "#VALUE! for an invalid index, invalid/stale/wrong-type context handle, a discount curve not constructed for the index, or a registry failure.",
        "QS.CURVE.DISCOUNT" => "#VALUE! for an invalid date or curve handle, a stale/wrong-type handle, or a discount-factor evaluation failure (including range restrictions).",
        "QS.CURVE.FORWARD" => "#VALUE! for invalid dates or rate conventions, an invalid/stale/wrong-type curve handle, or a forward-rate evaluation failure.",
        "QS.CURVE.TABLE" => "#VALUE! for invalid rate conventions or curve handles, curves exposing no nodes or an empty node set, or zero/forward-rate evaluation failures. Flat curves need not expose nodes; use scalar curve queries instead.",
        "QS.CURVE.ADD" => "#VALUE! for malformed, missing, or wrong-type handles, a stale curve handle, or a registry update failure.",
        "QS.QUOTES.CREATE" => "#VALUE! for an invalid reference date or registry/fingerprint failure.",
        "QS.QUOTES.FROM.JSON" => "#VALUE! for invalid QuoteStoreRecords JSON, quote-store conversion failures, or registry failures.",
        "QS.QUOTES.FROM.RANGE" => "#VALUE! for invalid reference date, nonvector or differently sized side arrays, invalid array dimensions, quote identifiers, nonnumeric quote levels, duplicate identifiers, or registry failures.",
        "QS.QUOTES.SET" => "#VALUE! for an invalid quote identifier, malformed/missing/wrong-type store handle, or registry/fingerprint failure.",
        "QS.QUOTES.GET" => "#N/A for an invalid/stale/wrong-type store handle, missing quote or requested level, or a level other than Mid, Bid, or Ask.",
        "QS.FIXINGS.FROM.JSON" => "#VALUE! for invalid native FixingStore JSON or registry failures.",
        "QS.FIXINGS.FROM.RANGE" => "#VALUE! for invalid range dimensions, index/date cells, nonfinite or nonnumeric values, duplicate index/date observations, or registry failures.",
        "QS.FIXINGS.SET" => "#VALUE! for an invalid index/date, malformed/missing/wrong-type store handle, or registry/fingerprint failure.",
        "QS.FIXINGS.GET" => "#VALUE! for index/date parse failures; #N/A for invalid/stale/wrong-type store handles or missing historical fixings.",
        "QS.FX.FROM.JSON" => "#VALUE! for invalid native FxRateRecord JSON or registry failures.",
        "QS.FX.SET" => "#VALUE! for invalid currency codes, malformed/missing/wrong-type store handles, or registry/fingerprint failures.",
        "QS.FX.GET" => "#VALUE! for currency parse failures; #N/A for invalid/stale/wrong-type store handles or an unavailable direct, inverse, or triangulated FX rate.",
        "QS.CURVE.CONFIGS.ADD" => "#VALUE! for invalid index/day-count/interpolator inputs, a blank or non-boolean/nonnumeric extrapolation flag, a missing/empty or nontext identifier range, or invalid configuration handles/registry updates.",
        "QS.CONTEXT.FROM.COMPONENTS" => "#VALUE! for invalid quote level, base currency/index, malformed/stale/wrong-type component handles, initialization/bootstrap/calibration failures, scenario application failures, or registry failures. Blank optional handles retain empty defaults, not automatic configurations.",
        "QS.CONTEXT.CURVES" | "QS.CONTEXT.VOL.SURFACES" | "QS.CONTEXT.VOL.CUBES" | "QS.CONTEXT.SIMULATIONS" =>
            "#VALUE! for an invalid, missing, stale, or wrong-type PricingContext handle.",
        "QS.VOL.SURFACE" => "#VALUE! for invalid index/expiry or context handles, a surface not constructed for the index, or surface lookup/interpolation/range failures.",
        "QS.VOL.CUBE" => "#VALUE! for invalid index/expiry/maturity unit or context handles, a cube not constructed for the index, or cube lookup/interpolation/range failures.",
        "QS.SIMULATION.NPATHS" => "#VALUE! for invalid index/context handles, a simulation not constructed for the index, or a path count that does not fit in i32.",
        "QS.SIMULATION.DT" | "QS.SIMULATION.DATES" =>
            "#VALUE! for invalid index/context handles or a simulation not constructed for the index.",
        "QS.SIMULATION.PATH" => "#VALUE! for invalid index/context handles, a missing simulation, a negative path index, or a path index outside the stored path vector.",
        "QS.SIMULATION.CALIBRATION" => "#VALUE! for invalid index/context handles, a missing simulation, or a simulation without Hull-White calibration diagnostics.",
        "QS.TRADE.FIXED.BOND" => "#VALUE! for an anchor other than exact CleanPrice, DirtyPrice, or ZSpread, an invalid anchor value, invalid dates/currency/index/rate conventions/side, or builder/registry failures.",
        "QS.TRADE.EQUITY.FORWARD" => "#VALUE! for nonfinite strike/notional, negative notional, invalid dates/index/currency/day count/side, or builder/registry failures.",
        "QS.TRADE.EUROPEAN.SWAPTION" => "#VALUE! for nonfinite strike/notional, negative notional, maturity not after expiry, swaption_type other than exact Payer or Receiver, invalid dates/index/currency/rate conventions/calendar/side, or builder/registry failures.",
        "QS.TRADE.CAPLET.FLOORLET" => "#VALUE! for invalid date/index/currency/Caplet-Floorlet direction/side text or registry/fingerprint failures.",
        "QS.TRADE.FX.FORWARD" => "#VALUE! for invalid trade/delivery dates, currency codes, side, deliverable FX-forward builder failures, or registry/fingerprint failures.",
        "QS.TRADE.SWAP" => "#VALUE! for invalid trade/start/maturity dates, currency/index, side, day count, compounding, or rate/leg frequencies, or swap-builder/registry failures.",
        "QS.TRADE.LOAN" => "#VALUE! for invalid contract terms, nonpositive principal, malformed settings handles or mismatched schedule arrays, unsupported repayment, missing balloon_fraction, floating amortization without amortization_rate, invalid reset grids, or registry failures. Pricing errors propagate through the valuation functions.",
        "QS.TRADE.BASIS.SWAP" => "#VALUE! for invalid trade/start/maturity dates, currency, pay/receive indices or frequencies, side, or basis-swap-builder/registry failures.",
        "QS.TRADE.XCCY.FLOAT.FLOAT" => "#VALUE! for invalid trade/start/maturity dates, domestic/foreign currencies, indices or frequencies, side, or cross-currency-swap-builder/registry failures.",
        "QS.TRADE.XCCY.FIX.FLOAT" => "#VALUE! for invalid trade/start/maturity dates, domestic/foreign currencies, foreign index, rate/leg conventions or frequencies, side, or cross-currency-swap-builder/registry failures.",
        "QS.TRADE.FIXED.DEPOSIT" => "#VALUE! for invalid trade/start/maturity dates, currency/discount index, day count, compounding, rate frequency, side, or deposit-builder/registry failures.",
        "QS.TRADE.FLOATING.NOTE" => "#VALUE! for invalid trade/start/maturity dates, currency/forward index, payment frequency, side, or floating-note-builder/registry failures.",
        "QS.TRADE.FX.OPTION" => "#VALUE! for invalid trade/expiry dates, currencies or FX pair, Call/Put direction, day count, side, or FX-option-builder/registry failures.",
        "QS.TRADE.EQUITY.OPTION" => "#VALUE! for invalid trade/expiry dates, market index/currency, Call/Put direction, day count, side, or equity-option-builder/registry failures.",
        "QS.TRADE.CDS" => "#VALUE! for invalid trade/start/maturity dates, credit/discount indices, currency, premium frequency, day count, side, native CDS contract validation, or registry/fingerprint failures.",
        "QS.TRADE.CAP.FLOOR" => "#VALUE! for invalid trade/start/maturity dates, index/currency, Cap/Floor direction, day count, compounding, frequencies, side, or cap/floor-builder/registry failures.",
        "QS.TRADE.RATE.FUTURE" => "#VALUE! for invalid trade/start/end dates, market index, side, rate-futures-builder failures, or registry/fingerprint failures.",
        "QS.BOND.OAS" => "#VALUE! for invalid trade/context handles, a non-fixed-rate-bond object, or the OAS request on a straight bond (currently always rejected). #N/A if an otherwise successful evaluation omitted OAS.",
        "QS.BOND.PRICE" => "#VALUE! for invalid trade/context handles, a non-fixed-rate-bond trade, or bond valuation/anchor-solving failures; #N/A if a successful evaluation omitted the price.",
        "QS.BOND.MODIFIED.DURATION" => "#VALUE! for invalid trade/context handles, a non-fixed-rate-bond trade, or valuation/yield/duration failures (including zero bond price); #N/A if a successful evaluation omitted modified duration.",
        "QS.BOND.Z.SPREAD" => "#VALUE! for invalid trade/context handles, a non-fixed-rate-bond trade, or valuation/anchor-solving failures; #N/A if a successful evaluation omitted Z-spread.",
        "QS.BOND.SENSITIVITIES" => "#VALUE! for invalid trade/context handles, a non-fixed-rate-bond trade, or valuation/risk failures; #N/A if a successful evaluation omitted sensitivities.",
        "QS.PRICE" => "#VALUE! for invalid trade/context handles, an unsupported trade, missing market data/model configuration, or valuation failures; #N/A if a successful evaluation omitted the price.",
        "QS.FAIR.RATE" => "#VALUE! for invalid trade/context handles, unsupported trade/request combinations, or pricing failures; #N/A if a successful evaluation omitted the fair rate.",
        "QS.SENSITIVITIES" => "#VALUE! for invalid trade/context handles, unsupported trade/request combinations, or valuation/risk failures; #N/A if a successful evaluation omitted sensitivities.",
        "QS.CASHFLOWS" => "#VALUE! for invalid trade/context handles, unsupported trade/request combinations, or cashflow-resolution failures; #N/A if a successful evaluation omitted cashflows.",
        name if name.ends_with(".CONFIGS.FROM.JSON") || name == "QS.SCENARIOS.FROM.JSON" =>
            "#VALUE! for invalid JSON or values that do not deserialize into the function's native configuration/scenario type, or registry failures. Parsing does not perform context construction or calibration.",
        "QS.CREDIT.CURVE.CONFIGS.ADD" => "#VALUE! for invalid configuration handles, non-credit indices, recovery outside [0,1], missing quote identifiers, invalid conventions, or registry failures.",
        "QS.ELEMENTS.CREATE" | "QS.FIXINGS.CREATE" | "QS.FX.CREATE" | "QS.CURVE.CONFIGS.CREATE" | "QS.CREDIT.CURVE.CONFIGS.CREATE" =>
            "#VALUE! for object-registry creation or revision failures.",
        _ => return None,
    };
    Some((returns, errors))
}

#[cfg(test)]
mod tests {
    use super::{argument_help, result_help};

    #[test]
    fn returns_one_hint_per_registered_argument() {
        let help = argument_help("QS.CURVE.TABLE", "curve,compounding,frequency");
        assert_eq!(help.len(), 3);
        assert!(help[0].contains("QS.CURVE.GET"));
        assert!(help[1].contains("Compounding"));
    }

    #[test]
    fn distinguishes_optional_conventions_from_required_range_inputs() {
        let help = argument_help("QS.CURVE.FORWARD", "compounding,frequency");
        assert!(help[0].contains("defaults to Simple"));
        assert!(help[1].contains("defaults to Annual"));
        let help = argument_help(
            "QS.CURVE.CONFIGS.ADD",
            "enable_extrapolation,quote_identifiers",
        );
        assert!(help[0].contains("blank is rejected"));
        assert!(help[1].contains("at least one identifier is required"));
        let help = argument_help(
            "QS.CONTEXT.FROM.COMPONENTS",
            "pricer_configurations,quote_level,scenarios",
        );
        assert!(help[0].contains("empty configuration collection"));
        assert!(help[1].contains("defaults to Mid"));
        assert!(help[2].contains("blank applies no shocks"));
    }

    #[test]
    fn documents_trade_defaults_without_defaulting_required_conventions() {
        let help = argument_help(
            "QS.TRADE.EUROPEAN.SWAPTION",
            "calendar,swaption_type,compounding,rate_frequency",
        );
        assert!(help[0].contains("empty text"));
        assert!(help[0].contains("unset (None)"));
        assert!(help[1..]
            .iter()
            .all(|hint| hint.contains("no blank-input default")));
        let help = argument_help("QS.QUOTES.SET", "mid,bid,ask");
        assert!(help.iter().all(|hint| hint.contains("level absent")));
    }

    #[test]
    fn documents_calendar_bounds_flags_and_native_semantics() {
        let help = argument_help(
            "QS.CALENDAR.BUSINESS.DAYS.BETWEEN",
            "from,to,include_first,include_last",
        );
        assert!(help[..2]
            .iter()
            .all(|hint| hint.contains("1902-01-01..2198-12-31")));
        assert!(help[2].contains("blank defaults to TRUE"));
        assert!(help[3].contains("blank defaults to FALSE"));
        let help = argument_help("QS.CALENDAR.HOLIDAYS", "from,to,include_weekends");
        assert!(help[..2]
            .iter()
            .all(|hint| hint.contains("from must be <= to")));
        assert!(help[2].contains("blank defaults to FALSE"));
        let (returns, _) = result_help("QS.CALENDAR.IS.END.OF.MONTH").unwrap();
        assert!(returns.contains("non-business days"));
        let (returns, _) = result_help("QS.CALENDAR.BUSINESS.DAYS.BETWEEN").unwrap();
        assert!(returns.contains("endpoint flags swapped"));
        assert!(returns.contains("both flags are TRUE"));
        for name in ["QS.CALENDAR.HOLIDAYS", "QS.CALENDAR.BUSINESS.DAYS"] {
            let (returns, errors) = result_help(name).unwrap();
            assert!(returns.contains("WeekendsOnly and NullCalendar return empty lists"));
            assert!(returns.contains("One-column spill"));
            assert!(errors.contains("from > to"));
        }
    }

    #[test]
    fn documents_lookup_errors_and_current_oas_limitation() {
        let (_, errors) = result_help("QS.QUOTES.GET").unwrap();
        assert!(errors.starts_with("#N/A"));
        for name in ["QS.FIXINGS.GET", "QS.FX.GET", "QS.PRICE"] {
            let (_, errors) = result_help(name).unwrap();
            assert!(errors.contains("#VALUE!"));
            assert!(errors.contains("#N/A"));
        }
        let (returns, errors) = result_help("QS.BOND.OAS").unwrap();
        assert!(returns.contains("no successful OAS result"));
        assert!(errors.contains("currently always rejected"));
        assert!(result_help("QS.NOT.REGISTERED").is_none());
    }
}
