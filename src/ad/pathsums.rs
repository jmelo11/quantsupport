//! Parallel scenario reduction results.

/// Deterministically chunk-reduced (for a fixed Rayon pool size) Monte Carlo
/// value and gradient sums. No normalization is applied to the results.
#[derive(Clone, Debug)]
pub struct PathSums {
    /// Sum of compiled values across all paths.
    value: f64,
    /// Component-wise sum of path gradients, in graph input order.
    gradients: Vec<f64>,
    /// Number of paths included in the sums.
    count: usize,
}

impl PathSums {
    /// Creates empty sums with one gradient slot per active numerical input.
    #[must_use]
    pub fn new(gradient_count: usize) -> Self {
        Self {
            value: 0.0,
            gradients: vec![0.0; gradient_count],
            count: 0,
        }
    }

    /// Returns the unnormalized sum of path values.
    #[must_use]
    pub const fn value(&self) -> f64 {
        self.value
    }

    /// Returns unnormalized gradient sums in graph input order.
    #[must_use]
    pub fn gradients(&self) -> &[f64] {
        &self.gradients
    }

    /// Returns the number of paths included in the sums.
    #[must_use]
    pub const fn count(&self) -> usize {
        self.count
    }

    /// Adds one path's value and gradient without applying estimator normalization.
    ///
    /// # Panics
    /// Panics if the gradient length differs from the configured input count.
    pub fn accumulate(&mut self, value: f64, gradient: &[f64]) {
        assert_eq!(self.gradients.len(), gradient.len(), "path gradient layout");
        self.value += value;
        for (sum, derivative) in self.gradients.iter_mut().zip(gradient) {
            *sum += derivative;
        }
        self.count += 1;
    }

    /// Combines compatible worker sums in the caller's deterministic reduction order.
    ///
    /// # Panics
    /// Panics if the worker sums use different gradient layouts.
    pub fn merge(&mut self, other: &Self) {
        assert_eq!(
            self.gradients.len(),
            other.gradients.len(),
            "worker gradient layout"
        );
        self.value += other.value;
        for (sum, derivative) in self.gradients.iter_mut().zip(&other.gradients) {
            *sum += derivative;
        }
        self.count += other.count;
    }

    /// Returns the arithmetic mean of the path values, or `None` when no paths were accumulated.
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // Path counts are used as floating-point divisors.
    pub fn mean(&self) -> Option<f64> {
        (self.count != 0).then(|| self.value / self.count as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::PathSums;
    /// Checks mean has no value for empty sums.
    #[test]
    fn mean_has_no_value_for_empty_sums() {
        let empty = PathSums::new(0);
        assert_eq!(empty.mean(), None);
        let mut nonempty = PathSums::new(0);
        for _ in 0..3 {
            nonempty.accumulate(3.0, &[]);
        }
        assert_eq!(nonempty.mean(), Some(3.0));
    }
}
