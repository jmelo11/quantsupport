use quantsupport::prelude::{DayCounter, Frequency};
use quantsupport_xll_macros::xll_bindgen;
use xll_rs::types::{XllError, XLOPER12};

use crate::{
    functions::helpers::{
        cached_handle, input_key, parse_currency, parse_date, parse_day_counter, parse_frequency,
        parse_index, value_error, worksheet_columns,
    },
    runtime::{
        loan_inputs,
        loans::LoanTerms,
        products::{InstrumentObject, NativeInstrument},
        registry::{with_registry, with_registry_mut, QsObject},
    },
};

fn optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn loan_options(
    settings: Option<String>,
    coupon_periods: *const XLOPER12,
    coupon_rates: *const XLOPER12,
    coupon_indices: *const XLOPER12,
    prepayment_dates: *const XLOPER12,
    prepayment_cprs: *const XLOPER12,
) -> Result<crate::runtime::loans::LoanOptions, XllError> {
    use crate::runtime::inputs::Cell;
    let settings = optional_text(settings)
        .map(|handle| vec![vec![Cell::Text(handle)]])
        .unwrap_or_default();
    let coupons = worksheet_columns(&[
        ("coupon_rates", coupon_rates),
        ("coupon_periods", coupon_periods),
        ("coupon_indices", coupon_indices),
    ])?
    .into_iter()
    .map(|row| vec![row[1].clone(), row[0].clone(), row[2].clone()])
    .collect();
    let prepayments = worksheet_columns(&[
        ("prepayment_dates", prepayment_dates),
        ("prepayment_cprs", prepayment_cprs),
    ])?;
    with_registry(|registry| {
        loan_inputs::with_settings(registry, &settings, &coupons, &prepayments)
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.LOAN.SETTINGS",
    category = "QuantSupport - Instruments",
    help = "Creates optional loan terms: balloon principal, amortization, borrower credit, resets, coupon bounds and prepayment; pass the handle to QS.INSTRUMENT.LOAN"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_loan_settings(
    name: &str,
    balloon_fraction: Option<f64>,
    amortization_rate: Option<f64>,
    credit_index: Option<String>,
    recovery: Option<f64>,
    fixing_frequency: Option<String>,
    fixing_lag: Option<f64>,
    index_tenor: Option<String>,
    calendar: Option<String>,
    business_day_convention: Option<String>,
    floor: Option<f64>,
    cap: Option<f64>,
    introductory_periods: Option<f64>,
    introductory_rate: Option<f64>,
    annual_cpr: Option<f64>,
    base_cpr: Option<f64>,
    incentive_multiplier: Option<f64>,
    maximum_cpr: Option<f64>,
) -> Result<String, XllError> {
    use crate::runtime::inputs::Cell;
    let number = |value: Option<f64>| value.map_or(Cell::Blank, Cell::Number);
    let text = |value: Option<String>| value.map_or(Cell::Blank, Cell::Text);
    let rows = vec![
        ("Balloon fraction", number(balloon_fraction)),
        ("Amortization rate", number(amortization_rate)),
        ("Credit index", text(credit_index)),
        ("Recovery", number(recovery)),
        ("Fixing frequency", text(fixing_frequency)),
        ("Fixing lag", number(fixing_lag)),
        ("Index tenor", text(index_tenor)),
        ("Calendar", text(calendar)),
        ("Business day convention", text(business_day_convention)),
        ("Floor", number(floor)),
        ("Cap", number(cap)),
        ("Introductory periods", number(introductory_periods)),
        ("Introductory rate", number(introductory_rate)),
        ("Annual CPR", number(annual_cpr)),
        ("Base CPR", number(base_cpr)),
        ("Incentive multiplier", number(incentive_multiplier)),
        ("Maximum CPR", number(maximum_cpr)),
    ]
    .into_iter()
    .map(|(name, value)| vec![Cell::Text(name.into()), value])
    .collect();
    let key = input_key(&rows)?;
    if let Some(handle) = cached_handle(name, "LoanSettings", &key)? {
        return Ok(handle);
    }
    let options = loan_inputs::options(&rows, &vec![], &vec![]).map_err(value_error)?;
    with_registry_mut(|registry| {
        registry.upsert_if_changed(name, QsObject::LoanSettings(Box::new(options)), key)
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.INSTRUMENT.LOAN",
    category = "QuantSupport - Instruments",
    help = "Creates a fixed or floating loan contract; rate is an annual decimal coupon or spread; defaults to French monthly repayment and Actual365; value with QS.EVALUATE"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_instrument_loan(
    name: &str,
    start_date: &str,
    maturity_date: &str,
    currency: &str,
    notional: f64,
    rate: f64,
    market_index: Option<String>,
    repayment: Option<String>,
    payment_frequency: Option<String>,
    day_counter: Option<String>,
    settings: Option<String>,
    coupon_periods: *const XLOPER12,
    coupon_rates: *const XLOPER12,
    coupon_indices: *const XLOPER12,
    prepayment_dates: *const XLOPER12,
    prepayment_cprs: *const XLOPER12,
) -> Result<String, XllError> {
    let terms = LoanTerms {
        start_date: parse_date(start_date)?,
        maturity_date: parse_date(maturity_date)?,
        currency: parse_currency(currency)?,
        notional,
        rate,
        market_index: optional_text(market_index)
            .as_deref()
            .map(parse_index)
            .transpose()?,
        repayment: optional_text(repayment).unwrap_or_else(|| "French".into()),
        payment_frequency: optional_text(payment_frequency)
            .as_deref()
            .map(parse_frequency)
            .transpose()?
            .unwrap_or(Frequency::Monthly),
        day_counter: optional_text(day_counter)
            .as_deref()
            .map(parse_day_counter)
            .transpose()?
            .unwrap_or(DayCounter::Actual365),
        options: loan_options(
            settings,
            coupon_periods,
            coupon_rates,
            coupon_indices,
            prepayment_dates,
            prepayment_cprs,
        )?,
    };
    let key = input_key(&terms)?;
    if let Some(handle) = cached_handle(name, "Loan", &key)? {
        return Ok(handle);
    }
    let notional = terms.notional;
    let loan = terms.build_instrument(name).map_err(value_error)?;
    with_registry_mut(|registry| {
        registry.upsert_if_changed(
            name,
            QsObject::Instrument(Box::new(InstrumentObject {
                native: NativeInstrument::Loan(loan),
                notional,
                foreign_notional: None,
            })),
            key,
        )
    })
    .map_err(value_error)
}

#[cfg(test)]
mod array_boundary_tests {
    use super::*;
    use xll_rs::{convert::build_multi, memory::xlAutoFree12};

    struct Array(*mut XLOPER12);
    impl Drop for Array {
        fn drop(&mut self) {
            xlAutoFree12(self.0);
        }
    }

    #[test]
    fn coupon_and_prepayment_fields_accept_independent_opposite_orientations() {
        let periods = Array(build_multi(
            vec![XLOPER12::from_f64(6.0), XLOPER12::nil()],
            1,
            2,
        ));
        let rates = Array(build_multi(
            vec![XLOPER12::from_f64(0.03), XLOPER12::from_f64(0.04)],
            2,
            1,
        ));
        let dates = Array(build_multi(
            vec![
                XLOPER12::from_str("2026-01-15"),
                XLOPER12::from_str("2026-07-15"),
            ],
            1,
            2,
        ));
        let cprs = Array(build_multi(
            vec![XLOPER12::from_f64(0.1), XLOPER12::from_f64(0.2)],
            2,
            1,
        ));
        let omitted = std::ptr::null();
        let options = loan_options(None, periods.0, rates.0, omitted, dates.0, cprs.0).unwrap();
        let segments = options.coupon_segments.unwrap();
        assert_eq!(segments[0].periods, Some(6));
        assert_eq!(segments[1].periods, None);
        assert_eq!(segments[1].rate, 0.04);
        assert!(options.prepayment.is_some());
        let loan = qs_instrument_loan(
            "array.boundary.loan",
            "2026-01-15",
            "2027-01-15",
            "USD",
            1000.0,
            0.03,
            None,
            None,
            None,
            None,
            None,
            periods.0,
            rates.0,
            omitted,
            dates.0,
            cprs.0,
        )
        .unwrap();
        assert_eq!(
            with_registry(|registry| registry.instrument(&loan).unwrap().notional),
            1000.0
        );
        assert!(loan_options(
            None,
            periods.0,
            &XLOPER12::from_f64(0.03),
            omitted,
            dates.0,
            cprs.0
        )
        .is_err());
    }
}
