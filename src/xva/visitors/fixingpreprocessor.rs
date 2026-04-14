use crate::{
    indices::marketindex::MarketIndex,
    quotes::fixingstore::FixingStore,
    time::{date::Date, daycounter::DayCounter},
    xva::{claimevaluationstrategy::ClaimEvaluationStrategy, contigentclaim::ContingentClaim},
};

use super::claimpreprocessor::ClaimPreprocessor;

/// Resolves realized fixings for claims whose fixing date is before the
/// reference date, using historical observations from a [`FixingStore`].
///
/// * **Not-in-arrears** (term rate): if `fixing_date < ref_date` the rate
///   is fully determined → looked up directly from the store and stored
///   as `realized_fixing`.
/// * **In-arrears** (OIS):
///   - If `accrual_end ≤ ref_date` → fully realized.  Daily fixings are
///     compounded into a single implied rate.
///   - If `accrual_start < ref_date < accrual_end` → partially realized;
///     daily fixings from `accrual_start` to `ref_date` are compounded
///     and stored as a `partial_fixing` so only the remaining period
///     needs simulation.
pub struct FixingPreprocessor {
    ref_date: Date,
    day_counter: DayCounter,
    fixing_store: FixingStore,
}

impl FixingPreprocessor {
    #[must_use]
    pub const fn new(ref_date: Date, day_counter: DayCounter, fixing_store: FixingStore) -> Self {
        Self {
            ref_date,
            day_counter,
            fixing_store,
        }
    }

    /// Compounds daily fixings from `start` (inclusive) up to `end` (exclusive)
    /// into a single accrual factor: ∏(1 + rᵢ × Δtᵢ).
    ///
    /// Returns `None` if no fixings are available for the index.
    fn compound_daily_fixings(&self, index: &MarketIndex, start: Date, end: Date) -> Option<f64> {
        let fixings = self.fixing_store.fixings(index).ok()?;
        let mut factor = 1.0;
        // Collect dates in [start, end) that have fixings.
        let dates: Vec<Date> = fixings.range(start..end).map(|(d, _)| *d).collect();
        for (i, &d) in dates.iter().enumerate() {
            let rate = fixings[&d];
            let next = if i + 1 < dates.len() {
                dates[i + 1]
            } else {
                end
            };
            let dt = self.day_counter.year_fraction(d, next);
            factor *= rate.mul_add(dt, 1.0);
        }
        Some(factor)
    }
}

impl ClaimPreprocessor for FixingPreprocessor {
    fn process(&self, claim: &mut ContingentClaim) {
        let is_rate_claim = matches!(
            claim.evaluation_strategy(),
            ClaimEvaluationStrategy::LinearRate { .. }
                | ClaimEvaluationStrategy::NonLinearRate { .. }
        );
        if !is_rate_claim {
            return;
        }

        let Some(fixing_date) = claim.fixing_date() else { return };
        if fixing_date >= self.ref_date {
            return;
        }

        let market_index = match claim.index() {
            Some(idx) => idx.clone(),
            None => return,
        };

        let in_arrears = market_index
            .rate_index_details()
            .map_or(true, |d| d.is_in_arrears());

        let accrual_start = claim.accrual_start().unwrap_or(fixing_date);
        let accrual_end = claim.accrual_end().unwrap_or_else(|| claim.payment_date());

        if !in_arrears {
            // Term rate: single historical fixing at fixing_date.
            if let Ok(rate) = self.fixing_store.fixing(&market_index, fixing_date) {
                claim.set_realized_fixing(rate);
            }
        } else if accrual_end <= self.ref_date {
            // In-arrears, fully realized: compound daily fixings.
            if let Some(factor) =
                self.compound_daily_fixings(&market_index, accrual_start, accrual_end)
            {
                let tau = self.day_counter.year_fraction(accrual_start, accrual_end);
                if tau.abs() > 1e-14 {
                    let rate = (factor - 1.0) / tau;
                    claim.set_realized_fixing(rate);
                }
            }
        } else if accrual_start < self.ref_date {
            // In-arrears, partially realized: compound daily fixings up to ref_date.
            if let Some(realized_factor) =
                self.compound_daily_fixings(&market_index, accrual_start, self.ref_date)
            {
                claim.set_partial_fixing(realized_factor, accrual_start, self.ref_date);
            }
        }
    }
}
