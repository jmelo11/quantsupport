//! Allocation-free first-order evaluation and opt-in second-order execution.

use super::derivativeorder::DerivativeOrder;
use super::{
    compileerror::CompileError, graph::Graph, jit, nativekernel::NativeKernel, pathsums::PathSums,
};
use rayon::prelude::*;
use std::sync::Arc;

/// Reusable thread-safe compiled value-and-gradient kernel.
///
/// The kernel owns its executable memory. Each invocation supplies its own
/// input and gradient buffers, while the generated function holds intermediate
/// values and derivatives in registers or private stack storage.
#[derive(Clone)]
pub struct CompiledKernel {
    owner: Arc<NativeKernel>,
    input_count: usize,
    second: Option<Arc<NativeKernel>>,
}

impl CompiledKernel {
    /// Compiles a value/gradient kernel and, only when selected, its second-order kernel.
    ///
    /// # Errors
    /// Returns a symbolic graph, buffer-layout or native compilation error.
    pub fn compile(graph: &Graph, order: DerivativeOrder) -> Result<Self, CompileError> {
        let owner = Arc::new(jit::compile(graph)?);
        let second = match order {
            DerivativeOrder::First => None,
            DerivativeOrder::Second => {
                Some(Arc::new(jit::compile(&graph.directional_derivative()?)?))
            }
        };
        Ok(Self {
            owner,
            input_count: graph.input_count(),
            second,
        })
    }

    /// Returns the highest derivative order available without tracing or compiling again.
    #[must_use]
    pub const fn derivative_order(&self) -> DerivativeOrder {
        if self.second.is_some() {
            DerivativeOrder::Second
        } else {
            DerivativeOrder::First
        }
    }

    /// Returns the number of inputs and corresponding gradient entries.
    #[must_use]
    pub const fn input_count(&self) -> usize {
        self.input_count
    }

    /// Executes the compiled forward/reverse kernel without allocating or
    /// recording any AD nodes.
    ///
    /// # Errors
    /// Returns an error when buffer lengths do not match the compiled layout.
    pub fn value_and_grad(
        &self,
        inputs: &[f64],
        gradients: &mut [f64],
    ) -> Result<f64, CompileError> {
        self.owner.value_and_grad(inputs, gradients)
    }

    /// Computes `H(inputs) * direction` exactly, without materializing a dense Hessian.
    ///
    /// # Errors
    /// Requires second-order compilation and exactly one direction/output slot per input.
    pub fn hessian_vector_product(
        &self,
        inputs: &[f64],
        direction: &[f64],
        product: &mut [f64],
    ) -> Result<(), CompileError> {
        let second = self
            .second
            .as_ref()
            .ok_or(CompileError::SecondOrderUnavailable)?;
        self.check_second_buffers(inputs, direction, product)?;
        let mut arguments = Vec::with_capacity(self.input_count * 2);
        arguments.extend_from_slice(inputs);
        arguments.extend_from_slice(direction);
        let mut derivatives = vec![0.0; arguments.len()];
        second.value_and_grad(&arguments, &mut derivatives)?;
        product.copy_from_slice(&derivatives[..self.input_count]);
        Ok(())
    }

    /// Returns the value, gradient and full row-major Hessian for small input sets.
    ///
    /// Storage is quadratic in input count. For a portfolio with many market
    /// factors, prefer [`Self::hessian_vector_product`] for selected risk directions.
    /// First-order evaluation never calls this method or allocates its workspace.
    ///
    /// # Errors
    /// Requires second-order compilation, matching buffers, and `n*n` Hessian slots.
    pub fn value_grad_hessian(
        &self,
        inputs: &[f64],
        gradient: &mut [f64],
        hessian: &mut [f64],
    ) -> Result<f64, CompileError> {
        let second = self
            .second
            .as_ref()
            .ok_or(CompileError::SecondOrderUnavailable)?;
        let n = self.input_count;
        let expected = n.checked_mul(n).ok_or(CompileError::OffsetOverflow)?;
        if hessian.len() != expected {
            return Err(CompileError::BufferCount {
                buffer: "Hessian",
                expected,
                actual: hessian.len(),
            });
        }
        let value = self.value_and_grad(inputs, gradient)?;
        let mut arguments = vec![0.0; n * 2];
        arguments[..n].copy_from_slice(inputs);
        let mut derivatives = vec![0.0; arguments.len()];
        for column in 0..n {
            arguments[n + column] = 1.0;
            second.value_and_grad(&arguments, &mut derivatives)?;
            for (row, derivative) in derivatives[..n].iter().enumerate() {
                hessian[row * n + column] = *derivative;
            }
            arguments[n + column] = 0.0;
        }
        Ok(value)
    }

    /// Checks the input, direction and product layouts for native second-order execution.
    fn check_second_buffers(
        &self,
        inputs: &[f64],
        direction: &[f64],
        product: &[f64],
    ) -> Result<(), CompileError> {
        for (buffer, actual) in [
            ("input", inputs.len()),
            ("direction", direction.len()),
            ("Hessian-vector product", product.len()),
        ] {
            if actual != self.input_count {
                return Err(CompileError::BufferCount {
                    buffer,
                    expected: self.input_count,
                    actual,
                });
            }
        }
        Ok(())
    }

    /// Runs arbitrary independently indexed Monte Carlo scenarios in parallel.
    ///
    /// The callback fills input slots for a path index (including any
    /// externally generated, reproducible random draws). Each Rayon task owns
    /// its private buffers, and the code is shared read-only. Partial results
    /// are collected in stable chunk order before the final reduction, so
    /// floating-point reductions are reproducible for a fixed pool size.
    ///
    /// This function returns raw sums. Callers decide whether / how to
    /// normalize for MC, CVA, DVA and other QS estimators.
    ///
    /// # Errors
    /// Propagates input-generation errors and compiled-kernel buffer errors.
    pub fn sum_paths<F>(&self, count: usize, fill_inputs: F) -> Result<PathSums, CompileError>
    where
        F: Fn(usize, &mut [f64]) -> Result<(), CompileError> + Send + Sync,
    {
        if count == 0 {
            return Ok(PathSums::new(self.input_count));
        }
        let threads = rayon::current_num_threads().max(1);
        let chunks = threads.saturating_mul(4).max(1);
        let chunk_size = count.div_ceil(chunks).max(1);
        let ranges: Vec<(usize, usize)> = (0..count)
            .step_by(chunk_size)
            .map(|start| (start, count.min(start.saturating_add(chunk_size))))
            .collect();

        let partials: Result<Vec<PathSums>, CompileError> = ranges
            .into_par_iter()
            .map(|(start, end)| {
                let mut inputs = vec![0.0; self.input_count];
                let mut path_gradients = vec![0.0; self.input_count];
                let mut totals = PathSums::new(self.input_count);
                for index in start..end {
                    fill_inputs(index, &mut inputs)?;
                    let value = self.value_and_grad(&inputs, &mut path_gradients)?;
                    totals.accumulate(value, &path_gradients);
                }
                Ok(totals)
            })
            .collect();
        let mut result = PathSums::new(self.input_count);
        for partial in partials? {
            result.merge(&partial);
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ad::{
            recorded::Recorded,
            scalar::{InnerScalar, Scalar},
            Recording,
        },
        math::probability::{norm_pdf::norm_pdf, normcdf::NormCDF},
    };

    /// Checks second order polynomial and cross derivatives.
    #[test]
    fn second_order_polynomial_and_cross_derivatives() -> Result<(), CompileError> {
        let graph = Graph::trace(&[2.0, 0.4, 7.0], |x| {
            x[0].powf(3.0) + x[0] * x[1].sin() + x[1] * x[1]
        })?;
        let first = graph.compile()?;
        assert_eq!(first.derivative_order(), DerivativeOrder::First);
        assert!(matches!(
            first.hessian_vector_product(&[2.0, 0.4, 7.0], &[1.0; 3], &mut [0.0; 3]),
            Err(CompileError::SecondOrderUnavailable)
        ));
        let second = graph.compile_with_order(DerivativeOrder::Second)?;
        assert_eq!(second.derivative_order(), DerivativeOrder::Second);
        for inputs in [[2.0_f64, 0.4_f64, 7.0], [-1.5, -0.7, 8.0]] {
            let [x, y, _] = inputs;
            let mut gradient = [0.0; 3];
            let mut hessian = [0.0; 9];
            let value = second.value_grad_hessian(&inputs, &mut gradient, &mut hessian)?;
            assert!((value - (x.powi(3) + x * y.sin() + y * y)).abs() < 1e-10);
            let expected = [
                6.0 * x,
                y.cos(),
                0.0,
                y.cos(),
                2.0 - x * y.sin(),
                0.0,
                0.0,
                0.0,
                0.0,
            ];
            for (actual, expected) in hessian.into_iter().zip(expected) {
                assert!((actual - expected).abs() < 1e-10, "{actual} vs {expected}");
            }
            let direction = [0.3, -0.8, 9.0];
            let mut product = [0.0; 3];
            second.hessian_vector_product(&inputs, &direction, &mut product)?;
            for row in 0..3 {
                let expected: f64 = (0..3)
                    .map(|column| hessian[row * 3 + column] * direction[column])
                    .sum();
                assert!((product[row] - expected).abs() < 1e-10);
            }
        }
        Ok(())
    }

    /// Checks second derivatives match changes in first derivatives.
    #[test]
    fn second_derivatives_match_changes_in_first_derivatives() -> Result<(), CompileError> {
        let graph = Graph::trace(&[1.2, 2.3], |x| {
            (x[0].exp() + x[1].ln() - x[0].sqrt()) / (x[0] * x[1] + 1.0)
                + x[0].powf(2.5)
                + x[0].pows(x[1])
                + x[0].cos()
        })?;
        let kernel = graph.compile_with_order(DerivativeOrder::Second)?;
        let inputs = [1.2, 2.3];
        let mut gradient = [0.0; 2];
        let mut hessian = [0.0; 4];
        kernel.value_grad_hessian(&inputs, &mut gradient, &mut hessian)?;
        for column in 0..2 {
            let mut above = inputs;
            let mut below = inputs;
            above[column] += 1e-5;
            below[column] -= 1e-5;
            let mut upper = [0.0; 2];
            let mut lower = [0.0; 2];
            kernel.value_and_grad(&above, &mut upper)?;
            kernel.value_and_grad(&below, &mut lower)?;
            for row in 0..2 {
                assert!(
                    (hessian[row * 2 + column] - (upper[row] - lower[row]) / 2e-5).abs() < 1e-6
                );
            }
        }
        assert!((hessian[1] - hessian[2]).abs() < 1e-10);
        Ok(())
    }

    /// Checks normal cdf and runtime selection have exact second derivatives.
    #[test]
    fn normal_cdf_and_runtime_selection_have_exact_second_derivatives() -> Result<(), CompileError>
    {
        let graph = Graph::trace(&[1.0], |x| x[0].norm_cdf())?;
        let kernel = graph.compile_with_order(DerivativeOrder::Second)?;
        for x in [-2.0_f64, 0.0, 1.5] {
            let mut gradient = [0.0];
            let mut hessian = [0.0];
            let value = kernel.value_grad_hessian(&[x], &mut gradient, &mut hessian)?;
            let density = norm_pdf(x);
            assert!((value - x.norm_cdf()).abs() < 1e-14);
            assert!((gradient[0] - density).abs() < 1e-14);
            assert!((hessian[0] + x * density).abs() < 1e-14);
        }
        let graph = Graph::trace(&[1.0], |x| (x[0] * x[0]).max_val(Recorded::from(1.0)))?;
        let kernel = graph.compile_with_order(DerivativeOrder::Second)?;
        for (x, expected) in [(2.0, 2.0), (0.5, 0.0), (1.0, 0.0)] {
            let mut gradient = [0.0];
            let mut hessian = [0.0];
            kernel.value_grad_hessian(&[x], &mut gradient, &mut hessian)?;
            assert_eq!(hessian[0], expected);
        }
        Ok(())
    }

    /// Checks second order buffer errors and zero power.
    #[test]
    fn second_order_buffer_errors_and_zero_power() -> Result<(), CompileError> {
        let kernel = Graph::trace(&[0.0], |x| x[0].powf(0.0))?
            .compile_with_order(DerivativeOrder::Second)?;
        let mut gradient = [9.0];
        let mut hessian = [9.0];
        assert_eq!(
            kernel.value_grad_hessian(&[0.0], &mut gradient, &mut hessian)?,
            1.0
        );
        assert_eq!(gradient, [0.0]);
        assert_eq!(hessian, [0.0]);
        assert!(kernel
            .value_grad_hessian(&[0.0], &mut gradient, &mut [])
            .is_err());
        assert!(kernel
            .hessian_vector_product(&[0.0], &[], &mut [0.0])
            .is_err());
        assert!(kernel
            .hessian_vector_product(&[], &[1.0], &mut [0.0])
            .is_err());
        assert!(kernel
            .hessian_vector_product(&[0.0], &[1.0], &mut [])
            .is_err());
        Ok(())
    }

    /// Checks value and gradient reuse.
    #[test]
    fn value_and_gradient_reuse() -> Result<(), CompileError> {
        let graph = Graph::trace(&[2.0, 3.0], |x| x[0] * x[0] + x[0] * x[1] + x[1].sin())?;
        let compiled = graph.compile()?;
        let mut gradient = vec![0.0; 2];
        for (x, y) in [(2.0, 3.0), (4.0, 5.0), (-3.0, 2.0)] {
            let value = compiled.value_and_grad(&[x, y], &mut gradient)?;
            assert!((value - (x * x + x * y + y.sin())).abs() < 1e-10);
            assert!((gradient[0] - (2.0 * x + y)).abs() < 1e-10);
            assert!((gradient[1] - (x + y.cos())).abs() < 1e-10);
        }
        Ok(())
    }

    /// Checks scalar frontend and explicit graph agree.
    #[test]
    fn scalar_frontend_and_explicit_graph_agree() -> Result<(), CompileError> {
        let graph = Graph::trace(&[2.0, 3.0], |x| x[0] * x[1] + x[0])?;
        let compiled = graph.compile()?;
        let mut gradients = [0.0; 2];
        let compiled_value = compiled.value_and_grad(&[2.0, 3.0], &mut gradients)?;
        Recording::start();
        let x = Recorded::new(2.0);
        let y = Recorded::new(3.0);
        let output: Recorded = (x * y + x).into();
        output
            .differentiate()
            .expect("existing AD differentiate must succeed");
        assert_eq!(compiled_value, output.value());
        assert_eq!(
            gradients,
            [
                x.derivative().expect("x derivative").value(),
                y.derivative().expect("y derivative").value()
            ]
        );
        Recording::stop();
        Ok(())
    }

    /// Checks max derivative agrees with qs tie policy.
    #[test]
    fn max_derivative_agrees_with_qs_tie_policy() -> Result<(), CompileError> {
        let graph = Graph::trace(&[3.0, 2.0], |x| x[0].max_val(x[1]))?;
        let compiled = graph.compile()?;
        let mut gradient = [0.0; 2];
        assert_eq!(compiled.value_and_grad(&[3.0, 2.0], &mut gradient)?, 3.0);
        assert_eq!(gradient, [1.0, 0.0]);
        assert_eq!(compiled.value_and_grad(&[2.0, 2.0], &mut gradient)?, 2.0);
        assert_eq!(gradient, [0.0, 0.0]);
        Ok(())
    }

    /// Checks rayon parallel sums are correct.
    #[test]
    fn rayon_parallel_sums_are_correct() -> Result<(), CompileError> {
        let graph = Graph::trace(&[2.0, 3.0], |x| x[0] * x[1])?;
        let compiled = graph.compile()?;
        let sums = compiled.sum_paths(10_000, |path, inputs| {
            inputs[0] = path as f64;
            inputs[1] = 2.0;
            Ok(())
        })?;
        assert_eq!(sums.count(), 10_000);
        assert_eq!(sums.value(), 99_990_000.0);
        assert_eq!(sums.gradients(), [20_000.0, 49_995_000.0]);
        Ok(())
    }

    /// Checks min max ignore nan like scalar.
    #[test]
    fn min_max_ignore_nan_like_scalar() -> Result<(), CompileError> {
        for maximum in [false, true] {
            let graph = Graph::trace(&[1.0, 2.0], |x| {
                if maximum {
                    x[0].max_val(x[1])
                } else {
                    x[0].min_val(x[1])
                }
            })?;
            let kernel = graph.compile()?;
            for inputs in [[f64::NAN, 2.0], [2.0, f64::NAN], [f64::NAN, f64::NAN]] {
                let expected = if maximum {
                    inputs[0].max(inputs[1])
                } else {
                    inputs[0].min(inputs[1])
                };
                let mut gradients = [1.0; 2];
                let actual = kernel.value_and_grad(&inputs, &mut gradients)?;
                assert!(
                    actual == expected || (actual.is_nan() && expected.is_nan()),
                    "max={maximum}, inputs={inputs:?}: {actual} vs {expected}"
                );
                // Both strict comparisons are false when either side is NaN.
                assert_eq!(gradients, [0.0; 2]);
            }
        }
        Ok(())
    }

    /// Checks scalar operations match finite differences.
    #[test]
    fn scalar_operations_match_finite_differences() -> Result<(), CompileError> {
        /// Provides a nonlinear scalar expression for an independent derivative check.
        fn formula<T: InnerScalar>(x: &[T]) -> T {
            let a = x[0];
            let b = x[1];
            let smooth = (a.exp() + b.ln() - a.sqrt()) / (a * b + T::one());
            let powers = a.powf(2.5) + a.pows(b);
            smooth + powers + a.sin() - b.cos() + (-a).abs() + a.max_val(b) + a.min_val(b)
        }
        let kernel = Graph::trace(&[1.2, 2.3], formula)?.compile()?;
        for inputs in [[1.2, 2.3], [3.4, 0.8]] {
            let mut gradients = [0.0; 2];
            let value = kernel.value_and_grad(&inputs, &mut gradients)?;
            assert!((value - formula(&inputs)).abs() < 1e-10);
            for (i, gradient) in gradients.into_iter().enumerate() {
                let mut above = inputs;
                let mut below = inputs;
                above[i] += 1e-5;
                below[i] -= 1e-5;
                let expected = (formula(&above) - formula(&below)) / 2e-5;
                assert!(
                    (gradient - expected).abs() < 1e-6,
                    "input {i}: {gradient} vs {expected}"
                );
            }
        }
        Ok(())
    }

    /// Checks cloned kernel keeps executable memory alive.
    #[test]
    fn cloned_kernel_keeps_executable_memory_alive() -> Result<(), CompileError> {
        let original = Graph::trace(&[1.0], |x| x[0] * x[0])?.compile()?;
        let cloned = original.clone();
        drop(original);
        let mut gradient = [0.0];
        assert_eq!(cloned.value_and_grad(&[3.0], &mut gradient)?, 9.0);
        assert_eq!(gradient, [6.0]);
        Ok(())
    }

    /// Checks empty paths and buffer errors.
    #[test]
    fn empty_paths_and_buffer_errors() -> Result<(), CompileError> {
        let kernel = Graph::trace(&[1.0], |x| x[0])?.compile()?;
        assert!(matches!(
            kernel.value_and_grad(&[], &mut [0.0]),
            Err(CompileError::InputCount {
                expected: 1,
                actual: 0
            })
        ));
        assert!(matches!(
            kernel.value_and_grad(&[1.0], &mut []),
            Err(CompileError::GradientCount {
                expected: 1,
                actual: 0
            })
        ));
        let sums = kernel.sum_paths(0, |_, _| panic!("no paths to fill"))?;
        assert_eq!(sums.count(), 0);
        assert_eq!(sums.value(), 0.0);
        assert_eq!(sums.gradients(), [0.0]);
        assert_eq!(sums.mean(), None);
        Ok(())
    }
}
