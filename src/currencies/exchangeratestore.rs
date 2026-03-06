use std::collections::{HashMap, HashSet, VecDeque};

use crate::{
    ad::adreal::{ADReal, IsReal},
    core::pillars::Pillars,
    currencies::currency::Currency,
    utils::errors::{QSError, Result},
};

/// Stores FX spot rates as [`ADReal`] values so that sensitivities to exchange
/// rates are captured automatically by the AD tape.
///
/// Rates are stored as directed pairs `(base, quote) → rate` meaning
/// *1 unit of base = rate units of quote*.  Triangulation via BFS is
/// performed when a direct rate is not available.
#[derive(Clone)]
pub struct ExchangeRateStore {
    exchange_rate_map: HashMap<(Currency, Currency), ADReal>,
}

impl Default for ExchangeRateStore {
    fn default() -> Self {
        Self::new()
    }
}

impl Pillars<ADReal> for ExchangeRateStore {
    fn pillar_labels(&self) -> Option<Vec<String>> {
        Some(
            self.exchange_rate_map
                .keys()
                .map(|(base, quote)| format!("{base}/{quote}"))
                .collect(),
        )
    }

    fn pillars(&self) -> Option<Vec<(String, &ADReal)>> {
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

impl ExchangeRateStore {
    /// Creates an empty store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            exchange_rate_map: HashMap::new(),
        }
    }

    /// Inserts a spot rate: 1 `base` = `rate` `quote`.
    pub fn add_exchange_rate(&mut self, base: Currency, quote: Currency, rate: ADReal) {
        self.exchange_rate_map.insert((base, quote), rate);
    }

    /// Retrieves the exchange rate `base → quote`, composing via BFS if no
    /// direct rate is stored.
    ///
    /// Returns `ADReal` so that the dependence on intermediate rates is
    /// tracked on the AD tape.
    pub fn get_exchange_rate(&self, base: Currency, quote: Currency) -> Result<ADReal> {
        if base == quote {
            return Ok(ADReal::one());
        }

        // Direct lookup
        if let Some(&rate) = self.exchange_rate_map.get(&(base, quote)) {
            return Ok(rate);
        }

        // BFS triangulation
        let mut queue: VecDeque<(Currency, ADReal)> = VecDeque::new();
        let mut visited: HashSet<Currency> = HashSet::new();
        queue.push_back((base, ADReal::one()));
        visited.insert(base);

        while let Some((current, accumulated)) = queue.pop_front() {
            for (&(src, dst), &map_rate) in &self.exchange_rate_map {
                if src == current && !visited.contains(&dst) {
                    let composed: ADReal = (accumulated * map_rate).into();
                    if dst == quote {
                        return Ok(composed);
                    }
                    visited.insert(dst);
                    queue.push_back((dst, composed));
                } else if dst == current && !visited.contains(&src) {
                    let composed: ADReal = (accumulated / map_rate).into();
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
        let store = ExchangeRateStore::new();
        let rate = store.get_exchange_rate(USD, USD).unwrap();
        assert!((rate.value() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_direct_rate() {
        let mut store = ExchangeRateStore::new();
        store.add_exchange_rate(USD, EUR, ADReal::from(0.85));

        let rate = store.get_exchange_rate(USD, EUR).unwrap();
        assert!((rate.value() - 0.85).abs() < f64::EPSILON);
    }

    #[test]
    fn test_inverse_rate() {
        let mut store = ExchangeRateStore::new();
        store.add_exchange_rate(USD, EUR, ADReal::from(0.85));

        let rate = store.get_exchange_rate(EUR, USD).unwrap();
        assert!((rate.value() - 1.0 / 0.85).abs() < 1e-12);
    }

    #[test]
    fn test_nonexistent_rate() {
        let store = ExchangeRateStore::new();
        assert!(store.get_exchange_rate(USD, EUR).is_err());
    }

    #[test]
    fn test_triangulation() {
        let mut store = ExchangeRateStore::new();
        store.add_exchange_rate(CLP, USD, ADReal::from(800.0));
        store.add_exchange_rate(USD, EUR, ADReal::from(1.1));

        let clp_eur = store.get_exchange_rate(CLP, EUR).unwrap();
        assert!((clp_eur.value() - 1.1 * 800.0).abs() < 1e-6);

        let eur_clp = store.get_exchange_rate(EUR, CLP).unwrap();
        assert!((eur_clp.value() - 1.0 / (1.1 * 800.0)).abs() < 1e-12);
    }

    #[test]
    fn test_pillars_labels() {
        let mut store = ExchangeRateStore::new();
        store.add_exchange_rate(USD, EUR, ADReal::from(0.92));
        store.add_exchange_rate(USD, CLP, ADReal::from(950.0));

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
