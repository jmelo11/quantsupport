//! Python bindings for the quantsupport derivative pricing library.

mod common;
mod market;
mod instruments;
mod pricing;
mod time;
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
    m.add_class::<common::enums::Level>()?;
    m.add_class::<common::enums::Interpolator>()?;
    m.add_class::<common::enums::UnitedStatesMarket>()?;
    m.add_class::<common::enums::BrazilMarket>()?;
    m.add_class::<common::enums::JointCalendarRule>()?;
    m.add_class::<common::enums::Currency>()?;
    m.add_class::<common::enums::MarketIndex>()?;
    m.add_class::<common::enums::Side>()?;
    m.add_class::<common::enums::Compounding>()?;
    m.add_class::<common::enums::Frequency>()?;
    m.add_class::<common::enums::DayCounter>()?;
    m.add_class::<common::enums::TimeUnit>()?;
    m.add_class::<common::enums::BusinessDayConvention>()?;
    m.add_class::<common::enums::DateGenerationRule>()?;
    m.add_class::<common::enums::Request>()?;
    m.add_class::<common::enums::VolatilityType>()?;
    m.add_class::<common::enums::SmileType>()?;
    m.add_class::<common::enums::ScenarioType>()?;
    m.add_class::<common::enums::OptionType>()?;
    m.add_class::<common::enums::CapFloorType>()?;
    m.add_class::<common::enums::CapletFloorletType>()?;
    m.add_class::<common::enums::PaymentStructure>()?;
    // Time
    m.add_class::<time::Date>()?;
    m.add_class::<time::Period>()?;
    m.add_class::<time::Calendar>()?;
    // Market data & configurations
    m.add_class::<market::stores::QuoteStore>()?;
    m.add_class::<market::stores::FixingStore>()?;
    m.add_class::<market::stores::FxStore>()?;
    m.add_class::<market::stores::CurveConstruction>()?;
    m.add_class::<market::stores::CurveConfiguration>()?;
    m.add_class::<market::stores::CreditCurveConfiguration>()?;
    m.add_class::<market::stores::VolatilitySurfaceConfiguration>()?;
    m.add_class::<market::stores::VolatilityCubeConfiguration>()?;
    m.add_class::<market::stores::PricerConfiguration>()?;
    m.add_class::<market::stores::SimulationConfiguration>()?;
    m.add_class::<market::scenario::Shock>()?;
    m.add_class::<market::scenario::Scenario>()?;
    m.add_class::<market::curves::ZeroRateTermStructure>()?;
    m.add_class::<market::curves::TenorBasedZeroRateTermStructure>()?;
    // Constructed element views
    m.add_class::<market::views::DiscountCurve>()?;
    m.add_class::<market::views::VolatilitySurface>()?;
    m.add_class::<market::views::VolatilityCube>()?;
    m.add_class::<market::views::Simulation>()?;
    // Context
    m.add_class::<pricing::collateral::SingleCurveCSADiscountPolicy>()?;
    m.add_class::<pricing::collateral::FixedIncomeDiscountPolicy>()?;
    m.add_class::<pricing::bond_anchor::BondPricingAnchorConfig>()?;
    m.add_class::<pricing::context::PricingContext>()?;
    // Python-native extension points
    m.add_class::<pricing::extension::Instrument>()?;
    m.add_class::<pricing::extension::Trade>()?;
    m.add_class::<pricing::extension::Pricer>()?;
    m.add_class::<pricing::extension::MarketDataProvider>()?;
    m.add_class::<pricing::extension::Evaluator>()?;
    m.add_class::<common::enums::SwaptionType>()?;
    m.add_class::<instruments::products::EquityForward>()?;
    m.add_class::<instruments::products::EuropeanSwaption>()?;
    // Instruments
    m.add_class::<instruments::products::Swap>()?;
    m.add_class::<instruments::loan::Loan>()?;
    m.add_class::<pricing::loan::LoanPricer>()?;
    m.add_class::<instruments::loan_terms::LoanRepaymentStructure>()?;
    m.add_class::<instruments::loan_terms::PrepaymentModelConfiguration>()?;
    m.add_class::<instruments::products::CrossCurrencySwap>()?;
    m.add_class::<instruments::products::BasisSwap>()?;
    m.add_class::<instruments::products::FixFloatCrossCurrencySwap>()?;
    m.add_class::<instruments::products::FixedRateBond>()?;
    m.add_class::<instruments::coupons::InterestRate>()?;
    m.add_class::<instruments::coupons::PayoffOps>()?;
    m.add_class::<instruments::coupons::FixedRateCoupon>()?;
    m.add_class::<instruments::coupons::FloatingRateCoupon>()?;
    m.add_class::<instruments::coupons::OptionEmbeddedCoupon>()?;
    m.add_class::<instruments::products::CallScheduleEntry>()?;
    m.add_class::<instruments::products::SteppedCouponBond>()?;
    m.add_class::<instruments::products::FloatingRateNote>()?;
    m.add_class::<instruments::products::FixedRateDeposit>()?;
    m.add_class::<instruments::products::FxForwardPy>()?;
    m.add_class::<instruments::products::FxOptionPy>()?;
    m.add_class::<instruments::products::EquityOption>()?;
    m.add_class::<instruments::products::CreditDefaultSwapPy>()?;
    m.add_class::<instruments::products::CapFloorPy>()?;
    m.add_class::<instruments::products::CapletFloorletPy>()?;
    m.add_class::<instruments::products::RateFuturesPy>()?;
    m.add_class::<instruments::scripting::ScriptedProduct>()?;
    // Results
    m.add_class::<pricing::results::EvaluationResults>()?;
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
