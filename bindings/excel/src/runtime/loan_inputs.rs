//! Loan settings, coupon schedules and dated CPRs from ordinary worksheet cells.

use std::collections::HashSet;

use quantsupport::prelude::{
    BusinessDayConvention, Calendar, Date, Frequency, MarketIndex, Period,
    PrepaymentModelConfiguration,
};

use super::{
    inputs::{self, Cell, Table},
    loans::{CouponSegment, LoanOptions},
};

fn index(cell: &Cell, name: &str) -> Result<MarketIndex, String> {
    let text = cell.text(name)?;
    if let Some(entity) = text.strip_prefix("Credit:") {
        if entity.trim().is_empty() {
            return Err(format!("{name} requires a credit entity"));
        }
        return Ok(MarketIndex::Credit(entity.trim().into()));
    }
    inputs::market_index(text)
}

/// Reads optional setting/value, periods/rate/index, and date/CPR ranges.
pub fn options(
    settings: &Table,
    coupons: &Table,
    prepayments: &Table,
) -> Result<LoanOptions, String> {
    let mut options = LoanOptions::default();
    let mut seen = HashSet::new();
    let mut model = None;
    let (mut cpr, mut base_cpr, mut incentive, mut maximum) = (None, None, None, None);
    for (position, row) in inputs::rows(settings, 2, "settings")?
        .into_iter()
        .enumerate()
    {
        let name = row[0].text("setting name")?;
        let key = inputs::label(name);
        if position == 0
            && matches!(key.as_str(), "setting" | "parameter")
            && inputs::label(row[1].text("settings header")?) == "value"
        {
            continue;
        }
        if !seen.insert(key.clone()) {
            return Err(format!("duplicate loan setting '{name}'"));
        }
        let value = &row[1];
        // Validate names even when their value cells are blank.
        if !matches!(
            key.as_str(),
            "balloonfraction"
                | "amortizationrate"
                | "creditindex"
                | "recovery"
                | "fixingfrequency"
                | "fixinglag"
                | "indextenor"
                | "calendar"
                | "businessdayconvention"
                | "floor"
                | "cap"
                | "introductoryperiods"
                | "introductoryrate"
                | "prepaymentmodel"
                | "annualcpr"
                | "basecpr"
                | "incentivemultiplier"
                | "maximumcpr"
        ) {
            return Err(format!("unknown loan setting '{name}'"));
        }
        if value.is_blank() {
            continue;
        }
        match key.as_str() {
            "balloonfraction" => options.balloon_fraction = Some(value.number(name)?),
            "amortizationrate" => options.amortization_rate = Some(value.number(name)?),
            "creditindex" => options.credit_index = Some(index(value, name)?),
            "recovery" => options.recovery = Some(value.number(name)?),
            "fixingfrequency" => {
                options.fixing_frequency = Some(
                    value
                        .text(name)?
                        .parse::<Frequency>()
                        .map_err(|e| e.to_string())?,
                )
            }
            "fixinglag" => options.fixing_lag = Some(value.count(name)? as i32),
            "indextenor" => {
                options.index_tenor = Some(
                    value
                        .text(name)?
                        .parse::<Period>()
                        .map_err(|e| e.to_string())?,
                )
            }
            "calendar" => {
                options.calendar = Some(
                    Calendar::try_from(value.text(name)?.to_string()).map_err(|e| e.to_string())?,
                )
            }
            "businessdayconvention" => {
                options.business_day_convention = Some(
                    BusinessDayConvention::try_from(value.text(name)?.to_string())
                        .map_err(|e| e.to_string())?,
                )
            }
            "floor" => options.floor = Some(value.number(name)?),
            "cap" => options.cap = Some(value.number(name)?),
            "introductoryperiods" => options.introductory_periods = Some(value.count(name)?),
            "introductoryrate" => options.introductory_rate = Some(value.number(name)?),
            "prepaymentmodel" => model = Some(inputs::label(value.text(name)?)),
            "annualcpr" => cpr = Some(value.number(name)?),
            "basecpr" => base_cpr = Some(value.number(name)?),
            "incentivemultiplier" => incentive = Some(value.number(name)?),
            "maximumcpr" => maximum = Some(value.number(name)?),
            _ => unreachable!(),
        }
    }

    let mut segments = Vec::new();
    for (position, row) in
        inputs::rows(coupons, 3, "coupons (Periods, Rate / spread, Market index)")?
            .into_iter()
            .enumerate()
    {
        if position == 0 && matches!(&row[0], Cell::Text(text) if inputs::label(text) == "periods")
        {
            if inputs::label(row[1].text("coupon header")?) != "ratespread"
                || inputs::label(row[2].text("coupon header")?) != "marketindex"
            {
                return Err("coupon headers must be Periods, Rate / spread, Market index".into());
            }
            continue;
        }
        segments.push(CouponSegment {
            periods: if row[0].is_blank() {
                None
            } else {
                Some(row[0].count("coupon periods")?)
            },
            rate: row[1].number("coupon rate / spread")?,
            market_index: if row[2].is_blank() {
                None
            } else {
                Some(index(&row[2], "coupon market index")?)
            },
        });
    }
    if !segments.is_empty() {
        options.coupon_segments = Some(segments);
    }

    let mut points = Vec::new();
    for (position, row) in inputs::rows(prepayments, 2, "prepayments (Date, CPR)")?
        .into_iter()
        .enumerate()
    {
        if position == 0 && matches!(&row[0], Cell::Text(text) if inputs::label(text) == "date") {
            if inputs::label(row[1].text("prepayment header")?) != "cpr" {
                return Err("prepayment headers must be Date, CPR".into());
            }
            continue;
        }
        let date = Date::from_str(row[0].text("CPR date (YYYY-MM-DD)")?, "%Y-%m-%d")
            .map_err(|e| e.to_string())?;
        points.push((date, row[1].number("dated annual CPR")?));
    }
    let refinancing = base_cpr.is_some() || incentive.is_some() || maximum.is_some();
    let supplied =
        usize::from(cpr.is_some()) + usize::from(!points.is_empty()) + usize::from(refinancing);
    if supplied > 1 {
        return Err(
            "supply only one prepayment model: annual CPR, dated CPRs, or refinancing settings"
                .into(),
        );
    }
    let inferred = if cpr.is_some() {
        "cpr"
    } else if !points.is_empty() {
        "cprcurve"
    } else if refinancing {
        "refinancing"
    } else {
        "none"
    };
    let selected = model.as_deref().unwrap_or(inferred);
    if selected != inferred {
        return Err(format!("prepayment model '{selected}' conflicts with its supplied inputs (expected {inferred})"));
    }
    options.prepayment = match selected {
        "none" => None,
        "cpr" => Some(PrepaymentModelConfiguration::Cpr(
            cpr.ok_or("Annual CPR is required")?,
        )),
        "cprcurve" => Some(PrepaymentModelConfiguration::CprCurve(points)),
        "refinancing" => Some(PrepaymentModelConfiguration::Refinancing {
            base_cpr: base_cpr.ok_or("Base CPR is required for refinancing")?,
            incentive_multiplier: incentive
                .ok_or("Incentive multiplier is required for refinancing")?,
            maximum_cpr: maximum.ok_or("Maximum CPR is required for refinancing")?,
        }),
        _ => return Err("Prepayment model must be None, CPR, CPRCurve, or Refinancing".into()),
    };
    if let Some(prepayment) = &options.prepayment {
        prepayment
            .validate_parameters()
            .map_err(|e| e.to_string())?;
    }
    Ok(options)
}

#[cfg(test)]
mod tests {
    use super::*;
    use Cell::{Blank, Boolean, Number, Text};

    fn text(value: &str) -> Cell {
        Text(value.into())
    }
    fn setting(name: &str, value: Cell) -> Vec<Cell> {
        vec![text(name), value]
    }

    #[test]
    fn typed_settings_and_schedules_preserve_native_values_and_defaults() {
        let settings = vec![
            setting("Setting", text("Value")),
            setting("Balloon fraction", Number(0.25)),
            setting("Amortization rate", Number(0.05)),
            setting("Credit index", text("Credit:BORROWER")),
            setting("Recovery", Number(0.4)),
            setting("Fixing frequency", text("Annual")),
            setting("Fixing lag", Number(2.0)),
            setting("Index tenor", text("12M")),
            setting("Calendar", text("TARGET")),
            setting("Business day convention", text("Following")),
            setting("Floor", Blank),
            vec![Blank, Blank],
        ];
        let coupons = vec![
            vec![text("Periods"), text("Rate / spread"), text("Market index")],
            vec![Number(12.0), Number(0.04), Blank],
            vec![Blank, Number(0.01), text("EURIBOR12m")],
            vec![Blank, Blank, Blank],
        ];
        let options = options(&settings, &coupons, &vec![]).unwrap();
        assert_eq!(options.balloon_fraction, Some(0.25));
        assert_eq!(options.amortization_rate, Some(0.05));
        assert_eq!(
            options.credit_index,
            Some(MarketIndex::Credit("BORROWER".into()))
        );
        assert_eq!(options.fixing_frequency, Some(Frequency::Annual));
        assert_eq!(options.fixing_lag, Some(2));
        assert_eq!(options.index_tenor.unwrap().to_string(), "12M");
        assert!(options.calendar.is_some());
        assert_eq!(
            options.business_day_convention,
            Some(BusinessDayConvention::Following)
        );
        let segments = options.coupon_segments.unwrap();
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].periods, Some(12));
        assert_eq!(segments[0].rate, 0.04);
        assert_eq!(segments[0].market_index, None);
        assert_eq!(segments[1].periods, None);
        assert_eq!(segments[1].market_index, Some(MarketIndex::EURIBOR12m));
        let defaults = super::options(&vec![vec![Blank]], &vec![], &vec![]).unwrap();
        assert!(defaults.coupon_segments.is_none());
    }

    #[test]
    fn prepayment_inputs_select_and_validate_constant_dated_and_refinancing_models() {
        let constant =
            options(&vec![setting("Annual CPR", Number(0.15))], &vec![], &vec![]).unwrap();
        assert!(matches!(
            constant.prepayment,
            Some(PrepaymentModelConfiguration::Cpr(0.15))
        ));
        let dates = vec![
            vec![text("Date"), text("CPR")],
            vec![text("2026-02-24"), Number(0.1)],
            vec![text("2027-02-24"), Number(0.2)],
        ];
        let dated = options(&vec![], &vec![], &dates).unwrap();
        assert!(
            matches!(dated.prepayment, Some(PrepaymentModelConfiguration::CprCurve(points)) if points.len() == 2)
        );
        let settings = vec![
            setting("Base CPR", Number(0.02)),
            setting("Incentive multiplier", Number(4.0)),
            setting("Maximum CPR", Number(0.4)),
        ];
        let refinancing = options(&settings, &vec![], &vec![]).unwrap();
        assert!(matches!(
            refinancing.prepayment,
            Some(PrepaymentModelConfiguration::Refinancing { .. })
        ));
        assert!(options(&settings[..1].to_vec(), &vec![], &vec![]).is_err());
        assert!(options(&settings, &vec![], &dates).is_err());
        assert!(options(&vec![setting("Annual CPR", Number(0.15))], &vec![], &dates).is_err());
        assert!(options(
            &vec![
                setting("Prepayment model", text("None")),
                setting("Annual CPR", Number(0.1))
            ],
            &vec![],
            &vec![]
        )
        .is_err());
        let reversed = vec![dates[2].clone(), dates[1].clone()];
        assert!(options(&vec![], &vec![], &reversed).is_err());
        assert!(options(&vec![setting("Annual CPR", Number(1.1))], &vec![], &vec![]).is_err());
    }

    #[test]
    fn malformed_ranges_and_mistyped_values_fail_instead_of_silently_defaulting() {
        for settings in [
            vec![setting("Typo", Blank)],
            vec![setting("Fixing lag", Number(1.5))],
            vec![setting("Fixing lag", Number(-1.0))],
            vec![setting("Recovery", text("0.4"))],
            vec![setting("Recovery", Number(f64::NAN))],
            vec![setting("Annual CPR", Boolean(true))],
            vec![setting("Rebuild amortization for fair rate", Number(2.0))],
            vec![setting("Discount index", Number(1.0))],
            vec![
                setting("Fixing lag", Blank),
                setting("fixing_lag", Number(2.0)),
            ],
            vec![vec![text(r#"{"amortization_rate":0.05}"#)]],
        ] {
            assert!(
                options(&settings, &vec![], &vec![]).is_err(),
                "{settings:?}"
            );
        }
        for coupons in [
            vec![vec![Number(12.0), Number(0.03)]],
            vec![vec![Number(12.5), Number(0.03), Blank]],
            vec![vec![Number(12.0), text("0.03"), Blank]],
            vec![vec![Number(12.0), Number(0.03), Number(1.0)]],
        ] {
            assert!(options(&vec![], &coupons, &vec![]).is_err());
        }
    }
}
