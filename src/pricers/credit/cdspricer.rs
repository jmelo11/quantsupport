//! Closed-form CDS pricer.
//!
//! Prices a [`CreditDefaultSwap`](crate::instruments::credit::creditdefaultswap::CreditDefaultSwap) off a bootstrapped survival curve
//! ([`MarketIndex::Credit`](crate::indices::marketindex::MarketIndex::Credit)) and a discount curve. When a
//! [`DiscountPolicy`] is set, discounting uses the CSA collateral curve of
//! the counterparty instead of the instrument's own `discount_index`.
//!
//! Leg conventions (per unit notional, protection buyer):
//! - premium leg: `spread · Σ Δ_i · DF(d_i) · (S(d_i) + ½·(S(d_{i-1}) − S(d_i)))`
//!   (running premium on survival plus half-period accrual-on-default),
//! - protection leg: `(1 − R) · Σ DF(d_i) · (S(d_{i-1}) − S(d_i))`.
//!
//! Value = `side.sign() · notional · (protection − premium)`, so a
//! [`Side::LongReceive`](crate::core::trade::Side) trade is a protection buyer.

use crate::{
    ad::{dual::DualFwd, scalar::Scalar, tape::Tape},
    core::{
        collateral::DiscountPolicy,
        evaluationresults::{EvaluationResults, SensitivityMap},
        instrument::Instrument,
        marketdatahandling::{
            constructedelementrequest::ConstructedElementRequest,
            marketdata::{MarketData, MarketDataProvider, MarketDataRequest},
        },
        pricer::Pricer,
        pricerstate::PricerState,
        request::{HandleFairRate, HandleSensitivities, HandleValue, Request},
        trade::Trade,
    },
    instruments::credit::creditdefaultswap::CdsTrade,
    time::{date::Date, schedule::MakeSchedule},
    utils::errors::{QSError, Result},
};
use std::collections::HashSet;

/// Prices a credit default swap using survival probabilities from a
/// bootstrapped credit curve.
pub struct CdsPricer {
    discount_policy: Option<Box<dyn DiscountPolicy>>,
}

impl CdsPricer {
    /// Creates a new [`CdsPricer`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            discount_policy: None,
        }
    }
}

impl Default for CdsPricer {
    fn default() -> Self {
        Self::new()
    }
}

/// State for CDS pricing.
#[derive(Default)]
struct CdsState {
    value: Option<DualFwd>,
    market_data: Option<MarketData>,
}

impl PricerState for CdsState {
    fn get_market_data_reponse(&self) -> Option<&MarketData> {
        self.market_data.as_ref()
    }

    fn get_market_data_reponse_mut(&mut self) -> Option<&mut MarketData> {
        self.market_data.as_mut()
    }
}

impl CdsPricer {
    fn resolve_discount_index(
        &self,
        trade: &CdsTrade,
    ) -> Result<crate::indices::marketindex::MarketIndex> {
        let cds = trade.instrument();
        self.discount_policy.as_ref().map_or_else(
            || Ok(cds.discount_index().clone()),
            |policy| policy.accept(cds),
        )
    }

    /// Computes the premium annuity (risky PV01 including accrual-on-default,
    /// per unit spread) and protection leg PV (per unit notional) as AD values.
    fn legs(&self, trade: &CdsTrade, state: &CdsState) -> Result<(DualFwd, DualFwd)> {
        let cds = trade.instrument();
        let discount_index = self.resolve_discount_index(trade)?;
        let discount_curve = state.get_discount_curve_element(&discount_index)?;
        let credit_curve = state.get_credit_curve_element(cds.credit_index())?;

        let schedule = MakeSchedule::new(cds.start_date(), cds.maturity_date())
            .with_frequency(cds.premium_frequency())
            .build()?;

        let discount = discount_curve.curve();
        let credit = credit_curve.curve();
        let ref_date: Date = discount.reference_date();

        let mut annuity = DualFwd::scalar(0.0);
        let mut protection = DualFwd::scalar(0.0);
        let mut s_prev: DualFwd = if schedule.dates()[0] > ref_date {
            credit.discount_factor(schedule.dates()[0])?
        } else {
            DualFwd::scalar(1.0)
        };
        for w in schedule.dates().windows(2) {
            let (d0, d1) = (w[0], w[1]);
            if d1 <= ref_date {
                continue;
            }
            let delta = cds.day_counter().year_fraction(d0, d1);
            let df: DualFwd = discount.discount_factor(d1)?;
            let s1: DualFwd = credit.discount_factor(d1)?;
            let default_prob: DualFwd = (s_prev - s1).into();
            let period_annuity: DualFwd =
                (df * (s1 + default_prob * DualFwd::scalar(0.5)) * DualFwd::scalar(delta)).into();
            annuity = (annuity + period_annuity).into();
            protection = (protection + df * default_prob).into();
            s_prev = s1;
        }
        let lgd = DualFwd::scalar(1.0 - cds.recovery());
        Ok((annuity, (protection * lgd).into()))
    }
}

impl HandleValue<CdsTrade, CdsState> for CdsPricer {
    fn handle_value(&self, trade: &CdsTrade, state: &mut CdsState) -> Result<f64> {
        let cds = trade.instrument();

        Tape::start_recording_fwd();
        Tape::set_mark_fwd();
        state.put_pillars_on_tape()?;

        let (annuity, protection) = self.legs(trade, state)?;
        let premium: DualFwd = (annuity * DualFwd::scalar(cds.spread())).into();
        let sign = trade.side().sign();
        let value: DualFwd =
            ((protection - premium) * DualFwd::scalar(sign * trade.notional())).into();
        state.value = Some(value);

        Tape::stop_recording_fwd();
        Ok(value.value())
    }
}

impl HandleFairRate<CdsTrade, CdsState> for CdsPricer {
    fn handle_fair_rate(
        &self,
        trade: &CdsTrade,
        state: &mut CdsState,
        target: usize,
    ) -> Result<f64> {
        if target != 0 {
            return Err(QSError::InvalidValueErr(
                "Fair-rate target must be zero for this product".into(),
            ));
        }
        Tape::start_recording_fwd();
        Tape::set_mark_fwd();
        state.put_pillars_on_tape()?;
        let (annuity, protection) = self.legs(trade, state)?;
        Tape::stop_recording_fwd();
        // Restarting the tape invalidates any previously cached value node.
        state.value = None;
        if annuity.value().abs() < 1e-16 {
            return Err(QSError::InvalidValueErr(
                "CDS risky annuity is zero; cannot compute par spread".into(),
            ));
        }
        Ok(protection.value() / annuity.value())
    }
}

impl HandleSensitivities<CdsTrade, CdsState> for CdsPricer {
    fn handle_sensitivities(
        &self,
        trade: &CdsTrade,
        state: &mut CdsState,
    ) -> Result<SensitivityMap> {
        let value = if let Some(v) = state.value {
            v
        } else {
            let _ = self.handle_value(trade, state)?;
            state.value.ok_or_else(|| {
                QSError::UnexpectedErr(
                    "State does not contain price after value computation.".into(),
                )
            })?
        };

        value.backward_to_mark()?;

        let cds = trade.instrument();
        let discount_index = self.resolve_discount_index(trade)?;

        let mut ids = Vec::new();
        let mut exposures = Vec::new();

        // Discount curve sensitivities.
        for (label, pillar) in state
            .get_discount_curve_element(&discount_index)?
            .curve()
            .pillars()
            .unwrap_or_default()
        {
            ids.push(label);
            exposures.push(pillar.adjoint()?.value());
        }

        // Credit curve sensitivities (to the CDS quotes via IFT).
        for (label, pillar) in state
            .get_credit_curve_element(cds.credit_index())?
            .curve()
            .pillars()
            .unwrap_or_default()
        {
            ids.push(label);
            exposures.push(pillar.adjoint()?.value());
        }

        Ok(SensitivityMap::default()
            .with_instrument_keys(&ids)
            .with_exposure(&exposures)
            .aggregate())
    }
}

impl Pricer for CdsPricer {
    type Item = CdsTrade;
    type Policy = dyn DiscountPolicy;

    fn evaluate(
        &self,
        trade: &CdsTrade,
        requests: &[Request],
        ctx: &impl MarketDataProvider,
    ) -> Result<EvaluationResults> {
        Request::validate(requests)?;
        let eval_date = ctx.evaluation_date();
        let identifier = trade.instrument().identifier();

        let md_request = self
            .market_data_request(trade)
            .ok_or_else(|| QSError::InvalidValueErr("Missing market data request".into()))?;

        let mut results = EvaluationResults::new(eval_date, identifier);
        let mut state = CdsState {
            value: None,
            market_data: Some(ctx.handle_request(&md_request)?),
        };

        for request in requests {
            match request {
                Request::Value => {
                    let price = self.handle_value(trade, &mut state)?;
                    results = results.with_price(price);
                }
                Request::FairRate(target) => {
                    let fair_rate = self.handle_fair_rate(trade, &mut state, *target)?;
                    results = results.with_fair_rate(fair_rate);
                }
                Request::Sensitivities => {
                    let sensitivities = self.handle_sensitivities(trade, &mut state)?;
                    results = results.with_sensitivities(sensitivities);
                }
                _ => {}
            }
        }

        Ok(results)
    }

    fn market_data_request(&self, trade: &CdsTrade) -> Option<MarketDataRequest> {
        let cds = trade.instrument();
        let mut elements = vec![
            ConstructedElementRequest::DiscountCurve {
                market_index: cds.discount_index().clone(),
            },
            ConstructedElementRequest::CreditCurve {
                market_index: cds.credit_index().clone(),
            },
        ];

        let mut seen_indices = HashSet::new();
        seen_indices.insert(cds.discount_index().clone());

        if let Some(policy) = &self.discount_policy {
            for policy_index in policy.discount_indices() {
                if seen_indices.insert(policy_index.clone()) {
                    elements.push(ConstructedElementRequest::DiscountCurve {
                        market_index: policy_index,
                    });
                }
            }
        }

        let request = MarketDataRequest::default().with_constructed_elements_request(elements);
        Some(request)
    }

    fn set_discount_policy(&mut self, policy: Box<Self::Policy>) {
        self.discount_policy = Some(policy);
    }

    fn discount_policy(&self) -> Option<&Self::Policy> {
        self.discount_policy.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::{
            elements::curveelement::{CreditCurveElement, DiscountCurveElement},
            marketdatahandling::constructedelementstore::ConstructedElementStore,
            trade::Side,
        },
        currencies::currency::Currency,
        indices::marketindex::MarketIndex,
        instruments::credit::creditdefaultswap::CreditDefaultSwap,
        math::interpolation::interpolator::Interpolator,
        rates::yieldtermstructure::discounttermstructure::DiscountTermStructure,
        time::{
            daycounter::DayCounter,
            enums::{Frequency, TimeUnit},
        },
    };
    use std::{cell::RefCell, collections::HashMap, rc::Rc};

    const DC: DayCounter = DayCounter::Actual360;

    struct FlatProvider {
        evaluation_date: Date,
        market_data: MarketData,
    }

    impl MarketDataProvider for FlatProvider {
        fn handle_request(&self, _: &MarketDataRequest) -> Result<MarketData> {
            Ok(MarketData::new(
                self.market_data.fixings().clone(),
                self.market_data.constructed_elements().clone(),
            ))
        }

        fn evaluation_date(&self) -> Date {
            self.evaluation_date
        }
    }

    /// Term structure with `exp(-rate·t)` factors at monthly nodes.
    fn flat_exp_curve(ref_date: Date, rate: f64) -> Result<DiscountTermStructure<DualFwd>> {
        let dates: Vec<Date> = (0..=120)
            .map(|i| ref_date.advance(i, TimeUnit::Months))
            .collect();
        let dfs: Vec<DualFwd> = dates
            .iter()
            .map(|d| DualFwd::scalar((-rate * DC.year_fraction(ref_date, *d)).exp()))
            .collect();
        DiscountTermStructure::<DualFwd>::new(dates, dfs, DC, Interpolator::LogLinear, true)
    }

    /// Provider with a flat `r` discount curve and a flat `λ` hazard curve.
    fn flat_provider(ref_date: Date, r: f64, lambda: f64) -> Result<FlatProvider> {
        let discount = DiscountCurveElement::new(
            MarketIndex::SOFR,
            Rc::new(RefCell::new(flat_exp_curve(ref_date, r)?)),
        );
        let credit = CreditCurveElement::new(
            MarketIndex::Credit("CORPORATE-A".to_string()),
            0.4,
            Rc::new(RefCell::new(flat_exp_curve(ref_date, lambda)?)),
        );
        let mut store = ConstructedElementStore::default();
        store
            .discount_curves_mut()
            .insert(MarketIndex::SOFR, discount);
        store
            .credit_curves_mut()
            .insert(MarketIndex::Credit("CORPORATE-A".to_string()), credit);
        Ok(FlatProvider {
            evaluation_date: ref_date,
            market_data: MarketData::new(HashMap::new(), store),
        })
    }

    fn make_trade(
        ref_date: Date,
        years: i32,
        spread: f64,
        recovery: f64,
        side: Side,
        notional: f64,
    ) -> Result<CdsTrade> {
        let cds = CreditDefaultSwap::new(
            format!("CDS_CORPORATE-A_{years}Y"),
            MarketIndex::Credit("CORPORATE-A".to_string()),
            MarketIndex::SOFR,
            Currency::USD,
            ref_date,
            ref_date.advance(years, TimeUnit::Years),
            spread,
            recovery,
            Frequency::Quarterly,
            DC,
        )?;
        Ok(CdsTrade::new(cds, ref_date, notional, side))
    }

    /// Benchmarks the discretized legs against the continuous-time closed
    /// forms for flat hazard `λ` and flat rate `r`:
    /// `protection = LGD·λ/(λ+r)·(1−e^{−(λ+r)T})`,
    /// `annuity = (1−e^{−(λ+r)T})/(λ+r)` and `par ≈ LGD·λ`.
    #[test]
    fn matches_continuous_time_closed_form() -> Result<()> {
        let ref_date = Date::new(2025, 1, 2);
        let (r, lambda, recovery, notional) = (0.03, 0.05, 0.4, 1_000_000.0);
        let lgd = 1.0 - recovery;
        let years = 5;
        let t = DC.year_fraction(ref_date, ref_date.advance(years, TimeUnit::Years));

        let provider = flat_provider(ref_date, r, lambda)?;
        let contract_spread = 0.02;
        let trade = make_trade(
            ref_date,
            years,
            contract_spread,
            recovery,
            Side::LongReceive,
            notional,
        )?;
        let results = CdsPricer::new().evaluate(
            &trade,
            &[Request::Value, Request::FairRate(0)],
            &provider,
        )?;

        let decay = 1.0 - (-(lambda + r) * t).exp();
        let protection = lgd * lambda / (lambda + r) * decay;
        let annuity = decay / (lambda + r);
        let expected_value = notional * (protection - contract_spread * annuity);
        let expected_par = protection / annuity; // ≈ lgd·λ

        let price = results
            .price()
            .ok_or_else(|| QSError::UnexpectedErr("missing price".into()))?;
        assert!(
            (price - expected_value).abs() / expected_value.abs() < 5e-3,
            "value {price} vs closed form {expected_value}"
        );

        let fair = results
            .fair_rate()
            .ok_or_else(|| QSError::UnexpectedErr("missing fair rate".into()))?;
        assert!(
            (fair - expected_par).abs() / expected_par < 5e-3,
            "fair spread {fair} vs closed form {expected_par}"
        );
        assert!(
            (fair - lgd * lambda).abs() / (lgd * lambda) < 5e-3,
            "credit triangle: fair {fair} vs lgd·λ {}",
            lgd * lambda
        );
        Ok(())
    }

    /// λ → 0 limit: no default risk, protection is worthless and the value
    /// collapses to minus the premium leg (a riskless annuity).
    #[test]
    fn zero_hazard_limit() -> Result<()> {
        let ref_date = Date::new(2025, 1, 2);
        let (r, notional, spread) = (0.03, 1_000_000.0, 0.01);
        let provider = flat_provider(ref_date, r, 0.0)?;
        let trade = make_trade(ref_date, 5, spread, 0.4, Side::LongReceive, notional)?;
        let results = CdsPricer::new().evaluate(&trade, &[Request::Value], &provider)?;
        let price = results
            .price()
            .ok_or_else(|| QSError::UnexpectedErr("missing price".into()))?;

        // Riskless quarterly annuity, computed directly.
        let mut annuity = 0.0;
        let schedule = MakeSchedule::new(ref_date, ref_date.advance(5, TimeUnit::Years))
            .with_frequency(Frequency::Quarterly)
            .build()?;
        for w in schedule.dates().windows(2) {
            let delta = DC.year_fraction(w[0], w[1]);
            annuity += delta * (-r * DC.year_fraction(ref_date, w[1])).exp();
        }
        let expected = -notional * spread * annuity;
        assert!(
            (price - expected).abs() < 1e-6 * notional,
            "zero hazard: price {price} vs riskless premium leg {expected}"
        );
        Ok(())
    }

    /// λ → ∞ limit: default is immediate, so the protection buyer's value
    /// approaches `LGD · notional` (premium leg vanishes with survival).
    #[test]
    fn extreme_hazard_limit() -> Result<()> {
        let ref_date = Date::new(2025, 1, 2);
        let (notional, recovery) = (1_000_000.0, 0.4);
        let provider = flat_provider(ref_date, 0.03, 50.0)?;
        let trade = make_trade(ref_date, 5, 0.01, recovery, Side::LongReceive, notional)?;
        let results = CdsPricer::new().evaluate(&trade, &[Request::Value], &provider)?;
        let price = results
            .price()
            .ok_or_else(|| QSError::UnexpectedErr("missing price".into()))?;
        let lgd_notional = (1.0 - recovery) * notional;
        // Default happens almost surely within the first quarter; the first
        // coupon is discounted over ~3 months so the bound is loose but tight
        // enough to catch sign or leg errors.
        assert!(
            price > 0.9 * lgd_notional && price <= lgd_notional,
            "extreme hazard: price {price} should approach LGD·notional {lgd_notional}"
        );
        Ok(())
    }

    /// R → 1 limit: recovery of everything makes protection worthless.
    #[test]
    fn full_recovery_limit() -> Result<()> {
        let ref_date = Date::new(2025, 1, 2);
        let provider = flat_provider(ref_date, 0.03, 0.05)?;
        let trade = make_trade(ref_date, 5, 0.0, 0.999_999, Side::LongReceive, 1_000_000.0)?;
        let results = CdsPricer::new().evaluate(&trade, &[Request::Value], &provider)?;
        let price = results
            .price()
            .ok_or_else(|| QSError::UnexpectedErr("missing price".into()))?;
        assert!(
            price.abs() < 1.0,
            "full recovery with zero spread must be worthless, got {price}"
        );
        Ok(())
    }

    /// Value must be monotonically increasing in hazard for a protection buyer.
    #[test]
    fn value_monotone_in_hazard() -> Result<()> {
        let ref_date = Date::new(2025, 1, 2);
        let mut last = f64::NEG_INFINITY;
        for lambda in [0.001, 0.01, 0.05, 0.10, 0.25] {
            let provider = flat_provider(ref_date, 0.03, lambda)?;
            let trade = make_trade(ref_date, 5, 0.02, 0.4, Side::LongReceive, 1_000_000.0)?;
            let price = CdsPricer::new()
                .evaluate(&trade, &[Request::Value], &provider)?
                .price()
                .ok_or_else(|| QSError::UnexpectedErr("missing price".into()))?;
            assert!(
                price > last,
                "buyer value must increase with hazard: {price} after {last}"
            );
            last = price;
        }
        Ok(())
    }
}
