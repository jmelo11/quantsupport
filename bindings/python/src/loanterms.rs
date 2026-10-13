//! Typed loan terms with the same serialization as the native contracts.

use pyo3::prelude::*;
use quantsupport::prelude::{
    LoanRateDefinition as QsLoanRateDefinition, LoanRepaymentStructure as QsLoanRepaymentStructure,
    PrepaymentModelConfiguration as QsPrepaymentModelConfiguration,
};

use crate::conv::{extract_date, extract_market_index, from_py, qs_err};

/// Fixed, floating, or fixed-to-floating contractual coupon.
#[pyclass(module = "quantsupport", frozen, from_py_object)]
#[derive(Clone)]
pub struct LoanRateDefinition {
    pub inner: QsLoanRateDefinition,
}

#[pymethods]
impl LoanRateDefinition {
    /// Annual fixed coupon, as a decimal.
    #[staticmethod]
    fn fixed(rate: f64) -> Self {
        Self {
            inner: QsLoanRateDefinition::Fixed(rate),
        }
    }

    /// Index fixing plus annual spread, as a decimal.
    #[staticmethod]
    #[pyo3(signature = (index, spread = 0.0))]
    fn floating(index: &Bound<'_, PyAny>, spread: f64) -> PyResult<Self> {
        Ok(Self {
            inner: QsLoanRateDefinition::Floating {
                index: extract_market_index(index)?,
                spread,
            },
        })
    }

    /// Fixed coupon until a payment boundary, then index plus spread.
    #[staticmethod]
    #[pyo3(signature = (fixed_rate, switch_date, index, spread = 0.0))]
    fn mixed(
        fixed_rate: f64,
        switch_date: &Bound<'_, PyAny>,
        index: &Bound<'_, PyAny>,
        spread: f64,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: QsLoanRateDefinition::Mixed {
                fixed_rate,
                switch_date: extract_date(switch_date)?,
                index: extract_market_index(index)?,
                spread,
            },
        })
    }
}

/// French amortization, bullet repayment, or a residual balloon.
#[pyclass(module = "quantsupport", frozen, from_py_object)]
#[derive(Clone)]
pub struct LoanRepaymentStructure {
    pub inner: QsLoanRepaymentStructure,
}

#[pymethods]
impl LoanRepaymentStructure {
    /// Contractual principal schedule from level initial instalments.
    #[staticmethod]
    fn french() -> Self {
        Self {
            inner: QsLoanRepaymentStructure::French,
        }
    }

    /// Interest only until the final principal payment.
    #[staticmethod]
    fn bullet() -> Self {
        Self {
            inner: QsLoanRepaymentStructure::Bullet,
        }
    }

    /// French instalments with a residual fraction of original principal.
    #[staticmethod]
    fn balloon(residual_fraction: f64) -> Self {
        Self {
            inner: QsLoanRepaymentStructure::Balloon { residual_fraction },
        }
    }
}

/// Conditional prepayment model; annual CPR values are decimal fractions.
#[pyclass(module = "quantsupport", frozen, from_py_object)]
#[derive(Clone)]
pub struct PrepaymentModelConfiguration {
    pub inner: QsPrepaymentModelConfiguration,
}

#[pymethods]
impl PrepaymentModelConfiguration {
    /// No unscheduled principal payments.
    #[staticmethod]
    fn none() -> Self {
        Self {
            inner: QsPrepaymentModelConfiguration::None,
        }
    }

    /// Constant annual conditional prepayment rate.
    #[staticmethod]
    fn cpr(cpr: f64) -> Self {
        Self {
            inner: QsPrepaymentModelConfiguration::Cpr(cpr),
        }
    }

    /// Dated annual CPRs, supplied as (effective_date, cpr) pairs.
    #[staticmethod]
    fn cpr_curve(points: Vec<(Bound<'_, PyAny>, f64)>) -> PyResult<Self> {
        let points = points
            .into_iter()
            .map(|(date, cpr)| Ok((extract_date(&date)?, cpr)))
            .collect::<PyResult<Vec<_>>>()?;
        Ok(Self {
            inner: QsPrepaymentModelConfiguration::CprCurve(points),
        })
    }

    /// CPR driven by the positive coupon-minus-refinancing-rate incentive.
    #[staticmethod]
    fn refinancing(base_cpr: f64, incentive_multiplier: f64, maximum_cpr: f64) -> Self {
        Self {
            inner: QsPrepaymentModelConfiguration::Refinancing {
                base_cpr,
                incentive_multiplier,
                maximum_cpr,
            },
        }
    }
}

macro_rules! serialized_terms {
    ($name:ident, $native:ty, $label:literal) => {
        #[pymethods]
        impl $name {
            /// Loads the native JSON-compatible representation.
            #[staticmethod]
            fn from_dict(obj: &Bound<'_, PyAny>) -> PyResult<Self> {
                Ok(Self {
                    inner: from_py::<$native>(obj, $label)?,
                })
            }

            /// Returns the native JSON-compatible representation.
            fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
                pythonize::pythonize(py, &self.inner).map_err(qs_err)
            }

            fn __repr__(&self) -> String {
                format!("{}({:?})", stringify!($name), self.inner)
            }
        }
    };
}

serialized_terms!(LoanRateDefinition, QsLoanRateDefinition, "loan rate");
serialized_terms!(
    LoanRepaymentStructure,
    QsLoanRepaymentStructure,
    "loan repayment"
);
serialized_terms!(
    PrepaymentModelConfiguration,
    QsPrepaymentModelConfiguration,
    "prepayment"
);
