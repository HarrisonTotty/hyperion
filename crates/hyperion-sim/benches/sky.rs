//! Benchmarks of the sky (rendering plan R06): the luminosity tables' build, and later the census
//! and the band.
//!
//! They run on `GalaxyParams::milky_way_like()` with a fixed seed. A miss is a finding to record,
//! not a CI failure: CI compiles these and never runs them. Every figure below is provisional
//! until taken on a quiet machine (the roadmap's "Measurements on a quiet machine").
//!
//! | Bench | Figure | Conditions |
//! | ----- | ------ | ---------- |
//! | `sky/luminosity_tables` | pending | not yet run on a quiet machine |

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use hyperion_sim::Seed;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::sky::luminosity::LuminosityTables;

fn luminosity_tables(c: &mut Criterion) {
    let galaxy = Galaxy::from_params(Seed::new(0x0926_0000), GalaxyParams::milky_way_like())
        .expect("the Milky Way-like parameters are valid");
    let mut group = c.benchmark_group("sky");
    group.sample_size(10);
    group.bench_function("luminosity_tables", |b| {
        b.iter(|| LuminosityTables::build(black_box(&galaxy)));
    });
    group.finish();
}

criterion_group!(sky, luminosity_tables);
criterion_main!(sky);
