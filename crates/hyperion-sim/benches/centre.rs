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
//! - A trajectory's preparation and its state at a time, under the two-body law
//!   (`Trajectory::of`) and under the exact orbit in the centre's potential
//!   (`Trajectory::in_potential`, P09.T28.d.b), over 64 members from 0.05 to 5 ly at
//!   eccentricities to 0.98: no target, the two-body law's cost is the reference.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use hyperion_sim::Seed;
use hyperion_sim::coords::{GalacticPosition, GalacticVelocity};
use hyperion_sim::galaxy::features::centre::{
    CentreMemberSource, CentreModel, CentreProfile, KeepCentreOrbits,
};
use hyperion_sim::galaxy::motion::spherical::CentrePotential;
use hyperion_sim::galaxy::motion::{CentreMotion, EpochState, Trajectory};
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{SystemOrigin, SystemRecord};
use hyperion_sim::galaxy::potential::MassModel;
use hyperion_sim::galaxy::query::{LayerSet, QuerySphere, SystemSource};
use hyperion_sim::galaxy::{Galaxy, Population};
use hyperion_sim::id::{CentreMemberId, Layer, MemberSlot, SystemId};
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::consts::METRES_PER_LIGHT_YEAR;
use hyperion_sim::units::{GravitationalParameter, LightYears, SolarMasses, Years};

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

/// 64 centre members' epoch states, from 0.05 to 5 ly, at 0.1 to 1.3 times the circular speed
/// about the mass inside, so that their eccentricities run to about 0.98.
fn members(motion: &CentreMotion) -> Vec<EpochState> {
    let slot = MemberSlot::InCell {
        band: Layer::A,
        level: 4,
        cell: [0, 0, 0],
        index: 0,
    };
    let id = SystemId::from(CentreMemberId::new(slot).expect("a canonical slot"));
    (0..64_u32)
        .map(|k| {
            let k = f64::from(k);
            let r = 0.05 * hyperion_sim::math::exp10(2.0 * (k / 64.0));
            let factor = 0.1 + 1.2 * ((k * 0.618_034) % 1.0);
            let mu =
                GravitationalParameter::from_solar_masses(motion.enclosed_mass(LightYears::new(r)));
            let speed = factor * (mu.value() / (r * METRES_PER_LIGHT_YEAR)).sqrt();
            let (sin, cos) = hyperion_sim::math::sin_cos(0.3 * k);
            let record = SystemRecord::from_parts(
                id,
                GalacticPosition::from_light_years([r * cos, r * sin, 0.1 * r])
                    .expect("near the centre"),
                SystemOrigin::CentreMember { attempt: 0 },
                Population::NuclearDisc,
                SolarMasses::new(1.0),
                Years::new(1e9),
            );
            EpochState::new(
                record,
                GalacticVelocity::new([-speed * sin, speed * cos, 0.2 * speed]),
            )
            .expect("a finite state below light's")
        })
        .collect()
}

fn trajectories(c: &mut Criterion) {
    let params = GalaxyParams::milky_way_like();
    let motion =
        CentreMotion::new(&params, &MassModel::new(&params)).expect("the fixture's centre");
    let potential = CentrePotential::new(&motion);
    let starts = members(&motion);
    let t = UniverseTime::from_julian_years(-700).expect("in the clock window");
    let mut group = c.benchmark_group("trajectory");
    group.sample_size(30);
    group.bench_function("two_body_of_64", |b| {
        b.iter(|| {
            starts
                .iter()
                .map(|start| Trajectory::of(black_box(&motion), *start))
                .collect::<Vec<_>>()
        });
    });
    group.bench_function("exact_in_potential_64", |b| {
        b.iter(|| {
            starts
                .iter()
                .map(|start| Trajectory::in_potential(&motion, black_box(&potential), *start))
                .collect::<Vec<_>>()
        });
    });
    let two_body: Vec<Trajectory> = starts.iter().map(|s| Trajectory::of(&motion, *s)).collect();
    let exact: Vec<Trajectory> = starts
        .iter()
        .map(|s| Trajectory::in_potential(&motion, &potential, *s))
        .collect();
    group.bench_function("two_body_state_at_64", |b| {
        b.iter(|| {
            two_body
                .iter()
                .map(|tr| tr.state_at(black_box(t)))
                .collect::<Vec<_>>()
        });
    });
    group.bench_function("exact_state_at_64", |b| {
        b.iter(|| {
            exact
                .iter()
                .map(|tr| tr.state_at(black_box(t)))
                .collect::<Vec<_>>()
        });
    });
    group.bench_function("potential", |b| {
        b.iter(|| CentrePotential::new(black_box(&motion)));
    });
    group.finish();
}

criterion_group!(benches, centre, centre_query, trajectories);
criterion_main!(benches);
