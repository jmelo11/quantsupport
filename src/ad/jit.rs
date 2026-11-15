//! Cranelift lowering and native reverse-mode differentiation.
//!
//! All graph nodes are lowered to SSA `f64` values. The reverse pass is
//! generated at compilation time and follows input activity, so constants
//! do not receive unnecessary derivative calculations. The generated function
//! accepts only caller-owned buffers, making concurrent calls independent.

use crate::math::probability::{norm_pdf::FRAC_1_SQRT_2PI, normcdf::NormCDF};
use cranelift_codegen::ir::{
    condcodes::FloatCC, types, AbiParam, FuncRef, InstBuilder, MemFlagsData, Value,
};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{default_libcall_names, FuncId, Linkage, Module};
use std::{
    fmt::Display,
    mem::{size_of, transmute},
};

use super::{
    compileerror::CompileError,
    graph::Graph,
    nativekernel::{NativeKernel, RawKernel},
    operation::Operation as Op,
};

/// Exposes the host exponential through the native compiler's C ABI.
extern "C" fn host_exp(x: f64) -> f64 {
    x.exp()
}
/// Exposes the host natural logarithm through the native compiler's C ABI.
extern "C" fn host_ln(x: f64) -> f64 {
    x.ln()
}
/// Exposes the host sine through the native compiler's C ABI.
extern "C" fn host_sin(x: f64) -> f64 {
    x.sin()
}
/// Exposes the host cosine through the native compiler's C ABI.
extern "C" fn host_cos(x: f64) -> f64 {
    x.cos()
}
/// Exposes the host floating-point power through the native compiler's C ABI.
extern "C" fn host_pow(x: f64, y: f64) -> f64 {
    x.powf(y)
}
/// Exposes the library normal CDF through the native compiler's C ABI.
extern "C" fn host_normal_cdf(x: f64) -> f64 {
    x.norm_cdf()
}

struct MathIds {
    normal_cdf: FuncId,
    exp: FuncId,
    ln: FuncId,
    sin: FuncId,
    cos: FuncId,
    pow: FuncId,
}
struct MathRefs {
    normal_cdf: FuncRef,
    exp: FuncRef,
    ln: FuncRef,
    sin: FuncRef,
    cos: FuncRef,
    pow: FuncRef,
}

/// Preserves native compiler diagnostics as a library compilation error.
fn jit_err(e: impl Display) -> CompileError {
    CompileError::Jit(e.to_string())
}

/// Declares the imported host math functions with their native numerical signatures.
fn math_ids(module: &mut JITModule) -> Result<MathIds, CompileError> {
    let mut unary = module.make_signature();
    unary.params.push(AbiParam::new(types::F64));
    unary.returns.push(AbiParam::new(types::F64));
    let mut binary = unary.clone();
    binary.params.push(AbiParam::new(types::F64));
    Ok(MathIds {
        normal_cdf: module
            .declare_function("qs_jit_normal_cdf", Linkage::Import, &unary)
            .map_err(jit_err)?,
        exp: module
            .declare_function("qs_jit_exp", Linkage::Import, &unary)
            .map_err(jit_err)?,
        ln: module
            .declare_function("qs_jit_ln", Linkage::Import, &unary)
            .map_err(jit_err)?,
        sin: module
            .declare_function("qs_jit_sin", Linkage::Import, &unary)
            .map_err(jit_err)?,
        cos: module
            .declare_function("qs_jit_cos", Linkage::Import, &unary)
            .map_err(jit_err)?,
        pow: module
            .declare_function("qs_jit_pow", Linkage::Import, &binary)
            .map_err(jit_err)?,
    })
}

/// Emits a call to a host function with one floating-point argument.
fn call1(builder: &mut FunctionBuilder<'_>, f: FuncRef, x: Value) -> Value {
    let call = builder.ins().call(f, &[x]);
    builder.inst_results(call)[0]
}
/// Emits a call to a host function with two floating-point arguments.
fn call2(builder: &mut FunctionBuilder<'_>, f: FuncRef, x: Value, y: Value) -> Value {
    let call = builder.ins().call(f, &[x, y]);
    builder.inst_results(call)[0]
}

/// Lowers min/max with the same NaN behavior as the scalar implementation.
fn min_max(builder: &mut FunctionBuilder<'_>, a: Value, b: Value, maximum: bool) -> Value {
    let value = if maximum {
        builder.ins().fmax(a, b)
    } else {
        builder.ins().fmin(a, b)
    };
    // Cranelift min/max propagate NaNs; Scalar/f64 min/max ignore a single NaN.
    let a_nan = builder.ins().fcmp(FloatCC::Unordered, a, a);
    let b_nan = builder.ins().fcmp(FloatCC::Unordered, b, b);
    let value = builder.ins().select(a_nan, b, value);
    builder.ins().select(b_nan, a, value)
}

/// Converts scalar buffer indices to checked native byte offsets.
fn offset(index: usize) -> Result<i32, CompileError> {
    index
        .checked_mul(size_of::<f64>())
        .and_then(|x| i32::try_from(x).ok())
        .ok_or(CompileError::OffsetOverflow)
}

/// Adds a symbolic derivative contribution to a live reverse accumulator.
fn accumulate(
    builder: &mut FunctionBuilder<'_>,
    adjoints: &mut [Option<Value>],
    target: usize,
    contribution: Value,
) {
    let next = adjoints[target].map_or(contribution, |previous| {
        builder.ins().fadd(previous, contribution)
    });
    adjoints[target] = Some(next);
}

/// Compile a validated straight-line QS numerical graph to native machine code.
///
/// # Errors
/// Returns a native compilation, relocation or address-layout error.
pub fn compile(graph: &Graph) -> Result<NativeKernel, CompileError> {
    compile_outputs(graph, &[graph.output()], graph.input_count())
}

/// Compiles additional forward outputs and derivatives for an active input prefix.
///
/// Noise inputs outside the prefix participate in evaluation but receive no reverse work.
///
/// # Errors
/// Rejects invalid output indices, activity layouts and native code-generation failures.
pub fn compile_outputs(
    graph: &Graph,
    outputs: &[usize],
    active_inputs: usize,
) -> Result<NativeKernel, CompileError> {
    compile_selected(graph, outputs, active_inputs, false)
}

/// Compiles output-major Jacobian rows sharing one forward calculation.
///
/// # Errors
/// Rejects invalid outputs, overflowing layouts and native compilation failures.
pub fn compile_jacobian(graph: &Graph, outputs: &[usize]) -> Result<NativeKernel, CompileError> {
    compile_selected(graph, outputs, graph.input_count(), true)
}

/// Lowers requested observations and active first derivatives into one native function.
#[allow(clippy::too_many_lines)] // Keep the forward/reverse operation dispatch together.
fn compile_selected(
    graph: &Graph,
    outputs: &[usize],
    active_inputs: usize,
    jacobian: bool,
) -> Result<NativeKernel, CompileError> {
    let (pruned, outputs) = graph.prune(outputs)?;
    let graph = &pruned;
    if active_inputs > graph.input_count()
        || outputs.iter().any(|&id| id >= graph.operation_count())
    {
        return Err(CompileError::InvalidGraph(
            "invalid output or active-input layout".into(),
        ));
    }
    let roots = if jacobian {
        outputs.clone()
    } else {
        vec![graph.output()]
    };
    let gradient_count = roots
        .len()
        .checked_mul(active_inputs)
        .ok_or(CompileError::OffsetOverflow)?;
    let mut jit_builder =
        JITBuilder::with_flags(&[("opt_level", "speed")], default_libcall_names())
            .map_err(jit_err)?;
    jit_builder.symbol("qs_jit_exp", host_exp as *const u8);
    jit_builder.symbol("qs_jit_ln", host_ln as *const u8);
    jit_builder.symbol("qs_jit_sin", host_sin as *const u8);
    jit_builder.symbol("qs_jit_cos", host_cos as *const u8);
    jit_builder.symbol("qs_jit_pow", host_pow as *const u8);
    jit_builder.symbol("qs_jit_normal_cdf", host_normal_cdf as *const u8);
    let mut module = JITModule::new(jit_builder);
    let math = math_ids(&mut module)?;

    // ABI: (inputs: *const f64, output: *mut f64, grad: *mut f64) -> void
    let ptr_type = module.target_config().pointer_type();
    let mut signature = module.make_signature();
    for _ in 0..3 {
        signature.params.push(AbiParam::new(ptr_type));
    }
    let function_id = module
        .declare_function("qs_compiled_value_and_grad", Linkage::Local, &signature)
        .map_err(jit_err)?;
    let mut context = module.make_context();
    context.func.signature = signature;
    let mut fbc = FunctionBuilderContext::new();
    let frontend_config = module.target_config();

    {
        let mut b = FunctionBuilder::new(&mut context.func, &mut fbc);
        let block = b.create_block();
        b.append_block_params_for_function_params(block);
        b.switch_to_block(block);
        b.seal_block(block);
        let params = b.block_params(block).to_vec();
        let input_ptr = params[0];
        let output_ptr = params[1];
        let grad_ptr = params[2];

        let refs = MathRefs {
            normal_cdf: module.declare_func_in_func(math.normal_cdf, b.func),
            exp: module.declare_func_in_func(math.exp, b.func),
            ln: module.declare_func_in_func(math.ln, b.func),
            sin: module.declare_func_in_func(math.sin, b.func),
            cos: module.declare_func_in_func(math.cos, b.func),
            pow: module.declare_func_in_func(math.pow, b.func),
        };

        let mut values: Vec<Value> = Vec::with_capacity(graph.operations().len());
        let mut active: Vec<bool> = Vec::with_capacity(graph.operations().len());
        for op in graph.operations() {
            let (value, dependency) = match *op {
                Op::Input(index) => (
                    b.ins()
                        .load(types::F64, MemFlagsData::new(), input_ptr, offset(index)?),
                    index < active_inputs,
                ),
                Op::Constant(v) => (b.ins().f64const(v), false),
                Op::Add(a, c) => (b.ins().fadd(values[a], values[c]), active[a] || active[c]),
                Op::Sub(a, c) => (b.ins().fsub(values[a], values[c]), active[a] || active[c]),
                Op::Mul(a, c) => (b.ins().fmul(values[a], values[c]), active[a] || active[c]),
                Op::Div(a, c) => (b.ins().fdiv(values[a], values[c]), active[a] || active[c]),
                Op::Neg(a) => (b.ins().fneg(values[a]), active[a]),
                Op::Exp(a) => (call1(&mut b, refs.exp, values[a]), active[a]),
                Op::Ln(a) => (call1(&mut b, refs.ln, values[a]), active[a]),
                Op::Sqrt(a) => (b.ins().sqrt(values[a]), active[a]),
                Op::Sin(a) => (call1(&mut b, refs.sin, values[a]), active[a]),
                Op::Cos(a) => (call1(&mut b, refs.cos, values[a]), active[a]),
                Op::Abs(a) => (b.ins().fabs(values[a]), active[a]),
                Op::Pow(a, c) => (
                    call2(&mut b, refs.pow, values[a], values[c]),
                    active[a] || active[c],
                ),
                Op::Max(a, c) => (
                    min_max(&mut b, values[a], values[c], true),
                    active[a] || active[c],
                ),
                Op::Min(a, c) => (
                    min_max(&mut b, values[a], values[c], false),
                    active[a] || active[c],
                ),
                Op::NormalCdf(a) => (call1(&mut b, refs.normal_cdf, values[a]), active[a]),
                Op::SelectGreater(a, c, yes, no) => {
                    let condition = b.ins().fcmp(FloatCC::GreaterThan, values[a], values[c]);
                    (
                        b.ins().select(condition, values[yes], values[no]),
                        active[yes] || active[no],
                    )
                }
            };
            values.push(value);
            active.push(dependency);
        }
        for (slot, &output) in outputs.iter().enumerate() {
            b.ins().store(
                MemFlagsData::new(),
                values[output],
                output_ptr,
                offset(slot)?,
            );
        }

        for (row, root) in roots.into_iter().enumerate() {
            let mut adjoints = vec![None; values.len()];
            adjoints[root] = Some(b.ins().f64const(1.0));
            for (id, op) in graph.operations().iter().enumerate().rev() {
                let Some(seed) = adjoints[id] else { continue };
                match *op {
                    Op::Input(_) | Op::Constant(_) => {}
                    Op::SelectGreater(a, c, yes, no) => {
                        let zero = b.ins().f64const(0.0);
                        let condition = b.ins().fcmp(FloatCC::GreaterThan, values[a], values[c]);
                        if active[yes] {
                            let contribution = b.ins().select(condition, seed, zero);
                            accumulate(&mut b, &mut adjoints, yes, contribution);
                        }
                        if active[no] {
                            let contribution = b.ins().select(condition, zero, seed);
                            accumulate(&mut b, &mut adjoints, no, contribution);
                        }
                    }
                    Op::NormalCdf(a) => {
                        if active[a] {
                            let square = b.ins().fmul(values[a], values[a]);
                            let half = b.ins().f64const(-0.5);
                            let exponent = b.ins().fmul(square, half);
                            let exponential = call1(&mut b, refs.exp, exponent);
                            let scale = b.ins().f64const(FRAC_1_SQRT_2PI);
                            let density = b.ins().fmul(exponential, scale);
                            let contribution = b.ins().fmul(seed, density);
                            accumulate(&mut b, &mut adjoints, a, contribution);
                        }
                    }
                    Op::Add(a, c) => {
                        if active[a] {
                            accumulate(&mut b, &mut adjoints, a, seed);
                        }
                        if active[c] {
                            accumulate(&mut b, &mut adjoints, c, seed);
                        }
                    }
                    Op::Sub(a, c) => {
                        if active[a] {
                            accumulate(&mut b, &mut adjoints, a, seed);
                        }
                        if active[c] {
                            let neg = b.ins().fneg(seed);
                            accumulate(&mut b, &mut adjoints, c, neg);
                        }
                    }
                    Op::Mul(a, c) => {
                        if active[a] {
                            let d = b.ins().fmul(seed, values[c]);
                            accumulate(&mut b, &mut adjoints, a, d);
                        }
                        if active[c] {
                            let d = b.ins().fmul(seed, values[a]);
                            accumulate(&mut b, &mut adjoints, c, d);
                        }
                    }
                    Op::Div(a, c) => {
                        if active[a] {
                            let d = b.ins().fdiv(seed, values[c]);
                            accumulate(&mut b, &mut adjoints, a, d);
                        }
                        if active[c] {
                            let c_sq = b.ins().fmul(values[c], values[c]);
                            let neg_a = b.ins().fneg(values[a]);
                            let numerator = b.ins().fmul(seed, neg_a);
                            let d = b.ins().fdiv(numerator, c_sq);
                            accumulate(&mut b, &mut adjoints, c, d);
                        }
                    }
                    Op::Neg(a) => {
                        if active[a] {
                            let d = b.ins().fneg(seed);
                            accumulate(&mut b, &mut adjoints, a, d);
                        }
                    }
                    Op::Exp(a) => {
                        if active[a] {
                            let d = b.ins().fmul(seed, values[id]);
                            accumulate(&mut b, &mut adjoints, a, d);
                        }
                    }
                    Op::Ln(a) => {
                        if active[a] {
                            let d = b.ins().fdiv(seed, values[a]);
                            accumulate(&mut b, &mut adjoints, a, d);
                        }
                    }
                    Op::Sqrt(a) => {
                        if active[a] {
                            let twice = b.ins().f64const(2.0);
                            let denom = b.ins().fmul(twice, values[id]);
                            let d = b.ins().fdiv(seed, denom);
                            accumulate(&mut b, &mut adjoints, a, d);
                        }
                    }
                    Op::Sin(a) => {
                        if active[a] {
                            let cos = call1(&mut b, refs.cos, values[a]);
                            let d = b.ins().fmul(seed, cos);
                            accumulate(&mut b, &mut adjoints, a, d);
                        }
                    }
                    Op::Cos(a) => {
                        if active[a] {
                            let sin = call1(&mut b, refs.sin, values[a]);
                            let neg = b.ins().fneg(sin);
                            let d = b.ins().fmul(seed, neg);
                            accumulate(&mut b, &mut adjoints, a, d);
                        }
                    }
                    Op::Abs(a) => {
                        if active[a] {
                            let zero = b.ins().f64const(0.0);
                            let positive = b.ins().fcmp(FloatCC::GreaterThan, values[a], zero);
                            let negative = b.ins().fcmp(FloatCC::LessThan, values[a], zero);
                            let neg_seed = b.ins().fneg(seed);
                            let negative_or_zero = b.ins().select(negative, neg_seed, zero);
                            let d = b.ins().select(positive, seed, negative_or_zero);
                            accumulate(&mut b, &mut adjoints, a, d);
                        }
                    }
                    Op::Pow(a, c) => {
                        if active[a] {
                            let one = b.ins().f64const(1.0);
                            let reduced_power = b.ins().fsub(values[c], one);
                            let pow = call2(&mut b, refs.pow, values[a], reduced_power);
                            let scaled = b.ins().fmul(values[c], pow);
                            let d = b.ins().fmul(seed, scaled);
                            accumulate(&mut b, &mut adjoints, a, d);
                        }
                        if active[c] {
                            let ln = call1(&mut b, refs.ln, values[a]);
                            let scaled = b.ins().fmul(values[id], ln);
                            let d = b.ins().fmul(seed, scaled);
                            accumulate(&mut b, &mut adjoints, c, d);
                        }
                    }
                    Op::Max(a, c) | Op::Min(a, c) => {
                        // Match QS BinOp's tie behavior: both partials are zero.
                        let (lcond, rcond) = if matches!(op, Op::Max(_, _)) {
                            (FloatCC::GreaterThan, FloatCC::LessThan)
                        } else {
                            (FloatCC::LessThan, FloatCC::GreaterThan)
                        };
                        let zero = b.ins().f64const(0.0);
                        if active[a] {
                            let cmp = b.ins().fcmp(lcond, values[a], values[c]);
                            let d = b.ins().select(cmp, seed, zero);
                            accumulate(&mut b, &mut adjoints, a, d);
                        }
                        if active[c] {
                            let cmp = b.ins().fcmp(rcond, values[a], values[c]);
                            let d = b.ins().select(cmp, seed, zero);
                            accumulate(&mut b, &mut adjoints, c, d);
                        }
                    }
                }
            }
            for (i, derivative) in adjoints.iter().take(active_inputs).enumerate() {
                let gradient = derivative.unwrap_or_else(|| b.ins().f64const(0.0));
                b.ins().store(
                    MemFlagsData::new(),
                    gradient,
                    grad_ptr,
                    offset(row * active_inputs + i)?,
                );
            }
        }
        b.ins().return_(&[]);
        b.finalize(frontend_config);
    }

    module
        .define_function(function_id, &mut context)
        .map_err(jit_err)?;
    module.clear_context(&mut context);
    module.finalize_definitions().map_err(jit_err)?;
    let ptr = module.get_finalized_function(function_id);
    // SAFETY: Cranelift defined the exact three-pointer platform C ABI above.
    // The owning JITModule stays alive in NativeKernel for every function call.
    let function: RawKernel = unsafe { transmute(ptr) };
    // SAFETY: The generator above defines exactly this layout and C ABI.
    Ok(unsafe {
        NativeKernel::with_layout(
            module,
            function,
            graph.input_count(),
            outputs.len(),
            gradient_count,
        )
    })
}
