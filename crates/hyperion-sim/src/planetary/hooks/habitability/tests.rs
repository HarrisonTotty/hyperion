//! P14.T25's tests: the reference bodies' classes, each limit at its edge, the assessment as
//! `hooks_at` publishes it over the sample, and the share of FGK hosts with a `Temperate` body
//! (slow).

use super::*;
use crate::planetary::chem::partition::CondensatePhase;
use crate::planetary::derive::solar::{PLANETS, SOLAR_AGE, historic, historic_sun};
use crate::planetary::derive::{habitable_zone, habitable_zone_of};
use crate::planetary::hooks::Subsurface;
use crate::planetary::hooks::cold_traps::{RecordColdTraps, RecordField};
use crate::planetary::hooks::figures::table_figures;
use crate::planetary::hooks::figures::tests::{europa, measured_europa};
use crate::planetary::hooks::surface::tests::{inputs_of, spin_of};
use crate::planetary::hooks::surface::{SurfaceConditions, SurfaceInputs};
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

/// The table planet `name` about the present Sun at its age, by the table's record path: the
/// generator's derivation (plan 14's Solar System table, NASA's elements), its figures, its
/// conditions under its own spin with the figures' heat flow and ocean weight, and P14.T51.a's
/// published condensates, whose liquid water the conditions then read, as a record's do (the path
/// of `hooks::surface::tests::the_batch_publishes_the_table_s_condensates`): its inputs as
/// `hooks_at` gathers them, under the rules in force where it is called.
fn table_planet(name: &str) -> HabitabilityInputs {
    let body = historic(name);
    let figures = table_figures(&body).expect("a table planet has a surface");
    let inputs = SurfaceInputs {
        heat_flow: figures.heat_flow(),
        ocean_weight: figures.ocean_weight(),
        ..inputs_of(&body, spin_of(name))
    };
    let conditions = SurfaceConditions::derive(&inputs).expect("a table planet has a surface");
    let traps = RecordColdTraps {
        field: RecordField::Carried(None),
    };
    let (_, published) = figures.condensates_under(
        body.atmosphere(),
        &conditions,
        body.surface_gravity(),
        &traps,
        &conditions.zonal_insolation(),
    );
    let conditions = conditions.with_condensates(&published);
    let &(_, _, _, a, e) = PLANETS
        .iter()
        .find(|(n, ..)| *n == name)
        .expect("a table planet");
    let water = Gas::Water.substance();
    HabitabilityInputs {
        class: body.class(),
        mass: body.mass(),
        state: body.atmosphere().published_state(),
        surface: Some(SurfaceReading {
            pressure: conditions.surface_pressure(),
            liquid_water: conditions.liquid_water(),
            subsurface_ocean: matches!(figures.subsurface_ocean(), Subsurface::Ocean(_)),
            mean_temperature: conditions.mean_surface_temperature(),
            holds_water: [CondensatePhase::Liquid, CondensatePhase::Solid]
                .into_iter()
                .any(|phase| published.entry(water, phase).is_some()),
            water_state: figures.water_state(),
        }),
        zone: Some(sun_zone()),
        semi_major_axis: Metres::new(a * METRES_PER_AU),
        eccentricity: Eccentricity::new(e).unwrap(),
        locked: false,
        radiation: conditions.radiation(),
        host: sun_clock(),
    }
}

/// The table's Earth, Mars and Venus, assessed under the rules in force where it is called.
fn table_assessments() -> [(&'static str, HabitabilityInputs, HabitabilityAssessment); 3] {
    ["Earth", "Mars", "Venus"].map(|name| {
        let inputs = table_planet(name);
        (name, inputs, HabitabilityAssessment::assess(&inputs))
    })
}

/// The table's Earth, Mars and Venus under the rules at generator version 25 and under every 26
/// hold (`with_every_27_hold`), each printed with its surface pressure: Earth `Temperate` with no
/// limit, Mars a frozen `Marginal` desert, below water's triple point exactly where its pressure
/// is, and Venus a dry `Hostile` runaway.
#[test]
fn earth_is_temperate_mars_marginal_and_venus_a_hostile_runaway() {
    let triple = water_triple_pressure().value();
    for (rules, assessed) in [
        ("version 25", table_assessments()),
        ("every 26 hold", table_assessments()),
    ] {
        for (name, inputs, assessment) in &assessed {
            let surface = inputs.surface.expect("a table planet has a surface");
            eprintln!(
                "{rules}: {name} at {:.1} Pa, {:.1} K: {:?} {:?}",
                surface.pressure.value(),
                surface.mean_temperature.value(),
                assessment.class(),
                assessment.limits().iter().collect::<Vec<_>>()
            );
        }
        let [(_, _, earth), (_, mars_inputs, mars), (_, _, venus)] = &assessed;
        assert_eq!(earth.class(), HabitabilityClass::Temperate, "{rules}");
        assert!(earth.limits().is_empty(), "{rules}: {:?}", earth.limits());
        assert_eq!(mars.class(), HabitabilityClass::Marginal, "{rules}");
        let mars_pressure = mars_inputs.surface.unwrap().pressure.value();
        let limits = mars.limits();
        assert!(limits.contains(HabitabilityLimit::FrozenSurface), "{rules}");
        assert_eq!(
            limits.contains(HabitabilityLimit::BelowTriplePoint),
            mars_pressure < triple,
            "{rules}"
        );
        assert!(
            !limits.contains(HabitabilityLimit::OutsideOptimisticZone),
            "{rules}"
        );
        assert_eq!(venus.class(), HabitabilityClass::Hostile, "{rules}");
        for limit in [
            HabitabilityLimit::RunawayGreenhouse,
            HabitabilityLimit::OutsideOptimisticZone,
            HabitabilityLimit::DrySurface,
        ] {
            assert!(venus.limits().contains(limit), "{rules}: {limit:?}");
        }
    }
}

/// The table's Mars at its own spin (`derive::atmosphere::tests::table_mars_at_its_spin`), whose
/// CO₂ cap P14.T53.h balances under every 26 hold, as the record path above does not (it derives
/// with no spin): its surface pressure, CO₂ and N₂ and the rest, against water's triple point,
/// 611.657 Pa. The cap's pilot settled CO₂ alone at 565.8 Pa; the surface pressure, the sum of
/// the partial pressures, lies above the triple point, so the assessment's `BelowTriplePoint` does
/// not fire there either. Printed, with the CO₂ share, and the limit asserted to follow the sum.
#[test]
fn the_cap_balanced_mars_s_surface_pressure_against_the_triple_point() {
    use crate::planetary::derive::atmosphere::tests::table_mars_at_its_spin;

    let triple = water_triple_pressure().value();
    let (_, _, air, _) = table_mars_at_its_spin();
    let pressure = air.surface_pressure().expect("Mars has a surface").value();
    let co2 = air.partial_pressures().of(Gas::CarbonDioxide).value();
    eprintln!(
        "the table's Mars at its own spin, every 26 hold: {pressure:.2} Pa ({co2:.2} Pa of CO2), \
         {:.2} K, {:?}; water's triple point {triple} Pa",
        air.surface_temperature().value(),
        air.published_state()
    );
    let base = table_planet("Mars");
    let surface = base.surface.expect("the table's Mars has a surface");
    let inputs = HabitabilityInputs {
        state: air.published_state(),
        surface: Some(SurfaceReading {
            pressure: Pascals::new(pressure),
            mean_temperature: air.surface_temperature(),
            ..surface
        }),
        ..base
    };
    let (class, limits) = assess(&inputs);
    assert_eq!(class, HabitabilityClass::Marginal);
    assert_eq!(
        limits.contains(HabitabilityLimit::BelowTriplePoint),
        pressure < triple,
        "{pressure} Pa"
    );
    assert!(co2 < triple && pressure > triple, "{co2} and {pressure} Pa");
}

/// Europa, its measured bulk with its subsurface ocean by plan 14's rule under its tide and the
/// generated Europa's air, about Jupiter's orbit (the table's): airless, frozen, beyond the zone,
/// with an ocean beneath its ice.
#[test]
fn europa_is_a_subsurface_ocean() {
    let (figures, ocean, _) = measured_europa();
    assert!(matches!(ocean, Subsurface::Ocean(_)), "{ocean:?}");
    let air = europa();
    let &(_, _, _, a, e) = PLANETS.iter().find(|(n, ..)| *n == "Jupiter").unwrap();
    // Europa's measured mass, 4.7998 × 10²² kg (the figures' fixture), 8% water: rocky. Its
    // surface is water ice, which a record lists as its crust's.
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
            mean_temperature: air.atmosphere().surface_temperature(),
            holds_water: true,
            water_state: air.atmosphere().water_state(),
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
        HabitabilityLimit::MassOutsideTemperateRange,
        HabitabilityLimit::FrozenSurface,
    ] {
        assert!(assessment.limits().contains(limit), "{limit:?}");
    }
    assert!(!assessment.limits().contains(HabitabilityLimit::DrySurface));
}

/// The table's Earth's inputs, which every edge test changes one part of.
fn earth() -> HabitabilityInputs {
    table_planet("Earth")
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
    let wet = base.surface.expect("the table's Earth has a surface");
    let surface = |pressure: f64, liquid_water: bool, subsurface_ocean: bool| {
        Some(SurfaceReading {
            pressure: Pascals::new(pressure),
            liquid_water,
            subsurface_ocean,
            ..wet
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
            &[HabitabilityLimit::MoltenSurface],
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

/// The four limits added after the plan's list (lean, 2026-11-01), each at its edge on the table's
/// Earth: molten by its state, dry by its published water or a past runaway's, out of
/// `Temperate`'s masses, and frozen under water's melting point short of a snowball.
#[test]
fn the_added_limits_are_the_generator_s_own_states_at_their_edges() {
    let base = earth();
    let wet = base.surface.expect("the table's Earth has a surface");
    let limits = |state: SurfaceState, surface: SurfaceReading| {
        let inputs = HabitabilityInputs {
            state,
            surface: Some(surface),
            ..base
        };
        assess(&inputs).1
    };
    let temperate = SurfaceState::Temperate;
    // Molten: the state alone.
    assert!(limits(SurfaceState::MagmaOcean, wet).contains(HabitabilityLimit::MoltenSurface));
    assert!(!limits(temperate, wet).contains(HabitabilityLimit::MoltenSurface));
    // Dry: no published water, or a past runaway's water under a present state that is not one.
    let dry = |state, holds_water, water_state| {
        let surface = SurfaceReading {
            holds_water,
            water_state,
            ..wet
        };
        limits(state, surface).contains(HabitabilityLimit::DrySurface)
    };
    let runaway = SurfaceState::RunawayGreenhouse;
    assert!(!dry(temperate, true, temperate));
    assert!(dry(temperate, false, temperate));
    assert!(dry(temperate, true, runaway));
    assert!(dry(SurfaceState::Snowball, true, runaway));
    assert!(
        !dry(runaway, true, runaway),
        "a present runaway's own state"
    );
    assert!(dry(runaway, false, runaway));
    let hidden = HabitabilityInputs {
        state: SurfaceState::GasEnvelope,
        surface: None,
        ..base
    };
    assert!(!assess(&hidden).1.contains(HabitabilityLimit::DrySurface));
    // Mass: the band's ends are inside it.
    let (lo, hi) = TEMPERATE_MASSES;
    for (mass, outside) in [
        (lo, false),
        (hi, false),
        (EarthMasses::new(0.099), true),
        (EarthMasses::new(5.01), true),
    ] {
        let inputs = HabitabilityInputs { mass, ..base };
        assert_eq!(
            assess(&inputs)
                .1
                .contains(HabitabilityLimit::MassOutsideTemperateRange),
            outside,
            "{mass:?}"
        );
    }
    // Frozen: no liquid water under 273.15 K, but for a snowball.
    let frozen = |state, liquid_water, kelvin: f64| {
        let surface = SurfaceReading {
            liquid_water,
            mean_temperature: Kelvin::new(kelvin),
            ..wet
        };
        limits(state, surface).contains(HabitabilityLimit::FrozenSurface)
    };
    assert!(frozen(temperate, false, 273.149));
    assert!(!frozen(temperate, false, 273.15));
    assert!(!frozen(temperate, true, 250.0));
    assert!(!frozen(SurfaceState::Snowball, false, 200.0));
    assert!(frozen(SurfaceState::Airless, false, 100.0));
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
        .fold(0_u32, |bits, limit| bits | limit.bit());
    assert_eq!(bits.count_ones(), 17, "one bit each");
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
    let mut unexplained = Vec::new();
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
            if limits.contains(HabitabilityLimit::FrozenSurface) {
                assert!(!conditions.liquid_water(), "{name}");
                assert!(
                    conditions.mean_surface_temperature().value() < WATER_MELTING_POINT.value(),
                    "{name}"
                );
            }
            if assessment.class() != HabitabilityClass::Temperate && limits.is_empty() {
                unexplained.push(format!("{name} {:?}", assessment.class()));
            }
            *classes.entry(class_name(assessment.class())).or_insert(0) += 1;
        }
    }
    eprintln!("the sample's first 40 systems at the epoch: {classes:?}");
    assert!(classes.values().sum::<u32>() > 100, "{classes:?}");
    assert!(unexplained.is_empty(), "named no limit: {unexplained:?}");
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

/// The host's limits are its zone's (`YoungHost`, `LeavingMainSequence` and `ExtrapolatedZone`
/// read the zone's stars and clock alone): every body that `hooks_at` assesses at the epoch shares
/// them with every other body of the sample's first systems that orbits the same host, a moon by
/// its planet's host.
#[test]
fn every_body_of_one_host_shares_the_host_s_limits() {
    use crate::planetary::placement::OrbitHost;
    let t = UniverseTime::EPOCH;
    let zone_level = [
        HabitabilityLimit::YoungHost,
        HabitabilityLimit::LeavingMainSequence,
        HabitabilityLimit::ExtrapolatedZone,
    ];
    let (mut shared, mut hosts) = (0, 0);
    for (ctx, system) in whole().iter().take(40) {
        let mut by_host = std::collections::BTreeMap::<OrbitHost, Vec<[bool; 3]>>::new();
        for body in system.bodies() {
            let Some(hooks) = system.hooks_at(ctx, body.index(), t).unwrap() else {
                continue;
            };
            let Section::Ok(assessment) = hooks.habitability() else {
                continue;
            };
            let record = system.body_at(ctx, body.index(), t).unwrap();
            let host = match record.identity().parent() {
                Some(OrbitHost::Body(planet)) => system
                    .body_at(ctx, planet, t)
                    .unwrap()
                    .identity()
                    .parent()
                    .expect("a moon's planet orbits a host"),
                Some(host) => host,
                None => continue,
            };
            by_host
                .entry(host)
                .or_default()
                .push(zone_level.map(|limit| assessment.limits().contains(limit)));
        }
        for (host, limits) in by_host {
            assert!(
                limits.iter().all(|each| *each == limits[0]),
                "{:?} about {host:?}: {limits:?}",
                ctx.id()
            );
            shared += limits.len();
            hosts += 1;
        }
    }
    assert!(
        shared > 100 && hosts > 20,
        "{shared} bodies about {hosts} hosts"
    );
}

/// A present Sun's planets read a quiet host through `hooks_at`: about a 1 M☉ star of 4.57 Gyr,
/// every planet with a surface beyond 0.5 au, where its X-ray and ultraviolet flux is at most
/// about 4 times the present Earth's, under `Active`'s floor of 10, is `Quiet` and takes no host
/// limit.
#[test]
fn a_present_sun_s_planets_beyond_half_an_au_read_a_quiet_host() {
    use crate::planetary::hooks::RadiationClass;
    use crate::planetary::record::BodyKind;
    let t = UniverseTime::EPOCH;
    let mut checked = 0;
    for index in 0..40 {
        let ctx = synthetic_star(id(index), SolarMasses::new(1.0), Dex::ZERO, SOLAR_AGE).unwrap();
        let system = generate(SEED, &ctx);
        for body in system.bodies() {
            let record = system.body_at(&ctx, body.index(), t).unwrap();
            let (BodyKind::Planet, Section::Ok(orbit)) = (record.identity().kind(), record.orbit())
            else {
                continue;
            };
            if orbit.elements().semi_major_axis().value() < 0.5 * METRES_PER_AU {
                continue;
            }
            let hooks = system.hooks_at(&ctx, body.index(), t).unwrap().unwrap();
            let (Section::Ok(surface), Section::Ok(assessment)) =
                (hooks.surface(), hooks.habitability())
            else {
                continue;
            };
            let name = body.index().body_id(ctx.id());
            assert_eq!(surface.radiation(), RadiationClass::Quiet, "{name}");
            for limit in [
                HabitabilityLimit::ActiveHost,
                HabitabilityLimit::YoungHost,
                HabitabilityLimit::LeavingMainSequence,
                HabitabilityLimit::ExtrapolatedZone,
            ] {
                assert!(!assessment.limits().contains(limit), "{name}: {limit:?}");
            }
            checked += 1;
        }
    }
    assert!(checked > 10, "{checked} planets checked");
}

/// Every system of the census's FGK hosts (`census_tests`' `HostClass::FgkDwarf`: one star on its
/// main sequence at the epoch, of 0.7–1.3 M☉) among the composition census's first
/// [`FGK_SYSTEMS`] systems of the sample, generated whole, in the sample's order.
#[cfg(not(target_family = "wasm"))]
fn for_each_fgk_host(mut visit: impl FnMut(&SystemContext, &PlanetarySystem)) {
    use crate::galaxy::Galaxy;
    use crate::galaxy::params::GalaxyParams;
    use crate::planetary::testing::{SampleFilter, sample_records};
    use crate::stellar::Phase;

    let galaxy = Galaxy::from_params(SEED, GalaxyParams::milky_way_like())
        .expect("the Milky Way's parameters build a galaxy");
    let records = sample_records(&galaxy, FGK_SYSTEMS, SampleFilter::ALL)
        .expect("the solar circle holds the census's systems");
    for record in &records {
        let ctx = SystemContext::from_record(&galaxy, record);
        let [star] = ctx.stars() else {
            continue;
        };
        let m = star.initial_mass().value();
        let on_main_sequence = star
            .state_at(UniverseTime::EPOCH)
            .is_some_and(|state| state.phase() == Phase::MainSequence);
        if on_main_sequence && (0.7..1.3).contains(&m) {
            visit(&ctx, &generate(SEED, &ctx));
        }
    }
}

/// The share of the census's FGK hosts that hold a `Temperate` body at the epoch, reported and
/// asserted only to lie in 0.01–0.3, since η⊕ is that uncertain (the plan's test), over
/// [`for_each_fgk_host`]'s systems. Native only, as the census is: a share over thousands of
/// systems, whose bits on WebAssembly the goldens check.
#[cfg(not(target_family = "wasm"))]
#[test]
#[ignore = "slow: every body of the FGK hosts among 6,800 systems, assessed at the epoch"]
fn the_fgk_hosts_share_with_a_temperate_body_lies_within_eta_earth_s_range() {
    let (mut hosts, mut temperate, mut bodies) = (0_u32, 0_u32, 0_usize);
    for_each_fgk_host(|ctx, system| {
        hosts += 1;
        let assessed = assessments(ctx, system);
        bodies += assessed.len();
        if assessed
            .iter()
            .any(|assessment| assessment.class() == HabitabilityClass::Temperate)
        {
            temperate += 1;
        }
    });
    let share = f64::from(temperate) / f64::from(hosts);
    eprintln!(
        "{temperate} of {hosts} FGK hosts hold a Temperate body at the epoch ({share:.4}), \
         {bodies} bodies assessed, of the sample's first {FGK_SYSTEMS} systems"
    );
    assert!(hosts >= 100, "{hosts} FGK hosts");
    assert!((0.01..=0.3).contains(&share), "{share}");
}

/// Every body short of `Temperate` names at least one limit, over the FGK hosts' bodies at the
/// epoch ([`for_each_fgk_host`]): the four limits added after the plan's list (lean, 2026-11-01)
/// leave no class unexplained. Each limit's count is printed.
#[cfg(not(target_family = "wasm"))]
#[test]
#[ignore = "slow: every body of the FGK hosts among 6,800 systems, assessed at the epoch"]
fn every_body_short_of_temperate_names_a_limit() {
    let mut counts = std::collections::BTreeMap::new();
    let (mut assessed, mut unexplained) = (0_usize, Vec::new());
    for_each_fgk_host(|ctx, system| {
        for body in system.bodies() {
            let Some(hooks) = system
                .hooks_at(ctx, body.index(), UniverseTime::EPOCH)
                .unwrap()
            else {
                continue;
            };
            let Section::Ok(assessment) = hooks.habitability() else {
                continue;
            };
            assessed += 1;
            for limit in assessment.limits().iter() {
                *counts.entry(limit).or_insert(0_u32) += 1;
            }
            if assessment.class() != HabitabilityClass::Temperate && assessment.limits().is_empty()
            {
                unexplained.push(format!(
                    "{} {:?}",
                    body.index().body_id(ctx.id()),
                    assessment.class()
                ));
            }
        }
    });
    eprintln!("{assessed} bodies of the FGK hosts assessed at the epoch; each limit: {counts:?}");
    assert!(assessed > 1_000, "{assessed}");
    assert!(
        unexplained.is_empty(),
        "{} named no limit, the first: {:?}",
        unexplained.len(),
        &unexplained[..unexplained.len().min(20)]
    );
}

/// The desert label (ux-calls-data's ruling 9): under every 27 hold, over the FGK hosts' bodies at
/// the epoch ([`for_each_fgk_host`]), each record's surface publishes `Desert` exactly where its
/// state is `Temperate` with no liquid water listed, a label no hook reads. The sample's first Earth-like world (a water sea at 275–305 K) is published `Temperate`
/// and its first cold thin one as Mars is (below 273 K under 1 kPa) `Desert`. It holds no dry
/// `Temperate` as hot as 500 K (a dry 550 K world's label is
/// `a_dry_550_k_world_is_a_desert_under_100_bar_and_a_runaway_under_10`'s), so its hottest desert
/// is printed with the counts.
#[cfg(not(target_family = "wasm"))]
#[test]
#[ignore = "slow: every body of the FGK hosts among 6,800 systems, labelled and held"]
fn the_desert_label_moves_no_habitability_over_the_fgk_hosts() {
    use crate::planetary::derive::atmosphere::{Gas, SurfaceState};
    use crate::planetary::testing::with_every_27_hold;
    let (mut surfaces, mut deserts) = (0_usize, 0_usize);
    let (mut earth, mut cold) = (None, None);
    let mut hottest: Option<(crate::id::BodyId, f64, f64)> = None;
    with_every_27_hold(|| {
        for_each_fgk_host(|ctx, labelled| {
            for body in labelled.bodies() {
                let index = body.index();
                let name = index.body_id(ctx.id());
                let record = labelled.body_at(ctx, index, UniverseTime::EPOCH).unwrap();
                let Section::Ok(surface) = record.surface() else {
                    continue;
                };
                surfaces += 1;
                let state = surface.state();
                let liquid = surface
                    .condensates()
                    .entry(Gas::Water.substance(), CondensatePhase::Liquid)
                    .is_some();
                let desert = state == SurfaceState::Temperate && !liquid;
                assert_eq!(
                    surface.published_state(),
                    if desert { SurfaceState::Desert } else { state },
                    "{name}"
                );
                deserts += usize::from(desert);
                let t = surface.conditions().mean_surface_temperature().value();
                let p = surface.conditions().surface_pressure().value();
                let first = |slot: &mut Option<_>, found: bool| {
                    if found && slot.is_none() {
                        *slot = Some((name, surface.published_state(), t, p));
                    }
                };
                first(
                    &mut earth,
                    state == SurfaceState::Temperate && liquid && (275.0..=305.0).contains(&t),
                );
                if desert && hottest.is_none_or(|(.., hot, _)| t > hot) {
                    hottest = Some((name, t, p));
                }
                first(&mut cold, desert && t < 273.0 && p < 1e3);
            }
        });
    });
    eprintln!(
        "{surfaces} surfaces of the FGK hosts' bodies, {deserts} published Desert; the first \
         Earth-like {earth:?}, cold thin {cold:?} (name, published, K, Pa); the hottest desert \
         {hottest:?} (name, K, Pa)"
    );
    assert!(surfaces > 1_000, "{surfaces}");
    for (world, found, state) in [
        ("Earth-like", earth, SurfaceState::Temperate),
        ("cold thin", cold, SurfaceState::Desert),
    ] {
        let (name, published, ..) =
            found.unwrap_or_else(|| panic!("the sample holds a {world} world"));
        assert_eq!(published, state, "{name}, the first {world} world");
    }
}

/// The composition census's ordinary systems, whose FGK hosts the share reads.
#[cfg(not(target_family = "wasm"))]
const FGK_SYSTEMS: usize = 6_800;
