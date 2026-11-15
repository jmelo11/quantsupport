//! Prepared LGM path arithmetic with external, reproducible Brownian increments.

use crate::{
    ad::{operation::Operation, scalar::Scalar},
    models::lgm::lgmmarketmodel::sobol_path_normal,
    utils::errors::{QSError, Result},
    xva::visitors::marketmodel::{PathScenario, SimulationResponse},
};

/// Operation indices supplying one simulated market-response slot.
#[derive(Clone, Default)]
pub struct PathResponseLayout {
    /// Discount-factor operation, when requested and supported.
    discounts: Option<usize>,
    /// Forward-rate operation, when requested and supported.
    forward_rates: Option<usize>,
    /// Reporting-currency FX operation, when present.
    fx_rates: Option<usize>,
    /// Spot-observation operation, when present.
    spots: Option<usize>,
    /// Path-dependent observation operation, when present.
    path_dependent_observations: Option<usize>,
    /// Numeraire operation, when present.
    numeraire: Option<usize>,
}

impl PathResponseLayout {
    /// Sets the discount-factor operation, or clears an unrequested response.
    pub const fn set_discounts(&mut self, operation: Option<usize>) {
        self.discounts = operation;
    }

    /// Sets the forward-rate operation, or clears an unrequested response.
    pub const fn set_forward_rates(&mut self, operation: Option<usize>) {
        self.forward_rates = operation;
    }

    /// Sets the reporting-currency FX operation, or clears an unrequested response.
    pub const fn set_fx_rates(&mut self, operation: Option<usize>) {
        self.fx_rates = operation;
    }

    /// Sets the spot-observation operation, or clears an unrequested response.
    pub const fn set_spots(&mut self, operation: Option<usize>) {
        self.spots = operation;
    }

    /// Sets the path-dependent observation operation, or clears an unrequested response.
    pub const fn set_path_dependent_observations(&mut self, operation: Option<usize>) {
        self.path_dependent_observations = operation;
    }

    /// Sets the numeraire operation, or clears an unrequested response.
    pub const fn set_numeraire(&mut self, operation: Option<usize>) {
        self.numeraire = operation;
    }
}

#[derive(Clone)]
enum Instruction {
    Parameter(usize),
    Draw(usize),
    Arithmetic(Operation),
}

/// A numerical path program with setup-time coefficients and external Sobol draws.
///
/// The plan contains no borrowed model and can be replayed using ordinary or
/// recorded scalars. Numerical coefficients retain setup/calibration dependencies
/// until the caller explicitly linearizes them into published risk coordinates.
#[derive(Clone)]
pub struct MarketPathPlan<T: Scalar> {
    parameters: Vec<T>,
    instructions: Vec<Instruction>,
    responses: Vec<Vec<PathResponseLayout>>,
    square_roots: Vec<f64>,
    cholesky: Vec<Vec<f64>>,
    seed: u64,
}

impl<T: Scalar> MarketPathPlan<T> {
    /// Starts a program using the model's time-step square roots and Cholesky factor.
    #[must_use]
    pub const fn new(square_roots: Vec<f64>, cholesky: Vec<Vec<f64>>, seed: u64) -> Self {
        Self {
            parameters: Vec::new(),
            instructions: Vec::new(),
            responses: Vec::new(),
            square_roots,
            cholesky,
            seed,
        }
    }
    /// Registers a setup-time numerical coefficient and returns its operation index.
    pub fn parameter(&mut self, value: T) -> usize {
        let index = self.parameters.len();
        self.parameters.push(value);
        let operation = self.instructions.len();
        self.instructions.push(Instruction::Parameter(index));
        operation
    }
    /// Registers a correlated Brownian increment by its date-major index.
    pub fn draw(&mut self, index: usize) -> usize {
        let operation = self.instructions.len();
        self.instructions.push(Instruction::Draw(index));
        operation
    }
    /// Appends scalar arithmetic, referring only to earlier operation indices.
    ///
    /// # Panics
    /// Panics for a forward reference or an input operation outside the plan's input methods.
    pub fn operation(&mut self, operation: Operation) -> usize {
        assert!(
            !matches!(operation, Operation::Input(_)),
            "use parameter/draw for plan inputs"
        );
        assert!(
            operation
                .operands()
                .iter()
                .all(|&i| i < self.instructions.len()),
            "forward reference in path plan"
        );
        let index = self.instructions.len();
        self.instructions.push(Instruction::Arithmetic(operation));
        index
    }
    /// Appends the response layout for one date in global or compact request order.
    pub fn push_responses(&mut self, responses: Vec<PathResponseLayout>) {
        self.responses.push(responses);
    }
    /// Accesses response-layout metadata before finalizing dated spot observations.
    pub fn responses_mut(&mut self) -> &mut [Vec<PathResponseLayout>] {
        &mut self.responses
    }
    /// Returns the numerical coefficients in their stable program-input order.
    #[must_use]
    pub fn parameters(&self) -> &[T] {
        &self.parameters
    }
    /// Returns the number of externally generated correlated increments.
    #[must_use]
    pub const fn draw_count(&self) -> usize {
        self.square_roots.len() * self.cholesky.len()
    }
    /// Returns the scratch-buffer length needed to generate one step's independent normals.
    #[must_use]
    pub const fn factor_count(&self) -> usize {
        self.cholesky.len()
    }
    /// Maps coefficients while retaining exactly the same numerical path and request layout.
    #[must_use]
    pub fn map_parameters<U: Scalar>(&self, map: impl Fn(T) -> U) -> MarketPathPlan<U> {
        MarketPathPlan {
            parameters: self.parameters.iter().copied().map(map).collect(),
            instructions: self.instructions.clone(),
            responses: self.responses.clone(),
            square_roots: self.square_roots.clone(),
            cholesky: self.cholesky.clone(),
            seed: self.seed,
        }
    }
    /// Generates exactly the current LGM model's date-major correlated Brownian increments.
    ///
    /// # Errors
    /// Rejects mismatched buffers or invalid correlation layouts.
    pub fn fill_draws(&self, path: usize, draws: &mut [f64], scratch: &mut [f64]) -> Result<()> {
        let factors = self.factor_count();
        if draws.len() != self.draw_count() || scratch.len() != factors {
            return Err(QSError::InvalidValueErr(
                "compiled path draw buffer length".into(),
            ));
        }
        for (step, &root) in self.square_roots.iter().enumerate() {
            for (factor, normal) in scratch.iter_mut().enumerate() {
                *normal = sobol_path_normal(path, step * factors + factor, self.seed)?;
            }
            for (factor, row) in self.cholesky.iter().enumerate() {
                if row.len() != factors {
                    return Err(QSError::InvalidValueErr("invalid Cholesky layout".into()));
                }
                draws[step * factors + factor] = row
                    .iter()
                    .zip(scratch.iter())
                    .take(factor + 1)
                    .map(|(loading, normal)| loading * normal)
                    .sum::<f64>()
                    * root;
            }
        }
        Ok(())
    }
    /// Replays the prepared path arithmetic with supplied coefficients and external draws.
    ///
    /// # Errors
    /// Rejects incompatible inputs or unsupported arithmetic instructions.
    pub fn replay<U: Scalar>(&self, parameters: &[U], draws: &[U]) -> Result<PathScenario<U>> {
        if parameters.len() != self.parameters.len() || draws.len() != self.draw_count() {
            return Err(QSError::InvalidValueErr(
                "compiled market plan input layout".into(),
            ));
        }
        let mut values: Vec<U> = Vec::with_capacity(self.instructions.len());
        for instruction in &self.instructions {
            let value = match instruction {
                Instruction::Parameter(i) => parameters[*i],
                Instruction::Draw(i) => *draws
                    .get(*i)
                    .ok_or_else(|| QSError::InvalidValueErr("draw index outside plan".into()))?,
                Instruction::Arithmetic(operation) => match *operation {
                    Operation::Constant(value) => U::scalar(value),
                    Operation::Add(a, b) => values[a].add_val(values[b]),
                    Operation::Sub(a, b) => values[a].sub_val(values[b]),
                    Operation::Mul(a, b) => values[a].mul_val(values[b]),
                    Operation::Div(a, b) => values[a].div_val(values[b]),
                    Operation::Neg(a) => values[a].neg_val(),
                    Operation::Exp(a) => values[a].exp(),
                    Operation::Ln(a) => values[a].ln(),
                    _ => {
                        return Err(QSError::NotImplementedErr(
                            "unsupported market plan arithmetic".into(),
                        ));
                    }
                },
            };
            values.push(value);
        }
        Ok(self
            .responses
            .iter()
            .map(|row| {
                row.iter()
                    .map(|layout| SimulationResponse {
                        discounts: layout.discounts.map(|i| values[i]),
                        forward_rates: layout.forward_rates.map(|i| values[i]),
                        fx_rates: layout.fx_rates.map(|i| values[i]),
                        spots: layout.spots.map(|i| values[i]),
                        path_dependent_observations: layout
                            .path_dependent_observations
                            .map(|i| values[i]),
                        numeraire: layout.numeraire.map(|i| values[i]),
                    })
                    .collect()
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Checks plan checks buffers and reuses the draw layout.
    #[test]
    fn plan_checks_buffers_and_reuses_the_draw_layout() -> Result<()> {
        let mut plan = MarketPathPlan::new(vec![0.5], vec![vec![1.0]], 42);
        let coefficient = plan.parameter(2.0);
        let draw = plan.draw(0);
        let exponent = plan.operation(Operation::Mul(coefficient, draw));
        let value = plan.operation(Operation::Exp(exponent));
        let mut response = PathResponseLayout::default();
        response.set_discounts(Some(value));
        plan.push_responses(vec![response]);
        let mut first = [0.0];
        let mut second = [0.0];
        plan.fill_draws(0, &mut first, &mut [0.0])?;
        plan.fill_draws(1, &mut second, &mut [0.0])?;
        assert_eq!(first[0], -second[0]);
        let scenario = plan.replay(plan.parameters(), &first)?;
        assert_eq!(scenario[0][0].discounts, Some((2.0 * first[0]).exp()));
        let changed = plan.map_parameters(|parameter| parameter * 2.0);
        assert_eq!(
            changed.replay(changed.parameters(), &first)?[0][0].discounts,
            Some((4.0 * first[0]).exp())
        );
        assert!(plan.fill_draws(0, &mut [], &mut [0.0]).is_err());
        assert!(plan.fill_draws(0, &mut first, &mut []).is_err());
        assert!(plan.replay::<f64>(&[], &first).is_err());
        Ok(())
    }
}
