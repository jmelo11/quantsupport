//! Compiled AD benchmarks with recording/compilation separated from execution.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use quantsupport::{
    ad::{DerivativeOrder, Graph},
    models::utils::black_call_scalar,
};
use std::hint::black_box;

/// Measures native first-gradient execution across representative scalar operations.
fn native_gradients(c: &mut Criterion) {
    let mut group = c.benchmark_group("compiled_gradient");
    for count in [16_usize, 128, 1024] {
        let inputs = vec![1.0; count];
        let graph = Graph::trace(&inputs, |x| {
            x.iter().fold(x[0] * 0.0, |sum, &value| sum + value * value)
        })
        .unwrap();
        let kernel = graph.compile().unwrap();
        let mut gradient = vec![0.0; count];
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::new("reuse", count), &inputs, |b, x| {
            b.iter(|| black_box(kernel.value_and_grad(black_box(x), &mut gradient).unwrap()));
        });
    }
    group.finish();
}

/// Measures native compilation and explicitly requested Hessian execution.
fn compilation_and_second_order(c: &mut Criterion) {
    let inputs = [100.0, 0.2, 0.03];
    let graph = Graph::try_trace(&inputs, |x| {
        black_call_scalar(x[0], 100.0, x[1], 1.0).map(|value| value * (-x[2]).exp())
    })
    .unwrap();
    c.bench_function("compile_first_order", |b| {
        b.iter(|| black_box(graph.compile().unwrap()))
    });
    let kernel = graph.compile_with_order(DerivativeOrder::Second).unwrap();
    let mut gradient = [0.0; 3];
    let mut hessian = [0.0; 9];
    c.bench_function("native_gradient_and_hessian", |b| {
        b.iter(|| {
            black_box(
                kernel
                    .value_grad_hessian(black_box(&inputs), &mut gradient, &mut hessian)
                    .unwrap(),
            )
        })
    });
}

criterion_group!(benches, native_gradients, compilation_and_second_order);
criterion_main!(benches);
