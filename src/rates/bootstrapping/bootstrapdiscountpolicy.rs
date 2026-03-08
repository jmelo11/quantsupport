use std::collections::HashMap;

use crate::{
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    quotes::quote::BuiltInstrument,
    utils::errors::{QSError, Result},
};

/// Discount-curve resolution policy used during the bootstrap.
///
/// * **Derivatives** (swaps, basis-swaps, xccy-swaps) are discounted with the
///   CSA / collateral curve — typically SOFR for USD-posted collateral.
/// * **Fixed-income** instruments (deposits) are self-discounted: the deposit
///   rate *is* the zero rate, so the discount curve equals the projection curve.
/// * **Futures** are margined daily and carry no present-value discounting.
/// * **FX forwards / forward points** relate two discount curves via covered
///   interest-rate parity; each leg maps to the curve of its own currency.
///
/// For cross-currency legs the policy can look up a per-currency collateral
/// curve (`Collateral(ccy, csa_ccy)`).
pub struct BootstrapDiscountPolicy {
    /// Primary CSA discount index (e.g. SOFR).
    csa_index: MarketIndex,
    /// Currency of the primary CSA curve.
    csa_currency: Currency,
    /// Per-currency override for the collateral discount curve.
    /// Populated for legs whose cashflow currency differs from the CSA
    /// currency, e.g. `CLP → Collateral(CLP, USD)`.
    collateral_by_currency: HashMap<Currency, MarketIndex>,
}

impl BootstrapDiscountPolicy {
    /// Creates a new bootstrap discount policy.
    ///
    /// `csa_index` and `csa_currency` describe the primary collateral curve.
    #[must_use]
    pub fn new(csa_index: MarketIndex, csa_currency: Currency) -> Self {
        Self {
            csa_index,
            csa_currency,
            collateral_by_currency: HashMap::new(),
        }
    }

    /// Registers a collateral discount curve for a specific cashflow currency.
    ///
    /// For example, to discount CLP cashflows under a USD CSA, register
    /// `(CLP, Collateral(CLP, USD))` via this method.
    #[must_use]
    pub fn with_collateral_curve(
        mut self,
        cashflow_currency: Currency,
        discount_index: MarketIndex,
    ) -> Self {
        self.collateral_by_currency
            .insert(cashflow_currency, discount_index);
        self
    }

    /// Returns the CSA / primary collateral index.
    #[must_use]
    pub fn csa_index(&self) -> &MarketIndex {
        &self.csa_index
    }

    /// Returns the CSA currency.
    #[must_use]
    pub fn csa_currency(&self) -> Currency {
        self.csa_currency
    }

    /// Resolves the discount curve for a given cashflow currency.
    ///
    /// * If the currency matches the CSA currency the primary CSA curve is
    ///   returned.
    /// * Otherwise the per-currency collateral override is used if available.
    /// * Falls back to the CSA curve when no override is registered.
    #[must_use]
    pub fn discount_index_for_currency(&self, cashflow_currency: Currency) -> MarketIndex {
        if cashflow_currency == self.csa_currency {
            return self.csa_index.clone();
        }
        self.collateral_by_currency
            .get(&cashflow_currency)
            .cloned()
            .unwrap_or_else(|| self.csa_index.clone())
    }

    /// Resolves the discount curve index for a built instrument.
    ///
    /// `target_index` is the curve currently being bootstrapped.
    ///
    /// # Errors
    /// Returns an error when a required per-currency collateral curve has not
    /// been registered.
    pub fn discount_index(
        &self,
        built: &BuiltInstrument,
        target_index: &MarketIndex,
    ) -> Result<MarketIndex> {
        match built {
            // Fixed-income: self-discounted
            BuiltInstrument::FixedRateDeposit(_) => Ok(target_index.clone()),

            // Vanilla OIS swap: CSA curve
            BuiltInstrument::Swap(_) => Ok(self.csa_index.clone()),

            // Basis swap: CSA curve (both legs share the same discount curve)
            BuiltInstrument::BasisSwap(_) => Ok(self.csa_index.clone()),

            // Futures: not discounted — return a sentinel (the target curve)
            BuiltInstrument::RateFutures(_) => Ok(target_index.clone()),

            // Cross-currency swaps: per-leg discounting handled in the NPV
            // loop; here we return the CSA curve as the primary reference.
            BuiltInstrument::CrossCurrencySwap(_) => Ok(self.csa_index.clone()),

            // FX forward / forward points: per-currency discounting handled
            // in the NPV computation; return the CSA curve as a reference.
            BuiltInstrument::FxForward(_) => Ok(self.csa_index.clone()),

            _ => Err(QSError::InvalidValueErr(
                "Unsupported instrument for bootstrap discounting".into(),
            )),
        }
    }

    /// Returns all external discount-curve dependencies for an instrument
    /// during bootstrapping.
    ///
    /// Dependencies that equal `target_index` are **excluded** because they
    /// refer to the curve being solved for.
    pub fn dependencies(
        &self,
        built: &BuiltInstrument,
        target_index: &MarketIndex,
    ) -> Vec<MarketIndex> {
        let mut deps = Vec::new();

        match built {
            // Deposits are self-discounted — no external dependencies.
            BuiltInstrument::FixedRateDeposit(_) => {}

            // Vanilla swap: needs the CSA discount curve; the floating-leg
            // projection curve is the target.
            BuiltInstrument::Swap(s) => {
                Self::push_if_different(&mut deps, &self.csa_index, target_index);
                // floating leg projection
                let proj = s.market_index();
                Self::push_if_different(&mut deps, &proj, target_index);
            }

            // Basis swap: needs CSA + both legs' projection curves.
            BuiltInstrument::BasisSwap(bs) => {
                Self::push_if_different(&mut deps, &self.csa_index, target_index);
                let pay = bs.pay_market_index();
                let rec = bs.receive_market_index();
                Self::push_if_different(&mut deps, &pay, target_index);
                Self::push_if_different(&mut deps, &rec, target_index);
            }

            // Cross-currency swap: needs per-currency collateral curves +
            // both legs' projection curves.
            BuiltInstrument::CrossCurrencySwap(xccy) => {
                let dom_disc = self.discount_index_for_currency(xccy.domestic_currency());
                let for_disc = self.discount_index_for_currency(xccy.foreign_currency());
                Self::push_if_different(&mut deps, &dom_disc, target_index);
                Self::push_if_different(&mut deps, &for_disc, target_index);
                let dom_proj = xccy.domestic_market_index();
                let for_proj = xccy.foreign_market_index();
                Self::push_if_different(&mut deps, &dom_proj, target_index);
                Self::push_if_different(&mut deps, &for_proj, target_index);
            }

            // Rate futures: only the projection curve.
            BuiltInstrument::RateFutures(f) => {
                let proj = f.market_index();
                Self::push_if_different(&mut deps, &proj, target_index);
            }

            // FX forwards: both currency discount curves.
            BuiltInstrument::FxForward(fx) => {
                let base = self.discount_index_for_currency(fx.base_currency());
                let quote = self.discount_index_for_currency(fx.quote_currency());
                Self::push_if_different(&mut deps, &base, target_index);
                Self::push_if_different(&mut deps, &quote, target_index);
            }

            _ => {}
        }

        // Deduplicate without requiring Ord.
        let mut seen = std::collections::HashSet::new();
        deps.retain(|d| seen.insert(d.clone()));
        deps
    }

    /// Returns every discount curve index referenced by this policy.
    #[must_use]
    pub fn all_discount_indices(&self) -> Vec<MarketIndex> {
        let mut indices = vec![self.csa_index.clone()];
        for idx in self.collateral_by_currency.values() {
            if !indices.contains(idx) {
                indices.push(idx.clone());
            }
        }
        indices
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn push_if_different(
        out: &mut Vec<MarketIndex>,
        candidate: &MarketIndex,
        exclude: &MarketIndex,
    ) {
        if candidate != exclude && !out.contains(candidate) {
            out.push(candidate.clone());
        }
    }
}
