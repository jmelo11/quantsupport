use std::collections::{HashMap, HashSet, VecDeque};

use crate::{
    ad::dual::DualFwd,
    core::pillars::Pillars,
    currencies::currency::Currency,
    utils::errors::{QSError, Result},
};

/// Stores FX spot rates as [`DualFwd`] values so that sensitivities to exchange
/// rates are captured automatically by the AD tape.
///
/// Rates are stored as directed pairs `(base, quote) → rate` meaning
/// *1 unit of base = rate units of quote*.  Triangulation via BFS is
/// performed when a direct rate is not available.
#[derive(Clone, Debug, Default)]
pub struct FxStore {
    exchange_rate_map: HashMap<(Currency, Currency), DualFwd>,
}

impl Pillars<DualFwd> for FxStore {
    fn pillar_labels(&self) -> Option<Vec<String>> {
        Some(
            self.exchange_rate_map
                .keys()
                .map(|(base, quote)| format!("{base}/{quote}"))
                .collect(),
        )
    }

    fn pillars(&self) -> Option<Vec<(String, &DualFwd)>> {
        Some(
            self.exchange_rate_map
                .iter()
                .map(|((base, quote), rate)| (format!("{base}/{quote}"), rate))
                .collect(),
        )
    }

    fn put_pillars_on_tape(&mut self) {
        for rate in self.exchange_rate_map.values_mut() {
            rate.put_on_tape();
        }
    }
}

impl FxStore {
    /// Creates an empty store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            exchange_rate_map: HashMap::new(),
        }
    }

    /// Inserts a spot rate: 1 `base` = `rate` `quote`.
    pub fn add_fx_rate(&mut self, base: Currency, quote: Currency, rate: DualFwd) {
        self.exchange_rate_map.insert((base, quote), rate);
    }

    /// Retrieves the exchange rate `base → quote`, composing via BFS if no
    /// direct rate is stored.
    ///
    /// Returns [`DualFwd`] so that the dependence on intermediate rates is
    /// tracked on the AD tape.
    ///
    /// # Errors
    /// Returns an error if no path between the two currencies can be found.
    pub fn get_fx_rate(&self, base: Currency, quote: Currency) -> Result<DualFwd> {
        if base == quote {
            return Ok(DualFwd::one());
        }

        // Direct lookup
        if let Some(&rate) = self.exchange_rate_map.get(&(base, quote)) {
            return Ok(rate);
        }

        // BFS triangulation
        let mut queue: VecDeque<(Currency, DualFwd)> = VecDeque::new();
        let mut visited: HashSet<Currency> = HashSet::new();
        queue.push_back((base, DualFwd::one()));
        visited.insert(base);

        while let Some((current, accumulated)) = queue.pop_front() {
            for (&(src, dst), &map_rate) in &self.exchange_rate_map {
                if src == current && !visited.contains(&dst) {
                    let composed: DualFwd = (accumulated * map_rate).into();
                    if dst == quote {
                        return Ok(composed);
                    }
                    visited.insert(dst);
                    queue.push_back((dst, composed));
                } else if dst == current && !visited.contains(&src) {
                    let composed: DualFwd = (accumulated / map_rate).into();
                    if src == quote {
                        return Ok(composed);
                    }
                    visited.insert(src);
                    queue.push_back((src, composed));
                }
            }
        }

        Err(QSError::NotFoundErr(format!(
            "No exchange rate path between {base:?} and {quote:?}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::currencies::currency::Currency::{CLP, EUR, USD};

    #[test]
    fn test_same_currency() {
        let store = FxStore::new();
        let rate = store.get_fx_rate(USD, USD).unwrap();
        assert!((rate.value() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_direct_rate() {
        let mut store = FxStore::new();
        store.add_fx_rate(USD, EUR, DualFwd::from(0.85));

        let rate = store.get_fx_rate(USD, EUR).unwrap();
        assert!((rate.value() - 0.85).abs() < f64::EPSILON);
    }

    #[test]
    fn test_inverse_rate() {
        let mut store = FxStore::new();
        store.add_fx_rate(USD, EUR, DualFwd::from(0.85));

        let rate = store.get_fx_rate(EUR, USD).unwrap();
        assert!((rate.value() - 1.0 / 0.85).abs() < 1e-12);
    }

    #[test]
    fn test_nonexistent_rate() {
        let store = FxStore::new();
        assert!(store.get_fx_rate(USD, EUR).is_err());
    }

    #[test]
    fn test_triangulation() {
        let mut store = FxStore::new();
        store.add_fx_rate(CLP, USD, DualFwd::from(800.0));
        store.add_fx_rate(USD, EUR, DualFwd::from(1.1));

        let clp_eur = store.get_fx_rate(CLP, EUR).unwrap();
        assert!((clp_eur.value() - 1.1 * 800.0).abs() < 1e-6);

        let eur_clp = store.get_fx_rate(EUR, CLP).unwrap();
        assert!((eur_clp.value() - 1.0 / (1.1 * 800.0)).abs() < 1e-12);
    }

    #[test]
    fn test_pillars_labels() {
        let mut store = FxStore::new();
        store.add_fx_rate(USD, EUR, DualFwd::from(0.92));
        store.add_fx_rate(USD, CLP, DualFwd::from(950.0));

        let mut labels = store
            .pillars()
            .into_iter()
            .flatten()
            .map(|(label, _)| label)
            .collect::<Vec<String>>();
        labels.sort();
        assert_eq!(labels.len(), 2);
        assert!(labels.contains(&"USD/CLP".to_string()));
        assert!(labels.contains(&"USD/EUR".to_string()));
    }
}
