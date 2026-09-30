//! Benchmarks of plan 14's planetary stage (P14.T33.a). The targets are the brainstorm's: a full
//! system with its bodies in under a millisecond, measured as the mean of (ii) per system with plan
//! 06's and plan 11's stars already built (the target is this stage's), and `position_at` well
//! under a microsecond. A miss is a finding, not a CI failure: CI compiles these and never runs
//! them.
//!
//! - (i) `solar_like`: `generate` plus `snapshot_at` at the epoch for the Solar-like golden system
//!   of `tests/planetary_golden.rs` (`0x4200_6cba_0000_0009` in its fixture), its context built
//!   beforehand.
//! - (ii) `field_1000`: the same for the 1,000 field systems nearest the Sun-like point
//!   ([`sample_contexts`] with [`SampleFilter::ALL`]: the galaxy's own mixture of masses,
//!   multiplicities, populations and ages, binaries and evolved hosts included), their contexts,
//!   and so their stars, built beforehand. The figure per system is the mean, a thousandth of it.
//! - (iii) 1,000 rogue planets: waits for P14.T27.b, which generates a rogue planet's system; not
//!   built.
//! - (iv) `moon_position_at`: `position_at` of the Solar-like golden's first moon at the epoch.
//!
//! Measured on 2026-09-30 by `just bench planetary` under the heavy-test lock, in the `spin14b`
//! lane, at generator version 16, on the development machine (an i7-8665U, 8 threads, 1.9 GHz
//! base and 4.8 GHz boost; mean clock 2.2–2.8 GHz across the run) at a load average of 9–14 with
//! other lanes' builds and tests running, so each figure is an upper bound:
//!
//! | Bench | Measured | Target |
//! | ----- | -------- | ------ |
//! | (i) `solar_like`, 24 bodies | 1.75 ms (1.67–1.82) | under 1 ms: missed |
//! | (ii) `field_1000` | 1.21 s (1.19–1.25), 1.21 ms per system | under 1 ms per system: missed |
//! | (iv) `moon_position_at` | 7.04 µs (6.86–7.30) | well under 1 µs: missed |
//!
//! The misses are findings, provisional with their ruling deferred (plan 14's T33.a as-built
//! note). `position_at` builds the system's epoch (its stars' states at the time) and the fate
//! of the moon's primary on every call, which a caller asking for many bodies at one time
//! would share.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use hyperion_sim::Seed;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::id::SystemId;
use hyperion_sim::planetary::record::BodyKind;
use hyperion_sim::planetary::testing::{SampleFilter, sample_contexts};
use hyperion_sim::planetary::{self, SystemContext};
use hyperion_sim::time::UniverseTime;

/// The seed of `tests/planetary_golden.rs`'s fixture, which the golden systems live in.
const GOLDEN_SEED: u64 = 0x5eed_0000_0014_0032;

/// The Solar-like golden system's ID (`tests/planetary_golden.rs`, `solar_like`).
const SOLAR_LIKE: u64 = 0x4200_6cba_0000_0009;

/// How many field systems (ii) generates.
const FIELD_SYSTEMS: usize = 1_000;

/// `generate` plus `snapshot_at` at the epoch, what a request for a system costs this stage.
fn generate_and_snapshot(seed: Seed, context: &SystemContext) -> usize {
    let system = planetary::generate(seed, context);
    system
        .snapshot_at(context, UniverseTime::EPOCH)
        .bodies()
        .len()
}

fn planetary_stage(c: &mut Criterion) {
    let seed = Seed::new(GOLDEN_SEED);
    let galaxy = Galaxy::from_params(seed, GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral");
    let id = SystemId::from_raw(SOLAR_LIKE).expect("a pinned ID is well formed");
    let solar = SystemContext::for_system(&galaxy, id).expect("a pinned ID names a system");
    let field = sample_contexts(FIELD_SYSTEMS, seed, SampleFilter::ALL)
        .expect("the solar circle holds a thousand systems");

    let mut group = c.benchmark_group("planetary");
    group.bench_function("solar_like", |b| {
        b.iter(|| generate_and_snapshot(seed, black_box(&solar)));
    });
    group.sample_size(10);
    group.bench_function("field_1000", |b| {
        b.iter(|| {
            black_box(&field)
                .iter()
                .map(|context| generate_and_snapshot(seed, context))
                .sum::<usize>()
        });
    });
    // (iii), 1,000 rogue planets, waits for P14.T27.b.
    group.sample_size(100);
    let system = planetary::generate(seed, &solar);
    let moon = system
        .bodies()
        .iter()
        .find(|body| matches!(body.kind(), BodyKind::Moon(_)))
        .expect("the Solar-like golden has moons")
        .index();
    group.bench_function("moon_position_at", |b| {
        b.iter(|| {
            system
                .position_at(black_box(&solar), black_box(moon), UniverseTime::EPOCH)
                .expect("the moon is a body of its system")
        });
    });
    group.finish();
}

criterion_group!(benches, planetary_stage);
criterion_main!(benches);
