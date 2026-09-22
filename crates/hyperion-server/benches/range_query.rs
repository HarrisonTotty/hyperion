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
//! Release profile, seed `00000000000004d2`, eight cores, Criterion's median of a hundred samples
//! with the confidence interval beside it. The machine built four other lanes throughout, at a load
//! average of 11–14 against its eight cores, so every figure is pessimistic by an unknown factor; a
//! second run of the same file at a load of 14–17 gave 31.5 ms, 3.31 ms, 22.2 ms and 4.83 ms, which
//! is the spread to read these with. Targets from P04.T16 and the brainstorm's "The range query".
//!
//! This query visits 1,732 cells, examines 4,280 systems and returns 2,045, complete down to layer A
//! (measured with `QueryStats`), against the brainstorm's estimate of "about 1,400 fine cells".
//!
//! | Bench | Measured | Target |
//! | --- | --- | --- |
//! | 50 ly at 26,000 ly, cold cache | 36.3 ms [34.0, 38.9] | under 5 ms cold: **missed, by about 7×** |
//! | 50 ly at 26,000 ly, warm cache | 3.79 ms [3.66, 3.93] | — (a hit is 7× cheaper than generating) |
//! | 50 ly at 26,000 ly, no cache | 25.1 ms [24.5, 25.8] | the cache to add under 10%: **missed, +44% on a miss** |
//! | `systems_in_range`, 5,000 records, 1,290,424 bytes | 5.03 ms [4.91, 5.15] | — (0.25 GB/s of JSON) |
//!
//! What the figures say:
//!
//! - The cold query is the brainstorm's own coupling failing, not the cache's fault: 25.1 ms over
//!   1,732 cells is 14.5 µs a cell against the 1–2 µs it wanted for a sparse fine cell, and it says
//!   there that "the first and last targets stand or fall together". Closing that is plan 03's
//!   generation cost, not this crate's.
//! - The cache costs 44% on a miss — the per-cell `Vec`, the charge, the lock and the eviction
//!   bookkeeping, against `NoCache`'s one reused scratch buffer — and saves 85% on a hit. Two queries
//!   near one another share most of their cells, which is what it is for, so the trade is worth
//!   making; the 10% figure is not.
//! - A 5,000-record answer is 1.29 MB of JSON and 5 ms to serialise, so the 20,000-record limit is
//!   about 5 MB and 20 ms. That is why a large response is serialised as a pool job and never on the
//!   runtime (design note 21).
//! - End to end over loopback, the handler of P04.T14.d is what closes this, and it is not in this
//!   tree. Measured there instead, for this same query returning 2,045 records: 35.7 ms cold and
//!   12.3 ms warm, against T16's target of under 15 ms end to end — **met warm, missed cold**, by the
//!   same generation cost the cold figure above is.

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
