//! Benchmarks of the density map the server serves (plan 04, P04.T16): the banded build on the CPU
//! pool, and the quantisation and base64 of one response. Targets are recorded in the plan; a miss
//! is a finding to raise, not a CI failure: CI compiles these and never runs them.
//!
//! `map_512` builds a whole 512-pixel map through [`DensityMapService`] with the map cache's budget
//! set to zero, so every iteration is a cold build of every band, exactly as the first request for a
//! key is. The galaxy is built once and stays in the [`GalaxyCache`], as it does in a running
//! server. "All workers" is [`available_parallelism`](std::thread::available_parallelism), which was
//! eight on the machine below; "one worker" is the pool a two-core host defaults to
//! (`available_parallelism − 1`), and shows what the banding buys.
//!
//! `quantise_512` and `quantise_1024` take a face-on raster the group built once and measure
//! [`quantise_map`] and `QuantisedMap::to_base64` at both bit depths. Face-on is the upper bound
//! at each resolution, because an edge-on map of the same width is half as tall; the view otherwise
//! changes only how far the floor lies below the ceiling (5 dex against 7).
//!
//! # Measured, 2026-09-22
//!
//! Release profile, seed `00000000000004d2`, eight cores, Criterion's median of ten samples with the
//! confidence interval beside it. The machine built four other lanes throughout, at a load average
//! between 3 and 18 against its eight cores, so every figure is pessimistic by an unknown factor and
//! the eight-worker ones most of all: this is not a clean measurement, and P04.T11.c's idle figures
//! are quoted where they bear on a target. The two rows that have a target were measured again on
//! their own at a load of 8 to 17, and those are the figures given; the whole-file run at a load of 3
//! to 18 gave 1.47 s [1.18, 1.78] and 14.5 s [12.6, 16.5] for the same two, which is the spread to
//! read the rest with.
//!
//! | Bench | Measured | Target |
//! | --- | --- | --- |
//! | 512 face-on, all systems, eight workers | 1.07 s [0.99, 1.18] | under 1 s: **just missed**, met idle (0.7 s, P04.T11.c) |
//! | 512 face-on, young only, eight workers | 34.1 ms | — |
//! | 512 edge-on, all systems, eight workers | 12.8 s [11.2, 14.5] | under 3 s: **missed, by 4×** |
//! | 512 edge-on, young only, eight workers | 856 ms | — |
//! | 512 face-on, all systems, one worker | 8.05 s [6.92, 9.39] | — |
//! | 512 face-on, young only, one worker | 120 ms | — |
//! | 512 edge-on, all systems, one worker | 52.6 s [45.3, 60.2] | — |
//! | 512 edge-on, young only, one worker | 655 ms | — |
//! | quantise + base64, 512 face-on, 8 bits | 1.96 ms | — |
//! | quantise + base64, 512 face-on, 16 bits | 2.11 ms | — |
//! | quantise + base64, 1024 face-on, 8 bits | 7.00 ms | — |
//! | quantise + base64, 1024 face-on, 16 bits | 7.96 ms | — |
//!
//! What the figures say:
//!
//! - The edge-on miss is plan 02's raster cost and not this crate's: 0.40 ms a pixel here against
//!   face-on's 0.031, because every spheroid component integrates along its own line of sight per
//!   pixel (`EdgeOnPlan::pixel`). The per-pixel cost rises with resolution, since the integration
//!   step follows the pixel, which is why a 1,024-pixel edge-on map is 422 s on one core (P04.T11.c)
//!   and not four times this 52.6 s. No server-side change reaches 3 s; see the plan's Risks.
//! - Eight workers are worth 5.5× to 7.5× face-on but only 3.6× to 4.1× edge-on. An edge-on map's bands are not
//!   equal — the bands through the plane cost several times those at the top and bottom of the
//!   raster — and a map is done when its dearest band is, which at 256 rows is 16 bands over eight
//!   workers. `BAND_ROWS` is the knob if that ever matters.
//! - A young-only map costs a fortieth of a full one, because it integrates the young thin disc
//!   alone. The 655 ms edge-on figure on one worker came in under the 856 ms on eight because the
//!   host's load fell during the run, which is the clearest measure of how contaminated these are.
//! - Quantising a whole 1,024-pixel map and encoding it is about 8 ms, a thousandth of the raster it
//!   came from and well inside a response's budget. That is what lets the cache hold the raw grid and
//!   quantise per response, at whichever depth was asked for (design note 12).
//!
//! T16's other two targets are end to end over a real socket, which is a server and not a benchmark
//! fixture, so they were measured with a throwaway integration test in a release build at a load
//! average of 13, and are recorded here for the plan: a **cached** 512-pixel face-on map request is
//! 4.86 ms at 8 bits and 4.89 ms at 16 (five requests each, 3.5 to 6.4 ms), against a target of under
//! 5 ms — **met, with nothing to spare**, and most of it the quantise, the base64 and the 350 to 700
//! KiB of JSON above. A `ping` sent while a 1,024-pixel edge-on map's bands are on a one-worker pool
//! is answered in 114 µs (58 to 121 µs), against a target of 50 ms: **met by four hundredfold**,
//! which is design note 21 working — a band never runs on the runtime.

use std::hint::black_box;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use criterion::{Criterion, SamplingMode, criterion_group, criterion_main};
use hyperion_protocol::{MapPopulation, MapView};
use hyperion_server::compute::{
    CodeDepth, CpuPool, DensityMapService, GalaxyCache, GalaxyKey, MapKey, MapResolution,
    RawDensityMap, quantise_map,
};
use hyperion_server::limits::{BULK_QUEUE_CAPACITY, INTERACTIVE_QUEUE_CAPACITY};
use hyperion_sim::GENERATOR_VERSION;
use tokio::runtime::{Builder, Runtime};

/// The seed of every galaxy these benchmarks map: the plan's fixture universe.
const SEED: u64 = 0x4d2;

/// The runtime a build is driven from: one thread, because every pixel of a map is computed on the
/// pool's own threads and the runtime only awaits the bands.
fn runtime() -> Runtime {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a current-thread runtime starts")
}

/// A pool of `workers` workers, and a map service over it that caches nothing.
///
/// A budget of zero means every `get` recomputes its map, which is what a cold build is; the two
/// cache lookups it still makes are microseconds against seconds of raster.
fn service(workers: NonZeroUsize) -> (Arc<CpuPool>, DensityMapService) {
    let pool = Arc::new(
        CpuPool::new(workers, INTERACTIVE_QUEUE_CAPACITY, BULK_QUEUE_CAPACITY)
            .expect("the pool starts"),
    );
    let galaxies = Arc::new(GalaxyCache::new(Arc::clone(&pool)));
    let service = DensityMapService::new(Arc::clone(&pool), galaxies, 0);
    (pool, service)
}

/// The key of a map of the fixture galaxy.
fn key(view: MapView, population: MapPopulation, resolution: MapResolution) -> MapKey {
    MapKey::new(
        GalaxyKey::new(SEED, GENERATOR_VERSION),
        view,
        population,
        resolution,
    )
}

/// The raw map of `key`, computed on `service`'s pool.
fn build(runtime: &Runtime, service: &DensityMapService, key: MapKey) -> Arc<RawDensityMap> {
    runtime
        .block_on(service.get(key))
        .expect("the pool computes the map")
}

/// Every worker the host has, which is what the pool is given here; a server keeps one core for the
/// runtime (`--num-workers` defaults to `available_parallelism − 1`).
fn all_workers() -> NonZeroUsize {
    thread::available_parallelism().unwrap_or(NonZeroUsize::MIN)
}

/// A 512-pixel map built cold, for both views and both populations, on every worker and on one.
///
/// Both views are measured because they cost two orders of magnitude apart per pixel, and both
/// populations because a young-only map integrates fewer components.
fn map_512(c: &mut Criterion) {
    let runtime = runtime();
    let mut group = c.benchmark_group("map_512");
    // A build is seconds long, so a sample is one iteration (`Flat`) and ten samples are the
    // fewest Criterion accepts. The warm-up runs one build of its own.
    group.sampling_mode(SamplingMode::Flat);
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));
    for (workers, label) in [
        (all_workers(), "all workers"),
        (NonZeroUsize::MIN, "one worker"),
    ] {
        let (pool, service) = service(workers);
        for view in [MapView::FaceOn, MapView::EdgeOn] {
            for population in [MapPopulation::All, MapPopulation::Young] {
                let key = key(view, population, MapResolution::Px512);
                group.measurement_time(measurement_time(view, population, workers));
                group.bench_function(
                    format!(
                        "{} {} ({label})",
                        view_label(view),
                        population_label(population)
                    ),
                    |b| b.iter(|| build(&runtime, &service, black_box(key))),
                );
            }
        }
        // The workers are joined before the next pool starts, so that the two measurements do not
        // share the host's cores.
        runtime
            .block_on(pool.shutdown())
            .expect("the pool shuts down");
    }
    group.finish();
}

/// How long one configuration is measured for: room for the ten samples of one build each that
/// `Flat` sampling wants, from the costs measured in this file's module docs, with a little over for
/// a loaded host. The eight configurations span four orders of magnitude, so one time for all of
/// them would either waste an hour on the cheap ones or cut the dear ones short.
fn measurement_time(view: MapView, population: MapPopulation, workers: NonZeroUsize) -> Duration {
    let seconds = match (view, population, workers.get()) {
        // About 2 s a build on eight workers and 6.6 s on one: 0.025 ms a pixel over 262,144.
        (MapView::FaceOn, MapPopulation::All, 1) => 90,
        (MapView::FaceOn, MapPopulation::All, _) => 30,
        // 37 ms and 117 ms: a young-only map integrates the young thin disc alone.
        (MapView::FaceOn, MapPopulation::Young, _) => 10,
        // About 13 s on eight workers and 53 s on one: 0.40 ms a pixel over 131,072, plan 02's
        // line-of-sight integral per spheroid component.
        (MapView::EdgeOn, MapPopulation::All, 1) => 700,
        (MapView::EdgeOn, MapPopulation::All, _) => 180,
        // Under a second on eight workers, a few on one.
        (MapView::EdgeOn, MapPopulation::Young, 1) => 60,
        (MapView::EdgeOn, MapPopulation::Young, _) => 20,
    };
    Duration::from_secs(seconds)
}

/// One response's quantisation and base64 of a 512-pixel face-on map, at both depths.
fn quantise_512(c: &mut Criterion) {
    quantise_group(c, MapResolution::Px512, "quantise_512");
}

/// The same for a 1,024-pixel map: 1,048,576 pixels, the largest response M1 serves.
fn quantise_1024(c: &mut Criterion) {
    quantise_group(c, MapResolution::Px1024, "quantise_1024");
}

/// Measures [`quantise_map`] and `QuantisedMap::to_base64` over a face-on raster of `resolution`,
/// which is built once on every worker the host has.
fn quantise_group(c: &mut Criterion, resolution: MapResolution, name: &str) {
    let runtime = runtime();
    let (pool, service) = service(all_workers());
    let map = build(
        &runtime,
        &service,
        key(MapView::FaceOn, MapPopulation::All, resolution),
    );
    runtime
        .block_on(pool.shutdown())
        .expect("the pool shuts down");
    let mut group = c.benchmark_group(name);
    for depth in [CodeDepth::Eight, CodeDepth::Sixteen] {
        group.bench_function(format!("quantise + base64 ({} bits)", depth.bits()), |b| {
            b.iter(|| {
                let quantised = quantise_map(black_box(&map), MapView::FaceOn, depth);
                quantised.to_base64()
            });
        });
    }
    group.finish();
}

/// The view, as the benchmark's name spells it.
fn view_label(view: MapView) -> &'static str {
    match view {
        MapView::FaceOn => "face-on",
        MapView::EdgeOn => "edge-on",
    }
}

/// The population, as the benchmark's name spells it.
fn population_label(population: MapPopulation) -> &'static str {
    match population {
        MapPopulation::All => "all systems",
        MapPopulation::Young => "young only",
    }
}

criterion_group!(density_map, map_512, quantise_512, quantise_1024);
criterion_main!(density_map);
