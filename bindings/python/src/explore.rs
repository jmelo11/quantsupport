//! Read-only views over the market data elements constructed by the
//! pricing context: bootstrapped discount curves, volatility surfaces and
//! cubes, and Monte Carlo simulations.

use pyo3::prelude::*;
use quantsupport::prelude::{
    DiscountCurveElement, MonteCarloSimulationElement, VolatilityCubeElement,
    VolatilitySurfaceElement,
};

use crate::conv::{
    dataframe, extract_compounding, extract_date, extract_frequency, extract_period, qs_err,
};
use crate::enums::{DayCounter, MarketIndex, SmileType, VolatilityType};
use crate::time::Date;

/// A discount curve constructed by the pricing context.
#[pyclass(module = "quantsupport", name = "DiscountCurve", unsendable)]
pub struct DiscountCurve {
    pub element: DiscountCurveElement,
}

#[pymethods]
impl DiscountCurve {
    /// Market index the curve discounts.
    #[getter]
    fn market_index(&self) -> MarketIndex {
        MarketIndex {
            inner: self.element.market_index().clone(),
        }
    }

    /// Curve reference date.
    #[getter]
    fn reference_date(&self) -> Date {
        Date {
            inner: self.element.curve().reference_date(),
        }
    }

    /// Day count convention of the curve, if available.
    #[getter]
    fn day_counter(&self) -> Option<DayCounter> {
        self.element.curve().day_counter().map(Into::into)
    }

    /// Discount factor at a date.
    fn discount_factor(&self, date: &Bound<'_, PyAny>) -> PyResult<f64> {
        Ok(self
            .element
            .curve()
            .discount_factor(extract_date(date)?)
            .map_err(qs_err)?
            .value())
    }

    /// Forward rate between two dates (default Simple / Annual).
    #[pyo3(signature = (start, end, compounding = None, frequency = None))]
    fn forward_rate(
        &self,
        start: &Bound<'_, PyAny>,
        end: &Bound<'_, PyAny>,
        compounding: Option<&Bound<'_, PyAny>>,
        frequency: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<f64> {
        let comp = compounding
            .map(extract_compounding)
            .transpose()?
            .unwrap_or(quantsupport::prelude::Compounding::Simple);
        let freq = frequency
            .map(extract_frequency)
            .transpose()?
            .unwrap_or(quantsupport::prelude::Frequency::Annual);
        Ok(self
            .element
            .curve()
            .forward_rate(extract_date(start)?, extract_date(end)?, comp, freq)
            .map_err(qs_err)?
            .value())
    }

    /// Curve nodes as a DataFrame with columns `date` and `discount_factor`.
    fn nodes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let nodes = self.element.curve().nodes().unwrap_or_default();
        let (dates, dfs): (Vec<String>, Vec<f64>) = nodes
            .into_iter()
            .map(|(d, df)| (d.to_string(), df.value()))
            .unzip();
        dataframe(
            py,
            &[
                ("date", dates.into_pyobject(py)?.into_any()),
                ("discount_factor", dfs.into_pyobject(py)?.into_any()),
            ],
        )
    }

    /// Curve pillars (bootstrap instruments) as a DataFrame with columns
    /// `label` and `value`.
    fn pillars<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let pillars = self.element.curve().pillars().map_or_else(Vec::new, |p| {
            p.into_iter().map(|(l, v)| (l, v.value())).collect()
        });
        let (labels, values): (Vec<String>, Vec<f64>) = pillars.into_iter().unzip();
        dataframe(
            py,
            &[
                ("label", labels.into_pyobject(py)?.into_any()),
                ("value", values.into_pyobject(py)?.into_any()),
            ],
        )
    }

    fn __repr__(&self) -> String {
        format!(
            "DiscountCurve(market_index='{}')",
            self.element.market_index()
        )
    }
}

/// A constructed volatility surface.
#[pyclass(module = "quantsupport", name = "VolatilitySurface", unsendable)]
pub struct VolatilitySurface {
    pub element: VolatilitySurfaceElement,
}

#[pymethods]
impl VolatilitySurface {
    /// Underlying market index.
    #[getter]
    fn market_index(&self) -> MarketIndex {
        MarketIndex {
            inner: self.element.market_index().clone(),
        }
    }

    /// Surface reference date.
    #[getter]
    fn reference_date(&self) -> Date {
        Date {
            inner: self.element.surface().reference_date(),
        }
    }

    /// Volatility quotation convention (Black or Normal).
    #[getter]
    fn volatility_type(&self) -> VolatilityType {
        self.element.surface().volatility_type().into()
    }

    /// Smile axis (Strike, Delta or LogMoneyness).
    #[getter]
    fn smile_type(&self) -> SmileType {
        self.element.surface().smile_type().into()
    }

    /// Volatility at an expiry (`Date`, ISO string, `Period` or period
    /// string) and smile coordinate `key`.
    fn volatility(&self, expiry: &Bound<'_, PyAny>, key: f64) -> PyResult<f64> {
        let surface = self.element.surface();
        if let Ok(date) = extract_date(expiry) {
            return Ok(surface
                .volatility_from_date(date, key)
                .map_err(qs_err)?
                .value());
        }
        let period = extract_period(expiry)?;
        Ok(surface
            .volatility_from_period(period, key)
            .map_err(qs_err)?
            .value())
    }

    /// Surface pillars as a DataFrame with columns `label` and `value`.
    fn pillars<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let pillars = self.element.surface().pillars().map_or_else(Vec::new, |p| {
            p.into_iter().map(|(l, v)| (l, v.value())).collect()
        });
        let (labels, values): (Vec<String>, Vec<f64>) = pillars.into_iter().unzip();
        dataframe(
            py,
            &[
                ("label", labels.into_pyobject(py)?.into_any()),
                ("value", values.into_pyobject(py)?.into_any()),
            ],
        )
    }

    fn __repr__(&self) -> String {
        format!(
            "VolatilitySurface(market_index='{}')",
            self.element.market_index()
        )
    }
}

/// A constructed volatility cube (expiry x underlying maturity x smile).
#[pyclass(module = "quantsupport", name = "VolatilityCube", unsendable)]
pub struct VolatilityCube {
    pub element: VolatilityCubeElement,
}

#[pymethods]
impl VolatilityCube {
    /// Underlying market index.
    #[getter]
    fn market_index(&self) -> MarketIndex {
        MarketIndex {
            inner: self.element.market_index().clone(),
        }
    }

    /// Cube reference date.
    #[getter]
    fn reference_date(&self) -> Date {
        Date {
            inner: self.element.cube().reference_date(),
        }
    }

    /// Volatility quotation convention (Black or Normal).
    #[getter]
    fn volatility_type(&self) -> VolatilityType {
        self.element.cube().volatility_type().into()
    }

    /// Smile axis (Strike, Delta or LogMoneyness).
    #[getter]
    fn smile_type(&self) -> SmileType {
        self.element.cube().smile_type().into()
    }

    /// Volatility at an expiry (`Date` or `Period`), underlying maturity
    /// (`Period`) and smile coordinate `key`.
    fn volatility(
        &self,
        expiry: &Bound<'_, PyAny>,
        maturity: &Bound<'_, PyAny>,
        key: f64,
    ) -> PyResult<f64> {
        let cube = self.element.cube();
        let maturity = extract_period(maturity)?;
        if let Ok(date) = extract_date(expiry) {
            return Ok(cube
                .volatility_from_date(date, maturity, key)
                .map_err(qs_err)?
                .value());
        }
        let period = extract_period(expiry)?;
        Ok(cube
            .volatility_from_period(period, maturity, key)
            .map_err(qs_err)?
            .value())
    }

    /// Cube pillars as a DataFrame with columns `label` and `value`.
    fn pillars<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let pillars = self.element.cube().pillars().map_or_else(Vec::new, |p| {
            p.into_iter().map(|(l, v)| (l, v.value())).collect()
        });
        let (labels, values): (Vec<String>, Vec<f64>) = pillars.into_iter().unzip();
        dataframe(
            py,
            &[
                ("label", labels.into_pyobject(py)?.into_any()),
                ("value", values.into_pyobject(py)?.into_any()),
            ],
        )
    }

    fn __repr__(&self) -> String {
        format!(
            "VolatilityCube(market_index='{}')",
            self.element.market_index()
        )
    }
}

/// A constructed Monte Carlo simulation.
#[pyclass(module = "quantsupport", name = "Simulation", unsendable)]
pub struct Simulation {
    pub element: MonteCarloSimulationElement,
}

#[pymethods]
impl Simulation {
    /// Hull-White calibration records as a DataFrame, or `None` for an uncalibrated model.
    /// Columns include market/model prices, sigma and the signed price error.
    fn calibration_diagnostics<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        let Some(quality) = self.element.calibration_quality() else {
            return Ok(None);
        };
        let records = &quality.records;
        let identifiers: Vec<_> = records.iter().map(|r| r.identifier.clone()).collect();
        let expiries: Vec<_> = records.iter().map(|r| r.expiry.to_string()).collect();
        let mut columns = vec![
            ("identifier", identifiers.into_pyobject(py)?.into_any()),
            ("expiry", expiries.into_pyobject(py)?.into_any()),
        ];
        for (name, values) in [
            ("t", records.iter().map(|r| r.t).collect::<Vec<_>>()),
            ("big_t", records.iter().map(|r| r.big_t).collect()),
            ("market_vol", records.iter().map(|r| r.market_vol).collect()),
            (
                "market_price",
                records.iter().map(|r| r.market_price).collect(),
            ),
            (
                "model_price",
                records.iter().map(|r| r.model_price).collect(),
            ),
            (
                "calibrated_sigma",
                records.iter().map(|r| r.calibrated_sigma).collect(),
            ),
            (
                "forward_rate",
                records.iter().map(|r| r.forward_rate).collect(),
            ),
            (
                "effective_strike",
                records.iter().map(|r| r.effective_strike).collect(),
            ),
            (
                "price_error",
                records
                    .iter()
                    .map(|r| r.model_price - r.market_price)
                    .collect(),
            ),
        ] {
            columns.push((name, values.into_pyobject(py)?.into_any()));
        }
        dataframe(py, &columns).map(Some)
    }

    /// Simulated market index.
    #[getter]
    fn market_index(&self) -> MarketIndex {
        MarketIndex {
            inner: self.element.market_index().clone(),
        }
    }

    /// Number of simulated paths.
    #[getter]
    fn n_paths(&self) -> i64 {
        self.element.simulation().borrow().n_paths()
    }

    /// Time step between simulation dates, in years.
    #[getter]
    fn dt(&self) -> f64 {
        self.element.simulation().borrow().dt()
    }

    /// Simulation dates.
    fn dates(&self) -> Vec<Date> {
        self.element
            .simulation()
            .borrow()
            .dates()
            .iter()
            .map(|&inner| Date { inner })
            .collect()
    }

    /// Simulated paths as a DataFrame: one row per path, one column per
    /// simulation date (ISO strings).
    fn paths<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let sim = self.element.simulation().borrow();
        let dates: Vec<String> = sim.dates().iter().map(ToString::to_string).collect();
        let paths = sim.path();
        let mut columns = Vec::with_capacity(dates.len());
        for (j, date) in dates.iter().enumerate() {
            let col: Vec<f64> = paths.iter().map(|p| p[j].value()).collect();
            columns.push((date.clone(), col.into_pyobject(py)?.into_any()));
        }
        let refs: Vec<(&str, Bound<'py, PyAny>)> = columns
            .iter()
            .map(|(name, col)| (name.as_str(), col.clone()))
            .collect();
        dataframe(py, &refs)
    }

    fn __repr__(&self) -> String {
        format!(
            "Simulation(market_index='{}', n_paths={})",
            self.element.market_index(),
            self.element.simulation().borrow().n_paths()
        )
    }
}
