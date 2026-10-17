//! Bond trade pricing anchors.
use pyo3::prelude::*;
use quantsupport::prelude::BondPricingAnchor;
/// Market observable used to anchor native straight or callable bond pricing.
///
/// Use one of the named constructors; clean and dirty prices are quoted per
/// the bond's units. Price anchors imply Z-spread for straight bonds and OAS
/// for bonds with an issuer-call schedule.
#[pyclass(
    module = "quantsupport",
    name = "BondPricingAnchor",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub(crate) struct BondPricingAnchorConfig {
    inner: BondPricingAnchor,
}

impl From<BondPricingAnchor> for BondPricingAnchorConfig {
    fn from(inner: BondPricingAnchor) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl BondPricingAnchorConfig {
    /// Anchors the trade to a clean price.
    #[staticmethod]
    fn clean_price(value: f64) -> Self {
        Self {
            inner: BondPricingAnchor::CleanPrice(value),
        }
    }

    /// Anchors the trade to a dirty price.
    #[staticmethod]
    fn dirty_price(value: f64) -> Self {
        Self {
            inner: BondPricingAnchor::DirtyPrice(value),
        }
    }

    /// Anchors a straight bond to a continuously compounded Z-spread.
    #[staticmethod]
    fn z_spread(value: f64) -> Self {
        Self {
            inner: BondPricingAnchor::ZSpread(value),
        }
    }

    /// Anchors an issuer-call bond to an OAS.
    #[staticmethod]
    #[pyo3(name = "OAS")]
    fn oas(value: f64) -> Self {
        Self {
            inner: BondPricingAnchor::OAS(value),
        }
    }
}

impl BondPricingAnchorConfig {
    pub(crate) fn extract(obj: &Bound<'_, PyAny>) -> PyResult<BondPricingAnchor> {
        let anchor = if let Ok(value) = obj.extract::<Self>() {
            value.inner
        } else {
            crate::common::conversion::from_py(obj, "bond pricing anchor")?
        };
        anchor.validate().map_err(crate::common::conversion::qs_err)?;
        Ok(anchor)
    }
}
#[pymethods]
impl BondPricingAnchorConfig {
    /// Loads an anchor such as {"CleanPrice": 99.5} or {"OAS": 0.0075}.
    #[staticmethod]
    fn from_dict(obj: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: Self::extract(obj)?,
        })
    }
    /// Serializes the native anchor enum.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize::pythonize(py, &self.inner).map_err(crate::common::conversion::qs_err)
    }
}
