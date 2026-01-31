use crate::{
    cashflows::{
        cashflow::{Cashflow, CashflowType},
        traits::Payable,
    },
    time::date::Date,
};

/// A visitor trait that mutably visits an object of type `T`.
pub trait Visit<T> {
    /// The type returned by the visit operation.
    type Output;
    /// Visits the given mutable visitable object and returns an output.
    fn visit(&self, visitable: &mut T) -> Self::Output;
}

/// A visitor trait that immutably visits an object of type `T`.
pub trait ConstVisit<T> {
    /// The type returned by the visit operation.
    type Output;
    /// Visits the given immutable visitable object and returns an output.
    fn visit(&self, visitable: &T) -> Self::Output;
}

/// A trait for objects that have cashflows.
pub trait HasCashflows {
    /// Returns a slice of cashflows.
    fn cashflows(&self) -> &[Cashflow];

    /// Returns a mutable slice of cashflows.
    fn mut_cashflows(&mut self) -> &mut [Cashflow];

    /// Sets the discount curve ID for all cashflows.
    fn set_discount_curve_id(&mut self, id: usize) {
        self.mut_cashflows()
            .iter_mut()
            .for_each(|cf| cf.set_discount_curve_id(id));
    }
    /// Sets the forecast curve ID for all floating rate coupons.
    fn set_forecast_curve_id(&mut self, id: usize) {
        self.mut_cashflows().iter_mut().for_each(|cf| {
            if let Cashflow::FloatingRateCoupon(frcf) = cf {
                frcf.set_forecast_curve_id(id);
            }
        });
    }

    /// Finds the next cashflow of the specified type after the reference date.
    fn next_cashflow(&self, reference_date: Date, cashflow_type: CashflowType) -> Option<Cashflow> {
        match cashflow_type {
            CashflowType::Disbursement => self
                .cashflows()
                .iter()
                .filter(|cf| matches!(cf, Cashflow::Disbursement(_)))
                .filter(|cf| cf.payment_date() > reference_date)
                .min_by(|cf1, cf2| cf1.payment_date().cmp(&cf2.payment_date()))
                .copied(),
            CashflowType::Redemption => self
                .cashflows()
                .iter()
                .filter(|cf| matches!(cf, Cashflow::Redemption(_)))
                .filter(|cf| cf.payment_date() > reference_date)
                .min_by(|cf1, cf2| cf1.payment_date().cmp(&cf2.payment_date()))
                .copied(),
            CashflowType::FixedRateCoupon => self
                .cashflows()
                .iter()
                .filter(|cf| matches!(cf, Cashflow::FixedRateCoupon(_)))
                .filter(|cf| cf.payment_date() > reference_date)
                .min_by(|cf1, cf2| cf1.payment_date().cmp(&cf2.payment_date()))
                .copied(),

            CashflowType::FloatingRateCoupon => self
                .cashflows()
                .iter()
                .filter(|cf| matches!(cf, Cashflow::FloatingRateCoupon(_)))
                .filter(|cf| cf.payment_date() > reference_date)
                .min_by(|cf1, cf2| cf1.payment_date().cmp(&cf2.payment_date()))
                .copied(),
        }
    }
}
