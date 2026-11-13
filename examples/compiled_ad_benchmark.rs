//! Benchmarks QS's existing DualFwd tape against the experimental Cranelift
//! value-and-gradient kernel, serially and with Rayon's worker pool.
//!
//! This is a narrow numerical benchmark, NOT a full QS pricer/XVA benchmark.
//! Run: cargo run --release --features jit --example compiled_ad_benchmark

#[cfg(feature = "jit")]
fn inputs(path: usize) -> [f64; 2] {
    [
        1.0 + (path % 1000) as f64 * 0.001,
        0.3 + (path % 100) as f64 * 0.005,
    ]
}

#[cfg(feature = "jit")]
fn tape_one(path: usize) -> quantsupport::utils::errors::Result<[f64; 3]> {
    use quantsupport::ad::{dual::DualFwd, tape::Tape};
    Tape::start_recording_fwd();
    let [vx, vy] = inputs(path);
    let x = DualFwd::new(vx);
    let y = DualFwd::new(vy);
    let output: DualFwd = (x * x + x * y + y.sin()).into();
    output.backward()?;
    let result = [
        output.value(),
        x.adjoint()?.value(),
        y.adjoint()?.value(),
    ];
    Tape::stop_recording_fwd();
    Ok(result)
}

#[cfg(feature = "jit")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::{hint::black_box, time::Instant};
    use rayon::prelude::*;
    use quantsupport::ad::compiled::Graph;

    const COUNT: usize = 100_000;
    let compile_start = Instant::now();
    let graph = Graph::trace(&[1.0, 0.3], |xs| {
        xs[0] * xs[0] + xs[0] * xs[1] + xs[1].sin()
    })?;
    let kernel = graph.compile()?;
    let compile_time = compile_start.elapsed();

    // Warm the JIT code and the existing tape implementation.
    let mut warm_grad = [0.0; 2];
    for i in 0..100 {
        let args = inputs(i);
        black_box(kernel.value_and_grad(&args, &mut warm_grad)?);
        black_box(tape_one(i)?);
    }

    let start = Instant::now();
    let mut tape_seq = [0.0; 3];
    for i in 0..COUNT {
        let value = tape_one(i)?;
        for (sum, next) in tape_seq.iter_mut().zip(value) {
            *sum += next;
        }
    }
    let tape_serial_time = start.elapsed();
    black_box(&tape_seq);

    let start = Instant::now();
    let mut jit_seq = [0.0; 3];
    let mut grads = [0.0; 2];
    for i in 0..COUNT {
        let args = inputs(i);
        let val = kernel.value_and_grad(&args, &mut grads)?;
        jit_seq[0] += val;
        jit_seq[1] += grads[0];
        jit_seq[2] += grads[1];
    }
    let jit_serial_time = start.elapsed();
    black_box(&jit_seq);

    let start = Instant::now();
    let tape_par = (0..COUNT)
        .into_par_iter()
        .try_fold(
            || [0.0; 3],
            |mut total, i| {
                let contribution = tape_one(i)?;
                for (dst, value) in total.iter_mut().zip(contribution) {
                    *dst += value;
                }
                Ok::<_, quantsupport::utils::errors::QSError>(total)
            },
        )
        .try_reduce(
            || [0.0; 3],
            |mut a, b| {
                for (dst, value) in a.iter_mut().zip(b) {
                    *dst += value;
                }
                Ok(a)
            },
        )?;
    let tape_parallel_time = start.elapsed();
    black_box(&tape_par);

    let start = Instant::now();
    let jit_par = kernel.sum_paths(COUNT, |i, values| {
        values.copy_from_slice(&inputs(i));
        Ok(())
    })?;
    let jit_parallel_time = start.elapsed();
    black_box(&jit_par);

    for i in 0..3 {
        let tol = 1.0e-9 * tape_seq[i].abs().max(1.0);
        assert!((jit_seq[i] - tape_seq[i]).abs() < tol,
            "compiled result differs from QS tape at component {i}");
    }

    println!("QuantSupport scalar value+gradient benchmark");
    println!("Workload: f(x,y)=x*x+x*y+sin(y); paths={COUNT}");
    println!("Threads: {}", rayon::current_num_threads());
    println!("Tracing + Cranelift compilation: {:.3} ms", compile_time.as_secs_f64() * 1e3);
    println!("Tape serial: {:.3} ms", tape_serial_time.as_secs_f64() * 1e3);
    println!("JIT serial: {:.3} ms", jit_serial_time.as_secs_f64() * 1e3);
    println!("Tape parallel: {:.3} ms", tape_parallel_time.as_secs_f64() * 1e3);
    println!("JIT parallel: {:.3} ms", jit_parallel_time.as_secs_f64() * 1e3);
    println!("Serial warm speedup: {:.2}x", tape_serial_time.as_secs_f64() / jit_serial_time.as_secs_f64());
    println!("Parallel warm speedup: {:.2}x", tape_parallel_time.as_secs_f64() / jit_parallel_time.as_secs_f64());
    println!("JIT parallel vs tape serial (combined): {:.2}x", tape_serial_time.as_secs_f64() / jit_parallel_time.as_secs_f64());
    Ok(())
}

#[cfg(not(feature = "jit"))]
fn main() {
    eprintln!("cargo run --release --features jit --example compiled_ad_benchmark");
}
