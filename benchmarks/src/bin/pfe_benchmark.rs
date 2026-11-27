//! Complete swap PFE timings and cube fingerprints for separate branch binaries.

use benchmarks::pfe_fixture::{SwapPfeFixture, SwapPfeRun};
use serde_json::{json, Value};
use std::{
    collections::hash_map::DefaultHasher,
    env::args,
    error::Error,
    fs,
    hash::{Hash, Hasher},
    hint::black_box,
    result::Result as StdResult,
    time::Instant,
};

/// Fingerprints every labelled cube value and retains complete first-path rows.
fn snapshot(run: &SwapPfeRun) -> StdResult<Value, Box<dyn Error>> {
    let mut fingerprint = DefaultHasher::new();
    let mut entries = 0_u64;
    let mut cubes = Vec::new();
    for cube in &run.result().cubes {
        cube.trade_id.hash(&mut fingerprint);
        cube.dates.hash(&mut fingerprint);
        cube.npvs.len().hash(&mut fingerprint);
        let mut total = 0.0;
        for path in &cube.npvs {
            path.len().hash(&mut fingerprint);
            for &value in path {
                if !value.is_finite() {
                    return Err("nonfinite PFE cube value".into());
                }
                value.to_bits().hash(&mut fingerprint);
                total += value;
                entries += 1;
            }
        }
        cubes.push(json!({
            "trade": cube.trade_id, "paths": cube.npvs.len(),
            "dates": cube.dates.len(), "sum": total, "first_path": cube.npvs.first(),
        }));
    }
    Ok(
        json!({"entries": entries, "fingerprint": format!("{:016x}", fingerprint.finish()), "cubes": cubes}),
    )
}

/// Measures the unchanged full workload, keeping cube serialization outside timing.
fn main() -> StdResult<(), Box<dyn Error>> {
    let arguments: Vec<_> = args().skip(1).collect();
    if arguments.len() != 5 {
        return Err(
            "usage: <trades> <paths> <expire requests> <warm samples> <output.json>".into(),
        );
    }
    let trades: usize = arguments[0].parse()?;
    let paths: usize = arguments[1].parse()?;
    let expire_requests: bool = arguments[2].parse()?;
    let samples: usize = arguments[3].parse()?;
    if trades == 0 || paths == 0 {
        return Err("trade and path counts must be positive".into());
    }
    let setup = Instant::now();
    let fixture = SwapPfeFixture::new(trades, paths);
    let setup_ms = setup.elapsed().as_secs_f64() * 1e3;
    let start = Instant::now();
    let first = fixture.run(expire_requests)?;
    let first_ms = start.elapsed().as_secs_f64() * 1e3;
    let claims = first.claims();
    let requests = first.requests();
    let pipeline_setup_ms = first.setup_ms();
    let evaluation_ms = first.evaluation_ms();
    let snapshot = snapshot(&first)?;
    drop(first);
    let mut times = Vec::new();
    for _ in 0..samples {
        let start = Instant::now();
        black_box(fixture.run(expire_requests)?);
        times.push(start.elapsed().as_secs_f64() * 1e3);
    }
    times.sort_by(f64::total_cmp);
    fs::write(
        &arguments[4],
        serde_json::to_vec(&json!({
            "trades":trades, "paths":paths, "expire_requests":expire_requests, "samples":samples,
            "setup_ms":setup_ms, "first_ms":first_ms, "pipeline_setup_ms":pipeline_setup_ms,
            "evaluation_ms":evaluation_ms, "median_ms":times.get(times.len()/2),
            "claims":claims, "requests":requests, "snapshot":snapshot,
        }))?,
    )?;
    println!("trades={trades}; paths={paths}; claims={claims}; requests={requests}; first={first_ms:.3} ms");
    Ok(())
}
