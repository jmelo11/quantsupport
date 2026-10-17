use quantsupport::prelude::{
    BasisSwapTrade, CapFloorTrade, CapletFloorlet, CapletFloorletTrade, CdsTrade,
    CreditDefaultSwap, DualFwd, EquityEuropeanOptionTrade, FixFloatCrossCurrencySwapTrade,
    FixedRateBondTrade, FixedRateDepositTrade, FloatFloatCrossCurrencySwapTrade,
    FloatingRateNoteTrade, FxEuropeanOptionTrade, FxPair, MakeBasisSwap, MakeCapFloor,
    MakeEquityEuropeanOption, MakeFixFloatCrossCurrencySwap, MakeFixedRateBond,
    MakeFixedRateDeposit, MakeFloatFloatCrossCurrencySwap, MakeFloatingRateNote,
    MakeFxEuropeanOption, MakeFxForward, MakeRateFutures, MakeSwap, PaymentStructure,
    RateDefinition, RateFuturesTrade, Strike, SwapTrade,
};
use xll_rs::types::XllError;
use xllgen::xll_bindgen;

use crate::runtime::registry::{with_registry, with_registry_mut, QsObject};

use crate::functions::helpers::{
    input_key, parse_cap_floor_type, parse_caplet_floorlet_type, parse_compounding, parse_currency,
    parse_date, parse_day_counter, parse_frequency, parse_index, parse_option_type, parse_side,
    value_error,
};

fn cached_handle(name: &str, kind: &str, key: &str) -> Result<Option<String>, XllError> {
    with_registry(|registry| registry.handle_if_unchanged(name, kind, key)).map_err(value_error)
}

#[xll_bindgen(
    name = "QS.TRADE.FX.FORWARD",
    volatile,
    category = "QuantSupport - Instruments",
    help = "Creates a deliverable FX-forward trade"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_trade_fx_forward(
    name: &str,
    trade_date: &str,
    delivery_date: &str,
    base_currency: &str,
    quote_currency: &str,
    forward_rate: f64,
    notional: f64,
    side: &str,
) -> Result<String, XllError> {
    let definition_key = input_key(&(
        trade_date,
        delivery_date,
        base_currency,
        quote_currency,
        forward_rate.to_bits(),
        notional.to_bits(),
        side,
    ))?;
    if let Some(handle) = cached_handle(name, "FxForwardTrade", &definition_key)? {
        return Ok(handle);
    }
    let side = parse_side(side)?;
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
    let trade = quantsupport::prelude::FxForwardTrade::new(
        instrument,
        parse_date(trade_date)?,
        notional,
        side,
    );
    with_registry_mut(|registry| {
        registry.upsert_if_changed(
            name,
            QsObject::FxForwardTrade(Box::new(trade)),
            definition_key,
        )
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.TRADE.SWAP",
    volatile,
    category = "QuantSupport - Instruments",
    help = "Creates a vanilla fixed-versus-floating interest-rate swap trade"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_trade_swap(
    name: &str,
    trade_date: &str,
    start_date: &str,
    maturity_date: &str,
    currency: &str,
    market_index: &str,
    notional: f64,
    fixed_rate: f64,
    spread: f64,
    side: &str,
    day_counter: &str,
    compounding: &str,
    rate_frequency: &str,
    fixed_leg_frequency: &str,
    floating_leg_frequency: &str,
) -> Result<String, XllError> {
    let definition_key = input_key(&(
        trade_date,
        start_date,
        maturity_date,
        currency,
        market_index,
        notional.to_bits(),
        fixed_rate.to_bits(),
        spread.to_bits(),
        side,
        day_counter,
        compounding,
        rate_frequency,
        fixed_leg_frequency,
        floating_leg_frequency,
    ))?;
    if let Some(handle) = cached_handle(name, "SwapTrade", &definition_key)? {
        return Ok(handle);
    }
    let side = parse_side(side)?;
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
    let trade = SwapTrade::new(instrument, parse_date(trade_date)?, notional, side);
    with_registry_mut(|registry| {
        registry.upsert_if_changed(name, QsObject::SwapTrade(Box::new(trade)), definition_key)
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.TRADE.BASIS.SWAP",
    volatile,
    category = "QuantSupport - Instruments",
    help = "Creates a native floating-versus-floating BasisSwapTrade"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_trade_basis_swap(
    name: &str,
    trade_date: &str,
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
    side: &str,
) -> Result<String, XllError> {
    let definition_key = input_key(&(
        trade_date,
        start_date,
        maturity_date,
        currency,
        pay_index,
        receive_index,
        notional.to_bits(),
        pay_spread.to_bits(),
        receive_spread.to_bits(),
        pay_frequency,
        receive_frequency,
        side,
    ))?;
    if let Some(handle) = cached_handle(name, "BasisSwapTrade", &definition_key)? {
        return Ok(handle);
    }
    let side = parse_side(side)?;
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
    let trade = BasisSwapTrade::new(instrument, parse_date(trade_date)?, notional, side);
    with_registry_mut(|registry| {
        registry.upsert_if_changed(
            name,
            QsObject::BasisSwapTrade(Box::new(trade)),
            definition_key,
        )
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.TRADE.XCCY.FLOAT.FLOAT",
    volatile,
    category = "QuantSupport - Instruments",
    help = "Creates a native FloatFloatCrossCurrencySwapTrade"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_trade_xccy_float_float(
    name: &str,
    trade_date: &str,
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
    side: &str,
) -> Result<String, XllError> {
    let definition_key = input_key(&(
        trade_date,
        start_date,
        maturity_date,
        domestic_currency,
        foreign_currency,
        domestic_index,
        foreign_index,
        domestic_notional.to_bits(),
        foreign_notional.to_bits(),
        domestic_spread.to_bits(),
        foreign_spread.to_bits(),
        domestic_frequency,
        foreign_frequency,
        side,
    ))?;
    if let Some(handle) = cached_handle(name, "FloatFloatCrossCurrencySwapTrade", &definition_key)?
    {
        return Ok(handle);
    }
    let side = parse_side(side)?;
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
    let trade = FloatFloatCrossCurrencySwapTrade::new(
        instrument,
        parse_date(trade_date)?,
        domestic_notional,
        foreign_notional,
        side,
    );
    with_registry_mut(|registry| {
        registry.upsert_if_changed(
            name,
            QsObject::FloatFloatCrossCurrencySwapTrade(Box::new(trade)),
            definition_key,
        )
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.TRADE.XCCY.FIX.FLOAT",
    volatile,
    category = "QuantSupport - Instruments",
    help = "Creates a native FixFloatCrossCurrencySwapTrade"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_trade_xccy_fix_float(
    name: &str,
    trade_date: &str,
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
    side: &str,
) -> Result<String, XllError> {
    let definition_key = input_key(&(
        trade_date,
        start_date,
        maturity_date,
        domestic_currency,
        foreign_currency,
        foreign_index,
        domestic_notional.to_bits(),
        foreign_notional.to_bits(),
        fixed_rate.to_bits(),
        foreign_spread.to_bits(),
        day_counter,
        compounding,
        rate_frequency,
        domestic_frequency,
        foreign_frequency,
        side,
    ))?;
    if let Some(handle) = cached_handle(name, "FixFloatCrossCurrencySwapTrade", &definition_key)? {
        return Ok(handle);
    }
    let side = parse_side(side)?;
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
    let trade = FixFloatCrossCurrencySwapTrade::new(
        instrument,
        parse_date(trade_date)?,
        domestic_notional,
        foreign_notional,
        side,
    );
    with_registry_mut(|registry| {
        registry.upsert_if_changed(
            name,
            QsObject::FixFloatCrossCurrencySwapTrade(Box::new(trade)),
            definition_key,
        )
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.TRADE.FIXED.BOND",
    volatile,
    category = "QuantSupport - Instruments",
    help = "Creates a fixed-rate bond trade"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_trade_fixed_bond(
    name: &str,
    trade_date: &str,
    start_date: &str,
    maturity_date: &str,
    currency: &str,
    discount_index: &str,
    notional: f64,
    coupon_rate: f64,
    side: &str,
    day_counter: &str,
    compounding: &str,
    rate_frequency: &str,
    payment_frequency: &str,
    pricing_anchor: &str,
    anchor_value: f64,
) -> Result<String, XllError> {
    let definition_key = input_key(&(
        trade_date,
        start_date,
        maturity_date,
        currency,
        discount_index,
        notional.to_bits(),
        coupon_rate.to_bits(),
        side,
        day_counter,
        compounding,
        rate_frequency,
        payment_frequency,
        pricing_anchor,
        anchor_value.to_bits(),
    ))?;
    if let Some(handle) = cached_handle(name, "FixedRateBondTrade", &definition_key)? {
        return Ok(handle);
    }
    let anchor = match pricing_anchor {
        "CleanPrice" => quantsupport::prelude::BondPricingAnchor::CleanPrice(anchor_value),
        "DirtyPrice" => quantsupport::prelude::BondPricingAnchor::DirtyPrice(anchor_value),
        "ZSpread" => quantsupport::prelude::BondPricingAnchor::ZSpread(anchor_value),
        _ => {
            return Err(value_error(
                "fixed bonds require CleanPrice, DirtyPrice, or ZSpread",
            ))
        }
    };
    anchor.validate().map_err(value_error)?;
    let side = parse_side(side)?;
    let instrument = MakeFixedRateBond::<DualFwd>::default()
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
        .with_discount_index(parse_index(discount_index)?)
        .with_currency(parse_currency(currency)?)
        .with_payment_frequency(parse_frequency(payment_frequency)?)
        .with_payment_structure(PaymentStructure::Bullet)
        .build()
        .map_err(value_error)?;
    let trade =
        FixedRateBondTrade::new(instrument, parse_date(trade_date)?, notional, side, anchor);
    with_registry_mut(|registry| {
        registry.upsert_if_changed(
            name,
            QsObject::FixedRateBondTrade(Box::new(trade)),
            definition_key,
        )
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.TRADE.FIXED.DEPOSIT",
    volatile,
    category = "QuantSupport - Instruments",
    help = "Creates a fixed-rate deposit trade"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_trade_fixed_deposit(
    name: &str,
    trade_date: &str,
    start_date: &str,
    maturity_date: &str,
    currency: &str,
    discount_index: &str,
    notional: f64,
    rate: f64,
    side: &str,
    day_counter: &str,
    compounding: &str,
    rate_frequency: &str,
) -> Result<String, XllError> {
    let definition_key = input_key(&(
        trade_date,
        start_date,
        maturity_date,
        currency,
        discount_index,
        notional.to_bits(),
        rate.to_bits(),
        side,
        day_counter,
        compounding,
        rate_frequency,
    ))?;
    if let Some(handle) = cached_handle(name, "FixedRateDepositTrade", &definition_key)? {
        return Ok(handle);
    }
    let side = parse_side(side)?;
    let instrument = MakeFixedRateDeposit::<DualFwd>::default()
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
        .with_discount_index(Some(parse_index(discount_index)?))
        .with_currency(parse_currency(currency)?)
        .with_side(side)
        .build()
        .map_err(value_error)?;
    let trade = FixedRateDepositTrade::new(instrument, parse_date(trade_date)?, notional, side);
    with_registry_mut(|registry| {
        registry.upsert_if_changed(
            name,
            QsObject::FixedRateDepositTrade(Box::new(trade)),
            definition_key,
        )
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.TRADE.FLOATING.NOTE",
    volatile,
    category = "QuantSupport - Instruments",
    help = "Creates a native FloatingRateNoteTrade"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_trade_floating_note(
    name: &str,
    trade_date: &str,
    start_date: &str,
    maturity_date: &str,
    currency: &str,
    forward_index: &str,
    notional: f64,
    spread: f64,
    payment_frequency: &str,
    side: &str,
) -> Result<String, XllError> {
    let definition_key = input_key(&(
        trade_date,
        start_date,
        maturity_date,
        currency,
        forward_index,
        notional.to_bits(),
        spread.to_bits(),
        payment_frequency,
        side,
    ))?;
    if let Some(handle) = cached_handle(name, "FloatingRateNoteTrade", &definition_key)? {
        return Ok(handle);
    }
    let side = parse_side(side)?;
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
    let trade = FloatingRateNoteTrade::new(instrument, parse_date(trade_date)?, notional, side);
    with_registry_mut(|registry| {
        registry.upsert_if_changed(
            name,
            QsObject::FloatingRateNoteTrade(Box::new(trade)),
            definition_key,
        )
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.TRADE.FX.OPTION",
    volatile,
    category = "QuantSupport - Instruments",
    help = "Creates a native FxEuropeanOptionTrade"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_trade_fx_option(
    name: &str,
    trade_date: &str,
    expiry_date: &str,
    base_currency: &str,
    quote_currency: &str,
    strike: f64,
    option_type: &str,
    notional: f64,
    side: &str,
    day_counter: &str,
) -> Result<String, XllError> {
    let definition_key = input_key(&(
        trade_date,
        expiry_date,
        base_currency,
        quote_currency,
        strike.to_bits(),
        option_type,
        notional.to_bits(),
        side,
        day_counter,
    ))?;
    if let Some(handle) = cached_handle(name, "FxEuropeanOptionTrade", &definition_key)? {
        return Ok(handle);
    }
    let base = parse_currency(base_currency)?;
    let quote = parse_currency(quote_currency)?;
    let side = parse_side(side)?;
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
    let trade = FxEuropeanOptionTrade::new(instrument, parse_date(trade_date)?, notional, side);
    with_registry_mut(|registry| {
        registry.upsert_if_changed(
            name,
            QsObject::FxEuropeanOptionTrade(Box::new(trade)),
            definition_key,
        )
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.TRADE.EQUITY.OPTION",
    volatile,
    category = "QuantSupport - Instruments",
    help = "Creates a native EquityEuropeanOptionTrade"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_trade_equity_option(
    name: &str,
    trade_date: &str,
    expiry_date: &str,
    market_index: &str,
    currency: &str,
    strike: f64,
    option_type: &str,
    notional: f64,
    side: &str,
    day_counter: &str,
) -> Result<String, XllError> {
    let definition_key = input_key(&(
        trade_date,
        expiry_date,
        market_index,
        currency,
        strike.to_bits(),
        option_type,
        notional.to_bits(),
        side,
        day_counter,
    ))?;
    if let Some(handle) = cached_handle(name, "EquityEuropeanOptionTrade", &definition_key)? {
        return Ok(handle);
    }
    let side = parse_side(side)?;
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
    let trade = EquityEuropeanOptionTrade::new(instrument, notional, parse_date(trade_date)?, side);
    with_registry_mut(|registry| {
        registry.upsert_if_changed(
            name,
            QsObject::EquityEuropeanOptionTrade(Box::new(trade)),
            definition_key,
        )
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.TRADE.CDS",
    volatile,
    category = "QuantSupport - Instruments",
    help = "Creates a native CdsTrade; use a Credit:<entity> market index"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_trade_cds(
    name: &str,
    trade_date: &str,
    start_date: &str,
    maturity_date: &str,
    credit_index: &str,
    discount_index: &str,
    currency: &str,
    spread: f64,
    recovery: f64,
    premium_frequency: &str,
    day_counter: &str,
    notional: f64,
    side: &str,
) -> Result<String, XllError> {
    let definition_key = input_key(&(
        trade_date,
        start_date,
        maturity_date,
        credit_index,
        discount_index,
        currency,
        spread.to_bits(),
        recovery.to_bits(),
        premium_frequency,
        day_counter,
        notional.to_bits(),
        side,
    ))?;
    if let Some(handle) = cached_handle(name, "CdsTrade", &definition_key)? {
        return Ok(handle);
    }
    let side = parse_side(side)?;
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
    let trade = CdsTrade::new(instrument, parse_date(trade_date)?, notional, side);
    with_registry_mut(|registry| {
        registry.upsert_if_changed(name, QsObject::CdsTrade(Box::new(trade)), definition_key)
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.TRADE.CAP.FLOOR",
    volatile,
    category = "QuantSupport - Instruments",
    help = "Creates a native CapFloorTrade"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_trade_cap_floor(
    name: &str,
    trade_date: &str,
    start_date: &str,
    maturity_date: &str,
    market_index: &str,
    currency: &str,
    strike: f64,
    cap_floor_type: &str,
    notional: f64,
    frequency: &str,
    day_counter: &str,
    compounding: &str,
    rate_frequency: &str,
    side: &str,
) -> Result<String, XllError> {
    let definition_key = input_key(&(
        trade_date,
        start_date,
        maturity_date,
        market_index,
        currency,
        strike.to_bits(),
        cap_floor_type,
        notional.to_bits(),
        frequency,
        day_counter,
        compounding,
        rate_frequency,
        side,
    ))?;
    if let Some(handle) = cached_handle(name, "CapFloorTrade", &definition_key)? {
        return Ok(handle);
    }
    let side = parse_side(side)?;
    let instrument = MakeCapFloor::default()
        .with_identifier(name.to_string())
        .with_start_date(parse_date(start_date)?)
        .with_maturity_date(parse_date(maturity_date)?)
        .with_market_index(parse_index(market_index)?)
        .with_currency(parse_currency(currency)?)
        .with_strike(strike)
        .with_cap_floor_type(parse_cap_floor_type(cap_floor_type)?)
        .with_notional(notional)
        .with_frequency(parse_frequency(frequency)?)
        .with_rate_definition(RateDefinition::new(
            parse_day_counter(day_counter)?,
            parse_compounding(compounding)?,
            parse_frequency(rate_frequency)?,
        ))
        .with_side(side)
        .build()
        .map_err(value_error)?;
    let trade = CapFloorTrade::new(instrument, parse_date(trade_date)?, notional, side);
    with_registry_mut(|registry| {
        registry.upsert_if_changed(
            name,
            QsObject::CapFloorTrade(Box::new(trade)),
            definition_key,
        )
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.TRADE.CAPLET.FLOORLET",
    volatile,
    category = "QuantSupport - Instruments",
    help = "Creates a native CapletFloorletTrade"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_trade_caplet_floorlet(
    name: &str,
    trade_date: &str,
    fixing_date: &str,
    accrual_start: &str,
    accrual_end: &str,
    payment_date: &str,
    market_index: &str,
    currency: &str,
    strike: f64,
    caplet_floorlet_type: &str,
    notional: f64,
    side: &str,
) -> Result<String, XllError> {
    let definition_key = input_key(&(
        trade_date,
        fixing_date,
        accrual_start,
        accrual_end,
        payment_date,
        market_index,
        currency,
        strike.to_bits(),
        caplet_floorlet_type,
        notional.to_bits(),
        side,
    ))?;
    if let Some(handle) = cached_handle(name, "CapletFloorletTrade", &definition_key)? {
        return Ok(handle);
    }
    let side = parse_side(side)?;
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
    let trade = CapletFloorletTrade::new(instrument, parse_date(trade_date)?, notional, side);
    with_registry_mut(|registry| {
        registry.upsert_if_changed(
            name,
            QsObject::CapletFloorletTrade(Box::new(trade)),
            definition_key,
        )
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.TRADE.RATE.FUTURE",
    volatile,
    category = "QuantSupport - Instruments",
    help = "Creates an interest-rate futures trade"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_trade_rate_future(
    name: &str,
    trade_date: &str,
    start_date: &str,
    end_date: &str,
    market_index: &str,
    futures_price: f64,
    contract_size: f64,
    number_of_contracts: f64,
    side: &str,
) -> Result<String, XllError> {
    let definition_key = input_key(&(
        trade_date,
        start_date,
        end_date,
        market_index,
        futures_price.to_bits(),
        contract_size.to_bits(),
        number_of_contracts.to_bits(),
        side,
    ))?;
    if let Some(handle) = cached_handle(name, "RateFuturesTrade", &definition_key)? {
        return Ok(handle);
    }
    let side = parse_side(side)?;
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
    let trade = RateFuturesTrade::new(
        instrument,
        parse_date(trade_date)?,
        number_of_contracts,
        side,
    );
    with_registry_mut(|registry| {
        registry.upsert_if_changed(
            name,
            QsObject::RateFuturesTrade(Box::new(trade)),
            definition_key,
        )
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.TRADE.EQUITY.FORWARD",
    volatile,
    category = "QuantSupport - Instruments",
    help = "Creates a native equity-forward trade"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_trade_equity_forward(
    name: &str,
    trade_date: &str,
    delivery_date: &str,
    market_index: &str,
    currency: &str,
    strike: f64,
    notional: f64,
    side: &str,
    day_counter: &str,
) -> Result<String, XllError> {
    if !strike.is_finite() || !notional.is_finite() || notional < 0.0 {
        return Err(value_error(
            "strike must be finite and notional finite and non-negative",
        ));
    }
    let definition_key = input_key(&(
        trade_date,
        delivery_date,
        market_index,
        currency,
        strike.to_bits(),
        notional.to_bits(),
        side,
        day_counter,
    ))?;
    if let Some(handle) = cached_handle(name, "EquityForwardTrade", &definition_key)? {
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
    let trade = quantsupport::prelude::EquityForwardTrade::new(
        instrument,
        parse_date(trade_date)?,
        notional,
        parse_side(side)?,
    );
    with_registry_mut(|registry| {
        registry.upsert_if_changed(
            name,
            QsObject::EquityForwardTrade(Box::new(trade)),
            definition_key,
        )
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.TRADE.EUROPEAN.SWAPTION",
    volatile,
    category = "QuantSupport - Instruments",
    help = "Creates a European swaption; pricing uses the configured fixed or calibrated HullWhite model"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_trade_european_swaption(
    name: &str,
    trade_date: &str,
    expiry_date: &str,
    maturity_date: &str,
    currency: &str,
    market_index: &str,
    strike: f64,
    notional: f64,
    swaption_type: &str,
    side: &str,
    day_counter: &str,
    compounding: &str,
    rate_frequency: &str,
    fixed_leg_frequency: &str,
    floating_leg_frequency: &str,
    calendar: &str,
) -> Result<String, XllError> {
    if !strike.is_finite() || !notional.is_finite() || notional < 0.0 {
        return Err(value_error(
            "strike must be finite and notional finite and non-negative",
        ));
    }
    let expiry = parse_date(expiry_date)?;
    let maturity = parse_date(maturity_date)?;
    if expiry >= maturity {
        return Err(value_error("swaption maturity must follow expiry"));
    }
    let kind = match swaption_type {
        "Payer" => quantsupport::prelude::SwaptionType::Payer,
        "Receiver" => quantsupport::prelude::SwaptionType::Receiver,
        _ => return Err(value_error("swaption_type must be Payer or Receiver")),
    };
    // Nest tuples to stay within serde's tuple arity limit.
    let definition_key = input_key(&(
        (
            trade_date,
            expiry_date,
            maturity_date,
            currency,
            market_index,
            strike.to_bits(),
            notional.to_bits(),
            swaption_type,
            side,
        ),
        (
            day_counter,
            compounding,
            rate_frequency,
            fixed_leg_frequency,
            floating_leg_frequency,
            calendar,
        ),
    ))?;
    if let Some(handle) = cached_handle(name, "EuropeanSwaptionTrade", &definition_key)? {
        return Ok(handle);
    }
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
    let trade = quantsupport::prelude::EuropeanSwaptionTrade::new(
        instrument,
        parse_date(trade_date)?,
        notional,
        parse_side(side)?,
    );
    with_registry_mut(|registry| {
        registry.upsert_if_changed(
            name,
            QsObject::EuropeanSwaptionTrade(Box::new(trade)),
            definition_key,
        )
    })
    .map_err(value_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_trade_constructors_cache_and_revise_contract_inputs() {
        let forward = |strike| {
            qs_trade_equity_forward(
                "TEST-FWD",
                "2026-01-02",
                "2027-01-02",
                "EQ:SPX",
                "USD",
                strike,
                100.0,
                "LongReceive",
                "Actual365",
            )
            .unwrap()
        };
        let first = forward(100.0);
        assert_eq!(first, forward(100.0));
        let revised = forward(101.0);
        assert_ne!(first, revised);
        assert!(first.starts_with("QSObject:EquityForwardTrade:"));
        let swaption = |kind| {
            qs_trade_european_swaption(
                "TEST-SWPT",
                "2026-01-02",
                "2027-01-02",
                "2032-01-02",
                "USD",
                "SOFR",
                0.04,
                1000000.0,
                kind,
                "LongReceive",
                "Actual360",
                "Simple",
                "Annual",
                "Annual",
                "Quarterly",
                "JointCalendar(JoinHolidays,UnitedKingdom(Settlement),Japan)",
            )
            .unwrap()
        };
        let first = swaption("Payer");
        assert_eq!(first, swaption("Payer"));
        assert_ne!(first, swaption("Receiver"));
        assert!(first.starts_with("QSObject:EuropeanSwaptionTrade:"));
        assert!(qs_trade_equity_forward(
            "INVALID",
            "2026-01-02",
            "2027-01-02",
            "EQ:SPX",
            "USD",
            f64::NAN,
            100.0,
            "LongReceive",
            "Actual365"
        )
        .is_err());
    }
}
