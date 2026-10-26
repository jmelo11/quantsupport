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

/// Keeps date and tenor worksheet inputs separate and configures the native maker.
fn apply_date_input<M>(
    maker: M,
    date: &str,
    tenor: Option<&str>,
    label: &str,
    set_date: impl FnOnce(M, Date) -> M,
    set_tenor: impl FnOnce(M, Period) -> M,
) -> Result<M, XllError> {
    let date = date.trim();
    let tenor = tenor.map(str::trim).filter(|s| !s.is_empty());
    match (date.is_empty(), tenor) {
        (false, None) => Ok(set_date(maker, parse_date(date)?)),
        (true, Some(tenor)) => Ok(set_tenor(
            maker,
            Period::from_str(tenor).map_err(value_error)?,
        )),
        (false, Some(_)) => Err(value_error(format!(
            "supply {label} or its tenor, not both"
        ))),
        (true, None) => Err(value_error(format!("{label} or its tenor is required"))),
    }
}

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
    delivery_tenor: Option<String>,
) -> Result<String, XllError> {
    let key = input_key(&(delivery_date, base_currency, quote_currency, forward_rate))?;
    let key = input_key(&(&key, (delivery_tenor.as_deref(),)))?;
    if let Some(handle) = cached_handle(name, "FxForward", &key)? {
        return Ok(handle);
    }
    let side = Side::LongReceive;
    let instrument = MakeFxForward::default()
        .with_identifier(name.to_string())
        .with_forward_rate(forward_rate)
        .with_base_currency(parse_currency(base_currency)?)
        .with_quote_currency(parse_currency(quote_currency)?)
        .with_side(side)
        .as_deliverable();
    let instrument = apply_date_input(
        instrument,
        delivery_date,
        delivery_tenor.as_deref(),
        "delivery_date",
        |m, d| m.with_delivery_date(d),
        |m, t| m.with_delivery_tenor(t),
    )?;
    let object = InstrumentObject::from_maker(
        "FxForward",
        1.0,
        None,
        instrument,
        NativeInstrument::FxForward,
    );
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
    start_tenor: Option<String>,
    tenor: Option<String>,
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
    let key = input_key(&(&key, (start_tenor.as_deref(), tenor.as_deref())))?;
    if let Some(handle) = cached_handle(name, "Swap", &key)? {
        return Ok(handle);
    }
    let side = Side::LongReceive;
    let instrument = MakeSwap::<DualFwd>::default()
        .with_identifier(name.to_string())
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
        .with_floating_leg_frequency(parse_frequency(floating_leg_frequency)?);
    let instrument = apply_date_input(
        instrument,
        start_date,
        start_tenor.as_deref(),
        "start_date",
        |m, d| m.with_start_date(d),
        |m, t| m.with_start_tenor(t),
    )?;
    let instrument = apply_date_input(
        instrument,
        maturity_date,
        tenor.as_deref(),
        "maturity_date",
        |m, d| m.with_maturity_date(d),
        |m, t| m.with_tenor(t),
    )?;
    let object =
        InstrumentObject::from_maker("Swap", notional, None, instrument, NativeInstrument::Swap);
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
    start_tenor: Option<String>,
    tenor: Option<String>,
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
    let key = input_key(&(&key, (start_tenor.as_deref(), tenor.as_deref())))?;
    if let Some(handle) = cached_handle(name, "BasisSwap", &key)? {
        return Ok(handle);
    }
    let side = Side::LongReceive;
    let instrument = MakeBasisSwap::<DualFwd>::default()
        .with_identifier(name.to_string())
        .with_notional(notional)
        .with_pay_market_index(parse_index(pay_index)?)
        .with_receive_market_index(parse_index(receive_index)?)
        .with_currency(parse_currency(currency)?)
        .with_pay_spread(pay_spread)
        .with_receive_spread(receive_spread)
        .with_pay_leg_frequency(parse_frequency(pay_frequency)?)
        .with_receive_leg_frequency(parse_frequency(receive_frequency)?)
        .with_side(side);
    let instrument = apply_date_input(
        instrument,
        start_date,
        start_tenor.as_deref(),
        "start_date",
        |m, d| m.with_start_date(d),
        |m, t| m.with_start_tenor(t),
    )?;
    let instrument = apply_date_input(
        instrument,
        maturity_date,
        tenor.as_deref(),
        "maturity_date",
        |m, d| m.with_maturity_date(d),
        |m, t| m.with_tenor(t),
    )?;
    let object = InstrumentObject::from_maker(
        "BasisSwap",
        notional,
        None,
        instrument,
        NativeInstrument::BasisSwap,
    );
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
    start_tenor: Option<String>,
    tenor: Option<String>,
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
    let key = input_key(&(&key, (start_tenor.as_deref(), tenor.as_deref())))?;
    if let Some(handle) = cached_handle(name, "FloatFloatCrossCurrencySwap", &key)? {
        return Ok(handle);
    }
    let side = Side::LongReceive;
    let instrument = MakeFloatFloatCrossCurrencySwap::<DualFwd>::default()
        .with_identifier(name.to_string())
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
        .with_side(side);
    let instrument = apply_date_input(
        instrument,
        start_date,
        start_tenor.as_deref(),
        "start_date",
        |m, d| m.with_start_date(d),
        |m, t| m.with_start_tenor(t),
    )?;
    let instrument = apply_date_input(
        instrument,
        maturity_date,
        tenor.as_deref(),
        "maturity_date",
        |m, d| m.with_maturity_date(d),
        |m, t| m.with_tenor(t),
    )?;
    let object = InstrumentObject::from_maker(
        "FloatFloatCrossCurrencySwap",
        domestic_notional,
        Some(foreign_notional),
        instrument,
        NativeInstrument::FloatFloatCrossCurrencySwap,
    );
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
    start_tenor: Option<String>,
    tenor: Option<String>,
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
    let key = input_key(&(&key, (start_tenor.as_deref(), tenor.as_deref())))?;
    if let Some(handle) = cached_handle(name, "FixFloatCrossCurrencySwap", &key)? {
        return Ok(handle);
    }
    let side = Side::LongReceive;
    let instrument = MakeFixFloatCrossCurrencySwap::<DualFwd>::default()
        .with_identifier(name.to_string())
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
        .with_side(side);
    let instrument = apply_date_input(
        instrument,
        start_date,
        start_tenor.as_deref(),
        "start_date",
        |m, d| m.with_start_date(d),
        |m, t| m.with_start_tenor(t),
    )?;
    let instrument = apply_date_input(
        instrument,
        maturity_date,
        tenor.as_deref(),
        "maturity_date",
        |m, d| m.with_maturity_date(d),
        |m, t| m.with_tenor(t),
    )?;
    let object = InstrumentObject::from_maker(
        "FixFloatCrossCurrencySwap",
        domestic_notional,
        Some(foreign_notional),
        instrument,
        NativeInstrument::FixFloatCrossCurrencySwap,
    );
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
    start_tenor: Option<String>,
    tenor: Option<String>,
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
    let key = input_key(&(&key, (start_tenor.as_deref(), tenor.as_deref())))?;
    if let Some(handle) = cached_handle(name, "FixedRateBond", &key)? {
        return Ok(handle);
    }
    let mut builder = MakeFixedRateBond::<DualFwd>::default()
        .with_identifier(name.to_string())
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
    let instrument = builder;
    let instrument = apply_date_input(
        instrument,
        start_date,
        start_tenor.as_deref(),
        "start_date",
        |m, d| m.with_start_date(d),
        |m, t| m.with_start_tenor(t),
    )?;
    let instrument = apply_date_input(
        instrument,
        maturity_date,
        tenor.as_deref(),
        "maturity_date",
        |m, d| m.with_maturity_date(d),
        |m, t| m.with_tenor(t),
    )?;
    let object = InstrumentObject::from_maker(
        "FixedRateBond",
        notional,
        None,
        instrument,
        NativeInstrument::FixedRateBond,
    );
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
    start_tenor: Option<String>,
    tenor: Option<String>,
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
    let key = input_key(&(&key, (start_tenor.as_deref(), tenor.as_deref())))?;
    if let Some(handle) = cached_handle(name, "FixedRateDeposit", &key)? {
        return Ok(handle);
    }
    let side = Side::LongReceive;
    let mut builder = MakeFixedRateDeposit::<DualFwd>::default()
        .with_identifier(name.to_string())
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
    let instrument = builder;
    let instrument = apply_date_input(
        instrument,
        start_date,
        start_tenor.as_deref(),
        "start_date",
        |m, d| m.with_start_date(d),
        |m, t| m.with_start_tenor(t),
    )?;
    let instrument = apply_date_input(
        instrument,
        maturity_date,
        tenor.as_deref(),
        "maturity_date",
        |m, d| m.with_maturity_date(d),
        |m, t| m.with_tenor(t),
    )?;
    let object = InstrumentObject::from_maker(
        "FixedRateDeposit",
        notional,
        None,
        instrument,
        NativeInstrument::FixedRateDeposit,
    );
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
    start_tenor: Option<String>,
    tenor: Option<String>,
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
    let key = input_key(&(&key, (start_tenor.as_deref(), tenor.as_deref())))?;
    if let Some(handle) = cached_handle(name, "FloatingRateNote", &key)? {
        return Ok(handle);
    }
    let side = Side::LongReceive;
    let instrument = MakeFloatingRateNote::<DualFwd>::default()
        .with_identifier(name.to_string())
        .with_currency(parse_currency(currency)?)
        .with_forward_index(parse_index(forward_index)?)
        .with_notional(notional)
        .with_spread(spread)
        .with_payment_frequency(parse_frequency(payment_frequency)?)
        .with_payment_structure(PaymentStructure::Bullet)
        .with_side(side);
    let instrument = apply_date_input(
        instrument,
        start_date,
        start_tenor.as_deref(),
        "start_date",
        |m, d| m.with_start_date(d),
        |m, t| m.with_start_tenor(t),
    )?;
    let instrument = apply_date_input(
        instrument,
        maturity_date,
        tenor.as_deref(),
        "maturity_date",
        |m, d| m.with_maturity_date(d),
        |m, t| m.with_tenor(t),
    )?;
    let object = InstrumentObject::from_maker(
        "FloatingRateNote",
        notional,
        None,
        instrument,
        NativeInstrument::FloatingRateNote,
    );
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
    expiry_tenor: Option<String>,
) -> Result<String, XllError> {
    let key = input_key(&(
        expiry_date,
        base_currency,
        quote_currency,
        strike,
        option_type,
        day_counter,
    ))?;
    let key = input_key(&(&key, (expiry_tenor.as_deref(),)))?;
    if let Some(handle) = cached_handle(name, "FxEuropeanOption", &key)? {
        return Ok(handle);
    }
    let base = parse_currency(base_currency)?;
    let quote = parse_currency(quote_currency)?;
    let instrument = MakeFxEuropeanOption::default()
        .with_identifier(name.to_string())
        .with_strike(strike)
        .with_option_type(parse_option_type(option_type)?)
        .with_base_currency(base)
        .with_quote_currency(quote)
        .with_pair(FxPair::new(base, quote).map_err(value_error)?)
        .with_day_counter(parse_day_counter(day_counter)?);
    let instrument = apply_date_input(
        instrument,
        expiry_date,
        expiry_tenor.as_deref(),
        "expiry_date",
        |m, d| m.with_expiry_date(d),
        |m, t| m.with_expiry_tenor(t),
    )?;
    let object = InstrumentObject::from_maker(
        "FxEuropeanOption",
        1.0,
        None,
        instrument,
        NativeInstrument::FxEuropeanOption,
    );
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
    expiry_tenor: Option<String>,
) -> Result<String, XllError> {
    let key = input_key(&(
        expiry_date,
        market_index,
        currency,
        strike,
        option_type,
        day_counter,
    ))?;
    let key = input_key(&(&key, (expiry_tenor.as_deref(),)))?;
    if let Some(handle) = cached_handle(name, "EquityEuropeanOption", &key)? {
        return Ok(handle);
    }
    let instrument = MakeEquityEuropeanOption::default()
        .with_identifier(name.to_string())
        .with_market_index(parse_index(market_index)?)
        .with_currency(parse_currency(currency)?)
        .with_strike(strike)
        .with_option_type(parse_option_type(option_type)?)
        .with_day_counter(parse_day_counter(day_counter)?);
    let instrument = apply_date_input(
        instrument,
        expiry_date,
        expiry_tenor.as_deref(),
        "expiry_date",
        |m, d| m.with_expiry_date(d),
        |m, t| m.with_expiry_tenor(t),
    )?;
    let object = InstrumentObject::from_maker(
        "EquityEuropeanOption",
        1.0,
        None,
        instrument,
        NativeInstrument::EquityEuropeanOption,
    );
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
    start_tenor: Option<String>,
    tenor: Option<String>,
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
    let key = input_key(&(&key, (start_tenor.as_deref(), tenor.as_deref())))?;
    if let Some(handle) = cached_handle(name, "CreditDefaultSwap", &key)? {
        return Ok(handle);
    }
    let instrument = MakeCreditDefaultSwap::default()
        .with_identifier(name.to_string())
        .with_credit_index(parse_index(credit_index)?)
        .with_discount_index(parse_index(discount_index)?)
        .with_currency(parse_currency(currency)?)
        .with_spread(spread)
        .with_recovery(recovery)
        .with_premium_frequency(parse_frequency(premium_frequency)?)
        .with_day_counter(parse_day_counter(day_counter)?);
    let instrument = apply_date_input(
        instrument,
        start_date,
        start_tenor.as_deref(),
        "start_date",
        |m, d| m.with_start_date(d),
        |m, t| m.with_start_tenor(t),
    )?;
    let instrument = apply_date_input(
        instrument,
        maturity_date,
        tenor.as_deref(),
        "maturity_date",
        |m, d| m.with_maturity_date(d),
        |m, t| m.with_tenor(t),
    )?;
    let object = InstrumentObject::from_maker(
        "CreditDefaultSwap",
        1.0,
        None,
        instrument,
        NativeInstrument::CreditDefaultSwap,
    );
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
    start_tenor: Option<String>,
    tenor: Option<String>,
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
    let key = input_key(&(&key, (start_tenor.as_deref(), tenor.as_deref())))?;
    if let Some(handle) = cached_handle(name, "CapFloor", &key)? {
        return Ok(handle);
    }
    let side = Side::LongReceive;
    let instrument = MakeCapFloor::default()
        .with_identifier(name.to_string())
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
        .with_side(side);
    let instrument = apply_date_input(
        instrument,
        start_date,
        start_tenor.as_deref(),
        "start_date",
        |m, d| m.with_start_date(d),
        |m, t| m.with_start_tenor(t),
    )?;
    let instrument = apply_date_input(
        instrument,
        maturity_date,
        tenor.as_deref(),
        "maturity_date",
        |m, d| m.with_maturity_date(d),
        |m, t| m.with_tenor(t),
    )?;
    let object = InstrumentObject::from_maker(
        "CapFloor",
        1.0,
        None,
        instrument,
        NativeInstrument::CapFloor,
    );
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
    start_tenor: Option<String>,
    tenor: Option<String>,
    fixing_tenor: Option<String>,
    payment_tenor: Option<String>,
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
    let key = input_key(&(
        &key,
        (
            start_tenor.as_deref(),
            tenor.as_deref(),
            fixing_tenor.as_deref(),
            payment_tenor.as_deref(),
        ),
    ))?;
    if let Some(handle) = cached_handle(name, "CapletFloorlet", &key)? {
        return Ok(handle);
    }
    let instrument = MakeCapletFloorlet::default()
        .with_identifier(name.to_string())
        .with_market_index(parse_index(market_index)?)
        .with_currency(parse_currency(currency)?)
        .with_strike(Strike::Absolute(strike))
        .with_payoff_type(parse_caplet_floorlet_type(caplet_floorlet_type)?);
    let instrument = apply_date_input(
        instrument,
        accrual_start,
        start_tenor.as_deref(),
        "accrual_start",
        |m, d| m.with_start_date(d),
        |m, t| m.with_start_tenor(t),
    )?;
    let instrument = apply_date_input(
        instrument,
        accrual_end,
        tenor.as_deref(),
        "accrual_end",
        |m, d| m.with_end_date(d),
        |m, t| m.with_tenor(t),
    )?;
    let instrument = apply_date_input(
        instrument,
        fixing_date,
        fixing_tenor.as_deref(),
        "fixing_date",
        |m, d| m.with_fixing_date(d),
        |m, t| m.with_fixing_tenor(t),
    )?;
    let instrument = apply_date_input(
        instrument,
        payment_date,
        payment_tenor.as_deref(),
        "payment_date",
        |m, d| m.with_payment_date(d),
        |m, t| m.with_payment_tenor(t),
    )?;
    let object = InstrumentObject::from_maker(
        "CapletFloorlet",
        1.0,
        None,
        instrument,
        NativeInstrument::CapletFloorlet,
    );
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
    start_tenor: Option<String>,
    tenor: Option<String>,
) -> Result<String, XllError> {
    let key = input_key(&(
        start_date,
        end_date,
        market_index,
        futures_price,
        contract_size,
    ))?;
    let key = input_key(&(&key, (start_tenor.as_deref(), tenor.as_deref())))?;
    if let Some(handle) = cached_handle(name, "RateFutures", &key)? {
        return Ok(handle);
    }
    let side = Side::LongReceive;
    let instrument = MakeRateFutures::default()
        .with_identifier(name.to_string())
        .with_market_index(parse_index(market_index)?)
        .with_futures_price(futures_price)
        .with_contract_size(contract_size)
        .with_side(side);
    let instrument = apply_date_input(
        instrument,
        start_date,
        start_tenor.as_deref(),
        "start_date",
        |m, d| m.with_start_date(d),
        |m, t| m.with_start_tenor(t),
    )?;
    let instrument = apply_date_input(
        instrument,
        end_date,
        tenor.as_deref(),
        "end_date",
        |m, d| m.with_end_date(d),
        |m, t| m.with_end_tenor(t),
    )?;
    let object = InstrumentObject::from_maker(
        "RateFutures",
        1.0,
        None,
        instrument,
        NativeInstrument::RateFutures,
    );
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
    delivery_tenor: Option<String>,
) -> Result<String, XllError> {
    let key = input_key(&(delivery_date, market_index, currency, strike, day_counter))?;
    let key = input_key(&(&key, (delivery_tenor.as_deref(),)))?;
    if let Some(handle) = cached_handle(name, "EquityForward", &key)? {
        return Ok(handle);
    }
    let instrument = quantsupport::prelude::MakeEquityForward::default()
        .with_identifier(name.to_string())
        .with_market_index(parse_index(market_index)?)
        .with_currency(parse_currency(currency)?)
        .with_strike(strike)
        .with_day_counter(parse_day_counter(day_counter)?);
    let instrument = apply_date_input(
        instrument,
        delivery_date,
        delivery_tenor.as_deref(),
        "delivery_date",
        |m, d| m.with_delivery_date(d),
        |m, t| m.with_delivery_tenor(t),
    )?;
    let object = InstrumentObject::from_maker(
        "EquityForward",
        1.0,
        None,
        instrument,
        NativeInstrument::EquityForward,
    );
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
    tenor: Option<String>,
    expiry_tenor: Option<String>,
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
    let key = input_key(&(&key, (tenor.as_deref(), expiry_tenor.as_deref())))?;
    if let Some(handle) = cached_handle(name, "EuropeanSwaption", &key)? {
        return Ok(handle);
    }
    let kind = match swaption_type {
        "Payer" => SwaptionType::Payer,
        "Receiver" => SwaptionType::Receiver,
        _ => return Err(value_error("swaption_type must be Payer or Receiver")),
    };
    let mut builder = quantsupport::prelude::MakeSwaption::<DualFwd>::default()
        .with_identifier(name.to_string())
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
    let instrument = builder;
    let instrument = apply_date_input(
        instrument,
        maturity_date,
        tenor.as_deref(),
        "maturity_date",
        |m, d| m.with_swap_tenor_date(d),
        |m, t| m.with_swap_tenor(t),
    )?;
    let instrument = apply_date_input(
        instrument,
        expiry_date,
        expiry_tenor.as_deref(),
        "expiry_date",
        |m, d| m.with_expiry(d),
        |m, t| m.with_expiry_tenor(t),
    )?;
    let object = InstrumentObject::from_maker(
        "EuropeanSwaption",
        1.0,
        None,
        instrument,
        NativeInstrument::EuropeanSwaption,
    );
    with_registry_mut(|registry| {
        registry.upsert_if_changed(name, QsObject::Instrument(Box::new(object)), key)
    })
    .map_err(value_error)
}
