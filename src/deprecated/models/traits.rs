use crate::{
    core::meta::{
        DiscountFactorRequest, ExchangeRateRequest, ForwardRateRequest, MarketData,
        MarketRequest,
    },
    time::date::Date,
    utils::errors::Result,
};

/// # `Model`
/// A model that provides market data based in the current market state.
pub trait Model {
    /// Returns the reference date for the model.
    fn reference_date(&self) -> Date;
    /// Generates discount factor data based on the provided request.
    ///
    /// # Errors
    /// Returns an error if the model cannot produce the requested discount factor.
    fn gen_df_data(&self, df: DiscountFactorRequest) -> Result<f64>;
    /// Generates exchange rate data based on the provided request.
    ///
    /// # Errors
    /// Returns an error if the model cannot produce the requested exchange rate.
    fn gen_fx_data(&self, fx: ExchangeRateRequest) -> Result<f64>;
    /// Generates forward rate data based on the provided request.
    ///
    /// # Errors
    /// Returns an error if the model cannot produce the requested forward rate.
    fn gen_fwd_data(&self, fwd: ForwardRateRequest) -> Result<f64>;
    /// Generates numeraire data based on the provided market request.
    ///
    /// # Errors
    /// Returns an error if the model cannot produce the requested numeraire.
    fn gen_numerarie(&self, market_request: &MarketRequest) -> Result<f64>;
    /// Generates market data for a single market request.
    ///
    /// # Errors
    /// Returns an error if any required market data cannot be generated.
    fn gen_node(&self, market_request: &MarketRequest) -> Result<MarketData> {
        let id = market_request.id();
        let df = match market_request.df() {
            Some(df) => Some(self.gen_df_data(df)?),
            None => None,
        };

        let fwd = match market_request.fwd() {
            Some(fwd) => Some(self.gen_fwd_data(fwd)?),
            None => None,
        };

        let fx = match market_request.fx() {
            Some(fx) => Some(self.gen_fx_data(fx)?),
            None => None,
        };

        let numerarie = self.gen_numerarie(market_request)?;

        Ok(MarketData::new(
            id,
            self.reference_date(),
            df,
            fwd,
            fx,
            numerarie,
        ))
    }

    /// Generates market data for a slice of market requests.
    ///
    /// # Errors
    /// Returns an error if any request cannot be satisfied by the model.
    fn gen_market_data(&self, market_request: &[MarketRequest]) -> Result<Vec<MarketData>> {
        market_request.iter().map(|x| self.gen_node(x)).collect()
    }
}
