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
//! # Re-measured in validation, against a yardstick
//!
//! Timed on 2026-09-23 in one process against `math::exp`, as `placement.rs` describes, at a load
//! average of 7–10 and 3.3–4.2 GHz, where one `math::exp` took 7.3–7.9 ns; two runs agreed to 3%.
//! The query ran at `placement.rs`'s seed, where the 50 ly sphere holds 1,419 systems in the same
//! 1,732 cells. Version 8's fixture.
//!
//! | Call | `math::exp` calls | at 7.3–7.9 ns | Target |
//! | ---- | ----------------- | ------------- | ------ |
//! | `expected_counts (50 ly)` | 20,169–21,175 | 147–149 µs | under 0.3 ms: met |
//! | `expected_counts (5000 ly)` | 2.76–2.85 M | 20.1–20.6 ms | none |
//! | `range_50ly_cold` | 1.01–1.02 M | 7.18–7.32 ms | under 5 ms: **missed by 1.4×** |
//! | `range_50ly_warm` | 104,500 | 0.72–0.74 ms | none |
//! | `range_500ly_floor_d` (limit 65,536) | 11.85–11.93 M | 86–89 ms | none |
//!
//! **The cold query misses 5 ms by 1.4 times, not the four times of the table below**, which was the
//! same code at a third of the clock. The miss is the sparse-cell miss `placement.rs` records,
//! multiplied over the walk: of the 1.02 M, the primaries' mass draws alone are some 120,000.
//!
//! # As first measured, under load
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
//!   the long walk the plan asks for: 41,925 systems over 3,008 cells then, 37,675 at generator
//!   version 15 and 37,668 at 21 (see "The long-range pin" below). That the default limit stops a
//!   500 ly disc query before layer E is a finding about the census rule at long range, not a change
//!   to it — Design note 9's limit is on expected counts, and a 500 ly sphere in the disc is simply
//!   past it.
//!
//! # The long-range pin
//!
//! The walk's size is pinned, so that its figure says what work it covers. Nothing runs the bench in
//! CI, so the pin went stale unnoticed; perf08 re-measured it on 2026-09-29 at generator version
//! 15: **37,675 systems (29,424 in layer D, 8,251 in E) over the same 3,008 cells**, against
//! expected counts of 29,545 and 8,170, in 205–222 ms at a load of 16. Two changes to the fixture
//! moved it, and no change to the walk:
//!
//! - P02.T11's retune of `milky_way_like()` (2026-09-23: a thin-disc scale length of 7,000 ly for
//!   8,480, thicker discs) lowered the density at the Sun-like point by about a quarter. The pin of
//!   41,925, taken on the fixture before it, fell to the 30,606 that `sub13b` found at version 14,
//!   as plan 03's own Sun-area goldens fell with it (`query/range.golden`'s 20 ly sphere, 93
//!   systems to 71).
//! - Version 15's refit of Chabrier's high-mass scale, 0.68 to 0.92 (P15.T4.b, ruling 138), raised
//!   bands D and E's share of systems by 30%, 0.0335 to 0.0437, and the walk's count by 23%.
//!
//! Rendering plan R06.T17 re-measured it at generator version 21 (2026-10-11, criterion's `--test`;
//! the pin had gone stale again): **37,668 systems (29,412 in D, 8,256 in E) over the same 3,008
//! cells**, 7 fewer than at version 15, against the same expected counts, which the realised count
//! now matches to 0.13% (0.24σ of Poisson), in 72.7 ms (criterion's median of 20 samples, under the
//! heavy-test lock at a load of 5–10, so provisional). The sky's caps' count, which integrates the
//! same field by the trapezoid in ln r, gives 1.1% more (R06's Risks, "Deviations in T17, as
//! built").
//!
//! At generator version 27 (P12.T13, whose benches share this binary, so the stale pin stopped
//! them): **33,387 systems (28,299 in D, 5,088 in E) over the same 3,008 cells**; the features hold
//! most young stars from 27 (P09.T2.c), and layer E falls by 38%.
//!
//! # Observed mode (plan 12, P12.T3)
//!
//! `range_observed_50ly` is the 50 ly query observed from its own centre with its cells and its
//! 917 systems' stars warm, so it measures the observation alone against `range_50ly_warm`:
//! 4.19–4.79 ms against 1.13 ms on 2026-09-30 at load 9–11, about 4 µs a system, of which the
//! brief at the emitted time is 44%, the stated curvature error 27% and the retarded step 18%.
//! `range_observed_50ly_cold` builds every system's stars as well: 240 ms against
//! `range_50ly_cold`'s 9.45 ms. Plan 12's Risks record the figures for ruling 143.4. At 27
//! (P12.T13; pod, 48 vCPU, loaded): 4.04 ms warm over 910 systems, and 296 ms cold.
//!
//! `local_events_50ly_day` and `local_events_50ly_year` are plan 12's local events (P12.T5.e) over
//! the same observed rows, warm, for a sensor with V, K and radio bands, over a day and a year from
//! the query's time: what they add to the observed query, against the plan's under 1 ms, a miss
//! being a finding. Plan 12's Risks ("P12.T5.e as built") name the lever.
//!
//! `alerts_in_sun_day` is plan 12's "what went off?" (P12.T5.d) for the same rows and a sensor with
//! an X-ray band as well, over a day, with the global list lent: the census of every class (the
//! supernova classes reach the whole Galaxy, so they wait for the background, uncounted), the
//! centre's flares and disruptions, and the local events, warm. The galaxy has its kinematic
//! tables, which the global list needs.
//!
//! `alerts_interactive` is plan 12's bench of that name (P12.T13): every class galaxy-wide for the
//! Sun-like observer over the alert service's year of lookahead, through the client's stand-in
//! sensor, with the global list and no local rows, as the service's interactive scan asks it.
//! Its target is under 200 ms; a miss is a finding. At 27 (P12.T13; pod, 48 vCPU, loaded): 106 ms,
//! no detections in the year, beside `alerts_in_sun_day`'s 120 ms.
use std::collections::BTreeMap;
use std::hint::black_box;
use std::num::NonZeroU32;
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use hyperion_sim::Seed;
use hyperion_sim::alerts::local::LocalEventsCache;
use hyperion_sim::alerts::{
    AlertBand, AlertContext, AlertQuery, BandLimits, SensorHorizon, alerts_in, local_events,
};
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::events::TimeWindow;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::gas::ccm::Band;
use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
use hyperion_sim::galaxy::gas::noise::NoiseCache;
use hyperion_sim::galaxy::global_list::GlobalList;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{CellCache, CellKey, NoCache, SystemRecord, generate_cell};
use hyperion_sim::galaxy::query::{
    MassFloor, QueryMode, RangeQuery, SubstellarRequest, expected_counts, range_query,
    range_query_observed,
};
use hyperion_sim::id::{Layer, SystemId};
use hyperion_sim::observe::{NoStarsCache, StarsCache};
use hyperion_sim::stellar::system::SystemStars;
use hyperion_sim::time::{Span, UniverseTime};
use hyperion_sim::units::{LightYears, WattsPerSquareMetre};

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
        .expect("the Milky Way fixture's gas is mostly neutral")
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

/// A stars cache holding every system it has been asked for: the warm case of the observed query.
#[derive(Debug, Default)]
struct WarmStars {
    stars: BTreeMap<SystemId, SystemStars>,
}

impl StarsCache for WarmStars {
    fn with_stars<R>(
        &mut self,
        galaxy: &Galaxy,
        record: &SystemRecord,
        f: impl FnOnce(&SystemStars) -> R,
    ) -> R {
        f(self
            .stars
            .entry(record.id())
            .or_insert_with(|| SystemStars::generate(galaxy, record)))
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
            b.iter(|| {
                expected_counts(
                    black_box(&galaxy),
                    black_box(&sun),
                    black_box(radius),
                    SubstellarRequest::None,
                )
            });
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
/// At 500 ly from the Sun-like point the fixture expects 8,170 systems in layer E and 29,545 in
/// layer D (generator version 15), so the default limit admits nothing at all and the query generates no cell (see the
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
    let in_layer = |layer: Layer| {
        walked
            .systems()
            .iter()
            .filter(|hit| hit.record().layer() == layer)
            .count()
    };
    eprintln!(
        "range_500ly_floor_d: {} systems ({} in D, {} in E) over {} cells",
        walked.systems().len(),
        in_layer(Layer::D),
        in_layer(Layer::E),
        walked.stats().cells_visited()
    );
    assert_eq!(walked.census().complete_down_to(), Some(Layer::D));
    // The sky's caps test holds the same count, `tests/sky_caps.rs`' `RANGE_REALISED`: a re-pin
    // moves both.
    assert_eq!(walked.systems().len(), 33_387);
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

/// The brainstorm's local query in observed mode (plan 12, P12.T3), seen by a sensor at the
/// query's centre: warm, against `range_50ly_warm`, so that the difference is the observation of
/// each system (the retarded step, the brief at the emitted time and the stated error) with no
/// generation; and cold, against `range_50ly_cold`, where every system's stars are built as well.
fn observed(c: &mut Criterion) {
    let galaxy = galaxy();
    let query = RangeQuery::builder(sunlike_point(), LightYears::new(50.0))
        .mode(QueryMode::ObservedFrom(sunlike_point()))
        .build()
        .expect("50 ly at the Sun-like point is a query");
    let mut cells = Warm::default();
    let mut stars = WarmStars::default();
    let filled = range_query_observed(&galaxy, &mut cells, &mut stars, &[], &query)
        .expect("the Sun-like point holds no centre member");
    assert_eq!(filled.observed().len(), filled.systems().len());
    assert_eq!(stars.stars.len(), filled.systems().len());
    let mut group = c.benchmark_group("query");
    group.bench_function("range_observed_50ly", |b| {
        b.iter(|| {
            range_query_observed(
                black_box(&galaxy),
                &mut cells,
                &mut stars,
                &[],
                black_box(&query),
            )
        });
    });
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(30));
    group.bench_function("range_observed_50ly_cold", |b| {
        let mut cold = NoCache::new();
        b.iter(|| {
            range_query_observed(
                black_box(&galaxy),
                &mut cold,
                &mut NoStarsCache,
                &[],
                black_box(&query),
            )
        });
    });
    group.finish();
}

/// Plan 12's local events (P12.T5.e) over the observed 50 ly query's rows, with their stars and the
/// noise warm, for a sensor with V and K limits of 10⁻¹⁵ W m⁻² and a radio band, over a day and a
/// year from the query's time: what they add to the observed query.
fn local_events_50ly(c: &mut Criterion) {
    let galaxy = galaxy();
    let query = RangeQuery::builder(sunlike_point(), LightYears::new(50.0))
        .mode(QueryMode::ObservedFrom(sunlike_point()))
        .build()
        .expect("50 ly at the Sun-like point is a query");
    let mut stars = WarmStars::default();
    let found = range_query_observed(&galaxy, &mut Warm::default(), &mut stars, &[], &query)
        .expect("the Sun-like point holds no centre member");
    let w = WattsPerSquareMetre::new;
    let limits = |detect| BandLimits::new(w(detect), w(detect * 100.0)).expect("positive limits");
    let sensor = SensorHorizon::new([
        (AlertBand::Photometric(Band::V), limits(1e-15)),
        (AlertBand::Photometric(Band::K), limits(1e-15)),
        (AlertBand::Photometric(Band::Radio), limits(1e-28)),
    ])
    .expect("three bands");
    let mut noise = NoiseCache::with_capacity(4_096);
    let mut group = c.benchmark_group("alerts");
    for (name, days) in [
        ("local_events_50ly_day", 1),
        ("local_events_50ly_year", 365),
    ] {
        let end = UniverseTime::EPOCH
            .checked_add(Span::from_seconds(days * 86_400))
            .expect("a year after the epoch is on the clock");
        let window = TimeWindow::new(UniverseTime::EPOCH, end).expect("a window");
        let alerts = AlertQuery::builder(sunlike_point(), window, sensor.clone())
            .build()
            .expect("the window lies in the clock window");
        let mut seen = Vec::new();
        let mut ctx = AlertContext::new(&mut noise, &NoModifiers);
        let mut cache = LocalEventsCache::with_capacity(4_096);
        local_events(
            &galaxy,
            &mut ctx,
            &alerts,
            found.observed(),
            &mut stars,
            &mut cache,
            &mut seen,
        );
        eprintln!(
            "{name}: {} detections from {} systems",
            seen.len(),
            found.observed().len()
        );
        group.bench_function(name, |b| {
            b.iter(|| {
                seen.clear();
                local_events(
                    black_box(&galaxy),
                    &mut ctx,
                    black_box(&alerts),
                    found.observed(),
                    &mut stars,
                    &mut cache,
                    &mut seen,
                );
            });
        });
    }
    group.finish();
}

/// Plan 12's `alerts_in` (P12.T5.d) for one Sun-like observer's day: the observed 50 ly query's
/// rows, warm, a sensor with V and K limits of 10⁻¹⁵ W m⁻², a radio band and an X-ray limit of
/// 10⁻¹⁵ W m⁻², and the galaxy's global list.
fn alerts_in_sun_day(c: &mut Criterion) {
    let galaxy = galaxy().with_full_potential();
    let list = GlobalList::build(&galaxy);
    let query = RangeQuery::builder(sunlike_point(), LightYears::new(50.0))
        .mode(QueryMode::ObservedFrom(sunlike_point()))
        .build()
        .expect("50 ly at the Sun-like point is a query");
    let mut stars = WarmStars::default();
    let found = range_query_observed(&galaxy, &mut Warm::default(), &mut stars, &[], &query)
        .expect("the Sun-like point holds no centre member");
    let w = WattsPerSquareMetre::new;
    let limits = |detect| BandLimits::new(w(detect), w(detect * 100.0)).expect("positive limits");
    let sensor = SensorHorizon::new([
        (AlertBand::Photometric(Band::V), limits(1e-15)),
        (AlertBand::Photometric(Band::K), limits(1e-15)),
        (AlertBand::Photometric(Band::Radio), limits(1e-28)),
        (AlertBand::XRay, limits(1e-15)),
    ])
    .expect("four bands");
    let day = UniverseTime::EPOCH
        .checked_add(Span::from_seconds(86_400))
        .expect("a day after the epoch is on the clock");
    let window = TimeWindow::new(UniverseTime::EPOCH, day).expect("a window");
    let alerts = AlertQuery::builder(sunlike_point(), window, sensor)
        .build()
        .expect("the window lies in the clock window");
    let mut noise = NoiseCache::with_capacity(4_096);
    let mut ctx = AlertContext::new(&mut noise, &NoModifiers).with_global_list(&list);
    let mut cache = LocalEventsCache::with_capacity(4_096);
    let result = alerts_in(
        &galaxy,
        &mut ctx,
        &alerts,
        found.observed(),
        &mut stars,
        &mut cache,
    );
    eprintln!(
        "alerts_in_sun_day: {} detections from {} systems; census {:?}; sources {:?}",
        result.detections().len(),
        found.observed().len(),
        result
            .census()
            .iter()
            .map(|c| (c.class().name(), c.status(), c.expected_hosts()))
            .collect::<Vec<_>>(),
        result.sources(),
    );
    let mut group = c.benchmark_group("alerts");
    group.bench_function("alerts_in_sun_day", |b| {
        b.iter(|| {
            alerts_in(
                black_box(&galaxy),
                &mut ctx,
                black_box(&alerts),
                found.observed(),
                &mut stars,
                &mut cache,
            )
        });
    });
    group.finish();
}

/// Plan 12's `alerts_interactive` (P12.T13; plan 12's Verification): the interactive part of
/// "what went off?" galaxy-wide, as the alert service's interactive scan asks it (P12.T8.b): every
/// class, for the Sun-like observer over the service's year of lookahead, through the client's
/// stand-in sensor (V detecting at magnitude 17 and resolving at 12, X-rays at 10 mCrab and 1 Crab;
/// `lib/useAlerts.ts`), with the global list lent and no local rows. The classes whose walks
/// reach past the census's cells wait for the background, uncounted; the rest, the centre among
/// them, are walked.
fn alerts_interactive(c: &mut Criterion) {
    let galaxy = galaxy().with_full_potential();
    let list = GlobalList::build(&galaxy);
    let w = WattsPerSquareMetre::new;
    // V's zero point through Bessell's 840 Å top hat, and the Crab's 2–10 keV flux (Kirsch et al.
    // 2005), as the client's stand-in sensor reads them.
    let v = |magnitude: f64| 3.631e-11 * 84.0 * hyperion_sim::math::exp10(-0.4 * magnitude);
    let crab = 2.4e-11;
    let sensor = SensorHorizon::new([
        (
            AlertBand::Photometric(Band::V),
            BandLimits::new(w(v(17.0)), w(v(12.0))).expect("limits in order"),
        ),
        (
            AlertBand::XRay,
            BandLimits::new(w(0.01 * crab), w(crab)).expect("limits in order"),
        ),
    ])
    .expect("two bands");
    let year = UniverseTime::EPOCH
        .checked_add(Span::from_seconds(31_557_600))
        .expect("a year after the epoch is on the clock");
    let window = TimeWindow::new(UniverseTime::EPOCH, year).expect("a window");
    let alerts = AlertQuery::builder(sunlike_point(), window, sensor)
        .build()
        .expect("the window lies in the clock window");
    let mut noise = NoiseCache::with_capacity(4_096);
    let mut ctx = AlertContext::new(&mut noise, &NoModifiers).with_global_list(&list);
    let mut cache = LocalEventsCache::with_capacity(4_096);
    let result = alerts_in(
        &galaxy,
        &mut ctx,
        &alerts,
        &[],
        &mut NoStarsCache,
        &mut cache,
    );
    eprintln!(
        "alerts_interactive: {} detections; census {:?}; sources {:?}",
        result.detections().len(),
        result
            .census()
            .iter()
            .map(|c| (c.class().name(), c.status(), c.expected_hosts()))
            .collect::<Vec<_>>(),
        result.sources(),
    );
    let mut group = c.benchmark_group("alerts");
    group.bench_function("alerts_interactive", |b| {
        b.iter(|| {
            alerts_in(
                black_box(&galaxy),
                &mut ctx,
                black_box(&alerts),
                &[],
                &mut NoStarsCache,
                &mut cache,
            )
        });
    });
    group.finish();
}

criterion_group!(
    query,
    expected,
    local,
    long_range,
    observed,
    local_events_50ly,
    alerts_in_sun_day,
    alerts_interactive
);
criterion_main!(query);
