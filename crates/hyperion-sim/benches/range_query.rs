//! Benchmarks of the range query: expected counts, and the query cold, warm and long-range
//! (plan 03, P03.T11).
//!
//! All of them run on `GalaxyParams::milky_way_like()` with fixed seeds. A miss is a finding to
//! raise, not a CI failure: CI compiles these and never runs them.
//!
//! **The figures below predate P02.T11's tuning of `milky_way_like()`, which can move every one of
//! them, and are to be re-measured after it** (plan 03's task list: P02.T11 "may tune
//! `milky_way_like()`, which … moves T11's figures").
//!
//! Measured 2026-09-22 on an Intel i7-8665U (4 cores, 8 threads, 1.9 GHz base, 4.8 GHz turbo) with
//! other lanes building on the machine at the same time, in three rounds at different load averages,
//! against the machine's 8 threads. The load is given with each column because it moves the figures
//! by two to three times; `placement.rs` records the calibration this rests on, where plan 02's own
//! `Fields::densities` and `Fields::layer_bound` measure 1.35 µs and 1.41 µs at load 4–8 against the
//! 540–570 ns and 543–694 ns of its R16 and R17.
//!
//! | Bench | Target | Load 9–13 | Load 15–20 | Load 20 |
//! | ----- | ------ | --------- | ---------- | ------- |
//! | `expected_counts (50 ly)` | under 0.3 ms | 273 µs, met | 587 µs | 425 µs |
//! | `expected_counts (5000 ly)` | none; plan says 9–19 ms | 36.3 ms | 88.6 ms | 57.1 ms |
//! | `range_50ly_cold` | under 5 ms | **21.8 ms, missed** | 19.3 ms | 17.7 ms |
//! | `range_50ly_warm` | none | 1.58 ms | 1.76 ms | 1.72 ms |
//! | `range_500ly_floor_d` | none | 217 ms | 224 ms | — |
//!
//! **The brainstorm's 5 ms for a cold 50 ly query at Sun-like density is missed on every run**, by
//! four times at the lowest load reached. The 0.3 ms budget for `expected_counts` at 50 ly is met at
//! that load and missed above it. Neither figure was chased: nothing in the sim was optimised for
//! these benches and no target was moved to meet one.
//!
//! The plan's arithmetic for the cold query — about 1,700 bounds and 3,100 candidates at roughly
//! 0.6 µs each, some 3 ms — becomes 6.6 ms at the 1.35–1.41 µs those two calls actually cost today,
//! and the measured 21.8 ms is about three times that again. The excess is the rest of a candidate
//! (three more streams opened for the position, the mass and the age), the per-system test at `t` for
//! 1,360 systems, the sort, and whatever the load added. Design note 15's pre-filter and the first
//! risk's lazy component evaluation are the levers the plan reserves for it; both are plan 03's to
//! pull once the fixture is tuned, and neither is pulled here.
//!
//! What the figures cover:
//!
//! - The 50 ly query at the Sun-like point is complete down to layer A, stopped by the mass floor,
//!   and returns **1,360 systems** from **1,732 cells** in this seed — the plan's "about 1,600 at the
//!   reference density scaled to the seed". The warm case is the same query with all 1,732 cells
//!   already in a `BTreeMap` cache, so it measures the walk, the per-system test at `t` and the sort
//!   with no generation at all: a fourteenth of the cold cost, which is what a server's cache buys
//!   (plan 04).
//! - `expected_counts` at 50 ly integrates two panels of 4 × 4 × 8 nodes about the plane, and at
//!   5,000 ly sixteen panels a side of 8 × 8 × 16 (P03.T9.c), which is why it grows by two orders of
//!   magnitude between them. At 5,000 ly it is two to four times the plan's 9–19 ms estimate.
//! - **The long-range query needs a raised limit to walk anything at all.** At 500 ly from the
//!   Sun-like point the fixture expects 9,094 systems in layer E and 32,886 in layer D, so under the
//!   default census limit of 4,096 the census admits *no* layer, generates nothing, and the query
//!   costs only its 500 ly quadrature: 22.8 ms measured at load 20, all of it `expected_counts`. The
//!   bench therefore sets the limit to 65,536 (`LONG_RANGE_LIMIT`), which admits E and D and measures
//!   the long walk the plan asks for: 41,925 systems over 3,008 cells. That the default limit stops a
//!   500 ly disc query before layer E is a finding about the census rule at long range, not a change
//!   to it — Design note 9's limit is on expected counts, and a 500 ly sphere in the disc is simply
//!   past it.
use std::collections::BTreeMap;
use std::hint::black_box;
use std::num::NonZeroU32;
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use hyperion_sim::Seed;
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{CellCache, CellKey, NoCache, SystemRecord, generate_cell};
use hyperion_sim::galaxy::query::{MassFloor, RangeQuery, expected_counts, range_query};
use hyperion_sim::id::Layer;
use hyperion_sim::units::LightYears;

/// The seed every query here runs in.
const SEED: u64 = 0x0311_1000_0000_0000;

/// A point like the Sun's: in the plane, 26,000 ly out on the +y axis, clear of the bar.
///
/// The tests take this from `tests/common`, which a benchmark cannot see, so it is written out here.
fn sunlike_point() -> GalacticPosition {
    GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("26,000 ly is in the root cube")
}

fn galaxy() -> Galaxy {
    Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
}

/// A cache holding every cell it has been asked for: the warm case, and what a server's bounded
/// cache behaves like once a chart has settled on one point.
#[derive(Debug, Default)]
struct Warm {
    cells: BTreeMap<CellKey, Vec<SystemRecord>>,
}

impl CellCache for Warm {
    fn with_cell<R>(
        &mut self,
        galaxy: &Galaxy,
        key: CellKey,
        f: impl FnOnce(&[SystemRecord]) -> R,
    ) -> R {
        let cell = self.cells.entry(key).or_insert_with(|| {
            let mut systems = Vec::new();
            generate_cell(galaxy, key, &mut systems);
            systems
        });
        f(cell)
    }
}

/// The expected counts over a sphere, the census's input: the quadrature of P03.T9.c.
fn expected(c: &mut Criterion) {
    let galaxy = galaxy();
    let sun = sunlike_point();
    let mut group = c.benchmark_group("query");
    for radius in [50.0, 5_000.0] {
        let radius = LightYears::new(radius);
        group.bench_function(format!("expected_counts ({} ly)", radius.value()), |b| {
            b.iter(|| expected_counts(black_box(&galaxy), black_box(&sun), black_box(radius)));
        });
    }
    group.finish();
}

/// The brainstorm's local query: 50 ly at the Sun-like point, with no cache and with a warm one.
fn local(c: &mut Criterion) {
    let galaxy = galaxy();
    let query = RangeQuery::builder(sunlike_point(), LightYears::new(50.0))
        .build()
        .expect("50 ly at the Sun-like point is a query");
    let mut group = c.benchmark_group("query");
    group.bench_function("range_50ly_cold", |b| {
        let mut cache = NoCache::new();
        b.iter(|| range_query(black_box(&galaxy), &mut cache, &[], black_box(&query)));
    });

    // Filling the cache first is what makes the warm case warm. What the query answers is in the
    // module docs, so that the figures say what work they cover; these assertions pin it.
    let mut warm = Warm::default();
    let filled = range_query(&galaxy, &mut warm, &[], &query);
    assert_eq!(filled.census().complete_down_to(), Some(Layer::A));
    assert_eq!(filled.stats().cells_visited(), 1_732);
    assert_eq!(warm.cells.len(), 1_732);
    group.bench_function("range_50ly_warm", |b| {
        b.iter(|| range_query(black_box(&galaxy), &mut warm, &[], black_box(&query)));
    });
    group.finish();
}

/// The census limit this bench raises the default 4,096 to, so that the long-range query walks its
/// two layers instead of being stopped before layer E.
///
/// At 500 ly from the Sun-like point the fixture expects 9,094 systems in layer E and 32,886 in
/// layer D, so the default limit admits nothing at all and the query generates no cell (see the
/// module docs). This is the smallest power of two that admits both.
const LONG_RANGE_LIMIT: NonZeroU32 = NonZeroU32::new(1 << 16).expect("65,536 is not zero");

/// A long-range query with a mass floor: 500 ly down to layer D only.
fn long_range(c: &mut Criterion) {
    let galaxy = galaxy();
    let query = RangeQuery::builder(sunlike_point(), LightYears::new(500.0))
        .mass_floor(MassFloor::LayerD)
        .limit(LONG_RANGE_LIMIT)
        .build()
        .expect("500 ly with a mass floor is a query");
    // What the query answers, recorded in the module docs so that the figure says what work it
    // covers.
    let walked = range_query(&galaxy, &mut NoCache::new(), &[], &query);
    assert_eq!(walked.census().complete_down_to(), Some(Layer::D));
    assert_eq!(walked.systems().len(), 41_925);
    assert_eq!(walked.stats().cells_visited(), 3_008);
    let mut group = c.benchmark_group("query");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(20));
    group.bench_function("range_500ly_floor_d", |b| {
        let mut cache = NoCache::new();
        b.iter(|| range_query(black_box(&galaxy), &mut cache, &[], black_box(&query)));
    });
    group.finish();
}

criterion_group!(query, expected, local, long_range);
criterion_main!(query);
