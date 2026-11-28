//! Multi-output native programs with a selected reverse root and active inputs.

use std::sync::Arc;

use crate::utils::errors::{QSError, Result};

use super::{
    comparisonguard::ComparisonGuard, graph::Graph, jit, nativekernel::NativeKernel,
    recorded::Recorded,
};

/// Reusable native program returning observations and one selected risk gradient.
///
/// Only the leading `active_inputs` receive derivatives. Subsequent inputs can
/// hold external random draws, avoiding unnecessary random-draw adjoints.
#[derive(Clone)]
pub struct CompiledProgram {
    kernel: Arc<NativeKernel>,
    inputs: usize,
    outputs: usize,
    active_inputs: usize,
    graph: Arc<Graph>,
    output_operations: Vec<usize>,
    comparison_guards: Vec<ComparisonGuard>,
}

impl CompiledProgram {
    /// Captures output observations and a scalar reverse root, then compiles once.
    ///
    /// The closure returns `(risk_root, observations)`. Observations are written
    /// without automatically adding them to the reverse root.
    ///
    /// # Errors
    /// Propagates capture, pricing, layout and native compilation errors.
    /// Closure errors must convert into [`QSError`]; their category and cause
    /// are preserved.
    ///
    /// # Panics
    /// Has the capture-lifetime requirements of [`Graph::try_trace`].
    pub fn trace<F, E>(samples: &[f64], active_inputs: usize, function: F) -> Result<Self>
    where
        F: FnOnce(&[Recorded]) -> std::result::Result<(Recorded, Vec<Recorded>), E>,
        E: Into<QSError>,
    {
        Self::trace_reusing(samples, active_inputs, None, function)
    }

    /// Captures current arithmetic, reusing code only for an identical program and layout.
    ///
    /// Runtime inputs may change freely. Constants, output ordering and the
    /// active input prefix must match exactly; otherwise code is recompiled.
    ///
    /// # Errors
    /// Propagates capture, pricing, layout and compilation errors.
    /// Closure errors must convert into [`QSError`]; their category and cause
    /// are preserved.
    ///
    /// # Panics
    /// Has the capture-lifetime requirements of [`Graph::try_trace`].
    pub fn trace_reusing<F, E>(
        samples: &[f64],
        active_inputs: usize,
        previous: Option<&Self>,
        function: F,
    ) -> Result<Self>
    where
        F: FnOnce(&[Recorded]) -> std::result::Result<(Recorded, Vec<Recorded>), E>,
        E: Into<QSError>,
    {
        let mut outputs = Vec::new();
        let mut comparison_guards = Vec::new();
        let graph = Graph::try_trace(samples, |x| {
            function(x).map(|(root, observations)| {
                outputs = observations
                    .into_iter()
                    .map(Recorded::operation_index)
                    .collect();
                for guard in Recorded::take_comparison_guards() {
                    outputs.extend(guard.operands());
                    comparison_guards.push(guard);
                }
                root
            })
        })?;
        if let Some(previous) = previous {
            if previous.active_inputs == active_inputs
                && previous.inputs == samples.len()
                && previous.output_operations == outputs
                && previous.comparison_guards == comparison_guards
                && previous.graph.output() == graph.output()
                && previous.graph.operation_count() == graph.operation_count()
                && previous
                    .graph
                    .operations()
                    .iter()
                    .zip(graph.operations())
                    .all(|(a, b)| a.same_instruction(b))
            {
                return Ok(previous.clone());
            }
        }
        let kernel = jit::compile_outputs(&graph, &outputs, active_inputs)?;
        Ok(Self {
            kernel: Arc::new(kernel),
            inputs: samples.len(),
            outputs: outputs.len(),
            active_inputs,
            graph: Arc::new(graph),
            output_operations: outputs,
            comparison_guards,
        })
    }

    /// Returns the total input count, including externally generated random draws.
    #[must_use]
    pub const fn input_count(&self) -> usize {
        self.inputs
    }
    /// Returns the observation count, including operands retained for comparison guards.
    #[must_use]
    pub const fn output_count(&self) -> usize {
        self.outputs
    }
    /// Returns the number of active input derivatives produced by each call.
    #[must_use]
    pub const fn gradient_count(&self) -> usize {
        self.active_inputs
    }

    /// Returns whether both programs share the same executable code allocation.
    #[must_use]
    pub fn shares_code_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.kernel, &other.kernel)
    }
    /// Evaluates outputs and the selected risk gradient without allocating.
    ///
    /// # Errors
    /// Rejects mismatched input, output and gradient buffers.
    pub fn evaluate(
        &self,
        inputs: &[f64],
        outputs: &mut [f64],
        gradient: &mut [f64],
    ) -> Result<()> {
        self.kernel.values_and_grad(inputs, outputs, gradient)?;
        let start = self.outputs - 2 * self.comparison_guards.len();
        for (operands, guard) in outputs[start..]
            .as_chunks::<2>()
            .0
            .iter()
            .zip(&self.comparison_guards)
        {
            if !guard.matches(operands[0], operands[1]) {
                return Err(QSError::ComparisonChanged);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::errors::{QSError, Result};

    use super::CompiledProgram;

    /// Checks market values change without recompiling but program changes do recompile.
    #[test]
    fn market_values_change_without_recompiling_but_program_changes_do_recompile() -> Result<()> {
        let first = CompiledProgram::trace(&[2.0], 1, |x| {
            Ok::<_, QSError>((x[0] * x[0], vec![x[0] * x[0]]))
        })?;
        let rebound = CompiledProgram::trace_reusing(&[3.0], 1, Some(&first), |x| {
            Ok::<_, QSError>((x[0] * x[0], vec![x[0] * x[0]]))
        })?;
        assert!(rebound.shares_code_with(&first));
        let mut values = [0.0];
        let mut gradient = [0.0];
        rebound.evaluate(&[3.0], &mut values, &mut gradient)?;
        assert_eq!(values, [9.0]);
        assert_eq!(gradient, [6.0]);
        let changed = CompiledProgram::trace_reusing(&[3.0], 1, Some(&first), |x| {
            Ok::<_, QSError>((x[0] * 2.0, vec![x[0] * 2.0]))
        })?;
        assert!(!changed.shares_code_with(&first));
        Ok(())
    }

    /// Checks comparison traits guard every native execution.
    #[test]
    fn comparison_traits_guard_every_native_execution() -> Result<()> {
        let program = CompiledProgram::trace(&[1.0], 1, |x| {
            if !(x[0] > 0.0 && x[0] <= f64::MAX) {
                return Err(QSError::InvalidValueErr(
                    "discount must be finite and positive".into(),
                ));
            }
            Ok((x[0], vec![x[0]]))
        })?;
        let mut values = vec![0.0; program.output_count()];
        for input in [f64::MIN_POSITIVE, 2.0, f64::MAX] {
            program.evaluate(&[input], &mut values, &mut [0.0])?;
        }
        for input in [0.0, -1.0, f64::INFINITY, f64::NAN] {
            assert!(program.evaluate(&[input], &mut values, &mut [0.0]).is_err());
        }
        Ok(())
    }

    /// Checks comparison guards allow changed ordering with the same boolean result.
    #[test]
    fn comparison_guards_allow_changed_ordering_with_the_same_boolean_result() -> Result<()> {
        let program = CompiledProgram::trace(&[0.0], 1, |x| {
            Ok::<_, QSError>((if x[0] <= 0.0 { x[0] * 2.0 } else { x[0] * 3.0 }, vec![]))
        })?;
        let mut values = vec![0.0; program.output_count()];
        let mut gradient = [0.0];
        program.evaluate(&[-2.0], &mut values, &mut gradient)?;
        assert_eq!(gradient, [2.0]);
        for input in [1.0, f64::NAN] {
            assert!(matches!(
                program.evaluate(&[input], &mut values, &mut gradient),
                Err(QSError::ComparisonChanged)
            ));
        }
        Ok(())
    }

    /// Checks observations are separate from risk and noise is inactive.
    #[test]
    fn observations_are_separate_from_risk_and_noise_is_inactive() -> Result<()> {
        let program = CompiledProgram::trace(&[2.0, 3.0, 4.0], 2, |x| {
            Ok::<_, QSError>((x[0] * x[1] + x[2] * x[0], vec![x[0] * x[0], x[1] * x[2]]))
        })?;
        let mut output = [0.0; 2];
        let mut gradient = [0.0; 2];
        program.evaluate(&[2.0, 3.0, 4.0], &mut output, &mut gradient)?;
        assert_eq!(output, [4.0, 12.0]);
        assert_eq!(gradient, [7.0, 2.0]);
        assert!(program
            .evaluate(&[2.0, 3.0, 4.0], &mut [0.0], &mut gradient)
            .is_err());
        assert!(program
            .evaluate(&[2.0, 3.0, 4.0], &mut output, &mut [0.0; 3])
            .is_err());
        assert!(
            CompiledProgram::trace(&[1.0], 2, |x| Ok::<_, QSError>((x[0], vec![x[0]]))).is_err()
        );
        Ok(())
    }
}
