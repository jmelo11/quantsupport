use super::*;
use crate::prelude::*;

fn context() -> Result<PricingContext> {
    let date = Date::new(2026, 1, 15);
    let mut quotes = QuoteStore::new(date);
    let labels = [
        "FixedRateDeposit_EUR_ESTR_1Y",
        "FixedRateDeposit_EUR_ESTR_2Y",
    ];
    for (label, value) in labels.iter().zip([0.03, 0.04]) {
        quotes.add_quote(Quote::new(
            label.parse()?,
            QuoteLevels::new(Some(value), Some(value - 0.001), Some(value + 0.001)),
        ));
    }
    Ok(PricingContext::new()
        .with_quote_store(quotes)
        .with_base_currency(Currency::EUR)
        .with_base_index(MarketIndex::ESTR)
        .with_curve_configurations(vec![CurveConfiguration::new(
            MarketIndex::ESTR,
            DayCounter::Actual365,
            Interpolator::LogLinear,
            true,
            labels.iter().map(|label| (*label).into()).collect(),
        )]))
}

fn definition() -> RateDefinition {
    RateDefinition::new(
        DayCounter::Actual365,
        Compounding::Continuous,
        Frequency::Annual,
    )
}

fn factor_risk(
    provider: &impl MarketDataProvider,
    index: MarketIndex,
    date: Date,
) -> Result<(f64, HashMap<String, f64>)> {
    let _tape = ScratchTape::new();
    let request = MarketDataRequest::default().with_constructed_elements_request(vec![
        ConstructedElementRequest::DiscountCurve {
            market_index: index.clone(),
        },
    ]);
    let mut market = provider.handle_request(&request)?;
    let element = market
        .constructed_elements_mut()
        .discount_curves_mut()
        .get_mut(&index)
        .unwrap();
    Tape::start_recording_fwd();
    element.curve_mut().put_pillars_on_tape();
    let curve = element.curve();
    let factor = curve.discount_factor(date)?;
    factor.backward()?;
    let mut risk = HashMap::new();
    for (label, value) in curve.pillars().unwrap_or_default() {
        *risk.entry(label).or_insert(0.0) += value.adjoint()?.value();
    }
    Ok((factor.value(), risk))
}

#[test]
fn zero_risk_matches_node_bumps_and_preserves_off_grid_prices() -> Result<()> {
    let context = context()?;
    let index = MarketIndex::ESTR;
    let view = context.with_zero_rate_risk(&[ZeroRateRisk::new(index.clone(), definition())])?;
    let query = Date::new(2027, 7, 15);
    let (base_value, _) = factor_risk(&context, index.clone(), query)?;
    let (value, risk) = factor_risk(&view, index.clone(), query)?;
    assert_eq!(value, base_value);
    let curves = requested_curves(&context, &[index])?;
    let nodes = curves[&MarketIndex::ESTR].curve().nodes().unwrap();
    let dates: Vec<_> = nodes.iter().map(|(date, _)| *date).collect();
    for (column, label) in view.target_labels().iter().enumerate() {
        let bump = 1e-6;
        let mut values = Vec::new();
        for shift in [bump, -bump] {
            let factors = nodes
                .iter()
                .enumerate()
                .map(|(node, (date, factor))| {
                    let time =
                        DayCounter::Actual365.year_fraction(context.evaluation_date(), *date);
                    if node == column + 1 {
                        factor.value() * (-shift * time).exp()
                    } else {
                        factor.value()
                    }
                })
                .collect();
            let bumped = DiscountTermStructure::<f64>::new(
                dates.clone(),
                factors,
                DayCounter::Actual365,
                Interpolator::LogLinear,
                true,
            )?;
            values.push(bumped.discount_factor(query)?);
        }
        let finite_difference = (values[0] - values[1]) / (2.0 * bump);
        assert!((risk[label] - finite_difference).abs() < 1e-8);
    }
    Ok(())
}

#[test]
fn zero_risk_conventions_custom_dates_and_pricer_integration() -> Result<()> {
    let context = context()?;
    let maturity = Date::new(2027, 7, 15);
    let rate_definition = RateDefinition::new(
        DayCounter::Actual360,
        Compounding::Compounded,
        Frequency::Semiannual,
    );
    let view =
        context.with_zero_rate_risk(&[ZeroRateRisk::new(MarketIndex::ESTR, rate_definition)
            .with_dates(vec![Date::new(2027, 1, 15), Date::new(2028, 1, 15)])])?;
    let deposit = MakeFixedRateDeposit::<DualFwd>::default()
        .with_identifier("trade".into())
        .with_start_date(context.evaluation_date())
        .with_maturity_date(maturity)
        .with_notional(100.0)
        .with_rate(0.035)
        .with_rate_definition(RateDefinition::default())
        .with_currency(Currency::EUR)
        .with_discount_index(Some(MarketIndex::ESTR))
        .build()?;
    let trade =
        FixedRateDepositTrade::new(deposit, context.evaluation_date(), 100.0, Side::LongReceive);
    let pricer =
        DiscountedCashflowPricer::<FixedRateDeposit<DualFwd>, FixedRateDepositTrade<DualFwd>>::new(
        );
    let base = pricer.evaluate(&trade, &[Request::Value, Request::Sensitivities], &context)?;
    for _ in 0..2 {
        let result = pricer.evaluate(&trade, &[Request::Value, Request::Sensitivities], &view)?;
        assert_eq!(result.price(), base.price());
        let sensitivities = result.sensitivities().unwrap();
        assert_eq!(sensitivities.instrument_keys(), view.target_labels());
        assert!(sensitivities
            .exposure()
            .iter()
            .any(|value| value.abs() > 0.0));
    }
    let (factor, risk) = factor_risk(&view, MarketIndex::ESTR, Date::new(2027, 1, 15))?;
    let zero = view.target_values()[0];
    let time =
        DayCounter::Actual360.year_fraction(context.evaluation_date(), Date::new(2027, 1, 15));
    let expected = -time * factor / (1.0 + zero / 2.0);
    assert!((risk[&view.target_labels()[0]] - expected).abs() < 1e-9);
    Ok(())
}

#[test]
fn instrument_reprojection_matches_independent_target_rebootstrap() -> Result<()> {
    let context = context()?;
    let targets: Vec<QuoteDetails> = ["OIS_EUR_ESTR_1Y", "OIS_EUR_ESTR_2Y"]
        .iter()
        .map(|label| label.parse())
        .collect::<Result<_>>()?;
    let view = context.reproject_risk(&targets)?;
    let query = Date::new(2027, 7, 15);
    let (value, risk) = factor_risk(&view, MarketIndex::ESTR, query)?;
    assert_eq!(value, factor_risk(&context, MarketIndex::ESTR, query)?.0);
    for (column, label) in view.target_labels().iter().enumerate() {
        let bump = 1e-6;
        let mut prices = Vec::new();
        for shift in [bump, -bump] {
            let mut quotes = QuoteStore::new(context.evaluation_date());
            for (row, target) in targets.iter().enumerate() {
                quotes.add_quote(Quote::new(
                    target.clone(),
                    QuoteLevels::with_mid(
                        view.target_values()[row] + if column == row { shift } else { 0.0 },
                    ),
                ));
            }
            let config = CurveConfiguration::new(
                MarketIndex::ESTR,
                DayCounter::Actual365,
                Interpolator::LogLinear,
                true,
                view.target_labels().to_vec(),
            );
            let bumped = context
                .clone()
                .with_quote_store(quotes)
                .with_curve_configurations(vec![config]);
            prices.push(factor_risk(&bumped, MarketIndex::ESTR, query)?.0);
        }
        assert!(
            (risk[label] - (prices[0] - prices[1]) / (2.0 * bump)).abs() < 5e-4,
            "{label}: {} vs {}",
            risk[label],
            (prices[0] - prices[1]) / (2.0 * bump)
        );
    }
    Ok(())
}

#[test]
fn identity_reprojection_respects_selected_quote_side() -> Result<()> {
    for level in [Level::Mid, Level::Bid, Level::Ask] {
        let context = context()?.with_quote_level(level);
        let targets: Vec<_> = context.curve_configurations()[0]
            .quotes()
            .iter()
            .map(|label| label.parse())
            .collect::<Result<_>>()?;
        let view = context.reproject_risk(&targets)?;
        let query = Date::new(2027, 7, 15);
        let native = factor_risk(&context, MarketIndex::ESTR, query)?;
        let projected = factor_risk(&view, MarketIndex::ESTR, query)?;
        assert_eq!(native.0, projected.0);
        for (label, exposure) in native.1 {
            assert!((exposure - projected.1[&label]).abs() < 1e-5);
        }
        for (label, value) in view.target_labels().iter().zip(view.target_values()) {
            assert!(
                (context
                    .quote_store()
                    .quote(label)
                    .unwrap()
                    .levels()
                    .value(level)?
                    - value)
                    .abs()
                    < 1e-10
            );
        }
    }
    Ok(())
}

#[test]
fn transformation_preserves_existing_tape_and_native_pillars() -> Result<()> {
    let context = context()?;
    let curves = requested_curves(&context, &[MarketIndex::ESTR])?;
    let _tape = ScratchTape::new();
    Tape::start_recording_fwd();
    let x = DualFwd::new(2.0);
    let y: DualFwd = (x * x).into();
    let mut curve = curves[&MarketIndex::ESTR].clone();
    curve.curve_mut().put_pillars_on_tape();
    let pointers: Vec<_> = curve
        .curve()
        .pillars()
        .unwrap()
        .iter()
        .map(|(_, value)| value.node_ptr())
        .collect();
    let view =
        context.with_zero_rate_risk(&[ZeroRateRisk::new(MarketIndex::ESTR, definition())])?;
    assert_eq!(
        pointers,
        curve
            .curve()
            .pillars()
            .unwrap()
            .iter()
            .map(|(_, value)| value.node_ptr())
            .collect::<Vec<_>>()
    );
    let _ = factor_risk(&view, MarketIndex::ESTR, Date::new(2027, 7, 15))?;
    assert!(context.with_zero_rate_risk(&[]).is_err());
    y.backward()?;
    assert_eq!(x.adjoint()?.value(), 4.0);
    Ok(())
}

#[test]
fn rejects_empty_duplicate_and_unspanned_bases() -> Result<()> {
    let context = context()?;
    let grid = ZeroRateRisk::new(MarketIndex::ESTR, definition());
    assert!(context.with_zero_rate_risk(&[]).is_err());
    assert!(context.reproject_risk(&[]).is_err());
    for dates in [
        vec![],
        vec![context.evaluation_date()],
        vec![Date::new(2027, 1, 15)],
        vec![Date::new(2027, 1, 15); 2],
    ] {
        assert!(context
            .with_zero_rate_risk(&[grid.clone().with_dates(dates)])
            .is_err());
    }
    assert!(context
        .reproject_risk(&["OIS_EUR_ESTR_1Y".parse()?])
        .is_err());
    assert!(context
        .reproject_risk(&["OIS_EUR_ESTR_1Y".parse()?, "OIS_EUR_ESTR_1Y".parse()?])
        .is_err());
    assert!(context
        .reproject_risk(&["OIS_EUR_ESTR_3M".parse()?, "OIS_EUR_ESTR_6M".parse()?])
        .is_err());
    Ok(())
}

#[test]
fn zero_risk_supports_supplied_curves_and_redundant_target_grids() -> Result<()> {
    let date = Date::new(2026, 1, 15);
    let curve = FlatForwardTermStructure::new(date, DualFwd::from(0.03), definition())
        .with_pillar_label("flat".into());
    let mut store = ConstructedElementStore::default();
    store.discount_curves_mut().insert(
        MarketIndex::ESTR,
        DiscountCurveElement::new(MarketIndex::ESTR, Rc::new(RefCell::new(curve))),
    );
    let context = PricingContext::new()
        .with_quote_store(QuoteStore::new(date))
        .with_constructed_elements(store);
    let grid = ZeroRateRisk::new(MarketIndex::ESTR, definition())
        .with_dates(vec![Date::new(2027, 1, 15), Date::new(2028, 1, 15)]);
    let view = context.with_zero_rate_risk(&[grid])?;
    let query = Date::new(2027, 7, 15);
    let (value, risk) = factor_risk(&view, MarketIndex::ESTR, query)?;
    let time = DayCounter::Actual365.year_fraction(date, query);
    for label in view.target_labels() {
        assert!((risk[label] + time * value / 2.0).abs() < 1e-10);
    }
    assert!(context
        .reproject_risk(&["FixedRateDeposit_EUR_ESTR_1Y".parse()?])
        .is_err());
    Ok(())
}

#[test]
fn joint_zero_basis_removes_bootstrap_parent_risk_and_partial_basis_retains_other_quotes(
) -> Result<()> {
    let date = Date::new(2026, 2, 24);
    let mut quotes = QuoteStore::new(date);
    let parent = [
        "FixedRateDeposit_USD_SOFR_1Y",
        "FixedRateDeposit_USD_SOFR_2Y",
    ];
    let child = ["Swap_USD_TermSOFR3m_1Y", "Swap_USD_TermSOFR3m_2Y"];
    for (label, value) in parent.iter().chain(&child).zip([0.04, 0.039, 0.045, 0.046]) {
        quotes.add_quote(Quote::new(label.parse()?, QuoteLevels::with_mid(value)));
    }
    let configurations = [
        (MarketIndex::SOFR, parent),
        (MarketIndex::TermSOFR3m, child),
    ]
    .into_iter()
    .map(|(index, labels)| {
        CurveConfiguration::new(
            index,
            DayCounter::Actual360,
            Interpolator::LogLinear,
            true,
            labels.iter().map(|label| (*label).into()).collect(),
        )
    })
    .collect();
    let context = PricingContext::new()
        .with_quote_store(quotes)
        .with_curve_configurations(configurations);
    let query = Date::new(2028, 2, 24);
    let (base, native) = factor_risk(&context, MarketIndex::TermSOFR3m, query)?;
    assert!(parent.iter().any(|label| native[*label].abs() > 1e-5));

    let partial =
        context.with_zero_rate_risk(&[ZeroRateRisk::new(MarketIndex::SOFR, definition())])?;
    let (partial_value, partial_risk) = factor_risk(&partial, MarketIndex::TermSOFR3m, query)?;
    assert_eq!(partial_value, base);
    for label in child {
        assert!((partial_risk[label] - native[label]).abs() < 1e-10);
    }
    assert!(parent
        .iter()
        .all(|label| !partial_risk.contains_key(*label)));

    let joint = context.with_zero_rate_risk(&[
        ZeroRateRisk::new(MarketIndex::SOFR, definition()),
        ZeroRateRisk::new(MarketIndex::TermSOFR3m, definition()),
    ])?;
    let (joint_value, joint_risk) = factor_risk(&joint, MarketIndex::TermSOFR3m, query)?;
    assert_eq!(joint_value, base);
    for label in &joint.target_labels()[..2] {
        assert!(joint_risk[label].abs() < 1e-8);
    }
    let targets = parent
        .iter()
        .chain(&child)
        .map(|label| label.parse())
        .collect::<Result<Vec<_>>>()?;
    let identity = context.reproject_risk(&targets)?;
    let (projected_value, projected_risk) = factor_risk(&identity, MarketIndex::TermSOFR3m, query)?;
    assert_eq!(projected_value, base);
    for (label, exposure) in native {
        assert!((exposure - projected_risk[&label]).abs() < 1e-4, "{label}");
    }
    Ok(())
}
