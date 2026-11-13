use crate::{
    core::evaluationresults::SensitivityMap,
    quotes::quote::QuoteDetails,
    time::{date::Date, period::Period},
};

/// Sorts a slice of strings in alphabetical order and returns a new sorted vector.
#[must_use]
pub fn sort_strings_alphabetically(strings: &[String]) -> Vec<String> {
    let mut sorted_strings = strings.to_owned();
    sorted_strings.sort();
    sorted_strings
}

/// Returns a reporting copy ordered by pillar maturity, keeping labels and values aligned.
///
/// Quote metadata supplies explicit dates or tenors. Other labels, including
/// zero-rate risk labels, can contain underscore-separated dates or tenors.
/// Tenors are advanced from `reference_date` using calendar arithmetic.
/// Equal maturities retain their input order; labels without a recognized
/// maturity follow the dated pillars in their original order. Raw inputs
/// and risk-basis matrix ordering are unchanged.
#[must_use]
pub fn sort_sensitivities_by_tenor(
    sensitivities: &SensitivityMap,
    reference_date: Date,
) -> SensitivityMap {
    let mut pairs: Vec<_> = sensitivities
        .instrument_keys()
        .iter()
        .cloned()
        .zip(sensitivities.exposure().iter().copied())
        .collect();
    pairs.sort_by_cached_key(|(label, _)| {
        let maturity = sensitivity_pillar_date(label, reference_date);
        (maturity.is_none(), maturity)
    });
    let (keys, exposures): (Vec<_>, Vec<_>) = pairs.into_iter().unzip();
    SensitivityMap::default()
        .with_instrument_keys(&keys)
        .with_exposure(&exposures)
}

/// Finds the maturity encoded by a quoted instrument or a labelled risk coordinate.
fn sensitivity_pillar_date(label: &str, reference: Date) -> Option<Date> {
    let advance = |tenor: Period| reference.advance(tenor.length(), tenor.units()).ok();
    if let Ok(details) = label.parse::<QuoteDetails>() {
        if let Some(date) = details
            .maturity()
            .or_else(|| details.tenor().and_then(advance))
        {
            return Some(date);
        }
    }
    let tokens = label.split('_');
    tokens
        .clone()
        .find_map(|token| Date::from_str(token, "%Y-%m-%d").ok())
        .or_else(|| {
            tokens
                .filter_map(|token| token.parse::<Period>().ok())
                .find_map(advance)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensitivity_report_sorts_tenors_and_keeps_exposures_aligned() {
        let labels = [
            "Swap_EUR_ESTR_10Y",
            "FixedRateDeposit_EUR_ESTR_1Y",
            "FixedRateDeposit_EUR_ESTR_6M",
            "Swap_EUR_ESTR_2Y",
            "SOFR_1W",
            "SOFR_1D",
            "SOFR_0D",
            "SOFR_1M",
            "SPOT",
        ]
        .map(String::from);
        let exposures = [100.0, 10.0, 6.0, 20.0, 7.0, 1.0, 0.0, 30.0, 99.0];
        let raw = SensitivityMap::default()
            .with_instrument_keys(&labels)
            .with_exposure(&exposures);
        let report = sort_sensitivities_by_tenor(&raw, Date::new(2026, 1, 15));
        assert_eq!(
            report.instrument_keys(),
            &[
                "SOFR_0D",
                "SOFR_1D",
                "SOFR_1W",
                "SOFR_1M",
                "FixedRateDeposit_EUR_ESTR_6M",
                "FixedRateDeposit_EUR_ESTR_1Y",
                "Swap_EUR_ESTR_2Y",
                "Swap_EUR_ESTR_10Y",
                "SPOT",
            ]
        );
        assert_eq!(
            report.exposure(),
            &[0.0, 1.0, 7.0, 30.0, 6.0, 10.0, 20.0, 100.0, 99.0]
        );
        assert_eq!(raw.instrument_keys(), &labels);
        assert_eq!(raw.exposure(), &exposures);
    }

    #[test]
    fn sensitivity_report_sorts_zero_dates_and_calendar_tenors_stably() {
        let labels = [
            "DiscountFactor_EUR_ESTR_2028-01-15",
            "SPOT",
            "ZeroRateRisk_ESTR_2027-01-15_Actual365_Continuous_Annual",
            "SOFR_1Y",
            "SOFR_1M",
            "SOFR_30D",
            "FX:USD/EUR.café",
        ]
        .map(String::from);
        let raw = SensitivityMap::default()
            .with_instrument_keys(&labels)
            .with_exposure(&[2.0, 90.0, 1.0, 1.5, 0.1, 0.09, 91.0]);
        let report = sort_sensitivities_by_tenor(&raw, Date::new(2026, 1, 15));
        assert_eq!(
            report.instrument_keys(),
            &[
                "SOFR_30D",
                "SOFR_1M",
                "ZeroRateRisk_ESTR_2027-01-15_Actual365_Continuous_Annual",
                "SOFR_1Y",
                "DiscountFactor_EUR_ESTR_2028-01-15",
                "SPOT",
                "FX:USD/EUR.café",
            ]
        );
        assert_eq!(report.exposure(), &[0.09, 0.1, 1.0, 1.5, 2.0, 90.0, 91.0]);
        assert!(
            sort_sensitivities_by_tenor(&SensitivityMap::default(), Date::new(2026, 1, 15))
                .instrument_keys()
                .is_empty()
        );
    }
}
