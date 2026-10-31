use crate::{
    ad::{dual::DualFwd, scalar::Scalar},
    core::marketdatahandling::marketdata::MarketData,
    rates::interestrate::{InterestRate, RateDefinition},
    utils::errors::{QSError, Result},
};
use crate::{indices::marketindex::MarketIndex, time::date::Date};

/// Selects the existing numeric or differentiable implied-rate implementation.
pub trait IndexRateScalar: Scalar + From<DualFwd> {
    /// Converts a growth factor to the index's rate convention.
    ///
    /// # Errors
    /// Rejects invalid growth factors or observation times.
    fn implied_index_rate(growth: Self, definition: RateDefinition, time: f64) -> Result<Self>;
}

impl IndexRateScalar for f64 {
    fn implied_index_rate(growth: Self, definition: RateDefinition, time: f64) -> Result<Self> {
        Ok(InterestRate::<Self>::implied_rate(
            growth,
            definition.day_counter(),
            definition.compounding(),
            definition.frequency(),
            time,
        )?
        .rate())
    }
}

impl IndexRateScalar for DualFwd {
    fn implied_index_rate(growth: Self, definition: RateDefinition, time: f64) -> Result<Self> {
        Ok(InterestRate::<Self>::implied_rate(
            growth,
            definition.day_counter(),
            definition.compounding(),
            definition.frequency(),
            time,
        )?
        .rate())
    }
}

/// Request for a forward rate between two dates for a given market index.
#[derive(Clone)]
pub struct ForwardRateRequest {
    market_index: MarketIndex,
    fixing_date: Date,
    start_date: Option<Date>,
    end_date: Option<Date>,
}

impl ForwardRateRequest {
    /// Resolves this observation from an already requested market response.
    ///
    /// When `anchor_fixings` is true, observations before evaluation require
    /// stored fixings. Other observations use the projection curve. False
    /// projects every observation using the index's native quote convention.
    /// Forecasts retain AD dependencies while stored fixings are constants.
    ///
    /// # Errors
    /// Rejects missing historical fixings, incomplete or invalid intervals,
    /// missing projection curves, and invalid implied-rate conversions.
    pub fn resolve_index_rate(
        &self,
        data: &MarketData,
        evaluation_date: Date,
        anchor_fixings: bool,
    ) -> Result<DualFwd> {
        self.resolve_index_rate_as(data, evaluation_date, anchor_fixings)
    }

    /// Converts market observations before calculating an index rate.
    ///
    /// # Errors
    /// Rejects invalid observation intervals and missing fixings or curves.
    pub fn resolve_index_rate_as<T: IndexRateScalar>(
        &self,
        data: &MarketData,
        evaluation_date: Date,
        anchor_fixings: bool,
    ) -> Result<T> {
        if anchor_fixings && self.fixing_date < evaluation_date {
            if let Some(fixing) = data
                .fixings()
                .get(&self.market_index)
                .and_then(|dates| dates.get(&self.fixing_date))
            {
                return Ok(T::scalar(*fixing));
            }
            return Err(QSError::NotFoundErr(format!(
                "Missing {} fixing for {}",
                self.market_index, self.fixing_date,
            )));
        }
        let start = self
            .start_date
            .ok_or_else(|| QSError::ValueNotSetErr("Index observation start".into()))?;
        let end = self
            .end_date
            .ok_or_else(|| QSError::ValueNotSetErr("Index observation end".into()))?;
        if end <= start {
            return Err(QSError::InvalidValueErr(
                "Index observation end must follow start".into(),
            ));
        }
        let element = data
            .constructed_elements()
            .discount_curves()
            .get(&self.market_index)
            .ok_or_else(|| {
                QSError::NotFoundErr(format!("Projection curve for {}", self.market_index))
            })?;
        let curve = element.curve();
        let growth =
            T::from(curve.discount_factor(start)?).div_val(T::from(curve.discount_factor(end)?));
        let definition = self.market_index.rate_index_details()?.rate_definition();
        T::implied_index_rate(
            growth,
            definition,
            definition.day_counter().year_fraction(start, end),
        )
    }

    /// Creates a new [`ForwardRateRequest`] with the specified market index and fixing date.
    #[must_use]
    pub const fn new(market_index: MarketIndex, fixing_date: Date) -> Self {
        Self {
            market_index,
            fixing_date,
            start_date: None,
            end_date: None,
        }
    }

    /// Sets the start date for the forward rate calculation.
    #[must_use]
    pub const fn with_start_date(mut self, start_date: Date) -> Self {
        self.start_date = Some(start_date);
        self
    }

    /// Sets the end date for the forward rate calculation.
    #[must_use]
    pub const fn with_end_date(mut self, end_date: Date) -> Self {
        self.end_date = Some(end_date);
        self
    }

    /// Returns the market index associated with this forward rate request.
    #[must_use]
    pub fn market_index(&self) -> MarketIndex {
        self.market_index.clone()
    }

    /// Returns the fixing date for which the forward rate is requested.
    #[must_use]
    pub const fn fixing_date(&self) -> Date {
        self.fixing_date
    }

    /// Returns the start date for the forward rate calculation, if set.
    #[must_use]
    pub const fn start_date(&self) -> Option<Date> {
        self.start_date
    }

    /// Returns the end date for the forward rate calculation, if set.
    #[must_use]
    pub const fn end_date(&self) -> Option<Date> {
        self.end_date
    }
}
