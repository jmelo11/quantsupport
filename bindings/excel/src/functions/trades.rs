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

use crate::registry::{with_registry_mut, QsObject};

use super::helpers::{
    parse_cap_floor_type, parse_caplet_floorlet_type, parse_compounding, parse_currency,
    parse_date, parse_day_counter, parse_frequency, parse_index, parse_option_type, parse_side,
    value_error,
};

#[xll_bindgen(
    name = "QS.TRADE.FX.FORWARD",
    volatile,
    category = "QuantSupport - Trades",
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
    with_registry_mut(|registry| registry.upsert(name, QsObject::FxForwardTrade(Box::new(trade))))
        .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.TRADE.SWAP",
    volatile,
    category = "QuantSupport - Trades",
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
    with_registry_mut(|registry| registry.upsert(name, QsObject::SwapTrade(Box::new(trade))))
        .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.TRADE.BASIS.SWAP",
    volatile,
    category = "QuantSupport - Trades",
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
    with_registry_mut(|registry| registry.upsert(name, QsObject::BasisSwapTrade(Box::new(trade))))
        .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.TRADE.XCCY.FLOAT.FLOAT",
    volatile,
    category = "QuantSupport - Trades",
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
        registry.upsert(
            name,
            QsObject::FloatFloatCrossCurrencySwapTrade(Box::new(trade)),
        )
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.TRADE.XCCY.FIX.FLOAT",
    volatile,
    category = "QuantSupport - Trades",
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
        registry.upsert(
            name,
            QsObject::FixFloatCrossCurrencySwapTrade(Box::new(trade)),
        )
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.TRADE.FIXED.BOND",
    volatile,
    category = "QuantSupport - Trades",
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
) -> Result<String, XllError> {
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
        .with_side(side)
        .with_payment_frequency(parse_frequency(payment_frequency)?)
        .with_payment_structure(PaymentStructure::Bullet)
        .build()
        .map_err(value_error)?;
    let trade = FixedRateBondTrade::new(instrument, parse_date(trade_date)?, notional, side);
    with_registry_mut(|registry| {
        registry.upsert(name, QsObject::FixedRateBondTrade(Box::new(trade)))
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.TRADE.FIXED.DEPOSIT",
    volatile,
    category = "QuantSupport - Trades",
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
        registry.upsert(name, QsObject::FixedRateDepositTrade(Box::new(trade)))
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.TRADE.FLOATING.NOTE",
    volatile,
    category = "QuantSupport - Trades",
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
        registry.upsert(name, QsObject::FloatingRateNoteTrade(Box::new(trade)))
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.TRADE.FX.OPTION",
    volatile,
    category = "QuantSupport - Trades",
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
        registry.upsert(name, QsObject::FxEuropeanOptionTrade(Box::new(trade)))
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.TRADE.EQUITY.OPTION",
    volatile,
    category = "QuantSupport - Trades",
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
        registry.upsert(name, QsObject::EquityEuropeanOptionTrade(Box::new(trade)))
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.TRADE.CDS",
    volatile,
    category = "QuantSupport - Trades",
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
    with_registry_mut(|registry| registry.upsert(name, QsObject::CdsTrade(Box::new(trade))))
        .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.TRADE.CAP.FLOOR",
    volatile,
    category = "QuantSupport - Trades",
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
    with_registry_mut(|registry| registry.upsert(name, QsObject::CapFloorTrade(Box::new(trade))))
        .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.TRADE.CAPLET.FLOORLET",
    volatile,
    category = "QuantSupport - Trades",
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
        registry.upsert(name, QsObject::CapletFloorletTrade(Box::new(trade)))
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.TRADE.RATE.FUTURE",
    volatile,
    category = "QuantSupport - Trades",
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
    with_registry_mut(|registry| registry.upsert(name, QsObject::RateFuturesTrade(Box::new(trade))))
        .map_err(value_error)
}
