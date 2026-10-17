//! Typed loan terms with the same serialization as the native contracts.

use pyo3::prelude::*;
use quantsupport::prelude::{
    LoanRepaymentStructure as QsLoanRepaymentStructure,
    PrepaymentModelConfiguration as QsPrepaymentModelConfiguration,
};

use crate::common::conversion::{extract_date, from_py, qs_err};

/// French amortization, bullet repayment, or a residual balloon.
#[pyclass(module = "quantsupport", frozen, from_py_object)]
#[derive(Clone)]
pub(crate) struct LoanRepaymentStructure {
    /// Native principal amortization terms serialized by the Python wrapper.
    pub(crate) inner: QsLoanRepaymentStructure,
}

#[pymethods]
impl LoanRepaymentStructure {
    /// Contractual principal schedule from level initial instalments.
    ///
    /// Uses actual accrual fractions and the initial amortization rate. Principal
    /// stays fixed after construction even when floating observations change.
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
    ///
    /// ``residual_fraction`` must be finite and in [0, 1] when constructing a Loan;
    /// 0.25 reserves 25% of original principal for maturity. Prepayment reduces
    /// the expected balloon proportionally without changing the contract.
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
pub(crate) struct PrepaymentModelConfiguration {
    /// Native annual CPR parameters serialized by the Python wrapper.
    pub(crate) inner: QsPrepaymentModelConfiguration,
}

#[pymethods]
impl PrepaymentModelConfiguration {
    /// No unscheduled principal payments.
    #[staticmethod]
    fn no_prepayment() -> Self {
        Self {
            inner: QsPrepaymentModelConfiguration::None,
        }
    }

    /// Constant annual conditional prepayment rate.
    ///
    /// ``cpr`` is a decimal fraction in [0, 1], validated when constructing a Loan.
    /// The pricer converts it to the interval probability and applies it after
    /// scheduled principal: ``1 - (1 - cpr)**year_fraction``.
    #[staticmethod]
    fn constant_annual_cpr(cpr: f64) -> Self {
        Self {
            inner: QsPrepaymentModelConfiguration::Cpr(cpr),
        }
    }

    /// Dated annual CPRs, supplied as (effective_date, cpr) pairs.
    ///
    /// Dates must increase strictly and CPRs must lie in [0, 1] when constructing
    /// a Loan. Uses the latest point at/before the prepayment interval's start,
    /// zero before the first point, and the last CPR after the final point.
    #[staticmethod]
    fn dated_annual_cpr(points: Vec<(Bound<'_, PyAny>, f64)>) -> PyResult<Self> {
        let points = points
            .into_iter()
            .map(|(date, cpr)| Ok((extract_date(&date)?, cpr)))
            .collect::<PyResult<Vec<_>>>()?;
        Ok(Self {
            inner: QsPrepaymentModelConfiguration::CprCurve(points),
        })
    }

    /// CPR driven by the positive coupon-minus-refinancing-benchmark incentive.
    ///
    /// Annual CPR is ``min(maximum_cpr, base_cpr + incentive_multiplier *
    /// max(contractual_coupon_rate - refinancing_benchmark_rate, 0))``.
    /// Rates are decimals; the multiplier is nonnegative and
    /// ``0 <= base_cpr <= maximum_cpr <= 1``. Validation occurs
    /// when constructing a Loan. Configure a LoanPricer refinancing benchmark
    /// index to supply the observation; its AD dependencies enter sensitivities.
    #[staticmethod]
    fn refinancing_incentive(base_cpr: f64, incentive_multiplier: f64, maximum_cpr: f64) -> Self {
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
            ///
            /// Accepts the same tagged dictionary or unit-variant string returned
            /// by ``to_dict()``. Deserializes terms; economic parameter constraints
            /// are validated later when constructing a Loan.
            #[staticmethod]
            fn from_dict(obj: &Bound<'_, PyAny>) -> PyResult<Self> {
                Ok(Self {
                    inner: from_py::<$native>(obj, $label)?,
                })
            }

            /// Returns the native JSON-compatible representation.
            ///
            /// Unit variants serialize as strings; variants with parameters
            /// serialize as tagged dictionaries. Suitable for ``from_dict()``
            /// or embedding in Loan construction parameters.
            fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
                pythonize::pythonize(py, &self.inner).map_err(qs_err)
            }

            /// Returns the term type and native variant for inspection.
            fn __repr__(&self) -> String {
                format!("{}({:?})", stringify!($name), self.inner)
            }
        }
    };
}

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
