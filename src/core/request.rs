use crate::ad::adreal::IsReal;
use crate::{
    core::evaluationresults::{CashflowsTable, SensitivityMap},
    instruments::cashflows::{
        cashflow::Cashflow, cashflowtype::CashflowType, coupons::NonLinearCoupon, leg::Leg,
    },
    utils::errors::Result,
};

/// A [`Request`] of different types of requests that can be made for instrument evaluation,
/// including value, yield to maturity, modified duration, sensitivities, and cashflows.
pub enum Request {
    /// Price
    Value,
    /// Yield to maturity
    YieldToMaturity,
    /// Modified Duration
    ModifiedDuration,
    /// Sensitivities
    Sensitivities,
    /// Cashflows
    Cashflows,
}

impl Request {
    /// Returns the rank of the request, which can be used for ordering or prioritization.
    #[must_use]
    pub const fn rank(&self) -> u8 {
        match self {
            Self::Value => 0,
            Self::Sensitivities => 1,
            Self::YieldToMaturity => 2,
            Self::ModifiedDuration => 3,
            Self::Cashflows => 4,
        }
    }
}

/// The [`HandleValue`] trait defines a method for handling price-related operations.
pub trait HandleValue<T, S> {
    /// Handles price-related operations and returns a floating-point result.
    ///
    /// ## Errors
    /// Returns an error if the evaluation fails.
    fn handle_value(&self, trade: &T, state: &mut S) -> Result<f64>;
}

/// The [`HandleYieldToMaturity`] trait defines a method for handling yield-related operations.
pub trait HandleYieldToMaturity<T, S> {
    /// Handles yield-related operations and returns a floating-point result.
    ///
    /// ## Errors
    /// Returns an error if the evaluation fails.
    fn handle_yield_to_maturity(&self, trade: &T, state: &mut S) -> Result<f64>;
}

/// The [`HandleModifiedDuration`] trait defines a method for handling modified duration operations.
pub trait HandleModifiedDuration<T, S> {
    /// Handles modified duration operations and returns a floating-point result.
    ///
    /// ## Errors
    /// Returns an error if the evaluation fails.
    fn handle_modified_duration(&self, trade: &T, state: &mut S) -> Result<f64>;
}

/// The [`HandleSensitivities`] trait defines a method for handling sensitivities-related operations.
pub trait HandleSensitivities<T, S> {
    /// Handles sensitivities-related operations and returns a sensitivy map result.
    ///
    /// ## Errors
    /// Returns an error if the evaluation fails.
    fn handle_sensitivities(&self, trade: &T, state: &mut S) -> Result<SensitivityMap>;
}

/// The [`LegProvLegsProviderider`] trait defines a method for providing the cashflows of an instrument.
pub trait LegsProvider {
    /// Provides the cashflows of the instrument.
    fn legs(&self) -> &[Leg];
}

/// The [`HandleCashflows`] trait defines a method for handling a cashflows request.
/// The generic type parameter `T` represents the type of trade, and `S` represents the state or context in which the cashflows are evaluated.
/// As cashflows often depend on current evaluation and market conditions, the mutable state is responsable
/// for proving the a resolved instance of the legs and underlying cashflows.
pub trait HandleCashflows<T, S: LegsProvider> {
    /// Handles cashflow-related operations and returns a vector of cashflows.
    ///
    /// ## Errors
    /// Returns an error if the evaluation fails.
    fn handle_cashflows(&self, _trade: &T, state: &mut S) -> Result<CashflowsTable> {
        let mut cashflows_table = CashflowsTable::new();

        // Iterate through all legs provided by the state
        for leg in state.legs() {
            let currency = leg.currency().clone();
            let fx_parity = 1.0; // Default FX parity; can be enhanced with market data

            // Process each cashflow in the leg
            for cashflow in leg.cashflows() {
                match cashflow {
                    CashflowType::FixedRateCoupon(coupon) => {
                        let amount = coupon.amount()?.value();
                        let payment_date = coupon.payment_date();
                        let accrual_start = coupon.accrual_start_date();
                        let accrual_end = coupon.accrual_end_date();
                        let accrual_period = coupon
                            .rate()
                            .day_counter()
                            .year_fraction(accrual_start, accrual_end);

                        cashflows_table.add_cashflow(
                            payment_date,
                            "FixedRateCoupon".to_string(),
                            amount,
                            None,
                            accrual_period,
                            currency.clone(),
                            fx_parity,
                            None,
                            None,
                        );
                    }
                    CashflowType::FloatingRateCoupon(coupon) => {
                        let amount = coupon.amount()?.value();
                        let payment_date = coupon.payment_date();
                        let accrual_start = coupon.accrual_start_date();
                        let accrual_end = coupon.accrual_end_date();
                        let accrual_period = coupon
                            .day_counter()
                            .year_fraction(accrual_start, accrual_end);

                        // Extract fixing if available (usually set during evaluation)
                        let fixing = coupon.fixing().map(|f| f.value());

                        cashflows_table.add_cashflow(
                            payment_date,
                            "FloatingRateCoupon".to_string(),
                            amount,
                            fixing,
                            accrual_period,
                            currency.clone(),
                            fx_parity,
                            None,
                            None,
                        );
                    }
                    CashflowType::OptionEmbeddedCoupon(coupon) => {
                        let amount = coupon.amount()?.value();
                        let payment_date = coupon.payment_date();
                        let accrual_start = coupon.accrual_start_date();
                        let accrual_end = coupon.accrual_end_date();

                        // Use Actual360 as default day counter for accrual period calculation
                        let day_counter = crate::time::daycounter::DayCounter::Actual360;
                        let accrual_period = day_counter.year_fraction(accrual_start, accrual_end);

                        let fixing = coupon.fixing().map(|f| f.value());

                        cashflows_table.add_cashflow(
                            payment_date,
                            "OptionEmbeddedCoupon".to_string(),
                            amount,
                            fixing,
                            accrual_period,
                            currency.clone(),
                            fx_parity,
                            None,
                            None,
                        );
                    }
                    CashflowType::Redemption(cashflow) => {
                        let amount = cashflow.amount()?.value();
                        let payment_date = cashflow.payment_date();

                        cashflows_table.add_cashflow(
                            payment_date,
                            "Redemption".to_string(),
                            amount,
                            None,
                            0.0,
                            currency.clone(),
                            fx_parity,
                            None,
                            None,
                        );
                    }
                    CashflowType::Disbursement(cashflow) => {
                        let amount = cashflow.amount()?.value();
                        let payment_date = cashflow.payment_date();

                        cashflows_table.add_cashflow(
                            payment_date,
                            "Disbursement".to_string(),
                            amount,
                            None,
                            0.0,
                            currency.clone(),
                            fx_parity,
                            None,
                            None,
                        );
                    }
                }
            }
        }

        Ok(cashflows_table)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ad::adreal::ADReal,
        currencies::currency::Currency,
        instruments::cashflows::{
            cashflow::SimpleCashflow, fixedratecoupon::FixedRateCoupon, leg::Leg,
        },
        rates::interestrate::InterestRate,
        time::{date::Date, daycounter::DayCounter, enums::Frequency},
    };

    /// Mock trade type for testing
    struct MockTrade;

    /// Mock state that implements LegsProvider for testing
    struct MockState {
        legs: Vec<Leg>,
    }

    impl LegsProvider for MockState {
        fn legs(&self) -> &[Leg] {
            &self.legs
        }
    }

    /// Mock handler that implements HandleCashflows trait
    struct MockHandler;

    impl HandleCashflows<MockTrade, MockState> for MockHandler {}

    fn create_test_rate() -> InterestRate<ADReal> {
        InterestRate::from_rate_definition(
            ADReal::new(0.05),
            crate::rates::interestrate::RateDefinition::new(
                DayCounter::Actual360,
                crate::rates::compounding::Compounding::Simple,
                Frequency::Annual,
            ),
        )
    }

    #[test]
    fn test_handle_cashflows_with_fixed_rate_coupon() {
        let start_date = Date::new(2024, 1, 1);
        let end_date = Date::new(2024, 4, 1);
        let payment_date = Date::new(2024, 4, 1);

        let rate = create_test_rate();
        let coupon = FixedRateCoupon::new(
            100_000.0,
            Box::new(rate),
            start_date,
            end_date,
            payment_date,
        );
        let cashflow = CashflowType::FixedRateCoupon(coupon);

        let leg = Leg::new(
            0,
            vec![cashflow],
            Currency::USD,
            None,
            None,
            None,
            crate::core::trade::Side::PayShort,
            true,
        );

        let mut state = MockState { legs: vec![leg] };

        let handler = MockHandler;
        let result = handler.handle_cashflows(&MockTrade, &mut state);

        assert!(result.is_ok(), "handle_cashflows should succeed");

        let table = result.unwrap();
        assert_eq!(
            table.payment_dates().len(),
            1,
            "Table should have exactly one cashflow"
        );
        assert_eq!(
            table.cashflow_types()[0],
            "FixedRateCoupon",
            "Cashflow type should be FixedRateCoupon"
        );
        assert!(
            table.amounts()[0] > 0.0,
            "Amount should be positive for a coupon"
        );
    }

    #[test]
    fn test_handle_cashflows_with_redemption() {
        let payment_date = Date::new(2025, 1, 1);
        let redemption = SimpleCashflow::new(100_000.0, payment_date);
        let cashflow = CashflowType::Redemption(redemption);

        let leg = Leg::new(
            0,
            vec![cashflow],
            Currency::USD,
            None,
            None,
            None,
            crate::core::trade::Side::PayShort,
            true,
        );

        let mut state = MockState { legs: vec![leg] };

        let handler = MockHandler;
        let result = handler.handle_cashflows(&MockTrade, &mut state);

        assert!(result.is_ok());

        let table = result.unwrap();
        assert_eq!(table.payment_dates().len(), 1);
        assert_eq!(table.cashflow_types()[0], "Redemption");
        assert_eq!(table.amounts()[0], 100_000.0);
    }

    #[test]
    fn test_handle_cashflows_with_disbursement() {
        let payment_date = Date::new(2024, 1, 1);
        let disbursement = SimpleCashflow::new(100_000.0, payment_date);
        let cashflow = CashflowType::Disbursement(disbursement);

        let leg = Leg::new(
            0,
            vec![cashflow],
            Currency::USD,
            None,
            None,
            None,
            crate::core::trade::Side::PayShort,
            true,
        );

        let mut state = MockState { legs: vec![leg] };

        let handler = MockHandler;
        let result = handler.handle_cashflows(&MockTrade, &mut state);

        assert!(result.is_ok());

        let table = result.unwrap();
        assert_eq!(table.payment_dates().len(), 1);
        assert_eq!(table.cashflow_types()[0], "Disbursement");
        assert_eq!(table.amounts()[0], 100_000.0);
    }

    #[test]
    fn test_handle_cashflows_with_multiple_cashflows() {
        let start_date = Date::new(2024, 1, 1);
        let coupon_date = Date::new(2024, 4, 1);
        let redemption_date = Date::new(2025, 1, 1);

        let rate = create_test_rate();
        let coupon = FixedRateCoupon::new(
            100_000.0,
            Box::new(rate),
            start_date,
            coupon_date,
            coupon_date,
        );
        let coupon_cf = CashflowType::FixedRateCoupon(coupon);

        let redemption = SimpleCashflow::new(100_000.0, redemption_date);
        let redemption_cf = CashflowType::Redemption(redemption);

        let disbursement = SimpleCashflow::new(100_000.0, start_date);
        let disbursement_cf = CashflowType::Disbursement(disbursement);

        let leg = Leg::new(
            0,
            vec![disbursement_cf, coupon_cf, redemption_cf],
            Currency::USD,
            None,
            None,
            None,
            crate::core::trade::Side::PayShort,
            true,
        );

        let mut state = MockState { legs: vec![leg] };

        let handler = MockHandler;
        let result = handler.handle_cashflows(&MockTrade, &mut state);

        assert!(result.is_ok());

        let table = result.unwrap();
        assert_eq!(
            table.payment_dates().len(),
            3,
            "Table should have three cashflows"
        );
        assert_eq!(table.cashflow_types()[0], "Disbursement");
        assert_eq!(table.cashflow_types()[1], "FixedRateCoupon");
        assert_eq!(table.cashflow_types()[2], "Redemption");
    }

    #[test]
    fn test_handle_cashflows_with_multiple_legs() {
        let date1 = Date::new(2024, 1, 1);
        let date2 = Date::new(2024, 7, 1);

        // First leg
        let redemption1 = SimpleCashflow::new(50_000.0, date1);
        let leg1 = Leg::new(
            0,
            vec![CashflowType::Redemption(redemption1)],
            Currency::USD,
            None,
            None,
            None,
            crate::core::trade::Side::PayShort,
            true,
        );

        // Second leg
        let redemption2 = SimpleCashflow::new(50_000.0, date2);
        let leg2 = Leg::new(
            1,
            vec![CashflowType::Redemption(redemption2)],
            Currency::EUR,
            None,
            None,
            None,
            crate::core::trade::Side::LongRecieve,
            true,
        );

        let mut state = MockState {
            legs: vec![leg1, leg2],
        };

        let handler = MockHandler;
        let result = handler.handle_cashflows(&MockTrade, &mut state);

        assert!(result.is_ok());

        let table = result.unwrap();
        assert_eq!(
            table.payment_dates().len(),
            2,
            "Table should have cashflows from both legs"
        );
        assert_eq!(table.amounts()[0], 50_000.0);
        assert_eq!(table.amounts()[1], 50_000.0);
        assert_eq!(table.currencies()[0], Currency::USD);
        assert_eq!(table.currencies()[1], Currency::EUR);
    }

    #[test]
    fn test_handle_cashflows_accrual_period_calculation() {
        let start_date = Date::new(2024, 1, 1);
        let end_date = Date::new(2024, 1, 31); // 30 days
        let payment_date = Date::new(2024, 1, 31);

        let rate = create_test_rate();
        let coupon = FixedRateCoupon::new(
            100_000.0,
            Box::new(rate),
            start_date,
            end_date,
            payment_date,
        );
        let cashflow = CashflowType::FixedRateCoupon(coupon);

        let leg = Leg::new(
            0,
            vec![cashflow],
            Currency::USD,
            None,
            None,
            None,
            crate::core::trade::Side::PayShort,
            true,
        );

        let mut state = MockState { legs: vec![leg] };

        let handler = MockHandler;
        let result = handler.handle_cashflows(&MockTrade, &mut state);

        assert!(result.is_ok());

        let table = result.unwrap();
        assert!(
            table.accrual_periods()[0] > 0.0 && table.accrual_periods()[0] < 1.0,
            "Accrual period should be a fraction of a year"
        );
    }

    #[test]
    fn test_handle_cashflows_simple_cashflow_has_zero_accrual() {
        let payment_date = Date::new(2025, 1, 1);
        let redemption = SimpleCashflow::new(100_000.0, payment_date);
        let cashflow = CashflowType::Redemption(redemption);

        let leg = Leg::new(
            0,
            vec![cashflow],
            Currency::USD,
            None,
            None,
            None,
            crate::core::trade::Side::PayShort,
            true,
        );

        let mut state = MockState { legs: vec![leg] };

        let handler = MockHandler;
        let result = handler.handle_cashflows(&MockTrade, &mut state);

        assert!(result.is_ok());

        let table = result.unwrap();
        assert_eq!(
            table.accrual_periods()[0],
            0.0,
            "Redemptions should have zero accrual period"
        );
    }

    #[test]
    fn test_handle_cashflows_empty_legs() {
        let state = MockState { legs: vec![] };

        let mut state = state;

        let handler = MockHandler;
        let result = handler.handle_cashflows(&MockTrade, &mut state);

        assert!(result.is_ok());

        let table = result.unwrap();
        assert_eq!(table.payment_dates().len(), 0);
    }

    #[test]
    fn test_handle_cashflows_currency_preservation() {
        let date = Date::new(2024, 1, 1);
        let redemption = SimpleCashflow::new(100_000.0, date);

        let leg1 = Leg::new(
            0,
            vec![CashflowType::Redemption(redemption.clone())],
            Currency::USD,
            None,
            None,
            None,
            crate::core::trade::Side::PayShort,
            true,
        );

        let leg2 = Leg::new(
            1,
            vec![CashflowType::Redemption(redemption)],
            Currency::GBP,
            None,
            None,
            None,
            crate::core::trade::Side::PayShort,
            true,
        );

        let mut state = MockState {
            legs: vec![leg1, leg2],
        };

        let handler = MockHandler;
        let result = handler.handle_cashflows(&MockTrade, &mut state);

        assert!(result.is_ok());

        let table = result.unwrap();
        assert_eq!(table.currencies()[0], Currency::USD);
        assert_eq!(table.currencies()[1], Currency::GBP);
    }
}
