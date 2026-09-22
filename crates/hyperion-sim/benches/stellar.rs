//! Benchmarks of the stellar stage (plan 06).
//!
//! Targets are recorded in plan 06's "Verification": `ZCoeffs::new` under 5 µs. A regression is a
//! finding to raise, not a CI failure: CI compiles these and never runs them.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use hyperion_sim::stellar::sse::{ZCoeffs, zams};
use hyperion_sim::units::{MetalFraction, SolarMasses};

/// Every metallicity-dependent coefficient at one Z, and the zero-age main sequence from them.
fn backbone(c: &mut Criterion) {
    let mut group = c.benchmark_group("stellar");
    group.bench_function("zcoeffs_new", |b| {
        b.iter(|| ZCoeffs::new(black_box(MetalFraction::new(0.004))));
    });
    let coeffs = ZCoeffs::new(MetalFraction::new(0.004));
    group.bench_function("zams_luminosity_and_radius", |b| {
        b.iter(|| {
            let m = black_box(SolarMasses::new(3.7));
            (zams::luminosity(m, &coeffs), zams::radius(m, &coeffs))
        });
    });
    group.finish();
}

criterion_group!(stellar, backbone);
criterion_main!(stellar);
