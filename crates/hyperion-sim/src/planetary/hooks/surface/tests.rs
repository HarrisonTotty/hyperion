use hyperion_testkit::float::assert_same_bits;

use super::*;
use crate::orbit::{Eccentricity, Orientation};
use crate::planetary::derive::atmosphere::{Speciation, VolatileDraws};
use crate::planetary::derive::solar::{
    PLANETS, SMALL_BODIES, SOLAR_AGE, derive_under, orbit, solar_disc, table_under,
};
use crate::planetary::derive::{DerivedBody, PlacedBody};
use crate::planetary::frames::FrameSpin;
use crate::stellar::draws::UnitUniform;
use crate::units::consts::SECONDS_PER_DAY;
use crate::units::{Degrees, EarthMasses, GravitationalParameter, Years};

/// The table's bodies' own spins: obliquity to the seasonal orbit (degrees), sidereal rotation
/// period and seasonal orbital period (days). NASA's planetary and satellite fact sheets, but:
/// Mercury's 58.646 d is the IAU's rotation (the fact sheet's 1,407.6 h is 58.650 d); Saturn's
/// 10,759.22 d is the fact sheet's before its 2025 update to 10,755.70 d; the Moon's 1.5424° is
/// its equator's tilt to the ecliptic (Cassini's laws; the fact sheet gives 6.68° to its orbit;
/// not re-read); a moon's obliquity is its planet's, since its spin follows its orbit in its
/// planet's equator, and its seasonal orbit is its planet's; Ceres's 4.03° and 9.074 17 h are
/// Park et al. 2016's (Nature 537, 515).
const SPINS: [(&str, f64, f64, f64); 8] = [
    ("Mercury", 0.034, 58.646, 87.969),
    ("Venus", 177.36, 243.025, 224.701),
    ("Earth", 23.44, 0.997_27, 365.256),
    ("Mars", 25.19, 1.025_96, 686.980),
    ("Moon", 1.542_4, 27.321_7, 365.256),
    ("Ganymede", 3.13, 7.154_6, 4_332.59),
    ("Titan", 26.73, 15.945, 10_759.22),
    ("Ceres", 4.03, 9.074_17 / 24.0, 1_680.0),
];

/// The spin of the table's body `name`, its mean solar day 2π ÷ |ω ∓ n| as
/// [`SeasonalSpin::of`] takes it.
fn spin_of(name: &str) -> SeasonalSpin {
    let &(_, degrees, rotation, year) = SPINS.iter().find(|(n, ..)| *n == name).unwrap();
    let obliquity = Radians::from(Degrees::new(degrees));
    let tau = 2.0 * core::f64::consts::PI;
    let (rotation, year) = (rotation * SECONDS_PER_DAY, year * SECONDS_PER_DAY);
    let solar = if degrees <= 90.0 {
        (tau / rotation - tau / year).abs()
    } else {
        tau / rotation + tau / year
    };
    SeasonalSpin {
        obliquity,
        rotation_period: Seconds::new(rotation),
        solar_day: Some(Seconds::new(tau / solar)),
    }
}

/// The conditions of the derived body `body` under the spin `spin`.
fn conditions_of(body: &DerivedBody, spin: SeasonalSpin) -> Option<SurfaceConditions> {
    SurfaceConditions::derive(&SurfaceInputs {
        atmosphere: body.atmosphere(),
        equilibrium_temperature: body.equilibrium_temperature(),
        surface_gravity: body.surface_gravity(),
        radius: Metres::from(body.radius()),
        stellar_flux: WattsPerSquareMetre::from(body.flux()),
        xuv_flux: body.xuv_flux(),
        spin,
    })
}

/// Every body of the table under P14.T24.f's rules, the batch's (held), with its own spin.
fn table() -> Vec<(&'static str, DerivedBody, SurfaceConditions)> {
    table_under(Speciation::CarbonAndOxygen)
        .into_iter()
        .map(|(name, body)| {
            let conditions = conditions_of(&body, spin_of(name)).unwrap();
            (name, body, conditions)
        })
        .collect()
}

fn found<'a>(
    table: &'a [(&'static str, DerivedBody, SurfaceConditions)],
    name: &str,
) -> &'a (&'static str, DerivedBody, SurfaceConditions) {
    table.iter().find(|(n, ..)| *n == name).unwrap()
}

/// The mole fraction of `gas` among `conditions`' gases; zero where it is not listed.
fn fraction(conditions: &SurfaceConditions, gas: Gas) -> f64 {
    conditions
        .gases()
        .iter()
        .find(|&&(id, _)| id == gas.substance())
        .map_or(0.0, |&(_, x)| x)
}

/// P14.T24.a is held until the 21 → 22 batch's bump, so that the batch's goldens move once: until
/// then `PlanetarySystem::hooks_at` publishes no surface conditions. The bump removes the hold, and
/// this test with it.
#[test]
fn the_surface_conditions_are_held_only_until_the_batch_s_bump() {
    assert!(
        GENERATOR_VERSION < SURFACE_CONDITIONS_VERSION,
        "the 21 → 22 bump publishes P14.T24.a's surface conditions: call `hooks_at` where \
         `hooks_at_under` is called, and remove it, `ConditionsHold`, \
         `SURFACE_CONDITIONS_VERSION` and this test (plan 14, P14.T24.a as built)"
    );
    assert_eq!(ConditionsHold::in_force(), ConditionsHold::Held);
}

#[test]
fn earth_has_liquid_water_and_mars_and_venus_do_not() {
    let table = table();
    for (name, body, conditions) in &table {
        eprintln!(
            "{name}: {:?}, {:.2} K, ep {:.2} K, dn {:.3} K, {:.4e} Pa, mu {:?}, cp/R {:?}, \
             liquid {}, {:?}, flux {:.1} W/m2, xuv {:.3e} W/m2",
            body.atmosphere().state(),
            conditions.mean_surface_temperature().value(),
            conditions.equator_pole_contrast_k(),
            conditions.day_night_contrast_k(),
            conditions.surface_pressure().value(),
            conditions.mean_molar_mass_g_per_mol(),
            conditions.heat_capacity_over_r(),
            conditions.liquid_water(),
            conditions.radiation(),
            conditions.stellar_flux().value(),
            body.xuv_flux().value(),
        );
        for &(id, x) in conditions.gases() {
            eprintln!("    {} {x:.6e}", id.substance().key().as_str());
        }
    }
    assert!(found(&table, "Earth").2.liquid_water());
    assert!(!found(&table, "Mars").2.liquid_water());
    assert!(!found(&table, "Venus").2.liquid_water());
}

#[test]
fn a_locked_airless_body_s_day_night_contrast_exceeds_300_k_and_venus_s_is_under_10_k() {
    let table = table();
    let (_, moon, moon_conditions) = found(&table, "Moon");
    assert_eq!(moon.atmosphere().state(), SurfaceState::Airless);
    // The Moon, locked to Earth, turns under the Sun once in 29.5 days; Mercury, locked 3:2, once
    // in 176 days.
    let moon_dn = moon_conditions.day_night_contrast_k();
    assert!(
        moon_dn > 300.0,
        "the Moon's day–night contrast is {moon_dn} K"
    );
    let (_, mercury, _) = found(&table, "Mercury");
    assert_eq!(mercury.atmosphere().state(), SurfaceState::Airless);
    // Mercury locked 1:1 to the Sun, its day–night contrast about its substellar axis.
    let locked = conditions_of(
        mercury,
        SeasonalSpin {
            solar_day: None,
            ..spin_of("Mercury")
        },
    )
    .unwrap();
    assert!(locked.day_night_contrast_k() > 300.0);
    let venus = found(&table, "Venus").2.day_night_contrast_k();
    assert!(
        (0.0..10.0).contains(&venus),
        "Venus's day–night contrast is {venus} K"
    );
    // A locked Venus would be as uniform: its thick air redistributes almost everything.
    let (_, venus_body, _) = found(&table, "Venus");
    let locked_venus = conditions_of(
        venus_body,
        SeasonalSpin {
            solar_day: None,
            ..spin_of("Venus")
        },
    )
    .unwrap();
    assert!(locked_venus.day_night_contrast_k() < 10.0);
}

#[test]
fn the_published_fractions_are_mole_fractions() {
    // Equal weights of N₂ and CO₂ give x(N₂) = μ_CO₂ ÷ (μ_CO₂ + μ_N₂), with the registry's molar
    // masses.
    let (n2, co2) = (Gas::Nitrogen, Gas::CarbonDioxide);
    let gases = mole_fractions([
        (co2.substance(), Pascals::new(3e4)),
        (n2.substance(), Pascals::new(3e4)),
    ])
    .unwrap();
    let (m_n2, m_co2) = (n2.molar_mass_g_per_mol(), co2.molar_mass_g_per_mol());
    assert_eq!(gases[0].0, n2.substance());
    assert!((gases[0].1 - m_co2 / (m_co2 + m_n2)).abs() < 1e-12);
    assert!((gases[1].1 - m_n2 / (m_co2 + m_n2)).abs() < 1e-12);
    // Every body's μ equals p_s ÷ Σ(p_j ÷ μ_j), its list sums to 1 and runs largest first.
    let table = table();
    for (name, body, conditions) in &table {
        let sum = conditions.gases().iter().fold(0.0, |sum, &(_, x)| sum + x);
        let Some(mu) = conditions.mean_molar_mass_g_per_mol() else {
            assert!(conditions.gases().is_empty(), "{name}");
            continue;
        };
        assert!((sum - 1.0).abs() < 1e-9, "{name}'s fractions sum to {sum}");
        let partials = body.atmosphere().partial_pressures();
        let moles = Gas::ALL.iter().fold(0.0, |sum, &gas| {
            sum + partials.of(gas).value() / gas.molar_mass_g_per_mol()
        });
        let by_weight = partials.total().value() / moles;
        assert!(
            (mu - by_weight).abs() < 1e-12,
            "{name}: {mu} against {by_weight}"
        );
        assert_eq!(
            conditions.surface_pressure(),
            body.atmosphere().surface_pressure().unwrap()
        );
        for pair in conditions.gases().windows(2) {
            assert!(pair[0].1 >= pair[1].1, "{name}'s gases run largest first");
        }
    }
    // The table's Venus carries N₂ at its weight over its molar mass: about 1.65% by number from
    // 0.61 of 57.8 bar, not the 1.05% its weight is.
    let (_, venus, conditions) = found(&table, "Venus");
    let partials = venus.atmosphere().partial_pressures();
    let by_mass = partials.of(n2).value() / partials.total().value();
    let by_number = fraction(conditions, n2);
    eprintln!("Venus's N2: {by_number:.5} by number, {by_mass:.5} by mass");
    assert!((0.0160..0.0170).contains(&by_number), "{by_number}");
    assert!((0.0100..0.0110).contains(&by_mass), "{by_mass}");
}

#[test]
fn equal_fractions_are_ordered_by_substance() {
    // Weights in the ratio of the molar masses hold equal moles: the lower row comes first.
    let (water, methane) = (Gas::Water, Gas::Methane);
    let gases = mole_fractions([
        (
            methane.substance(),
            Pascals::new(methane.molar_mass_g_per_mol()),
        ),
        (
            water.substance(),
            Pascals::new(water.molar_mass_g_per_mol()),
        ),
    ])
    .unwrap();
    assert_same_bits(gases[0].1, gases[1].1);
    assert!(water.substance() < methane.substance());
    assert_eq!(gases[0].0, water.substance());
    // A weight of nothing is no gas, and no weight at all is an empty list.
    assert!(
        mole_fractions([(water.substance(), Pascals::ZERO)])
            .unwrap()
            .is_empty()
    );
}

#[test]
fn a_weight_without_a_molar_mass_or_not_finite_is_refused() {
    // Every row of the registry so far has a molar mass, so a lookup that gives none stands in
    // for a later row that has none.
    let (water, methane) = (Gas::Water.substance(), Gas::Methane.substance());
    let unweighable = |id: SubstanceId| (id != methane).then(|| Gas::Water.molar_mass_g_per_mol());
    assert_eq!(
        mole_fractions_by(
            [(water, Pascals::new(1.0)), (methane, Pascals::new(1.0))],
            unweighable
        ),
        Err(CountMolesError::NoMolarMass(methane))
    );
    // A row of no weight is not counted, so it needs no molar mass.
    assert_eq!(
        mole_fractions_by([(methane, Pascals::ZERO)], unweighable),
        Ok(Vec::new())
    );
    for weight in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
        assert_eq!(
            mole_fractions([(water, Pascals::new(1.0)), (methane, Pascals::new(weight))]),
            Err(CountMolesError::NotFinite(methane)),
            "{weight}"
        );
    }
}

#[test]
fn an_airless_body_has_no_gases_and_no_mean_molar_mass() {
    let mut airless = 0;
    for (name, body, conditions) in table() {
        if body.atmosphere().state() != SurfaceState::Airless {
            continue;
        }
        airless += 1;
        assert!(conditions.gases().is_empty(), "{name}");
        assert_eq!(conditions.mean_molar_mass_g_per_mol(), None, "{name}");
        assert_eq!(conditions.heat_capacity_over_r(), None, "{name}");
        assert_eq!(conditions.surface_pressure(), Pascals::ZERO, "{name}");
        assert!(!conditions.liquid_water(), "{name}");
    }
    // Mercury, the Moon, Ganymede and Ceres.
    assert_eq!(airless, 4);
}

#[test]
fn the_mean_molar_mass_and_heat_capacity_are_the_registry_s_over_the_published_order() {
    for (name, _, conditions) in table() {
        assert_eq!(
            conditions.mean_molar_mass_g_per_mol(),
            mean_molar_mass_g_per_mol(conditions.gases()),
            "{name}"
        );
        assert_eq!(
            conditions.heat_capacity_over_r(),
            heat_capacity_over_r(conditions.gases()),
            "{name}"
        );
    }
}

#[test]
fn methane_and_oxygen_reach_the_fractions_through_the_speciated_derivation() {
    // P14.T24.f's Titan carries Huygens's 5.65% CH₄ by number to 10% (5.96% as built, at
    // 1.473 bar).
    let table = table();
    let (_, titan, conditions) = found(&table, "Titan");
    let methane = fraction(conditions, Gas::Methane);
    assert!(((methane - 0.0565) / 0.0565).abs() < 0.10, "{methane}");
    assert_eq!(conditions.gases()[0].0, Gas::Nitrogen.substance());
    assert_eq!(conditions.gases()[1].0, Gas::Methane.substance());
    assert!(titan.atmosphere().partial_pressures().of(Gas::Methane) > Pascals::ZERO);
    // The wet runaway world of `derive_body`'s golden keeps its lost oceans' oxygen.
    let wet = VolatileDraws {
        water: UnitUniform::new(0.99).unwrap(),
        ..VolatileDraws::MEDIAN
    };
    let o = orbit(0.6, 0.0);
    let runaway = PlacedBody::new(
        EarthMasses::new(1.0),
        o,
        o.semi_major_axis(),
        UnitUniform::HALF,
    )
    .unwrap()
    .with_volatiles(wet);
    let body = derive_under(
        &runaway,
        &solar_disc(),
        SOLAR_AGE,
        Speciation::CarbonAndOxygen,
    );
    let conditions = conditions_of(&body, spin_of("Venus")).unwrap();
    let oxygen = fraction(&conditions, Gas::Oxygen);
    eprintln!("the wet runaway world: O2 {oxygen:.4} by number");
    assert!(oxygen > 0.5, "{oxygen}");
    assert_eq!(conditions.gases()[0].0, Gas::Oxygen.substance());
    // Under the held rules neither gas is airborne.
    let held = table_under(Speciation::Earlier);
    for (name, body) in &held {
        let conditions = conditions_of(body, spin_of(name)).unwrap();
        assert_same_bits(fraction(&conditions, Gas::Methane), 0.0);
        assert_same_bits(fraction(&conditions, Gas::Oxygen), 0.0);
    }
}

/// Earth's measured state as [`MERIDIONAL_DIFFUSIVITY_M2_S`] is fitted to: its column, mean
/// surface temperature, absorbed flux, radius and obliquity.
fn earth_column() -> (Column, SeasonalSpin) {
    let absorbed = (1.0 - 0.294) * crate::units::consts::SOLAR_CONSTANT_W_PER_M2 / 4.0;
    let column_mass = 5.1480e18 / 5.100e14;
    let column = Column {
        surface_temperature: Kelvin::new(288.0),
        equilibrium_temperature: Kelvin::new(math::powf(absorbed / STEFAN_BOLTZMANN, 0.25)),
        surface_pressure: Pascals::new(column_mass * 9.80),
        heat_capacity_j_per_m2_k: specific_heat_j_per_kg_k(3.5, 28.964_7) * column_mass,
        optical_depth: 0.8,
        radius: Metres::new(6_371.0e3),
    };
    let spin = SeasonalSpin {
        obliquity: Radians::from(Degrees::new(23.44)),
        rotation_period: EARTH_SIDEREAL_DAY,
        solar_day: Some(Seconds::new(SECONDS_PER_DAY)),
    };
    (column, spin)
}

#[test]
fn the_meridional_diffusivity_gives_earth_its_measured_contrast() {
    let (column, spin) = earth_column();
    // The fit, by hand: 6D ÷ B is the radiative contrast over the measured one, less 1.
    let restoring = column.restoring_w_per_m2_k();
    let (equator, pole) = annual_insolation(spin.obliquity);
    let radiative = 288.0 * (math::powf(equator, 0.25) - math::powf(pole, 0.25));
    let ratio = radiative / EARTH_EQUATOR_POLE_CONTRAST_K - 1.0;
    let fitted = ratio * restoring * math::powi(column.radius.value(), 2)
        / (6.0 * column.heat_capacity_j_per_m2_k);
    eprintln!("radiative {radiative} K, 6D/B {ratio}, K {fitted} m2/s");
    assert!(
        (fitted / MERIDIONAL_DIFFUSIVITY_M2_S - 1.0).abs() < 1e-7,
        "{fitted}"
    );
    let contrast = equator_pole_contrast_k(&column, &spin, Moisture::Wet);
    assert!(
        (contrast - EARTH_EQUATOR_POLE_CONTRAST_K).abs() < 1e-5,
        "Earth's contrast is {contrast} K"
    );
    // Dry, the same Earth keeps half its transport and a larger contrast.
    assert!(equator_pole_contrast_k(&column, &spin, Moisture::Dry) > contrast);
    // The annual insolation, by quadrature of its definitions at 23.44°: 1.2213 at the equator and
    // 0.5065 at a pole, each over S ÷ 4.
    assert!((equator - 1.221_269).abs() < 1e-6, "{equator}");
    assert!((pole - 0.506_480).abs() < 1e-6, "{pole}");
}

#[test]
fn the_annual_insolation_is_its_definitions_mean() {
    // The year's mean of the daily-mean insolation at the equator, (4 ÷ π) cos δ over S ÷ 4, and
    // at a pole, 4 sin δ in its summer, by the midpoint rule over the sun's longitude.
    let steps = 20_000;
    for degrees in [0.0, 1.0, 23.44, 53.9, 90.0, 120.0, 177.36] {
        let beta = Radians::from(Degrees::new(degrees)).value();
        let (mut equator, mut pole) = (0.0, 0.0);
        for i in 0..steps {
            let lambda = (f64::from(i) + 0.5) / f64::from(steps) * 2.0 * core::f64::consts::PI;
            let sin_delta = math::sin(beta) * math::sin(lambda);
            equator += 4.0 / core::f64::consts::PI * (1.0 - sin_delta * sin_delta).sqrt();
            pole += 4.0 * sin_delta.max(0.0);
        }
        let (equator, pole) = (equator / f64::from(steps), pole / f64::from(steps));
        let (closed_equator, closed_pole) = annual_insolation(Radians::new(beta));
        assert!(
            (closed_equator - equator).abs() < 1e-8,
            "{degrees}°: {closed_equator} against {equator}"
        );
        assert!(
            (closed_pole - pole).abs() < 1e-6,
            "{degrees}°: {closed_pole} against {pole}"
        );
    }
    // E(0) = π ÷ 2 and E(1) = 1, and the global mean of the zonal insolation is 1.
    assert_same_bits(complete_elliptic_e(0.0), core::f64::consts::FRAC_PI_2);
    assert_same_bits(complete_elliptic_e(1.0), 1.0);
    assert!((complete_elliptic_e(1.0 - 1e-12) - 1.0).abs() < 1e-9);
}

#[test]
fn a_ninety_degree_obliquity_world_s_equator_pole_contrast_is_negative() {
    let (column, spin) = earth_column();
    let at = |degrees: f64| {
        let spin = SeasonalSpin {
            obliquity: Radians::from(Degrees::new(degrees)),
            ..spin
        };
        equator_pole_contrast_k(&column, &spin, Moisture::Wet)
    };
    assert!(at(90.0) < 0.0);
    // At 90° a pole takes 4 ÷ π of the mean light and the equator 8 ÷ π², so the warm pole's
    // contrast is (8 ÷ π²)^¼ − (4 ÷ π)^¼ over the cold pole's (4 ÷ π)^¼ at 0°, by hand.
    let pi = core::f64::consts::PI;
    let by_hand = (math::powf(8.0 / (pi * pi), 0.25) - math::powf(4.0 / pi, 0.25))
        / math::powf(4.0 / pi, 0.25);
    assert!(
        (at(90.0) / at(0.0) - by_hand).abs() < 1e-12,
        "{}",
        at(90.0) / at(0.0)
    );
    // The sign changes where the equator's and the poles' light are equal, at 53.90°, and the
    // contrast is symmetric about 90°.
    assert!(
        at(53.8) > 0.0 && at(54.0) < 0.0,
        "{} {}",
        at(53.8),
        at(54.0)
    );
    assert!(at(126.0) < 0.0 && at(126.2) > 0.0);
    assert!((at(30.0) - at(150.0)).abs() < 1e-9);
    // Through the derivation: the table's Earth tilted to 90°.
    let table = table();
    let (_, earth, conditions) = found(&table, "Earth");
    assert!(conditions.equator_pole_contrast_k() > 0.0);
    let tilted = conditions_of(
        earth,
        SeasonalSpin {
            obliquity: Radians::from(Degrees::new(90.0)),
            ..spin_of("Earth")
        },
    )
    .unwrap();
    assert!(tilted.equator_pole_contrast_k() < 0.0);
}

#[test]
fn a_moon_s_contrasts_read_its_obliquity_to_its_planet_s_orbit() {
    // A Titan: its orbit in its planet's equator, 26.73° from the planet's orbit, and its spin
    // along its own orbit's normal. Its seasons are its planet's at that tilt.
    let tilt = Radians::from(Degrees::new(26.73));
    let saturn = |mu: GravitationalParameter, a: f64, inclination: Radians| {
        KeplerElements::from_semi_major_axis(
            Metres::new(a),
            mu,
            Eccentricity::new(0.0).unwrap(),
            Orientation::new(inclination, Radians::ZERO, Radians::ZERO).unwrap(),
            Radians::ZERO,
        )
        .unwrap()
    };
    let planet_orbit = saturn(
        GravitationalParameter::from_solar_masses(SolarMasses::new(1.0)),
        9.58 * METRES_PER_AU,
        Radians::ZERO,
    );
    let moon_orbit = saturn(GravitationalParameter::new(3.793_1e16), 1.221_87e9, tilt);
    let frame = BodyFixedFrame::new(
        &moon_orbit,
        &FrameSpin {
            obliquity: Radians::ZERO,
            pole_azimuth: Radians::ZERO,
            primordial_period: Seconds::new(15.945 * SECONDS_PER_DAY),
            locking_time: Seconds::new(1e10),
            phase_at_epoch: Radians::ZERO,
        },
        Years::new(4.57e9),
    )
    .unwrap();
    let t = UniverseTime::EPOCH;
    let seasonal = SeasonalSpin::of(&frame, t, &planet_orbit, SeasonsFrom::Planet);
    assert!(
        (seasonal.obliquity.value() - tilt.value()).abs() < 1e-9,
        "{:?}",
        seasonal.obliquity
    );
    // Its own orbit would give it none, and no seasons' contrast.
    let own = SeasonalSpin::of(&frame, t, &moon_orbit, SeasonsFrom::Own);
    assert!(own.obliquity.value().abs() < 1e-12);
    // Locked to its planet, it still turns under the Sun, once in about its orbit.
    let day = seasonal.solar_day.unwrap().value() / SECONDS_PER_DAY;
    assert!((day - 15.97).abs() < 0.05, "{day} d");
    assert_eq!(own.solar_day, None);
    let (column, _) = earth_column();
    let tilted = equator_pole_contrast_k(&column, &seasonal, Moisture::Wet);
    let untilted = equator_pole_contrast_k(&column, &own, Moisture::Wet);
    let at_tilt = SeasonalSpin {
        obliquity: tilt,
        ..own
    };
    assert!((tilted - equator_pole_contrast_k(&column, &at_tilt, Moisture::Wet)).abs() < 1e-9);
    assert!(
        untilted - tilted > 1.0,
        "{untilted} K untilted, {tilted} K tilted"
    );
}

#[test]
fn the_solar_day_is_the_sun_s_rate_about_the_pole() {
    // Venus, turning retrograde once in 243.025 d on a 224.701 d orbit: 116.75 d, through a frame
    // tilted 177.36° and through the table's spin.
    let venus_orbit = KeplerElements::from_period(
        Seconds::new(224.701 * SECONDS_PER_DAY),
        GravitationalParameter::from_solar_masses(SolarMasses::new(1.0)),
        Eccentricity::new(0.0068).unwrap(),
        Orientation::new(Radians::ZERO, Radians::ZERO, Radians::ZERO).unwrap(),
        Radians::ZERO,
    )
    .unwrap();
    let venus_frame = BodyFixedFrame::new(
        &venus_orbit,
        &FrameSpin {
            obliquity: Radians::from(Degrees::new(177.36)),
            pole_azimuth: Radians::ZERO,
            primordial_period: Seconds::new(243.025 * SECONDS_PER_DAY),
            locking_time: Seconds::new(f64::INFINITY),
            phase_at_epoch: Radians::ZERO,
        },
        Years::new(4.57e9),
    )
    .unwrap();
    let retrograde = SeasonalSpin::of(
        &venus_frame,
        UniverseTime::EPOCH,
        &venus_orbit,
        SeasonsFrom::Own,
    );
    let day = retrograde.solar_day.unwrap().value() / SECONDS_PER_DAY;
    assert!((day - 116.75).abs() < 0.02, "{day} d");
    let venus = spin_of("Venus").solar_day.unwrap().value() / SECONDS_PER_DAY;
    assert!((venus - 116.75).abs() < 0.02, "{venus} d");
    let mercury = spin_of("Mercury").solar_day.unwrap().value() / SECONDS_PER_DAY;
    assert!((mercury - 175.94).abs() < 0.02, "{mercury} d");
    let earth = spin_of("Earth").solar_day.unwrap().value() / SECONDS_PER_DAY;
    assert!((earth - 1.0).abs() < 1e-4, "{earth} d");
    // A planet locked 1:1 to its star has none.
    let orbit = orbit(0.05, 0.0);
    let frame = BodyFixedFrame::new(
        &orbit,
        &FrameSpin {
            obliquity: Radians::from(Degrees::new(5.0)),
            pole_azimuth: Radians::ZERO,
            primordial_period: Seconds::new(SECONDS_PER_DAY),
            locking_time: Seconds::new(1e13),
            phase_at_epoch: Radians::ZERO,
        },
        Years::new(4.57e9),
    )
    .unwrap();
    assert_eq!(
        frame.state_at(UniverseTime::EPOCH),
        SpinState::Locked(SpinOrbitResonance::Synchronous)
    );
    let locked = SeasonalSpin::of(&frame, UniverseTime::EPOCH, &orbit, SeasonsFrom::Own);
    assert_eq!(locked.solar_day, None);
    assert!((locked.rotation_period.value() / orbit.period().value() - 1.0).abs() < 1e-12);
}

#[test]
fn the_rotation_factor_follows_kaspi_and_showman() {
    let at =
        |ratio: f64| rotation_transport_factor(Seconds::new(EARTH_SIDEREAL_DAY.value() / ratio));
    assert!((at(1.0) - 1.0).abs() < 1e-15);
    // R09's transport law: about 1.7 at Ω⊕ ÷ 8 and 0.52 at 4 Ω⊕.
    assert!((at(1.0 / 8.0) - 1.68).abs() < 0.01, "{}", at(1.0 / 8.0));
    assert!((at(4.0) - 0.5).abs() < 1e-12);
    // Held beyond the runs' range, and continuous through Earth's rate.
    assert_same_bits(at(1.0 / 100.0), at(1.0 / 24.0));
    assert_same_bits(at(100.0), at(12.0));
    assert!((at(1.0 - 1e-9) - at(1.0 + 1e-9)).abs() < 1e-8);
    assert_same_bits(
        rotation_transport_factor(Seconds::new(f64::INFINITY)),
        at(1.0 / 24.0),
    );
}

#[test]
fn the_redistribution_is_koll_s() {
    let (earth, _) = earth_column();
    // By hand: X = τ^⅓ (p ÷ 1 bar)^⅔ (T_eq ÷ 600 K)^−4/3, ε = X ÷ (2 + X).
    let x = math::cbrt(earth.optical_depth)
        * math::powf(earth.surface_pressure.value() / 1e5, 2.0 / 3.0)
        * math::powf(earth.equilibrium_temperature.value() / 600.0, -4.0 / 3.0);
    let epsilon = heat_redistribution(&earth);
    assert!((epsilon - x / (2.0 + x)).abs() < 1e-15);
    assert!(epsilon > 0.5 && epsilon < 0.7, "{epsilon}");
    // No air or no greenhouse redistributes nothing; a deep air nearly all.
    let bare = Column {
        surface_pressure: Pascals::ZERO,
        ..earth
    };
    assert_same_bits(heat_redistribution(&bare), 0.0);
    let clear = Column {
        optical_depth: 0.0,
        ..earth
    };
    assert_same_bits(heat_redistribution(&clear), 0.0);
    let deep = Column {
        surface_pressure: Pascals::new(9e6),
        optical_depth: 100.0,
        ..earth
    };
    assert!(heat_redistribution(&deep) > 0.99);
}

#[test]
fn a_rotating_column_damps_the_day_night_contrast() {
    let (earth, spin) = earth_column();
    let at = |days: Option<f64>| {
        day_night_contrast_k(
            &earth,
            &SeasonalSpin {
                solar_day: days.map(|d| Seconds::new(d * SECONDS_PER_DAY)),
                ..spin
            },
        )
    };
    let locked = at(None);
    // Cowan and Agol's contrast at Koll's ε, by hand.
    let epsilon = heat_redistribution(&earth);
    let by_hand =
        288.0 * (math::powf(8.0 / 3.0 - 5.0 / 3.0 * epsilon, 0.25) - math::powf(epsilon, 0.25));
    assert!((locked - by_hand).abs() < 1e-9);
    // Longer days damp less, and the locked contrast is the limit of an ever longer day.
    let mut last = 0.0;
    for days in [0.1, 1.0, 10.0, 100.0, 1e4, 1e8] {
        let contrast = at(Some(days));
        assert!(
            contrast > last && contrast < locked,
            "{days} d: {contrast} K"
        );
        last = contrast;
    }
    assert!((at(Some(1e12)) / locked - 1.0).abs() < 1e-9);
    // An airless body's is its dayside's radiative temperature over a 0 K night, whatever its day.
    let airless = Column {
        surface_pressure: Pascals::ZERO,
        heat_capacity_j_per_m2_k: 0.0,
        optical_depth: 0.0,
        ..earth
    };
    let bare = day_night_contrast_k(&airless, &spin);
    assert!((bare - 288.0 * math::powf(8.0 / 3.0, 0.25)).abs() < 1e-9);
    assert_same_bits(
        bare,
        day_night_contrast_k(
            &airless,
            &SeasonalSpin {
                solar_day: None,
                ..spin
            },
        ),
    );
}

#[test]
fn no_light_gives_no_contrast() {
    let (earth, spin) = earth_column();
    let dark = Column {
        equilibrium_temperature: Kelvin::ZERO,
        ..earth
    };
    assert_same_bits(equator_pole_contrast_k(&dark, &spin, Moisture::Wet), 0.0);
    assert_same_bits(day_night_contrast_k(&dark, &spin), 0.0);
    assert_same_bits(heat_redistribution(&dark), 1.0);
}

#[test]
fn the_radiation_class_bands_are_decades_of_earth_s() {
    let earth = earth_xuv_flux().value();
    // The generated Sun's present X-ray and ultraviolet flux at 1 au, about 9.75 × 10⁻⁴ W m⁻².
    assert!((earth - 9.75e-4).abs() < 0.01e-4, "{earth}");
    let class = |ratio: f64| RadiationClass::of(WattsPerSquareMetre::new(ratio * earth));
    assert_eq!(
        RadiationClass::of(WattsPerSquareMetre::ZERO),
        RadiationClass::Dark
    );
    assert_eq!(class(f64::NAN), RadiationClass::Dark);
    assert_eq!(class(1.0), RadiationClass::Quiet);
    assert_eq!(class(10.0), RadiationClass::Active);
    assert_eq!(class(99.9), RadiationClass::Active);
    assert_eq!(class(100.0), RadiationClass::Intense);
    assert_eq!(class(1_000.0), RadiationClass::Extreme);
    // The saturated young Sun at 1 au: some 300 times today's.
    let young = XuvHistory::new(SolarMasses::new(1.0), REFERENCE_SUN_ZAMS_LUMINOSITY).flux(
        Years::new(5e7),
        Metres::new(METRES_PER_AU),
        0.0,
    );
    assert_eq!(RadiationClass::of(young), RadiationClass::Intense);
    // The table's planets today are quiet.
    for (name, _, conditions) in table() {
        assert_eq!(conditions.radiation(), RadiationClass::Quiet, "{name}");
    }
}

#[test]
fn a_gas_envelope_has_no_surface_conditions() {
    let disc = solar_disc();
    let &(_, kg, _, a, e) = PLANETS.iter().find(|(n, ..)| *n == "Jupiter").unwrap();
    let jupiter = crate::planetary::derive::solar::derive(
        &crate::planetary::derive::solar::placed(kg, a, e, 0.5),
        &disc,
    )
    .unwrap();
    assert_eq!(jupiter.atmosphere().state(), SurfaceState::GasEnvelope);
    assert_eq!(conditions_of(&jupiter, spin_of("Earth")), None);
}

#[test]
fn the_conditions_are_continuous_in_obliquity() {
    let table = table();
    for (name, body, _) in &table {
        let base = spin_of(name);
        let mut last: Option<SurfaceConditions> = None;
        for step in 0..=1_800 {
            let spin = SeasonalSpin {
                obliquity: Radians::from(Degrees::new(f64::from(step) * 0.1)),
                ..base
            };
            let conditions = conditions_of(body, spin).unwrap();
            if let Some(previous) = &last {
                // Two degrees and more from 0° and 180°, where a pole's light rises as
                // (sin β)^¼, a tenth of a degree moves the contrast by under a percent of the
                // mean temperature.
                let jump = (conditions.equator_pole_contrast_k()
                    - previous.equator_pole_contrast_k())
                .abs();
                let degrees = f64::from(step) * 0.1;
                if (2.0..=178.0).contains(&degrees) {
                    assert!(
                        jump < 0.01 * conditions.mean_surface_temperature().value(),
                        "{name} at {degrees}°: {jump} K"
                    );
                }
                assert_same_bits(
                    conditions.day_night_contrast_k(),
                    previous.day_night_contrast_k(),
                );
            }
            last = Some(conditions);
        }
        // At 0° the contrast is continuous too, its step shrinking as the step's fourth root.
        let at = |degrees: f64| {
            conditions_of(
                body,
                SeasonalSpin {
                    obliquity: Radians::from(Degrees::new(degrees)),
                    ..base
                },
            )
            .unwrap()
            .equator_pole_contrast_k()
        };
        let mut last_jump = f64::INFINITY;
        for degrees in [1e-2, 1e-4, 1e-6, 1e-8] {
            let jump = (at(degrees) - at(0.0)).abs();
            assert!(jump < last_jump, "{name} at {degrees}°: {jump} K");
            last_jump = jump;
        }
        assert!(
            last_jump < 0.01 * at(0.0).abs().max(1.0),
            "{name}: {last_jump} K"
        );
    }
}

#[test]
fn the_conditions_are_the_same_twice() {
    let first = table();
    let second = table();
    assert_eq!(first.len(), second.len());
    for ((name, _, a), (_, _, b)) in first.iter().zip(&second) {
        assert_eq!(a, b, "{name}");
    }
}

#[test]
fn the_small_bodies_of_the_table_are_spun() {
    for (name, ..) in SMALL_BODIES {
        assert!(SPINS.iter().any(|(n, ..)| *n == name), "{name}");
    }
}

#[test]
fn the_conditions_are_pinned() {
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;

    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    let pin = |w: &mut GoldenWriter, name: &str, c: &SurfaceConditions| {
        w.f64(
            &format!("{name}_mean_surface_temperature"),
            c.mean_surface_temperature().value(),
        );
        w.f64(
            &format!("{name}_equator_pole_contrast_k"),
            c.equator_pole_contrast_k(),
        );
        w.f64(
            &format!("{name}_day_night_contrast_k"),
            c.day_night_contrast_k(),
        );
        w.f64(
            &format!("{name}_surface_pressure"),
            c.surface_pressure().value(),
        );
        for &(id, x) in c.gases() {
            w.f64(
                &format!("{name}_mole_fraction_{}", id.substance().key().as_str()),
                x,
            );
        }
        for (label, value) in [
            ("mean_molar_mass_g_per_mol", c.mean_molar_mass_g_per_mol()),
            ("heat_capacity_over_r", c.heat_capacity_over_r()),
        ] {
            match value {
                Some(value) => w.f64(&format!("{name}_{label}"), value),
                None => w.line(&format!("{name}_{label} = None")),
            }
        }
        w.f64(&format!("{name}_stellar_flux"), c.stellar_flux().value());
        w.line(&format!("{name}_radiation = {:?}", c.radiation()));
        w.line(&format!("{name}_liquid_water = {}", c.liquid_water()));
    };
    let table = table();
    for (name, body, conditions) in &table {
        let name = name.to_lowercase();
        pin(&mut w, &name, conditions);
        w.f64(&format!("{name}_xuv_flux"), body.xuv_flux().value());
    }
    // Locked 1:1, an airless body's contrast is its rotating one; an aired one's is undamped.
    for name in ["Earth", "Venus"] {
        let (_, body, _) = found(&table, name);
        let locked = conditions_of(
            body,
            SeasonalSpin {
                solar_day: None,
                ..spin_of(name)
            },
        )
        .unwrap();
        pin(&mut w, &format!("{}_locked", name.to_lowercase()), &locked);
    }
    let (_, earth, _) = found(&table, "Earth");
    let tilted = conditions_of(
        earth,
        SeasonalSpin {
            obliquity: Radians::from(Degrees::new(90.0)),
            ..spin_of("Earth")
        },
    )
    .unwrap();
    pin(&mut w, "earth_at_90_degrees", &tilted);
    w.f64("earth_xuv_flux", earth_xuv_flux().value());
    let young = XuvHistory::new(SolarMasses::new(1.0), REFERENCE_SUN_ZAMS_LUMINOSITY).flux(
        Years::new(5e7),
        Metres::new(METRES_PER_AU),
        0.0,
    );
    w.f64("young_sun_xuv_flux_at_1_au", young.value());
    w.line(&format!(
        "young_sun_radiation_at_1_au = {:?}",
        RadiationClass::of(young)
    ));
    golden!("planetary/surface_conditions", w.as_str());
}
