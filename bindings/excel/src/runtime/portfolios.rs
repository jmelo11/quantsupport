//! Worksheet portfolio membership and streaming valuation.

use super::{
    pricing::PricerObject,
    registry::{QsObject, Registry},
};
use quantsupport::{
    prelude::{
        Currency, EvaluationResults, MarketDataProvider, MarketDataRequest, Portfolio,
        PortfolioPricer, Pricer, QSError, Request,
    },
    utils::errors::Result as QsResult,
};
use std::marker::PhantomData;

pub fn create(
    registry: &Registry,
    identifier: String,
    trades: Vec<String>,
) -> Result<Portfolio<String>, String> {
    for handle in &trades {
        let object = registry.object(handle).map_err(|error| error.to_string())?;
        if !object.kind().ends_with("Trade") {
            return Err(format!(
                "Portfolio members must be trades, received {}",
                object.kind()
            ));
        }
    }
    Portfolio::new(identifier, trades).map_err(|error| error.to_string())
}

fn validate_count(count: usize, trades: usize, label: &str) -> Result<(), String> {
    if count != 1 && count != trades {
        return Err(format!(
            "{label} must contain one value for all trades, or one value per trade ({trades})"
        ));
    }
    Ok(())
}

pub fn evaluate(
    registry: &Registry,
    portfolio: &str,
    pricers: &[String],
    context: &str,
    requests: &[Request],
    value_currency: Option<Currency>,
) -> Result<EvaluationResults, String> {
    let portfolio = registry
        .portfolio(portfolio)
        .map_err(|error| error.to_string())?;
    let count = portfolio.trades().len();
    validate_count(pricers.len(), count, "pricers")?;
    // Check every member and pricer before doing any expensive valuation.
    for trade in portfolio.trades() {
        registry.object(trade).map_err(|error| error.to_string())?;
    }
    for pricer in pricers {
        registry.pricer(pricer).map_err(|error| error.to_string())?;
    }
    let context = super::context::resolve(registry, context)?;
    if value_currency.is_some_and(|currency| currency != context.base_currency()) {
        return Err("Portfolio value currency must match the context base currency".into());
    }
    let members = portfolio
        .trades()
        .iter()
        .enumerate()
        .map(|(index, trade)| {
            let object = registry.object(trade).map_err(|error| error.to_string())?;
            let position = object;
            let pricer_index = if pricers.len() == 1 { 0 } else { index };
            let pricer = registry
                .pricer(&pricers[pricer_index])
                .map_err(|error| error.to_string())?;
            Ok(PortfolioMember { position, pricer })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let portfolio = Portfolio::new(portfolio.identifier().to_owned(), members)
        .map_err(|error| error.to_string())?;
    PortfolioPricer::new(PortfolioMemberPricer(PhantomData))
        .evaluate(&portfolio, requests, context.as_ref())
        .map_err(|error| error.to_string())
}

/// Associates a worksheet trade with its explicitly selected native pricer.
struct PortfolioMember<'a> {
    position: &'a QsObject,
    pricer: &'a PricerObject,
}

/// Dispatches borrowed worksheet positions through their selected member pricers.
struct PortfolioMemberPricer<'a>(PhantomData<&'a ()>);

impl<'a> Pricer for PortfolioMemberPricer<'a> {
    type Item = PortfolioMember<'a>;
    type Policy = ();

    fn evaluate(
        &self,
        trade: &Self::Item,
        requests: &[Request],
        context: &impl MarketDataProvider,
    ) -> QsResult<EvaluationResults> {
        trade
            .pricer
            .evaluate(trade.position, requests, context)
            .map_err(QSError::InvalidValueErr)
    }
    fn market_data_request(&self, _trade: &Self::Item) -> Option<MarketDataRequest> {
        None
    }
    fn set_discount_policy(&mut self, _policy: Box<()>) {}
    fn discount_policy(&self) -> Option<&()> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        inputs::Cell,
        pricing::{fixed_income_policy, PricerObject},
        registry::{CellId, QsObject},
    };
    use super::*;
    use quantsupport::{ad::tape::TapeHolder, prelude::*};
    use std::{cell::RefCell, rc::Rc};

    fn setup() -> (Registry, String, String, String) {
        let date = Date::new(2026, 2, 24);
        let curve = FlatForwardTermStructure::new(
            date,
            DualFwd::from(0.03),
            RateDefinition::new(
                DayCounter::Actual360,
                Compounding::Continuous,
                Frequency::Annual,
            ),
        )
        .with_pillar_label("SOFR.flat".into());
        let mut elements = ConstructedElementStore::default();
        elements.discount_curves_mut().insert(
            MarketIndex::SOFR,
            DiscountCurveElement::new(MarketIndex::SOFR, Rc::new(RefCell::new(curve))),
        );
        let context = PricingContext::new()
            .with_quote_store(QuoteStore::new(date))
            .with_constructed_elements(elements);
        let policy = fixed_income_policy(
            &vec![vec![Cell::Text("USD".into()), Cell::Text("SOFR".into())]],
            false,
        )
        .unwrap();
        let pricer = PricerObject::new("DiscountedCashflowPricer", Some(&policy), &vec![]).unwrap();
        let deposit = MakeFixedRateDeposit::<DualFwd>::default()
            .with_identifier("Deposit".into())
            .with_start_date(date)
            .with_maturity_date(Date::new(2027, 2, 24))
            .with_notional(1000.0)
            .with_rate(0.04)
            .with_currency(Currency::USD)
            .with_rate_definition(RateDefinition::new(
                DayCounter::Actual360,
                Compounding::Simple,
                Frequency::Annual,
            ))
            .build()
            .unwrap();
        let trade = FixedRateDepositTrade::new(deposit, date, 1000.0, Side::LongReceive);
        let mut registry = Registry::default();
        let context = registry
            .upsert_if_changed(
                "context",
                QsObject::PricingContext(std::rc::Rc::new(context)),
                "ctx",
            )
            .unwrap();
        let pricer = registry
            .upsert_if_changed("pricer", QsObject::Pricer(Box::new(pricer)), "pricer")
            .unwrap();
        registry.begin_cell(CellId {
            sheet: 1,
            row: 0,
            column: 0,
        });
        let trade = registry
            .upsert_if_changed(
                "deposit",
                QsObject::FixedRateDepositTrade(Box::new(trade)),
                "trade",
            )
            .unwrap();
        registry.finish_cell(true);
        (registry, context, pricer, trade)
    }

    #[test]
    fn deposits_aggregate_standard_results_with_bounded_ad_storage() {
        let (mut registry, context, pricer, trade) = setup();
        let requests = [Request::Value, Request::Sensitivities, Request::Cashflows];
        let single = registry
            .pricer(&pricer)
            .unwrap()
            .evaluate(
                registry.object(&trade).unwrap(),
                &requests,
                registry.pricing_context(&context).unwrap(),
            )
            .unwrap();
        let nodes = ADForward::with_tape(|tape| tape.book.len());
        let members = 2000;
        let portfolio = create(&registry, "TEST-PORTFOLIO".into(), vec![trade; members]).unwrap();
        let handle = registry
            .upsert_if_changed(
                "portfolio",
                QsObject::Portfolio(Box::new(portfolio)),
                "members",
            )
            .unwrap();
        let count = registry.len();
        let result: EvaluationResults = evaluate(
            &registry,
            &handle,
            &[pricer],
            &context,
            &requests,
            Some(Currency::USD),
        )
        .unwrap();
        assert_eq!(
            registry.len(),
            count,
            "member results must not accumulate in the registry"
        );
        assert!(
            ADForward::with_tape(|tape| tape.book.len()) <= nodes,
            "AD graph must remain bounded by one trade"
        );
        assert!((result.price().unwrap() - single.price().unwrap() * members as f64).abs() < 1e-7);
        let expected = single.sensitivities().unwrap();
        let actual = result.sensitivities().unwrap();
        assert_eq!(actual.instrument_keys().len(), 1);
        assert!((actual.exposure()[0] - expected.exposure()[0] * members as f64).abs() < 1e-7);
        let flows = result.cashflows().unwrap();
        assert_eq!(flows.payment_dates().len(), 3);
        let coupon = flows.coupon_details().iter().flatten().next().unwrap();
        assert_eq!(coupon.notional(), 1000.0 * members as f64);
        let expected_rate = single
            .cashflows()
            .unwrap()
            .coupon_details()
            .iter()
            .flatten()
            .next()
            .unwrap()
            .rate()
            .unwrap();
        assert!((coupon.rate().unwrap() - expected_rate).abs() < 1e-14);
        let rows = summarize_cashflows(flows).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].capital(), -1000.0 * members as f64);
        assert_eq!(rows[1].capital(), 1000.0 * members as f64);
        assert!(rows[1].interest() > 0.0);
    }

    #[test]
    fn membership_retains_trades_and_rejects_invalid_pricer_lists() {
        let (mut registry, context, pricer, trade) = setup();
        let cell = CellId {
            sheet: 1,
            row: 1,
            column: 0,
        };
        registry.begin_cell(cell);
        let portfolio = create(
            &registry,
            "TEST-PORTFOLIO".into(),
            vec![trade.clone(), trade.clone()],
        )
        .unwrap();
        let handle = registry
            .upsert_if_changed(
                "portfolio",
                QsObject::Portfolio(Box::new(portfolio)),
                "members",
            )
            .unwrap();
        registry.finish_cell(true);
        registry.release_cell(CellId {
            sheet: 1,
            row: 0,
            column: 0,
        });
        assert!(registry.object(&trade).is_ok());
        assert!(create(&registry, "TEST-PORTFOLIO".into(), vec![context.clone()]).is_err());
        assert!(create(&registry, "TEST-PORTFOLIO".into(), vec![]).is_err());
        assert!(evaluate(
            &registry,
            &handle,
            &[],
            &context,
            &[Request::Value],
            Some(Currency::USD)
        )
        .is_err());
        assert!(evaluate(
            &registry,
            &handle,
            &vec![pricer.clone(); 3],
            &context,
            &[Request::Value],
            Some(Currency::USD)
        )
        .is_err());
        let result = evaluate(
            &registry,
            &handle,
            &[pricer],
            &context,
            &[Request::FairRate(0)],
            None,
        )
        .unwrap();
        assert!(result.fair_rate().is_none());
        registry.release_cell(cell);
        assert!(registry.object(&trade).is_err());
    }
}
