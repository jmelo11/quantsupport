use quantsupport::prelude::*;

const TENORS: [&str; 4] = ["3M", "6M", "9M", "1Y"];
const PAR_RATES: [f64; 4] = [0.02598, 0.02781542, 0.029203, 0.0302858];
const TARGETS: [f64; 4] = [0.99326, 0.98612, 0.978322, 0.970002];

fn maturities() -> [Date; 4] {
    [
        Date::new(2027, 1, 4),
        Date::new(2027, 4, 2),
        Date::new(2027, 7, 2),
        Date::new(2027, 10, 4),
    ]
}

fn reconcile(
    reference: Date,
    lag: i32,
    fixed_frequency: &str,
    fixed_day_counter: DayCounter,
    single_curve: bool,
) -> Result<[f64; 4]> {
    let mut quotes = QuoteStore::new(reference);
    let mut estr_ids = Vec::new();
    for (tenor, rate) in [
        ("1W", 0.02453),
        ("2W", 0.02451),
        ("1M", 0.02462),
        ("3M", 0.02499),
        ("6M", 0.02620),
        ("9M", 0.02741),
        ("1Y", 0.02839),
    ] {
        let identifier = format!("OIS_EUR_ESTR_{tenor}___{lag}_0");
        quotes.add_quote(Quote::new(identifier.parse()?, QuoteLevels::with_mid(rate)));
        estr_ids.push(identifier);
    }
    let mut euribor_ids = Vec::new();
    // Use the reported 3M par quote, which equals the supplied 2.598% fixing.
    // Its ACT/360 case provides the matching money-market accrual anchor.
    for (tenor, rate) in TENORS.into_iter().zip(PAR_RATES) {
        let identifier = format!(
            "Swap_EUR_EURIBOR3m_{tenor}_{fixed_frequency}_Quarterly_{lag}_{fixed_day_counter:?}"
        );
        quotes.add_quote(Quote::new(identifier.parse()?, QuoteLevels::with_mid(rate)));
        euribor_ids.push(identifier);
    }
    let config = |index, ids| {
        CurveConfiguration::new(
            index,
            DayCounter::Actual360,
            Interpolator::LogLinear,
            true,
            ids,
        )
    };
    let mut specs = vec![config(MarketIndex::EURIBOR3m, euribor_ids)];
    let csa = if single_curve {
        MarketIndex::EURIBOR3m
    } else {
        MarketIndex::ESTR
    };
    if !single_curve {
        specs.push(config(MarketIndex::ESTR, estr_ids));
    }
    let curves =
        MultiCurveBootstrapper::new(specs, BootstrapDiscountPolicy::new(csa, Currency::EUR))
            .bootstrap(&quotes, Level::Mid)?;
    let curve = curves[&MarketIndex::EURIBOR3m].curve();
    let start = Date::new(2026, 10, 2);
    let start_df = curve.discount_factor(start)?.value();
    let effective = MarketIndex::EURIBOR3m
        .rate_index_details()?
        .calendar()
        .advance(
            reference,
            Period::new(lag, TimeUnit::Days),
            Some(BusinessDayConvention::Following),
            false,
        );
    println!(
        "\nreference={reference} effective={effective} lag={lag} fixed={fixed_frequency}/{fixed_day_counter:?} single_curve={single_curve} start_df={start_df:.10}"
    );
    let mut factors = [0.0; 4];
    for (i, (date, target)) in maturities().into_iter().zip(TARGETS).enumerate() {
        let absolute = curve.discount_factor(date)?.value();
        let normalized = absolute / start_df;
        factors[i] = normalized;
        println!(
            "{date}: target={target:.9} df={absolute:.9} spot_df={normalized:.9} error={:.9}",
            normalized - target
        );
    }
    Ok(factors)
}

// Independent, closed-form cashflow recursion: each swap has one fixed
// payment and quarterly floating coupons. Every coupon date is an OIS node,
// so no interpolation or nonlinear solver is involved in this check.
fn check_annual_factors(factors: [f64; 4], fixed_day_counter: DayCounter) -> Result<()> {
    let start = Date::new(2026, 10, 2);
    let mut previous = 1.0;
    let mut floating_pv = 0.0;
    let mut previous_target = 1.0;
    let mut target_floating_pv = 0.0;
    let mut max_error: f64 = 0.0;
    println!("\nIndependent annual {fixed_day_counter:?} cashflow check:");
    for (i, (date, ois_rate)) in maturities()
        .into_iter()
        .zip([0.02499, 0.02620, 0.02741, 0.02839])
        .enumerate()
    {
        let ois_accrual = DayCounter::Actual360.year_fraction(start, date);
        let discount = 1.0 / (1.0 + ois_rate * ois_accrual);
        let accrual = fixed_day_counter.year_fraction(start, date);
        let expected = previous / (1.0 + PAR_RATES[i] * accrual - floating_pv / discount);
        max_error = max_error.max((factors[i] - expected).abs());
        floating_pv += (previous / expected - 1.0) * discount;
        previous = expected;
        target_floating_pv += (previous_target / TARGETS[i] - 1.0) * discount;
        let implied_par = target_floating_pv / (accrual * discount);
        previous_target = TARGETS[i];
        println!(
            "{date}: analytic_df={expected:.12} target_implied_par={:.8}% quote_gap={:.6} bp",
            implied_par * 100.0,
            (implied_par - PAR_RATES[i]) * 10_000.0
        );
    }
    if max_error > 1e-10 {
        return Err(QSError::UnexpectedErr(format!(
            "Annual cashflow check failed: maximum DF error {max_error}"
        )));
    }
    println!("Maximum library / analytic DF difference: {max_error:.3e}");
    Ok(())
}

fn main() -> Result<()> {
    let standard = reconcile(
        Date::new(2026, 10, 2),
        0,
        "Annual",
        DayCounter::Thirty360,
        false,
    )?;
    check_annual_factors(standard, DayCounter::Thirty360)?;
    let mut annual = [0.0; 4];
    for (reference, lag) in [
        (Date::new(2026, 10, 2), 0),
        (Date::new(2026, 10, 1), 1),
        (Date::new(2026, 10, 1), 2),
    ] {
        for fixed in ["Semiannual", "Annual"] {
            let factors = reconcile(reference, lag, fixed, DayCounter::Actual360, false)?;
            if lag == 0 && fixed == "Annual" {
                annual = factors;
            }
            if lag == 1 && fixed == "Annual" {
                for (spot, extracted) in annual.into_iter().zip(factors) {
                    if (spot - extracted).abs() > 1e-10 {
                        return Err(QSError::UnexpectedErr(
                            "Spot normalization changed the reconstructed factors".into(),
                        ));
                    }
                }
            }
        }
    }
    reconcile(
        Date::new(2026, 10, 2),
        0,
        "Annual",
        DayCounter::Actual360,
        true,
    )?;
    check_annual_factors(annual, DayCounter::Actual360)
}
