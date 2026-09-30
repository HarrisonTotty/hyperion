//! Benchmarks of plan 09's galactic centre (P09.T24). Targets are the plan's; a miss is a
//! finding, not a CI failure: CI compiles these and never runs them.
//!
//! - The four Eddington inversions together (Design note 14's stars, black holes and young disc,
//!   and ruling 144.6's isotropic young stars), with the profile they share and the galaxy's
//!   monopole it holds (ruling 144's joint revision), from a built mass model: the plan's 50 ms
//!   was set for three, in the potential of the black hole and the cluster alone.
//! - `CentreModel::new`, which adds the marks' acceptances and the classes: under 100 ms (Design
//!   note 15's budget for building it with the galaxy).

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use hyperion_sim::Seed;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::features::centre::{CentreModel, CentreProfile};
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::potential::MassModel;

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

criterion_group!(benches, centre);
criterion_main!(benches);
