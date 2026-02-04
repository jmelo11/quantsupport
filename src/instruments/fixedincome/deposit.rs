// use serde::{Deserialize, Serialize};

// use crate::{
//     core::{instrument::Instrument, pricingcontext::PricingContext, trade::Trade},
//     indices::marketindex::MarketIndex,
//     rates::interestrate::InterestRate,
//     time::date::Date,
//     utils::errors::Result,
// };

// /// # `Deposit`
// ///
// /// Represents a deposit instrument.
// #[derive(Clone, Debug, Serialize, Deserialize)]
// pub struct Deposit {
//     name: String,
//     units: f64,
//     rate: InterestRate,
//     maturity_date: Date,
//     is_resolved: bool,
//     issuer_name: Option<String>,
//     start_date: Option<Date>,
//     final_payment: Option<f64>,
// }

// impl Deposit {
//     /// Creates a new `Deposit`.
//     #[must_use]
//     pub fn new(
//         name: String,
//         units: f64,
//         rate: InterestRate,
//         start_date: Date,
//         maturity_date: Date,
//     ) -> Self {
//         Self {
//             name,
//             units,
//             rate,
//             start_date: Some(start_date),
//             maturity_date,
//             is_resolved: true,
//             issuer_name: None,
//             final_payment: None,
//         }
//     }

//     /// Sets the issuer name for the deposit.
//     #[must_use]
//     pub fn with_issuer_name(mut self, issuer_name: String) -> Self {
//         self.issuer_name = Some(issuer_name);
//         self
//     }

//     /// Returns the units of the deposit.
//     #[must_use]
//     pub const fn units(&self) -> f64 {
//         self.units
//     }

//     /// Returns the interest rate of the deposit.
//     #[must_use]
//     pub const fn rate(&self) -> InterestRate {
//         self.rate
//     }

//     /// Returns the start date of the deposit, if set.
//     #[must_use]
//     pub const fn start_date(&self) -> Option<Date> {
//         self.start_date
//     }

//     /// Returns the end date of the deposit.
//     #[must_use]
//     pub const fn maturity_date(&self) -> Date {
//         self.maturity_date
//     }

//     /// Returns the final payment of the deposit, if set.
//     #[must_use]
//     pub const fn final_payment(&self) -> Option<f64> {
//         self.final_payment
//     }
// }

// impl Instrument for Deposit {
//     fn identifier(&self) -> String {
//         self.name.clone()
//     }

//     fn is_resolved(&self) -> bool {
//         self.is_resolved
//     }

//     fn resolve(&mut self, ctx: &PricingContext) -> Result<()> {
//         Ok(())
//     }
// }

// /// # `DepositTrade`
// ///
// /// Represents a trade of a deposit instrument.
// #[derive(Clone, Debug, Serialize, Deserialize)]
// pub struct DepositTrade {
//     deposit: Deposit, // Arc<Deposit>?
//     market_index: MarketIndex,
//     trade_date: Date,
//     notional: f64,
//     trade_price: Option<f64>,
// }

// impl DepositTrade {
//     /// Creates a new `DepositTrade`.
//     #[must_use]
//     pub fn new(
//         deposit: Deposit,
//         market_index: MarketIndex,
//         trade_date: Date,
//         notional: f64,
//     ) -> Self {
//         Self {
//             deposit,
//             market_index,
//             trade_date,
//             notional,
//             trade_price: None,
//         }
//     }
//     /// Sets the trade price for the deposit trade.
//     #[must_use]
//     pub fn with_trade_price(mut self, trade_price: f64) -> Self {
//         self.trade_price = Some(trade_price);
//         self
//     }

//     /// Returns the trade date.
//     #[must_use]
//     pub const fn trade_date(&self) -> Date {
//         self.trade_date
//     }

//     /// Returns the notional amount of the trade.
//     #[must_use]
//     pub const fn notional(&self) -> f64 {
//         self.notional
//     }
// }

// impl Trade<Deposit> for DepositTrade {
//     fn instrument(&self) -> Deposit {
//         self.deposit.clone()
//     }
// }
