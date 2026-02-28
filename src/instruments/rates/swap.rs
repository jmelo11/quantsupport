// use serde::{Deserialize, Serialize};

// use crate::{
//     core::{
//         instrument::{AssetClass, Instrument},
//         trade::Trade,
//     },
//     indices::marketindex::MarketIndex,
//     rates::interestrate::InterestRate,
//     time::{date::Date, enums::Frequency, period::Period},
// };

// /// Defines swap direction for fixed leg payments.
// #[derive(Clone, Copy, Debug, Serialize, Deserialize)]
// pub enum SwapDirection {
//     /// Pay fixed, receive floating.
//     Pay,
//     /// Receive fixed, pay floating.
//     Receive,
// }

// /// Simple fixed-float interest rate swap.
// #[derive(Clone, Debug, Serialize, Deserialize)]
// pub struct InterestRateSwap {
//     name: String,
//     market_index: MarketIndex,
//     fixed_leg_frequency: Frequency,
//     start_date: Date,
//     maturity_date: Option<Date>,
//     tenor: Option<Period>,
// }

// impl InterestRateSwap {
//     /// Creates a new interest rate swap.
//     #[must_use]
//     pub const fn new(
//         name: String,
//         start_date: Date,
//         fixed_leg_frequency: Frequency,
//         market_index: MarketIndex,
//     ) -> Self {
//         Self {
//             name,
//             start_date,
//             fixed_leg_frequency,
//             market_index,
//             maturity_date: None,
//             tenor: None,
//         }
//     }

//     /// Returns the market index.
//     #[must_use]
//     pub fn market_index(&self) -> MarketIndex {
//         self.market_index.clone()
//     }
// }

// impl Instrument for InterestRateSwap {
//     fn identifier(&self) -> String {
//         self.name.clone()
//     }

//     fn asset_class(&self) -> AssetClass {
//         AssetClass::InterestRate
//     }
// }

// /// Trade representation for interest rate swaps.
// #[derive(Clone, Debug, Serialize, Deserialize)]
// pub struct InterestRateSwapTrade {
//     swap: InterestRateSwap,
//     trade_date: Date,
//     notional: f64,
//     rate: InterestRate<f64>,
// }

// impl InterestRateSwapTrade {
//     /// Creates a new swap trade.
//     #[must_use]
//     pub const fn new(
//         swap: InterestRateSwap,
//         trade_date: Date,
//         notional: f64,
//         rate: InterestRate<f64>,
//     ) -> Self {
//         Self {
//             swap,
//             trade_date,
//             notional,
//             rate,
//         }
//     }

//     /// Returns the trade date.
//     #[must_use]
//     pub const fn trade_date(&self) -> Date {
//         self.trade_date
//     }

//     /// Returns the trade notional.
//     #[must_use]
//     pub const fn notional(&self) -> f64 {
//         self.notional
//     }
// }

// impl Trade<InterestRateSwap> for InterestRateSwapTrade {
//     fn instrument(&self) -> &InterestRateSwap {
//         &self.swap
//     }

//     fn trade_date(&self) -> Date {
//         self.trade_date
//     }
// }
