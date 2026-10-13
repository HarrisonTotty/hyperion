//! P14.T48.f's impactors against the Solar System, on JPL's elements and masses, and P14.T24.b's
//! global figures on the Solar System table.

use hyperion_testkit::float::assert_same_bits;

use super::*;
use crate::planetary::belts::{BeltHost, FIRST_BELT_SLOT, host_belts};
use crate::planetary::derive::DerivedBody;
use crate::planetary::derive::solar::{SOLAR_AGE, solar_disc};
use crate::planetary::moons::regular::DerivedMoon;
use crate::planetary::placement::OrbitHost;
use crate::planetary::placement::classes::orbits::SystemPlane;
use crate::planetary::placement::spacing::Neighbour;
use crate::substance;
use crate::units::consts::{EARTH_MASS_KG, GRAVITATIONAL_CONSTANT, METRES_PER_AU, SECONDS_PER_DAY};
use crate::units::{Degrees, SolarMasses};
use crate::{GENERATOR_VERSION, Seed, id::SystemId};
use hyperion_surface::craters::LUNAR_IMPACT_VELOCITY;

/// The planets: name, semi-major axis (au), eccentricity and inclination to the ecliptic (deg),
/// JPL's "Keplerian Elements for Approximate Positions of the Major Planets", Table 1 (J2000,
/// 1800–2050; E. M. Standish, JPL Solar System Dynamics), with the Earth–Moon barycentre for
/// Earth; then mass (10²⁴ kg) and volumetric mean radius (km), JPL's "Planetary Physical
/// Parameters" (Solar System Dynamics, read 2026-10-12). The ecliptic stands for the host's plane.
const PLANETS: [(&str, f64, f64, f64, f64, f64); 7] = [
    (
        "Mercury",
        0.387_099_27,
        0.205_635_93,
        7.004_979_02,
        0.330_103,
        2_439.4,
    ),
    (
        "Venus",
        0.723_335_66,
        0.006_776_72,
        3.394_676_05,
        4.867_31,
        6_051.8,
    ),
    (
        "Earth",
        1.000_002_61,
        0.016_711_23,
        -0.000_015_31,
        5.972_17,
        6_371.008_4,
    ),
    (
        "Mars",
        1.523_710_34,
        0.093_394_10,
        1.849_691_42,
        0.641_691,
        3_389.50,
    ),
    (
        "Jupiter",
        5.202_887_00,
        0.048_386_24,
        1.304_396_95,
        1_898.125,
        69_911.0,
    ),
    (
        "Saturn",
        9.536_675_94,
        0.053_861_79,
        2.485_991_87,
        568.317,
        58_232.0,
    ),
    (
        "Uranus",
        19.189_164_64,
        0.047_257_44,
        0.772_637_83,
        86.809_9,
        25_362.0,
    ),
];

/// Neptune's semi-major axis, au (JPL's Table 1, as [`PLANETS`]), whose 3:2 and 2:1 resonances
/// bound the Kuiper belt proper.
const NEPTUNE_AU: f64 = 30.069_922_76;

/// Ceres and Pluto: semi-major axis (au), eccentricity and inclination (deg), JPL's Small-Body
/// Database osculating elements (Ceres at JD 2461200.5, Pluto at JD 2457588.5; J2000 ecliptic;
/// read 2026-10-12); then mass (10²⁴ kg) and mean radius (km), JPL's "Planetary Physical
/// Parameters" (dwarf planets).
const CERES: (f64, f64, f64, f64, f64) = (
    2.765_552_595_034_094,
    0.079_692_295_148_165_86,
    10.588_027_801_834_62,
    0.938_416e-3,
    469.7,
);
const PLUTO: (f64, f64, f64, f64, f64) = (
    39.588_629_385_171_24,
    0.251_837_877_857_689_2,
    17.147_711_409_991_14,
    13.024_6e-3,
    1_188.3,
);

/// The regular moons of Jupiter, Saturn and Uranus, and Earth's Moon: name, parent, semi-major
/// axis (km) from JPL's "Planetary Satellite Mean Elements" (read 2026-10-12), GM (km³ s⁻²) and
/// mean radius (km) from its "Planetary Satellite Physical Parameters" (read 2026-10-12), and the
/// mean impact velocity (km/s) of Brasser, Wong and Werner 2025 (A&A 695, A276; arXiv
/// 2501.17509), Table 5, as
/// science-r09-craters.md item 4 read it.
const MOONS: [(&str, &str, f64, f64, f64, f64); 15] = [
    ("Io", "Jupiter", 421_800.0, 5_959.915_47, 1_821.49, 32.0),
    ("Europa", "Jupiter", 671_100.0, 3_202.712_10, 1_560.80, 24.9),
    (
        "Ganymede",
        "Jupiter",
        1_070_400.0,
        9_887.832_75,
        2_631.20,
        20.4,
    ),
    (
        "Callisto",
        "Jupiter",
        1_882_700.0,
        7_179.283_40,
        2_410.30,
        15.4,
    ),
    ("Mimas", "Saturn", 186_000.0, 2.503_49, 198.20, 23.2),
    ("Enceladus", "Saturn", 238_400.0, 7.210_37, 252.10, 21.7),
    ("Tethys", "Saturn", 295_000.0, 41.213_53, 531.10, 21.0),
    ("Dione", "Saturn", 377_700.0, 73.116_07, 561.40, 18.8),
    ("Rhea", "Saturn", 527_200.0, 153.941_75, 763.50, 15.1),
    ("Titan", "Saturn", 1_221_900.0, 8_978.137_10, 2_574.76, 10.8),
    ("Miranda", "Uranus", 129_846.0, 4.3, 235.8, 12.4),
    ("Ariel", "Uranus", 190_929.0, 83.5, 578.9, 10.2),
    ("Umbriel", "Uranus", 265_986.0, 85.1, 584.7, 8.7),
    ("Titania", "Uranus", 436_298.0, 226.9, 788.9, 7.0),
    ("Oberon", "Uranus", 583_511.0, 205.3, 761.4, 6.1),
];

/// The Moon: semi-major axis (km), JPL's mean elements; GM (km³ s⁻²) and mean radius (km), its
/// physical parameters (DE440).
const MOON: (f64, f64, f64) = (384_400.0, 4_902.800, 1_737.4);

/// Hyperion: semi-major axis (km), GM (km³ s⁻²), mean radius (km) and mean density (kg m⁻³), as
/// [`MOONS`] (JPL; 538.6 kg m⁻³, Thomas et al. 2007's 544 within its error).
const HYPERION: (f64, f64, f64, f64) = (1_481_500.0, 0.370_49, 135.00, 538.6);

/// A length of `x` astronomical units.
fn au(x: f64) -> Metres {
    Metres::new(x * METRES_PER_AU)
}

/// A mass of `kg` kilograms in Earth masses, as the generator's units convert it.
fn earths(kg: f64) -> EarthMasses {
    EarthMasses::new(kg / EARTH_MASS_KG)
}

/// A belt annulus from `inner` to `outer` au.
fn annulus(inner: f64, outer: f64) -> SourceAnnulus {
    SourceAnnulus {
        inner_edge: au(inner),
        outer_edge: au(outer),
    }
}

/// The Sun's gravitational parameter, the nominal GM☉.
fn sun_parameter() -> GravitationalParameter {
    GravitationalParameter::from_solar_masses(SolarMasses::new(1.0))
}

/// The planets of [`PLANETS`] as the sources read them: their masses and semi-major axes.
fn solar_planets() -> impl Iterator<Item = (EarthMasses, Metres)> {
    PLANETS
        .iter()
        .map(|&(_, a, .., mass, _)| (earths(mass * 1e24), au(a)))
}

/// The Solar System's belts: the asteroid belt between Jupiter's 4:1 and 2:1 resonances (rocky,
/// P14.T21.a), and the Kuiper belt from Neptune's 3:2 to its 2:1 with a scattered component
/// beyond (icy, P14.T21.b).
fn solar_belts() -> Vec<SourceBelt> {
    let jupiter = PLANETS[4].1;
    let resonance = |p: f64, q: f64, a: f64| a * math::powf(q / p, 2.0 / 3.0);
    vec![
        SourceBelt {
            kind: BeltKind::Asteroid,
            composition: BeltComposition::Rocky,
            main: annulus(resonance(4.0, 1.0, jupiter), resonance(2.0, 1.0, jupiter)),
            scattered: None,
        },
        SourceBelt {
            kind: BeltKind::Kuiper,
            composition: BeltComposition::Icy,
            main: annulus(
                resonance(2.0, 3.0, NEPTUNE_AU),
                resonance(1.0, 2.0, NEPTUNE_AU),
            ),
            scattered: Some(annulus(resonance(1.0, 2.0, NEPTUNE_AU), 100.0)),
        },
    ]
}

/// The Solar System's sources: the Sun, [`solar_belts`] and Jupiter the innermost giant.
fn solar_sources() -> ImpactorSources {
    ImpactorSources::new(sun_parameter(), solar_belts(), solar_planets()).unwrap()
}

/// The orbit of `a` au, eccentricity `e` and inclination `i` degrees.
fn orbit(a: f64, e: f64, i: f64) -> HostOrbit {
    HostOrbit {
        semi_major_axis: au(a),
        eccentricity: e,
        inclination: Radians::from(Degrees::new(i)),
    }
}

/// The planet `name` of [`PLANETS`], as a host orbiter.
fn planet(name: &str) -> HostOrbiter {
    let &(_, a, e, i, mass, _) = PLANETS.iter().find(|p| p.0 == name).unwrap();
    HostOrbiter {
        orbit: orbit(a, e, i),
        mass: earths(mass * 1e24),
        belt: None,
    }
}

/// A body of mass `kg` and mean radius `radius_km` on `orbit`, at `temperature` K with `water` kg.
fn body(
    orbit: ImpactorOrbit,
    kg: f64,
    radius_km: f64,
    temperature: f64,
    water: f64,
    sources: &ImpactorSources,
) -> ImpactorBody<'_> {
    let r = radius_km * 1e3;
    let volume = 4.0 / 3.0 * core::f64::consts::PI * r * r * r;
    ImpactorBody {
        orbit,
        radius: Metres::new(r),
        density: KilogramsPerCubicMetre::new(kg / volume),
        surface_gravity: MetresPerSecondSquared::new(GRAVITATIONAL_CONSTANT * kg / (r * r)),
        surface_temperature: Kelvin::new(temperature),
        water: Kilograms::new(water),
        sources,
    }
}

/// The impactors of the planet `name` under `sources`, with no water.
fn planet_impactors(name: &str, sources: &ImpactorSources) -> Impactors {
    let &(.., mass, radius) = PLANETS.iter().find(|p| p.0 == name).unwrap();
    let orbit = ImpactorOrbit::Host(planet(name));
    impactors(&body(orbit, mass * 1e24, radius, 200.0, 0.0, sources)).unwrap()
}

/// The impactors of a moon of `parent` at `a_km`, of GM `gm` km³ s⁻² and mean radius `radius_km`,
/// formed on `formed`, at `temperature` K with `water` kg.
fn moon_impactors(
    parent: &str,
    (a_km, gm, radius_km): (f64, f64, f64),
    formed: SnowLineSide,
    (temperature, water): (f64, f64),
    sources: &ImpactorSources,
) -> Impactors {
    let orbit = ImpactorOrbit::Moon {
        parent: planet(parent),
        semi_major_axis: Metres::new(a_km * 1e3),
        formed,
    };
    let kg = gm * 1e9 / GRAVITATIONAL_CONSTANT;
    impactors(&body(orbit, kg, radius_km, temperature, water, sources)).unwrap()
}

/// The Moon's impactors under `sources`.
fn the_moon(sources: &ImpactorSources) -> Impactors {
    moon_impactors("Earth", MOON, SnowLineSide::Inside, (250.0, 0.0), sources)
}

/// Ceres, a member of the asteroid belt proper, at `temperature` K with `water` kg.
fn ceres(temperature: f64, water: f64, sources: &ImpactorSources) -> Impactors {
    let (a, e, i, mass, radius) = CERES;
    let orbit = ImpactorOrbit::Host(HostOrbiter {
        orbit: orbit(a, e, i),
        mass: earths(mass * 1e24),
        belt: Some(BeltMembership {
            part: BeltPart::Main,
            composition: BeltComposition::Rocky,
        }),
    });
    impactors(&body(
        orbit,
        mass * 1e24,
        radius,
        temperature,
        water,
        sources,
    ))
    .unwrap()
}

/// Pluto, a member of the Kuiper belt proper.
fn pluto(sources: &ImpactorSources) -> Impactors {
    let (a, e, i, mass, radius) = PLUTO;
    let orbit = ImpactorOrbit::Host(HostOrbiter {
        orbit: orbit(a, e, i),
        mass: earths(mass * 1e24),
        belt: Some(BeltMembership {
            part: BeltPart::Main,
            composition: BeltComposition::Icy,
        }),
    });
    impactors(&body(orbit, mass * 1e24, radius, 40.0, 0.0, sources)).unwrap()
}

/// The cube of `x`.
fn cube(x: f64) -> f64 {
    x * x * x
}

/// Asserts that `actual` is the same rule as `expected`, with its excitation within 10⁻¹².
fn assert_source(actual: ImpactorSource, expected: ImpactorSource) {
    let same_rule = matches!(
        (actual, expected),
        (ImpactorSource::InBelt { composition: a, .. }, ImpactorSource::InBelt { composition: b, .. })
            | (ImpactorSource::NearestBelt { composition: a, .. }, ImpactorSource::NearestBelt { composition: b, .. })
            if a == b
    ) || matches!(
        (actual, expected),
        (ImpactorSource::Comets, ImpactorSource::Comets)
            | (
                ImpactorSource::InnerComets { .. },
                ImpactorSource::InnerComets { .. }
            )
    );
    assert!(same_rule, "{actual:?} against {expected:?}");
    let (a, b) = (actual.excitation(), expected.excitation());
    assert!((a - b).abs() <= 1e-12, "{actual:?} against {expected:?}");
}

/// A velocity in km/s.
fn kms(impactors: &Impactors) -> f64 {
    impactors.velocity().value() / 1e3
}

/// P14.T48.f: a Solar-System-like system gives the ruling's Mercury 41.4, Venus 25.7, Earth 19.9,
/// Moon 16.7 (R09's `LUNAR_IMPACT_VELOCITY`), Mars 10.6 and Ceres 5.6 km/s, each within 0.1 km/s,
/// an absolute bound (science-r09-craters.md F5.5). They come out at 41.43, 25.68, 19.87, 16.68,
/// 10.55 and 5.65 km/s.
#[test]
fn a_solar_system_like_system_gives_the_ruled_velocities() {
    let sun = solar_sources();
    let cases = [
        ("Mercury", kms(&planet_impactors("Mercury", &sun)), 41.4),
        ("Venus", kms(&planet_impactors("Venus", &sun)), 25.7),
        ("Earth", kms(&planet_impactors("Earth", &sun)), 19.9),
        ("the Moon", kms(&the_moon(&sun)), 16.7),
        ("Mars", kms(&planet_impactors("Mars", &sun)), 10.6),
        ("Ceres", kms(&ceres(163.0, 0.0, &sun)), 5.6),
    ];
    for (name, v, expected) in cases {
        assert!(
            (v - expected).abs() <= 0.1,
            "{name}: {v:.4} km/s against {expected}"
        );
    }
    let moon = the_moon(&sun).velocity().value();
    assert!(
        (moon - LUNAR_IMPACT_VELOCITY.value()).abs() <= 100.0,
        "the Moon's {moon} m/s against the map's reference"
    );
}

/// P14.T48.f: Mars ÷ the Moon lies in 0.53–0.65, between Bottke's and Le Feuvre and Wieczorek's
/// 0.53–0.54 and Ivanov 2001's 0.60 (science-r09-craters.md item 4). It is 0.632.
#[test]
fn mars_over_the_moon_lies_between_the_published_ratios() {
    let sun = solar_sources();
    let ratio = kms(&planet_impactors("Mars", &sun)) / kms(&the_moon(&sun));
    assert!((0.53..=0.65).contains(&ratio), "Mars ÷ the Moon {ratio:.4}");
}

/// P14.T48.f: the 15 regular moons of Jupiter, Saturn and Uranus lie within 10% of Brasser et al.
/// 2025's Table 5. They lie within −6.3% (Dione) and +7.4% (Mimas).
#[test]
fn the_regular_moons_lie_within_a_tenth_of_brasser_et_al() {
    let sun = solar_sources();
    for (name, parent, a_km, gm, radius_km, expected) in MOONS {
        let moon = moon_impactors(
            parent,
            (a_km, gm, radius_km),
            SnowLineSide::Beyond,
            (100.0, 0.0),
            &sun,
        );
        let v = kms(&moon);
        assert!(
            (v / expected - 1.0).abs() <= 0.10,
            "{name}: {v:.3} km/s against {expected}"
        );
        assert_eq!(moon.source(), ImpactorSource::Comets, "{name}");
        assert_eq!(moon.projectile_density(), ICY_PROJECTILE_DENSITY, "{name}");
    }
}

/// P14.T48.f: Pluto lies within 20% of Singer et al. 2019's 2.2 km/s, from its own belt. It is
/// 2.48 km/s, +12.7%.
#[test]
fn pluto_lies_within_a_fifth_of_singer_et_al() {
    let pluto = pluto(&solar_sources());
    let v = kms(&pluto);
    assert!((v / 2.2 - 1.0).abs() <= 0.20, "Pluto: {v:.3} km/s");
    assert_eq!(pluto.projectile_density(), ICY_PROJECTILE_DENSITY);
    assert_eq!(pluto.crust(), Crust::IceRich);
}

/// P14.T48.f: Ganymede and Ceres are ice-rich, Hyperion porous, and the Moon and Mars silicate.
///
/// Ceres's water is McCord and Sotin 2005's lowest, 17% of its mass (J. Geophys. Res. 110,
/// E05009, doi:10.1029/2004JE002244), a layer of 58 km, at its radiative equilibrium of about
/// 163 K (278.3 K at 1 au for the IAU's nominal L☉, × (1 − 0.09)^¼ ÷ √2.77, its geometric albedo
/// standing for its Bond albedo). Mars's water is its polar layered deposits, about 1.6 × 10⁶ km³
/// in the south (Plaut et al. 2007, Science 316, 92) and 1.2 × 10⁶ km³ in the north (the lower end
/// of Zuber et al. 1998's 1.2–1.7 × 10⁶ km³, Science 282, 2053), 2.6 × 10¹⁸ kg at 920 kg m⁻³, a
/// layer of 18 m, at NASA's fact sheet's average of 214 K. The Moon's crust holds no global water.
#[test]
fn the_crusts_and_classes_follow_the_ruling() {
    let sun = solar_sources();
    let ice = |impactors: &Impactors, temperature: f64, name: &str| {
        assert_eq!(impactors.crust(), Crust::IceRich, "{name}");
        assert_same_bits(impactors.k_target(), ICE_K_TARGET);
        assert_eq!(impactors.target().class, TargetClass::NonPorous, "{name}");
        assert_same_bits(
            impactors.target().density.value(),
            substance::ice_ih_density(Kelvin::new(temperature)).value(),
        );
    };
    let silicate = |impactors: &Impactors, name: &str| {
        assert_eq!(impactors.crust(), Crust::Silicate, "{name}");
        assert_same_bits(impactors.k_target(), SILICATE_K_TARGET);
        assert_eq!(impactors.target(), craters::LUNAR_TARGET, "{name}");
    };
    let ganymede = MOONS.iter().find(|m| m.0 == "Ganymede").unwrap();
    let ganymede = moon_impactors(
        "Jupiter",
        (ganymede.2, ganymede.3, ganymede.4),
        SnowLineSide::Beyond,
        (110.0, 0.0),
        &sun,
    );
    ice(&ganymede, 110.0, "Ganymede");
    let ceres_water = 0.17 * CERES.3 * 1e24;
    let depth = ice_layer_depth(Kilograms::new(ceres_water), Metres::new(CERES.4 * 1e3));
    assert!((depth.value() / 57.5e3 - 1.0).abs() < 0.01, "{depth:?}");
    ice(&ceres(163.0, ceres_water, &sun), 163.0, "Ceres");
    silicate(&ceres(163.0, 0.0, &sun), "a dry Ceres");

    let (a_km, gm, radius_km, density) = HYPERION;
    let hyperion = moon_impactors(
        "Saturn",
        (a_km, gm, radius_km),
        SnowLineSide::Beyond,
        (93.0, 0.0),
        &sun,
    );
    assert_eq!(hyperion.target().class, TargetClass::Porous);
    assert!((hyperion.target().density.value() / density - 1.0).abs() < 1e-3);
    assert_eq!(hyperion.crust(), Crust::IceRich);

    silicate(&the_moon(&sun), "the Moon");
    let &(.., kg, radius) = PLANETS.iter().find(|p| p.0 == "Mars").unwrap();
    let mars_water = (1.6e6 + 1.2e6) * 1e9 * 920.0;
    let mars = impactors(&body(
        ImpactorOrbit::Host(planet("Mars")),
        kg * 1e24,
        radius,
        214.0,
        mars_water,
        &sun,
    ))
    .unwrap();
    silicate(&mars, "Mars");
}

/// A cold body is ice-rich from a water layer of 10 km, a warm one never by its water, and a moon
/// formed beyond its parent's ice line or an icy belt's member whenever it lies under 273 K
/// (science-r09-craters.md F5.4).
#[test]
fn the_ice_rich_test_reads_the_temperature_the_water_and_the_birthplace() {
    let sun = solar_sources();
    let (.., mass, radius) = CERES;
    let r = Metres::new(radius * 1e3);
    let layer_of = |depth: f64| {
        let area = 4.0 * core::f64::consts::PI * r.value() * r.value();
        depth * area * WATER_LAYER_DENSITY.value()
    };
    let crust_at = |temperature: f64, depth: f64| ceres(temperature, layer_of(depth), &sun).crust();
    assert_eq!(crust_at(272.9, 10.0e3), Crust::IceRich);
    assert_eq!(crust_at(272.9, 9.99e3), Crust::Silicate);
    assert_eq!(crust_at(273.0, 50.0e3), Crust::Silicate);
    let io = MOONS[0];
    let inside = moon_impactors(
        "Jupiter",
        (io.2, io.3, io.4),
        SnowLineSide::Inside,
        (110.0, 0.0),
        &sun,
    );
    assert_eq!(inside.crust(), Crust::Silicate);
    let warm_member = ImpactorOrbit::Host(HostOrbiter {
        belt: Some(BeltMembership {
            part: BeltPart::Scattered,
            composition: BeltComposition::Icy,
        }),
        ..planet("Mars")
    });
    let member_at = |temperature: f64| {
        impactors(&body(
            warm_member,
            mass * 1e24,
            radius,
            temperature,
            0.0,
            &sun,
        ))
        .unwrap()
        .crust()
    };
    assert_eq!(member_at(300.0), Crust::Silicate);
    assert_eq!(member_at(272.9), Crust::IceRich);
    let ganymede = MOONS[2];
    let warm_moon = moon_impactors(
        "Jupiter",
        (ganymede.2, ganymede.3, ganymede.4),
        SnowLineSide::Beyond,
        (273.0, 0.0),
        &sun,
    );
    assert_eq!(warm_moon.crust(), Crust::Silicate);
    // `Crust::of` is the predicate `impactors` applies.
    let none = Kilograms::ZERO;
    assert_eq!(
        Crust::of(Birthplace::Icy, Kelvin::new(272.9), none, r),
        Crust::IceRich
    );
    assert_eq!(
        Crust::of(Birthplace::Other, Kelvin::new(100.0), none, r),
        Crust::Silicate
    );
}

/// The ice targets are ice Ih's at their bodies' mean surface temperatures, within 0.03 kg m⁻³
/// of science-r09-craters.md F5.3's figures: Ganymede's at 110 K 932.72, Ceres's at 163 K 929.64
/// and Pluto's at 40 K 933.78 kg m⁻³.
#[test]
fn the_ice_targets_are_ice_ih_at_the_surface_temperature() {
    let sun = solar_sources();
    let ganymede = MOONS[2];
    let ganymede = moon_impactors(
        "Jupiter",
        (ganymede.2, ganymede.3, ganymede.4),
        SnowLineSide::Beyond,
        (110.0, 0.0),
        &sun,
    );
    let ceres = ceres(163.0, 0.17 * CERES.3 * 1e24, &sun);
    let pluto = pluto(&sun);
    for (name, impactors, expected) in [
        ("Ganymede", ganymede, 932.72),
        ("Ceres", ceres, 929.64),
        ("Pluto", pluto, 933.78),
    ] {
        let density = impactors.target().density.value();
        assert!(
            (density - expected).abs() <= 0.03,
            "{name}: {density} kg m⁻³ against {expected}"
        );
    }
}

/// The sources follow the ruling's order: a member and a body in an annulus take their part's
/// excitation; a giant, a body beyond the innermost giant and their moons the comets; the inner
/// bodies the nearest asteroid belt; and the projectiles the source's density.
#[test]
fn the_sources_follow_the_ruled_order() {
    let sun = solar_sources();
    let earth = planet_impactors("Earth", &sun);
    let a_b = sun.belts()[0].main.mid_radius().value() / METRES_PER_AU;
    assert_source(
        earth.source(),
        ImpactorSource::NearestBelt {
            excitation: belt_excitation(BeltPart::Main) + cube(1.0 - PLANETS[2].1 / a_b),
            composition: BeltComposition::Rocky,
        },
    );
    assert_eq!(earth.projectile_density(), ROCKY_PROJECTILE_DENSITY);
    assert_source(the_moon(&sun).source(), earth.source());
    let in_belt = ImpactorSource::InBelt {
        excitation: belt_excitation(BeltPart::Main),
        composition: BeltComposition::Rocky,
    };
    assert_source(ceres(163.0, 0.0, &sun).source(), in_belt);
    // A body orbiting in the belt is in it, member or not.
    let free = HostOrbiter {
        belt: None,
        ..planet("Mars")
    };
    let free = HostOrbiter {
        orbit: HostOrbit {
            semi_major_axis: au(3.0),
            ..free.orbit
        },
        ..free
    };
    assert_source(ImpactorSource::of(&free, &sun), in_belt);
    // An annulus holds its edges.
    let main = sun.belts()[0].main;
    for edge in [main.inner_edge, main.outer_edge] {
        let at_edge = HostOrbiter {
            orbit: HostOrbit {
                semi_major_axis: edge,
                ..free.orbit
            },
            ..free
        };
        assert_source(ImpactorSource::of(&at_edge, &sun), in_belt);
    }
    // A body in the scattered component takes its excitation.
    let scattered = HostOrbiter {
        orbit: HostOrbit {
            semi_major_axis: au(60.0),
            ..free.orbit
        },
        ..free
    };
    assert_source(
        ImpactorSource::of(&scattered, &sun),
        ImpactorSource::InBelt {
            excitation: belt_excitation(BeltPart::Scattered),
            composition: BeltComposition::Icy,
        },
    );
    // Jupiter is a giant, and an Earth beyond it sees the comets.
    assert_source(
        planet_impactors("Jupiter", &sun).source(),
        ImpactorSource::Comets,
    );
    let beyond = HostOrbiter {
        orbit: HostOrbit {
            semi_major_axis: au(7.0),
            ..free.orbit
        },
        ..free
    };
    assert_source(ImpactorSource::of(&beyond, &sun), ImpactorSource::Comets);
    assert_eq!(
        ImpactorSource::Comets.projectile_density(),
        ICY_PROJECTILE_DENSITY
    );
    assert!((ImpactorSource::Comets.excitation() - 0.09).abs() < 1e-15);
}

/// The asteroid belt is the inner bodies' source before a nearer belt of another kind, and the
/// nearest of several by ratio, the first on a tie; with no belt the giant's comets, and with
/// neither the comets alone.
#[test]
fn rule_three_takes_the_nearest_asteroid_belt_then_any_then_the_giant() {
    let gap = SourceBelt {
        kind: BeltKind::Kuiper,
        composition: BeltComposition::Icy,
        main: annulus(1.2, 1.4),
        scattered: None,
    };
    let asteroid = SourceBelt {
        kind: BeltKind::Asteroid,
        composition: BeltComposition::Rocky,
        main: annulus(2.5, 3.5),
        scattered: None,
    };
    let sources = |belts: Vec<SourceBelt>, giant: Option<f64>| {
        let giants = giant.map(|a| (EarthMasses::new(317.8), au(a)));
        ImpactorSources::new(sun_parameter(), belts, giants).unwrap()
    };
    let earth = planet("Earth");
    let both = sources(vec![gap, asteroid], Some(5.2));
    assert_source(
        ImpactorSource::of(&earth, &both),
        ImpactorSource::NearestBelt {
            excitation: belt_excitation(BeltPart::Main) + cube(1.0 - PLANETS[2].1 / 3.0),
            composition: BeltComposition::Rocky,
        },
    );
    let kuiper_only = sources(vec![gap], Some(5.2));
    assert_source(
        ImpactorSource::of(&earth, &kuiper_only),
        ImpactorSource::NearestBelt {
            excitation: belt_excitation(BeltPart::Main) + cube(1.0 - PLANETS[2].1 / 1.3),
            composition: BeltComposition::Icy,
        },
    );
    // Two asteroid belts as near by ratio, at 0.5 and 2 au: the first given wins.
    let near = |inner: f64, outer: f64, composition| SourceBelt {
        kind: BeltKind::Asteroid,
        composition,
        main: annulus(inner, outer),
        scattered: None,
    };
    let at_one = HostOrbiter {
        orbit: orbit(1.0, 0.0, 0.0),
        ..earth
    };
    let tie = sources(
        vec![
            near(0.25, 0.75, BeltComposition::Icy),
            near(1.5, 2.5, BeltComposition::Rocky),
        ],
        None,
    );
    // The first, at 0.5 au, from outside: (1 − a_b ÷ a)³.
    assert_source(
        ImpactorSource::of(&at_one, &tie),
        ImpactorSource::NearestBelt {
            excitation: belt_excitation(BeltPart::Main) + cube(1.0 - 0.5 / 1.0),
            composition: BeltComposition::Icy,
        },
    );
    let giant_only = sources(Vec::new(), Some(5.2));
    let comets = COMET_EXCITATION * COMET_EXCITATION;
    assert_source(
        ImpactorSource::of(&at_one, &giant_only),
        ImpactorSource::InnerComets {
            excitation: comets + cube(1.0 - 1.0 / 5.2),
        },
    );
    assert_source(
        ImpactorSource::of(&at_one, &sources(Vec::new(), None)),
        ImpactorSource::InnerComets { excitation: comets },
    );
}

/// The ruling's sanity cases (science-r09-craters.md item 4, its `final_model.py`): an Earth at
/// 0.05 au, struck by near-radial crossers from the asteroid belt, at 134 km/s, and an Earth with
/// only the giants' comets at 25.9 km/s.
#[test]
fn the_ruled_sanity_cases_hold() {
    let sun = solar_sources();
    let &(.., mass, radius) = PLANETS.iter().find(|p| p.0 == "Earth").unwrap();
    let hot = HostOrbiter {
        orbit: orbit(0.05, 0.0, 0.0),
        ..planet("Earth")
    };
    let hot = impactors(&body(
        ImpactorOrbit::Host(hot),
        mass * 1e24,
        radius,
        1_000.0,
        0.0,
        &sun,
    ))
    .unwrap();
    assert!((kms(&hot) - 134.0).abs() <= 0.5, "{:.3}", kms(&hot));
    let giants_only = ImpactorSources::new(sun_parameter(), Vec::new(), solar_planets()).unwrap();
    let earth = planet_impactors("Earth", &giants_only);
    assert!((kms(&earth) - 25.9).abs() <= 0.1, "{:.3}", kms(&earth));
    assert_eq!(earth.projectile_density(), ICY_PROJECTILE_DENSITY);
}

/// The belts' excitations are 2σₑ² + 2 sin² σᵢ at their Rayleigh widths: 0.0587 for a belt proper
/// (σ 0.1, 8°) and 0.414 for a scattered component (σ 0.3, 20°).
#[test]
fn the_belts_excitations_follow_their_widths() {
    assert!((belt_excitation(BeltPart::Main) - 0.058_738).abs() < 1e-6);
    assert!((belt_excitation(BeltPart::Scattered) - 0.413_956).abs() < 1e-6);
}

/// The innermost giant is the innermost planet of at least 10 M⊕, and none without one.
#[test]
fn the_innermost_giant_is_the_innermost_of_ten_earth_masses() {
    let planets = [(30.0, 9.5), (9.99, 1.0), (10.0, 5.2), (317.8, 20.0)];
    let planets = planets.map(|(m, a)| (EarthMasses::new(m), au(a)));
    let sources = ImpactorSources::new(sun_parameter(), Vec::new(), planets).unwrap();
    assert_eq!(sources.innermost_giant(), Some(au(5.2)));
    let earth = [(EarthMasses::new(1.0), au(1.0))];
    let none = ImpactorSources::new(sun_parameter(), Vec::new(), earth).unwrap();
    assert_eq!(none.innermost_giant(), None);
}

/// The generator's own belts about a Solar-System-like host carry over: kinds, compositions and
/// edges, and Earth's and Mars's impact velocities under them within 0.1 km/s of the ruling's.
#[test]
fn the_generators_belts_carry_over() {
    let disc = solar_disc();
    let neighbours: Vec<Neighbour> = PLANETS
        .iter()
        .map(|&(_, a, e, _, mass, _)| Neighbour::new(earths(mass * 1e24), au(a), e))
        .chain([Neighbour::new(
            EarthMasses::new(17.15),
            au(NEPTUNE_AU),
            0.009,
        )])
        .collect();
    let plane = SystemPlane::new(Radians::ZERO, Radians::ZERO).unwrap();
    let host = BeltHost::new(OrbitHost::Star(0), &disc, &neighbours, plane);
    let system = SystemId::from_raw(0x0200_0800_2000_0000).unwrap();
    let belts = host_belts(Seed::new(0x48f), system, &host, FIRST_BELT_SLOT);
    let generated: Vec<SourceBelt> = belts.belts().iter().map(SourceBelt::from).collect();
    assert!(!generated.is_empty());
    for (belt, source) in belts.belts().iter().zip(&generated) {
        assert_eq!(source.kind, belt.kind());
        assert_eq!(source.composition, belt.composition());
        assert_eq!(source.main.inner_edge, belt.main().inner_edge());
        assert_eq!(source.main.outer_edge, belt.main().outer_edge());
        // The mid-radius rule 3 reads is the belts' own, to the bit.
        assert_same_bits(
            source.main.mid_radius().value(),
            belt.main().radius().value(),
        );
        assert_eq!(
            source.scattered.map(|s| (s.inner_edge, s.outer_edge)),
            belt.scattered().map(|s| (s.inner_edge(), s.outer_edge()))
        );
    }
    let sources = ImpactorSources::new(sun_parameter(), generated, solar_planets()).unwrap();
    for (name, expected) in [("Earth", 19.9), ("Mars", 10.6)] {
        let v = kms(&planet_impactors(name, &sources));
        assert!((v - expected).abs() <= 0.1, "{name}: {v:.4} km/s");
    }
}

/// Earth, with its oceans' 1.4 × 10²¹ kg, under `sources`.
fn earth_body(sources: &ImpactorSources) -> ImpactorBody<'_> {
    let &(.., mass, radius) = PLANETS.iter().find(|p| p.0 == "Earth").unwrap();
    body(
        ImpactorOrbit::Host(planet("Earth")),
        mass * 1e24,
        radius,
        288.0,
        1.4e21,
        sources,
    )
}

/// Every part of a body out of its field's range is refused, naming it.
#[test]
fn a_body_s_parts_out_of_range_are_refused() {
    let sun = solar_sources();
    let earth = earth_body(&sun);
    assert!(impactors(&earth).is_ok());
    let with_orbit = |change: fn(&mut HostOrbiter)| {
        let mut orbiter = planet("Earth");
        change(&mut orbiter);
        impactors(&ImpactorBody {
            orbit: ImpactorOrbit::Host(orbiter),
            ..earth
        })
    };
    assert_eq!(
        with_orbit(|o| o.orbit.semi_major_axis = Metres::ZERO),
        Err(DeriveImpactorsError::SemiMajorAxis(Metres::ZERO))
    );
    assert_eq!(
        with_orbit(|o| o.orbit.eccentricity = 1.0),
        Err(DeriveImpactorsError::Eccentricity(1.0))
    );
    assert!(matches!(
        with_orbit(|o| o.orbit.inclination = Radians::new(f64::NAN)),
        Err(DeriveImpactorsError::Inclination(_))
    ));
    assert_eq!(
        with_orbit(|o| o.mass = EarthMasses::new(-1.0)),
        Err(DeriveImpactorsError::Mass(EarthMasses::new(-1.0)))
    );
    let moon = ImpactorOrbit::Moon {
        parent: HostOrbiter {
            mass: EarthMasses::ZERO,
            ..planet("Earth")
        },
        semi_major_axis: Metres::new(3.844e8),
        formed: SnowLineSide::Inside,
    };
    assert_eq!(
        impactors(&ImpactorBody {
            orbit: moon,
            ..earth
        }),
        Err(DeriveImpactorsError::Mass(EarthMasses::ZERO))
    );
    let moon = ImpactorOrbit::Moon {
        parent: planet("Earth"),
        semi_major_axis: Metres::new(-1.0),
        formed: SnowLineSide::Inside,
    };
    assert_eq!(
        impactors(&ImpactorBody {
            orbit: moon,
            ..earth
        }),
        Err(DeriveImpactorsError::MoonSemiMajorAxis(Metres::new(-1.0)))
    );
    let refused = [
        (
            ImpactorBody {
                radius: Metres::ZERO,
                ..earth
            },
            DeriveImpactorsError::Radius(Metres::ZERO),
        ),
        (
            ImpactorBody {
                density: KilogramsPerCubicMetre::new(-1.0),
                ..earth
            },
            DeriveImpactorsError::Density(KilogramsPerCubicMetre::new(-1.0)),
        ),
        (
            ImpactorBody {
                surface_gravity: MetresPerSecondSquared::ZERO,
                ..earth
            },
            DeriveImpactorsError::SurfaceGravity(MetresPerSecondSquared::ZERO),
        ),
        (
            ImpactorBody {
                surface_temperature: Kelvin::new(-1.0),
                ..earth
            },
            DeriveImpactorsError::SurfaceTemperature(Kelvin::new(-1.0)),
        ),
        (
            ImpactorBody {
                water: Kilograms::new(-1.0),
                ..earth
            },
            DeriveImpactorsError::Water(Kilograms::new(-1.0)),
        ),
    ];
    for (body, error) in refused {
        assert_eq!(impactors(&body), Err(error));
    }
}

/// Every part of the sources out of its range is refused, naming it.
#[test]
fn the_sources_parts_out_of_range_are_refused() {
    let none = GravitationalParameter::new(0.0);
    assert_eq!(
        ImpactorSources::new(none, solar_belts(), solar_planets()),
        Err(BuildImpactorSourcesError::HostParameter(none))
    );
    let mut reversed = solar_belts();
    reversed[0].main = annulus(3.0, 2.0);
    assert_eq!(
        ImpactorSources::new(sun_parameter(), reversed, solar_planets()),
        Err(BuildImpactorSourcesError::BeltEdges(au(3.0), au(2.0)))
    );
    let mut negative = solar_belts();
    negative[1].scattered = Some(annulus(-1.0, 50.0));
    assert_eq!(
        ImpactorSources::new(sun_parameter(), negative, solar_planets()),
        Err(BuildImpactorSourcesError::BeltEdges(au(-1.0), au(50.0)))
    );
    let giant = EarthMasses::new(317.8);
    assert_eq!(
        ImpactorSources::new(sun_parameter(), solar_belts(), [(giant, Metres::ZERO)]),
        Err(BuildImpactorSourcesError::Planet(giant, Metres::ZERO))
    );
    let not_a_mass = [(EarthMasses::new(f64::NAN), au(5.2))];
    assert!(matches!(
        ImpactorSources::new(sun_parameter(), solar_belts(), not_a_mass),
        Err(BuildImpactorSourcesError::Planet(..))
    ));
}

/// The name of `source`'s rule.
fn rule(source: ImpactorSource) -> &'static str {
    match source {
        ImpactorSource::InBelt { .. } => "in_belt",
        ImpactorSource::Comets => "comets",
        ImpactorSource::NearestBelt { .. } => "nearest_belt",
        ImpactorSource::InnerComets { .. } => "inner_comets",
    }
}

/// The Solar System's impactors, pinned to the bit: every planet's, the Moon's, Ceres's and
/// Pluto's, the 15 regular moons' and Hyperion's. Nothing on the record reads them yet, so the
/// 21 → 22 bump moves only the header, unless the batch changes the model.
#[test]
fn the_solar_system_s_impactors_are_pinned() {
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;

    let sun = solar_sources();
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    let pin = |w: &mut GoldenWriter, name: &str, i: &Impactors| {
        w.line(&format!("{name}_rule = {}", rule(i.source())));
        w.f64(&format!("{name}_excitation"), i.source().excitation());
        w.f64(&format!("{name}_velocity"), i.velocity().value());
        w.f64(
            &format!("{name}_projectile_density"),
            i.projectile_density().value(),
        );
        w.line(&format!("{name}_target_class = {:?}", i.target().class));
        w.f64(
            &format!("{name}_target_density"),
            i.target().density.value(),
        );
        w.f64(&format!("{name}_k_target"), i.k_target());
        w.line(&format!("{name}_crust = {:?}", i.crust()));
    };
    for (name, ..) in PLANETS {
        pin(&mut w, &name.to_lowercase(), &planet_impactors(name, &sun));
    }
    pin(&mut w, "moon", &the_moon(&sun));
    pin(&mut w, "ceres", &ceres(163.0, 0.17 * CERES.3 * 1e24, &sun));
    pin(&mut w, "pluto", &pluto(&sun));
    for (name, parent, a_km, gm, radius_km, _) in MOONS {
        let moon = moon_impactors(
            parent,
            (a_km, gm, radius_km),
            SnowLineSide::Beyond,
            (100.0, 0.0),
            &sun,
        );
        pin(&mut w, &name.to_lowercase(), &moon);
    }
    let (a_km, gm, radius_km, _) = HYPERION;
    let hyperion = moon_impactors(
        "Saturn",
        (a_km, gm, radius_km),
        SnowLineSide::Beyond,
        (93.0, 0.0),
        &sun,
    );
    pin(&mut w, "hyperion", &hyperion);
    golden!("planetary/impactors", w.as_str());
}

// P14.T24.b: the global figures' interior on the Solar System table (`derive::solar`), under
// P14.T24.f's rules, the batch's, with science-p14-t24b-laws Part 1's checks.

/// The table's bodies' sidereal rotation periods, days (NASA's fact sheets; Mercury's is the
/// IAU's), as the surface conditions' tests take them.
const SIDEREAL_DAYS: [(&str, f64); 8] = [
    ("Mercury", 58.646),
    ("Venus", 243.025),
    ("Earth", 0.997_27),
    ("Mars", 1.025_96),
    ("Moon", 27.321_7),
    ("Ganymede", 7.154_6),
    ("Titan", 15.945),
    ("Ceres", 9.074_17 / 24.0),
];

fn sidereal_period(name: &str) -> Seconds {
    let &(_, days) = SIDEREAL_DAYS.iter().find(|(n, _)| *n == name).unwrap();
    Seconds::new(days * SECONDS_PER_DAY)
}

/// The table's bodies with their figures about the present Sun, each turning in its own day.
fn figures_table() -> Vec<(&'static str, DerivedBody, GlobalFigures)> {
    crate::planetary::derive::solar::table_under(
        crate::planetary::derive::atmosphere::Speciation::CarbonAndOxygen,
    )
    .into_iter()
    .map(|(name, body)| {
        let figures = table_figures_spun(&body, sidereal_period(name))
            .expect("the table's bodies have surfaces");
        (name, body, figures)
    })
    .collect()
}

fn figures_of<'a>(
    table: &'a [(&'static str, DerivedBody, GlobalFigures)],
    name: &str,
) -> &'a GlobalFigures {
    &table.iter().find(|(n, ..)| *n == name).unwrap().2
}

fn body_of<'a>(
    table: &'a [(&'static str, DerivedBody, GlobalFigures)],
    name: &str,
) -> &'a DerivedBody {
    &table.iter().find(|(n, ..)| *n == name).unwrap().1
}

/// The figures' inputs of the derived body `body` about the present Sun, turning in `name`'s day.
fn inputs_of<'a>(body: &'a DerivedBody, name: &str) -> FiguresInputs<'a> {
    FiguresInputs {
        atmosphere: body.atmosphere(),
        mass: body.mass(),
        radius: Metres::from(body.radius()),
        surface_gravity: body.surface_gravity(),
        class: body.class(),
        fractions: body.fractions(),
        inventory_water: body.inventory().water(),
        rotation_period: sidereal_period(name),
        fe_h: Dex::new(0.0),
        age: SOLAR_AGE,
        tidal_heat_flow: WattsPerSquareMetre::new(0.0),
        subsurface_ocean: Subsurface::NotComputed,
    }
}

/// The Moon as the generator makes it, a giant-impact moon of a rocky planet: its measured mass
/// and radius (NASA's fact sheet), its bulk rock alone (`moon_bulk_of`), under the table's airless
/// sky, locked to its planet's orbit of 27.32 d.
fn the_giant_impact_moon() -> GlobalFigures {
    let table = figures_table();
    let moon = body_of(&table, "Moon");
    let r = 1.737_4e6;
    let mass = EarthMasses::new(7.342e22 / EARTH_MASS_KG);
    GlobalFigures::derive(&FiguresInputs {
        mass,
        radius: Metres::new(r),
        surface_gravity: MetresPerSecondSquared::new(GRAVITATIONAL_CONSTANT * 7.342e22 / (r * r)),
        class: PlanetClass::Rocky,
        fractions: MassFractions::solid(0.0, 1.0, 0.0),
        ..inputs_of(moon, "Moon")
    })
    .unwrap()
}

#[test]
fn the_table_s_heat_flows_are_their_calibrations() {
    let table = figures_table();
    for (name, body, figures) in &table {
        eprintln!(
            "{name}: {:?}, iron {:.3}, rock {:.3}, F {:.2} mW m⁻² (radiogenic {:.2}), {:?}, {:?}, \
             {:?}, O {:.3}",
            body.class(),
            body.fractions().iron(),
            body.fractions().rock(),
            figures.heat_flow().value() * 1e3,
            figures.radiogenic_heat_flow().value() * 1e3,
            figures.interior(),
            figures.tectonic_regime(),
            figures.magnetic_field(),
            figures.outgassing()
        );
    }
    // Earth: within a factor 1.5 of Davies and Davies 2010's 0.092 W m⁻², and within 3% (its
    // mobile lid's calibration); Mars: 22 mW m⁻² within 10% (its stagnant lid's); Mercury in
    // 9–15 mW m⁻², its core's heat (8.0 without it); the Moon, as the generator makes it, in
    // 9–21 mW m⁻² (Siegler and Smrekar 2014's mantle 9–13; science-p14-t24b-laws §1.4).
    let earth = figures_of(&table, "Earth").heat_flow().value();
    assert!((earth / 0.092 - 1.0).abs() < 0.03, "{earth}");
    assert!(earth > 0.092 / 1.5 && earth < 0.092 * 1.5);
    let mars = figures_of(&table, "Mars").heat_flow().value();
    assert!((mars / 0.022 - 1.0).abs() < 0.10, "{mars}");
    let mercury = figures_of(&table, "Mercury").heat_flow().value();
    assert!((0.009..0.015).contains(&mercury), "{mercury}");
    let moon = the_giant_impact_moon().heat_flow().value();
    eprintln!("the giant-impact Moon: {:.2} mW m⁻²", moon * 1e3);
    assert!((0.009..0.021).contains(&moon), "{moon}");
    // A body round a star has no tide, so its whole heat flow is its non-tidal part; the table's
    // Ganymede and Titan conduct, and the rest convect.
    for (name, _, figures) in &table {
        assert_same_bits(
            figures.heat_flow().value(),
            figures.non_tidal_heat_flow().value(),
        );
        assert_eq!(
            figures.subsurface_ocean(),
            &Subsurface::NotComputed,
            "{name}"
        );
        let interior = if matches!(*name, "Ganymede" | "Titan") {
            Interior::Conducting
        } else {
            Interior::Convecting
        };
        assert_eq!(figures.interior(), interior, "{name}");
    }
}

#[test]
fn the_table_s_regimes_and_fields_are_the_ruling_s() {
    let table = figures_table();
    for (name, regime, field) in [
        ("Earth", TectonicRegime::MobileLid, MagneticField::Strong),
        ("Venus", TectonicRegime::Episodic, MagneticField::None),
        ("Mars", TectonicRegime::StagnantLid, MagneticField::None),
        ("Mercury", TectonicRegime::StagnantLid, MagneticField::Weak),
        ("Moon", TectonicRegime::StagnantLid, MagneticField::None),
        ("Ganymede", TectonicRegime::StagnantLid, MagneticField::None),
        ("Titan", TectonicRegime::StagnantLid, MagneticField::None),
        ("Ceres", TectonicRegime::StagnantLid, MagneticField::None),
    ] {
        let figures = figures_of(&table, name);
        assert_eq!(figures.tectonic_regime(), regime, "{name}");
        assert_eq!(figures.magnetic_field(), field, "{name}");
    }
    let moon = the_giant_impact_moon();
    assert_eq!(moon.tectonic_regime(), TectonicRegime::StagnantLid);
    assert_eq!(moon.magnetic_field(), MagneticField::None);
    // Before 0.6 Gyr Mars's dynamo is strong and Venus's weak, its day being long.
    for (name, field) in [
        ("Mars", MagneticField::Strong),
        ("Venus", MagneticField::Weak),
    ] {
        let young = GlobalFigures::derive(&FiguresInputs {
            age: Years::new(0.5e9),
            ..inputs_of(body_of(&table, name), name)
        })
        .unwrap();
        assert_eq!(young.magnetic_field(), field, "{name}");
    }
    // An Earth twin locked in a 10-day orbit keeps a multipolar field.
    let locked = GlobalFigures::derive(&FiguresInputs {
        rotation_period: Seconds::new(10.0 * SECONDS_PER_DAY),
        ..inputs_of(body_of(&table, "Earth"), "Earth")
    })
    .unwrap();
    assert_eq!(locked.magnetic_field(), MagneticField::Weak);
}

#[test]
fn the_regime_reads_nothing_of_the_residual() {
    // Each body's regime is the rule's on inputs that hold no heat flow but the radiogenic term
    // before its residual, so that no Urey ratio moves it (science-p14-t24b-laws §1.1).
    let table = figures_table();
    for (name, body, figures) in &table {
        let inputs = inputs_of(body, name);
        let kg = Kilograms::from(body.mass()).value();
        let r = Metres::from(body.radius()).value();
        let water = inputs
            .inventory_water
            .value()
            .max(body.fractions().water() * kg);
        let rock = rock_core_radius(kg * (1.0 - body.fractions().water()));
        let regime = tectonic_regime(&interior::RegimeInputs {
            interior: Interior::of(body.class(), rock).unwrap(),
            mass: body.mass(),
            radiogenic: figures.radiogenic_heat_flow(),
            tidal: WattsPerSquareMetre::new(0.0),
            state: body.atmosphere().state(),
            water_depth: Metres::new(water / (1_000.0 * 4.0 * core::f64::consts::PI * r * r)),
            surface_gravity: body.surface_gravity(),
            ocean_shell: None,
        });
        assert_eq!(regime, figures.tectonic_regime(), "{name}");
    }
}

#[test]
fn an_earth_twin_s_plates_end_at_ten_gyr() {
    let table = figures_table();
    let earth = body_of(&table, "Earth");
    let at = |age: f64| {
        GlobalFigures::derive(&FiguresInputs {
            age: Years::new(age),
            ..inputs_of(earth, "Earth")
        })
        .unwrap()
    };
    let end = (0..=1_330)
        .map(|step| 0.5e9 + 1e7 * f64::from(step))
        .find(|&age| at(age).tectonic_regime() != TectonicRegime::MobileLid)
        .expect("the plates end");
    eprintln!("an Earth twin's plates end at {:.2} Gyr", end / 1e9);
    assert!((end / 1e9 - 10.0).abs() < 0.5, "{end}");
    assert_eq!(at(end).tectonic_regime(), TectonicRegime::StagnantLid);
}

#[test]
fn a_water_world_s_rock_convects_under_its_high_pressure_ice() {
    let table = figures_table();
    let earth = body_of(&table, "Earth");
    let fractions = MassFractions::solid(0.15, 0.35, 0.5);
    let world = GlobalFigures::derive(&FiguresInputs {
        class: PlanetClass::Icy,
        fractions,
        radius: Metres::new(1.25 * 6.371e6),
        ..inputs_of(earth, "Earth")
    })
    .unwrap();
    assert_eq!(world.interior(), Interior::Convecting);
    assert_eq!(world.tectonic_regime(), TectonicRegime::StagnantLid);
    let x = 0.15 / 0.5;
    let form = 1.0
        + (1.0 / interior::STAGNANT_LID_UREY_RATIO - 1.0)
            * (1.0 + interior::CORE_TO_MANTLE_HEAT_CAPACITY * x / (1.0 - x));
    let ratio = world.heat_flow().value() / world.radiogenic_heat_flow().value();
    assert!((ratio / form - 1.0).abs() < 1e-12, "{ratio} against {form}");
}

#[test]
fn the_generated_earth_s_heat_flow_is_the_outgassing_s_unit() {
    // `GENERATED_EARTH_HEAT_FLOW` is the table's Earth's, refitted here.
    let table = figures_table();
    let earth = figures_of(&table, "Earth").heat_flow().value();
    eprintln!("the table's Earth: {earth:.12} W m⁻²");
    assert!(
        (earth / interior::GENERATED_EARTH_HEAT_FLOW.value() - 1.0).abs() < 1e-9,
        "{earth}"
    );
}

#[test]
fn the_present_outgassing_is_earth_s_and_none_on_the_moon_mercury_and_mars() {
    let table = figures_table();
    assert!((figures_of(&table, "Earth").outgassing() - 1.0).abs() < 1e-8);
    for name in ["Moon", "Mercury", "Mars"] {
        assert_same_bits(figures_of(&table, name).outgassing(), 0.0);
    }
    assert_same_bits(the_giant_impact_moon().outgassing(), 0.0);
    let venus = figures_of(&table, "Venus").outgassing();
    eprintln!("Venus's outgassing: {venus}");
    assert!((0.3..3.0).contains(&venus), "{venus}");
}

#[test]
fn the_heat_flow_is_continuous_in_time_but_at_a_regime_change() {
    let table = figures_table();
    let earth = body_of(&table, "Earth");
    let at = |age: f64, fe_h: f64| {
        GlobalFigures::derive(&FiguresInputs {
            age: Years::new(age),
            fe_h: Dex::new(fe_h),
            ..inputs_of(earth, "Earth")
        })
        .unwrap()
    };
    // From 0.5 to 13.8 Gyr in steps of 10 Myr: the heat flow and the outgassing fall smoothly, and
    // step once, where the falling production stagnates the lid and the mobile lid's Urey ratio
    // gives way to the stagnant one's.
    let fractions = earth.fractions();
    let x = fractions.iron() / (fractions.iron() + fractions.rock());
    let unit = WattsPerSquareMetre::new(1.0);
    let step_ratio = convecting_heat_flow(unit, x, TectonicRegime::StagnantLid).value()
        / convecting_heat_flow(unit, x, TectonicRegime::MobileLid).value();
    let mut last = at(0.5e9, 0.0);
    let mut changes = 0;
    for step in 1..=1_330 {
        let age = 0.5e9 + 1e7 * f64::from(step);
        let now = at(age, 0.0);
        let ratio = now.heat_flow().value() / last.heat_flow().value();
        assert!(now.outgassing() <= last.outgassing(), "{age}");
        assert!(ratio < 1.0, "{age}: {ratio}");
        if now.tectonic_regime() == last.tectonic_regime() {
            assert!(ratio > 0.99, "{age}: {ratio}");
        } else {
            changes += 1;
            eprintln!(
                "Earth's lid stagnates at {:.2} Gyr, its heat flow stepping by {ratio:.4}",
                age / 1e9
            );
            assert_eq!(now.tectonic_regime(), TectonicRegime::StagnantLid);
            assert!((ratio / step_ratio - 1.0).abs() < 0.005, "{ratio}");
        }
        last = now;
    }
    assert_eq!(changes, 1);
    // At fixed composition and age it scales as 10^[Fe/H], at a fixed regime.
    let base = at(SOLAR_AGE.value(), 0.0);
    for fe_h in [-0.2, 0.1, 0.3] {
        let scaled = at(SOLAR_AGE.value(), fe_h);
        assert_eq!(scaled.tectonic_regime(), TectonicRegime::MobileLid);
        let ratio = scaled.heat_flow().value() / base.heat_flow().value();
        assert!(
            (ratio / math::exp10(fe_h) - 1.0).abs() < 1e-12,
            "{fe_h}: {ratio}"
        );
    }
}

#[test]
fn an_icy_body_s_heat_flow_carries_its_rock_s_lag() {
    let table = figures_table();
    let ganymede = body_of(&table, "Ganymede");
    let figures = figures_of(&table, "Ganymede");
    assert_eq!(ganymede.class(), PlanetClass::Icy);
    let mass = Kilograms::from(ganymede.mass()).value();
    let core = rock_core_radius(mass * (1.0 - ganymede.fractions().water()));
    let lag = conductive_lag(core, SOLAR_AGE, interior::ROCK_THERMAL_DIFFUSIVITY_M2_PER_S);
    eprintln!(
        "Ganymede: rock core {:.0} km, lag {lag:.4}",
        core.value() / 1e3
    );
    assert_eq!(figures.interior(), Interior::Conducting);
    assert!(
        (figures.heat_flow().value() / figures.radiogenic_heat_flow().value() - lag).abs() < 1e-12
    );
    assert_eq!(figures.tectonic_regime(), TectonicRegime::StagnantLid);
}

#[test]
fn a_moon_s_tide_adds_to_its_heat_flow() {
    let table = figures_table();
    let moon = body_of(&table, "Moon");
    let figures = figures_of(&table, "Moon");
    let heated = |tide: f64| {
        GlobalFigures::derive(&FiguresInputs {
            tidal_heat_flow: WattsPerSquareMetre::new(tide),
            ..inputs_of(moon, "Moon")
        })
        .unwrap()
    };
    let warm = heated(0.1);
    assert_same_bits(
        warm.non_tidal_heat_flow().value(),
        figures.non_tidal_heat_flow().value(),
    );
    assert_same_bits(
        warm.heat_flow().value(),
        figures.non_tidal_heat_flow().value() + 0.1,
    );
    assert!(warm.outgassing() > 0.0);
    // Below the heat pipe's edge its regime is its own; at it the tide's heat leaves through
    // eruptions, and a mantle heated by its tides keeps no dynamo.
    assert_eq!(warm.tectonic_regime(), figures.tectonic_regime());
    let io_like = heated(0.25);
    assert_eq!(io_like.tectonic_regime(), TectonicRegime::HeatPipe);
    assert_eq!(io_like.magnetic_field(), MagneticField::None);
    // A heat pipe shares the stagnant lid's ratio, so its residual is continuous at the edge.
    assert_same_bits(
        io_like.non_tidal_heat_flow().value(),
        figures.non_tidal_heat_flow().value(),
    );
}

#[test]
fn a_gas_envelope_has_no_figures_and_a_strong_field() {
    use crate::planetary::derive::solar::{PLANETS, derive, placed, solar_disc};
    let &(_, kg, _, a, e) = PLANETS.iter().find(|(n, ..)| *n == "Jupiter").unwrap();
    let jupiter = derive(&placed(kg, a, e, 0.5), &solar_disc()).unwrap();
    assert_eq!(table_figures(&jupiter), None);
    for class in [
        PlanetClass::SubNeptune,
        PlanetClass::IceGiant,
        PlanetClass::GasGiant,
    ] {
        let field = magnetic_field(&interior::FieldInputs {
            class,
            interior: Interior::Conducting,
            mass: EarthMasses::new(300.0),
            iron_share: 0.0,
            regime: TectonicRegime::StagnantLid,
            radiogenic: WattsPerSquareMetre::new(0.0),
            tidal: WattsPerSquareMetre::new(0.0),
            age: SOLAR_AGE,
            rotation_period: Seconds::new(0.41 * SECONDS_PER_DAY),
        });
        assert_eq!(field, MagneticField::Strong, "{class:?}");
    }
}

/// A moon at its measured mass and mean elements about the table's Jupiter, derived as P14.T17's
/// regular moons are: mass (kg), semi-major axis (km) and eccentricity (JPL's satellite mean
/// elements and physical parameters, read 2026-10-12 for P14.T48.f).
fn jovian(kg: f64, a_km: f64, e: f64) -> DerivedMoon {
    use crate::orbit::{Eccentricity, KeplerElements, Orientation};
    use crate::planetary::derive::BodyHosts;
    use crate::planetary::derive::solar::{self, PLANETS};
    use crate::planetary::moons::regular::{MoonNursery, MoonSky, derive_moon};
    use crate::planetary::moons::testing::planet_id;
    use crate::planetary::moons::{MoonParent, ParentKind};
    use crate::stellar::Composition;
    use crate::stellar::draws::UnitUniform;
    use crate::time::UniverseTime;
    use crate::units::consts::SOLAR_MASS_KG;

    let planets = solar::solar_system();
    let jupiter = *solar::found(&planets, "Jupiter");
    let &(_, _, _, a, e_j) = PLANETS.iter().find(|p| p.0 == "Jupiter").unwrap();
    let orbit = solar::orbit(a, e_j);
    let host = Kilograms::new(SOLAR_MASS_KG);
    let parent =
        MoonParent::from_derived(planet_id(0, 5), ParentKind::Planet, &jupiter, orbit, host)
            .unwrap();
    let lights = [solar::sun()];
    let hosts = BodyHosts::new(host, Composition::SOLAR, &lights, &[]).unwrap();
    let nursery = MoonNursery::new(
        &parent,
        &Composition::SOLAR,
        &solar_disc(),
        orbit.semi_major_axis(),
    )
    .unwrap();
    let sky = MoonSky::new(&jupiter, &orbit, &hosts).unwrap();
    eprintln!(
        "Jupiter's ice line: {:?} km",
        nursery.ice_line().map(|line| line.value() / 1e3)
    );
    let flat = Orientation::new(Radians::ZERO, Radians::ZERO, Radians::ZERO).unwrap();
    let moon_orbit = KeplerElements::from_semi_major_axis(
        Metres::new(a_km * 1e3),
        GravitationalParameter::from_earth_masses(parent.mass()),
        Eccentricity::new(e).unwrap(),
        flat,
        Radians::ZERO,
    )
    .unwrap();
    derive_moon(
        EarthMasses::new(kg / EARTH_MASS_KG),
        &moon_orbit,
        &parent,
        &nursery,
        &sky,
        UnitUniform::HALF,
        SOLAR_AGE,
        UniverseTime::EPOCH,
    )
    .unwrap()
}

/// Europa at its measured mass and elements: 4.7998 × 10²² kg at 671,034 km, e = 0.0094.
fn europa() -> DerivedMoon {
    jovian(4.7998e22, 671_034.0, 0.0094)
}

/// The figures of the derived regular moon `moon`, its tide and ocean its derivation's, locked to
/// its orbit of `days`.
fn moon_figures(moon: &DerivedMoon, days: f64) -> GlobalFigures {
    GlobalFigures::derive(&FiguresInputs {
        atmosphere: moon.atmosphere(),
        mass: EarthMasses::new(
            moon.density().value() * 4.0 / 3.0
                * core::f64::consts::PI
                * math::powi(moon.radius().value(), 3)
                / EARTH_MASS_KG,
        ),
        radius: moon.radius(),
        surface_gravity: MetresPerSecondSquared::new(
            GRAVITATIONAL_CONSTANT * moon.density().value() * 4.0 / 3.0
                * core::f64::consts::PI
                * moon.radius().value(),
        ),
        class: moon.class(),
        fractions: moon.fractions(),
        inventory_water: moon.inventory().water(),
        rotation_period: Seconds::new(days * SECONDS_PER_DAY),
        fe_h: Dex::new(0.0),
        age: SOLAR_AGE,
        tidal_heat_flow: moon.heat_flux(),
        subsurface_ocean: Subsurface::of_regular_moon(
            moon.radius(),
            moon.density(),
            moon.ice_fraction(),
            moon.equilibrium_temperature(),
            moon.heat_flux(),
        ),
    })
    .unwrap()
}

#[test]
fn io_at_its_measured_elements_is_a_heat_pipe_with_no_field() {
    // 8.931 9 × 10²² kg at 421,700 km, e = 0.0041 (JPL), locked in 1.769 d.
    let io = jovian(8.931_9e22, 421_700.0, 0.0041);
    let figures = moon_figures(&io, 1.769);
    eprintln!(
        "Io: tide {:.3} W m⁻², {:?}, {:?}, {:?}",
        io.heat_flux().value(),
        figures.interior(),
        figures.tectonic_regime(),
        figures.magnetic_field()
    );
    assert_eq!(figures.tectonic_regime(), TectonicRegime::HeatPipe);
    assert_eq!(figures.magnetic_field(), MagneticField::None);
}

/// Europa's measured bulk: radius 1,560.8 km and density 3,013 kg m⁻³ (NASA's Jovian satellite
/// fact sheet), 8% water by mass (the water layer some 120–150 km deep, as science-p14-subsurface-
/// ocean §3 takes it; Schubert et al. 2004, from memory), and a mean surface of 102 K (the same).
const EUROPA_MEASURED: (f64, f64, f64, f64) = (1.560_8e6, 3_013.0, 0.08, 102.0);

/// Europa's measured bulk with its subsurface ocean under the tide P14.T17.b's rule raises at its
/// measured elements about the table's Jupiter, at its own radius.
fn measured_europa() -> (GlobalFigures, Subsurface, WattsPerSquareMetre) {
    let derived = europa();
    let (radius, density, water, surface) = EUROPA_MEASURED;
    let tide = WattsPerSquareMetre::new(
        derived.heat_flux().value() * math::powi(radius / derived.radius().value(), 3),
    );
    let ocean = Subsurface::of_regular_moon(
        Metres::new(radius),
        KilogramsPerCubicMetre::new(density),
        water,
        Kelvin::new(surface),
        tide,
    );
    let volume = 4.0 / 3.0 * core::f64::consts::PI * math::powi(radius, 3);
    let kg = density * volume;
    let figures = GlobalFigures::derive(&FiguresInputs {
        atmosphere: derived.atmosphere(),
        mass: EarthMasses::new(kg / EARTH_MASS_KG),
        radius: Metres::new(radius),
        surface_gravity: MetresPerSecondSquared::new(
            GRAVITATIONAL_CONSTANT * kg / (radius * radius),
        ),
        class: PlanetClass::Rocky,
        fractions: MassFractions::solid(0.1, 0.82, water),
        inventory_water: Kilograms::new(water * kg),
        rotation_period: Seconds::new(3.551 * SECONDS_PER_DAY),
        fe_h: Dex::new(0.0),
        age: SOLAR_AGE,
        tidal_heat_flow: tide,
        subsurface_ocean: ocean,
    })
    .unwrap();
    (figures, ocean, tide)
}

#[test]
fn europa_holds_a_subsurface_ocean_of_water_on_rock_under_an_ice_shell() {
    let (figures, ocean, tide) = measured_europa();
    let (radius, density, water, surface) = EUROPA_MEASURED;
    let (radius, density, surface) = (
        Metres::new(radius),
        KilogramsPerCubicMetre::new(density),
        Kelvin::new(surface),
    );
    eprintln!(
        "the measured Europa under a tide of {:.3} W m⁻²: {ocean:?}",
        tide.value()
    );
    let Subsurface::Ocean(ocean) = ocean else {
        panic!("Europa holds an ocean: {ocean:?}");
    };
    assert_eq!(ocean.floor(), OceanFloor::Rock);
    assert_eq!(ocean.liquid(), Gas::Water.substance());
    assert_same_bits(ocean.solute_mass_fraction(), 0.0);
    assert_eq!(ocean.base_temperature(), ICE_MELTING_POINT);
    let layers = ocean_layers(radius, density, water, surface, tide).unwrap();
    assert_same_bits(
        ocean.shell_thickness().value(),
        layers.shell_thickness.value(),
    );
    assert!(
        (ocean.thickness().value() + ocean.shell_thickness().value() - layers.water_depth.value())
            .abs()
            < 1e-6
    );
    // Its regime is an ice shell's, whatever its tides, and its tidally heated mantle keeps no
    // dynamo.
    assert_eq!(figures.subsurface_ocean(), &Subsurface::Ocean(ocean));
    assert_eq!(figures.tectonic_regime(), TectonicRegime::IceShell);
    assert_eq!(figures.magnetic_field(), MagneticField::None);
    // A thinner tide than the water layer's keeps none, by the same rule.
    assert_eq!(
        Subsurface::of_regular_moon(
            radius,
            density,
            water,
            surface,
            WattsPerSquareMetre::new(1e-4)
        ),
        Subsurface::NoOcean
    );
    // The generator's own Europa at its measured elements formed inside Jupiter's ice line, so it
    // holds no water and its flag no ocean (P14.T17.b's two classes of moon: plan 14's Risks,
    // "Deviations in P14.T24.b, as built"); its figures say so.
    let derived = europa();
    eprintln!(
        "the generated Europa: ice {:.3}, {:.1} K, tide {:.3} W m⁻², flag {}",
        derived.ice_fraction(),
        derived.equilibrium_temperature().value(),
        derived.heat_flux().value(),
        derived.subsurface_ocean()
    );
    let generated = moon_figures(&derived, 3.551);
    assert_same_bits(derived.ice_fraction(), 0.0);
    assert!(!derived.subsurface_ocean());
    assert_eq!(generated.subsurface_ocean(), &Subsurface::NoOcean);
}

#[test]
fn the_figures_are_the_same_twice() {
    let first = figures_table();
    let second = figures_table();
    for ((name, _, a), (_, _, b)) in first.iter().zip(&second) {
        assert_eq!(a, b, "{name}");
    }
}

/// The table's figures, Io's and the measured Europa's, pinned to the bit (P14.T24.b). Nothing
/// published reads them before the 21 → 22 bump, so it moves only their header, unless the batch
/// changes them.
#[test]
fn the_table_s_figures_are_pinned() {
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;

    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    let pin = |w: &mut GoldenWriter, name: &str, figures: &GlobalFigures| {
        w.f64(&format!("{name}_heat_flow"), figures.heat_flow().value());
        w.f64(
            &format!("{name}_radiogenic_heat_flow"),
            figures.radiogenic_heat_flow().value(),
        );
        w.line(&format!("{name}_interior = {:?}", figures.interior()));
        w.line(&format!(
            "{name}_tectonic_regime = {:?}",
            figures.tectonic_regime()
        ));
        w.line(&format!(
            "{name}_magnetic_field = {:?}",
            figures.magnetic_field()
        ));
        w.f64(&format!("{name}_outgassing"), figures.outgassing());
        match figures.subsurface_ocean() {
            Subsurface::Ocean(ocean) => {
                w.f64(
                    &format!("{name}_shell_thickness"),
                    ocean.shell_thickness().value(),
                );
                w.f64(
                    &format!("{name}_ocean_thickness"),
                    ocean.thickness().value(),
                );
            }
            other @ (Subsurface::NoOcean | Subsurface::NotComputed) => {
                w.line(&format!("{name}_subsurface_ocean = {other:?}"));
            }
        }
    };
    for (name, _, figures) in &figures_table() {
        pin(&mut w, &name.to_lowercase(), figures);
    }
    pin(&mut w, "giant_impact_moon", &the_giant_impact_moon());
    let io = jovian(8.931_9e22, 421_700.0, 0.0041);
    pin(&mut w, "io", &moon_figures(&io, 1.769));
    pin(&mut w, "measured_europa", &measured_europa().0);
    for (name, radius) in [
        ("pluto", 850e3),
        ("enceladus", 190e3),
        ("ganymede", 1_800e3),
    ] {
        w.f64(
            &format!("{name}_lag_at_4_5_gyr"),
            conductive_lag(
                Metres::new(radius),
                Years::new(4.5e9),
                interior::ROCK_THERMAL_DIFFUSIVITY_M2_PER_S,
            ),
        );
    }
    golden!("planetary/global_figures", w.as_str());
}
