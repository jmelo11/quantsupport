use crate::{
    currencies::enums::Currency,
    rates::enums::Compounding,
    time::{date::Date, enums::Frequency},
    utils::errors::{AtlasError, Result},
};

/// # `ExchangeRateRequest`
/// Meta data for an exchange rate. Holds the first currency, the second currency and the reference
/// date required to fetch the exchange rate.
///
/// ## Parameters
/// * `first_currency` - The first currency of the exchange rate.
/// * `second_currency` - The second currency of the exchange rate.
/// * `reference_date` - The reference date of the exchange rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExchangeRateRequest {
    first_currency: Currency,
    second_currency: Option<Currency>,
    reference_date: Option<Date>,
}

impl ExchangeRateRequest {
    /// Creates a new `ExchangeRateRequest`.
    #[must_use]
    pub const fn new(
        first_currency: Currency,
        second_currency: Option<Currency>,
        reference_date: Option<Date>,
    ) -> Self {
        Self {
            first_currency,
            second_currency,
            reference_date,
        }
    }

    /// Returns the first currency.
    #[must_use]
    pub const fn first_currency(&self) -> Currency {
        self.first_currency
    }

    /// Returns the second currency.
    #[must_use]
    pub const fn second_currency(&self) -> Option<Currency> {
        self.second_currency
    }

    /// Returns the reference date.
    #[must_use]
    pub const fn reference_date(&self) -> Option<Date> {
        self.reference_date
    }
}

/// # `DiscountFactorRequest`
/// Meta data for a discount factor. Holds the discount curve id and the reference date required to
/// fetch the discount factor.
///
/// ## Parameters
/// * `discount_curve_id` - The discount curve id of the discount factor.
/// * `date` - The reference date of the discount factor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiscountFactorRequest {
    provider_id: usize,
    date: Date,
}

impl DiscountFactorRequest {
    /// Creates a new `DiscountFactorRequest`.
    #[must_use]
    pub const fn new(provider_id: usize, date: Date) -> Self {
        Self { provider_id, date }
    }

    /// Returns the provider id.
    #[must_use]
    pub const fn provider_id(&self) -> usize {
        self.provider_id
    }

    /// Returns the date.
    #[must_use]
    pub const fn date(&self) -> Date {
        self.date
    }
}

/// # `ForwardRateRequest`
/// Meta data for a forward rate. Holds the forward curve id and the start and end dates required
/// to fetch the forward rate.
///
/// ## Parameters
/// * `provider_id` - The forward curve id of the forward rate.
/// * `start_date` - The start date of the forward rate.
/// * `end_date` - The end date of the forward rate.
/// * `compounding` - The compounding of the forward rate.
/// * `frequency` - The frequency of the forward rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForwardRateRequest {
    provider_id: usize,
    fixing_date: Date,
    start_date: Date,
    end_date: Date,
    compounding: Compounding,
    frequency: Frequency,
}

impl ForwardRateRequest {
    /// Creates a new `ForwardRateRequest`.
    #[must_use]
    pub const fn new(
        provider_id: usize,
        fixing_date: Date,
        start_date: Date,
        end_date: Date,
        compounding: Compounding,
        frequency: Frequency,
    ) -> Self {
        Self {
            provider_id,
            fixing_date,
            start_date,
            end_date,
            compounding,
            frequency,
        }
    }

    /// Returns the provider id.
    #[must_use]
    pub const fn provider_id(&self) -> usize {
        self.provider_id
    }

    /// Returns the start date.
    #[must_use]
    pub const fn start_date(&self) -> Date {
        self.start_date
    }

    /// Returns the end date.
    #[must_use]
    pub const fn end_date(&self) -> Date {
        self.end_date
    }

    /// Returns the compounding.
    #[must_use]
    pub const fn compounding(&self) -> Compounding {
        self.compounding
    }

    /// Returns the frequency.
    #[must_use]
    pub const fn frequency(&self) -> Frequency {
        self.frequency
    }
}

/// # `MarketRequest`
/// Meta data for market data. Holds all the meta data required to fetch the market data.
///
/// ## Parameters
/// * `id` - The id of the market data.
/// * `df` - The discount factor meta data.
/// * `fwd` - The forward rate meta data.
/// * `fx` - The exchange rate meta data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarketRequest {
    id: usize,
    df: Option<DiscountFactorRequest>,
    fwd: Option<ForwardRateRequest>,
    fx: Option<ExchangeRateRequest>,
}

impl MarketRequest {
    /// Creates a new `MarketRequest`.
    #[must_use]
    pub const fn new(
        id: usize,
        df: Option<DiscountFactorRequest>,
        fwd: Option<ForwardRateRequest>,
        fx: Option<ExchangeRateRequest>,
    ) -> Self {
        Self { id, df, fwd, fx }
    }

    /// Returns the id.
    #[must_use]
    pub const fn id(&self) -> usize {
        self.id
    }

    /// Returns the discount factor request.
    #[must_use]
    pub const fn df(&self) -> Option<DiscountFactorRequest> {
        self.df
    }

    /// Returns the forward rate request.
    #[must_use]
    pub const fn fwd(&self) -> Option<ForwardRateRequest> {
        self.fwd
    }

    /// Returns the exchange rate request.
    #[must_use]
    pub const fn fx(&self) -> Option<ExchangeRateRequest> {
        self.fx
    }
}

/// # `MarketDataNode`
/// Market data. Holds all the data required to price a cashflow.
///
/// ## Parameters
/// * `id` - The id of the market data.
/// * `df` - The discount factor.
/// * `fwd` - The forward rate.
/// * `fx` - The exchange rate.
#[derive(Debug, Clone, Copy)]
pub struct MarketData {
    id: usize,
    reference_date: Date,
    df: Option<f64>,
    fwd: Option<f64>,
    fx: Option<f64>,
    numerarie: f64,
}

impl MarketData {
    /// Creates a new `MarketData`.
    #[must_use]
    pub const fn new(
        id: usize,
        reference_date: Date,
        df: Option<f64>,
        fwd: Option<f64>,
        fx: Option<f64>,
        numerarie: f64,
    ) -> Self {
        Self {
            id,
            reference_date,
            df,
            fwd,
            fx,
            numerarie,
        }
    }

    /// Returns the id.
    #[must_use]
    pub const fn id(&self) -> usize {
        self.id
    }

    /// Returns the reference date.
    #[must_use]
    pub const fn reference_date(&self) -> Date {
        self.reference_date
    }

    /// Returns the discount factor.
    ///
    /// # Errors
    /// Returns an error if the discount factor was not set in this market data.
    pub fn df(&self) -> Result<f64> {
        self.df.ok_or(AtlasError::ValueNotSetErr("df".to_owned()))
    }

    /// Returns the forward rate.
    ///
    /// # Errors
    /// Returns an error if the forward rate was not set in this market data.
    pub fn fwd(&self) -> Result<f64> {
        self.fwd.ok_or(AtlasError::ValueNotSetErr("fwd".to_owned()))
    }

    /// Returns the exchange rate.
    ///
    /// # Errors
    /// Returns an error if the exchange rate was not set in this market data.
    pub fn fx(&self) -> Result<f64> {
        self.fx.ok_or(AtlasError::ValueNotSetErr("fx".to_owned()))
    }

    /// Returns the numeraire.
    #[must_use]
    pub const fn numerarie(&self) -> f64 {
        self.numerarie
    }
}
