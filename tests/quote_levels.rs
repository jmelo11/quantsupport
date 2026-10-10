//! Context quote selection across construction and model calibration.

use quantsupport::prelude::*;
use serde_json::json;

type TestResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

const DEPOSIT: &str = "FixedRateDeposit_USD_SOFR_1D";
const SWAP: &str = "OIS_USD_SOFR_5Y";
const CDS: &str = "Cds_TEST_USD_5Y";
const CAPLET: &str = "CapletFloorlet_USD_SOFR_3M_1Y_Absolute_0.04_Straddle_Black";
const SWAPTION: &str = "Swaption_USD_SOFR_1Y_5Y_Absolute_0.04_Black";

fn context(level: Level, only_selected_level: bool) -> TestResult<PricingContext> {
    let mut quotes = QuoteStore::new(Date::new(2026, 2, 24));
    let surface_quotes: Vec<_> = ["1Y", "2Y"]
        .into_iter()
        .flat_map(|expiry| {
            ["0.02", "0.04", "0.06"].into_iter().map(move |strike| {
                format!("CapletFloorlet_USD_SOFR_3M_{expiry}_Absolute_{strike}_Straddle_Black")
            })
        })
        .collect();
    for (identifier, mid, bid, ask) in [
        (DEPOSIT, 0.04, 0.03, 0.05),
        (SWAP, 0.04, 0.03, 0.05),
        (CDS, 0.01, 0.005, 0.02),
        (SWAPTION, 0.25, 0.20, 0.35),
    ]
    .into_iter()
    .chain(
        surface_quotes
            .iter()
            .map(|id| (id.as_str(), 0.20, 0.15, 0.30)),
    ) {
        quotes.add_quote(Quote::new(
            identifier.parse()?,
            QuoteLevels::new(
                (!only_selected_level || level == Level::Mid).then_some(mid),
                (!only_selected_level || level == Level::Bid).then_some(bid),
                (!only_selected_level || level == Level::Ask).then_some(ask),
            ),
        ));
    }
    Ok(PricingContext::new()
        .with_quote_level(level)
        .with_quote_store(quotes)
        .with_curve_configurations(vec![serde_json::from_value(json!({
            "market_index": "SOFR", "quotes": [DEPOSIT, SWAP]
        }))?])
        .with_credit_curve_configurations(vec![CreditCurveConfiguration::new(
            MarketIndex::Credit("TEST".into()),
            Currency::USD,
            MarketIndex::SOFR,
            0.4,
            vec![CDS.into()],
        )])
        .with_volatility_surface_configurations(vec![VolatilitySurfaceConfiguration::new(
            MarketIndex::SOFR,
            VolatilityType::Black,
            SmileType::Strike,
            surface_quotes,
        )])
        .with_volatility_cube_configurations(vec![VolatilityCubeConfiguration::new(
            MarketIndex::SOFR,
            VolatilityType::Black,
            SmileType::Strike,
            vec![SWAPTION.into()],
        )])
        .with_simulation_configurations(vec![serde_json::from_value(json!({
            "market_index": "SOFR",
            "model": {"HullWhite": {"alpha": 0.05, "parameter_source": {
                "Calibrated": {"source": {"Surface": {"market_index": "SOFR"}},
                    "calibration_basket": {"strike": "Atm"}}
            }}},
            "n_paths": 4, "seed": 42, "horizon": "1Y", "frequency": "Quarterly"
        }))?]))
}

#[test]
fn selected_level_reaches_all_builders_and_xva_calibration() -> TestResult {
    for level in [Level::Mid, Level::Bid, Level::Ask] {
        // With no other levels present, any hard-coded mid in a downstream
        // calibrator fails, even if its constructed surface uses the right side.
        let mut ctx = context(level, true)?;
        ctx.initialize()?;
        let elements = ctx.constructed_elements();
        assert!(elements
            .credit_curve(&MarketIndex::Credit("TEST".into()))
            .is_some());
        let simulation = elements
            .simulations()
            .get(&MarketIndex::SOFR)
            .ok_or("simulation")?;
        assert_eq!(simulation.simulation().borrow().n_paths(), 4);
        let surface = elements
            .volatility_surface(&MarketIndex::SOFR)
            .ok_or("surface")?
            .surface();
        let expected = ctx
            .quote_store()
            .quote(CAPLET)
            .ok_or("caplet")?
            .levels()
            .value(level)?;
        assert!((surface.pillars().ok_or("pillars")?[0].1.value() - expected).abs() < 1e-12);
        let cube = elements
            .volatility_cube(&MarketIndex::SOFR)
            .ok_or("cube")?
            .cube();
        let expected = ctx
            .quote_store()
            .quote(SWAPTION)
            .ok_or("swaption")?
            .levels()
            .value(level)?;
        assert!((cube.pillars().ok_or("pillars")?[0].1.value() - expected).abs() < 1e-12);

        let market = ctx.handle_request(
            &MarketDataRequest::default()
                .with_calibration_request(ModelCalibrationConfiguration::new(
                    CalibrationSource::Surface {
                        market_index: MarketIndex::SOFR,
                    },
                ))
                .with_constructed_elements_request(vec![
                    ConstructedElementRequest::VolatilitySurface {
                        market_index: MarketIndex::SOFR,
                    },
                ]),
        )?;
        assert_eq!(market.quote_level(), level);
        assert!(market.calibration_quotes().is_some());

        let config = serde_json::from_value(json!({
            "model_configs": [{"market_index": "SOFR", "lambda": 0.05,
                "parameter_source": {"Calibrated": {
                    "source": {"Surface": {"market_index": "SOFR"}},
                    "calibration_basket": {"strike": "Atm"}
                }}
            }],
            "n_paths": 4, "seed": 42, "frequency": "Quarterly"
        }))?;
        XvaEngine::new(&ctx, config)?;
    }
    Ok(())
}

#[test]
fn selecting_and_changing_levels_rebuilds_the_market() -> TestResult {
    assert_eq!(PricingContext::new().quote_level(), Level::Mid);
    assert_eq!(PricingContext::default().quote_level(), Level::Mid);
    assert_eq!(MarketData::default().quote_level(), Level::Mid);
    let mut ctx = context(Level::Bid, false)?;
    let mut discount_factors = Vec::new();
    let mut survivals = Vec::new();
    for level in [Level::Bid, Level::Mid, Level::Ask] {
        ctx = ctx.with_quote_level(level);
        ctx.initialize()?;
        let elements = ctx.constructed_elements();
        discount_factors.push(
            elements
                .discount_curve(&MarketIndex::SOFR)
                .ok_or("curve")?
                .curve()
                .discount_factor(Date::new(2030, 2, 24))?
                .value(),
        );
        survivals.push(
            elements
                .credit_curve(&MarketIndex::Credit("TEST".into()))
                .ok_or("credit")?
                .curve()
                .discount_factor(Date::new(2030, 2, 24))?
                .value(),
        );
    }
    assert!(discount_factors.windows(2).all(|pair| pair[0] > pair[1]));
    assert!(survivals.windows(2).all(|pair| pair[0] > pair[1]));
    Ok(())
}

#[test]
fn missing_selected_level_is_an_error() -> TestResult {
    for level in [Level::Bid, Level::Ask] {
        let mut ctx = context(Level::Mid, true)?.with_quote_level(level);
        let error = ctx
            .initialize()
            .err()
            .ok_or("expected missing level error")?;
        assert!(error.to_string().contains(&format!(
            "No {} quote available",
            format!("{level:?}").to_lowercase()
        )));
    }
    Ok(())
}

#[test]
fn scenarios_preserve_the_selected_level_and_base_quotes() -> TestResult {
    let mut ctx = context(Level::Ask, true)?.with_scenarios(vec![Scenario::new(
        CAPLET,
        0.01,
        ScenarioType::Absolute,
    )]);
    for _ in 0..2 {
        ctx.initialize()?;
        let surface = ctx
            .constructed_elements()
            .volatility_surface(&MarketIndex::SOFR)
            .ok_or("surface")?
            .surface();
        let pillars = surface.pillars().ok_or("pillars")?;
        let (_, value) = pillars
            .iter()
            .find(|(label, _)| label == CAPLET)
            .ok_or("caplet pillar")?;
        assert!((value.value() - 0.31).abs() < 1e-12);
        assert_eq!(
            ctx.base_quote_store()
                .quote(CAPLET)
                .ok_or("caplet")?
                .levels()
                .ask(),
            Some(0.30)
        );
    }
    Ok(())
}
