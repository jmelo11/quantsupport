//! Native instrument constructors; positions and pricing are separate worksheet objects.
use crate::functions::helpers::*;
use crate::runtime::{
    products::{InstrumentObject, NativeInstrument},
    registry::{with_registry_mut, QsObject},
};
use quantsupport::prelude::*;
use quantsupport_xll_macros::xll_bindgen;
use std::result::Result;
use xll_rs::types::XllError;

#[xll_bindgen(
    name = "QS.INSTRUMENT.FX.FORWARD",
    category = "QuantSupport - Instruments",
    help = "Creates a native FxForward contract; use QS.TRADE.CREATE for a position"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_instrument_fx_forward(
    name: &str,
    delivery_date: &str,
    base_currency: &str,
    quote_currency: &str,
    forward_rate: f64,
) -> Result<String, XllError> {
    let key = input_key(&(delivery_date, base_currency, quote_currency, forward_rate))?;
    if let Some(handle) = cached_handle(name, "FxForward", &key)? {
        return Ok(handle);
    }
    let side = Side::LongReceive;
    let instrument = MakeFxForward::default()
        .with_identifier(name.to_string())
        .with_delivery_date(parse_date(delivery_date)?)
        .with_forward_rate(forward_rate)
        .with_base_currency(parse_currency(base_currency)?)
        .with_quote_currency(parse_currency(quote_currency)?)
        .with_side(side)
        .as_deliverable()
        .build()
        .map_err(value_error)?;
    let object = InstrumentObject {
        native: NativeInstrument::FxForward(instrument),
        notional: 1.0,
        foreign_notional: None,
    };
    with_registry_mut(|registry| {
        registry.upsert_if_changed(name, QsObject::Instrument(Box::new(object)), key)
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.INSTRUMENT.SWAP",
    category = "QuantSupport - Instruments",
    help = "Creates a native Swap contract; use QS.TRADE.CREATE for a position"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_instrument_swap(
    name: &str,
    start_date: &str,
    maturity_date: &str,
    currency: &str,
    market_index: &str,
    notional: f64,
    fixed_rate: f64,
    spread: f64,
    day_counter: &str,
    compounding: &str,
    rate_frequency: &str,
    fixed_leg_frequency: &str,
    floating_leg_frequency: &str,
) -> Result<String, XllError> {
    let key = input_key(&(
        start_date,
        maturity_date,
        currency,
        market_index,
        notional,
        fixed_rate,
        spread,
        day_counter,
        compounding,
        rate_frequency,
        fixed_leg_frequency,
        floating_leg_frequency,
    ))?;
    if let Some(handle) = cached_handle(name, "Swap", &key)? {
        return Ok(handle);
    }
    let side = Side::LongReceive;
    let instrument = MakeSwap::<DualFwd>::default()
        .with_identifier(name.to_string())
        .with_start_date(parse_date(start_date)?)
        .with_maturity_date(parse_date(maturity_date)?)
        .with_fixed_rate(fixed_rate)
        .with_spread(spread)
        .with_notional(notional)
        .with_rate_definition(RateDefinition::new(
            parse_day_counter(day_counter)?,
            parse_compounding(compounding)?,
            parse_frequency(rate_frequency)?,
        ))
        .with_market_index(parse_index(market_index)?)
        .with_currency(parse_currency(currency)?)
        .with_side(side)
        .with_fixed_leg_frequency(parse_frequency(fixed_leg_frequency)?)
        .with_floating_leg_frequency(parse_frequency(floating_leg_frequency)?)
        .build()
        .map_err(value_error)?;
    let object = InstrumentObject {
        native: NativeInstrument::Swap(instrument),
        notional,
        foreign_notional: None,
    };
    with_registry_mut(|registry| {
        registry.upsert_if_changed(name, QsObject::Instrument(Box::new(object)), key)
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.INSTRUMENT.BASIS.SWAP",
    category = "QuantSupport - Instruments",
    help = "Creates a native BasisSwap contract; use QS.TRADE.CREATE for a position"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_instrument_basis_swap(
    name: &str,
    start_date: &str,
    maturity_date: &str,
    currency: &str,
    pay_index: &str,
    receive_index: &str,
    notional: f64,
    pay_spread: f64,
    receive_spread: f64,
    pay_frequency: &str,
    receive_frequency: &str,
) -> Result<String, XllError> {
    let key = input_key(&(
        start_date,
        maturity_date,
        currency,
        pay_index,
        receive_index,
        notional,
        pay_spread,
        receive_spread,
        pay_frequency,
        receive_frequency,
    ))?;
    if let Some(handle) = cached_handle(name, "BasisSwap", &key)? {
        return Ok(handle);
    }
    let side = Side::LongReceive;
    let instrument = MakeBasisSwap::<DualFwd>::default()
        .with_identifier(name.to_string())
        .with_start_date(parse_date(start_date)?)
        .with_maturity_date(parse_date(maturity_date)?)
        .with_notional(notional)
        .with_pay_market_index(parse_index(pay_index)?)
        .with_receive_market_index(parse_index(receive_index)?)
        .with_currency(parse_currency(currency)?)
        .with_pay_spread(pay_spread)
        .with_receive_spread(receive_spread)
        .with_pay_leg_frequency(parse_frequency(pay_frequency)?)
        .with_receive_leg_frequency(parse_frequency(receive_frequency)?)
        .with_side(side)
        .build()
        .map_err(value_error)?;
    let object = InstrumentObject {
        native: NativeInstrument::BasisSwap(instrument),
        notional,
        foreign_notional: None,
    };
    with_registry_mut(|registry| {
        registry.upsert_if_changed(name, QsObject::Instrument(Box::new(object)), key)
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.INSTRUMENT.XCCY.FLOAT.FLOAT",
    category = "QuantSupport - Instruments",
    help = "Creates a native FloatFloatCrossCurrencySwap contract; use QS.TRADE.CREATE for a position"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_instrument_xccy_float_float(
    name: &str,
    start_date: &str,
    maturity_date: &str,
    domestic_currency: &str,
    foreign_currency: &str,
    domestic_index: &str,
    foreign_index: &str,
    domestic_notional: f64,
    foreign_notional: f64,
    domestic_spread: f64,
    foreign_spread: f64,
    domestic_frequency: &str,
    foreign_frequency: &str,
) -> Result<String, XllError> {
    let key = input_key(&(
        start_date,
        maturity_date,
        domestic_currency,
        foreign_currency,
        domestic_index,
        foreign_index,
        domestic_notional,
        foreign_notional,
        domestic_spread,
        foreign_spread,
        domestic_frequency,
        foreign_frequency,
    ))?;
    if let Some(handle) = cached_handle(name, "FloatFloatCrossCurrencySwap", &key)? {
        return Ok(handle);
    }
    let side = Side::LongReceive;
    let instrument = MakeFloatFloatCrossCurrencySwap::<DualFwd>::default()
        .with_identifier(name.to_string())
        .with_start_date(parse_date(start_date)?)
        .with_maturity_date(parse_date(maturity_date)?)
        .with_domestic_currency(parse_currency(domestic_currency)?)
        .with_foreign_currency(parse_currency(foreign_currency)?)
        .with_domestic_market_index(parse_index(domestic_index)?)
        .with_foreign_market_index(parse_index(foreign_index)?)
        .with_domestic_notional(domestic_notional)
        .with_foreign_notional(foreign_notional)
        .with_domestic_spread(domestic_spread)
        .with_foreign_spread(foreign_spread)
        .with_domestic_leg_frequency(parse_frequency(domestic_frequency)?)
        .with_foreign_leg_frequency(parse_frequency(foreign_frequency)?)
        .with_side(side)
        .build()
        .map_err(value_error)?;
    let object = InstrumentObject {
        native: NativeInstrument::FloatFloatCrossCurrencySwap(instrument),
        notional: domestic_notional,
        foreign_notional: Some(foreign_notional),
    };
    with_registry_mut(|registry| {
        registry.upsert_if_changed(name, QsObject::Instrument(Box::new(object)), key)
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.INSTRUMENT.XCCY.FIX.FLOAT",
    category = "QuantSupport - Instruments",
    help = "Creates a native FixFloatCrossCurrencySwap contract; use QS.TRADE.CREATE for a position"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_instrument_xccy_fix_float(
    name: &str,
    start_date: &str,
    maturity_date: &str,
    domestic_currency: &str,
    foreign_currency: &str,
    foreign_index: &str,
    domestic_notional: f64,
    foreign_notional: f64,
    fixed_rate: f64,
    foreign_spread: f64,
    day_counter: &str,
    compounding: &str,
    rate_frequency: &str,
    domestic_frequency: &str,
    foreign_frequency: &str,
) -> Result<String, XllError> {
    let key = input_key(&(
        start_date,
        maturity_date,
        domestic_currency,
        foreign_currency,
        foreign_index,
        domestic_notional,
        foreign_notional,
        fixed_rate,
        foreign_spread,
        day_counter,
        compounding,
        rate_frequency,
        domestic_frequency,
        foreign_frequency,
    ))?;
    if let Some(handle) = cached_handle(name, "FixFloatCrossCurrencySwap", &key)? {
        return Ok(handle);
    }
    let side = Side::LongReceive;
    let instrument = MakeFixFloatCrossCurrencySwap::<DualFwd>::default()
        .with_identifier(name.to_string())
        .with_start_date(parse_date(start_date)?)
        .with_maturity_date(parse_date(maturity_date)?)
        .with_domestic_currency(parse_currency(domestic_currency)?)
        .with_foreign_currency(parse_currency(foreign_currency)?)
        .with_floating_index(parse_index(foreign_index)?)
        .with_domestic_notional(domestic_notional)
        .with_foreign_notional(foreign_notional)
        .with_fixed_rate(fixed_rate)
        .with_spread(foreign_spread)
        .with_rate_definition(RateDefinition::new(
            parse_day_counter(day_counter)?,
            parse_compounding(compounding)?,
            parse_frequency(rate_frequency)?,
        ))
        .with_domestic_leg_frequency(parse_frequency(domestic_frequency)?)
        .with_foreign_leg_frequency(parse_frequency(foreign_frequency)?)
        .with_side(side)
        .build()
        .map_err(value_error)?;
    let object = InstrumentObject {
        native: NativeInstrument::FixFloatCrossCurrencySwap(instrument),
        notional: domestic_notional,
        foreign_notional: Some(foreign_notional),
    };
    with_registry_mut(|registry| {
        registry.upsert_if_changed(name, QsObject::Instrument(Box::new(object)), key)
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.INSTRUMENT.FIXED.RATE.BOND",
    category = "QuantSupport - Instruments",
    help = "Creates a native FixedRateBond contract; use QS.TRADE.CREATE for a position"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_instrument_fixed_bond(
    name: &str,
    start_date: &str,
    maturity_date: &str,
    currency: &str,
    discount_index: Option<String>,
    notional: f64,
    coupon_rate: f64,
    day_counter: &str,
    compounding: &str,
    rate_frequency: &str,
    payment_frequency: &str,
) -> Result<String, XllError> {
    let key = input_key(&(
        start_date,
        maturity_date,
        currency,
        discount_index.as_deref(),
        notional,
        coupon_rate,
        day_counter,
        compounding,
        rate_frequency,
        payment_frequency,
    ))?;
    if let Some(handle) = cached_handle(name, "FixedRateBond", &key)? {
        return Ok(handle);
    }
    let mut builder = MakeFixedRateBond::<DualFwd>::default()
        .with_identifier(name.to_string())
        .with_start_date(parse_date(start_date)?)
        .with_maturity_date(parse_date(maturity_date)?)
        .with_rate(coupon_rate)
        .with_notional(notional)
        .with_rate_definition(RateDefinition::new(
            parse_day_counter(day_counter)?,
            parse_compounding(compounding)?,
            parse_frequency(rate_frequency)?,
        ))
        .with_currency(parse_currency(currency)?)
        .with_payment_frequency(parse_frequency(payment_frequency)?)
        .with_payment_structure(PaymentStructure::Bullet);
    if let Some(index) = discount_index
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        builder = builder.with_discount_index(parse_index(index)?);
    }
    let instrument = builder.build().map_err(value_error)?;
    let object = InstrumentObject {
        native: NativeInstrument::FixedRateBond(instrument),
        notional,
        foreign_notional: None,
    };
    with_registry_mut(|registry| {
        registry.upsert_if_changed(name, QsObject::Instrument(Box::new(object)), key)
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.INSTRUMENT.FIXED.RATE.DEPOSIT",
    category = "QuantSupport - Instruments",
    help = "Creates a native FixedRateDeposit contract; use QS.TRADE.CREATE for a position"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_instrument_fixed_deposit(
    name: &str,
    start_date: &str,
    maturity_date: &str,
    currency: &str,
    discount_index: Option<String>,
    notional: f64,
    rate: f64,
    day_counter: &str,
    compounding: &str,
    rate_frequency: &str,
) -> Result<String, XllError> {
    let key = input_key(&(
        start_date,
        maturity_date,
        currency,
        discount_index.as_deref(),
        notional,
        rate,
        day_counter,
        compounding,
        rate_frequency,
    ))?;
    if let Some(handle) = cached_handle(name, "FixedRateDeposit", &key)? {
        return Ok(handle);
    }
    let side = Side::LongReceive;
    let mut builder = MakeFixedRateDeposit::<DualFwd>::default()
        .with_identifier(name.to_string())
        .with_start_date(parse_date(start_date)?)
        .with_maturity_date(parse_date(maturity_date)?)
        .with_rate(rate)
        .with_notional(notional)
        .with_rate_definition(RateDefinition::new(
            parse_day_counter(day_counter)?,
            parse_compounding(compounding)?,
            parse_frequency(rate_frequency)?,
        ))
        .with_currency(parse_currency(currency)?)
        .with_side(side);
    if let Some(index) = discount_index
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        builder = builder.with_discount_index(Some(parse_index(index)?));
    }
    let instrument = builder.build().map_err(value_error)?;
    let object = InstrumentObject {
        native: NativeInstrument::FixedRateDeposit(instrument),
        notional,
        foreign_notional: None,
    };
    with_registry_mut(|registry| {
        registry.upsert_if_changed(name, QsObject::Instrument(Box::new(object)), key)
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.INSTRUMENT.FLOATING.RATE.NOTE",
    category = "QuantSupport - Instruments",
    help = "Creates a native FloatingRateNote contract; use QS.TRADE.CREATE for a position"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_instrument_floating_note(
    name: &str,
    start_date: &str,
    maturity_date: &str,
    currency: &str,
    forward_index: &str,
    notional: f64,
    spread: f64,
    payment_frequency: &str,
) -> Result<String, XllError> {
    let key = input_key(&(
        start_date,
        maturity_date,
        currency,
        forward_index,
        notional,
        spread,
        payment_frequency,
    ))?;
    if let Some(handle) = cached_handle(name, "FloatingRateNote", &key)? {
        return Ok(handle);
    }
    let side = Side::LongReceive;
    let instrument = MakeFloatingRateNote::<DualFwd>::default()
        .with_identifier(name.to_string())
        .with_start_date(parse_date(start_date)?)
        .with_maturity_date(parse_date(maturity_date)?)
        .with_currency(parse_currency(currency)?)
        .with_forward_index(parse_index(forward_index)?)
        .with_notional(notional)
        .with_spread(spread)
        .with_payment_frequency(parse_frequency(payment_frequency)?)
        .with_payment_structure(PaymentStructure::Bullet)
        .with_side(side)
        .build()
        .map_err(value_error)?;
    let object = InstrumentObject {
        native: NativeInstrument::FloatingRateNote(instrument),
        notional,
        foreign_notional: None,
    };
    with_registry_mut(|registry| {
        registry.upsert_if_changed(name, QsObject::Instrument(Box::new(object)), key)
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.INSTRUMENT.FX.OPTION",
    category = "QuantSupport - Instruments",
    help = "Creates a native FxEuropeanOption contract; use QS.TRADE.CREATE for a position"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_instrument_fx_option(
    name: &str,
    expiry_date: &str,
    base_currency: &str,
    quote_currency: &str,
    strike: f64,
    option_type: &str,
    day_counter: &str,
) -> Result<String, XllError> {
    let key = input_key(&(
        expiry_date,
        base_currency,
        quote_currency,
        strike,
        option_type,
        day_counter,
    ))?;
    if let Some(handle) = cached_handle(name, "FxEuropeanOption", &key)? {
        return Ok(handle);
    }
    let base = parse_currency(base_currency)?;
    let quote = parse_currency(quote_currency)?;
    let instrument = MakeFxEuropeanOption::default()
        .with_identifier(name.to_string())
        .with_expiry_date(parse_date(expiry_date)?)
        .with_strike(strike)
        .with_option_type(parse_option_type(option_type)?)
        .with_base_currency(base)
        .with_quote_currency(quote)
        .with_pair(FxPair::new(base, quote).map_err(value_error)?)
        .with_day_counter(parse_day_counter(day_counter)?)
        .build()
        .map_err(value_error)?;
    let object = InstrumentObject {
        native: NativeInstrument::FxEuropeanOption(instrument),
        notional: 1.0,
        foreign_notional: None,
    };
    with_registry_mut(|registry| {
        registry.upsert_if_changed(name, QsObject::Instrument(Box::new(object)), key)
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.INSTRUMENT.EQUITY.OPTION",
    category = "QuantSupport - Instruments",
    help = "Creates a native EquityEuropeanOption contract; use QS.TRADE.CREATE for a position"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_instrument_equity_option(
    name: &str,
    expiry_date: &str,
    market_index: &str,
    currency: &str,
    strike: f64,
    option_type: &str,
    day_counter: &str,
) -> Result<String, XllError> {
    let key = input_key(&(
        expiry_date,
        market_index,
        currency,
        strike,
        option_type,
        day_counter,
    ))?;
    if let Some(handle) = cached_handle(name, "EquityEuropeanOption", &key)? {
        return Ok(handle);
    }
    let instrument = MakeEquityEuropeanOption::default()
        .with_identifier(name.to_string())
        .with_expiry_date(parse_date(expiry_date)?)
        .with_market_index(parse_index(market_index)?)
        .with_currency(parse_currency(currency)?)
        .with_strike(strike)
        .with_option_type(parse_option_type(option_type)?)
        .with_day_counter(parse_day_counter(day_counter)?)
        .build()
        .map_err(value_error)?;
    let object = InstrumentObject {
        native: NativeInstrument::EquityEuropeanOption(instrument),
        notional: 1.0,
        foreign_notional: None,
    };
    with_registry_mut(|registry| {
        registry.upsert_if_changed(name, QsObject::Instrument(Box::new(object)), key)
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.INSTRUMENT.CDS",
    category = "QuantSupport - Instruments",
    help = "Creates a native CreditDefaultSwap contract; use QS.TRADE.CREATE for a position"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_instrument_cds(
    name: &str,
    start_date: &str,
    maturity_date: &str,
    credit_index: &str,
    discount_index: &str,
    currency: &str,
    spread: f64,
    recovery: f64,
    premium_frequency: &str,
    day_counter: &str,
) -> Result<String, XllError> {
    let key = input_key(&(
        start_date,
        maturity_date,
        credit_index,
        discount_index,
        currency,
        spread,
        recovery,
        premium_frequency,
        day_counter,
    ))?;
    if let Some(handle) = cached_handle(name, "CreditDefaultSwap", &key)? {
        return Ok(handle);
    }
    let instrument = CreditDefaultSwap::new(
        name.to_string(),
        parse_index(credit_index)?,
        parse_index(discount_index)?,
        parse_currency(currency)?,
        parse_date(start_date)?,
        parse_date(maturity_date)?,
        spread,
        recovery,
        parse_frequency(premium_frequency)?,
        parse_day_counter(day_counter)?,
    )
    .map_err(value_error)?;
    let object = InstrumentObject {
        native: NativeInstrument::CreditDefaultSwap(instrument),
        notional: 1.0,
        foreign_notional: None,
    };
    with_registry_mut(|registry| {
        registry.upsert_if_changed(name, QsObject::Instrument(Box::new(object)), key)
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.INSTRUMENT.CAP.FLOOR",
    category = "QuantSupport - Instruments",
    help = "Creates a native CapFloor contract; use QS.TRADE.CREATE for a position"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_instrument_cap_floor(
    name: &str,
    start_date: &str,
    maturity_date: &str,
    market_index: &str,
    currency: &str,
    strike: f64,
    cap_floor_type: &str,
    frequency: &str,
    day_counter: &str,
    compounding: &str,
    rate_frequency: &str,
) -> Result<String, XllError> {
    let key = input_key(&(
        start_date,
        maturity_date,
        market_index,
        currency,
        strike,
        cap_floor_type,
        frequency,
        day_counter,
        compounding,
        rate_frequency,
    ))?;
    if let Some(handle) = cached_handle(name, "CapFloor", &key)? {
        return Ok(handle);
    }
    let side = Side::LongReceive;
    let instrument = MakeCapFloor::default()
        .with_identifier(name.to_string())
        .with_start_date(parse_date(start_date)?)
        .with_maturity_date(parse_date(maturity_date)?)
        .with_market_index(parse_index(market_index)?)
        .with_currency(parse_currency(currency)?)
        .with_strike(strike)
        .with_cap_floor_type(parse_cap_floor_type(cap_floor_type)?)
        .with_notional(1.0)
        .with_frequency(parse_frequency(frequency)?)
        .with_rate_definition(RateDefinition::new(
            parse_day_counter(day_counter)?,
            parse_compounding(compounding)?,
            parse_frequency(rate_frequency)?,
        ))
        .with_side(side)
        .build()
        .map_err(value_error)?;
    let object = InstrumentObject {
        native: NativeInstrument::CapFloor(instrument),
        notional: 1.0,
        foreign_notional: None,
    };
    with_registry_mut(|registry| {
        registry.upsert_if_changed(name, QsObject::Instrument(Box::new(object)), key)
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.INSTRUMENT.CAPLET.FLOORLET",
    category = "QuantSupport - Instruments",
    help = "Creates a native CapletFloorlet contract; use QS.TRADE.CREATE for a position"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_instrument_caplet_floorlet(
    name: &str,
    fixing_date: &str,
    accrual_start: &str,
    accrual_end: &str,
    payment_date: &str,
    market_index: &str,
    currency: &str,
    strike: f64,
    caplet_floorlet_type: &str,
) -> Result<String, XllError> {
    let key = input_key(&(
        fixing_date,
        accrual_start,
        accrual_end,
        payment_date,
        market_index,
        currency,
        strike,
        caplet_floorlet_type,
    ))?;
    if let Some(handle) = cached_handle(name, "CapletFloorlet", &key)? {
        return Ok(handle);
    }
    let instrument = CapletFloorlet::new(
        name.to_string(),
        parse_index(market_index)?,
        parse_currency(currency)?,
        parse_date(fixing_date)?,
        parse_date(accrual_start)?,
        parse_date(accrual_end)?,
        parse_date(payment_date)?,
        parse_caplet_floorlet_type(caplet_floorlet_type)?,
        Strike::Absolute(strike),
    );
    let object = InstrumentObject {
        native: NativeInstrument::CapletFloorlet(instrument),
        notional: 1.0,
        foreign_notional: None,
    };
    with_registry_mut(|registry| {
        registry.upsert_if_changed(name, QsObject::Instrument(Box::new(object)), key)
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.INSTRUMENT.RATE.FUTURES",
    category = "QuantSupport - Instruments",
    help = "Creates a native RateFutures contract; use QS.TRADE.CREATE for a position"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_instrument_rate_future(
    name: &str,
    start_date: &str,
    end_date: &str,
    market_index: &str,
    futures_price: f64,
    contract_size: f64,
) -> Result<String, XllError> {
    let key = input_key(&(
        start_date,
        end_date,
        market_index,
        futures_price,
        contract_size,
    ))?;
    if let Some(handle) = cached_handle(name, "RateFutures", &key)? {
        return Ok(handle);
    }
    let side = Side::LongReceive;
    let instrument = MakeRateFutures::default()
        .with_identifier(name.to_string())
        .with_market_index(parse_index(market_index)?)
        .with_start_date(parse_date(start_date)?)
        .with_end_date(parse_date(end_date)?)
        .with_futures_price(futures_price)
        .with_contract_size(contract_size)
        .with_side(side)
        .build()
        .map_err(value_error)?;
    let object = InstrumentObject {
        native: NativeInstrument::RateFutures(instrument),
        notional: 1.0,
        foreign_notional: None,
    };
    with_registry_mut(|registry| {
        registry.upsert_if_changed(name, QsObject::Instrument(Box::new(object)), key)
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.INSTRUMENT.EQUITY.FORWARD",
    category = "QuantSupport - Instruments",
    help = "Creates a native EquityForward contract; use QS.TRADE.CREATE for a position"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_instrument_equity_forward(
    name: &str,
    delivery_date: &str,
    market_index: &str,
    currency: &str,
    strike: f64,
    day_counter: &str,
) -> Result<String, XllError> {
    let key = input_key(&(delivery_date, market_index, currency, strike, day_counter))?;
    if let Some(handle) = cached_handle(name, "EquityForward", &key)? {
        return Ok(handle);
    }
    let instrument = quantsupport::prelude::MakeEquityForward::default()
        .with_identifier(name.to_string())
        .with_market_index(parse_index(market_index)?)
        .with_delivery_date(parse_date(delivery_date)?)
        .with_currency(parse_currency(currency)?)
        .with_strike(strike)
        .with_day_counter(parse_day_counter(day_counter)?)
        .build()
        .map_err(value_error)?;
    let object = InstrumentObject {
        native: NativeInstrument::EquityForward(instrument),
        notional: 1.0,
        foreign_notional: None,
    };
    with_registry_mut(|registry| {
        registry.upsert_if_changed(name, QsObject::Instrument(Box::new(object)), key)
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.INSTRUMENT.EUROPEAN.SWAPTION",
    category = "QuantSupport - Instruments",
    help = "Creates a native EuropeanSwaption contract; use QS.TRADE.CREATE for a position"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_instrument_european_swaption(
    name: &str,
    expiry_date: &str,
    maturity_date: &str,
    currency: &str,
    market_index: &str,
    strike: f64,
    swaption_type: &str,
    day_counter: &str,
    compounding: &str,
    rate_frequency: &str,
    fixed_leg_frequency: &str,
    floating_leg_frequency: &str,
    calendar: &str,
) -> Result<String, XllError> {
    let key = input_key(&(
        expiry_date,
        maturity_date,
        currency,
        market_index,
        strike,
        swaption_type,
        day_counter,
        compounding,
        rate_frequency,
        fixed_leg_frequency,
        floating_leg_frequency,
        calendar,
    ))?;
    if let Some(handle) = cached_handle(name, "EuropeanSwaption", &key)? {
        return Ok(handle);
    }
    let expiry = parse_date(expiry_date)?;
    let maturity = parse_date(maturity_date)?;
    if expiry >= maturity {
        return Err(value_error("swaption maturity must follow expiry"));
    }
    let kind = match swaption_type {
        "Payer" => SwaptionType::Payer,
        "Receiver" => SwaptionType::Receiver,
        _ => return Err(value_error("swaption_type must be Payer or Receiver")),
    };
    let mut builder = quantsupport::prelude::MakeSwaption::<DualFwd>::default()
        .with_identifier(name.to_string())
        .with_expiry(expiry)
        .with_swap_tenor_date(maturity)
        .with_currency(parse_currency(currency)?)
        .with_market_index(parse_index(market_index)?)
        .with_strike(strike)
        .with_notional(1.0)
        .with_swaption_type(kind)
        .with_rate_definition(RateDefinition::new(
            parse_day_counter(day_counter)?,
            parse_compounding(compounding)?,
            parse_frequency(rate_frequency)?,
        ))
        .with_fixed_leg_frequency(parse_frequency(fixed_leg_frequency)?)
        .with_floating_leg_frequency(parse_frequency(floating_leg_frequency)?);
    if !calendar.is_empty() {
        builder = builder.with_calendar(
            quantsupport::prelude::Calendar::try_from(calendar.to_string()).map_err(value_error)?,
        );
    }
    let instrument = builder.build().map_err(value_error)?;
    let object = InstrumentObject {
        native: NativeInstrument::EuropeanSwaption(instrument),
        notional: 1.0,
        foreign_notional: None,
    };
    with_registry_mut(|registry| {
        registry.upsert_if_changed(name, QsObject::Instrument(Box::new(object)), key)
    })
    .map_err(value_error)
}
