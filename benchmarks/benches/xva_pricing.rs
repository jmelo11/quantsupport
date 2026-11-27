//! Full XVA pricing with prepared execution and refresh of the current market.

use std::{hint::black_box, time::Duration};

use benchmarks::xvapricing::XvaPricingFixture;
use criterion::{
    criterion_group, criterion_main, BenchmarkId, Criterion, SamplingMode, Throughput,
};

/// Measures complete repeated pricing after the engine's explicit preparation.
fn prepared_pricing(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("xva_pricing");
    group
        .sample_size(10)
        .sampling_mode(SamplingMode::Flat)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3));
    for paths in [16, 1_000, 10_000] {
        group.throughput(Throughput::Elements(paths as u64));
        for (mode, margin) in [("margined", true), ("unmargined", false)] {
            let mut fixture = XvaPricingFixture::new(paths, margin).expect("XVA fixture");
            fixture.prepare().expect("native XVA preparation");
            black_box(fixture.run().expect("initial complete XVA run"));
            group.bench_function(BenchmarkId::new(mode, paths), |bencher| {
                bencher.iter(|| black_box(fixture.run().expect("complete XVA pricing")));
            });
        }
    }
    group.finish();
}

/// Measures changing FX data, market rebinding and complete pricing on retained native code.
fn market_refresh(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("xva_market_refresh");
    group
        .sample_size(10)
        .sampling_mode(SamplingMode::Flat)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3));
    for paths in [1_000, 10_000] {
        group.throughput(Throughput::Elements(paths as u64));
        let mut fixture = XvaPricingFixture::new(paths, true).expect("XVA fixture");
        fixture.prepare().expect("native XVA preparation");
        black_box(fixture.reprice_fx(1.01).expect("initial market refresh"));
        assert!(
            fixture.preparation_reused_code(),
            "FX refresh must retain native code"
        );
        let mut up = false;
        group.bench_function(BenchmarkId::new("margined", paths), |bencher| {
            bencher.iter(|| {
                let multiplier = if up { 1.01 } else { 0.99 };
                up = !up;
                black_box(
                    fixture
                        .reprice_fx(multiplier)
                        .expect("fresh-market XVA pricing"),
                )
            });
        });
    }
    group.finish();
}

criterion_group!(benches, prepared_pricing, market_refresh);
criterion_main!(benches);
