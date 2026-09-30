//! Benchmarks of plan 12's retarded observation and microlensing (P12.T1, P12.T4.b; ruling 143).
//! Targets are the ruling's; a miss is a finding, not a CI failure: CI compiles these and never
//! runs them.
//!
//! - `retarded`: one fixed-point step for a drifting source, ≤ 0.7 µs at low load (ruling 143.4;
//!   the plan's "tens of nanoseconds" is not reachable with the exact-pair drift), and
//!   `retarded_from`, from the present position a range query already has, ≤ 0.5 µs.
//! - `lens_walk_26kly`: the cold lens walk from the Sun to a bulge source at Baade's window,
//!   26,000 ly, over a year, in a galaxy whose systems move (the cone of Design note 13 as built),
//!   ≤ 15 s on one thread; `lens_walk_26kly_pool`, the same walk in chunks on the server's pool
//!   (its default, the cores less one), ≤ 5 s wall; `lens_renew_26kly`, a year's window renewed
//!   from candidates walked for two centuries, ≤ 10 ms; `lens_walk_26kly_still`, the plan's thin
//!   tube in a galaxy whose systems keep their epoch positions.
//! - `lens_cells_26kly`: generating the bulge walk's cells alone, the cold walk's floor.
//! - `lens_walk_disc_5kly`: a cold walk of a 5,000 ly disc sightline in the moving galaxy, ≤ 1 s.

use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use hyperion_sim::Seed;
use hyperion_sim::coords::{GalacticPosition, GalacticVelocity};
use hyperion_sim::events::TimeWindow;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{CellKey, NoCache, generate_cell};
use hyperion_sim::id::{Layer, SystemId};
use hyperion_sim::lensing::{
    LensCandidates, LensQuery, LensSightline, LensWalkPlan, lens_candidates, lenses_along,
    lenses_among,
};
use hyperion_sim::observe::{Drift, Observer, Trajectory, retarded, retarded_from};
use hyperion_sim::time::UniverseTime;

fn observe(c: &mut Criterion) {
    let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in range");
    let observer = Observer::new(sun, UniverseTime::EPOCH).expect("inside");
    let star = Drift::new(
        GalacticPosition::from_light_years([1_234.5, -22_000.25, 310.0]).expect("in range"),
        GalacticVelocity::new([-180e3, 95e3, 12e3]),
    );
    let later = Observer::new(sun, UniverseTime::from_julian_years(300).expect("in range"))
        .expect("inside");
    let present = star.position_at(later.time());
    let mut group = c.benchmark_group("observe");
    group.bench_function("retarded", |b| {
        b.iter(|| retarded(black_box(&observer), black_box(&star)));
    });
    group.bench_function("retarded_later", |b| {
        b.iter(|| retarded(black_box(&later), black_box(&star)));
    });
    group.bench_function("retarded_from", |b| {
        b.iter(|| retarded_from(black_box(&later), black_box(&star), black_box(&present)));
    });
    // One of the step's three evaluations of the exact-pair drift.
    let back = UniverseTime::from_julian_years(-47_000).expect("in range");
    group.bench_function("drift_position_at", |b| {
        b.iter(|| black_box(&star).position_at(black_box(back)));
    });
    group.finish();
}

/// The first system of a cell, as a lensed source.
fn source_in(galaxy: &Galaxy, key: CellKey) -> SystemId {
    let mut cell = Vec::new();
    generate_cell(galaxy, key, &mut cell);
    cell.first().expect("the cell holds systems").id()
}

/// The walk of `plan` in chunks of cells on `workers` threads, each taking the next chunk.
fn walk_on_pool(galaxy: &Galaxy, plan: LensWalkPlan, workers: usize) -> LensCandidates {
    let chunks: Vec<&[CellKey]> = plan.cells().chunks(2_048).collect();
    let next = AtomicUsize::new(0);
    let walked = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    let mut mine = Vec::new();
                    loop {
                        let k = next.fetch_add(1, Ordering::Relaxed);
                        let Some(cells) = chunks.get(k) else {
                            return mine;
                        };
                        mine.push(plan.walk(galaxy, &mut NoCache::new(), cells));
                    }
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("a walker does not panic"))
            .collect::<Vec<_>>()
    });
    plan.finish(walked)
}

fn lensing(c: &mut Criterion) {
    let still = Galaxy::from_params(Seed::new(0x1204_b100), GalaxyParams::milky_way_like())
        .expect("the fixture builds");
    let moving = Galaxy::from_params(Seed::new(0x1204_b100), GalaxyParams::milky_way_like())
        .expect("the fixture builds")
        .with_full_potential();
    // Baade's window: 3.9° south of the centre seen from the Sun, 1,770 ly below the plane.
    let bulge = source_in(&still, CellKey::new(Layer::C, [0, 0, -56]).expect("inside"));
    // A disc source 5,000 ly towards the anticentre, 100 ly above the plane.
    let disc = source_in(&still, CellKey::new(Layer::C, [0, 968, 3]).expect("inside"));
    let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in range");
    let year = |y| UniverseTime::from_julian_years(y).expect("in range");
    let query = |source, from, to| {
        LensQuery::builder(
            LensSightline::new(sun, source),
            TimeWindow::new(year(from), year(to)).expect("ordered"),
        )
        .build()
        .expect("a valid query")
    };
    let bulge_year = query(bulge, 0, 1);
    let workers = std::thread::available_parallelism()
        .map_or(1, std::num::NonZeroUsize::get)
        .saturating_sub(1)
        .max(1);
    let span = query(bulge, -100, 100);
    let candidates =
        lens_candidates(&moving, &mut NoCache::new(), &span).expect("the span's walk fits");
    let mut group = c.benchmark_group("lensing");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(60));
    group.bench_function("lens_walk_26kly_still", |b| {
        b.iter(|| lenses_along(&still, &mut NoCache::new(), &[], black_box(&bulge_year)));
    });
    group.bench_function("lens_walk_26kly", |b| {
        b.iter(|| lenses_along(&moving, &mut NoCache::new(), &[], black_box(&bulge_year)));
    });
    group.bench_function("lens_walk_26kly_pool", |b| {
        b.iter(|| {
            let plan = LensWalkPlan::new(&moving, black_box(&bulge_year)).expect("fits");
            lenses_among(
                &moving,
                &walk_on_pool(&moving, plan, workers),
                &[],
                &bulge_year,
            )
        });
    });
    // The cold walk's floor: generating the cone's cells, without examining a system.
    let plan = LensWalkPlan::new(&moving, &bulge_year).expect("fits");
    eprintln!(
        "the bulge walk: {} cells a year; {} candidates over two centuries, from {} cells and {} \
         systems",
        plan.cells().len(),
        candidates.len(),
        candidates.walked().cells(),
        candidates.walked().systems_examined()
    );
    group.bench_function("lens_cells_26kly", |b| {
        b.iter(|| {
            let mut systems = 0_usize;
            let mut cell = Vec::new();
            for &key in black_box(plan.cells()) {
                generate_cell(&moving, key, &mut cell);
                systems += cell.len();
            }
            systems
        });
    });
    group.bench_function("lens_renew_26kly", |b| {
        b.iter(|| lenses_among(&moving, &candidates, &[], black_box(&bulge_year)));
    });
    let disc_year = query(disc, 0, 1);
    group.bench_function("lens_walk_disc_5kly", |b| {
        b.iter(|| lenses_along(&moving, &mut NoCache::new(), &[], black_box(&disc_year)));
    });
    group.finish();
}

criterion_group!(benches, observe, lensing);
criterion_main!(benches);
