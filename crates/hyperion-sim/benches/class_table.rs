//! Benchmarks of plan 08's class table (P08.T9), split by stage (ruling 128.5). Targets are the
//! plan's; a miss is a finding, not a CI failure: CI compiles these and never runs them.
//!
//! - `ClassTable::build` at the Milky Way fixture: P08.T9's 150 ms, inside P08.T16's 1 s
//!   `kinematic_tables_build` budget, which binds.
//! - Its stages per band-E mass node: `kick_bins::speed_bin_shares` (the kick quadrature: one or two
//!   tracks' fates, the seam's `binarity::stripped_share` and the remnant branches),
//!   `binarity::stripped_share` alone, a full track, and `stellar::lifetime`.
//! - `LifetimeBracket::new`, 858 lifetimes, galaxy-independent; `LifetimeBracket::shared` keeps one
//!   a process.

use std::hint::black_box;
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use hyperion_sim::Seed;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::displaced::GalaxyScales;
use hyperion_sim::galaxy::displaced::class_table::{ClassTable, FormTable};
use hyperion_sim::galaxy::displaced::kick_bins::speed_bin_shares;
use hyperion_sim::galaxy::displaced::marks::LifetimeBracket;
use hyperion_sim::galaxy::displaced::{BirthSource, binarity};
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::stellar::draws::StarDraws;
use hyperion_sim::stellar::remnant::StandardKickLaw;
use hyperion_sim::stellar::sse::Track;
use hyperion_sim::stellar::{Composition, lifetime};
use hyperion_sim::units::SolarMasses;

/// The class table and its stages at the fixture.
fn class_table(c: &mut Criterion) {
    let galaxy = Galaxy::from_params(Seed::new(8), GalaxyParams::milky_way_like())
        .expect("the fixture builds")
        .with_full_potential();
    let forms = FormTable::committed();
    let scales = GalaxyScales::new(galaxy.params(), galaxy.potential());
    let law = StandardKickLaw::default();
    let comp = Composition::SOLAR;
    let mut group = c.benchmark_group("class_table");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(30));
    group.bench_function("ClassTable::build", |b| {
        b.iter(|| ClassTable::build(black_box(&galaxy), black_box(&forms)));
    });
    for m in [8.2, 15.0, 40.0] {
        let mass = SolarMasses::new(m);
        group.bench_function(format!("speed_bin_shares/{m}"), |b| {
            b.iter(|| {
                speed_bin_shares(&law, black_box(mass), &comp, &scales, BirthSource::ThinDisc)
            });
        });
        group.bench_function(format!("stripped_share/{m}"), |b| {
            b.iter(|| binarity::stripped_share(black_box(mass), &comp));
        });
        group.bench_function(format!("Track::full/{m}"), |b| {
            b.iter(|| Track::full(black_box(mass), &comp, &StarDraws::median()));
        });
        group.bench_function(format!("lifetime/{m}"), |b| {
            b.iter(|| lifetime(black_box(mass), &comp, &StarDraws::median()));
        });
    }
    group.bench_function("LifetimeBracket::new", |b| b.iter(LifetimeBracket::new));
    group.finish();
}

criterion_group!(benches, class_table);
criterion_main!(benches);
