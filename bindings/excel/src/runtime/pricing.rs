//! Explicit native discount policies, pricers and evaluation of typed positions.

use super::{
    inputs::{self, Cell, Table},
    registry::QsObject,
};
use quantsupport::{
    core::{evaluator::Evaluator, pricer::ErasedPricer},
    prelude::*,
};
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    result::Result,
    sync::Arc,
};

#[derive(Clone)]
pub struct PolicyObject {
    pub native: Arc<dyn DiscountPolicy>,
    pub kind: &'static str,
}

impl DiscountPolicy for PolicyObject {
    fn accept(
        &self,
        target: &dyn Discountable,
    ) -> quantsupport::utils::errors::Result<MarketIndex> {
        self.native.accept(target)
    }
    fn discount_indices(&self) -> Vec<MarketIndex> {
        self.native.discount_indices()
    }
}

pub fn fixed_income_policy(
    mappings: &Table,
    prefer_instrument_index: bool,
) -> Result<PolicyObject, String> {
    let mut native = FixedIncomeDiscountPolicy::new(prefer_instrument_index);
    let mut seen = std::collections::HashSet::new();
    for (position, row) in inputs::rows(mappings, 2, "discount mappings (Currency, Index)")?
        .into_iter()
        .enumerate()
    {
        if position == 0
            && matches!(&row[0], Cell::Text(value) if inputs::label(value) == "currency")
        {
            if inputs::label(row[1].text("mapping header")?) != "index" {
                return Err("discount mapping headers must be Currency, Index".into());
            }
            continue;
        }
        let currency = row[0]
            .text("discount currency")?
            .to_ascii_uppercase()
            .parse::<Currency>()
            .map_err(|error| error.to_string())?;
        if !seen.insert(currency) {
            return Err(format!("duplicate discount mapping for {currency}"));
        }
        let text = row[1].text("discount curve index")?;
        if text.is_empty() {
            return Err("discount curve index cannot be blank".into());
        }
        let index = inputs::market_index(text)?;
        native = native.with_risk_free_index(currency, index);
    }
    if seen.is_empty() && !prefer_instrument_index {
        return Err("fixed-income policy requires a currency/index mapping".into());
    }
    Ok(PolicyObject {
        native: Arc::new(native),
        kind: "FixedIncomeDiscountPolicy",
    })
}

pub struct PricerObject {
    native: Evaluator,
}

impl PricerObject {
    pub fn new(
        implementation: &str,
        policy: Option<&PolicyObject>,
        settings: &Table,
    ) -> Result<Self, String> {
        let key = inputs::label(implementation);
        if key != "ratefuturespricer" && policy.is_none() {
            return Err("an explicit discount-policy handle is required".into());
        }
        if key == "ratefuturespricer" && policy.is_some() {
            return Err(
                "native RateFuturesPricer does not use a discount policy; leave it blank".into(),
            );
        }
        let mut refi = None;
        let mut rebuild = false;
        let mut alpha = None;
        let mut sigma = None;
        let mut seen = std::collections::HashSet::new();
        for (position, row) in inputs::rows(settings, 2, "pricer settings")?
            .into_iter()
            .enumerate()
        {
            let name = row[0].text("pricer setting name")?;
            let setting = inputs::label(name);
            if position == 0
                && setting == "setting"
                && inputs::label(row[1].text("settings header")?) == "value"
            {
                continue;
            }
            if !seen.insert(setting.clone()) {
                return Err(format!("duplicate pricer setting '{name}'"));
            }
            match (key.as_str(), setting.as_str()) {
                ("loanpricer", "refinancingbenchmarkindex") => {
                    if !row[1].is_blank() {
                        refi = Some(inputs::market_index(row[1].text(name)?)?);
                    }
                }
                ("loanpricer", "rebuildamortizationforfairrate") => {
                    if !row[1].is_blank() {
                        rebuild = row[1].flag(name)?;
                    }
                }
                ("closedformhullwhiteswaptionpricer", "alpha") => {
                    if !row[1].is_blank() {
                        alpha = Some(row[1].number(name)?);
                    }
                }
                ("closedformhullwhiteswaptionpricer", "sigma") => {
                    if !row[1].is_blank() {
                        sigma = Some(row[1].number(name)?);
                    }
                }
                _ => {
                    return Err(format!(
                        "setting '{name}' is not supported by {implementation}"
                    ))
                }
            }
        }
        let mut pricers = HashMap::new();
        macro_rules! register {
            ($pricer:expr) => {{
                let mut pricer = $pricer;
                if let Some(policy) = policy {
                    pricer.set_discount_policy(Box::new(policy.clone()));
                }
                register(&mut pricers, pricer);
            }};
        }
        match key.as_str() {
            "loanpricer" => {
                let mut pricer = LoanPricer::new().with_fair_rate_amortization_rebuild(rebuild);
                if let Some(index) = refi {
                    pricer = pricer.with_refinancing_benchmark_index(index);
                }
                register!(pricer);
            }
            "discountedcashflowpricer" => {
                register!(DiscountedCashflowPricer::<Swap<DualFwd>, SwapTrade<DualFwd>>::new());
                register!(DiscountedCashflowPricer::<
                    BasisSwap<DualFwd>,
                    BasisSwapTrade<DualFwd>,
                >::new());
                register!(DiscountedCashflowPricer::<
                    FloatFloatCrossCurrencySwap<DualFwd>,
                    FloatFloatCrossCurrencySwapTrade<DualFwd>,
                >::new());
                register!(DiscountedCashflowPricer::<
                    FixFloatCrossCurrencySwap<DualFwd>,
                    FixFloatCrossCurrencySwapTrade<DualFwd>,
                >::new());
                register!(DiscountedCashflowPricer::<
                    FloatingRateNote<DualFwd>,
                    FloatingRateNoteTrade<DualFwd>,
                >::new());
                register!(DiscountedCashflowPricer::<
                    FixedRateDeposit<DualFwd>,
                    FixedRateDepositTrade<DualFwd>,
                >::new());
            }
            "bondpricer" => register!(BondPricer::new()),
            "fxforwardpricer" => register!(FxForwardPricer::new()),
            "fxeuropeanoptionpricer" => register!(FxEuropeanOptionPricer::new()),
            "blackeuropeanoptionpricer" => register!(BlackEuropeanOptionPricer::new()),
            "equityforwardpricer" => register!(EquityForwardPricer::new()),
            "cdspricer" => register!(CdsPricer::new()),
            "ratefuturespricer" => register(&mut pricers, RateFuturesPricer::new()),
            "closedformblackcappricer" => register!(ClosedFormBlackCapPricer::new()),
            "closedformblackcapletpricer" => register!(ClosedFormBlackCapletPricer::new()),
            "closedformhullwhiteswaptionpricer" => {
                let alpha = alpha.ok_or("Hull-White pricer requires an Alpha setting")?;
                let sigma = sigma.ok_or("Hull-White pricer requires a Sigma setting")?;
                if alpha <= 0.0 || sigma < 0.0 {
                    return Err("Alpha must be positive and Sigma nonnegative".into());
                }
                register!(ClosedFormHullWhiteSwaptionPricer::new(alpha, sigma));
            }
            _ => return Err(format!("unknown native pricer '{implementation}'")),
        }
        Ok(Self {
            native: Evaluator::new(pricers),
        })
    }

    pub fn evaluate(
        &self,
        trade: &QsObject,
        requests: &[Request],
        context: &PricingContext,
    ) -> Result<EvaluationResults, String> {
        let native: &dyn Any = match trade {
            QsObject::LoanTrade(trade) => trade.as_ref(),
            QsObject::SwapTrade(trade) => trade.as_ref(),
            QsObject::BasisSwapTrade(trade) => trade.as_ref(),
            QsObject::FloatFloatCrossCurrencySwapTrade(trade) => trade.as_ref(),
            QsObject::FixFloatCrossCurrencySwapTrade(trade) => trade.as_ref(),
            QsObject::FixedRateBondTrade(trade) => trade.as_ref(),
            QsObject::FixedRateDepositTrade(trade) => trade.as_ref(),
            QsObject::FloatingRateNoteTrade(trade) => trade.as_ref(),
            QsObject::FxForwardTrade(trade) => trade.as_ref(),
            QsObject::FxEuropeanOptionTrade(trade) => trade.as_ref(),
            QsObject::EquityEuropeanOptionTrade(trade) => trade.as_ref(),
            QsObject::EquityForwardTrade(trade) => trade.as_ref(),
            QsObject::CdsTrade(trade) => trade.as_ref(),
            QsObject::CapFloorTrade(trade) => trade.as_ref(),
            QsObject::CapletFloorletTrade(trade) => trade.as_ref(),
            QsObject::RateFuturesTrade(trade) => trade.as_ref(),
            QsObject::EuropeanSwaptionTrade(trade) => trade.as_ref(),
            other => {
                return Err(format!(
                    "evaluate requires a trade, received {}",
                    other.kind()
                ))
            }
        };
        self.native
            .evaluate(native, requests, context)
            .map_err(|error| error.to_string())
    }
}

/// Computes values without retaining EvaluationResults objects in the registry.
/// Excel owns the returned scalar/array cache and its dependency scheduling.
pub fn evaluate_values(
    registry: &super::registry::Registry,
    pricer: &str,
    trades: &[String],
    context: &str,
) -> Result<Vec<f64>, String> {
    if trades.is_empty() {
        return Err("at least one trade is required".into());
    }
    let pricer = registry.pricer(pricer).map_err(|error| error.to_string())?;
    let context = registry
        .pricing_context(context)
        .map_err(|error| error.to_string())?;
    // Validate all handles before beginning an expensive portfolio calculation.
    let trades = trades
        .iter()
        .map(|trade| registry.object(trade).map_err(|error| error.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    trades
        .into_iter()
        .map(|trade| {
            pricer
                .evaluate(trade, &[Request::Value], context)?
                .price()
                .ok_or_else(|| "native pricer did not supply Value".into())
        })
        .collect()
}

fn register<P: Pricer + 'static>(pricers: &mut HashMap<TypeId, Box<dyn ErasedPricer>>, pricer: P)
where
    P::Item: 'static,
{
    pricers.insert(TypeId::of::<P::Item>(), Box::new(pricer));
}

/// Parses native request names from worksheet cells. FairRate always carries its target.
pub fn parse_requests(names: &[String]) -> Result<Vec<Request>, String> {
    if names.is_empty() {
        return Err("at least one request is required".into());
    }
    let mut requests = Vec::with_capacity(names.len());
    let mut fair_rate = false;
    for name in names {
        let normalized = name.trim().to_ascii_lowercase().replace('_', "");
        let request = match normalized.as_str() {
            "value" => Request::Value,
            "yieldtomaturity" => Request::YieldToMaturity,
            "modifiedduration" => Request::ModifiedDuration,
            "zspread" => Request::ZSpread,
            "oas" => Request::OAS,
            "sensitivities" => Request::Sensitivities,
            "cashflows" => Request::Cashflows,
            _ => {
                let index = normalized.strip_prefix("fairrate(").and_then(|s| s.strip_suffix(')'))
                    .and_then(|s| s.trim().parse::<usize>().ok())
                    .ok_or_else(|| format!("unknown request '{name}'; use FairRate(index) or QS.REQUEST.FAIR.RATE(index)"))?;
                if fair_rate {
                    return Err("request one fair-rate target per evaluation".into());
                }
                fair_rate = true;
                Request::FairRate(index)
            }
        };
        requests.push(request);
    }
    Ok(requests)
}

#[cfg(test)]
mod tests {
    use super::super::products::{InstrumentObject, NativeInstrument, PositionTerms};
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    fn context() -> PricingContext {
        let date = Date::new(2026, 2, 24);
        let mut elements = ConstructedElementStore::default();
        for (index, rate) in [
            (MarketIndex::ESTR, 0.03),
            (MarketIndex::EURIBOR12m, 0.04),
            (MarketIndex::Other("FUNDING".into()), 0.06),
        ] {
            let curve = FlatForwardTermStructure::new(
                date,
                DualFwd::from(rate),
                RateDefinition::new(
                    DayCounter::Actual360,
                    Compounding::Continuous,
                    Frequency::Annual,
                ),
            )
            .with_pillar_label(index.to_string());
            elements.discount_curves_mut().insert(
                index.clone(),
                DiscountCurveElement::new(index, Rc::new(RefCell::new(curve))),
            );
        }
        let mut context = PricingContext::new()
            .with_quote_store(QuoteStore::new(date))
            .with_base_currency(Currency::EUR)
            .with_base_index(MarketIndex::ESTR)
            .with_constructed_elements(elements);
        context.initialize().unwrap();
        context
    }
    fn policy(index: &str) -> PolicyObject {
        fixed_income_policy(
            &vec![vec![Cell::Text("EUR".into()), Cell::Text(index.into())]],
            false,
        )
        .unwrap()
    }

    #[test]
    fn swap_targets_reprice_both_legs_and_positions_preserve_contract() {
        let context = context();
        let date = Date::new(2026, 2, 24);
        let make = |fixed, spread| {
            MakeSwap::<DualFwd>::default()
                .with_identifier("swap".into())
                .with_start_date(date)
                .with_maturity_date(Date::new(2031, 2, 24))
                .with_currency(Currency::EUR)
                .with_market_index(MarketIndex::EURIBOR12m)
                .with_notional(100_000.0)
                .with_fixed_rate(fixed)
                .with_spread(spread)
                .with_rate_definition(RateDefinition::new(
                    DayCounter::Actual360,
                    Compounding::Simple,
                    Frequency::Annual,
                ))
                .with_fixed_leg_frequency(Frequency::Semiannual)
                .with_floating_leg_frequency(Frequency::Quarterly)
                .with_side(Side::LongReceive)
                .build()
                .unwrap()
        };
        let contract = InstrumentObject {
            native: NativeInstrument::Swap(make(0.05, 0.01)),
            notional: 100_000.0,
            foreign_notional: None,
        };
        let csa = PolicyObject {
            native: Arc::new(SingleCurveCSADiscountPolicy::new(
                MarketIndex::Other("FUNDING".into()),
                Currency::EUR,
            )),
            kind: "SingleCurveCSADiscountPolicy",
        };
        let pricer = PricerObject::new("DiscountedCashflowPricer", Some(&csa), &vec![]).unwrap();
        let trade = contract
            .trade(date, Side::LongReceive, &PositionTerms::default())
            .unwrap();
        let pv = pricer
            .evaluate(&trade, &[Request::Value], &context)
            .unwrap()
            .price()
            .unwrap();
        for target in [0, 1] {
            let fair = pricer
                .evaluate(&trade, &[Request::FairRate(target)], &context)
                .unwrap()
                .fair_rate()
                .unwrap();
            let candidate = InstrumentObject {
                native: NativeInstrument::Swap(if target == 0 {
                    make(fair, 0.01)
                } else {
                    make(0.05, fair)
                }),
                notional: 100_000.0,
                foreign_notional: None,
            };
            let candidate = candidate
                .trade(date, Side::LongReceive, &PositionTerms::default())
                .unwrap();
            assert!(
                pricer
                    .evaluate(&candidate, &[Request::Value], &context)
                    .unwrap()
                    .price()
                    .unwrap()
                    .abs()
                    < 1e-8
            );
        }
        assert!(pricer
            .evaluate(&trade, &[Request::FairRate(2)], &context)
            .err()
            .unwrap()
            .to_string()
            .contains("out of range"));
        let reverse = contract
            .trade(
                date,
                Side::PayShort,
                &PositionTerms {
                    notional: Some(200_000.0),
                    ..PositionTerms::default()
                },
            )
            .unwrap();
        assert!(
            (pricer
                .evaluate(&reverse, &[Request::Value], &context)
                .unwrap()
                .price()
                .unwrap()
                + 2.0 * pv)
                .abs()
                < 1e-8
        );
        assert_eq!(
            pricer
                .evaluate(&trade, &[Request::Value], &context)
                .unwrap()
                .price(),
            Some(pv)
        );
    }

    #[test]
    fn nonlinear_daily_spread_is_rejected_instead_of_returning_a_linear_approximation() {
        let context = context();
        let date = Date::new(2026, 2, 24);
        let end = Date::new(2026, 5, 24);
        let swap = MakeSwap::<DualFwd>::default()
            .with_identifier("ois".into())
            .with_start_date(date)
            .with_maturity_date(end)
            .with_currency(Currency::EUR)
            .with_market_index(MarketIndex::ESTR)
            .with_notional(100_000.0)
            .with_fixed_rate(0.04)
            .with_fixed_leg_frequency(Frequency::Quarterly)
            .with_floating_leg_frequency(Frequency::Quarterly)
            .with_side(Side::LongReceive)
            .with_rate_definition(RateDefinition::default())
            .build()
            .unwrap();
        let mut coupon = FloatingRateCoupon::new(
            100_000.0,
            DualFwd::from(0.01),
            MarketIndex::ESTR,
            date,
            end,
            end,
        );
        coupon
            .set_overnight_conventions(
                OvernightCouponConventions::new().with_compound_spread_daily(true),
            )
            .unwrap();
        let floating = Leg::new(
            1,
            vec![CashflowType::FloatingRateCoupon(coupon)],
            Currency::EUR,
            None,
            Some(MarketIndex::ESTR),
            None,
            None,
            Side::PayShort,
            true,
            AssetClass::InterestRate,
            date,
            end,
        );
        let swap = Swap::new(
            "ois".into(),
            swap.fixed_leg().clone(),
            floating,
            MarketIndex::ESTR,
            Currency::EUR,
        );
        let trade = QsObject::SwapTrade(Box::new(SwapTrade::new(
            swap,
            date,
            100_000.0,
            Side::LongReceive,
        )));
        let policy = PolicyObject {
            native: Arc::new(SingleCurveCSADiscountPolicy::new(
                MarketIndex::ESTR,
                Currency::EUR,
            )),
            kind: "SingleCurveCSADiscountPolicy",
        };
        let pricer = PricerObject::new("DiscountedCashflowPricer", Some(&policy), &vec![]).unwrap();
        assert!(pricer
            .evaluate(&trade, &[Request::FairRate(0)], &context)
            .is_ok());
        assert!(pricer
            .evaluate(&trade, &[Request::FairRate(1)], &context)
            .err()
            .unwrap()
            .contains("daily spread compounding"));
    }

    #[test]
    fn ordinary_curve_policy_changes_loan_funding_without_changing_contract() {
        let context = context();
        let date = Date::new(2026, 2, 24);
        let loan = MakeLoan::<DualFwd>::default()
            .with_identifier("loan".into())
            .with_start_date(date)
            .with_maturity_date(Date::new(2031, 2, 24))
            .with_currency(Currency::EUR)
            .with_notional(100_000.0)
            .with_coupons(
                vec![CashflowType::fixed(0.05); 12]
                    .into_iter()
                    .chain([CashflowType::fixed(0.06)])
                    .collect(),
            )
            .with_payment_frequency(Frequency::Monthly)
            .with_day_counter(DayCounter::Actual360)
            .build()
            .unwrap();
        let contract = InstrumentObject {
            native: NativeInstrument::Loan(loan),
            notional: 100_000.0,
            foreign_notional: None,
        };
        let trade = contract
            .trade(date, Side::LongReceive, &PositionTerms::default())
            .unwrap();
        let risk_free = PricerObject::new("LoanPricer", Some(&policy("ESTR")), &vec![]).unwrap();
        let funded = PricerObject::new("LoanPricer", Some(&policy("FUNDING")), &vec![]).unwrap();
        let first = risk_free
            .evaluate(
                &trade,
                &[Request::Value, Request::Cashflows, Request::FairRate(1)],
                &context,
            )
            .unwrap();
        let second = funded
            .evaluate(
                &trade,
                &[Request::Value, Request::Cashflows, Request::FairRate(1)],
                &context,
            )
            .unwrap();
        assert!(first.price().unwrap() > second.price().unwrap());
        assert!(first.fair_rate().unwrap() < second.fair_rate().unwrap());
        assert_eq!(
            first.cashflows().unwrap().amounts(),
            second.cashflows().unwrap().amounts()
        );
        assert!(contract
            .trade(
                date,
                Side::LongReceive,
                &PositionTerms {
                    notional: Some(200_000.0),
                    ..PositionTerms::default()
                }
            )
            .is_err());
        assert!(PricerObject::new("LoanPricer", None, &vec![]).is_err());
        assert!(
            PricerObject::new("BondPricer", Some(&policy("ESTR")), &vec![])
                .unwrap()
                .evaluate(&trade, &[Request::Value], &context)
                .is_err()
        );
        assert!(PricerObject::new(
            "LoanPricer",
            Some(&policy("ESTR")),
            &vec![vec![
                Cell::Text("Discount index".into()),
                Cell::Text("FUNDING".into())
            ]]
        )
        .is_err());
    }

    #[test]
    fn bond_anchor_is_a_position_term_and_policy_is_a_pricer_term() {
        let context = context();
        let date = Date::new(2026, 2, 24);
        let bond = MakeFixedRateBond::<DualFwd>::default()
            .with_identifier("bond".into())
            .with_start_date(date)
            .with_maturity_date(Date::new(2031, 2, 24))
            .with_currency(Currency::EUR)
            .with_notional(100_000.0)
            .with_rate_definition(RateDefinition::default())
            .with_rate(0.05)
            .with_payment_frequency(Frequency::Semiannual)
            .build()
            .unwrap();
        let contract = InstrumentObject {
            native: NativeInstrument::FixedRateBond(bond),
            notional: 100_000.0,
            foreign_notional: None,
        };
        assert!(contract
            .trade(date, Side::LongReceive, &PositionTerms::default())
            .is_err());
        let terms = PositionTerms {
            pricing_anchor: Some(BondPricingAnchor::ZSpread(0.0)),
            ..PositionTerms::default()
        };
        let trade = contract.trade(date, Side::LongReceive, &terms).unwrap();
        let pricer = PricerObject::new("BondPricer", Some(&policy("ESTR")), &vec![]).unwrap();
        let original = pricer
            .evaluate(&trade, &[Request::Value], &context)
            .unwrap()
            .price()
            .unwrap();
        let reverse = contract
            .trade(
                date,
                Side::PayShort,
                &PositionTerms {
                    notional: Some(200_000.0),
                    ..terms
                },
            )
            .unwrap();
        assert!(
            (pricer
                .evaluate(&reverse, &[Request::Value], &context)
                .unwrap()
                .price()
                .unwrap()
                + 2.0 * original)
                .abs()
                < 1e-8
        );
    }

    #[test]
    fn requests_and_policy_mappings_reject_ambiguous_inputs() {
        assert_eq!(
            parse_requests(&["Value".into(), "FairRate(1)".into()]).unwrap(),
            vec![Request::Value, Request::FairRate(1)]
        );
        for request in ["FairRate", "FairRate(-1)", "FairRate(1.5)", "Missing"] {
            assert!(parse_requests(&[request.into()]).is_err());
        }
        assert!(parse_requests(&["FairRate(0)".into(), "FairRate(1)".into()]).is_err());
        let collateral = fixed_income_policy(
            &vec![vec![
                Cell::Text("EUR".into()),
                Cell::Text("Collateral:EUR/USD".into()),
            ]],
            false,
        )
        .unwrap();
        assert_eq!(
            collateral.discount_indices(),
            vec![MarketIndex::Collateral(Currency::EUR, Currency::USD)]
        );
        assert!(fixed_income_policy(&vec![], false).is_err());
        assert!(fixed_income_policy(
            &vec![vec![Cell::Text("EUR".into()), Cell::Text("ESTR".into())]; 2],
            false
        )
        .is_err());
    }

    #[test]
    fn ten_thousand_positions_return_values_without_storing_ten_thousand_results() {
        use super::super::registry::{CellId, Registry};
        let started = std::time::Instant::now();
        let mut registry = Registry::default();
        let cell = |row, column| CellId {
            sheet: 100,
            row,
            column,
        };
        registry.begin_cell(cell(0, 0));
        let context_handle = registry
            .upsert_if_changed(
                "context",
                QsObject::PricingContext(std::rc::Rc::new(context())),
                "context",
            )
            .unwrap();
        registry.finish_cell(true);
        registry.begin_cell(cell(0, 1));
        let policy = policy("ESTR");
        let pricer = PricerObject::new("DiscountedCashflowPricer", Some(&policy), &vec![]).unwrap();
        let pricer_handle = registry
            .upsert_if_changed("pricer", QsObject::Pricer(Box::new(pricer)), "pricer")
            .unwrap();
        registry.finish_cell(true);
        let date = Date::new(2026, 2, 24);
        let instrument = InstrumentObject {
            native: NativeInstrument::FixedRateDeposit(
                MakeFixedRateDeposit::<DualFwd>::default()
                    .with_identifier("portfolio deposit".into())
                    .with_start_date(date)
                    .with_maturity_date(Date::new(2027, 2, 24))
                    .with_currency(Currency::EUR)
                    .with_notional(1000.0)
                    .with_rate(0.04)
                    .with_rate_definition(RateDefinition::new(
                        DayCounter::Actual360,
                        Compounding::Simple,
                        Frequency::Annual,
                    ))
                    .build()
                    .unwrap(),
            ),
            notional: 1000.0,
            foreign_notional: None,
        };
        let mut trades = Vec::new();
        for row in 1..=10_000 {
            registry.begin_cell(cell(row, 0));
            let trade = instrument
                .trade(date, Side::LongReceive, &PositionTerms::default())
                .unwrap();
            trades.push(
                registry
                    .upsert_if_changed(&format!("trade{row}"), trade, "trade")
                    .unwrap(),
            );
            registry.finish_cell(true);
        }
        registry.begin_cell(cell(0, 2));
        let values = evaluate_values(&registry, &pricer_handle, &trades, &context_handle).unwrap();
        registry.finish_cell(true);
        assert_eq!(values.len(), 10_000);
        assert!(values
            .iter()
            .all(|value| value.is_finite() && (*value - values[0]).abs() < 1e-10));
        assert_eq!(
            registry.len(),
            10_002,
            "no EvaluationResults object is retained"
        );
        use quantsupport::ad::{forward::ADForward, tape::TapeHolder};
        let nodes = ADForward::with_tape(|tape| tape.book.len());
        registry.begin_cell(cell(0, 2));
        let repeated =
            evaluate_values(&registry, &pricer_handle, &trades, &context_handle).unwrap();
        registry.finish_cell(true);
        assert_eq!(values, repeated);
        assert_eq!(
            ADForward::with_tape(|tape| tape.book.len()),
            nodes,
            "the AD tape does not accumulate per position"
        );
        for row in 1..=10_000 {
            registry.release_cell(cell(row, 0));
        }
        registry.release_cell(cell(0, 2));
        assert_eq!(registry.len(), 2);
        registry.release_cell(cell(0, 0));
        registry.release_cell(cell(0, 1));
        assert_eq!(registry.len(), 0);
        eprintln!(
            "10,000 native positions, two valuation passes and collection: {:?}",
            started.elapsed()
        );
    }
}
