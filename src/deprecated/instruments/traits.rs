use crate::{
    cashflows::{
        cashflow::{Cashflow, CashflowType, Side},
        simplecashflow::SimpleCashflow,
    },
    currencies::enums::Currency,
    time::date::Date,
    utils::errors::{AtlasError, Result},
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::hash::BuildHasher;

/// # `Structure`
/// A struct that contains the information needed to define a structure.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Structure {
    /// Bullet structure.
    Bullet,
    /// Equal redemptions structure.
    EqualRedemptions,
    /// Zero structure.
    Zero,
    /// Equal payments structure.
    EqualPayments,
    /// Other structure.
    Other,
}

impl TryFrom<String> for Structure {
    type Error = AtlasError;

    fn try_from(s: String) -> Result<Self> {
        match s.as_str() {
            "Bullet" => Ok(Self::Bullet),
            "EqualRedemptions" => Ok(Self::EqualRedemptions),
            "Zero" => Ok(Self::Zero),
            "EqualPayments" => Ok(Self::EqualPayments),
            "Other" => Ok(Self::Other),
            _ => Err(AtlasError::InvalidValueErr(format!(
                "Invalid structure: {s}"
            ))),
        }
    }
}

impl From<Structure> for String {
    fn from(structure: Structure) -> Self {
        match structure {
            Structure::Bullet => "Bullet".to_string(),
            Structure::EqualRedemptions => "EqualRedemptions".to_string(),
            Structure::Zero => "Zero".to_string(),
            Structure::EqualPayments => "EqualPayments".to_string(),
            Structure::Other => "Other".to_string(),
        }
    }
}

impl TryFrom<String> for CashflowType {
    type Error = AtlasError;

    fn try_from(s: String) -> Result<Self> {
        match s.as_str() {
            "Redemption" => Ok(Self::Redemption),
            "Disbursement" => Ok(Self::Disbursement),
            "FixedRateCoupon" => Ok(Self::FixedRateCoupon),
            "FloatingRateCoupon" => Ok(Self::FloatingRateCoupon),
            _ => Err(AtlasError::InvalidValueErr(format!(
                "Invalid cashflow type: {s}"
            ))),
        }
    }
}

impl From<CashflowType> for String {
    fn from(cashflow_type: CashflowType) -> Self {
        match cashflow_type {
            CashflowType::Redemption => "Redemption".to_string(),
            CashflowType::Disbursement => "Disbursement".to_string(),
            CashflowType::FixedRateCoupon => "FixedRateCoupon".to_string(),
            CashflowType::FloatingRateCoupon => "FloatingRateCoupon".to_string(),
        }
    }
}

/// Infer cashflows from amounts to handle negative amounts and sides.
#[must_use]
pub fn infer_cashflows_from_amounts(
    dates: &[Date],
    amounts: &[f64],
    side: Side,
    currency: Currency,
) -> Vec<Cashflow> {
    let mut cashflows = Vec::new();
    dates.iter().zip(amounts).for_each(|(date, amount)| {
        if *amount < 0.0 {
            let cashflow =
                SimpleCashflow::new(*date, currency, side.inverse()).with_amount(amount.abs());
            match side.inverse() {
                Side::Receive => cashflows.push(Cashflow::Redemption(cashflow)),
                Side::Pay => cashflows.push(Cashflow::Disbursement(cashflow)),
            }
        } else {
            let cashflow = SimpleCashflow::new(*date, currency, side).with_amount(*amount);
            match side {
                Side::Receive => cashflows.push(Cashflow::Redemption(cashflow)),
                Side::Pay => cashflows.push(Cashflow::Disbursement(cashflow)),
            }
        }
    });
    cashflows
}

/// This function add cashflows of the given type and side to a vector.
pub fn add_cashflows_to_vec(
    cashflows: &mut Vec<Cashflow>,
    dates: &[Date],
    amounts: &[f64],
    side: Side,
    currency: Currency,
    cashflow_type: CashflowType,
) {
    for (date, amount) in dates.iter().zip(amounts) {
        let cashflow = SimpleCashflow::new(*date, currency, side).with_amount(*amount);
        match cashflow_type {
            CashflowType::Redemption => cashflows.push(Cashflow::Redemption(cashflow)),
            CashflowType::Disbursement => cashflows.push(Cashflow::Disbursement(cashflow)),
            _ => (),
        }
    }
}

/// Calculate the notionals for a given structure
#[must_use]
pub fn notionals_vector(n: usize, notional: f64, structure: Structure) -> Vec<f64> {
    match structure {
        Structure::Bullet => vec![notional; n],
        Structure::EqualRedemptions => {
            let redemptions = vec![
                notional
                    / f64::from(u32::try_from(n).unwrap_or_else(|_| {
                        panic!("notional schedule length should fit in u32")
                    }));
                n
            ];
            let mut results = Vec::new();
            let mut sum = 0.0;
            for r in redemptions {
                results.push(notional - sum);
                sum += r;
            }
            results
        }
        Structure::Zero => vec![notional; 1],
        _ => vec![],
    }
}

/// Calculate the outstanding amounts for a given set of disbursements and redemptions
#[must_use]
pub fn calculate_outstanding(
    disbursements: &HashMap<Date, f64, impl BuildHasher>,
    redemptions: &HashMap<Date, f64, impl BuildHasher>,
    additional_dates: &HashSet<Date, impl BuildHasher>,
) -> Vec<(Date, Date, f64)> {
    let mut outstanding = Vec::new();

    // Combine disbursements and redemptions into a timeline of events
    let mut timeline: Vec<(Date, f64)> = disbursements.iter().map(|(k, v)| (*k, *v)).collect();

    for (date, amount) in redemptions {
        match timeline.iter_mut().find(|(d, _)| *d == *date) {
            Some((_, a)) => *a -= amount,
            None => timeline.push((*date, -amount)),
        }
    }

    // Add zero-value entries for additional dates not in the timeline
    for date in additional_dates {
        if timeline.iter().all(|(d, _)| d != date) {
            timeline.push((*date, 0.0));
        }
    }

    // Sort the timeline based on the date
    timeline.sort_by_key(|k| k.0);

    // Process the timeline
    let mut event_iter = timeline.iter();
    let (mut period_start, mut current_amount) = match event_iter.next() {
        Some((date, amount)) => (*date, *amount),
        None => return Vec::new(),
    };

    for (date, amount) in event_iter {
        let period_end = *date;
        outstanding.push((period_start, period_end, current_amount));
        current_amount += amount;
        period_start = period_end;
    }
    outstanding
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    #[test]
    fn basic_test_cases() {
        let mut redemptions = HashMap::new();
        redemptions.insert(Date::new(2023, 8, 27), 50.0);
        redemptions.insert(Date::new(2023, 9, 27), 50.0);
        redemptions.insert(Date::new(2023, 10, 27), 50.0);

        let mut disbursements = HashMap::new();
        disbursements.insert(Date::new(2023, 6, 27), 150.0);

        let result = calculate_outstanding(&disbursements, &redemptions, &HashSet::new());

        assert_eq!(result.len(), 3);
        assert_eq!(
            result[0],
            (Date::new(2023, 6, 27), Date::new(2023, 8, 27), 150.0)
        );
        assert_eq!(
            result[1],
            (Date::new(2023, 8, 27), Date::new(2023, 9, 27), 100.0)
        );
        assert_eq!(
            result[2],
            (Date::new(2023, 9, 27), Date::new(2023, 10, 27), 50.0)
        );
    }

    #[test]
    fn additional_dates() {
        let mut redemptions = HashMap::new();
        redemptions.insert(Date::new(2023, 8, 27), 50.0);
        redemptions.insert(Date::new(2023, 9, 27), 50.0);
        redemptions.insert(Date::new(2023, 11, 27), 50.0);

        let mut disbursements = HashMap::new();
        disbursements.insert(Date::new(2023, 6, 27), 150.0);

        let mut additional_dates = HashSet::new();
        additional_dates.insert(Date::new(2023, 10, 27));

        let result = calculate_outstanding(&disbursements, &redemptions, &additional_dates);

        assert_eq!(result.len(), 4);
        assert_eq!(
            result[0],
            (Date::new(2023, 6, 27), Date::new(2023, 8, 27), 150.0)
        );
        assert_eq!(
            result[1],
            (Date::new(2023, 8, 27), Date::new(2023, 9, 27), 100.0)
        );
        assert_eq!(
            result[2],
            (Date::new(2023, 9, 27), Date::new(2023, 10, 27), 50.0)
        );
        assert_eq!(
            result[3],
            (Date::new(2023, 10, 27), Date::new(2023, 11, 27), 50.0)
        );
    }

    #[test]
    fn test_add_cashflows_to_vec() {
        let mut cashflows = Vec::new();
        let dates = vec![Date::new(2023, 8, 27), Date::new(2023, 9, 27)];

        let amounts = vec![50.0, 50.0];

        add_cashflows_to_vec(
            &mut cashflows,
            &dates,
            &amounts,
            Side::Receive,
            Currency::USD,
            CashflowType::Redemption,
        );

        assert_eq!(cashflows.len(), 2);
        assert_eq!(
            cashflows[0],
            Cashflow::Redemption(
                SimpleCashflow::new(Date::new(2023, 8, 27), Currency::USD, Side::Receive)
                    .with_amount(50.0)
            )
        );
        assert_eq!(
            cashflows[1],
            Cashflow::Redemption(
                SimpleCashflow::new(Date::new(2023, 9, 27), Currency::USD, Side::Receive,)
                    .with_amount(50.0)
            )
        );
    }

    //#[test]
    //fn test_add_cashflows_to_vec_negative_amount() {
    //    let mut cashflows = Vec::new();
    //    let dates = vec![Date::new(2023, 8, 27),
    //                                Date::new(2023, 9, 27)];

    //    let amounts = vec![50.0,
    //                                 50.0];
    //
    //    add_cashflows_to_vec(
    //        &mut cashflows,
    //        &dates,
    //        &amounts,
    //        Side::Receive,
    //        Currency::USD,
    //        CashflowType::Redemption,
    //    );

    //    assert_eq!(cashflows.len(), 2);
    //    assert_eq!(
    //        cashflows[0],
    //        Cashflow::Redemption(SimpleCashflow::new(
    //            Date::new(2023, 8, 27),
    //            Currency::USD,
    //            Side::Receive
    //        )
    //        .with_amount(50.0))
    //    );
    //    assert_eq!(
    //        cashflows[1],
    //        Cashflow::Disbursement(SimpleCashflow::new(
    //            Date::new(2023, 9, 27),
    //            Currency::USD,
    //            Side::Pay,
    //        )
    //        .with_amount(50.0))
    //    );
    //}
}
