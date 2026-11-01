//! The tests of P14.T29's strip radius and its hold ([`STRIP_VERSION`]).

use std::sync::OnceLock;

use hyperion_testkit::float::{assert_same_bits, bits};
use hyperion_testkit::order::assert_order_independent;

use super::*;
use crate::Seed;
use crate::coords::{GalacticPosition, GalacticVelocity};
use crate::galaxy::Population;
use crate::galaxy::features::FeatureProcess;
use crate::galaxy::features::catalogue::NoFeatureCache;
use crate::galaxy::features::members::{KeepInteriors, MemberRecord, NoInteriorCache};
use crate::galaxy::imf::MassBand;
use crate::galaxy::motion::{Regime, regime_of};
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::{CellKey, STELLAR_LAYERS, generate_cell};
use crate::galaxy::query::{QuerySphere, cells_in_sphere};
use crate::id::{CentreMemberId, Layer, MemberSlot, SystemId};
use crate::orbit::Eccentricity;
use crate::planetary::record::BodyKind;
use crate::planetary::testing::with_every_26_hold;
use crate::planetary::{PlanetarySystem, generate, generate_planets};
use crate::time::UniverseTime;
use crate::units::{AstronomicalUnits, GravitationalParameter, KilometresPerSecond};

const SEED: Seed = Seed::new(0x0e14_0029_0000_0001);

/// The Milky Way fixture under the version's rules (the Kepler regime among them), with its
/// kinematic tables, which feature members and grid velocities need. Built once per test binary.
fn kepler_galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(
            Seed::new(0x0e14_0029_0000_0002),
            GalaxyParams::milky_way_like(),
        )
        .expect("the Milky Way fixture builds")
        .with_full_potential()
    })
}

fn au(x: f64) -> Metres {
    Metres::from(AstronomicalUnits::new(x))
}

fn in_au(m: Metres) -> f64 {
    AstronomicalUnits::from(m).value()
}

/// A solar-cell ID for synthetic hosts, so that their streams differ.
fn id(index: u32) -> SystemId {
    CellKey::new(Layer::C, [0, 812, 0])
        .unwrap()
        .candidate_id(index)
        .unwrap()
}

/// A Sun-like synthetic host of `age` years.
fn sun(index: u32, age: f64) -> crate::planetary::context::SystemContextBuilder {
    SystemContext::builder()
        .system(id(index))
        .star(SolarMasses::new(1.0))
        .age_at_epoch(Years::new(age))
}

/// A globular core's environment: `n` systems per cubic light-year at 10 km/s, of 0.45 M☉.
fn core(n: f64) -> EncounterEnvironment {
    EncounterEnvironment::new(
        PerCubicLightYear::new(n),
        KilometresPerSecond::new(10.0),
        SolarMasses::new(0.45),
    )
    .unwrap()
}

/// Asserts that nothing `system` generated for `ctx` lies beyond its strip radius: every planet's
/// and dwarf planet's apoapsis, every belt's outer edge and the halo's. Returns the planets and
/// dwarf planets checked.
fn assert_inside(ctx: &SystemContext, system: &PlanetarySystem) -> usize {
    let strip = ctx.strip_radius();
    let mut checked = 0;
    for body in system.bodies() {
        if matches!(body.kind(), BodyKind::Planet | BodyKind::DwarfPlanet)
            && let Some(orbit) = body.orbit()
        {
            assert!(
                orbit.apoapsis() <= strip,
                "{:?} of {:?}: apoapsis {} au beyond the strip radius {} au",
                body.index(),
                ctx.id(),
                in_au(orbit.apoapsis()),
                in_au(strip)
            );
            checked += 1;
        }
    }
    for belt in system.belts() {
        assert!(
            belt.outer_edge() <= strip * (1.0 + 1e-12),
            "a belt of {:?} reaches {} au, beyond {} au",
            ctx.id(),
            in_au(belt.outer_edge()),
            in_au(strip)
        );
    }
    if let Some(halo) = system.halo() {
        assert!(halo.outer_edge() <= strip);
    }
    checked
}

/// The epoch state of a system at `r` ly from the black hole along +x on a circular orbit about
/// the mass inside `r`, moving along +y.
fn circular_state(galaxy: &Galaxy, r: f64) -> EpochState {
    let mu = GravitationalParameter::from_solar_masses(
        galaxy.centre_motion().enclosed_mass(LightYears::new(r)),
    );
    let speed = (mu.value() / (r * METRES_PER_LIGHT_YEAR)).sqrt();
    let slot = MemberSlot::InCell {
        band: Layer::A,
        level: 4,
        cell: [0, 0, 0],
        index: 0,
    };
    let record = SystemRecord::from_parts(
        SystemId::from(CentreMemberId::new(slot).unwrap()),
        GalacticPosition::from_light_years([r, 0.0, 0.0]).unwrap(),
        SystemOrigin::CentreMember { attempt: 0 },
        Population::NuclearDisc,
        SolarMasses::new(1.0),
        Years::new(4.57e9),
    );
    EpochState::new(record, GalacticVelocity::new([0.0, speed, 0.0])).unwrap()
}

/// The first `count` members of each of bands A, C and E of `interior`, from its innermost cells
/// out.
fn members_of(galaxy: &Galaxy, interior: &FeatureInterior, count: usize) -> Vec<MemberRecord> {
    let mut out = Vec::new();
    let mut cell = Vec::new();
    for band in [MassBand::A, MassBand::C, MassBand::E] {
        let mut taken = 0;
        for c in interior.grid().owned_cells() {
            interior.members_in_cell(galaxy, band, c, &mut cell);
            for (member, _) in cell.iter().take(count - taken) {
                out.push(*member);
                taken += 1;
            }
            if taken == count {
                break;
            }
        }
    }
    out
}

/// The first globular of the Kepler fixture with members, and its interior.
fn globular() -> &'static FeatureInterior {
    static INTERIOR: OnceLock<FeatureInterior> = OnceLock::new();
    INTERIOR.get_or_init(|| {
        let galaxy = kepler_galaxy();
        FeatureCatalogue::walk_process(galaxy, FeatureProcess::Globular)
            .find_map(|f| FeatureInterior::of(galaxy, &f))
            .expect("the fixture has a globular with members")
    })
}

/// The first feature with members within 3,000 ly of the Sun in the Kepler fixture: an open
/// cluster or a nursery.
fn near_the_sun() -> &'static FeatureInterior {
    static INTERIOR: OnceLock<FeatureInterior> = OnceLock::new();
    INTERIOR.get_or_init(|| {
        let galaxy = kepler_galaxy();
        let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).unwrap();
        FeatureCatalogue::near(galaxy, &sun, LightYears::new(3_000.0), &NoFeatureCache)
            .find_map(|f| FeatureInterior::of(galaxy, &f))
            .expect("a feature with members lies within 3,000 ly of the Sun")
    })
}

/// The grid systems of the Kepler fixture inside its influence radius, the Kepler regime's.
fn kepler_regime_systems() -> &'static [SystemRecord] {
    static RECORDS: OnceLock<Vec<SystemRecord>> = OnceLock::new();
    RECORDS.get_or_init(|| {
        let galaxy = kepler_galaxy();
        let edge = galaxy.centre_motion().influence_radius();
        let centre = GalacticPosition::from_light_years([0.0, 0.0, 0.0]).unwrap();
        let sphere = QuerySphere::new(centre, edge, UniverseTime::EPOCH, LightYears::ZERO).unwrap();
        let mut out = Vec::new();
        let mut cell = Vec::new();
        for spec in STELLAR_LAYERS {
            for key in cells_in_sphere(spec.layer(), &sphere) {
                generate_cell(galaxy, key, &mut cell);
                out.extend(cell.iter().filter(|r| {
                    matches!(r.origin(), SystemOrigin::Grid(_))
                        && regime_of(galaxy, r) == Regime::Kepler
                }));
            }
        }
        out
    })
}

/// A few grid systems of the Kepler fixture at the solar circle, of layers A, C and E.
fn solar_systems() -> Vec<SystemRecord> {
    let galaxy = kepler_galaxy();
    let mut out = Vec::new();
    let mut cell = Vec::new();
    for layer in [Layer::A, Layer::C, Layer::E] {
        let size = i32::try_from(layer.cell_size_ly()).unwrap();
        generate_cell(
            galaxy,
            CellKey::new(layer, [0, 26_000 / size, 0]).unwrap(),
            &mut cell,
        );
        out.extend(cell.iter().take(4));
    }
    out
}

// The hold.

/// P14.T29 is held until the 25 → 26 bump: every context is version 24's and 25's.
#[test]
fn the_stripping_is_held_only_until_the_26_bump() {
    assert!(
        GENERATOR_VERSION < STRIP_VERSION,
        "the 25 → 26 bump puts P14.T29's stripping on the record: delete `STRIP_VERSION`, \
         `StripRules` with its `Held` arm and the arms that read it (`strip_radius` and \
         `Surroundings::of_record`), the tests' `with_stripping`, its switch in \
         `with_every_26_hold` and this test, and add the galaxy README's relaxation 14 to the \
         relaxations removed"
    );
    assert_eq!(StripRules::in_force(), StripRules::Held);
    assert_eq!(
        with_stripping(StripRules::Stripped, StripRules::in_force),
        StripRules::Stripped
    );
    assert_eq!(StripRules::in_force(), StripRules::Held);
    assert_eq!(
        with_every_26_hold(StripRules::in_force),
        StripRules::Stripped
    );
}

/// Under the hold every context is version 24's and 25's to the bit: plan 02's radius at the epoch
/// position, even in the Kepler regime and for a feature member, no encounter environment, and an
/// environment given to a synthetic host not applied.
#[test]
fn under_the_hold_every_context_is_version_24_s() {
    let galaxy = kepler_galaxy();
    let mut records: Vec<SystemRecord> = kepler_regime_systems().iter().take(8).copied().collect();
    records.extend(
        members_of(galaxy, globular(), 2)
            .iter()
            .map(|m| *m.record()),
    );
    assert!(records.len() > 4, "{} records", records.len());
    for record in &records {
        let ctx = SystemContext::from_record(galaxy, record);
        let plan_02 = galaxy.potential().tidal_radius(
            ctx.hierarchy().unwrap().system_mass(),
            &PointLy::from(record.epoch_position()),
        );
        assert_same_bits(ctx.tidal_radius().value(), plan_02.value());
        assert_same_bits(
            ctx.strip_radius().value(),
            plan_02.value() * SATELLITE_STABILITY_FRACTION,
        );
        assert_eq!(ctx.encounter_environment(), None);
    }
    let given = sun(0, 1.2e10)
        .encounter_environment(core(1e4))
        .build()
        .unwrap();
    assert_same_bits(
        given.strip_radius().value(),
        given.tidal_radius().value() * SATELLITE_STABILITY_FRACTION,
    );
}

// The encounter environment.

/// A member reads its cluster's environment at its place, or from inside the core at the core's
/// edge on the same line from the centre (the module's lean): never the density of a cusp's
/// centre.
#[test]
fn a_member_reads_its_environment_at_its_place_or_its_core_s_edge() {
    let galaxy = kepler_galaxy();
    let (mut inside, mut outside) = (0, 0);
    for interior in [globular(), near_the_sun()] {
        let core = interior.model().profile_core_radius().value();
        for member in members_of(galaxy, interior, 6) {
            let local = local_offset(
                interior.feature().position(),
                member.record().epoch_position(),
            );
            let r = (local.x * local.x + local.y * local.y + local.z * local.z).sqrt();
            let place = if r < core {
                inside += 1;
                PointLy::new(local.x * core / r, local.y * core / r, local.z * core / r)
            } else {
                outside += 1;
                local
            };
            let ctx = with_stripping(StripRules::Stripped, || {
                SystemContext::from_record(galaxy, member.record())
            });
            let environment = ctx
                .encounter_environment()
                .expect("a member has neighbours");
            let n = interior.table().density(&place);
            assert!(
                (environment.number_density().value() / n - 1.0).abs() < 1e-12,
                "{} against {n} at {r} ly, the core {core} ly",
                environment.number_density().value()
            );
            assert_same_bits(
                environment.velocity_dispersion().value(),
                interior.model().sigma(LightYears::new(r.max(core))).value(),
            );
        }
    }
    assert!(
        inside > 0 && outside > 0,
        "{inside} inside the core, {outside} outside"
    );
}

// The encounter radius.

/// The radius solves one removing encounter in the age, Γ(a) × age = 1, the rate at
/// [`ENCOUNTER_REMOVAL_FACTOR`] of every encounter within a, to rounding, from the sparse geometric
/// regime to the dense focused one.
#[test]
fn the_encounter_radius_is_one_encounter_in_the_age() {
    let root_pi = core::f64::consts::PI.sqrt();
    for (n, sigma, mean, mass, age) in [
        (1e4, 10.0, 0.45, 1.0, 1.2e10),
        (0.3, 1.0, 0.5, 1.0, 1.0e9),
        (1e-3, 30.0, 0.3, 0.2, 1.0e8),
        (50.0, 3.0, 0.4, 2.0, 5.0e8),
    ] {
        let environment = EncounterEnvironment::new(
            PerCubicLightYear::new(n),
            KilometresPerSecond::new(sigma),
            SolarMasses::new(mean),
        )
        .unwrap();
        let a = encounter_radius(&environment, SolarMasses::new(mass), Years::new(age)).unwrap();
        let b = a.value() / METRES_PER_LIGHT_YEAR;
        let m = mass + mean;
        let rate = ENCOUNTER_REMOVAL_FACTOR
            * 4.0
            * root_pi
            * n
            * sigma
            * b
            * b
            * (1.0 + G * m / (2.0 * sigma * sigma * b))
            * LIGHT_YEARS_PER_YEAR_PER_KM_S;
        assert!(
            (rate * age - 1.0).abs() < 1e-12,
            "n {n}: {} encounters within {} au",
            rate * age,
            in_au(a)
        );
    }
}

/// The two limits: where focusing dominates, `σ ÷ (2√π f G M n t)`; where it is negligible,
/// `1 ÷ √(4√π f n σ t)`, f the [`ENCOUNTER_REMOVAL_FACTOR`].
#[test]
fn the_encounter_radius_takes_its_two_limits() {
    let root_pi = core::f64::consts::PI.sqrt();
    let k = LIGHT_YEARS_PER_YEAR_PER_KM_S;
    let f = ENCOUNTER_REMOVAL_FACTOR;
    let focused = encounter_radius(&core(1e5), SolarMasses::new(1.0), Years::new(1.2e10)).unwrap();
    let limit = 10.0 / (2.0 * root_pi * f * G * 1.45 * 1e5 * 1.2e10 * k);
    let ratio = focused.value() / METRES_PER_LIGHT_YEAR / limit;
    assert!((0.99..1.0).contains(&ratio), "{ratio}");
    let fast = EncounterEnvironment::new(
        PerCubicLightYear::new(1e-2),
        KilometresPerSecond::new(300.0),
        SolarMasses::new(0.3),
    )
    .unwrap();
    let geometric = encounter_radius(&fast, SolarMasses::new(0.3), Years::new(1e10)).unwrap();
    let limit = 1.0 / (4.0 * root_pi * f * 1e-2 * 300.0 * 1e10 * k).sqrt();
    let ratio = geometric.value() / METRES_PER_LIGHT_YEAR / limit;
    assert!((0.99..1.0).contains(&ratio), "{ratio}");
}

/// The figures of the module documentation: about 0.108 au in a globular core of 10⁴ systems per
/// cubic light-year at 10 km/s after 12 Gyr, and about 1,380 au in an open cluster of 0.3 at 1
/// km/s after 1 Gyr. A system no older than the epoch has had no encounter.
#[test]
fn the_encounter_radius_of_a_core_and_of_an_open_cluster() {
    let globular = encounter_radius(&core(1e4), SolarMasses::new(1.0), Years::new(1.2e10)).unwrap();
    assert!(
        (0.106..0.109).contains(&in_au(globular)),
        "{} au",
        in_au(globular)
    );
    let open = EncounterEnvironment::new(
        PerCubicLightYear::new(0.3),
        KilometresPerSecond::new(1.0),
        SolarMasses::new(0.5),
    )
    .unwrap();
    let cluster = encounter_radius(&open, SolarMasses::new(1.0), Years::new(1e9)).unwrap();
    assert!(
        (1_370.0..1_390.0).contains(&in_au(cluster)),
        "{} au",
        in_au(cluster)
    );
    // Denser, older or slower keeps less.
    let denser = encounter_radius(&core(2e4), SolarMasses::new(1.0), Years::new(1.2e10)).unwrap();
    assert!(denser < globular);
    let younger = encounter_radius(&core(1e4), SolarMasses::new(1.0), Years::new(1e9)).unwrap();
    assert!(younger > globular);
    for age in [0.0, -1e6, f64::NAN] {
        assert_eq!(
            encounter_radius(&core(1e4), SolarMasses::new(1.0), Years::new(age)),
            None
        );
    }
}

// The plan's three figures, under the 26 arm.

/// A system on a circular orbit 0.01 ly from a Milky Way-mass black hole has a sphere of
/// influence of 2.7 au, the brainstorm's figure, and nothing beyond it; nothing is generated
/// beyond 0.49 of it, 1.32 au.
#[test]
fn a_hundredth_of_a_light_year_from_the_black_hole_keeps_nothing_beyond_2_7_au() {
    let galaxy = kepler_galaxy();
    let state = circular_state(galaxy, 0.01);
    let m = SolarMasses::new(1.0);
    let sphere = tidal_radius(galaxy, &state, m);
    assert!((in_au(sphere) - 2.7).abs() < 0.05, "{} au", in_au(sphere));
    let mut checked = 0;
    with_every_26_hold(|| {
        for i in 0..24 {
            let ctx = sun(i, 4.57e9).tidal_radius(sphere).build().unwrap();
            let strip = ctx.strip_radius();
            assert!((in_au(strip) - 1.32).abs() < 0.03, "{} au", in_au(strip));
            let system = generate(SEED, &ctx);
            checked += assert_inside(&ctx, &system);
            for body in system.bodies() {
                if let Some(orbit) = body.orbit()
                    && matches!(body.kind(), BodyKind::Planet | BodyKind::DwarfPlanet)
                {
                    assert!(orbit.apoapsis() <= sphere);
                }
            }
        }
    });
    assert!(checked > 10, "{checked} planets inside 1.32 au");
}

/// A member of a globular core of 10⁴ systems per cubic light-year keeps nothing beyond a few au:
/// its encounter cut, about 0.11 au after 12 Gyr, is far inside its galactic sphere of influence.
#[test]
fn a_member_of_a_dense_globular_core_keeps_nothing_beyond_a_few_au() {
    with_every_26_hold(|| {
        for i in 0..24 {
            let ctx = sun(i, 1.2e10)
                .encounter_environment(core(1e4))
                .build()
                .unwrap();
            let strip = ctx.strip_radius();
            assert!(in_au(strip) < 3.0, "{} au", in_au(strip));
            assert!(strip < ctx.tidal_radius() * 1e-5);
            assert_inside(&ctx, &generate(SEED, &ctx));
        }
    });
}

/// A field star at 26,000 ly is unaffected inside 10⁴ au: outside the Kepler regime its context
/// is the held one, bit for bit, its strip radius beyond 10⁴ au, and its planets the same.
#[test]
fn a_field_star_at_26_000_ly_is_unaffected_inside_10_4_au() {
    let galaxy = kepler_galaxy();
    for record in solar_systems() {
        assert_eq!(regime_of(galaxy, &record), Regime::Straight);
        let held = SystemContext::from_record(galaxy, &record);
        let stripped = with_stripping(StripRules::Stripped, || {
            SystemContext::from_record(galaxy, &record)
        });
        assert_eq!(stripped, held);
        let strip = with_stripping(StripRules::Stripped, || stripped.strip_radius());
        assert_same_bits(strip.value(), held.strip_radius().value());
        assert!(in_au(strip) > 1e4, "{} au", in_au(strip));
        assert_eq!(
            with_stripping(StripRules::Stripped, || generate_planets(SEED, &stripped)),
            generate_planets(SEED, &held)
        );
    }
}

/// A free-floating brown dwarf (P14.T27.a's host, under every 26 hold) takes the field rule at its
/// own mass: outside the Kepler regime 0.4895 of plan 02's radius at its epoch position, bit for
/// bit, beyond 10⁴ au, no encounter cut, and nothing generated beyond it.
#[test]
fn a_free_floating_brown_dwarf_takes_the_field_rule_at_its_own_mass() {
    let galaxy = kepler_galaxy();
    let size = i32::try_from(Layer::BrownDwarf.cell_size_ly()).unwrap();
    let mut cell = Vec::new();
    generate_cell(
        galaxy,
        CellKey::new(Layer::BrownDwarf, [0, 26_000 / size, 0]).unwrap(),
        &mut cell,
    );
    assert!(!cell.is_empty(), "layer F is empty at the solar circle");
    with_every_26_hold(|| {
        for record in cell.iter().take(6) {
            let ctx = SystemContext::from_record(galaxy, record);
            assert_eq!(ctx.host_kind(), crate::planetary::HostKind::BrownDwarf);
            let plan_02 = galaxy.potential().tidal_radius(
                ctx.hierarchy().unwrap().system_mass(),
                &PointLy::from(record.epoch_position()),
            );
            assert_same_bits(ctx.tidal_radius().value(), plan_02.value());
            assert_eq!(ctx.encounter_environment(), None);
            let strip = ctx.strip_radius();
            assert_same_bits(
                strip.value(),
                plan_02.value() * SATELLITE_STABILITY_FRACTION,
            );
            assert!(in_au(strip) > 1e4, "{} au", in_au(strip));
            assert_inside(&ctx, &generate(SEED, &ctx));
        }
    });
}

// Every body inside its strip radius.

/// No body lies beyond its strip radius after placement, over strip radii from inside a disc's
/// inner edge to beyond its outer one, set by the tidal cut, the encounter cut or both, for single
/// stars of 0.2–2 M☉ and binaries.
#[test]
fn no_body_lies_beyond_its_strip_radius() {
    let environments = [
        None,
        Some(core(1e4)),
        Some(core(30.0)),
        Some(
            EncounterEnvironment::new(
                PerCubicLightYear::new(0.3),
                KilometresPerSecond::new(1.0),
                SolarMasses::new(0.5),
            )
            .unwrap(),
        ),
    ];
    let mut planets = 0;
    let mut cases = 0;
    with_every_26_hold(|| {
        for (i, tidal) in [0.05, 0.4, 3.0, 20.0, 150.0, 2e3, 3e5]
            .into_iter()
            .enumerate()
        {
            for (j, environment) in environments.iter().enumerate() {
                for (k, mass) in [0.2, 1.0, 2.0].into_iter().enumerate() {
                    let index = u32::try_from(100 + 12 * i + 3 * j + k).unwrap();
                    let mut builder = SystemContext::builder()
                        .system(id(index))
                        .star(SolarMasses::new(mass))
                        .age_at_epoch(Years::new(3e9))
                        .tidal_radius(au(tidal));
                    if let Some(environment) = environment {
                        builder = builder.encounter_environment(*environment);
                    }
                    let ctx = builder.build().unwrap();
                    planets += assert_inside(&ctx, &generate(SEED, &ctx));
                    cases += 1;
                }
            }
        }
        // Binaries, wide enough to have circumstellar zones and close enough for a
        // circumbinary one, cut by their neighbours alone.
        for (j, environment) in environments.iter().enumerate() {
            for (k, separation) in [0.3, 40.0].into_iter().enumerate() {
                let index = u32::try_from(300 + 2 * j + k).unwrap();
                let mut builder = SystemContext::builder()
                    .system(id(index))
                    .binary(
                        SolarMasses::new(1.0),
                        SolarMasses::new(0.6),
                        au(separation),
                        Eccentricity::new(0.2).unwrap(),
                    )
                    .age_at_epoch(Years::new(3e9));
                if let Some(environment) = environment {
                    builder = builder.encounter_environment(*environment);
                }
                let ctx = builder.build().unwrap();
                planets += assert_inside(&ctx, &generate(SEED, &ctx));
                cases += 1;
            }
        }
    });
    assert!(
        cases > 80 && planets > 50,
        "{planets} planets in {cases} systems"
    );
}

// The record's paths.

/// `from_record`, `from_stars`, `from_stars_with` a kept interior and `for_system` give one
/// context for a grid system, inside the Kepler regime and outside it, and for a feature member,
/// under both arms.
#[test]
fn every_path_gives_one_context_under_both_arms() {
    let galaxy = kepler_galaxy();
    let mut records = solar_systems()[..2].to_vec();
    records.extend(kepler_regime_systems().iter().take(2));
    records.extend(
        members_of(galaxy, globular(), 1)
            .iter()
            .map(|m| *m.record()),
    );
    let keep = KeepInteriors::new(galaxy);
    for rules in [StripRules::Held, StripRules::Stripped] {
        with_stripping(rules, || {
            for record in &records {
                let stars = crate::observe::stars_of(galaxy, &NoInteriorCache, record)
                    .expect("a grid system or a feature member has stars");
                let from_stars = SystemContext::from_stars(galaxy, &stars);
                assert_eq!(SystemContext::from_record(galaxy, record), from_stars);
                assert_eq!(
                    SystemContext::from_stars_with(galaxy, &keep, &stars),
                    from_stars
                );
                assert_eq!(
                    SystemContext::for_system(galaxy, record.id()),
                    Ok(from_stars.clone())
                );
                if rules == StripRules::Stripped
                    && matches!(record.origin(), SystemOrigin::FeatureMember { .. })
                {
                    assert!(from_stars.encounter_environment().is_some());
                }
            }
        });
    }
}

/// D4: under the new arm the same seed gives the same contexts and the same systems, twice and in
/// any order.
#[test]
fn the_stripping_is_the_same_in_every_run_and_every_order() {
    let galaxy = kepler_galaxy();
    let mut records: Vec<SystemRecord> = kepler_regime_systems().iter().take(3).copied().collect();
    records.extend(
        members_of(galaxy, globular(), 1)
            .iter()
            .map(|m| *m.record()),
    );
    records.extend(
        members_of(galaxy, near_the_sun(), 1)
            .iter()
            .map(|m| *m.record()),
    );
    with_every_26_hold(|| {
        for record in &records {
            let first = SystemContext::from_record(galaxy, record);
            let second = SystemContext::from_record(galaxy, record);
            assert_eq!(first, second);
            assert_same_bits(first.strip_radius().value(), second.strip_radius().value());
            assert_eq!(generate(SEED, &first), generate(SEED, &second));
        }
        assert_order_independent(&records, |record| {
            SystemContext::from_record(galaxy, record).strip_radius()
        });
        // Through one cache kept across calls, with several members of one feature: the cache
        // lends each interior as it is built, whichever member asked first.
        let mut shared: Vec<SystemRecord> = members_of(galaxy, globular(), 3)
            .iter()
            .map(|m| *m.record())
            .collect();
        shared.extend(kepler_regime_systems().iter().take(2));
        let keep = KeepInteriors::new(galaxy);
        assert_order_independent(&shared, |record| {
            let stars = crate::observe::stars_of(galaxy, &keep, record)
                .expect("a grid system or a feature member has stars");
            SystemContext::from_stars_with(galaxy, &keep, &stars)
        });
    });
}

// The movers.

/// What the stripping moves of `records`: how many strip radii it cuts, the least it leaves (au),
/// and how many planetary systems' bodies change; the bodies of at most `generated` of them are
/// compared.
fn movers(galaxy: &Galaxy, records: &[SystemRecord], generated: usize) -> (usize, f64, usize) {
    let (mut cut, mut least, mut moved) = (0, f64::INFINITY, 0);
    for (i, record) in records.iter().enumerate() {
        let held = SystemContext::from_record(galaxy, record);
        let stripped = with_stripping(StripRules::Stripped, || {
            SystemContext::from_record(galaxy, record)
        });
        let strip = with_stripping(StripRules::Stripped, || stripped.strip_radius());
        assert!(strip <= held.strip_radius(), "{:?}", record.id());
        if bits(strip.value()) != bits(held.strip_radius().value()) {
            cut += 1;
            least = least.min(in_au(strip));
            if i < generated {
                let after = with_stripping(StripRules::Stripped, || {
                    generate_planets(SEED, &stripped).bodies().to_vec()
                });
                if after != generate_planets(SEED, &held).bodies() {
                    moved += 1;
                }
            }
        }
    }
    (cut, least, moved)
}

/// The systems the stripping moves: every feature member with neighbours takes an encounter cut,
/// and every grid system of the Kepler regime with an orbit its pericentre's radius. Outside the
/// regime no grid system moves (above); the counts over the whole regime are the slow test's.
#[test]
fn the_movers_are_the_members_and_the_kepler_regime() {
    let galaxy = kepler_galaxy();
    for (name, interior) in [("a globular", globular()), ("near the Sun", near_the_sun())] {
        let members: Vec<SystemRecord> = members_of(galaxy, interior, 4)
            .iter()
            .map(|m| *m.record())
            .collect();
        let (cut, least, moved) = movers(galaxy, &members, members.len());
        eprintln!(
            "members of the feature {name}: {} systems, {cut} cut (the least to {least:.4} au), \
             {moved} with their bodies moved",
            members.len()
        );
        assert_eq!(
            cut,
            members.len(),
            "every member near its core has neighbours"
        );
    }
    let sample: Vec<SystemRecord> = kepler_regime_systems().iter().take(40).copied().collect();
    let (cut, least, moved) = movers(galaxy, &sample, sample.len());
    eprintln!(
        "the first {} Kepler-regime grid systems: {cut} cut (the least to {least:.4} au), {moved} \
         with their bodies moved",
        sample.len()
    );
    assert_eq!(sample.len(), 40);
    assert!(cut > 0, "{cut} of 40 cut");
}

/// The movers counted for the record: every grid system of the Kepler regime, its bodies compared
/// for the first 2,000, and the members of the first five globulars and of the features within
/// 3,000 ly of the Sun with members, 24 of each.
#[test]
#[ignore = "slow: every grid system of the Kepler regime, and members of a dozen features"]
fn the_movers_are_counted() {
    let galaxy = kepler_galaxy();
    let kepler = kepler_regime_systems();
    let (cut, least, moved) = movers(galaxy, kepler, 2_000);
    eprintln!(
        "Kepler regime: {} grid systems, {cut} cut (the least to {least:.4} au); of the first \
         2,000, {moved} with their bodies moved",
        kepler.len()
    );
    let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).unwrap();
    let interiors: Vec<(&str, FeatureInterior)> =
        FeatureCatalogue::walk_process(galaxy, FeatureProcess::Globular)
            .filter_map(|f| FeatureInterior::of(galaxy, &f))
            .take(5)
            .map(|interior| ("globular", interior))
            .chain(
                FeatureCatalogue::near(galaxy, &sun, LightYears::new(3_000.0), &NoFeatureCache)
                    .filter_map(|f| FeatureInterior::of(galaxy, &f))
                    .take(8)
                    .map(|interior| ("near the Sun", interior)),
            )
            .collect();
    for (kind, interior) in &interiors {
        let members: Vec<SystemRecord> = members_of(galaxy, interior, 8)
            .iter()
            .map(|m| *m.record())
            .collect();
        let (cut, least, moved) = movers(galaxy, &members, members.len());
        eprintln!(
            "{kind} {:?} ({:?}): {} members, {cut} cut (the least to {least:.4} au), {moved} \
             with their bodies moved",
            interior.feature().id(),
            interior.model().kind(),
            members.len()
        );
    }
}
