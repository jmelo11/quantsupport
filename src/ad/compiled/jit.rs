//! Cranelift lowering and native reverse-mode differentiation.
//!
//! All graph nodes are lowered to SSA `f64` values. The reverse pass is
//! generated at compilation time and follows input activity, so constants
//! do not receive unnecessary adjoint calculations. The generated function
//! accepts only caller-owned buffers, making concurrent calls independent.

use std::sync::{Arc, Mutex};

use cranelift_codegen::ir::{
    condcodes::FloatCC, types, AbiParam, FuncRef, InstBuilder, MemFlags, Value,
};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{default_libcall_names, FuncId, Linkage, Module};

use super::{graph::Op, CompileError, CompiledKernel, Graph, KernelOwner, RawKernel};

extern "C" fn host_exp(x: f64) -> f64 { x.exp() }
extern "C" fn host_ln(x: f64) -> f64 { x.ln() }
extern "C" fn host_sin(x: f64) -> f64 { x.sin() }
extern "C" fn host_cos(x: f64) -> f64 { x.cos() }
extern "C" fn host_pow(x: f64, y: f64) -> f64 { x.powf(y) }

struct MathIds {
    exp: FuncId,
    ln: FuncId,
    sin: FuncId,
    cos: FuncId,
    pow: FuncId,
}
struct MathRefs {
    exp: FuncRef,
    ln: FuncRef,
    sin: FuncRef,
    cos: FuncRef,
    pow: FuncRef,
}

fn jit_err(e: impl std::fmt::Display) -> CompileError {
    CompileError::Jit(e.to_string())
}

fn math_ids(module: &mut JITModule) -> Result<MathIds, CompileError> {
    let mut unary = module.make_signature();
    unary.params.push(AbiParam::new(types::F64));
    unary.returns.push(AbiParam::new(types::F64));
    let mut binary = unary.clone();
    binary.params.push(AbiParam::new(types::F64));
    Ok(MathIds {
        exp: module.declare_function("qs_jit_exp", Linkage::Import, &unary).map_err(jit_err)?,
        ln: module.declare_function("qs_jit_ln", Linkage::Import, &unary).map_err(jit_err)?,
        sin: module.declare_function("qs_jit_sin", Linkage::Import, &unary).map_err(jit_err)?,
        cos: module.declare_function("qs_jit_cos", Linkage::Import, &unary).map_err(jit_err)?,
        pow: module.declare_function("qs_jit_pow", Linkage::Import, &binary).map_err(jit_err)?,
    })
}

fn call1(builder: &mut FunctionBuilder<'_>, f: FuncRef, x: Value) -> Value {
    let call = builder.ins().call(f, &[x]);
    builder.inst_results(call)[0]
}
fn call2(builder: &mut FunctionBuilder<'_>, f: FuncRef, x: Value, y: Value) -> Value {
    let call = builder.ins().call(f, &[x, y]);
    builder.inst_results(call)[0]
}

fn offset(index: usize) -> Result<i32, CompileError> {
    index.checked_mul(std::mem::size_of::<f64>())
        .and_then(|x| i32::try_from(x).ok())
        .ok_or(CompileError::OffsetOverflow)
}

fn accumulate(
    builder: &mut FunctionBuilder<'_>,
    adjoints: &mut [Option<Value>],
    target: usize,
    contribution: Value,
) {
    let next = match adjoints[target] {
        Some(previous) => builder.ins().fadd(previous, contribution),
        None => contribution,
    };
    adjoints[target] = Some(next);
}

/// Compile a validated straight-line QS numerical graph to native machine code.
pub(super) fn compile(graph: &Graph) -> Result<CompiledKernel, CompileError> {
    let mut jit_builder = JITBuilder::new(default_libcall_names()).map_err(jit_err)?;
    jit_builder.symbol("qs_jit_exp", host_exp as *const u8);
    jit_builder.symbol("qs_jit_ln", host_ln as *const u8);
    jit_builder.symbol("qs_jit_sin", host_sin as *const u8);
    jit_builder.symbol("qs_jit_cos", host_cos as *const u8);
    jit_builder.symbol("qs_jit_pow", host_pow as *const u8);
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
            exp: module.declare_func_in_func(math.exp, b.func),
            ln: module.declare_func_in_func(math.ln, b.func),
            sin: module.declare_func_in_func(math.sin, b.func),
            cos: module.declare_func_in_func(math.cos, b.func),
            pow: module.declare_func_in_func(math.pow, b.func),
        };

        let mut values: Vec<Value> = Vec::with_capacity(graph.ops.len());
        let mut active: Vec<bool> = Vec::with_capacity(graph.ops.len());
        for op in &graph.ops {
            let (value, dependency) = match *op {
                Op::Input(index) => (
                    b.ins().load(types::F64, MemFlags::from_u32(0), input_ptr, offset(index)?),
                    true,
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
                Op::Max(a, c) => (b.ins().fmax(values[a], values[c]), active[a] || active[c]),
                Op::Min(a, c) => (b.ins().fmin(values[a], values[c]), active[a] || active[c]),
            };
            values.push(value);
            active.push(dependency);
        }
        b.ins().store(MemFlags::from_u32(0), values[graph.output], output_ptr, 0);

        let mut adjoints = vec![None; values.len()];
        adjoints[graph.output] = Some(b.ins().f64const(1.0));
        for (id, op) in graph.ops.iter().enumerate().rev() {
            let Some(seed) = adjoints[id] else { continue };
            match *op {
                Op::Input(_) | Op::Constant(_) => {}
                Op::Add(a, c) => {
                    if active[a] { accumulate(&mut b, &mut adjoints, a, seed); }
                    if active[c] { accumulate(&mut b, &mut adjoints, c, seed); }
                }
                Op::Sub(a, c) => {
                    if active[a] { accumulate(&mut b, &mut adjoints, a, seed); }
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
        for i in 0..graph.inputs {
            let gradient = adjoints[i].unwrap_or_else(|| b.ins().f64const(0.0));
            b.ins().store(MemFlags::from_u32(0), gradient, grad_ptr, offset(i)?);
        }
        b.ins().return_(&[]);
        b.finalize(frontend_config);
    }

    module.define_function(function_id, &mut context).map_err(jit_err)?;
    module.clear_context(&mut context);
    module.finalize_definitions().map_err(jit_err)?;
    let ptr = module.get_finalized_function(function_id);
    // SAFETY: Cranelift defined the exact three-pointer platform C ABI above.
    // The owning JITModule stays alive in KernelOwner for every function call.
    let function: RawKernel = unsafe { std::mem::transmute(ptr) };
    Ok(CompiledKernel {
        owner: Arc::new(KernelOwner {
            _module: Mutex::new(module),
            function,
        }),
        input_count: graph.inputs,
    })
}
