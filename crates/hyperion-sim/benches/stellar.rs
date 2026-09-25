//! Benchmarks of the stellar stage (plan 06).
//!
//! Targets are recorded in plan 06's "Verification": `ZCoeffs::new` under 5 µs; `stellar::lifetime`
//! about 5 µs for a primary of layer D or E (2.5–8 and 8–150 M☉, plan 03), which plan 08's
//! placement calls up to twice per accepted record; and a remnant's full track within the 150 µs
//! that plan 06 gives `generate` for a remnant. A regression is a finding to raise, not a CI
//! failure: CI compiles these and never runs them.
//!
//! Every track figure is also given in calls of `math::exp`, timed in the same process before and
//! after the other groups (`stellar/reference`), because this laptop's clock swings 1.9–4.8 GHz
//! and other work shares it; a per-call cost only means something against that. The measurement
//! is recorded in plan 06's "Deviations in T10.c–e, as built".
//!
//! The `stellar/system` and `stellar/briefs` groups are P06.T32's remaining benches and P06.T38.a's
//! measurements: `generate` for four kinds of primary found near the Sun-like point, `brief_at` and
//! `summary_at` on a built system, the draws, hierarchy and classification a range row's brief is
//! made of, and the briefs of every row of `range_query.rs`'s 50 ly query through today's exact
//! path. Plan 06's budget for those briefs is 15 ms per 1,600 rows; the results are recorded
//! under P06.T38.a.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use hyperion_sim::Seed;
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{NoCache, SystemRecord};
use hyperion_sim::galaxy::query::{MassFloor, RangeQuery, range_query};
use hyperion_sim::id::{BodyId, Layer};
use hyperion_sim::math;
use hyperion_sim::stellar::brief::{BriefModel, range_brief};
use hyperion_sim::stellar::classify::{ClassExtras, classify};
use hyperion_sim::stellar::draws::StarDraws;
use hyperion_sim::stellar::multiplicity::{
    MultiplicityContext, RedrawAttempt, draw_hierarchy, draw_star_count,
};
use hyperion_sim::stellar::sse::{Track, ZCoeffs, main_sequence_state, zams};
use hyperion_sim::stellar::system::{SystemStars, draw_metallicity};
use hyperion_sim::stellar::{Composition, ObjectKind, evolve, lifetime};
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::{LightYears, MetalFraction, SolarMasses, Years};

/// One `math::exp`, the unit the track figures are normalised by, before the other groups.
fn exp_before(c: &mut Criterion) {
    let mut group = c.benchmark_group("stellar/reference");
    group.bench_function("math::exp (before)", |b| {
        b.iter(|| math::exp(black_box(0.5)));
    });
    group.finish();
}

/// `math::powf` against `math::powf_positive`, the stellar formulae's power (ruling 77.1), at a
/// mass-luminosity law's arguments.
fn powers(c: &mut Criterion) {
    let mut group = c.benchmark_group("stellar/reference");
    group.bench_function("math::powf", |b| {
        b.iter(|| math::powf(black_box(5.3), black_box(3.8)));
    });
    group.bench_function("math::powf_positive", |b| {
        b.iter(|| math::powf_positive(black_box(5.3), black_box(3.8)));
    });
    group.finish();
}

/// [`exp_before`] again, after every other group.
fn exp_after(c: &mut Criterion) {
    let mut group = c.benchmark_group("stellar/reference");
    group.bench_function("math::exp (after)", |b| {
        b.iter(|| math::exp(black_box(0.5)));
    });
    group.finish();
}

/// Every metallicity-dependent coefficient at one Z, and the zero-age main sequence from them.
fn backbone(c: &mut Criterion) {
    let mut group = c.benchmark_group("stellar");
    group.bench_function("zcoeffs_new", |b| {
        b.iter(|| ZCoeffs::new(black_box(MetalFraction::new(0.004))));
    });
    let coeffs = ZCoeffs::new(MetalFraction::new(0.004));
    group.bench_function("zams_luminosity_and_radius", |b| {
        b.iter(|| {
            let m = black_box(SolarMasses::new(3.7));
            (zams::luminosity(m, &coeffs), zams::radius(m, &coeffs))
        });
    });
    group.finish();
}

/// The track integrator (P06.T10.c–e) under the generator's options: the lifetime of layer D and E
/// primaries, tracks built to an age for a main-sequence dwarf and a giant, full tracks of stars
/// dead at the epoch, and one state of a built track.
fn tracks(c: &mut Criterion) {
    let solar = Composition::SOLAR;
    let draws = StarDraws::median();
    let mut group = c.benchmark_group("stellar/track");
    // P06.T32's four masses beside the layer D and E primaries. `ZCoeffs` is always built inside
    // the call: no public entry takes one already built.
    for (name, m) in [
        ("lifetime (1 Msun)", 1.0),
        ("lifetime (4 Msun, layer D)", 4.0),
        ("lifetime (5 Msun)", 5.0),
        ("lifetime (12 Msun)", 12.0),
        ("lifetime (20 Msun, layer E)", 20.0),
        ("lifetime (40 Msun)", 40.0),
    ] {
        group.bench_function(name, |b| {
            b.iter(|| lifetime(black_box(SolarMasses::new(m)), &solar, &draws));
        });
    }
    for (name, m, age) in [
        ("to_age (0.4 Msun dwarf at 5 Gyr)", 0.4, 5e9),
        ("to_age (1 Msun at 4.6 Gyr)", 1.0, 4.6e9),
        ("to_age (2 Msun giant at 1.2 Gyr)", 2.0, 1.2e9),
    ] {
        group.bench_function(name, |b| {
            b.iter(|| {
                Track::to_age(
                    black_box(SolarMasses::new(m)),
                    &solar,
                    &draws,
                    Years::new(age),
                )
            });
        });
    }
    for (name, m) in [
        ("full (1 Msun, white dwarf)", 1.0),
        ("full (5 Msun, white dwarf)", 5.0),
        ("full (20 Msun, neutron star)", 20.0),
    ] {
        group.bench_function(name, |b| {
            b.iter(|| Track::full(black_box(SolarMasses::new(m)), &solar, &draws));
        });
    }
    // `evolve` at a typical age for its mass: a dwarf, the Sun, an intermediate-mass star on its
    // main sequence and a massive one halfway through its main sequence.
    for (name, m, age) in [
        ("evolve (0.3 Msun at 5 Gyr)", 0.3, 5e9),
        ("evolve (1 Msun at 4.6 Gyr)", 1.0, 4.6e9),
        ("evolve (2 Msun at 0.6 Gyr)", 2.0, 6e8),
        ("evolve (20 Msun at 4 Myr)", 20.0, 4e6),
    ] {
        group.bench_function(name, |b| {
            b.iter(|| {
                evolve(
                    black_box(SolarMasses::new(m)),
                    &solar,
                    &draws,
                    Years::new(age),
                )
            });
        });
    }
    let sun = Track::full(SolarMasses::new(1.0), &solar, &draws);
    group.bench_function("state_at (1 Msun at 4.6 Gyr)", |b| {
        b.iter(|| sun.state_at(black_box(Years::new(4.6e9))));
    });
    let giant = Track::full(SolarMasses::new(5.0), &solar, &draws);
    let late = giant
        .lifetime()
        .map_or(Years::new(1e8), |l| Years::new(l.value() * 0.999));
    group.bench_function("state_at (5 Msun on the AGB)", |b| {
        b.iter(|| giant.state_at(black_box(late)));
    });
    group.finish();
}

/// The seed of the system benches: `range_query.rs`'s, so that the 50 ly query is the one whose
/// cost plan 03 records.
const SEED: u64 = 0x0311_1000_0000_0000;

/// A point like the Sun's: in the plane, 26,000 ly out on the +y axis, clear of the bar.
fn sunlike_point() -> GalacticPosition {
    GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("26,000 ly is in the root cube")
}

/// The records of a query of `radius` about the Sun-like point, down to `floor`.
fn records_near_sun(galaxy: &Galaxy, radius: f64, floor: MassFloor) -> Vec<SystemRecord> {
    let query = RangeQuery::builder(sunlike_point(), LightYears::new(radius))
        .mass_floor(floor)
        .build()
        .expect("a query about the Sun-like point");
    range_query(galaxy, &mut NoCache::new(), &[], &query)
        .systems()
        .iter()
        .map(|hit| *hit.record())
        .collect()
}

/// The first of `records` whose primary is of `kind` at the epoch, and, for a dwarf, in layer A.
fn exemplar(galaxy: &Galaxy, records: &[SystemRecord], kind: ObjectKind) -> Option<SystemRecord> {
    records.iter().copied().find(|record| {
        (kind != ObjectKind::Dwarf || record.layer() == Layer::A)
            && SystemStars::generate(galaxy, record)
                .brief_at(UniverseTime::EPOCH)
                .is_some_and(|brief| brief.kind() == kind)
    })
}

/// The galaxy of the system benches, the records of its 50 ly query about the Sun-like point, and
/// a primary of each of four kinds.
struct Fixture {
    galaxy: Galaxy,
    local: Vec<SystemRecord>,
    exemplars: [(&'static str, SystemRecord); 4],
}

fn fixture() -> Fixture {
    let galaxy = Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral");
    let local = records_near_sun(&galaxy, 50.0, MassFloor::default());
    // The rarer kinds are looked for among the heavier layers of a wider sphere.
    let heavy = records_near_sun(&galaxy, 150.0, MassFloor::LayerD);
    let find = |kind| {
        exemplar(&galaxy, &local, kind)
            .or_else(|| exemplar(&galaxy, &heavy, kind))
            .expect("the Sun's neighbourhood holds a primary of every kind benched")
    };
    let exemplars = [
        ("layer-A dwarf", find(ObjectKind::Dwarf)),
        ("giant", find(ObjectKind::Giant)),
        ("white dwarf", find(ObjectKind::WhiteDwarf)),
        ("neutron star", find(ObjectKind::NeutronStar)),
    ];
    Fixture {
        galaxy,
        local,
        exemplars,
    }
}

/// A system's stars and a range row's brief (P06.T32, and P06.T38.a's measurements): `generate` for
/// four kinds of primary, `brief_at` and `summary_at` on a built system, the draws and the
/// classification a brief is made of, and the briefs of a whole 50 ly query through today's exact
/// path.
fn systems(c: &mut Criterion) {
    let Fixture {
        galaxy,
        local,
        exemplars,
    } = fixture();

    let mut group = c.benchmark_group("stellar/system");
    for (name, record) in &exemplars {
        group.bench_function(format!("generate ({name})"), |b| {
            b.iter(|| SystemStars::generate(&galaxy, black_box(record)));
        });
    }
    for (name, record) in &exemplars {
        let stars = SystemStars::generate(&galaxy, record);
        group.bench_function(format!("brief_at ({name})"), |b| {
            b.iter(|| stars.brief_at(black_box(UniverseTime::EPOCH)));
        });
        group.bench_function(format!("summary_at ({name})"), |b| {
            b.iter(|| stars.summary_at(black_box(UniverseTime::EPOCH)));
        });
    }
    let (_, dwarf) = exemplars[0];
    group.bench_function("draw_metallicity", |b| {
        b.iter(|| draw_metallicity(&galaxy, black_box(&dwarf)));
    });
    group.bench_function("draw_hierarchy", |b| {
        b.iter(|| {
            draw_hierarchy(
                &galaxy,
                black_box(&dwarf),
                MultiplicityContext::Free,
                RedrawAttempt::FIRST,
            )
        });
    });
    group.bench_function("StarDraws::for_star", |b| {
        b.iter(|| StarDraws::for_star(galaxy.seed(), black_box(BodyId::new(dwarf.id(), 0))));
    });
    for (name, record) in &exemplars {
        let stars = SystemStars::generate(&galaxy, record);
        let primary = stars.primary();
        let state = primary
            .state_at(UniverseTime::EPOCH)
            .expect("an exemplar exists at the epoch");
        group.bench_function(format!("classify ({name})"), |b| {
            b.iter(|| {
                classify(
                    black_box(&state),
                    primary.composition(),
                    primary.draws(),
                    &ClassExtras::NONE,
                )
            });
        });
    }
    group.finish();

    let mut group = c.benchmark_group("stellar/briefs");
    group.sample_size(10);
    group.bench_function(format!("50 ly query, {} rows, cold", local.len()), |b| {
        b.iter(|| {
            local
                .iter()
                .filter_map(|record| {
                    SystemStars::generate(&galaxy, record).brief_at(UniverseTime::EPOCH)
                })
                .count()
        });
    });
    group.finish();
}

/// P06.T38.b and T38.e: the routed brief of the same 50 ly query, cold and from built models, the
/// count-only hierarchy, the main-sequence fast path, and a brief model of each exemplar.
fn routed(c: &mut Criterion) {
    let Fixture {
        galaxy,
        local,
        exemplars,
    } = fixture();
    let (_, dwarf) = exemplars[0];
    let mut group = c.benchmark_group("stellar/briefs");
    group.sample_size(10);
    group.bench_function(
        format!("50 ly query, {} rows, routed, cold", local.len()),
        |b| {
            b.iter(|| {
                local
                    .iter()
                    .filter_map(|record| range_brief(&galaxy, record, UniverseTime::EPOCH))
                    .count()
            });
        },
    );
    let models: Vec<BriefModel> = local
        .iter()
        .map(|record| BriefModel::new(&galaxy, record))
        .collect();
    group.bench_function(
        format!("50 ly query, {} rows, routed, warm", local.len()),
        |b| {
            b.iter(|| {
                models
                    .iter()
                    .filter_map(|model| model.brief_at(black_box(UniverseTime::EPOCH)))
                    .count()
            });
        },
    );
    group.finish();

    let mut group = c.benchmark_group("stellar/routed");
    group.bench_function("draw_star_count (layer-A dwarf)", |b| {
        b.iter(|| {
            draw_star_count(
                &galaxy,
                black_box(&dwarf),
                MultiplicityContext::Free,
                RedrawAttempt::FIRST,
            )
        });
    });
    group.bench_function("main_sequence_state (0.4 Msun at 5 Gyr)", |b| {
        b.iter(|| {
            main_sequence_state(
                black_box(SolarMasses::new(0.4)),
                &Composition::SOLAR,
                &StarDraws::median(),
                Years::new(5e9),
            )
        });
    });
    for (name, record) in &exemplars {
        group.bench_function(format!("BriefModel::new ({name})"), |b| {
            b.iter(|| BriefModel::new(&galaxy, black_box(record)));
        });
    }
    group.finish();
}

criterion_group!(
    stellar, exp_before, powers, backbone, tracks, systems, routed, exp_after
);
criterion_main!(stellar);
