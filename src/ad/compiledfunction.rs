//! Compiled vector functions and AD-generated Jacobians for solver residuals.

use super::{
    compiledkernel::CompiledKernel, compileerror::CompileError, derivativeorder::DerivativeOrder,
    graph::Graph, jit, nativekernel::NativeKernel, recorded::Recorded,
};
use crate::{
    math::solvers::solvertraits::{ContFunc, JacobianFunc, VectorFunc},
    utils::errors::{QSError, Result as QSResult},
};
use std::{convert::Infallible, fmt::Display, sync::Arc};

/// A reusable vector-valued function with automatically generated derivatives.
///
/// Values and Jacobian rows use the same captured residual definitions. Value
/// evaluation performs no derivative work. Jacobian rows share one native
/// forward calculation; second-order kernels are generated only on request.
/// Buffers use output-major order: `jacobian[row * inputs + column]`, and
/// `hessians[(output * inputs + row) * inputs + column]`.
#[derive(Clone)]
pub struct CompiledFunction {
    values: Arc<NativeKernel>,
    jacobian: Arc<NativeKernel>,
    rows: Vec<CompiledKernel>,
    inputs: usize,
    outputs: usize,
    order: DerivativeOrder,
}

impl CompiledFunction {
    /// Captures a vector function and generates its first-order native kernels.
    ///
    /// # Errors
    /// Rejects empty outputs and propagates capture and compilation errors.
    ///
    /// # Panics
    /// Has the scalar-lifetime requirements of [`Graph::trace`].
    pub fn trace<F>(samples: &[f64], function: F) -> Result<Self, CompileError>
    where
        F: FnOnce(&[Recorded]) -> Vec<Recorded>,
    {
        Self::try_trace(samples, |inputs| Ok::<_, Infallible>(function(inputs)))
    }

    /// Captures a fallible vector function with first-order derivatives.
    ///
    /// # Errors
    /// Propagates residual, capture, shape and native compilation errors.
    ///
    /// # Panics
    /// Has the scalar-lifetime requirements of [`Graph::try_trace`].
    pub fn try_trace<F, E>(samples: &[f64], function: F) -> Result<Self, CompileError>
    where
        F: FnOnce(&[Recorded]) -> Result<Vec<Recorded>, E>,
        E: Display,
    {
        Self::try_trace_with_order(samples, DerivativeOrder::First, function)
    }

    /// Captures a residual definition, compiling only the requested derivative order.
    ///
    /// # Errors
    /// Propagates residual, capture, shape and native compilation errors.
    ///
    /// # Panics
    /// Has the scalar-lifetime requirements of [`Graph::try_trace`].
    pub fn try_trace_with_order<F, E>(
        samples: &[f64],
        order: DerivativeOrder,
        function: F,
    ) -> Result<Self, CompileError>
    where
        F: FnOnce(&[Recorded]) -> Result<Vec<Recorded>, E>,
        E: Display,
    {
        let mut outputs = Vec::new();
        let graph = Graph::try_trace(samples, |inputs| {
            function(inputs).map(|values| {
                outputs = values.iter().map(|value| value.operation_index()).collect();
                values
                    .first()
                    .copied()
                    .unwrap_or_else(|| Recorded::from(0.0))
            })
        })?;
        if outputs.is_empty() {
            return Err(CompileError::InvalidGraph(
                "a vector function needs at least one output".into(),
            ));
        }
        let values = Arc::new(jit::compile_outputs(&graph, &outputs, 0)?);
        let jacobian = Arc::new(jit::compile_jacobian(&graph, &outputs)?);
        let rows = if order == DerivativeOrder::Second {
            outputs
                .iter()
                .copied()
                .map(|output| {
                    Graph::new(graph.operations().to_vec(), samples.len(), output)?
                        .compile_with_order(order)
                })
                .collect::<Result<_, CompileError>>()?
        } else {
            Vec::new()
        };
        Ok(Self {
            values,
            jacobian,
            rows,
            inputs: samples.len(),
            outputs: outputs.len(),
            order,
        })
    }

    /// Returns the number of independent runtime inputs.
    #[must_use]
    pub const fn input_count(&self) -> usize {
        self.inputs
    }

    /// Returns the number of residual or observation outputs.
    #[must_use]
    pub const fn output_count(&self) -> usize {
        self.outputs
    }

    /// Returns the highest derivative order prepared for this function.
    #[must_use]
    pub const fn derivative_order(&self) -> DerivativeOrder {
        self.order
    }

    /// Checks the value, Jacobian and optional Hessian buffer dimensions.
    fn validate(
        &self,
        inputs: &[f64],
        outputs: &[f64],
        jacobian: Option<&[f64]>,
    ) -> Result<(), CompileError> {
        if inputs.len() != self.inputs {
            return Err(CompileError::InputCount {
                expected: self.inputs,
                actual: inputs.len(),
            });
        }
        if outputs.len() != self.output_count() {
            return Err(CompileError::BufferCount {
                buffer: "output",
                expected: self.output_count(),
                actual: outputs.len(),
            });
        }
        if let Some(jacobian) = jacobian {
            let expected = self
                .output_count()
                .checked_mul(self.inputs)
                .ok_or(CompileError::OffsetOverflow)?;
            if jacobian.len() != expected {
                return Err(CompileError::BufferCount {
                    buffer: "Jacobian",
                    expected,
                    actual: jacobian.len(),
                });
            }
        }
        Ok(())
    }

    /// Evaluates all values without recording, allocating or calculating derivatives.
    ///
    /// # Errors
    /// Rejects mismatched input or output buffers before native execution.
    pub fn values(&self, inputs: &[f64], outputs: &mut [f64]) -> Result<(), CompileError> {
        self.values.values_and_grad(inputs, outputs, &mut [])
    }

    /// Evaluates values and their output-major Jacobian without execution allocations.
    ///
    /// # Errors
    /// Rejects mismatched buffers before native execution.
    pub fn values_and_jacobian(
        &self,
        inputs: &[f64],
        outputs: &mut [f64],
        jacobian: &mut [f64],
    ) -> Result<(), CompileError> {
        self.validate(inputs, outputs, Some(jacobian))?;
        self.jacobian.values_and_grad(inputs, outputs, jacobian)
    }

    /// Evaluates values, Jacobian and one dense Hessian per output.
    ///
    /// Unlike first-order execution, this allocates temporary directional
    /// buffers inside each second-order scalar kernel.
    ///
    /// # Errors
    /// Requires second-order preparation and exactly shaped buffers.
    pub fn values_jacobian_hessians(
        &self,
        inputs: &[f64],
        outputs: &mut [f64],
        jacobian: &mut [f64],
        hessians: &mut [f64],
    ) -> Result<(), CompileError> {
        self.validate(inputs, outputs, Some(jacobian))?;
        if self.derivative_order() != DerivativeOrder::Second {
            return Err(CompileError::SecondOrderUnavailable);
        }
        let square = self
            .inputs
            .checked_mul(self.inputs)
            .ok_or(CompileError::OffsetOverflow)?;
        let expected = self
            .output_count()
            .checked_mul(square)
            .ok_or(CompileError::OffsetOverflow)?;
        if hessians.len() != expected {
            return Err(CompileError::BufferCount {
                buffer: "output Hessians",
                expected,
                actual: hessians.len(),
            });
        }
        for (row, kernel) in self.rows.iter().enumerate() {
            outputs[row] = kernel.value_grad_hessian(
                inputs,
                &mut jacobian[row * self.inputs..(row + 1) * self.inputs],
                &mut hessians[row * square..(row + 1) * square],
            )?;
        }
        Ok(())
    }
}

impl ContFunc<[f64], Vec<f64>> for CompiledFunction {
    /// Evaluates compiled residuals through the existing continuous-function solver trait.
    fn call(&self, inputs: &[f64]) -> QSResult<Vec<f64>> {
        let mut values = vec![0.0; self.output_count()];
        self.values(inputs, &mut values)
            .map_err(|error| QSError::SolverErr(format!("AD residual: {error}")))?;
        Ok(values)
    }
}

impl VectorFunc<f64, f64> for CompiledFunction {}

impl JacobianFunc<f64, f64, f64> for CompiledFunction {
    /// Returns compiled residual partials through the existing Newton-solver Jacobian trait.
    fn jacobian(&self, inputs: &[f64]) -> QSResult<Vec<Vec<f64>>> {
        let mut values = vec![0.0; self.output_count()];
        let mut jacobian = vec![0.0; self.output_count() * self.inputs];
        self.values_and_jacobian(inputs, &mut values, &mut jacobian)
            .map_err(|error| QSError::SolverErr(format!("AD residual Jacobian: {error}")))?;
        if self.inputs == 0 {
            return Ok(vec![vec![]; self.output_count()]);
        }
        Ok(jacobian
            .chunks_exact(self.inputs)
            .map(<[f64]>::to_vec)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::solvers::vectornewton::VectorNewton;
    use std::f64::consts::E;

    /// Checks nonlinear residual jacobians and hessians are generated from one definition.
    #[test]
    fn nonlinear_residual_jacobians_and_hessians_are_generated_from_one_definition(
    ) -> Result<(), CompileError> {
        let function =
            CompiledFunction::try_trace_with_order(&[2.0, 3.0], DerivativeOrder::Second, |x| {
                Ok::<_, &str>(vec![x[0] * x[0] + x[0] * x[1], x[1].exp()])
            })?;
        let mut values = [0.0; 2];
        let mut jacobian = [0.0; 4];
        let mut hessians = [0.0; 8];
        function.values_jacobian_hessians(
            &[2.0, 3.0],
            &mut values,
            &mut jacobian,
            &mut hessians,
        )?;
        assert_eq!(values, [10.0, 3.0_f64.exp()]);
        assert_eq!(jacobian, [7.0, 2.0, 0.0, 3.0_f64.exp()]);
        assert_eq!(hessians, [2.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 3.0_f64.exp()]);
        function.values_and_jacobian(&[4.0, 1.0], &mut values, &mut jacobian)?;
        assert_eq!(values, [20.0, E]);
        assert_eq!(jacobian, [9.0, 4.0, 0.0, E]);
        Ok(())
    }

    /// Checks shared outputs keep independent jacobian rows and inactive columns.
    #[test]
    fn shared_outputs_keep_independent_jacobian_rows_and_inactive_columns(
    ) -> Result<(), CompileError> {
        let function = CompiledFunction::trace(&[2.0, 3.0, 7.0], |x| {
            let common = x[0] * x[1];
            vec![common + 5.0, common * common, common, 4.0.into()]
        })?;
        let mut values = [0.0; 4];
        let mut jacobian = [0.0; 12];
        function.values_and_jacobian(&[2.0, 3.0, 9.0], &mut values, &mut jacobian)?;
        assert_eq!(values, [11.0, 36.0, 6.0, 4.0]);
        assert_eq!(
            jacobian,
            [3.0, 2.0, 0.0, 36.0, 24.0, 0.0, 3.0, 2.0, 0.0, 0.0, 0.0, 0.0]
        );
        let mut plain = [0.0; 4];
        function.values(&[2.0, 3.0, 9.0], &mut plain)?;
        assert_eq!(plain, values);
        Ok(())
    }

    /// Checks existing newton solver uses ad without a handwritten jacobian.
    #[test]
    fn existing_newton_solver_uses_ad_without_a_handwritten_jacobian() -> QSResult<()> {
        let function =
            CompiledFunction::trace(&[1.0, 1.0], |x| vec![x[0] * x[0] - 4.0, x[1] * x[1] - 9.0])
                .map_err(|error| QSError::SolverErr(error.to_string()))?;
        let solution = VectorNewton::new(1e-12, 30).solve(&function, &[1.0, 1.0])?;
        assert!((solution.x[0] - 2.0).abs() < 1e-10);
        assert!((solution.x[1] - 3.0).abs() < 1e-10);
        Ok(())
    }

    /// Checks rejection of invalid shapes and unprepared second order.
    #[test]
    fn rejects_invalid_shapes_and_unprepared_second_order() -> Result<(), CompileError> {
        let function = CompiledFunction::trace(&[1.0], |x| vec![x[0].exp(), x[0] * x[0]])?;
        assert_eq!(function.derivative_order(), DerivativeOrder::First);
        assert!(function
            .values_and_jacobian(&[1.0], &mut [0.0; 2], &mut [0.0; 1])
            .is_err());
        assert!(function
            .values_jacobian_hessians(&[1.0], &mut [0.0; 2], &mut [0.0; 2], &mut [0.0; 2])
            .is_err());
        assert!(CompiledFunction::trace(&[], |_| vec![]).is_err());
        let constant = CompiledFunction::trace(&[], |_| vec![Recorded::from(3.0)])?;
        let mut value = [0.0];
        constant.values_and_jacobian(&[], &mut value, &mut [])?;
        assert_eq!(value, [3.0]);
        Ok(())
    }
}
