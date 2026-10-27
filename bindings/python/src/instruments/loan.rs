//! Native contractual loans and position conversion.

use crate::{
    common::{
        conversion::{
            dataframe, extract_business_day_convention, extract_calendar, extract_currency,
            extract_date, extract_frequency, extract_market_index, extract_period, from_py, qs_err,
        },
        enums::Currency,
    },
    instruments::coupons::CouponDefinition,
    pricing::{
        context::PricingContext,
        extension::{Instrument, Trade},
    },
};
use pyo3::{
    prelude::*,
    types::{PyDict, PyType},
};
use quantsupport::prelude::{
    Currency as QsCurrency, DualFwd, Loan as QsLoan, LoanRepaymentStructure, LoanTrade,
    MakeInstrument, MakeLoan, PrepaymentModelConfiguration, QSError,
};
use std::collections::HashSet;

/// A native loan contract; rates and CPRs are fractions, not percentages.
///
/// Floating amortizing principal needs an explicit initial amortization rate or
/// construction context. The built principal schedule stays fixed. Market rates,
/// prepayment, and default are applied during valuation through ``Trade`` and
/// ``PricingContext``. Dates and enums accept their typed values
/// or string representations; coupons, repayment, and prepayment terms also accept
/// their native JSON-compatible representations.
///
/// Constructs and validates the contractual coupon and principal schedule.
///
/// Parameters
/// ----------
/// identifier : str
///     Nonempty contract identifier.
/// start_date, maturity_date : Date, datetime.date or str
///     Origination and final accrual dates, with start before maturity.
/// notional : float
///     Finite, strictly positive principal in the contract currency.
/// coupons : list[FixedRateCoupon, FloatingRateCoupon, OptionEmbeddedCoupon, or dict]
///     Positional coupon definitions, as for SteppedCouponBond. The last entry
///     repeats through maturity; each entry uses a native coupon class.
///     Accrual day counters belong to coupons. Floating coupons use their explicit
///     day counter, or the index convention when omitted; amortization uses the
///     same accrual conventions as interest.
/// currency : enum or str
///     Payment currency; the pricing policy selects the discount curve.
/// repayment : LoanRepaymentStructure, str or dict, optional
///     French by default; bullet and balloon schedules are also supported.
/// payment_frequency, fixing_frequency : Frequency or str, optional
///     Payments default to monthly. Resets default to index metadata, or the
///     payment frequency for daily indices, and must align with accrual dates.
/// prepayment : PrepaymentModelConfiguration, str or dict, optional
///     Conditional prepayment model; defaults to none. Applied by the pricer.
/// credit_index : MarketIndex or str, optional
///     Borrower survival curve; must be a Credit index. Defaults to no default.
/// recovery : float, optional
///     Recovery fraction in [0, 1]; defaults to 0.4.
/// fixing_lag : int, optional
///     Nonnegative business days before reset; defaults to zero.
/// index_tenor : Period or str, optional
///     Positive observation tenor, independent of payments and resets;
///     defaults to the index's metadata frequency.
/// calendar : Calendar or str, optional
///     Payment/fixing calendar; defaults to a null calendar.
/// business_day_convention : BusinessDayConvention or str, optional
///     Payment adjustment; defaults to Unadjusted. Accrual dates stay fixed.
/// amortization_rate : float, optional
///     Optional initial annual all-in decimal rate for principal construction.
///     Blank uses the first fixed coupon, or resolves the first floating
///     observation during build_using. Contractual principal stays fixed.
///
/// Raises
/// ------
/// QuantSupportError
///     If contractual terms are invalid, or
///     coupon lists are empty/oversized or resets do not align with accrual dates.
#[pyclass(module = "quantsupport", extends = Instrument, subclass, dict, unsendable, from_py_object)]
#[derive(Clone)]
pub struct Loan {
    /// Native contractual cashflows and prepayment/credit terms used in valuation.
    pub inner: QsLoan<DualFwd>,
    principal: f64,
    payment_currency: QsCurrency,
    builder: MakeLoan<DualFwd>,
    construction_parameters: serde_json::Value,
}

impl Loan {
    /// Builds a native position, rebuilding principal for a notional override.
    ///
    /// Reuses the original loan otherwise. Execution date and side come from
    /// the Python trade; the Python instrument remains unchanged.
    ///
    /// # Errors
    /// Propagates invalid overridden principal or contractual construction terms.
    pub fn build_native_loan_trade(&self, position: &Trade) -> PyResult<LoanTrade<DualFwd>> {
        let loan = match position.notional() {
            Some(notional) if notional != self.inner.notional() => self
                .builder
                .clone()
                .with_notional(notional)
                .build()
                .map_err(qs_err)?,
            _ => self.inner.clone(),
        };
        Ok(LoanTrade::new(
            loan,
            position.trade_date()?,
            position.side(),
        ))
    }
}

#[pymethods]
impl Loan {
    /// Constructs and validates the loan using the documented builder defaults.
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (identifier, start_date, maturity_date, notional, coupons, currency,
        repayment = None, payment_frequency = None, fixing_frequency = None,
        prepayment = None, credit_index = None, recovery = None,
        fixing_lag = None, index_tenor = None, calendar = None, business_day_convention = None,
        amortization_rate = None, context = None))]
    fn new(
        identifier: String,
        start_date: &Bound<'_, PyAny>,
        maturity_date: &Bound<'_, PyAny>,
        notional: f64,
        coupons: &Bound<'_, PyAny>,
        currency: &Bound<'_, PyAny>,
        repayment: Option<&Bound<'_, PyAny>>,
        payment_frequency: Option<&Bound<'_, PyAny>>,
        fixing_frequency: Option<&Bound<'_, PyAny>>,
        prepayment: Option<&Bound<'_, PyAny>>,
        credit_index: Option<&Bound<'_, PyAny>>,
        recovery: Option<f64>,
        fixing_lag: Option<i32>,
        index_tenor: Option<&Bound<'_, PyAny>>,
        calendar: Option<&Bound<'_, PyAny>>,
        business_day_convention: Option<&Bound<'_, PyAny>>,
        amortization_rate: Option<f64>,
        context: Option<PyRef<'_, PricingContext>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let coupons = CouponDefinition::list_from_py(coupons, "Loan.coupons")?;
        let start_date = extract_date(start_date)?;
        let maturity_date = extract_date(maturity_date)?;
        let currency = extract_currency(currency)?;
        let repayment = repayment
            .map(|value| from_py::<LoanRepaymentStructure>(value, "loan repayment"))
            .transpose()?;
        let payment_frequency = payment_frequency.map(extract_frequency).transpose()?;
        let fixing_frequency = fixing_frequency.map(extract_frequency).transpose()?;
        let prepayment = prepayment
            .map(|value| from_py::<PrepaymentModelConfiguration>(value, "prepayment"))
            .transpose()?;
        let credit_index = credit_index.map(extract_market_index).transpose()?;
        let index_tenor = index_tenor.map(extract_period).transpose()?;
        let calendar = calendar.map(extract_calendar).transpose()?;
        let business_day_convention = business_day_convention
            .map(extract_business_day_convention)
            .transpose()?;
        let mut construction_parameters = serde_json::json!({
            "identifier": identifier, "start_date": start_date, "maturity_date": maturity_date,
            "notional": notional, "coupons": coupons.iter().map(CouponDefinition::specification).collect::<PyResult<Vec<_>>>()?,
            "currency": currency, "repayment": repayment,
            "payment_frequency": payment_frequency, "fixing_frequency": fixing_frequency,
            "prepayment": prepayment, "credit_index": credit_index,
            "recovery": recovery, "fixing_lag": fixing_lag, "index_tenor": index_tenor,
            "calendar": calendar, "business_day_convention": business_day_convention,
            "amortization_rate": amortization_rate,
        });
        let mut builder = MakeLoan::default()
            .with_identifier(identifier.clone())
            .with_start_date(start_date)
            .with_maturity_date(maturity_date)
            .with_notional(notional)
            .with_coupons(
                coupons
                    .iter()
                    .map(|coupon| coupon.inner.clone().into())
                    .collect(),
            )
            .with_currency(currency);
        if let Some(value) = repayment {
            builder = builder.with_repayment_structure(value);
        }
        if let Some(value) = payment_frequency {
            builder = builder.with_payment_frequency(value);
        }
        if let Some(value) = fixing_frequency {
            builder = builder.with_coupon_reset_frequency(value);
        }
        if let Some(value) = prepayment {
            builder = builder.with_prepayment_configuration(value);
        }
        if let Some(value) = credit_index {
            builder = builder.with_credit_curve_index(value);
        }
        if let Some(value) = recovery {
            builder = builder.with_recovery_fraction(value);
        }
        if let Some(value) = fixing_lag {
            builder = builder.with_fixing_lag_business_days(value);
        }
        if let Some(value) = index_tenor {
            builder = builder.with_index_observation_tenor(value);
        }
        if let Some(value) = calendar {
            builder = builder.with_calendar(value);
        }
        if let Some(value) = business_day_convention {
            builder = builder.with_business_day_convention(value);
        }
        if let Some(value) = amortization_rate {
            builder = builder.with_initial_amortization_rate(value);
        }
        let inner = match context {
            Some(context) => builder
                .build_using(&context.market_view()?)
                .map_err(qs_err)?,
            None if !builder.is_resolved() => {
                return Err(qs_err(QSError::UnresolvedProductErr(
                    "Loan needs an initial floating coupon observation for amortization. Supply amortization_rate, set the first coupon fixing, or use MakeLoan(...).build_using(context) with an active PricingContext".into(),
                )));
            }
            None => builder.build().map_err(qs_err)?,
        };
        construction_parameters["amortization_rate"] =
            serde_json::json!(inner.initial_amortization_rate());
        if let Some(rate) = inner.initial_amortization_rate() {
            builder = builder.with_initial_amortization_rate(rate);
        }
        Ok(
            PyClassInitializer::from(Instrument::new(identifier)).add_subclass(Self {
                inner,
                principal: notional,
                payment_currency: currency,
                builder,
                construction_parameters,
            }),
        )
    }

    /// Returns unique coupon index observations as a pandas DataFrame.
    ///
    /// Columns are ``market_index``, ``fixing_date``, ``start_date``, and
    /// ``end_date``; dates are ISO strings. Rows retain contractual reset order
    /// and are deduplicated by index/fixing date. A fixed loan has no rows.
    /// This lists contractual resets; the pricer determines which past fixings
    /// are required for unpaid coupons at a particular evaluation date.
    fn coupon_fixing_requests<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let loan = &self.inner;
        let mut seen = HashSet::new();
        let requests: Vec<_> = loan
            .coupon_rate_observations()
            .iter()
            .flatten()
            .filter(|request| seen.insert((request.market_index(), request.fixing_date())))
            .collect();
        let market_index: Vec<_> = requests
            .iter()
            .map(|request| request.market_index().to_string())
            .collect();
        let fixing_date: Vec<_> = requests
            .iter()
            .map(|request| request.fixing_date().to_string())
            .collect();
        let start_date: Vec<_> = requests
            .iter()
            .map(|request| request.start_date().map(|date| date.to_string()))
            .collect();
        let end_date: Vec<_> = requests
            .iter()
            .map(|request| request.end_date().map(|date| date.to_string()))
            .collect();
        dataframe(
            py,
            &[
                ("market_index", market_index.into_pyobject(py)?.into_any()),
                ("fixing_date", fixing_date.into_pyobject(py)?.into_any()),
                ("start_date", start_date.into_pyobject(py)?.into_any()),
                ("end_date", end_date.into_pyobject(py)?.into_any()),
            ],
        )
    }

    /// Constructs a loan from keyword-compatible construction parameters.
    ///
    /// Accepts the representation returned by ``to_dict()`` and preserves Python
    /// subclasses. Validation and defaults are the same as the constructor.
    #[classmethod]
    fn from_dict<'py>(
        cls: &Bound<'py, PyType>,
        obj: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        cls.call(
            (),
            Some(crate::common::conversion::expect_dict(
                obj,
                "Loan.from_dict",
            )?),
        )
    }

    /// Returns JSON-compatible construction parameters for ``from_dict()``.
    ///
    /// Includes optional settings as supplied, which may be ``None`` when a
    /// builder default was used. Contains no projected cashflows, market state,
    /// or trade terms; it does not expose the resolved default principal schedule.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize::pythonize(py, &self.construction_parameters).map_err(qs_err)
    }

    /// Original principal advanced under the contract.
    #[getter]
    fn notional(&self) -> f64 {
        self.principal
    }

    /// Native payment currency.
    #[getter]
    fn currency(&self) -> Currency {
        self.payment_currency.into()
    }

    /// Reads a serialized construction parameter by name.
    ///
    /// Optional parameters remain ``None`` when omitted at construction.
    /// Raises ``AttributeError`` for names outside the construction schema.
    fn __getattr__<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
        let terms = self.to_dict(py)?;
        terms.cast::<PyDict>()?.get_item(name)?.ok_or_else(|| {
            pyo3::exceptions::PyAttributeError::new_err(format!("Loan has no attribute '{name}'"))
        })
    }
}

crate::common::representation::impl_dict_protocol!(Loan);
