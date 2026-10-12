//! Python bindings for the quantsupport derivative pricing library.

mod bondpricinganchor;
mod collateral;
mod context;
mod conv;
mod curves;
mod enums;
mod explore;
mod extension;
mod market;
mod results;
mod scripting;
mod time;
mod trades;
mod xva;

use pyo3::prelude::*;

pyo3::create_exception!(
    quantsupport,
    QuantSupportError,
    pyo3::exceptions::PyException,
    "Raised when a quantsupport operation fails."
);

#[pymodule]
fn quantsupport(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Enums
    m.add_class::<enums::Level>()?;
    m.add_class::<enums::Currency>()?;
    m.add_class::<enums::MarketIndex>()?;
    m.add_class::<enums::Side>()?;
    m.add_class::<enums::Compounding>()?;
    m.add_class::<enums::Frequency>()?;
    m.add_class::<enums::DayCounter>()?;
    m.add_class::<enums::TimeUnit>()?;
    m.add_class::<enums::BusinessDayConvention>()?;
    m.add_class::<enums::DateGenerationRule>()?;
    m.add_class::<enums::Request>()?;
    m.add_class::<enums::VolatilityType>()?;
    m.add_class::<enums::SmileType>()?;
    m.add_class::<enums::ScenarioType>()?;
    m.add_class::<enums::OptionType>()?;
    m.add_class::<enums::CapFloorType>()?;
    m.add_class::<enums::CapletFloorletType>()?;
    m.add_class::<enums::PaymentStructure>()?;
    // Time
    m.add_class::<time::Date>()?;
    m.add_class::<time::Period>()?;
    m.add_class::<time::Calendar>()?;
    // Market data & configurations
    m.add_class::<market::QuoteStore>()?;
    m.add_class::<market::FixingStore>()?;
    m.add_class::<market::FxStore>()?;
    m.add_class::<market::CurveConfiguration>()?;
    m.add_class::<market::CreditCurveConfiguration>()?;
    m.add_class::<market::VolatilitySurfaceConfiguration>()?;
    m.add_class::<market::VolatilityCubeConfiguration>()?;
    m.add_class::<market::PricerConfiguration>()?;
    m.add_class::<market::SimulationConfiguration>()?;
    m.add_class::<market::Scenario>()?;
    m.add_class::<curves::ZeroRateTermStructure>()?;
    m.add_class::<curves::TenorBasedZeroRateTermStructure>()?;
    // Constructed element views
    m.add_class::<explore::DiscountCurve>()?;
    m.add_class::<explore::VolatilitySurface>()?;
    m.add_class::<explore::VolatilityCube>()?;
    m.add_class::<explore::Simulation>()?;
    // Context
    m.add_class::<collateral::SingleCurveCSADiscountPolicy>()?;
    m.add_class::<collateral::FixedIncomeDiscountPolicy>()?;
    m.add_class::<bondpricinganchor::BondPricingAnchorConfig>()?;
    m.add_class::<context::PricingContext>()?;
    // Python-native extension points
    m.add_class::<extension::Instrument>()?;
    m.add_class::<extension::Trade>()?;
    m.add_class::<extension::Pricer>()?;
    m.add_class::<extension::MarketDataProvider>()?;
    m.add_class::<extension::Evaluator>()?;
    m.add_class::<enums::SwaptionType>()?;
    m.add_class::<trades::EquityForward>()?;
    m.add_class::<trades::EuropeanSwaption>()?;
    // Instruments
    m.add_class::<trades::Swap>()?;
    m.add_class::<trades::CrossCurrencySwap>()?;
    m.add_class::<trades::BasisSwap>()?;
    m.add_class::<trades::FixFloatCrossCurrencySwap>()?;
    m.add_class::<trades::FixedRateBond>()?;
    m.add_class::<trades::Coupon>()?;
    m.add_class::<trades::CallScheduleEntry>()?;
    m.add_class::<trades::SteppedCouponBond>()?;
    m.add_class::<trades::FloatingRateNote>()?;
    m.add_class::<trades::FixedRateDeposit>()?;
    m.add_class::<trades::FxForwardPy>()?;
    m.add_class::<trades::FxOptionPy>()?;
    m.add_class::<trades::EquityOption>()?;
    m.add_class::<trades::CreditDefaultSwapPy>()?;
    m.add_class::<trades::CapFloorPy>()?;
    m.add_class::<trades::CapletFloorletPy>()?;
    m.add_class::<trades::RateFuturesPy>()?;
    m.add_class::<scripting::ScriptedProduct>()?;
    // Results
    m.add_class::<results::EvaluationResults>()?;
    // XVA
    m.add_class::<xva::XvaConfig>()?;
    m.add_class::<xva::CsaTerms>()?;
    m.add_class::<xva::NettingSet>()?;
    m.add_class::<xva::XvaResult>()?;
    m.add_class::<xva::ExposureProfile>()?;
    m.add("QuantSupportError", m.py().get_type::<QuantSupportError>())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::ffi::CString;

    use pyo3::{prelude::*, types::PyDict};

    #[test]
    fn python_zero_curve_contracts() {
        Python::initialize();
        Python::attach(|py| {
            let run = || -> PyResult<()> {
                let module = PyModule::new(py, "quantsupport")?;
                super::quantsupport(&module)?;
                py.import("sys")?
                    .getattr("modules")?
                    .set_item("quantsupport", module)?;
                let globals = PyDict::new(py);
                globals.set_item("__name__", "zero_curve_contract_tests")?;
                let source = CString::new(include_str!("../tests/test_zero_curves.py"))
                    .map_err(|error| pyo3::exceptions::PyValueError::new_err(error.to_string()))?;
                py.run(&source, Some(&globals), None)?;
                py.run(c"suite = unittest.defaultTestLoader.loadTestsFromTestCase(ZeroCurveTests)\nresult = unittest.TextTestRunner(verbosity=2).run(suite)\nassert result.wasSuccessful(), 'Zero-curve contract tests failed'", Some(&globals), None)
            };
            if let Err(error) = run() {
                error.print(py);
                panic!("Python zero-curve contract tests failed");
            }
        });
    }

    #[test]
    fn python_dict_and_script_contracts() {
        Python::initialize();
        Python::attach(|py| {
            let run = || -> PyResult<()> {
                let module = PyModule::new(py, "quantsupport")?;
                super::quantsupport(&module)?;
                py.import("sys")?
                    .getattr("modules")?
                    .set_item("quantsupport", module)?;
                let globals = PyDict::new(py);
                globals.set_item("__name__", "binding_contract_tests")?;
                globals.set_item(
                    "__file__",
                    concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/tests/test_dict_and_scripting.py"
                    ),
                )?;
                let source = CString::new(include_str!("../tests/test_dict_and_scripting.py"))
                    .map_err(|error| pyo3::exceptions::PyValueError::new_err(error.to_string()))?;
                py.run(&source, Some(&globals), None)?;
                py.run(c"suite = unittest.defaultTestLoader.loadTestsFromTestCase(DictAndScriptTests)\nresult = unittest.TextTestRunner(verbosity=2).run(suite)\nassert result.wasSuccessful(), 'Python binding contract tests failed'", Some(&globals), None)?;
                globals.set_item(
                    "__file__",
                    concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/tests/test_trade_instrument.py"
                    ),
                )?;
                let source = CString::new(include_str!("../tests/test_trade_instrument.py"))
                    .map_err(|error| pyo3::exceptions::PyValueError::new_err(error.to_string()))?;
                py.run(&source, Some(&globals), None)?;
                py.run(c"suite = unittest.defaultTestLoader.loadTestsFromTestCase(TradeInstrumentTests)\nresult = unittest.TextTestRunner(verbosity=2).run(suite)\nassert result.wasSuccessful(), 'Trade/instrument contract tests failed'", Some(&globals), None)
            };
            if let Err(error) = run() {
                error.print(py);
                panic!("Python binding contract tests failed");
            }
        });
    }
}
