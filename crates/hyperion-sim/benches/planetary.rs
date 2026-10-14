//! Benchmarks of plan 14's planetary stage (P14.T33.a).
//!
//! The targets: a full system with its bodies in under a millisecond, the brainstorm's ("Runtime
//! and code shape"), measured as the mean of (ii) per system with plan 06's and plan 11's stars
//! already built (the target is this stage's), and `position_at` well under a microsecond, plan
//! 14's (P14.T33.a). A miss is a finding, not a CI failure: CI compiles these and never runs them.
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
//! - (v) `body_detail_surface_1000`: `body_at` at the epoch degraded to
//!   [`DetailLevel::Surface`], what the server's `body_detail` computes at `surface`
//!   (`hyperion-server`'s `convert/planetary.rs`), for 1,000 bodies of (ii)'s field systems: every
//!   index those systems hold a record for (planets, moons, rings, belts, belts' members and
//!   haloes, as they come), spread evenly over them ([`sample_bodies`]). Their systems are
//!   generated beforehand, as the server holds them. The figure per body is a thousandth of it.
//!   Of the 18,020 indices, the 1,000 are 522 dwarf planets, 207 planets, 138 moons, 110 belts,
//!   21 rings and 2 haloes, from 716 systems, so the dwarf planets weigh most in the mean.
//! - (vi) `system_bodies_surface`: `snapshot_at` at the epoch degraded to `Surface`, what the
//!   server's `system_bodies` computes at `surface`, for the Solar-like golden, generated
//!   beforehand.
//!
//! Measured on 2026-10-14 by `just bench --bench planetary` under the heavy-test lock, in the
//! `lane/13-p14-t33-port` lane, at generator version 22, on the development machine (an i7-8665U,
//! 8 threads, 1.9 GHz base and 4.8 GHz boost) at a load average of 0.8–1.5, no other lane
//! running; provisional, as every figure from the shared machine is:
//!
//! | Bench | Measured | Target |
//! | ----- | -------- | ------ |
//! | (i) `solar_like`, 24 bodies, 20 surfaces | 6.27 ms (6.24–6.30) | under 1 ms: missed |
//! | (ii) `field_1000` | 3.47 s (3.45–3.50), 3.47 ms per system | under 1 ms per system: missed |
//! | (iv) `moon_position_at` | 4.44 µs (4.43–4.45) | well under 1 µs: missed |
//! | (v) `body_detail_surface_1000` | 173 ms (172–173), 173 µs per body | none |
//! | (vi) `system_bodies_surface` | 5.06 ms (5.05–5.06) | none |
//!
//! An earlier run that afternoon, at a load of 0.6–1.4 and with (v) taking its bodies at a
//! stride that left out the list's last few percent, read 6.52 ms, 3.57 s, 4.42 µs, 171 ms and
//! 5.07 ms.
//!
//! The misses are findings, provisional with their ruling deferred (plan 14's Risks, "Deferred
//! findings (2026-10-13 rule), from P14.T33.a–b"). (i) and (ii) are 3.6 and 2.9 times version
//! 16's (1.75 ms and 1.21 ms per system, measured 2026-09-30 at a load of 9–14 in the `spin14b`
//! lane): mostly the surface section since P14.T48.e, partly P14.T46.b's rotation pass, which
//! took (ii) from 682 µs to 1,098 µs a system (plan 14's T46.b record, under a load of 35–55).
//! (vi) puts (i)'s snapshot at no more than 5.06 ms of its 6.27, `degrade`'s copies included.
//! (iv) was 7.04 µs at version 16, under that load. `position_at` builds the system's epoch (its
//! stars' states at the time) and the fate of the moon's primary on every call, which a caller
//! asking for many bodies at one time would share. (v) and (vi) time whole records, not the
//! surface section alone, so they do not measure decision-composition §1.3's estimate of 10–25
//! µs a section.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use hyperion_sim::Seed;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::id::SystemId;
use hyperion_sim::planetary::record::{BodyKind, DetailLevel};
use hyperion_sim::planetary::testing::{SampleFilter, sample_contexts};
use hyperion_sim::planetary::{self, BodyIndex, PlanetarySystem, SystemContext};
use hyperion_sim::time::UniverseTime;

/// The seed of `tests/planetary_golden.rs`'s fixture, which the golden systems live in.
const GOLDEN_SEED: u64 = 0x5eed_0000_0014_0032;

/// The Solar-like golden system's ID (`tests/planetary_golden.rs`, `solar_like`).
const SOLAR_LIKE: u64 = 0x4200_6cba_0000_0009;

/// How many field systems (ii) generates, and (v) takes its bodies from.
const FIELD_SYSTEMS: usize = 1_000;

/// How many bodies (v) resolves.
const SAMPLE_BODIES: usize = 1_000;

/// `generate` plus `snapshot_at` at the epoch, what a request for a system costs this stage.
fn generate_and_snapshot(seed: Seed, context: &SystemContext) -> usize {
    let system = planetary::generate(seed, context);
    system
        .snapshot_at(context, UniverseTime::EPOCH)
        .bodies()
        .len()
}

/// (v)'s bodies: every index the field systems hold a record for, system by system in the
/// sample's order and in index order within one, the `k`-th of the 1,000 taken at `k` thousandths
/// of the list, so that they spread over the whole sample. Each is the position of its system in
/// `systems` and the body's index.
fn sample_bodies(systems: &[PlanetarySystem]) -> Vec<(usize, BodyIndex)> {
    let every: Vec<(usize, BodyIndex)> = systems
        .iter()
        .enumerate()
        .flat_map(|(k, system)| system.indices().into_iter().map(move |index| (k, index)))
        .collect();
    assert!(
        every.len() >= SAMPLE_BODIES,
        "the field systems hold {} bodies, fewer than {SAMPLE_BODIES}",
        every.len()
    );
    (0..SAMPLE_BODIES)
        .map(|k| every[k * every.len() / SAMPLE_BODIES])
        .collect()
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
    // (v) and (vi), the server's `body_detail` and `system_bodies` at `surface`
    // (`hyperion-server`'s `convert/planetary.rs`): each request reads a system the server holds
    // generated, so the systems are generated beforehand.
    let systems: Vec<PlanetarySystem> = field
        .iter()
        .map(|context| planetary::generate(seed, context))
        .collect();
    let bodies = sample_bodies(&systems);
    group.sample_size(10);
    group.bench_function("body_detail_surface_1000", |b| {
        b.iter(|| {
            for &(k, index) in black_box(&bodies) {
                let record = systems[k]
                    .body_at(&field[k], index, UniverseTime::EPOCH)
                    .expect("an index the system lists names a body");
                black_box(record.degrade(DetailLevel::Surface));
            }
        });
    });
    group.sample_size(100);
    group.bench_function("system_bodies_surface", |b| {
        b.iter(|| {
            system
                .snapshot_at(black_box(&solar), UniverseTime::EPOCH)
                .degrade(DetailLevel::Surface)
        });
    });
    group.finish();
}

criterion_group!(benches, planetary_stage);
criterion_main!(benches);
