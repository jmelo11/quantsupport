//! Deterministic pricing of fixed-rate bond trades.
use super::bondanalytics::{
    bond_market_data_request, contractual_market_request, discounted_value, evaluate_bond,
    impl_bond_pricer, BondPricingEngine, BondState, BondTerms, BondValuation,
};
use crate::instruments::fixedincome::fixedratebond::FixedRateBondTrade;
use crate::{
    ad::{dual::DualFwd, scalar::Scalar},
    core::{
        collateral::DiscountPolicy,
        evaluationresults::EvaluationResults,
        marketdatahandling::marketdata::{MarketData, MarketDataProvider, MarketDataRequest},
        pricer::Pricer,
        request::{HandleCashflows, Request},
    },
    indices::marketindex::MarketIndex,
    utils::errors::Result,
};
/// Deterministic bond valuation using the trade's price or Z-spread anchor.
#[derive(Default)]
pub struct BondPricer {
    discount_policy: Option<Box<dyn DiscountPolicy>>,
}
impl BondPricer {
    /// Creates a reusable pricer; quote and settlement terms belong to each trade.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}
impl BondPricingEngine for BondPricer {
    type Model<T: Scalar> = ();
    fn policy(&self) -> Option<&dyn DiscountPolicy> {
        self.discount_policy.as_deref()
    }
    fn prepare(
        &self,
        trade: &impl BondTerms,
        index: &MarketIndex,
        context: &impl MarketDataProvider,
    ) -> Result<(MarketData, ())> {
        let request = contractual_market_request(trade, index, context.evaluation_date());
        let market = context.handle_request(&request)?;
        Ok((market, ()))
    }
    fn ad_model(&self, (): &(), _: &MarketData) -> Result<()> {
        Ok(())
    }
    fn value<T: Scalar>(&self, (): &(), valuation: &BondValuation<'_, T>, spread: T) -> Result<T> {
        discounted_value(valuation, spread)
    }
}
impl<T: BondTerms> HandleCashflows<T, BondState> for BondPricer {}
impl_bond_pricer!(BondPricer, FixedRateBondTrade<DualFwd>);

/// Shared fixtures for bond-pricer tests.
#[cfg(test)]
pub mod test_support {
    use super::BondPricer;
    use crate::prelude::*;
    use crate::{
        core::{
            collateral::FixedIncomeDiscountPolicy,
            elements::curveelement::DiscountCurveElement,
            marketdatahandling::constructedelementstore::ConstructedElementStore,
            pricerconfiguration::{PricerConfiguration, PricingModelConfiguration},
        },
        currencies::currency::Currency,
        instruments::{
            cashflows::makeleg::PaymentStructure, fixedincome::makefixedratebond::MakeFixedRateBond,
        },
        models::modelconfiguration::{GaussianRateModelParameters, ParameterSource},
        rates::{
            compounding::Compounding, interestrate::RateDefinition,
            yieldtermstructure::flatforwardtermstructure::FlatForwardTermStructure,
        },
        time::enums::Frequency,
    };
    use std::{cell::RefCell, collections::HashMap, rc::Rc};

    /// Face value and quote units used by test bonds.
    pub const FACE: f64 = 100.0;

    /// Flat continuously compounded discount rate.
    pub const RATE: f64 = 0.03;

    /// Hull-White mean reversion used by callable-bond tests.
    pub const ALPHA: f64 = 0.05;

    /// Hull-White volatility used by callable-bond tests.
    pub const SIGMA: f64 = 0.01;

    /// Creates a discount policy using the USD SOFR curve.
    pub fn policy() -> Box<dyn DiscountPolicy> {
        Box::new(
            FixedIncomeDiscountPolicy::new(false)
                .with_discount_curve_index(Currency::USD, MarketIndex::SOFR),
        )
    }

    /// Creates a straight-bond pricer with the test discount policy.
    pub fn fixed_pricer() -> BondPricer {
        let mut pricer = BondPricer::new();
        pricer.set_discount_policy(policy());
        pricer
    }

    /// Supplies fixed market data to bond-pricer tests.
    #[derive(Clone)]
    pub struct TestProvider {
        /// Valuation date reported to pricers.
        pub evaluation_date: Date,
        /// Market data returned for each request.
        pub market_data: MarketData,
    }

    impl MarketDataProvider for TestProvider {
        fn handle_request(&self, _: &MarketDataRequest) -> Result<MarketData> {
            Ok(self.market_data.clone())
        }

        fn evaluation_date(&self) -> Date {
            self.evaluation_date
        }
    }

    /// Creates a market provider with tree controls and a benchmark flat curve.
    pub fn provider(
        evaluation_date: Date,
        steps_per_year: usize,
    ) -> (TestProvider, FlatForwardTermStructure<f64>) {
        provider_with_method(evaluation_date, steps_per_year, false)
    }

    /// Creates a market provider with tree or Fourier controls and a benchmark curve.
    pub fn provider_with_method(
        evaluation_date: Date,
        steps_per_year: usize,
        use_fourier: bool,
    ) -> (TestProvider, FlatForwardTermStructure<f64>) {
        let definition = RateDefinition::new(
            DayCounter::Actual365,
            Compounding::Continuous,
            Frequency::Annual,
        );
        let ad_curve =
            FlatForwardTermStructure::new(evaluation_date, DualFwd::from(RATE), definition);
        let benchmark_curve = FlatForwardTermStructure::new(evaluation_date, RATE, definition);
        let mut elements = ConstructedElementStore::default();
        elements.discount_curves_mut().insert(
            MarketIndex::SOFR,
            DiscountCurveElement::new(MarketIndex::SOFR, Rc::new(RefCell::new(ad_curve))),
        );
        let time_grid = TimeGridConfiguration::new(steps_per_year, 4096, DayCounter::Actual365);
        let pricer = if use_fourier {
            PricerImplementation::BackwardFourierEvolution {
                time_grid,
                fourier_grid: FourierGridConfiguration::new(256, 10.0),
            }
        } else {
            PricerImplementation::TreeBased { time_grid }
        };
        let configuration = PricerConfiguration::new(
            InstrumentType::SteppedCouponBond,
            pricer,
            vec![PricingModelConfiguration::new(
                MarketIndex::SOFR,
                ModelConfiguration::HullWhite {
                    alpha: ALPHA,
                    parameter_source: ParameterSource::Fixed(GaussianRateModelParameters::new(
                        SIGMA,
                    )),
                },
            )],
        );
        let market_data =
            MarketData::new(HashMap::new(), elements).with_pricer_configurations(HashMap::from([
                (InstrumentType::SteppedCouponBond, configuration),
            ]));
        (
            TestProvider {
                evaluation_date,
                market_data,
            },
            benchmark_curve,
        )
    }

    /// Creates a semiannual USD bullet bond with a zero-spread anchor.
    pub fn fixed_bond(
        evaluation_date: Date,
        maturity_date: Date,
        coupon: f64,
    ) -> FixedRateBondTrade<DualFwd> {
        let definition = RateDefinition::new(
            DayCounter::Actual365,
            Compounding::Compounded,
            Frequency::Semiannual,
        );
        let instrument = MakeFixedRateBond::<DualFwd>::default()
            .with_identifier("TEST-BOND".into())
            .with_start_date(evaluation_date)
            .with_maturity_date(maturity_date)
            .with_rate(coupon)
            .with_units(FACE)
            .with_notional(FACE)
            .with_rate_definition(definition)
            .with_discount_index(MarketIndex::SOFR)
            .with_currency(Currency::USD)
            .with_payment_frequency(Frequency::Semiannual)
            .with_payment_structure(PaymentStructure::Bullet)
            .build()
            .unwrap_or_else(|error| panic!("test fixed bond: {error}"));
        FixedRateBondTrade::new(
            instrument,
            evaluation_date,
            FACE,
            crate::core::trade::Side::LongReceive,
            BondPricingAnchor::ZSpread(0.0),
        )
    }

    /// Returns a straight bond's value, panicking if evaluation fails.
    pub fn fixed_value(
        pricer: &BondPricer,
        trade: &FixedRateBondTrade<DualFwd>,
        provider: &TestProvider,
    ) -> f64 {
        pricer
            .evaluate(trade, &[Request::Value], provider)
            .unwrap_or_else(|error| panic!("fixed bond value: {error}"))
            .price()
            .unwrap_or_else(|| panic!("fixed bond value must be present"))
    }
}

#[cfg(test)]
mod tests {
    use super::{test_support::*, BondPricer};
    use crate::prelude::*;

    #[test]
    fn z_spread_price_anchor_round_trip_duration_and_risk() {
        let evaluation_date = Date::new(2025, 1, 1);
        let (provider, _) = provider(evaluation_date, 48);
        let trade = fixed_bond(evaluation_date, Date::new(2030, 1, 1), 0.04);
        let spread = 0.0125;
        let direct = fixed_pricer();
        let direct_results = direct
            .evaluate(
                &trade
                    .clone()
                    .with_pricing_anchor(BondPricingAnchor::ZSpread(spread)),
                &[
                    Request::Value,
                    Request::ZSpread,
                    Request::YieldToMaturity,
                    Request::ModifiedDuration,
                    Request::Sensitivities,
                ],
                &provider,
            )
            .unwrap_or_else(|error| panic!("direct bond price: {error}"));
        let direct_value = direct_results
            .price()
            .unwrap_or_else(|| panic!("price result"));
        assert_eq!(direct_results.z_spread(), Some(spread));
        assert!(direct_results.ytm().is_some_and(f64::is_finite));
        assert!(direct_results
            .modified_duration()
            .is_some_and(|duration| duration > 0.0));

        let quoted_price = direct_value / FACE * FACE;
        let implied = fixed_pricer();
        let implied_results = implied
            .evaluate(
                &trade
                    .clone()
                    .with_pricing_anchor(BondPricingAnchor::DirtyPrice(quoted_price)),
                &[Request::Value, Request::ZSpread],
                &provider,
            )
            .unwrap_or_else(|error| panic!("price-anchored bond: {error}"));
        assert!((implied_results.z_spread().unwrap_or_default() - spread).abs() < 1.0e-10);
        assert!((implied_results.price().unwrap_or_default() - direct_value).abs() < 1.0e-9);

        let risk = direct_results
            .sensitivities()
            .unwrap_or_else(|| panic!("sensitivity result"));
        let spread_risk = risk
            .instrument_keys()
            .iter()
            .zip(risk.exposure())
            .find_map(|(label, exposure)| (label == "TEST-BOND.ZSpread").then_some(*exposure))
            .unwrap_or_else(|| panic!("Z-spread sensitivity"));
        let bump = 1.0e-6;
        let finite_difference = (fixed_value(
            &fixed_pricer(),
            &trade
                .clone()
                .with_pricing_anchor(BondPricingAnchor::ZSpread(spread + bump)),
            &provider,
        ) - fixed_value(
            &fixed_pricer(),
            &trade.with_pricing_anchor(BondPricingAnchor::ZSpread(spread - bump)),
            &provider,
        )) / (2.0 * bump);
        assert!((spread_risk - finite_difference).abs() < 1.0e-4);
    }

    #[test]
    fn trade_owns_anchor_settlement_and_side_across_scalar_conversion() -> Result<()> {
        let evaluation = Date::new(2025, 1, 1);
        let settlement = Date::new(2025, 7, 1);
        let maturity = Date::new(2027, 1, 1);
        let (provider, _) = provider(evaluation, 12);
        let base = fixed_bond(evaluation, maturity, 0.0);
        let long = FixedRateBondTrade::new(
            base.instrument().clone(),
            evaluation,
            FACE,
            Side::LongReceive,
            BondPricingAnchor::DirtyPrice(97.0),
        )
        .with_settlement_date(settlement)
        .with_spread_day_counter(DayCounter::Actual360);
        let scalar: FixedRateBondTrade<f64> = long.clone().into();
        let roundtrip: FixedRateBondTrade<DualFwd> = scalar.into();
        assert_eq!(roundtrip.pricing_anchor(), long.pricing_anchor());
        assert_eq!(roundtrip.settlement_date(), Some(settlement));
        assert_eq!(roundtrip.spread_day_counter(), Some(DayCounter::Actual360));
        let short = FixedRateBondTrade::new(
            base.instrument().clone(),
            evaluation,
            FACE,
            Side::PayShort,
            long.pricing_anchor(),
        )
        .with_settlement_date(settlement)
        .with_spread_day_counter(DayCounter::Actual360);
        let pricer = fixed_pricer();
        let requests = [
            Request::Value,
            Request::ZSpread,
            Request::Sensitivities,
            Request::Cashflows,
        ];
        let long_result = pricer.evaluate(&roundtrip, &requests, &provider)?;
        let short_result = pricer.evaluate(&short, &requests, &provider)?;
        assert!(
            (long_result.price().unwrap_or_default() + short_result.price().unwrap_or_default())
                .abs()
                < 1.0e-10
        );
        assert_eq!(long_result.z_spread(), short_result.z_spread());
        let long_cashflows = long_result
            .cashflows()
            .ok_or_else(|| QSError::ValueNotSetErr("Long cashflows".into()))?;
        let short_cashflows = short_result
            .cashflows()
            .ok_or_else(|| QSError::ValueNotSetErr("Short cashflows".into()))?;
        assert_eq!(
            long_cashflows.payment_dates(),
            short_cashflows.payment_dates()
        );
        assert_eq!(
            long_cashflows.amounts().len(),
            short_cashflows.amounts().len()
        );
        for (long, short) in long_cashflows
            .amounts()
            .iter()
            .zip(short_cashflows.amounts())
        {
            assert!((long + short).abs() < 1e-10);
        }
        let spread = long_result
            .z_spread()
            .ok_or_else(|| QSError::ValueNotSetErr("Z-spread".into()))?;
        let spread_result = pricer.evaluate(
            &long.with_pricing_anchor(BondPricingAnchor::ZSpread(spread)),
            &requests,
            &provider,
        )?;
        assert!(
            (spread_result.price().unwrap_or_default() - long_result.price().unwrap_or_default())
                .abs()
                < 1.0e-9
        );
        let long_risk = long_result
            .sensitivities()
            .ok_or_else(|| QSError::ValueNotSetErr("Long risk".into()))?;
        let short_risk = short_result
            .sensitivities()
            .ok_or_else(|| QSError::ValueNotSetErr("Short risk".into()))?;
        for (left, right) in long_risk.exposure().iter().zip(short_risk.exposure()) {
            assert!((left + right).abs() < 1.0e-9);
        }
        let short_scalar: FixedRateBondTrade<f64> = short.into();
        assert!(short_scalar
            .into_contingent_claims("SHORT")?
            .iter()
            .all(|claim| claim.side() == Side::PayShort));
        assert_eq!(base.instrument().leg().side(), Side::LongReceive);
        Ok(())
    }

    #[test]
    fn csa_policy_resolves_collateral_curve_for_foreign_bond() -> Result<()> {
        let date = Date::new(2025, 1, 1);
        let trade = fixed_bond(date, Date::new(2027, 1, 1), 0.0);
        let mut pricer = BondPricer::new();
        pricer.set_discount_policy(Box::new(SingleCurveCSADiscountPolicy::new(
            MarketIndex::Other("EUR-COLLATERAL".into()),
            Currency::EUR,
        )));
        let request = pricer
            .market_data_request(&trade)
            .ok_or_else(|| QSError::NotFoundErr("Foreign CSA request".into()))?;
        assert!(matches!(
            request
                .constructed_elements_request()
                .and_then(|elements| elements.first()),
            Some(ConstructedElementRequest::DiscountCurve {
                market_index: MarketIndex::Collateral(Currency::USD, Currency::EUR)
            })
        ));
        Ok(())
    }
}
