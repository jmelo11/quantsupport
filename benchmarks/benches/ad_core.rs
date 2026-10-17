//! Micro-benchmarks for the AD library, designed to isolate suspected
//! bottlenecks:
//!
//! 1. `ad_record`   — tape recording throughput compared against a plain
//!    `f64` baseline. Nodes store child pointers and derivatives inline.
//! 2. `ad_backward` — adjoint reset and backward propagation on a prebuilt
//!    tape. Root lookup uses the node's stored index, not a linear scan.
//! 3. `ad_full_grad` — realistic record + backward + adjoint readout for a
//!    dot-product style workload with many independent leaves.
//! 4. `ad_second_order` — mixed-mode (`Dual<ADForward>`) Black-Scholes pricing,
//!    the realistic 2nd-order workload.
//!
//! Scaling across chain/workload sizes (1k/10k/100k iterations) reveals whether cost is
//! flat (O(n) total) or grows (accidental O(n^2)).

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use pprof::criterion::{Output, PProfProfiler};
use quantsupport::ad::dual::{Dual, DualFwd};
use quantsupport::ad::expr::{exp, log, sqrt};
use quantsupport::ad::forward::ADForward;
use quantsupport::ad::tape::Tape;

const SIZES: &[usize] = &[1_000, 10_000, 100_000];

// ───────────────────────────────────────────────────────────────────────────
// Workloads
// ───────────────────────────────────────────────────────────────────────────

/// Plain f64 chain: y <- y * 0.5 + 1.0 (bounded, converges to 2).
#[inline(never)]
fn f64_chain(n: usize, x0: f64) -> f64 {
    let mut y = x0;
    for _ in 0..n {
        y = black_box(y) * 0.5 + 1.0;
    }
    y
}

/// Same chain with `Dual<f64>`; each iteration flattens to one tape node.
fn dual_chain(n: usize, x0: f64) -> Dual<f64> {
    let mut y = Dual::<f64>::new(x0);
    for _ in 0..n {
        y = (y * 0.5 + 1.0).into();
    }
    y
}

/// Same chain with `DualFwd` (mixed backward + forward 2nd order).
fn dualfwd_chain(n: usize, x0: f64) -> DualFwd {
    let mut y = DualFwd::new_from_inner(ADForward::var(x0));
    for _ in 0..n {
        y = (y * 0.5 + 1.0).into();
    }
    y
}

/// Dot-product style accumulation: n independent leaves, each step records a
/// node with two live children (sum, x[i]) — the common pricing pattern.
fn dual_dot(xs: &[Dual<f64>]) -> Dual<f64> {
    let mut sum = Dual::<f64>::zero();
    for (i, &x) in xs.iter().enumerate() {
        let w = 1.0 + (i % 7) as f64 * 0.1;
        sum = (sum + x * w).into();
    }
    sum
}

/// Logistic approximation of the standard normal CDF (keeps the workload
/// self-contained; accuracy is irrelevant for benchmarking).
fn norm_cdf(x: DualFwd) -> DualFwd {
    let e: DualFwd = exp(-x * 1.702).into();
    (DualFwd::one() / (e + 1.0)).into()
}

/// Black-Scholes call with 2nd-order AD (spot seeded forward => gamma).
fn black_scholes(s: DualFwd, sigma: DualFwd, t: DualFwd, k: f64, r: f64) -> DualFwd {
    let sqrt_t: DualFwd = sqrt(t).into();
    let sig_sqrt_t: DualFwd = (sigma * sqrt_t).into();
    let d1: DualFwd = ((log(s / k) + (sigma * sigma * 0.5 + r) * t) / sig_sqrt_t).into();
    let d2: DualFwd = (d1 - sig_sqrt_t).into();
    let disc: DualFwd = exp(-t * r).into();
    (s * norm_cdf(d1) - disc * norm_cdf(d2) * k).into()
}

// ───────────────────────────────────────────────────────────────────────────
// 1. Recording throughput
// ───────────────────────────────────────────────────────────────────────────

fn bench_record(c: &mut Criterion) {
    let mut g = c.benchmark_group("ad_record");
    for &n in SIZES {
        g.throughput(Throughput::Elements(n as u64));

        g.bench_with_input(BenchmarkId::new("f64_baseline", n), &n, |b, &n| {
            b.iter(|| f64_chain(n, black_box(0.2)));
        });

        g.bench_with_input(BenchmarkId::new("dual_f64", n), &n, |b, &n| {
            b.iter(|| {
                Tape::start_recording();
                let y = dual_chain(n, black_box(0.2));
                Tape::stop_recording();
                y.value()
            });
        });

        g.bench_with_input(BenchmarkId::new("dual_fwd", n), &n, |b, &n| {
            b.iter(|| {
                Tape::start_recording_fwd();
                let y = dualfwd_chain(n, black_box(0.2));
                Tape::stop_recording_fwd();
                y.value()
            });
        });
    }
    g.finish();
}

// ───────────────────────────────────────────────────────────────────────────
// 2. Backward-pass throughput (tape prebuilt outside the timed loop)
// ───────────────────────────────────────────────────────────────────────────

fn bench_backward(c: &mut Criterion) {
    let mut g = c.benchmark_group("ad_backward");
    for &n in SIZES {
        g.throughput(Throughput::Elements(n as u64));

        // Prebuild the tape once; each iteration resets adjoints + propagates.
        Tape::start_recording();
        let x = Dual::<f64>::new(0.2);
        let mut y = x;
        for _ in 0..n {
            y = (y * 0.5 + 1.0).into();
        }
        Tape::stop_recording();

        g.bench_with_input(BenchmarkId::new("dual_f64", n), &n, |b, _| {
            b.iter(|| {
                Tape::reset_adjoints();
                y.backward().unwrap();
                black_box(x.adjoint().unwrap())
            });
        });

        // Adjoint reset alone, to subtract from the number above.
        g.bench_with_input(BenchmarkId::new("reset_adjoints_only", n), &n, |b, _| {
            b.iter(Tape::reset_adjoints);
        });

        Tape::rewind_to_init();
    }
    g.finish();
}

// ───────────────────────────────────────────────────────────────────────────
// 3. Full gradient: record + backward + adjoint readout (many leaves)
// ───────────────────────────────────────────────────────────────────────────

fn bench_full_grad(c: &mut Criterion) {
    let mut g = c.benchmark_group("ad_full_grad");
    for &n in SIZES {
        g.throughput(Throughput::Elements(n as u64));

        g.bench_with_input(BenchmarkId::new("dot_product", n), &n, |b, &n| {
            b.iter(|| {
                Tape::start_recording();
                let xs: Vec<Dual<f64>> = (0..n).map(|i| Dual::new(1.0 + i as f64 * 1e-6)).collect();
                let sum = dual_dot(&xs);
                Tape::stop_recording();
                sum.backward().unwrap();
                let g0 = xs[0].adjoint().unwrap();
                let gl = xs[n - 1].adjoint().unwrap();
                black_box((sum.value(), g0, gl))
            });
        });
    }
    g.finish();
}

// ───────────────────────────────────────────────────────────────────────────
// 4. Second-order (mixed-mode) realistic pricing workload
// ───────────────────────────────────────────────────────────────────────────

fn bench_second_order(c: &mut Criterion) {
    let mut g = c.benchmark_group("ad_second_order");
    // Number of Black-Scholes pricings per iteration.
    for &reps in &[100usize, 1_000] {
        g.throughput(Throughput::Elements(reps as u64));

        g.bench_with_input(
            BenchmarkId::new("black_scholes_gamma", reps),
            &reps,
            |b, &reps| {
                b.iter(|| {
                    Tape::start_recording_fwd();
                    let mut acc = 0.0;
                    for i in 0..reps {
                        let s = DualFwd::new_from_inner(ADForward::var(100.0 + i as f64 * 0.01));
                        let sigma = DualFwd::new(0.2);
                        let t = DualFwd::new(1.0);
                        let price = black_scholes(s, sigma, t, 100.0, 0.03);
                        price.backward().unwrap();
                        let adj = s.adjoint().unwrap();
                        // delta (backward) + gamma (forward-over-backward)
                        acc += price.value() + adj.first_derivative() + adj.second_derivative();
                        Tape::reset_adjoints_fwd();
                    }
                    Tape::stop_recording_fwd();
                    black_box(acc)
                });
            },
        );
    }
    g.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default().with_profiler(PProfProfiler::new(100, Output::Protobuf));
    targets = bench_record, bench_backward, bench_full_grad, bench_second_order
}
criterion_main!(benches);
