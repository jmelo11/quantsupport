//! Complete swap exposure evaluation with fixed portfolio, grid and path counts.

use benchmarks::pfe_fixture::SwapPfeFixture;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, SamplingMode};
use pprof::criterion::{Output, PProfProfiler};
use std::hint::black_box;

/// Measures the full PFE pipeline with and without market-request expiration.
fn bench_swap_pfe(c: &mut Criterion) {
    let mut group = c.benchmark_group("swap_pfe");
    group.sample_size(10).sampling_mode(SamplingMode::Flat);
    for n in [100, 1000] {
        let fixture = SwapPfeFixture::new(n, 500);
        for (mode, expire_requests) in [("all_requests", false), ("expiring_requests", true)] {
            group.bench_with_input(BenchmarkId::new(mode, n), &fixture, |b, fixture| {
                b.iter(|| black_box(fixture.run(expire_requests).expect("PFE evaluation")));
            });
        }
    }
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default().with_profiler(PProfProfiler::new(100, Output::Protobuf));
    targets = bench_swap_pfe
}
criterion_main!(benches);
