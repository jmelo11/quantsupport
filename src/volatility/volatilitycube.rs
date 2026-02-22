use crate::{
    ad::adreal::IsReal,
    indices::marketindex::MarketIndex,
    time::{date::Date, enums::TimeUnit, period::Period},
    utils::errors::Result,
    volatility::volatilityindexing::{SmileType, VolatilityType},
};

pub trait VolatilityCube<T: IsReal> {
    /// Returns the volatility for a given expiry and key (e.g., strike, delta, log-moneyness).
    #[must_use]
    fn volatility_from_date(&self, expiry: Date, maturity: Period, key: f64) -> Result<T> {
        let today = self.reference_date();
        let days = expiry - today;
        let period = Period::new(days as i32, TimeUnit::Days);
        self.volatility_from_period(period, maturity, key)
    }

    /// Returns the volatility for a given time to expiry and key (e.g., strike, delta, log-moneyness).
    #[must_use]
    fn volatility_from_period(&self, expirty: Period, maturity: Period, key: f64) -> Result<T>;

    /// Returns the volatility type (e.g., Black, Normal).
    #[must_use]
    fn volatility_type(&self) -> VolatilityType;

    /// Returns the market index associated with the volatility surface.
    #[must_use]
    fn market_index(&self) -> &MarketIndex;

    /// Returns the reference date of the volatility surface.
    #[must_use]
    fn reference_date(&self) -> Date;

    #[must_use]
    fn smile_type(&self) -> SmileType;
}
