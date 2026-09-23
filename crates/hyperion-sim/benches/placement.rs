//! Benchmarks of placement: a cell's systems, and one ID resolved (plan 03, P03.T11).
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
//! Bare nanoseconds on this laptop say more about its clock than about the code: the clock moves
//! between 1.9 and 4.8 GHz with the load, and the tables further down were taken under other
//! lanes' builds (plan 02, R21). So on 2026-09-23 the same calls were timed in one process against
//! `math::exp`, measured immediately before and after each one with its loop's cost taken off, at
//! a load average of 7–10 and 3.3–4.2 GHz (`scaling_cur_freq`), where one `math::exp` took 7.3–7.9
//! ns. Two runs agreed to 5%. The cost is given in `math::exp` calls, which the clock does not
//! move, and then in microseconds at that run's 7.3–7.9 ns. The fixture is version 8's.
//!
//! | Call | `math::exp` calls | µs at 7.3–7.9 ns |
//! | ---- | ----------------- | ---------------- |
//! | `sparse_fine_cell (Sun-like point)`, one candidate | 289–291 | 2.14–2.23 |
//! | `sparse_fine_cell (rim, 35,000 ly)`, none | 104–106 | 0.76–0.79 |
//! | `cell_by_layer (A, Sun-like point)` | 286 | 2.09–2.14 |
//! | `cell_by_layer (B, Sun-like point)` | 477–481 | 3.48–3.57 |
//! | `cell_by_layer (C, Sun-like point)` | 3,251–3,425 | 23.7–24.7 |
//! | `cell_by_layer (D, Sun-like point)` | 2,365–2,371 | 17.3–18.1 |
//! | `cell_by_layer (E, Sun-like point)` | 6,564–6,657 | 48.3–50.3 |
//! | `cell_by_layer (A, bulge at 1,000 ly)` | 79,888–80,381 | 587–598 |
//! | `resolve (sparse cell)` | 295–307 | 2.16–2.21 |
//! | `resolve (fullest layer-A cell)` | 220–227 | 1.60–1.64 |
//! | `Fields::layer_bound`, the layer-A cell at the Sun-like point | 88–89 | 0.64–0.70 |
//! | `Fields::densities`, the Sun-like point | 74–75 | 0.55–0.59 |
//! | `Stream::open` and `sample_in_band(A)` under Chabrier | 84–86 | 0.61–0.64 |
//! | `Stream::open` and a Poisson draw, mean 1.3 | 7.5–7.7 | 0.06 |
//! | `Stream::open` and an age draw | 5.8 | 0.04 |
//!
//! **A sparse fine cell costs 290 `math::exp`, 2.1–2.2 µs at this machine's best clock: the
//! brainstorm's 1–2 µs is missed by about a tenth, not by the 3.31 µs below.** It is the layer bound
//! (88), one candidate's densities (74), **its primary's mass (85)**, the Poisson draw (8), the age
//! (6) and some 30 for the four position and acceptance words, the pick and the record. Plan 02's
//! R21 put the cell at 119 `math::exp`, inside the budget, by counting the bound and the densities
//! alone; the mass draw, as dear as a whole density evaluation, is what that left out. The cheapest
//! lever is that draw, which is plan 02's sampler and moves every mass, so it is a version bump and
//! not this plan's to pull. `resolve` stays constant time, the fullest cell 0.74 times the sparse
//! one. The 3.31 µs of the old table is the same code at a lower clock: 290 × 11.4 ns.
//!
//! # As first measured, under load
//!
//! Measured 2026-09-22 on an Intel i7-8665U (4 cores, 8 threads, 1.9 GHz base, 4.8 GHz turbo), the
//! same processor plan 02's R16 and R17 used, with other lanes building on the machine at the same
//! time. Two rounds, because the load average moved a great deal between them: **round 1 at load
//! 14.3–20.9** and **round 2 at load 4.0–9.1**, against the machine's 8 threads. Round 2 is the
//! fairer of the two and the one the verdicts below use; round 1 is kept because it shows how far
//! load alone moves these figures — up to three times.
//!
//! | Bench | Target | Round 2, load 4–9 | Round 1, load 14–21 |
//! | ----- | ------ | ----------------- | ------------------- |
//! | `sparse_fine_cell (Sun-like point)` | 1–2 µs a cell | **3.31 µs, missed** | 9.88 µs |
//! | `sparse_fine_cell (rim, 35,000 ly)` | 1–2 µs a cell | 1.47 µs, met | 3.29 µs |
//! | `cell_by_layer (A, Sun-like point)` | none | 3.02 µs | 12.9 µs |
//! | `cell_by_layer (B, Sun-like point)` | none | 5.05 µs | 32.4 µs |
//! | `cell_by_layer (C, Sun-like point)` | none | 37.3 µs | 175 µs |
//! | `cell_by_layer (D, Sun-like point)` | none | 32.1 µs | 80.4 µs |
//! | `cell_by_layer (E, Sun-like point)` | none | 72.0 µs | 128 µs |
//! | `cell_by_layer (A, bulge at 1,000 ly)` | none | 844 µs | 1.72 ms |
//! | `resolve (sparse cell)` | constant time | 3.10 µs, met | 5.94 µs |
//! | `resolve (fullest layer-A cell)` | constant time | 2.23 µs, met | 5.30 µs |
//!
//! **The brainstorm's 1–2 µs for a sparse fine cell is missed, at 3.31 µs** (a load-inflated
//! figure: see the section above), and the cost sits in the two calls the target rests on rather
//! than in anything this task added (corrected above: the mass draw costs as much as either).
//! Re-run beside these benches at load 4–8, plan 02's own `Fields::densities` takes 1.35 µs at a
//! disc point against the 540–570 ns its R16 records, and `Fields::layer_bound` 1.41 µs over a
//! layer-A disc cell against R17's 543–694 ns: two to two and a half times the recorded figures, on
//! the same processor. The plan's arithmetic with today's costs gives 2.8 µs for one bound and one
//! candidate's densities before any draw, which is what the 3.31 µs is; with R16 and R17's figures
//! the same structure gives the 1.1–1.3 µs the plan expects. Whether those two benches were taken
//! on a quieter or cooler machine, or whether the T7 revision's tabulated vertical profiles made
//! the densities dearer — which the plan's first risk expects T11 to find out — is plan 02's
//! question. Nothing here was optimised to chase the target and no target was weakened to meet it.
//!
//! What the figures cover is the plan's model of a sparse cell exactly: the layer-A cell at the
//! Sun-like point draws **one** candidate and accepts it, so it costs one `Fields::layer_bound`, one
//! Poisson draw, one candidate's position, acceptance, mass and age, and one `Fields::densities`; the
//! rim cell at 35,000 ly draws **none**, so its 1.47 µs is the bound and the Poisson draw alone. The
//! two bracket the cost of a candidate at about 1.8 µs, most of it `Fields::densities`.
//!
//! `resolve` is constant time, as "Identifiers" requires: an ID in the fullest layer-A cell there is
//! — the one at the galactic centre, thousands of candidates — resolves *faster* than one in a cell
//! holding a single candidate (2.23 µs against 3.10 µs), because resolution is one bound, one Poisson
//! draw and one candidate whatever the cell holds, and the bulge's bound is the cheaper of the two.
//! The plan asks only that the two be within a small factor; they are within 1.4.
//!
//! The coarser layers cost more per cell because a coarser cell is bigger: layer E's 128 ly cell has
//! 4,096 times the volume of layer A's 8 ly cell. The order across layers is not monotone — C costs
//! more than D — because a layer's candidate count follows its share of the mass function as well as
//! its volume. The bulge cell is a layer-A cell 1,000 ly from the centre, where the density is some
//! hundreds of times the solar circle's.
use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use hyperion_sim::Seed;
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{
    CellKey, SystemRecord, candidate_count, generate_cell, resolve,
};
use hyperion_sim::id::{Layer, SystemId};

/// The seed every bench here places systems with.
const SEED: u64 = 0x0311_0000_0000_0000;

/// A point like the Sun's: in the plane, 26,000 ly out on the +y axis, clear of the bar.
///
/// The tests take this from `tests/common`, which a benchmark cannot see, so it is written out here.
fn sunlike_point() -> GalacticPosition {
    GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("26,000 ly is in the root cube")
}

/// The cell of `layer` holding the point `ly` light-years from the centre along +y, in the plane.
fn cell_at(layer: Layer, ly: f64) -> CellKey {
    let position =
        GalacticPosition::from_light_years([0.0, ly, 0.0]).expect("inside the root cube");
    CellKey::containing(layer, &position).expect("inside the root cube")
}

/// A layer-A cell at the Sun-like point, and one 35,000 ly out at the rim: the sparse fine cells the
/// brainstorm's 1–2 µs target is about.
fn sparse_fine_cell(c: &mut Criterion) {
    let galaxy = Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like());
    let mut out = Vec::new();
    let mut group = c.benchmark_group("placement");
    for (label, key) in [
        (
            "Sun-like point",
            CellKey::containing(Layer::A, &sunlike_point()).expect("inside the root cube"),
        ),
        ("rim, 35,000 ly", cell_at(Layer::A, 35_000.0)),
    ] {
        // What each cell holds, recorded in the module docs so that the figures say what work they
        // cover: the Sun-like cell draws one candidate and accepts it, the rim cell draws none.
        generate_cell(&galaxy, key, &mut out);
        let (candidates, systems) = (candidate_count(&galaxy, key), out.len());
        assert_eq!(
            (candidates, systems),
            if label == "Sun-like point" {
                (1, 1)
            } else {
                (0, 0)
            },
            "{label}"
        );
        group.bench_function(format!("sparse_fine_cell ({label})"), |b| {
            b.iter(|| generate_cell(black_box(&galaxy), black_box(key), &mut out));
        });
    }
    group.finish();
}

/// One cell of each layer at the Sun-like point, and a layer-A cell in the bulge.
fn cell_by_layer(c: &mut Criterion) {
    let galaxy = Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like());
    let sun = sunlike_point();
    let mut out = Vec::new();
    let mut group = c.benchmark_group("placement");
    for layer in [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E] {
        let key = CellKey::containing(layer, &sun).expect("inside the root cube");
        group.bench_function(format!("cell_by_layer ({layer:?}, Sun-like point)"), |b| {
            b.iter(|| generate_cell(black_box(&galaxy), black_box(key), &mut out));
        });
    }
    let bulge = cell_at(Layer::A, 1_000.0);
    group.bench_function("cell_by_layer (A, bulge at 1,000 ly)", |b| {
        b.iter(|| generate_cell(black_box(&galaxy), black_box(bulge), &mut out));
    });
    group.finish();
}

/// One ID resolved in a sparse cell and one in the fullest layer-A cell there is, which "Identifiers"
/// requires to cost the same.
fn resolve_one(c: &mut Criterion) {
    let galaxy = Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like());
    let sparse = CellKey::containing(Layer::A, &sunlike_point()).expect("inside the root cube");
    // The fullest layer-A cell is the one at the galactic centre, where every component peaks
    // (plan 03, Design note 6: about 6,000 candidates of the 65,536 a layer-A cell can hold).
    let fullest = CellKey::new(Layer::A, [0, 0, 0]).expect("the origin's cell");
    let mut group = c.benchmark_group("placement");
    for (label, key) in [("sparse cell", sparse), ("fullest layer-A cell", fullest)] {
        let id = a_system_of(&galaxy, key);
        let candidates = candidate_count(&galaxy, key);
        assert!(candidates > 0, "{label} holds no candidate to resolve");
        group.bench_function(format!("resolve ({label})"), |b| {
            b.iter(|| resolve(black_box(&galaxy), black_box(id)));
        });
    }
    group.finish();
}

/// The ID of the first system of `key`, which is a candidate that resolves.
fn a_system_of(galaxy: &Galaxy, key: CellKey) -> SystemId {
    let mut cell = Vec::new();
    generate_cell(galaxy, key, &mut cell);
    cell.first()
        .map(SystemRecord::id)
        .expect("every cell benched here holds a system")
}

criterion_group!(placement, sparse_fine_cell, cell_by_layer, resolve_one);
criterion_main!(placement);
