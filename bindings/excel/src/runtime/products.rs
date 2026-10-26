//! Native contracts and position construction, kept separate from pricing policy.

use super::registry::QsObject;
use quantsupport::instruments::cashflows::{
    cashflow::SimpleCashflow, optionembeddedcoupon::OptionEmbeddedCoupon,
};
use quantsupport::prelude::*;
use std::result::Result;

type InstrumentResolver =
    dyn Fn(&ResolutionContext<'_>) -> Result<NativeInstrument, String> + Send + Sync;

#[derive(Clone)]
pub struct PendingInstrument {
    kind: &'static str,
    resolver: std::sync::Arc<InstrumentResolver>,
}

pub struct PendingTrade {
    instrument: PendingInstrument,
    notional: f64,
    foreign_notional: Option<f64>,
    date: Option<Date>,
    trade_tenor: Option<Period>,
    side: Side,
    terms: PositionTerms,
}

impl PendingTrade {
    pub fn kind(&self) -> &'static str {
        match self.instrument.kind {
            "Loan" => "LoanTrade",
            "Swap" => "SwapTrade",
            "BasisSwap" => "BasisSwapTrade",
            "FloatFloatCrossCurrencySwap" => "FloatFloatCrossCurrencySwapTrade",
            "FixFloatCrossCurrencySwap" => "FixFloatCrossCurrencySwapTrade",
            "FixedRateBond" => "FixedRateBondTrade",
            "FixedRateDeposit" => "FixedRateDepositTrade",
            "FloatingRateNote" => "FloatingRateNoteTrade",
            "FxForward" => "FxForwardTrade",
            "FxEuropeanOption" => "FxEuropeanOptionTrade",
            "EquityEuropeanOption" => "EquityEuropeanOptionTrade",
            "EquityForward" => "EquityForwardTrade",
            "CreditDefaultSwap" => "CdsTrade",
            "CapFloor" => "CapFloorTrade",
            "CapletFloorlet" => "CapletFloorletTrade",
            "RateFutures" => "RateFuturesTrade",
            "EuropeanSwaption" => "EuropeanSwaptionTrade",
            _ => "UnresolvedTrade",
        }
    }

    pub fn resolve_trade(&self, market: &impl MarketDataProvider) -> Result<QsObject, String> {
        let context = ResolutionContext::new(market);
        let native = (self.instrument.resolver)(&context)?;
        let date = context
            .resolve_date(
                self.date,
                self.trade_tenor,
                context.reference_date(),
                "Trade date",
            )
            .map_err(|e| e.to_string())?;
        InstrumentObject {
            native,
            notional: self.notional,
            foreign_notional: self.foreign_notional,
        }
        .trade(date, self.side, &self.terms)
    }
}

#[derive(Clone)]
pub enum NativeInstrument {
    Pending(PendingInstrument),
    Loan(Loan<DualFwd>),
    Swap(Swap<DualFwd>),
    BasisSwap(BasisSwap<DualFwd>),
    FloatFloatCrossCurrencySwap(FloatFloatCrossCurrencySwap<DualFwd>),
    FixFloatCrossCurrencySwap(FixFloatCrossCurrencySwap<DualFwd>),
    FixedRateBond(FixedRateBond<DualFwd>),
    FixedRateDeposit(FixedRateDeposit<DualFwd>),
    FloatingRateNote(FloatingRateNote<DualFwd>),
    FxForward(FxForward),
    FxEuropeanOption(FxEuropeanOption),
    EquityEuropeanOption(EquityEuropeanOption),
    EquityForward(EquityForward),
    CreditDefaultSwap(CreditDefaultSwap),
    CapFloor(CapFloor),
    CapletFloorlet(CapletFloorlet),
    RateFutures(RateFutures),
    EuropeanSwaption(EuropeanSwaption<DualFwd>),
}

pub struct InstrumentObject {
    pub native: NativeInstrument,
    pub notional: f64,
    pub foreign_notional: Option<f64>,
}

#[derive(Clone, Default)]
pub struct PositionTerms {
    pub notional: Option<f64>,
    pub foreign_notional: Option<f64>,
    pub pricing_anchor: Option<BondPricingAnchor>,
    pub settlement_date: Option<Date>,
    pub spread_day_counter: Option<DayCounter>,
}

impl InstrumentObject {
    pub fn from_maker<M, F>(
        kind: &'static str,
        notional: f64,
        foreign_notional: Option<f64>,
        maker: M,
        wrap: F,
    ) -> Self
    where
        M: Resolve + Send + Sync + 'static,
        F: Fn(M::Resolved) -> NativeInstrument + Send + Sync + 'static,
    {
        Self::from_resolver(kind, notional, foreign_notional, move |context| {
            maker
                .resolve(context)
                .map(&wrap)
                .map_err(|error| error.to_string())
        })
    }

    pub fn from_resolver<F>(
        kind: &'static str,
        notional: f64,
        foreign_notional: Option<f64>,
        resolver: F,
    ) -> Self
    where
        F: Fn(&ResolutionContext<'_>) -> Result<NativeInstrument, String> + Send + Sync + 'static,
    {
        Self {
            native: NativeInstrument::Pending(PendingInstrument {
                kind,
                resolver: std::sync::Arc::new(resolver),
            }),
            notional,
            foreign_notional,
        }
    }

    pub fn kind(&self) -> &'static str {
        match &self.native {
            NativeInstrument::Pending(pending) => pending.kind,
            NativeInstrument::Loan(_) => "Loan",
            NativeInstrument::Swap(_) => "Swap",
            NativeInstrument::BasisSwap(_) => "BasisSwap",
            NativeInstrument::FloatFloatCrossCurrencySwap(_) => "FloatFloatCrossCurrencySwap",
            NativeInstrument::FixFloatCrossCurrencySwap(_) => "FixFloatCrossCurrencySwap",
            NativeInstrument::FixedRateBond(_) => "FixedRateBond",
            NativeInstrument::FixedRateDeposit(_) => "FixedRateDeposit",
            NativeInstrument::FloatingRateNote(_) => "FloatingRateNote",
            NativeInstrument::FxForward(_) => "FxForward",
            NativeInstrument::FxEuropeanOption(_) => "FxEuropeanOption",
            NativeInstrument::EquityEuropeanOption(_) => "EquityEuropeanOption",
            NativeInstrument::EquityForward(_) => "EquityForward",
            NativeInstrument::CreditDefaultSwap(_) => "CreditDefaultSwap",
            NativeInstrument::CapFloor(_) => "CapFloor",
            NativeInstrument::CapletFloorlet(_) => "CapletFloorlet",
            NativeInstrument::RateFutures(_) => "RateFutures",
            NativeInstrument::EuropeanSwaption(_) => "EuropeanSwaption",
        }
    }

    pub fn trade_from_tenor(
        &self,
        tenor: Period,
        side: Side,
        terms: &PositionTerms,
    ) -> Result<QsObject, String> {
        let instrument = match &self.native {
            NativeInstrument::Pending(pending) => pending.clone(),
            native => {
                let native = native.clone();
                PendingInstrument {
                    kind: self.kind(),
                    resolver: std::sync::Arc::new(move |_| Ok(native.clone())),
                }
            }
        };
        Ok(QsObject::UnresolvedTrade(Box::new(PendingTrade {
            instrument,
            notional: self.notional,
            foreign_notional: self.foreign_notional,
            date: None,
            trade_tenor: Some(tenor),
            side,
            terms: terms.clone(),
        })))
    }

    pub fn trade(&self, date: Date, side: Side, terms: &PositionTerms) -> Result<QsObject, String> {
        if let NativeInstrument::Pending(instrument) = &self.native {
            return Ok(QsObject::UnresolvedTrade(Box::new(PendingTrade {
                instrument: instrument.clone(),
                notional: self.notional,
                foreign_notional: self.foreign_notional,
                date: Some(date),
                trade_tenor: None,
                side,
                terms: terms.clone(),
            })));
        }
        let notional = terms.notional.unwrap_or(self.notional);
        if !notional.is_finite() || notional < 0.0 {
            return Err("position notional must be finite and nonnegative".into());
        }
        let foreign = terms.foreign_notional.or(self.foreign_notional);
        if terms.foreign_notional.is_some() && self.foreign_notional.is_none() {
            return Err("foreign_notional requires a cross-currency swap".into());
        }
        let bond = matches!(self.native, NativeInstrument::FixedRateBond(_));
        if !bond
            && (terms.pricing_anchor.is_some()
                || terms.settlement_date.is_some()
                || terms.spread_day_counter.is_some())
        {
            return Err(
                "pricing anchor, settlement date and spread day counter require a bond".into(),
            );
        }
        let scale = if self.notional == 0.0 {
            if notional != 0.0 {
                return Err("cannot resize a zero-notional contract".into());
            }
            1.0
        } else {
            notional / self.notional
        };
        Ok(match &self.native {
            NativeInstrument::Pending(_) => {
                return Err("instrument must be resolved before positioning".into())
            }
            NativeInstrument::Loan(loan) => {
                if notional != loan.notional() {
                    return Err("Loan notional belongs to the instrument; create a loan with the desired principal".into());
                }
                QsObject::LoanTrade(Box::new(LoanTrade::new(loan.clone(), date, side)))
            }
            NativeInstrument::Swap(instrument) => {
                let positioned = Swap::new(
                    instrument.identifier(),
                    positioned_leg(instrument.fixed_leg(), scale, side)?,
                    positioned_leg(instrument.floating_leg(), scale, side)?,
                    instrument.forward_index(),
                    instrument.currency(),
                );
                QsObject::SwapTrade(Box::new(SwapTrade::new(positioned, date, notional, side)))
            }
            NativeInstrument::BasisSwap(instrument) => {
                let positioned = BasisSwap::new(
                    instrument.identifier(),
                    positioned_leg(instrument.pay_leg(), scale, side)?,
                    positioned_leg(instrument.receive_leg(), scale, side)?,
                    instrument.pay_forward_index(),
                    instrument.receive_forward_index(),
                    instrument.currency(),
                );
                QsObject::BasisSwapTrade(Box::new(BasisSwapTrade::new(
                    positioned, date, notional, side,
                )))
            }
            NativeInstrument::FloatFloatCrossCurrencySwap(instrument) => {
                let foreign = foreign.ok_or("foreign notional is required")?;
                if !foreign.is_finite() || foreign < 0.0 {
                    return Err("foreign notional must be finite and nonnegative".into());
                }
                let base = self
                    .foreign_notional
                    .ok_or("contract foreign notional is required")?;
                let foreign_scale = if base == 0.0 {
                    if foreign != 0.0 {
                        return Err("cannot resize zero foreign principal".into());
                    }
                    1.0
                } else {
                    foreign / base
                };
                let positioned = FloatFloatCrossCurrencySwap::new(
                    instrument.identifier(),
                    positioned_leg(instrument.domestic_leg(), scale, side)?,
                    positioned_leg(instrument.foreign_leg(), foreign_scale, side)?,
                    instrument.domestic_currency(),
                    instrument.foreign_currency(),
                    instrument.domestic_forward_index(),
                    instrument.foreign_forward_index(),
                );
                QsObject::FloatFloatCrossCurrencySwapTrade(Box::new(
                    FloatFloatCrossCurrencySwapTrade::new(
                        positioned, date, notional, foreign, side,
                    ),
                ))
            }
            NativeInstrument::FixFloatCrossCurrencySwap(instrument) => {
                let foreign = foreign.ok_or("foreign notional is required")?;
                if !foreign.is_finite() || foreign < 0.0 {
                    return Err("foreign notional must be finite and nonnegative".into());
                }
                let base = self
                    .foreign_notional
                    .ok_or("contract foreign notional is required")?;
                let foreign_scale = if base == 0.0 {
                    if foreign != 0.0 {
                        return Err("cannot resize zero foreign principal".into());
                    }
                    1.0
                } else {
                    foreign / base
                };
                let positioned = FixFloatCrossCurrencySwap::new(
                    instrument.identifier(),
                    positioned_leg(instrument.domestic_leg(), scale, side)?,
                    positioned_leg(instrument.foreign_leg(), foreign_scale, side)?,
                    instrument.domestic_currency(),
                    instrument.foreign_currency(),
                    instrument.forward_index(),
                );
                QsObject::FixFloatCrossCurrencySwapTrade(Box::new(
                    FixFloatCrossCurrencySwapTrade::new(positioned, date, notional, foreign, side),
                ))
            }
            NativeInstrument::FixedRateBond(instrument) => {
                let anchor = terms
                    .pricing_anchor
                    .ok_or("bond positions require a pricing_anchor and anchor_value")?;
                anchor.validate().map_err(|error| error.to_string())?;
                let positioned = FixedRateBond::new(
                    instrument.identifier(),
                    instrument.units(),
                    positioned_leg(instrument.leg(), scale, Side::LongReceive)?,
                    instrument.discount_index(),
                    instrument.currency(),
                );
                let mut trade = FixedRateBondTrade::new(positioned, date, notional, side, anchor);
                if let Some(value) = terms.settlement_date {
                    trade = trade.with_settlement_date(value);
                }
                if let Some(value) = terms.spread_day_counter {
                    trade = trade.with_spread_day_counter(value);
                }
                QsObject::FixedRateBondTrade(Box::new(trade))
            }
            NativeInstrument::FixedRateDeposit(instrument) => {
                let positioned = FixedRateDeposit::new(
                    instrument.identifier(),
                    instrument.units(),
                    positioned_leg(instrument.leg(), scale, side)?,
                    instrument.discount_index(),
                    instrument.start_date(),
                    instrument.maturity_date(),
                    instrument.currency(),
                );
                QsObject::FixedRateDepositTrade(Box::new(FixedRateDepositTrade::new(
                    positioned, date, notional, side,
                )))
            }
            NativeInstrument::FloatingRateNote(instrument) => {
                let positioned = FloatingRateNote::new(
                    instrument.identifier(),
                    instrument.units(),
                    positioned_leg(instrument.leg(), scale, side)?,
                    instrument.discount_index(),
                    instrument.leg().forward_index().cloned(),
                    instrument.currency(),
                );
                QsObject::FloatingRateNoteTrade(Box::new(FloatingRateNoteTrade::new(
                    positioned, date, notional, side,
                )))
            }
            NativeInstrument::FxForward(instrument) => QsObject::FxForwardTrade(Box::new(
                FxForwardTrade::new(instrument.clone(), date, notional, side),
            )),
            NativeInstrument::FxEuropeanOption(instrument) => {
                QsObject::FxEuropeanOptionTrade(Box::new(FxEuropeanOptionTrade::new(
                    instrument.clone(),
                    date,
                    notional,
                    side,
                )))
            }
            NativeInstrument::EquityEuropeanOption(instrument) => {
                QsObject::EquityEuropeanOptionTrade(Box::new(EquityEuropeanOptionTrade::new(
                    instrument.clone(),
                    notional,
                    date,
                    side,
                )))
            }
            NativeInstrument::EquityForward(instrument) => {
                QsObject::EquityForwardTrade(Box::new(EquityForwardTrade::new(
                    EquityForward::new(
                        instrument.identifier(),
                        instrument.market_index(),
                        instrument.delivery_date(),
                        instrument.strike(),
                        *instrument.currency(),
                        *instrument.day_counter(),
                    ),
                    date,
                    notional,
                    side,
                )))
            }
            NativeInstrument::CreditDefaultSwap(instrument) => QsObject::CdsTrade(Box::new(
                CdsTrade::new(instrument.clone(), date, notional, side),
            )),
            NativeInstrument::CapFloor(instrument) => QsObject::CapFloorTrade(Box::new(
                CapFloorTrade::new(instrument.clone(), date, notional, side),
            )),
            NativeInstrument::CapletFloorlet(instrument) => {
                QsObject::CapletFloorletTrade(Box::new(CapletFloorletTrade::new(
                    instrument.clone(),
                    date,
                    notional,
                    side,
                )))
            }
            NativeInstrument::RateFutures(instrument) => QsObject::RateFuturesTrade(Box::new(
                RateFuturesTrade::new(instrument.clone(), date, notional, side),
            )),
            NativeInstrument::EuropeanSwaption(instrument) => {
                QsObject::EuropeanSwaptionTrade(Box::new(EuropeanSwaptionTrade::new(
                    instrument.clone(),
                    date,
                    notional,
                    side,
                )))
            }
        })
    }
}

/// Position sizing and leg orientation affect a clone, preserving the contract.
fn positioned_leg(leg: &Leg<DualFwd>, scale: f64, direction: Side) -> Result<Leg<DualFwd>, String> {
    let cashflows = leg
        .cashflows()
        .iter()
        .map(|cashflow| {
            Ok(match cashflow {
                CashflowType::FixedRateCoupon(coupon) => {
                    CashflowType::FixedRateCoupon(FixedRateCoupon::new(
                        coupon.notional() * scale,
                        Box::new(*coupon.rate()),
                        coupon.accrual_start_date(),
                        coupon.accrual_end_date(),
                        coupon.payment_date(),
                    ))
                }
                CashflowType::FloatingRateCoupon(coupon) => {
                    let mut positioned = FloatingRateCoupon::new(
                        coupon.notional() * scale,
                        coupon.spread(),
                        coupon.market_index().clone(),
                        coupon.accrual_start_date(),
                        coupon.accrual_end_date(),
                        coupon.payment_date(),
                    )
                    .with_day_counter(coupon.day_counter());
                    if let Some(conventions) = coupon.overnight_conventions() {
                        positioned
                            .set_overnight_conventions(conventions.clone())
                            .map_err(|error| error.to_string())?;
                    }
                    if let Some(fixing) = coupon.fixing() {
                        positioned.set_fixing(fixing);
                    }
                    CashflowType::FloatingRateCoupon(positioned)
                }
                CashflowType::OptionEmbeddedCoupon(coupon) => {
                    let mut positioned = OptionEmbeddedCoupon::new(
                        coupon.notional() * scale,
                        coupon.market_index().clone(),
                        coupon.spread(),
                        coupon.accrual_start_date(),
                        coupon.accrual_end_date(),
                        coupon.payment_date(),
                        coupon.payoff_ops().clone(),
                    )
                    .with_day_counter(coupon.day_counter().map_err(|error| error.to_string())?);
                    if let Some(fixing) = coupon.fixing() {
                        positioned.set_fixing(fixing);
                    }
                    CashflowType::OptionEmbeddedCoupon(positioned)
                }
                CashflowType::Redemption(flow) => CashflowType::Redemption(SimpleCashflow::new(
                    flow.amount().map_err(|error| error.to_string())? * scale,
                    flow.payment_date(),
                )),
                CashflowType::Disbursement(flow) => {
                    CashflowType::Disbursement(SimpleCashflow::new(
                        flow.amount().map_err(|error| error.to_string())? * scale,
                        flow.payment_date(),
                    ))
                }
                CashflowType::ConstantAmount(flow) => {
                    CashflowType::ConstantAmount(SimpleCashflow::new(
                        flow.amount().map_err(|error| error.to_string())? * scale,
                        flow.payment_date(),
                    ))
                }
                CashflowType::OptionEmbeddedCashflow(_) => {
                    return Err("position sizing does not support option-embedded cashflows".into())
                }
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let side = if leg.side().sign() * direction.sign() > 0.0 {
        Side::LongReceive
    } else {
        Side::PayShort
    };
    Ok(Leg::new(
        leg.id(),
        cashflows,
        leg.currency(),
        leg.discount_index(),
        leg.forward_index().cloned(),
        leg.spread(),
        leg.interest_rate(),
        side,
        leg.is_linear(),
        leg.asset_class(),
        leg.first_payment_date(),
        leg.last_payment_date(),
    ))
}
