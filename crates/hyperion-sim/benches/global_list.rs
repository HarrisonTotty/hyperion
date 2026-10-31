//! Benchmarks of the global list's tube tables (plan 10). Targets are recorded in the plan; a miss
//! is a finding to raise, not a CI failure: CI compiles these and never runs them.
//!
//! - `TubeTable::build` for `pal5_like` on the Milky Way fixture (P10.T6.c): about 0.2 s and about
//!   80 kB (brainstorm, "Streams and accreted structure"). The build sprays its 2,000 tracers one
//!   after another, 20,474 steps each; a caller's pool sprays them in parallel and gathers them by
//!   index (ruling 146.3), as `TubeTable::build (pool)` does on `HYPERION_WORKERS` threads, else
//!   the machine's threads less one. Measured (pod, 16 vCPU, 2026-10-30, the release profile):
//!   2.12 s alone and 0.138 s on 16 threads, so the pool meets 0.2 s and ruling 146.3's fallback,
//!   `τ_p ÷ 32`, is not applied; the table holds 90,256 bytes, its 512 knots 86,016.
//! - `TubeTable::from_tracers`, the reduction of those tracers alone: 1.67 ms (pod, 16 vCPU,
//!   2026-10-31), against 1.33 ms before its centre line was fitted in the orbital plane, one
//!   `atan2` a kernel tracer (P10.T6.a's tips); the pool's build took 91 ms then, against 96 ms
//!   before.
//! - `TubeTable::cells_touching`, the cells a 50 ly sphere about `pal5_like`'s axis touches, which
//!   P10.T9's query walks: 10.5 µs (pod, 16 vCPU), the scan for the segments included (module
//!   documentation of `global_list::tube`, "Finding the segments").
//! - `GlobalListSource`'s hits and census for a 50 ly sphere on `pal5_like`'s axis, its table
//!   kept by the caller (P10.T9), against the plan's "about 160 [members generated] to find two",
//!   which the setup prints for the sphere it times. Measured (pod, 16 vCPU, 2026-10-31, the
//!   release profile, the tips fitted in the orbital plane): 9 generated and 3 returned; the hits
//!   1.03 ms, the census 49 µs.

use std::env::VarError;
use std::hint::black_box;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use hyperion_sim::Seed;
use hyperion_sim::galaxy::features::centre::NoCentreOrbitCache;
use hyperion_sim::galaxy::global_list::members::{StreamBandShares, stream_cell_members};
use hyperion_sim::galaxy::global_list::spray::{Spray, SprayForce, SprayParams, Tracer};
use hyperion_sim::galaxy::global_list::testing::pal5_like;
use hyperion_sim::galaxy::global_list::{
    GlobalList, GlobalListSource, StreamNumber, StreamSpec, TubeLookup, TubeTable,
};
use hyperion_sim::galaxy::imf::MassBand;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::query::{LayerSet, QuerySphere, SystemSource};
use hyperion_sim::galaxy::{Galaxy, Population};
use hyperion_sim::id::Layer;
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::LightYears;

/// The pool's width: `HYPERION_WORKERS` where it is set, the server's own variable for its pool's
/// width, so that a run can take a CPU quota the machine's thread count does not show; otherwise
/// the machine's threads less one, as the server leaves one to its runtime.
fn workers() -> u32 {
    match std::env::var("HYPERION_WORKERS") {
        Err(VarError::NotPresent) => std::thread::available_parallelism()
            .map_or(1, |n| u32::try_from(n.get()).unwrap_or(u32::MAX))
            .saturating_sub(1)
            .max(1),
        Err(VarError::NotUnicode(_)) => panic!("HYPERION_WORKERS is not Unicode"),
        Ok(v) => v
            .parse::<u32>()
            .ok()
            .filter(|&n| n > 0)
            .unwrap_or_else(|| panic!("HYPERION_WORKERS={v} is not a positive count")),
    }
}

/// `spec`'s tube in `galaxy`, its tracers sprayed on `workers` threads in chunks of 16, each
/// thread taking the next chunk, and gathered by index.
fn build_on_pool(
    galaxy: &Galaxy,
    force: &SprayForce,
    spec: &StreamSpec,
    workers: u32,
) -> TubeTable {
    let spray = Spray::new(
        galaxy.seed(),
        force,
        galaxy.potential(),
        spec,
        &SprayParams::default(),
    )
    .expect("a finite stripping time");
    let count = spray.tracer_count();
    let next = AtomicU32::new(0);
    let mut sprayed: Vec<(u32, Tracer)> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    let mut mine = Vec::new();
                    loop {
                        let start = next.fetch_add(16, Ordering::Relaxed);
                        if start >= count {
                            return mine;
                        }
                        mine.extend((start..count.min(start + 16)).map(|i| (i, spray.tracer(i))));
                    }
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("a sprayer does not panic"))
            .collect()
    });
    sprayed.sort_unstable_by_key(|(i, _)| *i);
    let tracers: Vec<Tracer> = sprayed.into_iter().map(|(_, t)| t).collect();
    TubeTable::from_tracers(
        galaxy.seed(),
        galaxy.mean_system_mass(Population::Halo),
        spec,
        &tracers,
    )
    .expect("enough tracers in each arm")
}

/// `pal5_like`'s tube on the Milky Way fixture: built alone and on a pool, reduced, and queried.
fn tube(c: &mut Criterion) {
    let galaxy = Galaxy::from_params(Seed::new(7), GalaxyParams::milky_way_like())
        .expect("the fixture builds")
        .with_full_potential();
    let spec = pal5_like(galaxy.potential());
    let force = SprayForce::new(galaxy.potential()).expect("the full potential");
    let workers = workers();
    let tracers = Spray::new(
        galaxy.seed(),
        &force,
        galaxy.potential(),
        &spec,
        &SprayParams::default(),
    )
    .expect("a finite stripping time")
    .tracers();
    let table = TubeTable::build(&galaxy, &spec).expect("the full potential");
    assert_eq!(build_on_pool(&galaxy, &force, &spec, workers), table);
    println!(
        "pal5_like: {} bytes, {workers} workers for the pool",
        table.byte_size()
    );
    let mut group = c.benchmark_group("tube");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(30));
    group.bench_function("TubeTable::build (pal5_like)", |b| {
        b.iter(|| TubeTable::build(black_box(&galaxy), black_box(&spec)));
    });
    group.bench_function("TubeTable::build (pal5_like, pool)", |b| {
        b.iter(|| build_on_pool(black_box(&galaxy), &force, black_box(&spec), workers));
    });
    group.finish();
    let mut group = c.benchmark_group("tube");
    let (seed, mean) = (galaxy.seed(), galaxy.mean_system_mass(Population::Halo));
    group.bench_function("TubeTable::from_tracers (pal5_like)", |b| {
        b.iter(|| TubeTable::from_tracers(seed, mean, black_box(&spec), black_box(&tracers)));
    });
    let centre = table.locate(0.3, 0.2, -0.1);
    group.bench_function("TubeTable::cells_touching (pal5_like, 50 ly)", |b| {
        b.iter(|| {
            table
                .cells_touching(black_box(&centre), LightYears::new(50.0))
                .count()
        });
    });
    group.finish();
}

/// A caller's cache that holds one stream's table: `pal5_like`'s, stream 0.
struct One {
    list: GlobalList,
    table: Arc<TubeTable>,
}

impl TubeLookup for One {
    fn global_list(&self) -> &GlobalList {
        &self.list
    }

    fn tube(&self, _: StreamNumber) -> Arc<TubeTable> {
        Arc::clone(&self.table)
    }
}

/// The global list's source for a 50 ly sphere on `pal5_like`'s axis, at the epoch, its table
/// kept (P10.T9).
fn query(c: &mut Criterion) {
    let galaxy = Galaxy::from_params(Seed::new(7), GalaxyParams::milky_way_like())
        .expect("the fixture builds")
        .with_full_potential();
    let spec = pal5_like(galaxy.potential());
    let table = Arc::new(TubeTable::build(&galaxy, &spec).expect("the full potential"));
    let list = GlobalList::of_parts(&galaxy, galaxy.potential(), vec![spec.clone()], Vec::new());
    let lookup = One { list, table };
    let source = GlobalListSource::new(&galaxy, &lookup, &NoCentreOrbitCache)
        .expect("a list without the centre");
    let centre = lookup.table.locate(0.3, 0.0, 0.0);
    let radius = LightYears::new(50.0);
    let sphere = QuerySphere::new(centre, radius, UniverseTime::EPOCH, LightYears::ZERO)
        .expect("a positive radius");
    let layers = [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E]
        .into_iter()
        .fold(LayerSet::EMPTY, LayerSet::with);
    let mut hits = Vec::new();
    source.systems_in_sphere(&galaxy, &sphere, layers, &mut hits);
    let shares = StreamBandShares::of(&galaxy, &spec);
    let mut members = Vec::new();
    let mut generated = 0;
    for cell in lookup.table.cells_touching(&centre, radius) {
        for band in [
            MassBand::E,
            MassBand::D,
            MassBand::C,
            MassBand::B,
            MassBand::A,
        ] {
            stream_cell_members(&galaxy, &lookup.table, &shares, cell, band, &mut members);
            generated += members.len();
        }
    }
    println!(
        "pal5_like, 50 ly on its axis: {generated} members generated, {} returned",
        hits.len()
    );
    let mut group = c.benchmark_group("query");
    group.bench_function(
        "GlobalListSource::systems_in_sphere (pal5_like, 50 ly)",
        |b| {
            b.iter(|| {
                let mut out = Vec::new();
                source.systems_in_sphere(&galaxy, black_box(&sphere), layers, &mut out);
                out.len()
            });
        },
    );
    group.bench_function(
        "GlobalListSource::expected_in_sphere (pal5_like, 50 ly)",
        |b| {
            b.iter(|| source.expected_in_sphere(&galaxy, black_box(&sphere)));
        },
    );
    group.finish();
}

criterion_group!(global_list, tube, query);
criterion_main!(global_list);
