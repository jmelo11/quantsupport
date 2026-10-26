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
    start_tenor: Option<String>,
    tenor: Option<String>,
) -> Result<String, XllError> {
    use quantsupport::prelude::*;
    let start_date = if start_date.trim().is_empty() {
        None
    } else {
        Some(parse_date(start_date)?)
    };
    let maturity_date = if maturity_date.trim().is_empty() {
        None
    } else {
        Some(parse_date(maturity_date)?)
    };
    let start_tenor = optional_text(start_tenor)
        .map(|value| Period::from_str(&value).map_err(value_error))
        .transpose()?;
    let tenor = optional_text(tenor)
        .map(|value| Period::from_str(&value).map_err(value_error))
        .transpose()?;
    if start_date.is_some() == start_tenor.is_some() || maturity_date.is_some() == tenor.is_some() {
        return Err(value_error(
            "supply one date or tenor for each loan start and maturity",
        ));
    }
    let currency = parse_currency(currency)?;
    let market_index = optional_text(market_index)
        .as_deref()
        .map(parse_index)
        .transpose()?;
    let repayment = optional_text(repayment).unwrap_or_else(|| "French".into());
    let payment_frequency = optional_text(payment_frequency)
        .as_deref()
        .map(parse_frequency)
        .transpose()?
        .unwrap_or(Frequency::Monthly);
    let day_counter = optional_text(day_counter)
        .as_deref()
        .map(parse_day_counter)
        .transpose()?;
    let options = loan_options(
        settings,
        coupon_periods,
        coupon_rates,
        coupon_indices,
        prepayment_dates,
        prepayment_cprs,
    )?;
    let key = input_key(&(
        start_date,
        maturity_date,
        start_tenor,
        tenor,
        currency,
        notional,
        rate,
        &market_index,
        &repayment,
        payment_frequency,
        day_counter,
        &options,
    ))?;
    if let Some(handle) = cached_handle(name, "Loan", &key)? {
        return Ok(handle);
    }
    let identifier = name.to_string();
    let object = InstrumentObject::from_resolver("Loan", notional, None, move |context| {
        let calendar = options.calendar.clone().unwrap_or_else(|| {
            Calendar::NullCalendar(quantsupport::time::calendars::nullcalendar::NullCalendar::new())
        });
        let dates = context.with_date_conventions(
            &calendar,
            options
                .business_day_convention
                .unwrap_or(BusinessDayConvention::Unadjusted),
            false,
        );
        let start = dates
            .resolve_date(
                start_date,
                start_tenor,
                context.reference_date(),
                "Loan start",
            )
            .map_err(|error| error.to_string())?;
        let end = dates
            .resolve_date(maturity_date, tenor, start, "Loan maturity")
            .map_err(|error| error.to_string())?;
        LoanTerms {
            start_date: start,
            maturity_date: end,
            currency,
            notional,
            rate,
            market_index: market_index.clone(),
            repayment: repayment.clone(),
            payment_frequency,
            day_counter,
            options: options.clone(),
        }
        .resolve_instrument(&identifier, context)
        .map(NativeInstrument::Loan)
    });
    with_registry_mut(|registry| {
        registry.upsert_if_changed(name, QsObject::Instrument(Box::new(object)), key)
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
            None,
            None,
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
