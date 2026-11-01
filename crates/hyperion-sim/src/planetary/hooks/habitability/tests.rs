//! P14.T25's tests: the reference bodies' classes, each limit at its edge, the assessment as
//! `hooks_at` publishes it over the sample, and the share of FGK hosts with a `Temperate` body
//! (slow).

use super::*;
use crate::planetary::derive::solar::{PLANETS, SOLAR_AGE, historic_sun};
use crate::planetary::derive::{DerivedBody, habitable_zone, habitable_zone_of};
use crate::planetary::hooks::figures::tests::{europa, figures_of, figures_table, measured_europa};
use crate::planetary::hooks::surface::tests::{conditions_of, spin_of};
use crate::planetary::hooks::{GlobalFigures, Subsurface};
use crate::planetary::record::Section;
use crate::planetary::system::tests::{SEED, id, whole};
use crate::planetary::testing::synthetic_star;
use crate::planetary::{PlanetarySystem, SystemContext, generate};
use crate::stellar::Composition;
use crate::stellar::draws::StarDraws;
use crate::stellar::sse::Track;
use crate::time::UniverseTime;
use crate::units::consts::{EARTH_MASS_KG, METRES_PER_AU};
use crate::units::{Dex, Kelvin, SolarLuminosities, SolarMasses};

/// The Sun's zone at its age: the table's present Sun's, alone.
fn sun_zone() -> HabitableZone {
    habitable_zone_of(&[historic_sun()], &[])
}

/// What the Sun's stars give at its age: plan 06's track of 1 M☉ at solar composition with the
/// median draws, its main sequence's end less 4.57 Gyr.
fn sun_clock() -> HostClock {
    let track = Track::to_age(
        SolarMasses::new(1.0),
        &Composition::SOLAR,
        &StarDraws::median(),
        SOLAR_AGE,
    );
    let end = track
        .main_sequence_ends()
        .expect("the Sun at its age is on its main sequence");
    HostClock {
        before_main_sequence: false,
        main_sequence_left: Some(Years::new(end.value() - SOLAR_AGE.value())),
    }
}

/// The table planet `name` about the present Sun at its age, as the generator's rules derive it
/// (plan 14's Solar System table, NASA's elements), with its conditions under its own spin and
/// its figures: its inputs as `hooks_at` would gather them.
fn table_planet(name: &str, figures: &GlobalFigures) -> HabitabilityInputs {
    let body: DerivedBody = crate::planetary::derive::solar::historic(name);
    let conditions = conditions_of(&body, spin_of(name)).expect("a table planet has a surface");
    let &(_, _, _, a, e) = PLANETS
        .iter()
        .find(|(n, ..)| *n == name)
        .expect("a table planet");
    HabitabilityInputs {
        class: body.class(),
        mass: body.mass(),
        state: body.atmosphere().published_state(),
        surface: Some(reading(&conditions, figures)),
        zone: Some(sun_zone()),
        semi_major_axis: Metres::new(a * METRES_PER_AU),
        eccentricity: Eccentricity::new(e).unwrap(),
        locked: false,
        radiation: conditions.radiation(),
        host: sun_clock(),
    }
}

/// What a surface of conditions `conditions` and figures `figures` gives the assessment.
fn reading(
    conditions: &crate::planetary::hooks::SurfaceConditions,
    figures: &GlobalFigures,
) -> SurfaceReading {
    SurfaceReading {
        pressure: conditions.surface_pressure(),
        liquid_water: conditions.liquid_water(),
        subsurface_ocean: matches!(figures.subsurface_ocean(), Subsurface::Ocean(_)),
    }
}

/// The table's Earth, Mars and Venus, assessed.
fn table_assessments() -> [(&'static str, HabitabilityAssessment); 3] {
    let table = figures_table();
    ["Earth", "Mars", "Venus"].map(|name| {
        let inputs = table_planet(name, figures_of(&table, name));
        (name, HabitabilityAssessment::assess(&inputs))
    })
}

#[test]
fn earth_is_temperate_mars_marginal_and_venus_a_hostile_runaway() {
    let [earth, mars, venus] = table_assessments();
    for (name, assessment) in [earth, mars, venus] {
        eprintln!(
            "{name}: {:?} {:?}",
            assessment.class(),
            assessment.limits().iter().collect::<Vec<_>>()
        );
    }
    assert_eq!(earth.1.class(), HabitabilityClass::Temperate);
    assert!(earth.1.limits().is_empty(), "{:?}", earth.1.limits());
    assert_eq!(mars.1.class(), HabitabilityClass::Marginal);
    assert!(
        !mars
            .1
            .limits()
            .contains(HabitabilityLimit::OutsideOptimisticZone)
    );
    assert_eq!(venus.1.class(), HabitabilityClass::Hostile);
    assert!(
        venus
            .1
            .limits()
            .contains(HabitabilityLimit::RunawayGreenhouse)
    );
    assert!(
        venus
            .1
            .limits()
            .contains(HabitabilityLimit::OutsideOptimisticZone)
    );
}

/// Europa, its measured bulk with its subsurface ocean by plan 14's rule under its tide and the
/// generated Europa's air, about Jupiter's orbit (the table's): airless, beyond the zone, with an
/// ocean beneath its ice.
#[test]
fn europa_is_a_subsurface_ocean() {
    let (figures, ocean, _) = measured_europa();
    assert!(matches!(ocean, Subsurface::Ocean(_)), "{ocean:?}");
    let air = europa();
    let &(_, _, _, a, e) = PLANETS.iter().find(|(n, ..)| *n == "Jupiter").unwrap();
    // Europa's measured mass, 4.7998 × 10²² kg (the figures' fixture), 8% water: rocky.
    let assessment = HabitabilityAssessment::assess(&HabitabilityInputs {
        class: PlanetClass::Rocky,
        mass: EarthMasses::new(4.7998e22 / EARTH_MASS_KG),
        state: air.atmosphere().published_state(),
        surface: Some(SurfaceReading {
            pressure: air
                .atmosphere()
                .surface_pressure()
                .expect("Europa has a surface"),
            liquid_water: false,
            subsurface_ocean: matches!(figures.subsurface_ocean(), Subsurface::Ocean(_)),
        }),
        zone: Some(sun_zone()),
        semi_major_axis: Metres::new(a * METRES_PER_AU),
        eccentricity: Eccentricity::new(e).unwrap(),
        locked: false,
        radiation: RadiationClass::of(air.xuv_flux()),
        host: sun_clock(),
    });
    eprintln!(
        "Europa: {:?} {:?}",
        assessment.class(),
        assessment.limits().iter().collect::<Vec<_>>()
    );
    assert_eq!(assessment.class(), HabitabilityClass::SubsurfaceOcean);
    for limit in [
        HabitabilityLimit::OutsideOptimisticZone,
        HabitabilityLimit::OutsideConservativeZone,
        HabitabilityLimit::NoAtmosphere,
        HabitabilityLimit::BelowTriplePoint,
    ] {
        assert!(assessment.limits().contains(limit), "{limit:?}");
    }
}

/// The table's Earth's inputs, which every edge test changes one part of.
fn earth() -> HabitabilityInputs {
    let table = figures_table();
    table_planet("Earth", figures_of(&table, "Earth"))
}

/// The class and limits of `inputs`.
fn assess(inputs: &HabitabilityInputs) -> (HabitabilityClass, HabitabilityLimits) {
    let assessment = HabitabilityAssessment::assess(inputs);
    (assessment.class(), assessment.limits())
}

#[test]
fn temperate_needs_a_rocky_body_of_its_masses_in_the_conservative_zone_with_water() {
    let base = earth();
    assert_eq!(assess(&base).0, HabitabilityClass::Temperate);
    let (lo, hi) = TEMPERATE_MASSES;
    for (mass, class) in [
        (lo, HabitabilityClass::Temperate),
        (hi, HabitabilityClass::Temperate),
        (EarthMasses::new(0.099), HabitabilityClass::Marginal),
        (EarthMasses::new(5.01), HabitabilityClass::Marginal),
    ] {
        let inputs = HabitabilityInputs { mass, ..base };
        assert_eq!(assess(&inputs).0, class, "{mass:?}");
    }
    let icy = HabitabilityInputs {
        class: PlanetClass::Icy,
        ..base
    };
    assert_eq!(assess(&icy).0, HabitabilityClass::Marginal);
    let dry = HabitabilityInputs {
        surface: base.surface.map(|surface| SurfaceReading {
            liquid_water: false,
            ..surface
        }),
        ..base
    };
    assert_eq!(assess(&dry).0, HabitabilityClass::Marginal);
    // Between the moist greenhouse and recent Venus: in the optimistic zone alone.
    let zone = sun_zone();
    let between = f64::midpoint(zone.recent_venus().value(), zone.moist_greenhouse().value());
    let hot = HabitabilityInputs {
        semi_major_axis: Metres::new(between),
        eccentricity: Eccentricity::CIRCULAR,
        ..base
    };
    let (class, limits) = assess(&hot);
    assert_eq!(class, HabitabilityClass::Marginal);
    assert!(limits.contains(HabitabilityLimit::OutsideConservativeZone));
    assert!(!limits.contains(HabitabilityLimit::OutsideOptimisticZone));
}

#[test]
fn a_zone_s_edges_are_inside_it_and_its_orbit_mean_distance_decides() {
    let base = earth();
    let zone = sun_zone();
    let at = |d: f64, e: f64| HabitabilityInputs {
        semi_major_axis: Metres::new(d),
        eccentricity: Eccentricity::new(e).unwrap(),
        ..base
    };
    let (inner, outer) = zone.conservative();
    for d in [inner.value(), outer.value()] {
        assert_eq!(assess(&at(d, 0.0)).0, HabitabilityClass::Temperate, "{d}");
    }
    let (class, limits) = assess(&at(outer.value() * (1.0 + 1e-9), 0.0));
    assert_eq!(class, HabitabilityClass::Marginal);
    assert!(limits.contains(HabitabilityLimit::OutsideConservativeZone));
    // An eccentric orbit whose axis lies beyond the outer edge, but whose orbit-mean distance
    // a (1 − e²)^¼ lies inside it, is inside.
    let e: f64 = 0.2;
    let a = outer.value() * 0.995 / (1.0 - e * e).sqrt().sqrt();
    assert!(a > outer.value());
    assert_eq!(assess(&at(a, e)).0, HabitabilityClass::Temperate);
    // Beyond early Mars, and with no zone at all, it is outside both.
    let (_, far) = zone.optimistic();
    for inputs in [
        at(far.value() * 1.01, 0.0),
        HabitabilityInputs { zone: None, ..base },
    ] {
        let (class, limits) = assess(&inputs);
        assert_eq!(class, HabitabilityClass::Hostile);
        assert!(limits.contains(HabitabilityLimit::OutsideOptimisticZone));
        assert!(limits.contains(HabitabilityLimit::OutsideConservativeZone));
        assert!(!limits.contains(HabitabilityLimit::HighEccentricity));
    }
}

#[test]
fn an_orbit_that_leaves_the_optimistic_zone_is_highly_eccentric() {
    let base = earth();
    let zone = sun_zone();
    let (near, far) = zone.optimistic();
    // At 1.1 au the pericentre reaches recent Venus at e = 1 − near ÷ a, about 0.32, while the
    // apocentre and the orbit-mean distance stay inside the zone.
    let a = 1.1 * METRES_PER_AU;
    let edge = 1.0 - near.value() / a;
    assert!(a * (1.0 + edge) < far.value());
    let at = |e: f64| HabitabilityInputs {
        semi_major_axis: Metres::new(a),
        eccentricity: Eccentricity::new(e).unwrap(),
        ..base
    };
    assert!(
        !assess(&at(edge * 0.999))
            .1
            .contains(HabitabilityLimit::HighEccentricity)
    );
    let (class, limits) = assess(&at(edge * 1.001));
    assert!(limits.contains(HabitabilityLimit::HighEccentricity));
    assert_eq!(
        class,
        HabitabilityClass::Temperate,
        "the mean decides the class"
    );
    // And by the apocentre past early Mars.
    let a = far.value() * 0.9;
    let apocentre = HabitabilityInputs {
        semi_major_axis: Metres::new(a),
        eccentricity: Eccentricity::new(0.15).unwrap(),
        ..base
    };
    assert!(a * 1.15 > far.value());
    assert!(
        assess(&apocentre)
            .1
            .contains(HabitabilityLimit::HighEccentricity)
    );
}

#[test]
fn each_state_takes_its_limit_and_its_class() {
    let base = earth();
    let surface = |pressure: f64, liquid_water: bool, subsurface_ocean: bool| {
        Some(SurfaceReading {
            pressure: Pascals::new(pressure),
            liquid_water,
            subsurface_ocean,
        })
    };
    let cases = [
        (
            SurfaceState::Airless,
            surface(0.0, false, false),
            HabitabilityClass::Hostile,
            [
                HabitabilityLimit::NoAtmosphere,
                HabitabilityLimit::BelowTriplePoint,
            ]
            .as_slice(),
        ),
        (
            SurfaceState::Tenuous,
            surface(1.0, false, true),
            HabitabilityClass::SubsurfaceOcean,
            &[
                HabitabilityLimit::NoAtmosphere,
                HabitabilityLimit::BelowTriplePoint,
            ],
        ),
        (
            SurfaceState::MagmaOcean,
            surface(1e7, false, false),
            HabitabilityClass::Hostile,
            &[],
        ),
        (
            SurfaceState::RunawayGreenhouse,
            surface(9.2e6, false, false),
            HabitabilityClass::Hostile,
            &[HabitabilityLimit::RunawayGreenhouse],
        ),
        (
            SurfaceState::Snowball,
            surface(1e5, false, false),
            HabitabilityClass::Marginal,
            &[HabitabilityLimit::Snowball],
        ),
        (
            SurfaceState::Temperate,
            surface(600.0, false, false),
            HabitabilityClass::Marginal,
            &[HabitabilityLimit::BelowTriplePoint],
        ),
        (
            SurfaceState::GasEnvelope,
            None,
            HabitabilityClass::Hostile,
            &[HabitabilityLimit::GasEnvelope],
        ),
        (
            SurfaceState::SteamEnvelope,
            None,
            HabitabilityClass::Hostile,
            &[
                HabitabilityLimit::RunawayGreenhouse,
                HabitabilityLimit::GasEnvelope,
            ],
        ),
    ];
    for (state, surface, class, expected) in cases {
        let inputs = HabitabilityInputs {
            state,
            surface,
            ..base
        };
        let (got, limits) = assess(&inputs);
        assert_eq!(got, class, "{state:?}");
        assert_eq!(
            limits.iter().collect::<Vec<_>>(),
            expected,
            "{state:?}: {limits:?}"
        );
    }
    // The triple point's pressure itself holds liquid water.
    let at_triple = HabitabilityInputs {
        surface: surface(water_triple_pressure().value(), true, false),
        ..base
    };
    assert!(
        !assess(&at_triple)
            .1
            .contains(HabitabilityLimit::BelowTriplePoint)
    );
}

#[test]
fn the_host_s_limits_are_its_lock_its_activity_its_age_and_its_track() {
    let base = earth();
    let only = |inputs: &HabitabilityInputs| {
        let (class, limits) = assess(inputs);
        assert_eq!(
            class,
            HabitabilityClass::Temperate,
            "no host limit moves the class"
        );
        limits.iter().collect::<Vec<_>>()
    };
    let locked = HabitabilityInputs {
        locked: true,
        ..base
    };
    assert_eq!(only(&locked), [HabitabilityLimit::TidallyLocked]);
    for (radiation, active) in [
        (RadiationClass::Dark, false),
        (RadiationClass::Quiet, false),
        (RadiationClass::Active, true),
        (RadiationClass::Intense, true),
        (RadiationClass::Extreme, true),
    ] {
        let inputs = HabitabilityInputs { radiation, ..base };
        assert_eq!(
            only(&inputs).contains(&HabitabilityLimit::ActiveHost),
            active,
            "{radiation:?}"
        );
    }
    let young = HabitabilityInputs {
        host: HostClock {
            before_main_sequence: true,
            ..base.host
        },
        ..base
    };
    assert_eq!(only(&young), [HabitabilityLimit::YoungHost]);
    for (left, leaving) in [(0.0, true), (0.999e9, true), (1e9, false), (5e9, false)] {
        let inputs = HabitabilityInputs {
            host: HostClock {
                main_sequence_left: Some(Years::new(left)),
                ..base.host
            },
            ..base
        };
        assert_eq!(
            only(&inputs) == [HabitabilityLimit::LeavingMainSequence],
            leaving,
            "{left} yr"
        );
    }
    let never = HabitabilityInputs {
        host: HostClock {
            main_sequence_left: None,
            ..base.host
        },
        ..base
    };
    assert_eq!(only(&never), []);
    // A host of 7,500 K lies beyond the fit: its zone is extrapolated.
    let hot = habitable_zone(SolarLuminosities::new(1.0), Kelvin::new(7_500.0));
    assert!(hot.extrapolated());
    let a = hot.conservative().0.value() * 1.1;
    let extrapolated = HabitabilityInputs {
        zone: Some(hot),
        semi_major_axis: Metres::new(a),
        eccentricity: Eccentricity::CIRCULAR,
        ..base
    };
    assert_eq!(only(&extrapolated), [HabitabilityLimit::ExtrapolatedZone]);
}

#[test]
fn a_set_of_limits_holds_each_once_in_the_plan_s_order() {
    let all: HabitabilityLimits = HabitabilityLimit::ALL.into_iter().rev().collect();
    assert_eq!(all.iter().collect::<Vec<_>>(), HabitabilityLimit::ALL);
    let bits = HabitabilityLimit::ALL
        .into_iter()
        .fold(0_u16, |bits, limit| bits | limit.bit());
    assert_eq!(bits.count_ones(), 13, "one bit each");
    assert!(HabitabilityLimits::EMPTY.is_empty());
    assert_eq!(HabitabilityLimits::default(), HabitabilityLimits::EMPTY);
}

/// The assessments `hooks_at` publishes over the sample's first systems at the epoch, each
/// against the hooks it is read with: a present body's section is the surface's state, or `Ok` and
/// `Hostile` under an envelope; a body not present has none; each class and each surface limit
/// agrees with the published conditions and figures.
#[test]
fn the_published_assessment_agrees_with_the_published_surface() {
    let t = UniverseTime::EPOCH;
    let triple = water_triple_pressure().value();
    let mut classes = std::collections::BTreeMap::new();
    for (ctx, system) in whole().iter().take(40) {
        for body in system.bodies() {
            let Some(hooks) = system.hooks_at(ctx, body.index(), t).unwrap() else {
                continue;
            };
            let name = body.index().body_id(ctx.id());
            let assessment = match (hooks.surface(), hooks.habitability()) {
                (Section::Ok(_), Section::Ok(assessment)) => assessment,
                (Section::NotApplicable, Section::Ok(assessment)) => {
                    assert_eq!(assessment.class(), HabitabilityClass::Hostile, "{name}");
                    assert!(
                        assessment.limits().contains(HabitabilityLimit::GasEnvelope),
                        "{name}"
                    );
                    *classes.entry("an envelope, Hostile").or_insert(0_u32) += 1;
                    continue;
                }
                (surface, habitability) => {
                    assert_eq!(surface.state(), habitability.state(), "{name}");
                    continue;
                }
            };
            let Section::Ok(conditions) = hooks.surface() else {
                unreachable!("matched above");
            };
            let Section::Ok(figures) = hooks.figures() else {
                panic!("{name} has conditions but no figures");
            };
            let limits = assessment.limits();
            assert!(!limits.contains(HabitabilityLimit::GasEnvelope), "{name}");
            assert_eq!(
                limits.contains(HabitabilityLimit::BelowTriplePoint),
                conditions.surface_pressure().value() < triple,
                "{name}"
            );
            match assessment.class() {
                HabitabilityClass::Temperate => {
                    assert!(conditions.liquid_water(), "{name}");
                    assert!(
                        !limits.contains(HabitabilityLimit::OutsideConservativeZone),
                        "{name}"
                    );
                }
                HabitabilityClass::Marginal => {
                    assert!(
                        !limits.contains(HabitabilityLimit::OutsideOptimisticZone),
                        "{name}"
                    );
                    assert!(!limits.contains(HabitabilityLimit::NoAtmosphere), "{name}");
                }
                HabitabilityClass::SubsurfaceOcean => {
                    assert!(
                        matches!(figures.subsurface_ocean(), Subsurface::Ocean(_)),
                        "{name}"
                    );
                }
                HabitabilityClass::Hostile => {}
            }
            *classes.entry(class_name(assessment.class())).or_insert(0) += 1;
        }
    }
    eprintln!("the sample's first 40 systems at the epoch: {classes:?}");
    assert!(classes.values().sum::<u32>() > 100, "{classes:?}");
}

/// A class's name in a count.
fn class_name(class: HabitabilityClass) -> &'static str {
    match class {
        HabitabilityClass::Hostile => "Hostile",
        HabitabilityClass::SubsurfaceOcean => "SubsurfaceOcean",
        HabitabilityClass::Marginal => "Marginal",
        HabitabilityClass::Temperate => "Temperate",
    }
}

/// The present assessments of the bodies of `system` at the epoch.
fn assessments(ctx: &SystemContext, system: &PlanetarySystem) -> Vec<HabitabilityAssessment> {
    system
        .bodies()
        .iter()
        .filter_map(|body| {
            let hooks = system
                .hooks_at(ctx, body.index(), UniverseTime::EPOCH)
                .unwrap()?;
            hooks.habitability().ok().copied()
        })
        .collect()
}

/// Every present body about a single star of `mass` M☉ and age `age`, over the first synthetic
/// systems that hold any, assessed at the epoch.
fn about(mass: f64, age: f64) -> Vec<HabitabilityAssessment> {
    (0..40)
        .flat_map(|index| {
            let ctx = synthetic_star(
                id(index),
                SolarMasses::new(mass),
                Dex::ZERO,
                Years::new(age),
            )
            .unwrap();
            let system = generate(SEED, &ctx);
            assessments(&ctx, &system)
        })
        .collect()
}

/// The zone's stars reach the assessment through `hooks_at`: a 10 Myr old Sun is still contracting
/// to its main sequence, a 2.5 M☉ star of 0.5 Gyr ends its main sequence within 1 Gyr, and the
/// present Sun does neither.
#[test]
fn the_host_s_clock_reaches_the_published_assessment() {
    let (young, leaving) = (
        HabitabilityLimit::YoungHost,
        HabitabilityLimit::LeavingMainSequence,
    );
    for (mass, age, expected) in [
        (1.0, 1e7, [young].as_slice()),
        (2.5, 5e8, &[leaving]),
        (1.0, 4.57e9, &[]),
    ] {
        let assessed = about(mass, age);
        assert!(!assessed.is_empty(), "{mass} M☉ at {age} yr");
        for assessment in &assessed {
            for limit in [young, leaving] {
                assert_eq!(
                    assessment.limits().contains(limit),
                    expected.contains(&limit),
                    "{mass} M☉ at {age} yr: {limit:?}"
                );
            }
        }
    }
}

/// The share of the census's FGK hosts that hold a `Temperate` body at the epoch, reported and
/// asserted only to lie in 0.01–0.3, since η⊕ is that uncertain (the plan's test). The hosts are
/// the census's single FGK dwarfs (`census_tests`' `HostClass::FgkDwarf`: one star on its main
/// sequence at the epoch, of 0.7–1.3 M☉) among the composition census's first [`FGK_SYSTEMS`]
/// systems of the sample. Native only, as the census is: a share over thousands of systems, whose
/// bits on WebAssembly the goldens check.
#[cfg(not(target_family = "wasm"))]
#[test]
#[ignore = "slow: every body of the FGK hosts among 6,800 systems, assessed at the epoch"]
fn the_fgk_hosts_share_with_a_temperate_body_lies_within_eta_earth_s_range() {
    use crate::galaxy::Galaxy;
    use crate::galaxy::params::GalaxyParams;
    use crate::planetary::testing::{SampleFilter, sample_records};
    use crate::stellar::Phase;

    let galaxy = Galaxy::from_params(SEED, GalaxyParams::milky_way_like())
        .expect("the Milky Way's parameters build a galaxy");
    let records = sample_records(&galaxy, FGK_SYSTEMS, SampleFilter::ALL)
        .expect("the solar circle holds the census's systems");
    let (mut hosts, mut temperate, mut bodies) = (0_u32, 0_u32, 0_usize);
    for record in &records {
        let ctx = SystemContext::from_record(&galaxy, record);
        let [star] = ctx.stars() else {
            continue;
        };
        let m = star.initial_mass().value();
        let on_main_sequence = star
            .state_at(UniverseTime::EPOCH)
            .is_some_and(|state| state.phase() == Phase::MainSequence);
        if !(on_main_sequence && (0.7..1.3).contains(&m)) {
            continue;
        }
        hosts += 1;
        let system = generate(SEED, &ctx);
        let assessed = assessments(&ctx, &system);
        bodies += assessed.len();
        if assessed
            .iter()
            .any(|assessment| assessment.class() == HabitabilityClass::Temperate)
        {
            temperate += 1;
        }
    }
    let share = f64::from(temperate) / f64::from(hosts);
    eprintln!(
        "{temperate} of {hosts} FGK hosts hold a Temperate body at the epoch ({share:.4}), \
         {bodies} bodies assessed, of the sample's first {FGK_SYSTEMS} systems"
    );
    assert!(hosts >= 100, "{hosts} FGK hosts");
    assert!((0.01..=0.3).contains(&share), "{share}");
}

/// The composition census's ordinary systems, whose FGK hosts the share reads.
#[cfg(not(target_family = "wasm"))]
const FGK_SYSTEMS: usize = 6_800;
