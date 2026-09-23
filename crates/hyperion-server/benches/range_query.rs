//! Benchmarks of the range query as the server runs it (plan 04, P04.T16): plan 03's query over the
//! shared cell cache, cold and warm, against its uncached path, and the serialisation of a large
//! response. Targets are recorded in the plan; a miss is a finding to raise, not a CI failure: CI
//! compiles these and never runs them.
//!
//! `range_query` asks for the systems within 50 ly of a point 26,000 ly from the galactic centre,
//! the brainstorm's reference query, three ways:
//!
//! - **cold**, over a [`SharedCellCache`] built fresh for the iteration, so every cell it visits is
//!   generated and then charged to the cache;
//! - **warm**, over one cache the group filled before measuring, so every cell is a hit;
//! - **uncached**, over plan 03's [`NoCache`], which generates every cell into a scratch buffer and
//!   drops it, and so shows what the cache and its lock are worth.
//!
//! The cold and the uncached paths generate the same cells; the difference between them is the
//! cache's cost on a miss (the lookup, the charge, and the eviction it may force).
//!
//! `response` serialises a `systems_in_range` response of 5,000 records into the text of a frame,
//! which the server does as a pool job for a large answer and never on the runtime (design note 21).
//! The records are protocol values built here rather than converted from the query's
//! [`SystemHit`](hyperion_sim::galaxy::query::SystemHit)s, because that conversion is P04.T14.d's;
//! what is measured is `serde_json` over the wire form, which is the cost the plan asks about.
//!
//! # Measured, 2026-09-22
//!
//! Release profile, seed `00000000000004d2`, generator version 8 (the version-9 fixture moves every
//! count here), on an i7-8665U: four cores and eight hyperthreads, whose clock moves between 1.9 and
//! 4.8 GHz with load and heat. **A bare time on this machine is not a measurement**: the uncached
//! query below took 10.0 ms idle and 31.3 ms under Criterion at a load of 14. Each figure is
//! therefore given with the sim's `math::exp` timed in the same process, and a per-cell cost in those
//! calls, which stays put while the clock moves (plan 02, R21; plan 04, Risks, the validation of
//! T16).
//!
//! Idle (load average 1.6, 3.4 to 4.2 GHz, one `math::exp` 7.4 to 7.9 ns), best of 15, three runs:
//!
//! | Path | Measured | Per visited cell | Target |
//! | --- | --- | --- | --- |
//! | 50 ly at 26,000 ly, no cache | 10.0–10.1 ms | 5.8 µs, 770–780 `exp` | under 5 ms cold: **missed, by 2×** |
//! | 50 ly at 26,000 ly, cold cache | 10.8–11.8 ms | 6.2–6.8 µs | the cache to add under 10%: **met, +8% to +18%** |
//! | 50 ly at 26,000 ly, warm cache | 1.10–1.73 ms | 0.6–1.0 µs | — (a hit is 6–9× cheaper than generating) |
//!
//! This query visits 1,732 cells, examines 4,280 systems and returns 2,045, complete down to layer
//! A (`QueryStats`), against the brainstorm's estimate of "about 1,400 fine cells": 1,424 of the
//! cells are layer A's. Placing one cell costs, by layer: A 3.8–4.2 µs (500–555 `exp`), B 5.3–5.7
//! µs, C 30–32 µs, D 30–32 µs, E 69–73 µs.
//!
//! What the figures say:
//!
//! - A layer-A cell at the solar circle, the sparse fine cell the brainstorm means, is 3.8 µs
//!   against its 1–2 µs: **missed, by 2 to 4 times**. Plan 02's R21 estimated 119 `exp` for a
//!   sparse cell from the bound and one candidate's densities; the whole placement measures about
//!   four times that.
//! - The coarse layers are not "cheap in absolute terms": B to E are 3.9 of the 9.3 ms of placing,
//!   42%, so even layer-A cells at 1–2 µs would leave this query at 5.3 to 6.7 ms at least. The brainstorm's
//!   "the first and last targets stand or fall together" is therefore not quite so: the 5 ms would
//!   fall with the coarse layers alone.
//! - The cache costs about a tenth on a miss and saves 84% to 90% on a hit, as the plan wanted.
//! - A 5,000-record answer is 1.29 MB of JSON, 5.0–5.4 ms to serialise under a load of 11–17 (not
//!   normalised; no target), so the 20,000-record limit is about 5 MB and 20 ms: why a large
//!   response is serialised as a pool job and never on the runtime (design note 21).
//! - End to end over loopback, P04.T14.d's handler returning the same 2,045 records: cold 18.1 to
//!   20.2 ms and warm 6.4 to 7.4 ms at a load of 10 with `exp` at 12.3 ns, which is 1.6 times the
//!   idle figure; idle, that is about 11 to 14 ms cold, from the compute above plus about 1 ms each
//!   of serialising and parsing, against T16's under 15 ms: **met, narrowly, on an idle machine**,
//!   and met warm by a wide margin. At a load of 22 the same run read 52 to 72 ms cold.
//!
//! The figures first recorded here (cold 36.3 ms, warm 3.79 ms, uncached 25.1 ms, so 14.5 µs a
//! cell and a cache costing 44% on a miss) were taken at a load of 11 to 17 and not normalised; a
//! Criterion run at a load of 14 during the validation gave cold 20.1 ms, warm 3.33 ms and uncached
//! 31.3 ms, the cache's cost reversing its sign. They measured the machine, not the code.

use std::hint::black_box;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use hyperion_protocol::{
    Census, GalacticPosition, LayerCensus, LayerStatus, MassLayer, Population, RequestId,
    ResponseBody, ServerMessage, SystemIdHex, SystemRecord, SystemsInRange, UniverseIdHex,
    UniverseTime,
};
use hyperion_server::compute::{GalaxyKey, SharedCellCache};
use hyperion_sim::coords::GalacticPosition as SimPosition;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::placement::NoCache;
use hyperion_sim::galaxy::query::{RangeQuery, range_query};
use hyperion_sim::units::LightYears;
use hyperion_sim::{GENERATOR_VERSION, Seed};

/// The seed of the galaxy every query here runs over: the plan's fixture universe.
const SEED: u64 = 0x4d2;

/// How far from the galactic centre the query's centre lies, in light-years: the solar circle, where
/// the brainstorm's 5 ms figure is quoted.
const CENTRE_RADIUS_LY: f64 = 26_000.0;

/// The query's radius in light-years: the brainstorm's reference sphere.
const QUERY_RADIUS_LY: f64 = 50.0;

/// The cell cache's budget in bytes: the server's default (`--cell-cache`, 256 MiB), which holds
/// every cell of a 50 ly sphere many times over.
const CELL_CACHE_BYTES: usize = 256 << 20;

/// How many records the serialised response holds: a quarter of the largest answer M1 serves
/// ([`MAX_CENSUS_LIMIT`](hyperion_server::limits::MAX_CENSUS_LIMIT) is 20,000), and the plan's
/// figure for T16.
const RECORDS: usize = 5_000;

/// The ID of the first record, a valid grid ID (`H7K 4C0RFZ A-7`'s neighbourhood), so that the hex
/// strings serialised are the length a real answer's are.
const FIRST_SYSTEM_ID: u64 = 0x0228_C386_27FF_0000;

/// Metres in a light-year, for the records' offsets within their light-year cells (IAU 2012:
/// `9.460_730_472_580_8` × 10¹⁵ m).
const METRES_PER_LY: f64 = 9.460_730_472_580_8e15;

/// The query the benchmarks run: 50 ly at the solar circle, at the epoch, every layer.
fn query() -> RangeQuery {
    let centre = SimPosition::from_light_years([0.0, CENTRE_RADIUS_LY, 0.0])
        .expect("the solar circle is inside the root cube");
    RangeQuery::builder(centre, LightYears::new(QUERY_RADIUS_LY))
        .build()
        .expect("a 50 ly sphere at the epoch is a valid query")
}

/// The 50 ly query over a cold cache, a warm one and no cache at all.
fn query_50_ly(c: &mut Criterion) {
    let galaxy = Galaxy::new(Seed::new(SEED));
    let key = GalaxyKey::new(SEED, GENERATOR_VERSION);
    let query = query();
    let mut group = c.benchmark_group("range_query");
    group.bench_function("50 ly at 26,000 ly (cold cache)", |b| {
        b.iter_batched_ref(
            || SharedCellCache::new(CELL_CACHE_BYTES),
            |cache| {
                let mut handle = cache.handle(key);
                range_query(&galaxy, &mut handle, &[], black_box(&query))
            },
            // One cache per iteration: a warm cell would make this the warm measurement.
            BatchSize::PerIteration,
        );
    });
    let warm = SharedCellCache::new(CELL_CACHE_BYTES);
    drop(range_query(&galaxy, &mut warm.handle(key), &[], &query));
    group.bench_function("50 ly at 26,000 ly (warm cache)", |b| {
        b.iter(|| {
            let mut handle = warm.handle(key);
            range_query(&galaxy, &mut handle, &[], black_box(&query))
        });
    });
    group.bench_function("50 ly at 26,000 ly (no cache)", |b| {
        b.iter(|| range_query(&galaxy, &mut NoCache::new(), &[], black_box(&query)));
    });
    group.finish();
}

/// Serialising a `systems_in_range` response of [`RECORDS`] records into the text of a frame.
fn response(c: &mut Criterion) {
    let message = ServerMessage::Response {
        id: RequestId(1),
        body: ResponseBody::SystemsInRange(systems_in_range(RECORDS)),
    };
    let bytes = serde_json::to_string(&message)
        .expect("a server message serialises")
        .len();
    let mut group = c.benchmark_group("response");
    group.bench_function(
        format!("systems_in_range, {RECORDS} records ({bytes} bytes)"),
        |b| {
            b.iter(|| serde_json::to_string(black_box(&message)).expect("the message serialises"));
        },
    );
    group.finish();
}

/// A response holding `records` systems, shaped as a real answer at the solar circle: every field
/// populated, designations of the grid form, and a census of all five layers.
fn systems_in_range(records: usize) -> SystemsInRange {
    let layers = [
        MassLayer::A,
        MassLayer::B,
        MassLayer::C,
        MassLayer::D,
        MassLayer::E,
    ];
    let count = u32::try_from(records).expect("the record count fits in a u32");
    let systems = (0..count)
        .map(|index| {
            // Spread over the sphere and over the layers, so that no field serialises the same
            // digits twice and every layer's letter appears in a designation.
            let along = f64::from(index) / f64::from(count);
            let layer = layers[usize::try_from(index % 5).expect("below five")];
            SystemRecord {
                id: SystemIdHex::from_u64(FIRST_SYSTEM_ID + u64::from(index)),
                designation: format!("H7K 4C0RFZ {}-{index}", layer_letter(layer)),
                position: GalacticPosition {
                    cell_ly: [
                        i32::try_from(index % 100).expect("below one hundred"),
                        25_950,
                        -3,
                    ],
                    // Each offset stays inside its light-year cell, as `GalacticPosition` requires.
                    offset_m: [
                        along * METRES_PER_LY,
                        (0.75 - along * 0.5) * METRES_PER_LY,
                        (0.25 + along * 0.5) * METRES_PER_LY,
                    ],
                },
                layer,
                initial_mass_msun: 0.1 + along * 8.0,
                age_myr: 100.0 + along * 12_000.0,
                population: Population::OldThinDisc,
                stellar: None,
            }
        })
        .collect();
    SystemsInRange {
        universe: UniverseIdHex::from_u64(0x0123_4567_89ab_cdef),
        centre: GalacticPosition {
            cell_ly: [0, 26_000, 0],
            offset_m: [0.0, 0.0, 0.0],
        },
        radius_ly: QUERY_RADIUS_LY,
        time: UniverseTime {
            seconds: 0,
            nanos: 0,
        },
        census: Census {
            limit: 20_000,
            complete_above_msun: Some(0.08),
            layers: layers
                .into_iter()
                .map(|layer| LayerCensus {
                    layer,
                    mass_min_msun: 0.08,
                    mass_max_msun: 150.0,
                    expected: 4_812.5,
                    returned: u32::try_from(records / 5).expect("a fifth of the records"),
                    status: LayerStatus::Included,
                })
                .collect(),
        },
        systems,
    }
}

/// The letter a layer's designation carries.
fn layer_letter(layer: MassLayer) -> char {
    match layer {
        MassLayer::A => 'A',
        MassLayer::B => 'B',
        MassLayer::C => 'C',
        MassLayer::D => 'D',
        MassLayer::E => 'E',
    }
}

criterion_group!(range_query_benches, query_50_ly, response);
criterion_main!(range_query_benches);
