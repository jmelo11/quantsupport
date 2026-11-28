//! Native first derivatives with bounded compilation units for large pricing graphs.

use std::{
    collections::{hash_map::DefaultHasher, BTreeSet, HashMap},
    hash::{Hash, Hasher},
    mem::discriminant,
    sync::{Arc, Mutex, OnceLock, PoisonError},
};

use crate::utils::errors::{QSError, Result};

use super::{graph::Graph, jit, nativekernel::NativeKernel, operation::Operation};

const BLOCK_SIZE: usize = 384;
const PARTITION_THRESHOLD: usize = 2048;

struct Block {
    inputs: Vec<usize>,
    outputs: Vec<usize>,
    active_inputs: usize,
    forward: Arc<NativeKernel>,
    reverse: Arc<NativeKernel>,
}

enum Execution {
    Native(Box<NativeKernel>),
    Blocks {
        blocks: Vec<Block>,
        operations: usize,
        root: usize,
    },
}

/// Compiled gradient execution, partitioning large graphs into native blocks.
///
/// Blocks contain fixed machine code and fixed buffer layouts. Execution does
/// not interpret arithmetic instructions or local derivative rules. Forward
/// blocks return live boundary values; reverse blocks apply their compiled
/// vector-Jacobian products to boundary seeds. First order has no Hessian work.
pub struct GradientKernel {
    execution: Execution,
    inputs: usize,
    active: usize,
}

impl GradientKernel {
    /// Compiles a scalar gradient, bounding individual compilation-unit size.
    ///
    /// # Errors
    /// Rejects invalid activity layouts and propagates native compilation errors.
    #[allow(clippy::too_many_lines)] // Forward and VJP block layouts are assembled together.
    pub fn compile(graph: &Graph, active: usize) -> Result<Self> {
        if active > graph.input_count() {
            return Err(QSError::InputCount {
                expected: graph.input_count(),
                actual: active,
            });
        }
        let execution = if graph.operation_count() <= PARTITION_THRESHOLD {
            Execution::Native(Box::new(jit::compile_outputs(graph, &[], active)?))
        } else {
            let operations = graph.operations();
            let mut last_use = vec![0; operations.len()];
            let mut activity = Vec::with_capacity(operations.len());
            for (index, operation) in operations.iter().enumerate() {
                for child in operation.operands() {
                    last_use[child] = index;
                }
                activity.push(match operation {
                    Operation::Input(input) => *input < active,
                    Operation::Constant(_) => false,
                    Operation::SelectGreater(_, _, yes, no) => activity[*yes] || activity[*no],
                    _ => operation
                        .operands()
                        .into_iter()
                        .any(|child| activity[child]),
                });
            }
            last_use[graph.output()] = operations.len();
            let mut blocks = Vec::new();
            let mut mapping = vec![usize::MAX; operations.len()];
            for start in (graph.input_count()..operations.len()).step_by(BLOCK_SIZE) {
                let end = (start + BLOCK_SIZE).min(operations.len());
                let mut boundaries = BTreeSet::new();
                for operation in &operations[start..end] {
                    boundaries.extend(
                        operation
                            .operands()
                            .into_iter()
                            .filter(|&input| input < start),
                    );
                }
                let mut inputs: Vec<_> = boundaries.into_iter().collect();
                inputs.sort_by_key(|&input| (!activity[input], input));
                let active_inputs = inputs.iter().take_while(|&&input| activity[input]).count();
                let outputs: Vec<_> = (start..end)
                    .filter(|&index| last_use[index] >= end)
                    .collect();
                if outputs.is_empty() {
                    continue;
                }
                for (local, &global) in inputs.iter().enumerate() {
                    mapping[global] = local;
                }
                let mut forward_operations =
                    (0..inputs.len()).map(Operation::Input).collect::<Vec<_>>();
                for index in start..end {
                    mapping[index] = forward_operations.len();
                    forward_operations.push(operations[index].remap(|child| mapping[child]));
                }
                let observed: Vec<_> = outputs.iter().map(|&output| mapping[output]).collect();
                let forward_graph = Graph::new(forward_operations, inputs.len(), observed[0])?;
                let forward = shared_native(&forward_graph, &observed, 0)?;
                // Reverse seeds are inactive runtime inputs. The compiler generates
                // the local VJP from the very same block definition.
                let mut reverse_operations = (0..inputs.len() + outputs.len())
                    .map(Operation::Input)
                    .collect::<Vec<_>>();
                for index in start..end {
                    mapping[index] = reverse_operations.len();
                    reverse_operations.push(operations[index].remap(|child| mapping[child]));
                }
                let mut root = reverse_operations.len();
                reverse_operations.push(Operation::Constant(0.0));
                for (seed, &output) in outputs.iter().enumerate() {
                    let weighted = reverse_operations.len();
                    reverse_operations.push(Operation::Mul(mapping[output], inputs.len() + seed));
                    let next = reverse_operations.len();
                    reverse_operations.push(Operation::Add(root, weighted));
                    root = next;
                }
                let reverse_graph =
                    Graph::new(reverse_operations, inputs.len() + outputs.len(), root)?;
                let reverse = shared_native(&reverse_graph, &[], active_inputs)?;
                blocks.push(Block {
                    inputs,
                    outputs,
                    active_inputs,
                    forward,
                    reverse,
                });
            }
            Execution::Blocks {
                blocks,
                operations: operations.len(),
                root: graph.output(),
            }
        };
        Ok(Self {
            execution,
            inputs: graph.input_count(),
            active,
        })
    }

    /// Runs compiled derivative arithmetic against current values.
    ///
    /// # Errors
    /// Rejects input/gradient shapes and propagates native execution errors.
    pub fn execute(&self, inputs: &[f64], gradient: &mut [f64]) -> Result<()> {
        if inputs.len() != self.inputs {
            return Err(QSError::InputCount {
                expected: self.inputs,
                actual: inputs.len(),
            });
        }
        if gradient.len() != self.active {
            return Err(QSError::GradientCount {
                expected: self.active,
                actual: gradient.len(),
            });
        }
        match &self.execution {
            Execution::Native(kernel) => kernel.values_and_grad(inputs, &mut [], gradient),
            Execution::Blocks {
                blocks,
                operations,
                root,
            } => {
                let mut values = vec![0.0; *operations];
                values[..self.inputs].copy_from_slice(inputs);
                let mut derivatives = vec![0.0; *operations];
                let max_inputs = blocks
                    .iter()
                    .map(|block| block.inputs.len() + block.outputs.len())
                    .max()
                    .unwrap_or(0);
                let max_outputs = blocks
                    .iter()
                    .map(|block| block.outputs.len())
                    .max()
                    .unwrap_or(0);
                let mut arguments = vec![0.0; max_inputs];
                let mut outputs = vec![0.0; max_outputs];
                let mut partials = vec![0.0; max_inputs];
                for block in blocks {
                    for (value, &input) in arguments.iter_mut().zip(&block.inputs) {
                        *value = values[input];
                    }
                    block.forward.values_and_grad(
                        &arguments[..block.inputs.len()],
                        &mut outputs[..block.outputs.len()],
                        &mut [],
                    )?;
                    for (&output, &value) in block.outputs.iter().zip(&outputs) {
                        values[output] = value;
                    }
                }
                derivatives[*root] = 1.0;
                for block in blocks.iter().rev() {
                    for (value, &input) in arguments.iter_mut().zip(&block.inputs) {
                        *value = values[input];
                    }
                    for (seed, &output) in arguments[block.inputs.len()..]
                        .iter_mut()
                        .zip(&block.outputs)
                    {
                        *seed = derivatives[output];
                    }
                    block.reverse.values_and_grad(
                        &arguments[..block.inputs.len() + block.outputs.len()],
                        &mut [],
                        &mut partials[..block.active_inputs],
                    )?;
                    for (&input, &partial) in
                        block.inputs.iter().zip(&partials[..block.active_inputs])
                    {
                        derivatives[input] += partial;
                    }
                }
                gradient.copy_from_slice(&derivatives[..self.active]);
                Ok(())
            }
        }
    }
}

struct SharedProgram {
    graph: Graph,
    outputs: Vec<usize>,
    active: usize,
    kernel: Arc<Mutex<Option<Arc<NativeKernel>>>>,
}
type SharedCache = Mutex<HashMap<u64, Vec<SharedProgram>>>;
static SHARED: OnceLock<SharedCache> = OnceLock::new();

/// Reuses an exactly matching native block within the shared bounded code cache.
fn shared_native(graph: &Graph, outputs: &[usize], active: usize) -> Result<Arc<NativeKernel>> {
    shared_native_in(
        SHARED.get_or_init(|| Mutex::new(HashMap::new())),
        graph,
        outputs,
        active,
        || jit::compile_outputs(graph, outputs, active),
    )
}

/// Serializes compilation only for the same block, leaving unrelated cache entries available.
fn shared_native_in(
    cache: &SharedCache,
    graph: &Graph,
    outputs: &[usize],
    active: usize,
    compile: impl FnOnce() -> Result<NativeKernel>,
) -> Result<Arc<NativeKernel>> {
    let mut hasher = DefaultHasher::new();
    active.hash(&mut hasher);
    graph.output().hash(&mut hasher);
    graph.input_count().hash(&mut hasher);
    outputs.hash(&mut hasher);
    for operation in graph.operations() {
        discriminant(operation).hash(&mut hasher);
        match operation {
            Operation::Input(index) => index.hash(&mut hasher),
            Operation::Constant(value) => value.to_bits().hash(&mut hasher),
            _ => operation.operands().hash(&mut hasher),
        }
    }
    let hash = hasher.finish();
    let slot = {
        let mut shared = cache.lock().unwrap_or_else(PoisonError::into_inner);
        let cached = shared
            .get(&hash)
            .and_then(|programs| {
                programs.iter().find(|program| {
                    program.active == active
                        && program.outputs == outputs
                        && program.graph.output() == graph.output()
                        && program.graph.input_count() == graph.input_count()
                        && program.graph.operation_count() == graph.operation_count()
                        && program
                            .graph
                            .operations()
                            .iter()
                            .zip(graph.operations())
                            .all(|(a, b)| a.same_instruction(b))
                })
            })
            .map(|program| Arc::clone(&program.kernel));
        let slot = cached.unwrap_or_else(|| {
            if shared.len() >= 256 {
                shared.clear();
            }
            let slot = Arc::new(Mutex::new(None));
            shared.entry(hash).or_default().push(SharedProgram {
                graph: graph.clone(),
                outputs: outputs.to_vec(),
                active,
                kernel: Arc::clone(&slot),
            });
            slot
        });
        drop(shared);
        slot
    };
    // The global map is unlocked before any compiler work. A per-entry lock
    // still prevents duplicate compilation of the same resident block.
    let mut kernel = slot.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(kernel) = kernel.as_ref() {
        return Ok(Arc::clone(kernel));
    }
    let compiled = Arc::new(compile()?);
    *kernel = Some(Arc::clone(&compiled));
    drop(kernel);
    Ok(compiled)
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc, Mutex,
        },
    };

    use cranelift_module::ModuleError;

    use crate::{
        ad::{graph::Graph, jit},
        utils::errors::{QSError, Result},
    };

    use super::{shared_native_in, GradientKernel};

    /// Checks compiler work can access other entries and a resident block compiles once.
    #[test]
    fn block_compilation_releases_cache_lock_and_reuses_code() -> Result<()> {
        let cache = Mutex::new(HashMap::new());
        let graph = Graph::trace(&[2.0], |x| x[0] * x[0])?;
        let first = shared_native_in(&cache, &graph, &[], 1, || {
            assert!(
                cache.try_lock().is_ok(),
                "compiler must not hold the cache map lock"
            );
            jit::compile_outputs(&graph, &[], 1)
        })?;
        let second = shared_native_in(&cache, &graph, &[], 1, || {
            panic!("resident native block must be reused")
        })?;
        assert!(Arc::ptr_eq(&first, &second));
        let mut gradient = [0.0];
        second.values_and_grad(&[3.0], &mut [], &mut gradient)?;
        assert_eq!(gradient, [6.0]);
        Ok(())
    }

    /// Checks concurrent requests for the same block share one compilation.
    #[test]
    fn concurrent_block_requests_compile_once() -> Result<()> {
        let cache = Mutex::new(HashMap::new());
        let graph = Graph::trace(&[2.0], |x| x[0] * x[0])?;
        let compilations = AtomicUsize::new(0);
        let kernels = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..4)
                .map(|_| {
                    scope.spawn(|| {
                        shared_native_in(&cache, &graph, &[], 1, || {
                            compilations.fetch_add(1, Ordering::Relaxed);
                            jit::compile_outputs(&graph, &[], 1)
                        })
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| {
                    handle
                        .join()
                        .unwrap_or_else(|_| panic!("native compiler worker panicked"))
                })
                .collect::<Result<Vec<_>>>()
        })?;
        assert_eq!(compilations.load(Ordering::Relaxed), 1);
        assert!(kernels
            .iter()
            .all(|kernel| Arc::ptr_eq(&kernels[0], kernel)));
        Ok(())
    }

    /// Checks a failed compilation leaves the entry available for a retry.
    #[test]
    fn failed_block_compilation_can_be_retried() -> Result<()> {
        let cache = Mutex::new(HashMap::new());
        let graph = Graph::trace(&[2.0], |x| x[0] * x[0])?;
        assert!(shared_native_in(&cache, &graph, &[], 1, || {
            Err(QSError::Jit(Box::new(ModuleError::Undeclared(
                "injected compilation failure".into(),
            ))))
        })
        .is_err());
        let kernel = shared_native_in(&cache, &graph, &[], 1, || {
            jit::compile_outputs(&graph, &[], 1)
        })?;
        let mut gradient = [0.0];
        kernel.values_and_grad(&[4.0], &mut [], &mut gradient)?;
        assert_eq!(gradient, [8.0]);
        Ok(())
    }
    /// Checks partitioned gradient composes shared boundaries and inactive draws.
    #[test]
    fn partitioned_gradient_composes_shared_boundaries_and_inactive_draws() -> Result<()> {
        let graph = Graph::trace(&[2.0, 3.0], |x| {
            (0..2500).fold(x[0], |value, _| value * 0.999 + x[0] * x[1] * 0.001)
        })?;
        let kernel = GradientKernel::compile(&graph, 1)?;
        let mut gradient = [0.0];
        kernel.execute(&[2.0, 3.0], &mut gradient)?;
        let decay = 0.999_f64.powi(2500);
        assert!((gradient[0] - (decay + 3.0 * (1.0 - decay))).abs() < 1e-10);
        kernel.execute(&[4.0, 5.0], &mut gradient)?;
        assert!((gradient[0] - (decay + 5.0 * (1.0 - decay))).abs() < 1e-10);
        Ok(())
    }
}
