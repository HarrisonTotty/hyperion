//! Benchmarks of plan 12's retarded observation and microlensing (P12.T1, P12.T4.b). Targets are
//! the plan's; a miss is a finding, not a CI failure: CI compiles these and never runs them.
//!
//! - `retarded`: one fixed-point step for a drifting source, tens of nanoseconds (plan 12,
//!   Verification).
//! - `lens_walk_26kly`: the lens walk from the Sun to a bulge source at Baade's window, 26,000 ly,
//!   over a year, in a galaxy whose systems move (the cone of Design note 13 as built) and in one
//!   whose systems keep their epoch positions (the plan's thin tube).

use std::hint::black_box;
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use hyperion_sim::Seed;
use hyperion_sim::coords::{GalacticPosition, GalacticVelocity};
use hyperion_sim::events::TimeWindow;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{CellKey, NoCache, generate_cell};
use hyperion_sim::id::Layer;
use hyperion_sim::lensing::{LensQuery, LensSightline, lenses_along};
use hyperion_sim::observe::{Drift, Observer, retarded};
use hyperion_sim::time::UniverseTime;

fn observe(c: &mut Criterion) {
    let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in range");
    let observer = Observer::new(sun, UniverseTime::EPOCH).expect("inside");
    let star = Drift::new(
        GalacticPosition::from_light_years([1_234.5, -22_000.25, 310.0]).expect("in range"),
        GalacticVelocity::new([-180e3, 95e3, 12e3]),
    );
    let mut group = c.benchmark_group("observe");
    group.bench_function("retarded", |b| {
        b.iter(|| retarded(black_box(&observer), black_box(&star)));
    });
    group.finish();
}

fn lensing(c: &mut Criterion) {
    let still = Galaxy::from_params(Seed::new(0x1204_b100), GalaxyParams::milky_way_like())
        .expect("the fixture builds");
    let moving = Galaxy::from_params(Seed::new(0x1204_b100), GalaxyParams::milky_way_like())
        .expect("the fixture builds")
        .with_full_potential();
    let mut cell = Vec::new();
    // Baade's window: 3.9° south of the centre seen from the Sun, 1,770 ly below the plane.
    generate_cell(
        &still,
        CellKey::new(Layer::C, [0, 0, -56]).expect("inside"),
        &mut cell,
    );
    let source = cell.first().expect("a bulge cell holds systems").id();
    let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in range");
    let year = UniverseTime::from_julian_years(1).expect("in range");
    let window = TimeWindow::new(UniverseTime::EPOCH, year).expect("ordered");
    let query = LensQuery::builder(LensSightline::new(sun, source), window)
        .build()
        .expect("a valid query");
    let mut group = c.benchmark_group("lensing");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(60));
    group.bench_function("lens_walk_26kly_still", |b| {
        b.iter(|| lenses_along(&still, &mut NoCache::new(), &[], black_box(&query)));
    });
    group.bench_function("lens_walk_26kly", |b| {
        b.iter(|| lenses_along(&moving, &mut NoCache::new(), &[], black_box(&query)));
    });
    group.finish();
}

criterion_group!(benches, observe, lensing);
criterion_main!(benches);
