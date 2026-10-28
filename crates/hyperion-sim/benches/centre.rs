//! Benchmarks of plan 09's galactic centre (P09.T24). Targets are the plan's; a miss is a
//! finding, not a CI failure: CI compiles these and never runs them.
//!
//! - The four Eddington inversions together (Design note 14's stars, black holes and young disc,
//!   and ruling 144.6's isotropic young stars), with the profile they share and the galaxy's
//!   monopole it holds (ruling 144's joint revision), from a built mass model: the plan's 50 ms
//!   was set for three, in the potential of the black hole and the cluster alone.
//! - `CentreModel::new`, which adds the marks' acceptances and the classes: under 100 ms (Design
//!   note 15's budget for building it with the galaxy).
//! - The century query with cached orbits (P09.T29): the centre source's systems within 0.1 ly of
//!   the black hole a century from now, every cell's members and orbits already in a
//!   `KeepCentreOrbits`, so that only the cells' choice and the propagation are timed: "about 10
//!   ms" (brainstorm, "The range query").

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use hyperion_sim::Seed;
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::features::centre::{
    CentreMemberSource, CentreModel, CentreProfile, KeepCentreOrbits,
};
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::potential::MassModel;
use hyperion_sim::galaxy::query::{LayerSet, QuerySphere, SystemSource};
use hyperion_sim::id::Layer;
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::LightYears;

fn centre(c: &mut Criterion) {
    let params = GalaxyParams::milky_way_like();
    let mut group = c.benchmark_group("centre");
    group.sample_size(20);
    let model = MassModel::new(&params);
    group.bench_function("profile_and_four_inversions", |b| {
        b.iter(|| {
            let profile = CentreProfile::new(
                black_box(params.nuclear_cluster()),
                params.black_hole().mass(),
            )
            .expect("the fixture's profile")
            .in_galaxy(black_box(&model));
            CentreModel::invert(&profile).expect("the fixture's centre inverts")
        });
    });
    let galaxy = Galaxy::from_params(Seed::new(0x0926_0000), params).expect("the fixture builds");
    group.bench_function("model", |b| {
        b.iter(|| CentreModel::new(black_box(&galaxy)).expect("the fixture's centre builds"));
    });
    group.finish();
}

fn centre_query(c: &mut Criterion) {
    let galaxy = Galaxy::from_params(Seed::new(0x0929_0000), GalaxyParams::milky_way_like())
        .expect("the fixture builds");
    let orbits = KeepCentreOrbits::new(&galaxy);
    let source = CentreMemberSource::new(&galaxy, &orbits).expect("the fixture's centre builds");
    let century = UniverseTime::from_julian_years(100).expect("in the clock window");
    let sphere = QuerySphere::new(
        GalacticPosition::ORIGIN,
        LightYears::new(0.1),
        century,
        LightYears::ZERO,
    )
    .expect("a positive radius");
    let layers = [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E]
        .into_iter()
        .fold(LayerSet::EMPTY, LayerSet::with);
    let mut hits = Vec::new();
    // The cold query places every cell's members, once; the bench then times the warm one.
    source.systems_in_sphere(&galaxy, &sphere, layers, &mut hits);
    eprintln!(
        "century query: {} hits, {} members cached and propagated",
        hits.len(),
        orbits.members()
    );
    let mut group = c.benchmark_group("centre_query");
    group.sample_size(20);
    group.bench_function("century_query_cached", |b| {
        b.iter(|| {
            hits.clear();
            source.systems_in_sphere(&galaxy, black_box(&sphere), layers, &mut hits);
            hits.len()
        });
    });
    group.finish();
}

criterion_group!(benches, centre, centre_query);
criterion_main!(benches);
