//! Native loan construction and valuation shared by worksheet boundaries and tests.

use quantsupport::prelude::{
    BusinessDayConvention, Calendar, CashflowType, Currency, Date, DayCounter, DualFwd,
    EvaluationResults, FixedIncomeDiscountPolicy, Frequency, LoanPricer, LoanRepaymentStructure,
    LoanTrade, MakeLoan, MakeSchedule, MarketIndex, Period, PrepaymentModelConfiguration, Pricer,
    PricingContext, Request, Side,
};
use serde::{Deserialize, Serialize};
use std::result::Result;

/// Optional contract and pricing settings supplied as worksheet JSON text.
#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct LoanOptions {
    pub balloon_fraction: Option<f64>,
    pub prepayment: Option<PrepaymentModelConfiguration>,
    pub credit_index: Option<MarketIndex>,
    pub recovery: Option<f64>,
    pub amortization_rate: Option<f64>,
    pub fixing_frequency: Option<Frequency>,
    pub fixing_lag: Option<i32>,
    pub index_tenor: Option<Period>,
    pub calendar: Option<Calendar>,
    pub business_day_convention: Option<BusinessDayConvention>,
    pub floor: Option<f64>,
    pub cap: Option<f64>,
    pub introductory_periods: Option<usize>,
    pub introductory_rate: Option<f64>,
    pub refinancing_benchmark_index: Option<MarketIndex>,
    pub discount_index: Option<MarketIndex>,
    pub rebuild_amortization_for_fair_rate: bool,
}

/// Contractual and position inputs; rates are annual decimal coupon/spread values.
#[derive(Serialize)]
pub struct LoanTerms {
    pub trade_date: Date,
    pub start_date: Date,
    pub maturity_date: Date,
    pub currency: Currency,
    pub notional: f64,
    pub rate: f64,
    pub market_index: Option<MarketIndex>,
    pub repayment: String,
    pub payment_frequency: Frequency,
    pub day_counter: DayCounter,
    #[serde(serialize_with = "serialize_side")]
    pub side: Side,
    pub options: LoanOptions,
}

fn serialize_side<S: serde::Serializer>(side: &Side, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&format!("{side:?}"))
}

/// A native loan trade and its explicit pricer settings.
pub struct LoanPosition {
    pub trade: LoanTrade<DualFwd>,
    options: LoanOptions,
}

impl LoanTerms {
    pub fn build(self, name: &str) -> Result<LoanPosition, String> {
        let options = &self.options;
        let repayment = match self.repayment.trim().to_ascii_lowercase().as_str() {
            "french" => LoanRepaymentStructure::French,
            "bullet" => LoanRepaymentStructure::Bullet,
            "balloon" => LoanRepaymentStructure::Balloon {
                residual_fraction: options
                    .balloon_fraction
                    .ok_or("Balloon requires options.balloon_fraction")?,
            },
            _ => return Err("repayment must be French, Bullet, or Balloon".into()),
        };
        if options.balloon_fraction.is_some()
            && !matches!(repayment, LoanRepaymentStructure::Balloon { .. })
        {
            return Err("balloon_fraction requires Balloon repayment".into());
        }
        if self.market_index.is_none()
            && (options.floor.is_some()
                || options.cap.is_some()
                || options.introductory_rate.is_some()
                || options.introductory_periods.is_some())
        {
            return Err(
                "coupon bounds and introductory periods require a floating market_index".into(),
            );
        }
        let coupon = self.market_index.map_or_else(
            || CashflowType::fixed(self.rate),
            |index| CashflowType::floating(index, self.rate, options.floor, options.cap),
        );
        let coupons =
            match (options.introductory_periods, options.introductory_rate) {
                (None, None) => vec![coupon],
                (Some(periods), Some(rate)) if periods > 0 => {
                    // Bound allocation by the actual contractual schedule before creating templates.
                    let schedule = MakeSchedule::new(self.start_date, self.maturity_date)
                        .with_frequency(self.payment_frequency)
                        .build()
                        .map_err(|error| error.to_string())?;
                    if periods >= schedule.dates().len().saturating_sub(1) {
                        return Err(
                            "introductory_periods must leave at least one floating payment".into(),
                        );
                    }
                    let mut coupons = vec![CashflowType::fixed(rate); periods];
                    coupons.push(coupon);
                    coupons
                }
                _ => return Err(
                    "introductory_rate and positive introductory_periods must be supplied together"
                        .into(),
                ),
            };
        let mut builder = MakeLoan::default()
            .with_identifier(name.to_string())
            .with_start_date(self.start_date)
            .with_maturity_date(self.maturity_date)
            .with_currency(self.currency)
            .with_notional(self.notional)
            .with_coupons(coupons)
            .with_repayment_structure(repayment)
            .with_payment_frequency(self.payment_frequency)
            .with_day_counter(self.day_counter);
        if let Some(value) = &options.prepayment {
            builder = builder.with_prepayment_configuration(value.clone());
        }
        if let Some(value) = &options.credit_index {
            builder = builder.with_credit_curve_index(value.clone());
        }
        if let Some(value) = options.recovery {
            builder = builder.with_recovery_fraction(value);
        }
        if let Some(value) = options.amortization_rate {
            builder = builder.with_initial_amortization_rate(value);
        }
        if let Some(value) = options.fixing_frequency {
            builder = builder.with_coupon_reset_frequency(value);
        }
        if let Some(value) = options.fixing_lag {
            builder = builder.with_fixing_lag_business_days(value);
        }
        if let Some(value) = options.index_tenor {
            builder = builder.with_index_observation_tenor(value);
        }
        if let Some(value) = &options.calendar {
            builder = builder.with_calendar(value.clone());
        }
        if let Some(value) = options.business_day_convention {
            builder = builder.with_business_day_convention(value);
        }
        let loan = builder.build().map_err(|error| error.to_string())?;
        Ok(LoanPosition {
            trade: LoanTrade::new(loan, self.trade_date, self.side),
            options: self.options,
        })
    }
}

impl LoanPosition {
    pub fn evaluate(
        &self,
        context: &PricingContext,
        requests: &[Request],
    ) -> Result<EvaluationResults, String> {
        let mut pricer = LoanPricer::new()
            .with_fair_rate_amortization_rebuild(self.options.rebuild_amortization_for_fair_rate);
        if let Some(index) = &self.options.refinancing_benchmark_index {
            pricer = pricer.with_refinancing_benchmark_index(index.clone());
        }
        pricer.set_discount_policy(Box::new(
            FixedIncomeDiscountPolicy::new(true).with_risk_free_index(
                context.base_currency(),
                self.options
                    .discount_index
                    .as_ref()
                    .unwrap_or(context.base_index())
                    .clone(),
            ),
        ));
        pricer
            .evaluate(&self.trade, requests, context)
            .map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quantsupport::prelude::{
        Compounding, ConstructedElementStore, CreditCurveConfiguration, DiscountCurveElement,
        FlatForwardTermStructure, Quote, QuoteLevels, QuoteStore, RateDefinition,
    };
    use std::{cell::RefCell, rc::Rc};

    fn terms() -> LoanTerms {
        LoanTerms {
            trade_date: Date::new(2026, 2, 24),
            start_date: Date::new(2026, 2, 24),
            maturity_date: Date::new(2031, 2, 24),
            currency: Currency::EUR,
            notional: 250_000.0,
            rate: 0.06,
            market_index: None,
            repayment: "French".into(),
            payment_frequency: Frequency::Monthly,
            day_counter: DayCounter::Actual360,
            side: Side::LongReceive,
            options: LoanOptions::default(),
        }
    }

    fn context() -> PricingContext {
        let mut elements = ConstructedElementStore::default();
        for (index, rate) in [(MarketIndex::ESTR, 0.03), (MarketIndex::EURIBOR12m, 0.04)] {
            elements.discount_curves_mut().insert(
                index.clone(),
                DiscountCurveElement::new(
                    index,
                    Rc::new(RefCell::new(
                        FlatForwardTermStructure::new(
                            Date::new(2026, 2, 24),
                            DualFwd::from(rate),
                            RateDefinition::new(
                                DayCounter::Actual360,
                                Compounding::Continuous,
                                Frequency::Annual,
                            ),
                        )
                        .with_pillar_label("flat".into()),
                    )),
                ),
            );
        }
        let mut context = PricingContext::new()
            .with_quote_store(QuoteStore::new(Date::new(2026, 2, 24)))
            .with_base_currency(Currency::EUR)
            .with_base_index(MarketIndex::ESTR)
            .with_constructed_elements(elements);
        context.initialize().unwrap();
        context
    }

    #[test]
    fn prices_repayment_structures_and_retains_signed_principal() {
        let context = context();
        for structure in ["French", "Bullet", "Balloon"] {
            let mut input = terms();
            input.repayment = structure.into();
            if structure == "Balloon" {
                input.options.balloon_fraction = Some(0.25);
            }
            let loan = input.build("loan").unwrap();
            let results = loan
                .evaluate(
                    &context,
                    &[
                        Request::Value,
                        Request::Cashflows,
                        Request::FairRate,
                        Request::Sensitivities,
                    ],
                )
                .unwrap();
            assert!(results.price().unwrap() > 250_000.0);
            assert!(results.fair_rate().unwrap() > 0.0);
            assert!(!results
                .sensitivities()
                .unwrap()
                .instrument_keys()
                .is_empty());
            let principal: f64 = results
                .cashflows()
                .unwrap()
                .amounts()
                .iter()
                .zip(results.cashflows().unwrap().cashflow_types())
                .filter(|(_, kind)| kind.contains("Redemption"))
                .map(|(amount, _)| amount)
                .sum();
            assert!((principal - 250_000.0).abs() < 1e-6, "{principal}");
        }
        let long = terms()
            .build("long")
            .unwrap()
            .evaluate(&context, &[Request::Value])
            .unwrap()
            .price()
            .unwrap();
        let mut short = terms();
        short.side = Side::PayShort;
        assert!(
            (long
                + short
                    .build("short")
                    .unwrap()
                    .evaluate(&context, &[Request::Value])
                    .unwrap()
                    .price()
                    .unwrap())
            .abs()
                < 1e-6
        );
    }

    #[test]
    fn floating_prepayment_and_funding_options_reach_native_pricer() {
        let context = context();
        let mut floating = terms();
        floating.rate = 0.01;
        floating.market_index = Some(MarketIndex::EURIBOR12m);
        assert!(terms_with_floating_without_amortization()
            .build("missing")
            .is_err());
        floating.options.amortization_rate = Some(0.05);
        let base = floating
            .build("floating")
            .unwrap()
            .evaluate(&context, &[Request::Value, Request::FairRate])
            .unwrap();
        assert!(base.price().unwrap().is_finite());
        let mut prepaid = terms();
        prepaid.options.prepayment = Some(PrepaymentModelConfiguration::Cpr(0.15));
        let prepaid = prepaid
            .build("prepaid")
            .unwrap()
            .evaluate(&context, &[Request::Value])
            .unwrap()
            .price()
            .unwrap();
        let fixed = terms()
            .build("fixed")
            .unwrap()
            .evaluate(&context, &[Request::Value])
            .unwrap()
            .price()
            .unwrap();
        assert!(prepaid < fixed);
        let mut funded = terms();
        funded.options.discount_index = Some(MarketIndex::EURIBOR12m);
        assert!(
            funded
                .build("funded")
                .unwrap()
                .evaluate(&context, &[Request::Value])
                .unwrap()
                .price()
                .unwrap()
                < fixed
        );
    }

    fn terms_with_floating_without_amortization() -> LoanTerms {
        let mut value = terms();
        value.market_index = Some(MarketIndex::EURIBOR12m);
        value.rate = 0.01;
        value
    }

    #[test]
    fn rejects_ignored_or_incomplete_options() {
        assert!(serde_json::from_str::<LoanOptions>(r#"{"unknown":1}"#).is_err());
        let mut input = terms();
        input.options.balloon_fraction = Some(0.25);
        assert!(input.build("loan").is_err());
        let mut input = terms();
        input.repayment = "Balloon".into();
        assert!(input.build("loan").is_err());
        let mut input = terms();
        input.options.floor = Some(0.02);
        assert!(input.build("loan").is_err());
    }

    #[test]
    fn introductory_bounded_and_credit_loans_follow_native_cashflow_semantics() {
        let context = context();
        let mut bounded = terms_with_floating_without_amortization();
        bounded.options =
            serde_json::from_str(r#"{"amortization_rate":0.05,"floor":0.02,"cap":0.045}"#).unwrap();
        let bounded = bounded.build("bounded").unwrap();
        let results = bounded
            .evaluate(&context, &[Request::Value, Request::Cashflows])
            .unwrap();
        assert!(results.price().unwrap().is_finite());
        assert!(bounded.evaluate(&context, &[Request::FairRate]).is_err());
        let mut introductory = terms_with_floating_without_amortization();
        introductory.options =
            serde_json::from_str(r#"{"introductory_periods":12,"introductory_rate":0.04}"#)
                .unwrap();
        let results = introductory
            .build("introductory")
            .unwrap()
            .evaluate(&context, &[Request::Value, Request::Cashflows])
            .unwrap();
        assert!(results.price().unwrap().is_finite());

        let mut quotes = QuoteStore::new(Date::new(2026, 2, 24));
        let identifiers: Vec<String> = ["1Y", "3Y", "5Y", "10Y"]
            .iter()
            .map(|tenor| format!("Cds_BORROWER_EUR_{tenor}"))
            .collect();
        for identifier in &identifiers {
            quotes.add_quote(Quote::new(
                identifier.parse().unwrap(),
                QuoteLevels::with_mid(0.01),
            ));
        }
        let configuration: CreditCurveConfiguration = serde_json::from_value(serde_json::json!({
            "market_index":{"Credit":"BORROWER"}, "currency":"EUR", "discount_index":"ESTR", "recovery":0.4, "quotes":identifiers,
        })).unwrap();
        let curve: quantsupport::prelude::CurveConfiguration = serde_json::from_value(serde_json::json!({
            "market_index":"ESTR", "interpolator":"Linear", "day_counter":"Actual360",
            "construction":{"type":"ZeroRate","pillars":["0D","5Y","10Y"],"rates":[0.03,0.03,0.03],"compounding":"Continuous","frequency":"Annual"},
        })).unwrap();
        let mut credit_context = PricingContext::new()
            .with_quote_store(quotes)
            .with_base_currency(Currency::EUR)
            .with_base_index(MarketIndex::ESTR)
            .with_curve_configurations(vec![curve])
            .with_credit_curve_configurations(vec![configuration]);
        credit_context.initialize().unwrap();
        let mut credit = terms();
        credit.options =
            serde_json::from_str(r#"{"credit_index":{"Credit":"BORROWER"},"recovery":0.4}"#)
                .unwrap();
        let results = credit
            .build("credit")
            .unwrap()
            .evaluate(
                &credit_context,
                &[
                    Request::Value,
                    Request::Cashflows,
                    Request::FairRate,
                    Request::Sensitivities,
                ],
            )
            .unwrap();
        let risk_free = terms()
            .build("risk_free")
            .unwrap()
            .evaluate(&credit_context, &[Request::Value])
            .unwrap()
            .price()
            .unwrap();
        assert!(results.price().unwrap() < risk_free);
        assert!(results.cashflows().unwrap().leg_indices().contains(&2));
    }
}
