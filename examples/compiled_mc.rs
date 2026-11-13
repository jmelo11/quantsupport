//! Cranelift-compiled, Rayon-parallel Monte Carlo value and pathwise Greeks.
//!
//! Run with: cargo run --release --features jit --example compiled_mc
//!
//! Each path is indexed deterministically. The same machine code is shared
//! by all Rayon tasks and no per-path reverse-mode tape is recorded.

#[cfg(feature = "jit")]
fn normal_for_path(index: usize) -> f64 {
    // Counter-based SplitMix64 uniforms, independent of Rayon task order.
    fn uniform(mut x: u64) -> f64 {
        x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
        x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        x ^= x >> 31;
        ((x >> 11) as f64 + 0.5) / ((1_u64 << 53) as f64)
    }
    let a = uniform((index as u64).wrapping_mul(2));
    let b = uniform((index as u64).wrapping_mul(2).wrapping_add(1));
    (-2.0 * a.ln()).sqrt() * (std::f64::consts::TAU * b).cos()
}

#[cfg(feature = "jit")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use quantsupport::ad::{compiled::{Graph, Recorded}, scalar::Scalar};

    // Inputs: spot, volatility, risk-free rate, strike, standard-normal draw.
    // Model constants and numerical operations are captured once.
    let graph = Graph::trace(&[100.0, 0.2, 0.03, 100.0, 0.0], |x| {
        let drift = x[2] - x[1] * x[1] * 0.5;
        let diffusion = x[1] * x[4];
        let terminal = x[0] * (drift + diffusion).exp();
        let payoff = (terminal - x[3]).max_val(Recorded::from(0.0));
        payoff * (-x[2]).exp()
    })?;
    let kernel = graph.compile()?;
    let n_paths = 200_000;
    let totals = kernel.sum_paths(n_paths, |index, inputs| {
        inputs.copy_from_slice(&[
            100.0, 0.2, 0.03, 100.0, normal_for_path(index),
        ]);
        Ok(())
    })?;

    println!("Paths: {}", totals.count);
    println!("Mean option value: {:.8}", totals.mean().unwrap_or(0.0));
    println!("Pathwise delta: {:.8}", totals.gradients[0] / n_paths as f64);
    println!("Pathwise vega:  {:.8}", totals.gradients[1] / n_paths as f64);
    println!("Pathwise rho:   {:.8}", totals.gradients[2] / n_paths as f64);
    Ok(())
}

#[cfg(not(feature = "jit"))]
fn main() {
    eprintln!("Enable the optional compiler: cargo run --release --features jit --example compiled_mc");
}
