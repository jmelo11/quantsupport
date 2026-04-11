mod utils;

use std::collections::HashMap;

use quantsupport::{
    core::{collateral::SingleCurveCSADiscountPolicy, trade::Side},
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    instruments::{
        fx::{
            fxforward::FxForwardTrade,
            fxoption::{FxOptionTrade, FxOptionType},
            makefxforward::MakeFxForward,
            makefxoption::MakeFxOption,
        },
        rates::{
            crosscurrencyswap::FixFloatCrossCurrencySwapTrade,
            makefixfloatcrosscurrencyswap::MakeFixFloatCrossCurrencySwap, makeswap::MakeSwap,
            swap::SwapTrade,
        },
    },
    models::lgm::{
        lgmcomponents::{LgmFxModel, LgmRateModel},
        lgmmarketmodel::LgmMarketModel,
    },
    rates::{compounding::Compounding, interestrate::RateDefinition},
    time::{
        daycounter::DayCounter,
        enums::{Frequency, TimeUnit},
        schedule::MakeSchedule,
    },
    utils::plot::Plot,
    xva::visitors::{
        exposureevaluator::ExposureEvaluator, inspector::Inspector, marketmodel::MarketModel,
    },
};

use utils::{bootstrap_curves, extract_f64_curve, load_curve_specs, load_quotes};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dc = DayCounter::Actual365;

    // ── 1. Load market data and bootstrap curves ────────────────
    let cwd = std::env::current_dir()?;
    let data_dir = cwd.join("./data");

    let quote_store = load_quotes(&data_dir.join("quotes.json"))?;
    let ref_date = quote_store.reference_date();
    let curve_specs = load_curve_specs(&data_dir.join("curve_specs.json"))?;

    println!("Reference date : {ref_date}");

    let curves = bootstrap_curves(&quote_store, curve_specs)?;

    let sofr_elem = curves
        .get(&MarketIndex::SOFR)
        .expect("SOFR curve not found");
    let estr_elem = curves
        .get(&MarketIndex::ESTR)
        .expect("ESTR curve not found");

    let usd_curve = extract_f64_curve(sofr_elem, ref_date, 35)?;
    let eur_curve = extract_f64_curve(estr_elem, ref_date, 35)?;
    println!("SOFR and ESTR curves bootstrapped.");

    // ── 2. Build trades ─────────────────────────────────────────

    // Trade 1 — 5Y USD IRS (receive fixed @ 3.78%, pay SOFR)
    let swap = MakeSwap::<f64>::default()
        .with_identifier("USD_IRS_5Y".to_string())
        .with_start_date(ref_date)
        .with_maturity_date(ref_date.advance(5, TimeUnit::Years))
        .with_fixed_rate(0.0378)
        .with_notional(10_000_000.0)
        .with_rate_definition(RateDefinition::new(
            DayCounter::Actual360,
            Compounding::Simple,
            Frequency::Semiannual,
        ))
        .with_currency(Currency::USD)
        .with_market_index(MarketIndex::SOFR)
        .with_side(Side::LongReceive)
        .with_fixed_leg_frequency(Frequency::Quarterly)
        .with_floating_leg_frequency(Frequency::Semiannual)
        .build()
        .expect("Failed to build swap");
    let irs_trade = SwapTrade::new(swap, ref_date, 10_000_000.0, Side::LongReceive);

    // Trade 2 — 1Y EUR/USD FX Forward (buy EUR / sell USD at 1.10)
    let fx_fwd = MakeFxForward::default()
        .with_identifier("FX_FWD_EURUSD_1Y".to_string())
        .with_delivery_date(ref_date.advance(1, TimeUnit::Years))
        .with_forward_price(1.10)
        .with_base_currency(Currency::EUR)
        .with_quote_currency(Currency::USD)
        .as_deliverable()
        .build()
        .expect("Failed to build FX forward");
    let fxfwd_trade = FxForwardTrade::new(fx_fwd, ref_date, 5_000_000.0, Side::LongReceive);

    // Trade 3 — 1Y EUR/USD FX call option (buy EUR call, strike 1.12)
    let fx_spot_index = MarketIndex::Other("EURUSD".to_string());
    let fx_opt = MakeFxOption::default()
        .with_identifier("FX_OPT_EURUSD_1Y".to_string())
        .with_expiry_date(ref_date.advance(1, TimeUnit::Years))
        .with_strike(1.12)
        .with_option_type(FxOptionType::Call)
        .with_base_currency(Currency::EUR)
        .with_quote_currency(Currency::USD)
        .with_underlying_index(fx_spot_index.clone())
        .build()
        .expect("Failed to build FX option");
    let fxopt_trade = FxOptionTrade::new(fx_opt, ref_date, 5_000_000.0, Side::LongReceive);
    let fx_option_claims = fxopt_trade.into_contingent_claims()?;

    // Trade 4 — 3Y USD/EUR cross-currency swap (pay USD fixed, receive EUR ESTR float)
    let xccy = MakeFixFloatCrossCurrencySwap::<f64>::default()
        .with_identifier("XCCY_USDEUR_3Y".to_string())
        .with_start_date(ref_date)
        .with_maturity_date(ref_date.advance(3, TimeUnit::Years))
        .with_domestic_notional(10_000_000.0)
        .with_foreign_notional(9_200_000.0)
        .with_fixed_rate(0.038)
        .with_rate_definition(RateDefinition::new(
            DayCounter::Actual360,
            Compounding::Simple,
            Frequency::Semiannual,
        ))
        .with_domestic_currency(Currency::USD)
        .with_foreign_currency(Currency::EUR)
        .with_floating_index(MarketIndex::ESTR)
        .with_side(Side::LongReceive)
        .with_domestic_leg_frequency(Frequency::Semiannual)
        .with_foreign_leg_frequency(Frequency::Quarterly)
        .build()
        .expect("Failed to build cross-currency swap");
    let xccy_trade = FixFloatCrossCurrencySwapTrade::new(
        xccy,
        ref_date,
        10_000_000.0,
        9_200_000.0,
        Side::LongReceive,
    );

    // ── 3. Decompose all trades into contingent claims ──────────
    let irs_claims = irs_trade
        .into_contingent_claims()
        .expect("Failed to decompose IRS");
    let fxfwd_claims = fxfwd_trade
        .into_contingent_claims()
        .expect("Failed to decompose FX forward");
    let fxopt_claims = fx_option_claims;
    let xccy_claims = xccy_trade
        .into_contingent_claims()
        .expect("Failed to decompose cross-currency swap");

    let irs_n = irs_claims.len();
    let fxfwd_n = fxfwd_claims.len();
    let fxopt_n = fxopt_claims.len();
    let xccy_n = xccy_claims.len();

    println!("Claims: IRS={irs_n}, FxFwd={fxfwd_n}, FxOpt={fxopt_n}, Xccy={xccy_n}",);

    // Merge all claims into a single vector so the Inspector assigns
    // globally-unique indices in one pass.
    let mut all_claims = Vec::with_capacity(irs_n + fxfwd_n + fxopt_n + xccy_n);
    all_claims.extend(irs_claims);
    all_claims.extend(fxfwd_claims);
    all_claims.extend(fxopt_claims);
    all_claims.extend(xccy_claims);

    // ── 4. Inspector: assign indices & collect simulation requests
    let discount_policy = SingleCurveCSADiscountPolicy::new(MarketIndex::SOFR, Currency::USD);
    let mut inspector = Inspector::with_discount_policy(Box::new(discount_policy));
    inspector.visit(&mut all_claims);

    let requests: Vec<_> = inspector.requests().to_vec();

    // ── 5. Build LGM market model (USD domestic + EUR foreign) ──
    let n_paths: usize = 1000;

    // Rate model instances for FX model references (kept alive in scope)
    let usd_rate_fx = LgmRateModel::new(0.05, 0.01, &usd_curve);
    let eur_rate_fx = LgmRateModel::new(0.03, 0.008, &eur_curve);
    let eur_fx = LgmFxModel::new(&usd_rate_fx, &eur_rate_fx, 0.08, 1.08, 0.15);

    // Separate instances for the curve models (moved into the market model)
    let usd_rate = LgmRateModel::new(0.05, 0.01, &usd_curve);
    let eur_rate = LgmRateModel::new(0.03, 0.008, &eur_curve);
    let eur_rate_coll = LgmRateModel::new(0.03, 0.008, &eur_curve);

    let mut model = LgmMarketModel::new(Currency::USD, MarketIndex::SOFR, ref_date, dc)
        .with_n_paths(n_paths)
        .with_seed(42);

    model.add_curve_model(MarketIndex::SOFR, usd_rate);
    model.add_curve_model(MarketIndex::ESTR, eur_rate);
    model.add_curve_model(
        MarketIndex::Collateral(Currency::EUR, Currency::USD),
        eur_rate_coll,
    );
    model.add_fx_model(Currency::EUR, eur_fx);
    // Register EUR/USD FX spot index for the FX option
    model.register_fx_spot_index(fx_spot_index, Currency::EUR);

    // ── 6. Set evaluation dates and requests ────────────────────
    let max_maturity = ref_date.advance(5, TimeUnit::Years);
    let schedule = MakeSchedule::new(ref_date, max_maturity)
        .with_frequency(Frequency::Monthly)
        .build()
        .expect("Failed to build evaluation schedule");
    let dates = schedule.dates().clone();

    model.set_evaluation_dates(dates.clone());
    model.set_requests(requests);

    // ── 7. Run ExposureEvaluator ────────────────────────────────
    println!(
        "Running ExposureEvaluator with {n_paths} paths over {} monthly dates...",
        dates.len()
    );

    let evaluator = ExposureEvaluator::<f64>::new(dates.clone(), &model);

    // Slice into the merged claims vector by trade
    let irs_end = irs_n;
    let fxfwd_end = irs_end + fxfwd_n;
    let fxopt_end = fxfwd_end + fxopt_n;
    let xccy_end = fxopt_end + xccy_n;

    let mut trades_map: HashMap<String, &[_]> = HashMap::new();
    trades_map.insert("USD_IRS_5Y".to_string(), &all_claims[..irs_end]);
    trades_map.insert(
        "FX_FWD_EURUSD_1Y".to_string(),
        &all_claims[irs_end..fxfwd_end],
    );
    trades_map.insert(
        "FX_OPT_EURUSD_1Y".to_string(),
        &all_claims[fxfwd_end..fxopt_end],
    );
    trades_map.insert(
        "XCCY_USDEUR_3Y".to_string(),
        &all_claims[fxopt_end..xccy_end],
    );

    let results = evaluator.evaluate(&trades_map);
    println!("Evaluation complete.");

    // ── 8. Extract and print results ────────────────────────────
    let times: Vec<f64> = dates
        .iter()
        .map(|d| dc.year_fraction(ref_date, *d))
        .collect();

    for eval in &results {
        println!("\nTrade: {}", eval.identifier());
        println!("{:<8} {:>14} {:>14} {:>14}", "Time", "EPE", "ENE", "EE");
        for (i, t) in times.iter().enumerate() {
            println!(
                "{:<8.2} {:>14.2} {:>14.2} {:>14.2}",
                t,
                eval.epe()[i],
                eval.ene()[i],
                eval.ee()[i]
            );
        }
    }

    // ── 9. Plot exposure profiles ───────────────────────────────
    let example_dir = data_dir.join("..");
    for eval in &results {
        let filename = format!("exposure_{}.png", eval.identifier().to_lowercase());
        let plot_path = example_dir.join(&filename);
        eval.plot(plot_path.to_str().unwrap())?;
        println!("Plot saved: {}", plot_path.display());
    }

    Ok(())
}
