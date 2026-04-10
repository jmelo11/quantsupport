use std::collections::HashSet;

use crate::{
    ad::{dual::DualFwd, tape::Tape},
    core::{
        collateral::{DiscountPolicy, Discountable},
        evaluationresults::{EvaluationResults, SensitivityMap},
        instrument::{AssetClass, Instrument},
        marketdatahandling::{
            constructedelementrequest::ConstructedElementRequest,
            fxrequest::FxRequest,
            marketdata::{MarketData, MarketDataProvider, MarketDataRequest},
        },
        pillars::Pillars,
        pricer::Pricer,
        pricerstate::PricerState,
        request::{HandleSensitivities, HandleValue, Request},
        trade::Trade,
    },
    currencies::currency::Currency,
    instruments::fx::fxforward::FxForwardTrade,
    utils::errors::{QSError, Result},
};

/// Pricer for FX forward trades.
///
/// The model forward is computed as
/// `F = S * DF_quote(T) / DF_base(T)`,
/// and the NPV of the trade (from the buyer's side) is
/// `NPV = N * (F - K) * DF_quote(T)`,
/// where `K` is the agreed forward price.
///
/// When a [`DiscountPolicy`] is set, the pricer uses the policy-resolved
/// discount curve for the quote-currency leg instead of the natural curve.
///
/// ## Example
/// ```rust
/// use quantsupport::prelude::*;
///
/// let pricer = FxForwardPricer::new();
///
/// // Build the instrument:
/// let fx_fwd = MakeFxForward::default()
///     .with_identifier("EURUSD-1M".to_string())
///     .with_delivery_date(Date::new(2024, 7, 1))
///     .with_forward_price(1.1025)
///     .with_base_currency(Currency::EUR)
///     .with_quote_currency(Currency::USD)
///     .as_deliverable()
///     .build()
///     .expect("failed to build fx forward");
///
/// // Wrap in a trade and evaluate with a MarketDataProvider:
/// //   let trade = FxForwardTrade::new(fx_fwd, Date::new(2024, 6, 1), 1_000_000.0);
/// //   let results = pricer.evaluate(&trade, &[Request::Value], &ctx);
/// ```
pub struct FxForwardPricer {
    discount_policy: Option<Box<dyn DiscountPolicy>>,
}

impl FxForwardPricer {
    /// Creates a new [`FxForwardPricer`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            discount_policy: None,
        }
    }
}

impl Default for FxForwardPricer {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default)]
struct FxForwardState {
    value: Option<DualFwd>,
    market_data: Option<MarketData>,
}

impl PricerState for FxForwardState {
    fn get_market_data_reponse(&self) -> Option<&MarketData> {
        self.market_data.as_ref()
    }

    fn get_market_data_reponse_mut(&mut self) -> Option<&mut MarketData> {
        self.market_data.as_mut()
    }
}

/// Lightweight [`Discountable`] wrapper used to resolve a discount curve
/// for a specific currency through the discount policy.
struct CurrencyDiscountable {
    currency: Currency,
}

impl Discountable for CurrencyDiscountable {
    fn asset_class(&self) -> AssetClass {
        AssetClass::Fx
    }

    fn currency(&self) -> Currency {
        self.currency
    }
}

impl HandleValue<FxForwardTrade, FxForwardState> for FxForwardPricer {
    fn handle_value(&self, trade: &FxForwardTrade, state: &mut FxForwardState) -> Result<f64> {
        Tape::start_recording_fwd();
        Tape::set_mark_fwd();
        state.put_pillars_on_tape()?;

        let inst = trade.instrument();
        let base = inst.base_currency();
        let quote = inst.quote_currency();

        let policy = self.discount_policy.as_ref().ok_or_else(|| {
            QSError::InvalidValueErr("Discount policy required for FX forward pricing".into())
        })?;
        let base_idx = policy.accept(&CurrencyDiscountable { currency: base })?;
        let quote_idx = policy.accept(&CurrencyDiscountable { currency: quote })?;

        let df_base = state
            .get_discount_curve_element(&base_idx)?
            .curve()
            .discount_factor(inst.delivery_date())?;
        let df_quote = state
            .get_discount_curve_element(&quote_idx)?
            .curve()
            .discount_factor(inst.delivery_date())?;

        let spot = state.get_exchange_rate(base, quote)?;
        let forward: DualFwd = (spot * df_quote / df_base).into();

        // NPV = notional * (F_model - K) * DF_quote * side
        let notional = DualFwd::new(trade.notional());
        let npv: DualFwd = inst.forward_price().map_or_else(
            || (notional * forward * df_quote).into(),
            |k| {
                let side = DualFwd::new(trade.side().sign());
                (notional * (forward - k) * df_quote * side).into()
            },
        );
        state.value = Some(npv);

        Tape::stop_recording_fwd();
        Ok(npv.value())
    }
}

impl HandleSensitivities<FxForwardTrade, FxForwardState> for FxForwardPricer {
    fn handle_sensitivities(
        &self,
        trade: &FxForwardTrade,
        state: &mut FxForwardState,
    ) -> Result<SensitivityMap> {
        let value = if let Some(v) = state.value {
            v
        } else {
            let _ = self.handle_value(trade, state)?;
            state
                .value
                .ok_or_else(|| QSError::UnexpectedErr("Missing value in FX forward state".into()))?
        };

        value.backward_to_mark()?;

        let inst = trade.instrument();
        let policy = self.discount_policy.as_ref().ok_or_else(|| {
            QSError::InvalidValueErr("Discount policy required for FX forward pricing".into())
        })?;
        let base_idx = policy.accept(&CurrencyDiscountable {
            currency: inst.base_currency(),
        })?;
        let quote_idx = policy.accept(&CurrencyDiscountable {
            currency: inst.quote_currency(),
        })?;

        let mut ids = Vec::new();
        let mut exposures = Vec::new();

        for idx in [base_idx, quote_idx] {
            let element = state.get_discount_curve_element(&idx)?;
            for (label, value) in element.curve().pillars().into_iter().flatten() {
                ids.push(label);
                exposures.push(value.adjoint().map(|a| a.value()).unwrap_or(0.0));
            }
        }

        if let Some(store) = state.get_fx_store() {
            for (label, value) in store.pillars().into_iter().flatten() {
                ids.push(label);
                exposures.push(value.adjoint().map(|a| a.value()).unwrap_or(0.0));
            }
        }

        Ok(SensitivityMap::default()
            .with_instrument_keys(&ids)
            .with_exposure(&exposures)
            .aggregate())
    }
}

impl Pricer for FxForwardPricer {
    type Item = FxForwardTrade;
    type Policy = dyn DiscountPolicy;

    fn evaluate(
        &self,
        trade: &FxForwardTrade,
        requests: &[Request],
        ctx: &impl MarketDataProvider,
    ) -> Result<EvaluationResults> {
        let eval_date = ctx.evaluation_date();
        let identifier = trade.instrument().identifier();
        let md_request = self.market_data_request(trade).ok_or_else(|| {
            QSError::InvalidValueErr("Missing market-data request for FX forward".into())
        })?;

        let mut state = FxForwardState {
            value: None,
            market_data: Some(ctx.handle_request(&md_request)?),
        };

        let mut out = EvaluationResults::new(eval_date, identifier);
        for req in requests {
            match req {
                Request::Value => out = out.with_price(self.handle_value(trade, &mut state)?),
                Request::Sensitivities => {
                    out = out.with_sensitivities(self.handle_sensitivities(trade, &mut state)?);
                }
                _ => {}
            }
        }

        Ok(out)
    }

    fn market_data_request(&self, trade: &FxForwardTrade) -> Option<MarketDataRequest> {
        let policy = self.discount_policy.as_ref()?;
        let inst = trade.instrument();
        let mut elements = Vec::new();
        let mut seen_indices = HashSet::new();

        for ccy in [inst.base_currency(), inst.quote_currency()] {
            if let Ok(idx) = policy.accept(&CurrencyDiscountable { currency: ccy }) {
                if seen_indices.insert(idx.clone()) {
                    elements.push(ConstructedElementRequest::DiscountCurve { market_index: idx });
                }
            }
        }

        let mut request = MarketDataRequest::default().with_fx_request(vec![FxRequest::pair(
            inst.base_currency(),
            inst.quote_currency(),
        )]);

        if !elements.is_empty() {
            request = request.with_constructed_elements_request(elements);
        }

        Some(request)
    }

    fn set_discount_policy(&mut self, policy: Box<Self::Policy>) {
        self.discount_policy = Some(policy);
    }

    fn discount_policy(&self) -> Option<&Self::Policy> {
        self.discount_policy.as_deref()
    }
}
