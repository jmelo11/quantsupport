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
///
/// Resolution is best-effort: missing term fixings leave the claim unchanged.
/// Overnight compounding uses only stored observations in `[start, end)`;
/// absent dates are not validated or filled, and an empty range on an existing
/// index yields an accrual factor of one. Today's fixing is not resolved here.
pub struct FixingPreprocessor {
    ref_date: Date,
    day_counter: DayCounter,
    fixing_store: FixingStore,
}

impl FixingPreprocessor {
    /// Creates a preprocessor for fixings observed before `ref_date`.
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
        let mut observations = fixings.range(start..end).peekable();
        let mut factor = 1.0;
        while let Some((&date, &rate)) = observations.next() {
            let next_date = observations.peek().map_or(end, |(date, _)| **date);
            let dt = self.day_counter.year_fraction(date, next_date);
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

        let Some(fixing_date) = claim.fixing_date() else {
            return;
        };
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compounding_uses_sparse_observations_and_clips_the_final_period() {
        let start = Date::new(2026, 1, 2);
        let end = Date::new(2026, 1, 7);
        let mut store = FixingStore::default();
        for (date, rate) in [
            (start - 1, 0.99),
            (start, 0.04),
            (start + 3, 0.05),
            (end, 0.99),
        ] {
            store.add_fixing(&MarketIndex::SOFR, date, rate);
        }
        let preprocessor = FixingPreprocessor::new(end, DayCounter::Actual360, store);
        let factor = preprocessor
            .compound_daily_fixings(&MarketIndex::SOFR, start, end)
            .unwrap();
        let expected = (1.0 + 0.04 * 3.0 / 360.0) * (1.0 + 0.05 * 2.0 / 360.0);
        assert!((factor - expected).abs() < 1e-14);

        let single = preprocessor
            .compound_daily_fixings(&MarketIndex::SOFR, start + 3, end)
            .unwrap();
        assert!((single - (1.0 + 0.05 * 2.0 / 360.0)).abs() < 1e-14);
        assert_eq!(
            preprocessor.compound_daily_fixings(&MarketIndex::SOFR, start + 1, start + 2),
            Some(1.0)
        );
        assert_eq!(
            preprocessor.compound_daily_fixings(&MarketIndex::SOFR, end, end),
            Some(1.0)
        );
        assert_eq!(
            preprocessor.compound_daily_fixings(&MarketIndex::ESTR, start, end),
            None
        );
    }
}
