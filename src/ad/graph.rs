//! Validated numerical programs and exact directional differentiation.

use std::f64::consts::{FRAC_1_SQRT_2, FRAC_2_SQRT_PI};

use crate::{
    ad::scalar::Scalar,
    math::probability::normcdf::NormCDF,
    utils::errors::{QSError, Result},
};

use super::{
    compiledkernel::CompiledKernel, derivativeorder::DerivativeOrder, operation::Operation,
    recorded::Recorded,
};

/// Immutable numerical program with a fixed input layout and one scalar output.
#[derive(Clone, Debug)]
pub struct Graph {
    operations: Vec<Operation>,
    inputs: usize,
    output: usize,
}

impl Graph {
    /// Validates a topologically ordered program with its inputs at the beginning.
    ///
    /// # Errors
    /// Rejects missing inputs, invalid outputs and forward operation references.
    pub fn new(operations: Vec<Operation>, inputs: usize, output: usize) -> Result<Self> {
        if inputs > operations.len() || output >= operations.len() {
            return Err(QSError::InvalidGraph(
                "input or output index is out of range".into(),
            ));
        }
        for (id, operation) in operations.iter().enumerate() {
            if id < inputs && !matches!(operation, Operation::Input(index) if *index == id) {
                return Err(QSError::InvalidGraph(
                    "inputs must form an ordered prefix".into(),
                ));
            }
            if id >= inputs && matches!(operation, Operation::Input(_)) {
                return Err(QSError::InvalidGraph(
                    "input outside the input prefix".into(),
                ));
            }
            if operation.operands().iter().any(|&operand| operand >= id) {
                return Err(QSError::InvalidGraph(
                    "operand must precede its operation".into(),
                ));
            }
        }
        Ok(Self {
            operations,
            inputs,
            output,
        })
    }

    /// Captures arithmetic once; sample input values remain runtime inputs.
    ///
    /// # Errors
    /// Rejects nested captures, foreign values and extraction of recorded values.
    ///
    /// # Panics
    /// Panics if the closure mixes scalars from different captures or uses a
    /// recorded scalar on another thread.
    pub fn trace<F>(samples: &[f64], function: F) -> Result<Self>
    where
        F: FnOnce(&[Recorded]) -> Recorded,
    {
        Recorded::capture(samples, function)
    }

    /// Captures a fallible numerical pricing function without unwrapping its result.
    ///
    /// # Errors
    /// Propagates capture errors and preserves the pricing error's category and
    /// cause with context identifying the numerical capture. Closure errors must
    /// convert into [`QSError`].
    ///
    /// # Panics
    /// Has the same capture-lifetime requirements as [`Self::trace`].
    pub fn try_trace<F, E>(samples: &[f64], function: F) -> Result<Self>
    where
        F: FnOnce(&[Recorded]) -> std::result::Result<Recorded, E>,
        E: Into<QSError>,
    {
        let mut failure = None;
        let graph = Self::trace(samples, |x| match function(x) {
            Ok(value) => value,
            Err(error) => {
                failure = Some(error.into());
                // Finish capture cleanup before returning the original failure below.
                Recorded::from(0.0)
            }
        });
        failure.map_or(graph, |error: QSError| {
            Err(error.context("capturing numerical function"))
        })
    }

    /// Returns the number of runtime inputs and corresponding gradient entries.
    #[must_use]
    pub const fn input_count(&self) -> usize {
        self.inputs
    }

    /// Returns the number of instructions, including inputs and constants.
    #[must_use]
    pub const fn operation_count(&self) -> usize {
        self.operations.len()
    }

    /// Returns the instructions in dependency order.
    #[must_use]
    pub fn operations(&self) -> &[Operation] {
        &self.operations
    }

    /// Returns the index of the instruction that supplies the scalar output.
    #[must_use]
    pub const fn output(&self) -> usize {
        self.output
    }

    /// Removes operations unused by the reverse root or requested observations.
    ///
    /// Input slots remain unchanged. This also removes unused host-math calls
    /// whose side effects a native optimizer cannot otherwise rule out.
    ///
    /// # Errors
    /// Rejects observations outside the validated graph.
    pub fn prune(&self, observations: &[usize]) -> Result<(Self, Vec<usize>)> {
        if observations.iter().any(|&id| id >= self.operations.len()) {
            return Err(QSError::InvalidGraph(
                "observation index is out of range".into(),
            ));
        }
        let mut keep = vec![false; self.operations.len()];
        let mut pending = observations.to_vec();
        pending.push(self.output);
        while let Some(id) = pending.pop() {
            if keep[id] {
                continue;
            }
            keep[id] = true;
            pending.extend(self.operations[id].operands());
        }
        keep[..self.inputs].fill(true);
        let mut mapping = vec![0; self.operations.len()];
        let mut operations = Vec::new();
        for (id, operation) in self.operations.iter().enumerate() {
            if keep[id] {
                mapping[id] = operations.len();
                operations.push(operation.remap(|index| mapping[index]));
            }
        }
        let graph = Self::new(operations, self.inputs, mapping[self.output])?;
        Ok((graph, observations.iter().map(|&id| mapping[id]).collect()))
    }

    /// Compiles a first-order kernel without generating second-order work.
    ///
    /// # Errors
    /// Returns a native compilation or layout error.
    pub fn compile(&self) -> Result<CompiledKernel> {
        self.compile_with_order(DerivativeOrder::First)
    }

    /// Compiles only the requested derivative order.
    ///
    /// # Errors
    /// Returns a symbolic-layout or native compilation error.
    pub fn compile_with_order(&self, order: DerivativeOrder) -> Result<CompiledKernel> {
        CompiledKernel::compile(self, order)
    }

    /// Builds `gradient(f)(x) dot direction` as a replayable scalar program.
    ///
    /// Its inputs are `[x, direction]`. Differentiating its first input block
    /// gives the exact Hessian-vector product; its second gives the gradient.
    /// This is symbolic differentiation, with no finite-difference step size.
    ///
    /// # Errors
    /// Returns an error if the doubled input layout overflows or capture fails.
    pub fn directional_derivative(&self) -> Result<Self> {
        let inputs = self.inputs.checked_mul(2).ok_or(QSError::OffsetOverflow)?;
        Self::trace(&vec![1.0; inputs], |x| {
            let values = self.replay_recorded(&x[..self.inputs]);
            let gradients = self.reverse_recorded(&values);
            gradients
                .into_iter()
                .zip(&x[self.inputs..])
                .fold(Recorded::from(0.0), |sum, (gradient, direction)| {
                    sum + gradient * *direction
                })
        })
    }

    /// Recreates the primal expression symbolically in a new compiler capture.
    fn replay_recorded(&self, inputs: &[Recorded]) -> Vec<Recorded> {
        let mut values: Vec<Recorded> = Vec::with_capacity(self.operations.len());
        for operation in &self.operations {
            let value = match *operation {
                Operation::Input(i) => inputs[i],
                Operation::Constant(v) => Recorded::from(v),
                Operation::Add(a, b) => values[a] + values[b],
                Operation::Sub(a, b) => values[a] - values[b],
                Operation::Mul(a, b) => values[a] * values[b],
                Operation::Div(a, b) => values[a] / values[b],
                Operation::Neg(a) => -values[a],
                Operation::Exp(a) => values[a].exp(),
                Operation::Ln(a) => values[a].ln(),
                Operation::Sqrt(a) => values[a].sqrt(),
                Operation::Sin(a) => values[a].sin(),
                Operation::Cos(a) => values[a].cos(),
                Operation::Abs(a) => values[a].abs(),
                Operation::Pow(a, b) => values[a].pows(values[b]),
                Operation::Max(a, b) => values[a].max_val(values[b]),
                Operation::Min(a, b) => values[a].min_val(values[b]),
                Operation::NormalCdf(a) => values[a].norm_cdf(),
                Operation::SelectGreater(a, b, yes, no) => {
                    values[a].select_greater(values[b], values[yes], values[no])
                }
            };
            values.push(value);
        }
        values
    }

    /// Builds symbolic first-gradient expressions for compiling directional second derivatives.
    #[allow(clippy::too_many_lines)] // The derivative rules follow the operation enum.
    fn reverse_recorded(&self, values: &[Recorded]) -> Vec<Recorded> {
        let zero = Recorded::from(0.0);
        let one = Recorded::from(1.0);
        let mut active = vec![false; values.len()];
        for (id, operation) in self.operations.iter().enumerate() {
            active[id] = match *operation {
                Operation::Input(_) => true,
                Operation::Constant(_) => false,
                Operation::SelectGreater(_, _, yes, no) => active[yes] || active[no],
                _ => operation.operands().iter().any(|&i| active[i]),
            };
        }
        let mut adjoints = vec![None; values.len()];
        adjoints[self.output] = Some(one);
        for (id, operation) in self.operations.iter().enumerate().rev() {
            let Some(seed) = adjoints[id] else { continue };
            let mut add = |target: usize, contribution: Recorded| {
                if active[target] {
                    adjoints[target] = Some(
                        adjoints[target].map_or(contribution, |previous| previous + contribution),
                    );
                }
            };
            match *operation {
                Operation::Input(_) | Operation::Constant(_) => {}
                Operation::Add(a, b) => {
                    add(a, seed);
                    add(b, seed);
                }
                Operation::Sub(a, b) => {
                    add(a, seed);
                    add(b, -seed);
                }
                Operation::Mul(a, b) => {
                    add(a, seed * values[b]);
                    add(b, seed * values[a]);
                }
                Operation::Div(a, b) => {
                    add(a, seed / values[b]);
                    add(b, -seed * values[a] / (values[b] * values[b]));
                }
                Operation::Neg(a) => add(a, -seed),
                Operation::Exp(a) => add(a, seed * values[id]),
                Operation::Ln(a) => add(a, seed / values[a]),
                Operation::Sqrt(a) => add(a, seed / (values[id] * 2.0)),
                Operation::Sin(a) => add(a, seed * values[a].cos()),
                Operation::Cos(a) => add(a, -seed * values[a].sin()),
                Operation::Abs(a) => add(
                    a,
                    values[a].select_greater(
                        zero,
                        seed,
                        zero.select_greater(values[a], -seed, zero),
                    ),
                ),
                Operation::Pow(a, b) => {
                    if active[a] {
                        add(a, seed * values[b] * values[a].pows(values[b] - one));
                    }
                    if active[b] {
                        add(b, seed * values[id] * values[a].ln());
                    }
                }
                Operation::Max(a, b) => {
                    add(a, values[a].select_greater(values[b], seed, zero));
                    add(b, values[b].select_greater(values[a], seed, zero));
                }
                Operation::Min(a, b) => {
                    add(a, values[b].select_greater(values[a], seed, zero));
                    add(b, values[a].select_greater(values[b], seed, zero));
                }
                Operation::NormalCdf(a) => add(
                    a,
                    seed * (-(values[a] * values[a]) * 0.5).exp()
                        * FRAC_1_SQRT_2
                        * FRAC_2_SQRT_PI
                        * 0.5,
                ),
                Operation::SelectGreater(a, b, yes, no) => {
                    add(yes, values[a].select_greater(values[b], seed, zero));
                    add(no, values[a].select_greater(values[b], zero, seed));
                }
            }
        }
        adjoints
            .into_iter()
            .take(self.inputs)
            .map(|derivative| derivative.unwrap_or(zero))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::{error::Error, io};

    use crate::{
        ad::{operation::Operation, CompiledFunction, CompiledProgram, Recorded},
        utils::errors::{QSError, Result},
    };

    use super::Graph;
    /// Checks pruning keeps requested observations and input layout.
    #[test]
    fn pruning_keeps_requested_observations_and_input_layout() -> Result<()> {
        let graph = Graph::new(
            vec![
                Operation::Input(0),
                Operation::Input(1),
                Operation::Exp(0),
                Operation::Sin(1),
                Operation::Mul(0, 1),
            ],
            2,
            4,
        )?;
        let (pruned, outputs) = graph.prune(&[3])?;
        assert_eq!(pruned.input_count(), 2);
        assert_eq!(pruned.operation_count(), 4);
        assert_eq!(outputs, [2]);
        assert_eq!(pruned.output(), 3);
        Ok(())
    }
    /// Checks invalid graphs are rejected before native compilation.
    #[test]
    fn invalid_graphs_are_rejected_before_native_compilation() {
        assert!(Graph::new(vec![Operation::Input(0), Operation::Add(0, 2)], 1, 1).is_err());
        assert!(Graph::new(vec![Operation::Constant(1.0)], 1, 0).is_err());
        assert!(Graph::new(vec![], 0, 0).is_err());
    }

    /// Checks fallible capture restores state after a pricing error.
    #[test]
    fn fallible_capture_restores_state_after_a_pricing_error() -> Result<()> {
        let error = Graph::try_trace(&[1.0], |_| {
            Err::<Recorded, _>(QSError::InvalidValueErr("invalid contract".into()))
        })
        .unwrap_err();
        assert!(
            matches!(error.root_cause(), QSError::InvalidValueErr(message) if message == "invalid contract")
        );
        assert_eq!(error.contexts(), ["capturing numerical function"]);
        let graph = Graph::try_trace(&[2.0], |x| Ok::<_, QSError>(x[0] * x[0]))?;
        assert_eq!(graph.input_count(), 1);
        assert_eq!(graph.compile()?.value_and_grad(&[3.0], &mut [0.0])?, 9.0);
        Ok(())
    }

    #[test]
    fn fallible_captures_preserve_external_causes() {
        let failures = [
            Graph::try_trace(&[1.0], |_| {
                Err::<Recorded, _>(io::Error::new(io::ErrorKind::NotFound, "discount curve"))
            })
            .map(|_| ()),
            CompiledFunction::try_trace(&[1.0], |_| {
                Err::<Vec<Recorded>, _>(io::Error::new(io::ErrorKind::NotFound, "discount curve"))
            })
            .map(|_| ()),
            CompiledProgram::trace(&[1.0], 1, |_| {
                Err::<(Recorded, Vec<Recorded>), _>(io::Error::new(
                    io::ErrorKind::NotFound,
                    "discount curve",
                ))
            })
            .map(|_| ()),
        ];
        for failure in failures {
            let error = failure.unwrap_err();
            assert_eq!(error.code(), "io");
            assert_eq!(error.contexts(), ["capturing numerical function"]);
            assert!(
                matches!(error.root_cause(), QSError::IoErr(source) if source.kind() == io::ErrorKind::NotFound)
            );
            assert!(error
                .root_cause()
                .source()
                .is_some_and(|source| source.is::<io::Error>()));
        }
    }
}
