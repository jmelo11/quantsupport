//! Owned Python views of native curve-risk transformations.

use std::borrow::Cow;

use pyo3::prelude::*;
use quantsupport::{
    core::marketdatahandling::marketview::MarketView,
    prelude::{
        Compounding, DayCounter, Frequency, MarketData, MarketDataProvider, MarketDataRequest,
        MarketIndex, PricingContext as QsPricingContext, Quote, QuoteDetails, QuoteSelector,
        RateDefinition, RiskContext as QsRiskContext, ZeroRateRisk as QsZeroRateRisk,
    },
};

use crate::{
    common::{
        conversion::{
            extract_compounding, extract_date, extract_day_counter, extract_frequency,
            extract_market_index, qs_err,
        },
        enums,
        errors::state_error,
    },
    pricing::context::PricingContext,
    time::Date,
};

/// Zero-rate reporting grid for one constructed discount or forwarding curve.
///
/// Defaults to the curve's future nodes and Actual365 / Continuous / Annual.
/// Supply dates for a different grid or for a curve without exposed nodes.
///
/// Parameters
/// ----------
/// market_index : MarketIndex or str
///     Discount or forwarding curve whose zero rates receive risk.
/// dates : sequence of Date, datetime.date or str, optional
///     Future reporting dates. None uses future curve nodes.
/// day_counter : DayCounter or str, optional
///     Year-fraction convention; None defaults to Actual365.
/// compounding : Compounding or str, optional
///     Compounding convention; None defaults to Continuous.
/// frequency : Frequency or str, optional
///     Compounding frequency; None defaults to Annual.
#[pyclass(module = "quantsupport", frozen, from_py_object)]
#[derive(Clone)]
pub struct ZeroRateRisk {
    index: MarketIndex,
    convention: RateDefinition,
    dates: Option<Vec<quantsupport::prelude::Date>>,
}

impl ZeroRateRisk {
    pub fn build(&self) -> QsZeroRateRisk {
        let grid = QsZeroRateRisk::new(self.index.clone(), self.convention);
        match &self.dates {
            Some(dates) => grid.with_dates(dates.clone()),
            None => grid,
        }
    }
}

#[pymethods]
impl ZeroRateRisk {
    #[new]
    #[pyo3(signature = (market_index, dates = None, day_counter = None, compounding = None, frequency = None))]
    fn new(
        market_index: &Bound<'_, PyAny>,
        dates: Option<Vec<Bound<'_, PyAny>>>,
        day_counter: Option<&Bound<'_, PyAny>>,
        compounding: Option<&Bound<'_, PyAny>>,
        frequency: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        Ok(Self {
            index: extract_market_index(market_index)?,
            convention: RateDefinition::new(
                day_counter
                    .map(extract_day_counter)
                    .transpose()?
                    .unwrap_or(DayCounter::Actual365),
                compounding
                    .map(extract_compounding)
                    .transpose()?
                    .unwrap_or(Compounding::Continuous),
                frequency
                    .map(extract_frequency)
                    .transpose()?
                    .unwrap_or(Frequency::Annual),
            ),
            dates: dates
                .map(|dates| dates.iter().map(extract_date).collect())
                .transpose()?,
        })
    }

    /// Curve whose zero rates receive risk.
    #[getter]
    fn market_index(&self) -> enums::MarketIndex {
        enums::MarketIndex {
            inner: self.index.clone(),
        }
    }

    /// Explicit grid, or None for future curve nodes. Returns a copy.
    #[getter]
    fn dates(&self) -> Option<Vec<Date>> {
        self.dates
            .as_ref()
            .map(|dates| dates.iter().map(|inner| Date { inner: *inner }).collect())
    }

    /// Day count used to quote the zero rates.
    #[getter]
    fn day_counter(&self) -> enums::DayCounter {
        self.convention.day_counter().into()
    }

    /// Zero-rate compounding convention.
    #[getter]
    fn compounding(&self) -> enums::Compounding {
        self.convention.compounding().into()
    }

    /// Zero-rate compounding frequency.
    #[getter]
    fn frequency(&self) -> enums::Frequency {
        self.convention.frequency().into()
    }

    fn __repr__(&self) -> String {
        format!(
            "ZeroRateRisk(market_index='{}', day_counter={:?}, compounding={:?}, frequency={:?})",
            self.index,
            self.convention.day_counter(),
            self.convention.compounding(),
            self.convention.frequency()
        )
    }
}

#[derive(Clone)]
pub enum RiskBasis {
    Instruments(Vec<QuoteDetails>),
    ZeroRates(Vec<QsZeroRateRisk>),
}

impl RiskBasis {
    pub fn build(&self, context: &QsPricingContext) -> PyResult<QsRiskContext<'static>> {
        match self {
            Self::Instruments(instruments) => context.reproject_risk(instruments),
            Self::ZeroRates(grids) => context.with_zero_rate_risk(grids),
        }
        .map(QsRiskContext::into_owned)
        .map_err(qs_err)
    }
}

/// PricingContext snapshot with a transformed curve sensitivity basis.
///
/// Created by PricingContext.reproject_risk() or with_zero_rate_risk(). Owns its
/// market inputs and supports evaluate(), portfolios, curve queries and context
/// management. Market inputs are read-only; build another view to change them.
/// Scenarios on this view reconstruct its basis from the shocked source quotes.
#[pyclass(module = "quantsupport", extends = PricingContext, unsendable)]
pub struct RiskContext;

#[pymethods]
impl RiskContext {
    #[new]
    fn new() -> PyResult<PyClassInitializer<Self>> {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "Use PricingContext.reproject_risk() or PricingContext.with_zero_rate_risk() to create a RiskContext",
        ))
    }

    /// Native curve-risk labels replaced by this view. Returns a copy.
    #[getter]
    fn source_labels(slf: PyRef<'_, Self>) -> PyResult<Vec<String>> {
        let context = slf.into_super();
        let view = context.market_view()?;
        Ok(view.projection()?.source_labels().to_vec())
    }

    /// Reporting labels in target quote order. Returns a copy.
    #[getter]
    fn target_labels(slf: PyRef<'_, Self>) -> PyResult<Vec<String>> {
        let context = slf.into_super();
        let view = context.market_view()?;
        Ok(view.projection()?.target_labels().to_vec())
    }

    /// Implied instrument quotes or decimal zero rates. Returns a copy.
    #[getter]
    fn target_values(slf: PyRef<'_, Self>) -> PyResult<Vec<f64>> {
        let context = slf.into_super();
        let view = context.market_view()?;
        Ok(view.projection()?.target_values().to_vec())
    }
}

/// The active source market with an optional, matching risk projection.
pub struct ContextMarketView<'a> {
    pub base: MarketView<'a>,
    pub risk: Option<Cow<'a, QsRiskContext<'static>>>,
}

impl ContextMarketView<'_> {
    fn projection(&self) -> PyResult<&QsRiskContext<'static>> {
        self.risk
            .as_deref()
            .ok_or_else(|| state_error("Context has no risk transformation"))
    }
}

impl QuoteSelector for ContextMarketView<'_> {
    fn select(&self, identifier: &str) -> Option<Quote> {
        self.base.select(identifier)
    }
    fn reference_date(&self) -> quantsupport::prelude::Date {
        QuoteSelector::reference_date(&self.base)
    }
}

impl MarketDataProvider for ContextMarketView<'_> {
    fn evaluation_date(&self) -> quantsupport::prelude::Date {
        self.base.evaluation_date()
    }
    fn anchor_fixings(&self) -> bool {
        self.base.anchor_fixings()
    }
    fn handle_request(
        &self,
        request: &MarketDataRequest,
    ) -> quantsupport::prelude::Result<MarketData> {
        let market = self.base.handle_request(request)?;
        match &self.risk {
            Some(risk) => risk.transform_market_data(market),
            None => Ok(market),
        }
    }
}
