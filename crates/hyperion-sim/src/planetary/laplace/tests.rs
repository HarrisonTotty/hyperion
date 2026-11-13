//! Tests of the classical Laplace surface (P14.T56.a), against the Solar System's moons and
//! Tremaine, Touma and Namouni's (2009) instability.

use core::f64::consts::TAU;

use hyperion_testkit::float::assert_same_bits;

use super::*;
use crate::orbit::{Eccentricity, Orientation};
use crate::planetary::derive::figure::darwin_radau_flattening;
use crate::units::consts::{METRES_PER_AU, SOLAR_MASS_KG};
use crate::units::{Degrees, GravitationalParameter};

/// `degrees` in radians.
fn deg(degrees: f64) -> Radians {
    Radians::from(Degrees::new(degrees))
}

/// `radians` in degrees.
fn degrees(radians: f64) -> f64 {
    Degrees::from(Radians::new(radians)).value()
}

/// Asserts two vectors equal bit for bit.
fn same(a: [f64; 3], b: [f64; 3]) {
    for (x, y) in a.into_iter().zip(b) {
        assert_same_bits(x, y);
    }
}

/// The angle between two unit vectors, rad.
fn angle(a: [f64; 3], b: [f64; 3]) -> f64 {
    math::atan2(norm(cross(a, b)), dot(a, b))
}

/// A heliocentric orbit of `a_au` and `e` in the galactic plane.
fn orbit(a_au: f64, e: f64) -> KeplerElements {
    KeplerElements::from_semi_major_axis(
        Metres::new(a_au * METRES_PER_AU),
        GravitationalParameter::from_solar_masses(crate::units::SolarMasses::new(1.0)),
        Eccentricity::new(e).unwrap(),
        Orientation::new(Radians::ZERO, Radians::ZERO, Radians::ZERO).unwrap(),
        Radians::ZERO,
    )
    .unwrap()
}

/// A planet of the Solar System as its moons' Laplace surface reads it: J₂ at its reference
/// radius, its orbit, its mass over the Sun's and its obliquity.
struct Planet {
    j2: f64,
    radius_km: f64,
    a_au: f64,
    e: f64,
    mass_over_sun: f64,
    obliquity_deg: f64,
}

impl Planet {
    /// The Laplace radius at `a_km` with the moons `moons` (mass over the planet's, axis in km)
    /// that lie inside it counted.
    fn laplace_radius(&self, moons: &[(f64, f64)], a_km: f64) -> Metres {
        let inner = moons
            .iter()
            .filter(|&&(_, axis)| axis < a_km)
            .map(|&(mass_ratio, axis)| InnerMoon {
                mass_ratio,
                semi_major_axis: Metres::new(axis * 1e3),
            });
        let j2r2 = effective_j2r2(self.j2, Metres::new(self.radius_km * 1e3), inner);
        let mass = Kilograms::new(self.mass_over_sun * SOLAR_MASS_KG);
        laplace_radius(
            j2r2,
            &orbit(self.a_au, self.e),
            mass,
            Kilograms::new(SOLAR_MASS_KG),
        )
    }

    /// The tilt of the local Laplace plane from the equator at `a_km`, degrees.
    fn tilt(&self, moons: &[(f64, f64)], a_km: f64) -> f64 {
        let rl = self.laplace_radius(moons, a_km);
        degrees(laplace_angle(deg(self.obliquity_deg), a_km * 1e3 / rl.value()).value())
    }
}

/// The Sun's GM, km³ s⁻² (JPL's DE440 planetary constants), which every mass below is taken over
/// as a ratio of GMs, so that G never enters.
const GM_SUN_KM3_S2: f64 = 1.327_124_400_419_4e11;

/// Saturn: J₂ = 16,290.615 × 10⁻⁶ referred to 60,330 km (Jacobson 2022, AJ 164, 199, Table 6), GM
/// 37,931,206.234 km³ s⁻² (its Table 4), its J2000 mean orbit, 9.537 070 32 au and e =
/// 0.054 150 60, and its 26.73° obliquity (NASA's Saturn fact sheet).
const SATURN: Planet = Planet {
    j2: 16_290.615e-6,
    radius_km: 60_330.0,
    a_au: 9.537_070_32,
    e: 0.054_150_60,
    mass_over_sun: 37_931_206.234 / GM_SUN_KM3_S2,
    obliquity_deg: 26.73,
};

/// Saturn's mass, kg, of its GM (JPL's SAT441), which its moons' masses are taken over.
const SATURN_KG: f64 = 5.683_17e26;

/// Saturn's major moons inside Iapetus, inside out: mass over Saturn's (JPL's SAT441 GMs) and
/// semi-major axis, km (NASA's Saturnian satellite fact sheet): Mimas, Enceladus, Tethys, Dione,
/// Rhea, Titan.
const SATURN_MOONS: [(f64, f64); 6] = [
    (3.7509e19 / SATURN_KG, 185_520.0),
    (1.0803e20 / SATURN_KG, 238_020.0),
    (6.1750e20 / SATURN_KG, 294_660.0),
    (1.0955e21 / SATURN_KG, 377_400.0),
    (2.3065e21 / SATURN_KG, 527_040.0),
    (1.345_18e23 / SATURN_KG, 1_221_870.0),
];

/// Jupiter: J₂ = 14,696.5063 × 10⁻⁶ referred to 71,492 km (Durante et al. 2020, as Militzer and
/// Hubbard 2023, PSJ 4, 95, Table 1, give it), GM 126,686,531.9 km³ s⁻² (JPL's JUP365), its 5.2038
/// au (NASA's Jupiter fact sheet), J2000 e = 0.048 392 66 and 3.13° obliquity.
const JUPITER: Planet = Planet {
    j2: 14_696.506_3e-6,
    radius_km: 71_492.0,
    a_au: 5.2038,
    e: 0.048_392_66,
    mass_over_sun: 126_686_531.9 / GM_SUN_KM3_S2,
    obliquity_deg: 3.13,
};

/// Jupiter's mass, kg (NASA's Jupiter fact sheet), which the Galileans' are taken over.
const JUPITER_KG: f64 = 1.898_13e27;

/// The Galilean moons, inside out: mass over Jupiter's and semi-major axis, km (NASA's Jovian
/// satellite fact sheet).
const GALILEANS: [(f64, f64); 4] = [
    (8.932e22 / JUPITER_KG, 421_800.0),
    (4.800e22 / JUPITER_KG, 671_100.0),
    (1.4819e23 / JUPITER_KG, 1_070_400.0),
    (1.0759e23 / JUPITER_KG, 1_882_700.0),
];

/// The Earth: J₂ = 1,082.63 × 10⁻⁶ referred to 6,378.137 km, 1.000 000 11 au, e = 0.0167 and
/// 23.44° (NASA's Earth fact sheet), and GM 398,600.436 km³ s⁻² (DE440).
const EARTH: Planet = Planet {
    j2: 1_082.63e-6,
    radius_km: 6_378.137,
    a_au: 1.000_000_11,
    e: 0.0167,
    mass_over_sun: 398_600.436 / GM_SUN_KM3_S2,
    obliquity_deg: 23.44,
};

/// Neptune: J₂ = 3,408.428 × 10⁻⁶ referred to 25,225 km (Jacobson 2009, AJ 137, 4322, Table 5),
/// its J2000 mean orbit, 30.068 96 au and e = 0.008 585 87, and its 28.32° (NASA's Neptune fact
/// sheet), with M ÷ M☉ = 5.150 31 × 10⁻⁵ (JPL's NEP097 GM).
const NEPTUNE: Planet = Planet {
    j2: 3_408.428e-6,
    radius_km: 25_225.0,
    a_au: 30.068_96,
    e: 0.008_585_87,
    mass_over_sun: 5.150_31e-5,
    obliquity_deg: 28.32,
};

#[test]
fn titan_s_plane_lies_about_0_6_degrees_off_saturn_s_equator() {
    // JPL's SAT441 gives 0.6°.
    let tilt = SATURN.tilt(&SATURN_MOONS, 1_221_870.0);
    assert!((0.55..=0.70).contains(&tilt), "{tilt}°");
}

#[test]
fn iapetus_s_plane_lies_about_15_degrees_off_saturn_s_equator_with_titan_counted() {
    // JPL's SAT441 gives 14.8°; Tremaine et al.'s eq. 4 counts Titan, at a third of Iapetus's
    // distance, a little short (the plan's Risks).
    let iapetus = 3_560_850.0;
    let tilt = SATURN.tilt(&SATURN_MOONS, iapetus);
    assert!((14.0..=17.0).contains(&tilt), "{tilt}°");
    // Without the inner moons' quadrupole the plane lies much nearer Saturn's orbit.
    let alone = SATURN.tilt(&[], iapetus);
    assert!(alone > tilt + 4.0, "{alone}° alone against {tilt}°");
}

#[test]
fn callisto_s_plane_lies_about_half_a_degree_off_jupiter_s_equator() {
    // JPL's JUP365 gives 0.4°; eq. 4 counts Ganymede short, as Titan for Iapetus.
    let tilt = JUPITER.tilt(&GALILEANS, 1_882_700.0);
    assert!((0.4..=0.6).contains(&tilt), "{tilt}°");
}

#[test]
fn the_moon_s_plane_is_the_ecliptic() {
    let tilt = EARTH.tilt(&[], 384_400.0);
    assert!(
        EARTH.obliquity_deg - tilt < 0.01,
        "{tilt}° of {}°",
        EARTH.obliquity_deg
    );
    // The Earth's Laplace radius is about 8.4 of its radii (Tremaine et al. 2009, §4.1).
    let rl = EARTH.laplace_radius(&[], 384_400.0).value() / 6_378.137e3;
    assert!((8.0..=8.8).contains(&rl), "{rl} Earth radii");
}

#[test]
fn triton_s_plane_is_neptune_s_equator() {
    // A test particle's plane: Triton itself holds about 2% of the system's angular momentum and
    // tilts Neptune's pole 0.46° from their invariable plane (Jacobson 2009), which is not
    // modelled (the plan's Risks).
    let tilt = NEPTUNE.tilt(&[], 354_760.0);
    assert!(tilt < 0.01, "{tilt}°");
}

#[test]
fn the_angle_runs_from_the_equator_to_the_orbit_continuously_and_monotonically() {
    for obliquity in [
        0.5, 10.0, 26.73, 45.0, 60.0, 80.0, 89.0, 91.0, 98.0, 135.0, 179.0,
    ] {
        let e = acute(deg(obliquity));
        let near = laplace_angle(deg(obliquity), 1e-3).value();
        let far = laplace_angle(deg(obliquity), 1e3).value();
        assert!(near < 1e-9, "{obliquity}°: φ {near} rad close in");
        assert!(
            (far - e).abs() < 1e-9,
            "{obliquity}°: φ {far} rad far out against {e}"
        );
        let mut previous = laplace_angle(deg(obliquity), 1e-2).value();
        // A factor of 1.001 a step from 0.01 to 100 r_L.
        for k in 1..=9_215 {
            let x = 1e-2 * math::powf(1.001, f64::from(k));
            let phi = laplace_angle(deg(obliquity), x).value();
            assert!(
                phi >= previous,
                "{obliquity}° at {x}: {phi} after {previous}"
            );
            // Near 90° the plane turns steeply about r_L, but without a jump: a step of the grid
            // over 0.01 rad resolves into steps under 0.01 rad a thousand times finer.
            if phi - previous > 0.01 {
                let mut last = previous;
                for j in 1..=1_000 {
                    let between = x / 1.001 * math::powf(1.001, f64::from(j) / 1_000.0);
                    let sub = laplace_angle(deg(obliquity), between).value();
                    assert!(
                        sub >= last && sub - last < 0.01,
                        "{obliquity}° at {between}: {sub} after {last}"
                    );
                    last = sub;
                }
            }
            assert!((0.0..=e).contains(&phi));
            previous = phi;
        }
    }
}

#[test]
fn the_angle_solves_tremaine_et_al_s_equilibrium() {
    // Their eq. 23, a⁵ sin 2(φ − φ⊙) + 2 r_L⁵ sin 2φ = 0, divided by a⁵.
    for obliquity in [5.0, 26.73, 60.0, 85.0] {
        for x in [0.3, 0.9, 1.0, 1.7, 4.0] {
            let phi = laplace_angle(deg(obliquity), x).value();
            let e = deg(obliquity).value();
            let residual =
                math::sin(2.0 * (phi - e)) + 2.0 * math::powi(x, -5) * math::sin(2.0 * phi);
            assert!(residual.abs() < 1e-12, "{obliquity}° at {x}: {residual}");
        }
    }
}

#[test]
fn the_pole_lies_between_the_axes_on_the_spin_s_side() {
    let rl = Metres::new(1.0);
    let normal = [0.0, 0.0, 1.0];
    for obliquity in [10.0, 60.0, 98.0, 150.0] {
        let (sin_e, cos_e) = math::sin_cos(deg(obliquity).value());
        let spin = [sin_e * math::cos(0.7), sin_e * math::sin(0.7), cos_e];
        for x in [0.2, 0.9, 1.1, 3.0] {
            let pole = laplace_pole(spin, normal, rl, Metres::new(x));
            assert!((norm(pole) - 1.0).abs() < 1e-15);
            // In the plane of the two axes.
            assert!(dot(pole, cross(spin, normal)).abs() < 1e-14);
            let phi = laplace_angle(deg(obliquity), x).value();
            assert!(
                (angle(pole, spin) - phi).abs() < 1e-12,
                "{obliquity}° at {x}"
            );
            // Within 90° of the spin, and towards the nearer sense of the normal.
            assert!(dot(pole, spin) > 0.0);
            let towards = if obliquity > 90.0 { -1.0 } else { 1.0 };
            let theta = acute(deg(obliquity)) - phi;
            assert!((angle(pole, normal.map(|v| v * towards)) - theta).abs() < 1e-12);
        }
    }
}

#[test]
fn a_parent_of_zero_obliquity_keeps_every_plane_on_its_equator() {
    let axis = unit([0.3, -0.2, 0.9]);
    for x in [0.01, 1.0, 100.0] {
        let pole = laplace_pole(axis, axis, Metres::new(1.0), Metres::new(x));
        assert!(angle(pole, axis) < 1e-15, "{x}");
    }
}

/// Whether a circular orbit at `x` `r_L` about a parent of obliquity `obliquity` is unstable by the
/// sign of Tremaine et al.'s (2009) eq. 34, their eq. 33 with the equilibrium of eq. 22
/// substituted: λ² = −9 ε⊙² ÷ (2048 sin² 2φ) × {…}, positive where the braces are negative.
fn unstable_by_eq_34(obliquity: f64, x: f64) -> (bool, f64) {
    let s = obliquity;
    let p = angle_of(obliquity, x);
    let c = math::cos;
    let braces = -106.0 + 24.0 * c(2.0 * p) + 146.0 * c(4.0 * p)
        - 100.0 * c(6.0 * p - 2.0 * s)
        - 24.0 * c(2.0 * p - 4.0 * s)
        + 224.0 * c(2.0 * p - 2.0 * s)
        - 54.0 * c(4.0 * p - 4.0 * s)
        - 8.0 * c(2.0 * s)
        - 11.0 * c(4.0 * s)
        - 124.0 * c(2.0 * p + 2.0 * s)
        + 25.0 * c(8.0 * p - 4.0 * s)
        + 8.0 * c(4.0 * p - 2.0 * s);
    let sin_2p = math::sin(2.0 * p);
    (braces < 0.0, -9.0 / (2048.0 * sin_2p * sin_2p) * braces)
}

#[test]
fn the_criterion_is_tremaine_et_al_s_eq_34() {
    let (mut compared, mut unstable) = (0, 0);
    for i in 1..900 {
        let obliquity = f64::from(i) * 0.1;
        for j in 0..=1_000 {
            let x = math::powf(10.0, -1.0 + 2.0 * f64::from(j) / 1_000.0);
            let (by_eq_34, lambda2) = unstable_by_eq_34(deg(obliquity).value(), x);
            let ours = eccentricity_unstable(deg(obliquity), x);
            // Where λ² is zero to rounding, either answer is the edge.
            assert!(
                ours == by_eq_34 || lambda2.abs() < 1e-9,
                "{obliquity}° at {x}: {ours} against λ² = {lambda2}"
            );
            unstable += usize::from(ours);
            compared += 1;
        }
    }
    assert!(unstable > 1_000, "{unstable} unstable of {compared}");
}

#[test]
fn the_band_is_one_interval_inside_its_scan() {
    for k in 0..=425 {
        let obliquity = 68.8 + 0.05 * f64::from(k);
        let obliquity = deg(obliquity.min(90.0 - 1e-9));
        let mut runs = 0;
        let mut was = false;
        for j in 0..=20_000 {
            let x = math::powf(10.0, -1.0 + 2.0 * f64::from(j) / 20_000.0);
            let now = eccentricity_unstable(obliquity, x);
            if now {
                assert!(
                    (BAND_SCAN.0..=BAND_SCAN.1).contains(&x),
                    "{obliquity:?} at {x}"
                );
            }
            runs += usize::from(now && !was);
            was = now;
        }
        assert!(runs <= 1, "{obliquity:?}: {runs} runs");
    }
    // None below the band-free obliquity, nor anywhere for a mirror-image parent.
    for k in 0..=688 {
        let obliquity = deg(0.1 * f64::from(k));
        for j in 0..=2_000 {
            let x = math::powf(10.0, -1.0 + 2.0 * f64::from(j) / 2_000.0);
            assert!(!eccentricity_unstable(obliquity, x), "{obliquity:?} at {x}");
        }
    }
}

#[test]
fn the_criterion_reads_the_axis_not_its_sense() {
    for obliquity in [70.0, 80.0, 89.0] {
        for j in 0..=200 {
            let x = 0.8 + 0.003 * f64::from(j);
            assert_eq!(
                eccentricity_unstable(deg(obliquity), x),
                eccentricity_unstable(deg(180.0 - obliquity), x),
                "{obliquity}° at {x}"
            );
        }
    }
}

/// Whether any orbit in the scan is unstable about a parent of `obliquity` degrees, on a grid of
/// 10⁻⁶ `r_L`, fine enough for the band's first width.
fn any_unstable(obliquity: f64) -> bool {
    (0..=450_000).any(|j| eccentricity_unstable(deg(obliquity), 0.85 + 1e-6 * f64::from(j)))
}

#[test]
fn the_band_begins_at_68_875_degrees() {
    let (mut stable, mut unstable) = (68.0, 70.0);
    for _ in 0..20 {
        let middle = f64::midpoint(stable, unstable);
        if any_unstable(middle) {
            unstable = middle;
        } else {
            stable = middle;
        }
    }
    assert!((unstable - 68.875).abs() < 0.001, "onset at {unstable}°");
}

#[test]
fn the_band_on_its_side_runs_from_0_922_to_1_149_laplace_radii() {
    let (inner, outer) = eccentricity_band(deg(90.0)).expect("a parent on its side has a band");
    assert!((inner - 0.922).abs() < 0.002, "{inner}");
    assert!((outer - 1.149).abs() < 0.002, "{outer}");
    for (obliquity, lo, hi) in [
        (70.0, 1.014, 1.104),
        (75.0, 0.962, 1.154),
        (80.0, 0.938, 1.170),
    ] {
        let (inner, outer) = eccentricity_band(deg(obliquity)).unwrap();
        assert!((inner - lo).abs() < 0.002, "{obliquity}°: {inner}");
        assert!((outer - hi).abs() < 0.002, "{obliquity}°: {outer}");
    }
    assert_eq!(eccentricity_band(deg(60.0)), None);
    assert_eq!(eccentricity_band(deg(68.85)), None);
    // Each edge is the criterion's own.
    assert!(eccentricity_unstable(deg(90.0), inner) && eccentricity_unstable(deg(90.0), outer));
    assert!(!eccentricity_unstable(deg(90.0), inner - 1e-9));
    assert!(!eccentricity_unstable(deg(90.0), outer + 1e-9));
}

#[test]
fn an_annulus_reaches_the_band_where_it_overlaps_it() {
    let side = deg(90.0);
    assert!(reaches_band(side, 0.5, 0.95));
    assert!(reaches_band(side, 1.0, 1.05));
    assert!(reaches_band(side, 0.1, 5.0));
    assert!(!reaches_band(side, 0.1, 0.9));
    assert!(!reaches_band(side, 1.2, 3.0));
    assert!(!reaches_band(deg(30.0), 0.5, 1.5));
}

#[test]
fn the_love_number_and_j2_are_the_darwin_radau_figure_s() {
    // A uniform body's k_f is 3/2.
    assert!((fluid_love_number(0.4) - 1.5).abs() < 1e-15);
    let mass = Kilograms::new(5.972e24);
    let radius = Metres::new(6.378e6);
    let rate = TAU / (23.9345 * 3_600.0);
    let q = rotational_parameter(rate, radius, mass);
    for moment_factor in [0.21, 0.25, 0.33, 0.4] {
        // Clairaut's first-order J₂ = (2f − q) ÷ 3 of the Darwin–Radau flattening.
        let f = darwin_radau_flattening(q, moment_factor);
        let j2 = hydrostatic_j2(moment_factor, rate, radius, mass, SpinState::Despinning);
        assert!(
            ((2.0 * f - q) / 3.0 / j2 - 1.0).abs() < 1e-12,
            "{moment_factor}"
        );
        let locked = SpinState::Locked(SpinOrbitResonance::Synchronous);
        let tidal = hydrostatic_j2(moment_factor, rate, radius, mass, locked);
        assert_same_bits(tidal, j2 * SYNCHRONOUS_TIDAL_FACTOR);
        let three_two = SpinState::Locked(SpinOrbitResonance::ThreeToTwo);
        assert_same_bits(
            hydrostatic_j2(moment_factor, rate, radius, mass, three_two),
            j2,
        );
    }
    // The Earth's at its class's 0.33 comes out within 2% of its measured 1.0826 × 10⁻³.
    let earth = hydrostatic_j2(0.33, rate, radius, mass, SpinState::Despinning);
    assert!((earth / 1.082_6e-3 - 1.0).abs() < 0.02, "{earth}");
}

#[test]
fn the_inner_moons_add_half_their_quadrupole() {
    let radius = Metres::new(2.0);
    let alone = effective_j2r2(0.01, radius, []);
    assert!((alone - 0.04).abs() < 1e-15);
    let inner = [
        InnerMoon {
            mass_ratio: 1e-3,
            semi_major_axis: Metres::new(10.0),
        },
        InnerMoon {
            mass_ratio: 2e-4,
            semi_major_axis: Metres::new(30.0),
        },
    ];
    let with = effective_j2r2(0.01, radius, inner);
    assert!((with - (0.04 + 0.5 * 1e-3 * 100.0 + 0.5 * 2e-4 * 900.0)).abs() < 1e-15);
}

/// A frame of obliquity `obliquity` degrees about a Jupiter at `a_au`.
fn jupiter_frame(obliquity: f64, a_au: f64) -> LaplaceFrame {
    let (sin_e, cos_e) = math::sin_cos(deg(obliquity).value());
    LaplaceFrame::new(&LaplaceFrameParts {
        spin_pole: [sin_e, 0.0, cos_e],
        orbit_normal: [0.0, 0.0, 1.0],
        j2: 0.014_7,
        equatorial_radius: Metres::new(71_492e3),
        parent_orbit: orbit(a_au, 0.05),
        parent_mass: Kilograms::new(1.898e27),
        host_mass: Kilograms::new(SOLAR_MASS_KG),
        read_at: Years::new(4.5e9),
    })
    .unwrap()
}

#[test]
fn a_frame_s_laplace_radius_is_the_free_function_s_bit_for_bit() {
    let frame = jupiter_frame(3.0, 5.2);
    let inner = [InnerMoon {
        mass_ratio: 4.7e-5,
        semi_major_axis: Metres::new(4.2e8),
    }];
    let free = laplace_radius(
        effective_j2r2(0.014_7, Metres::new(71_492e3), inner),
        &orbit(5.2, 0.05),
        Kilograms::new(1.898e27),
        Kilograms::new(SOLAR_MASS_KG),
    );
    assert_same_bits(frame.laplace_radius(inner).value(), free.value());
    assert!((degrees(frame.obliquity().value()) - 3.0).abs() < 1e-12);
}

#[test]
fn a_capture_s_pole_turns_to_the_orbit_s_side_only_about_a_retrograde_spin() {
    for obliquity in [30.0, 150.0] {
        let frame = jupiter_frame(obliquity, 5.2);
        let rl = frame.laplace_radius([]);
        let far = Metres::new(rl.value() * 50.0);
        let spin = frame.local(rl, PoleSense::Spin).pole_at(far);
        let orbit = frame.local(rl, PoleSense::Orbit).pole_at(far);
        assert!(dot(orbit, frame.orbit_normal()) > 0.999);
        if obliquity < 90.0 {
            same(spin, orbit);
        } else {
            same(spin, orbit.map(|v| -v));
            assert!(dot(spin, frame.spin_pole()) > 0.0);
        }
    }
}

#[test]
fn a_frame_is_validated_once() {
    let good = LaplaceFrameParts {
        spin_pole: [0.0, 0.0, 2.0],
        orbit_normal: [0.0, 1.0, 1.0],
        j2: 0.01,
        equatorial_radius: Metres::new(1e7),
        parent_orbit: orbit(1.0, 0.0),
        parent_mass: Kilograms::new(1e25),
        host_mass: Kilograms::new(SOLAR_MASS_KG),
        read_at: Years::new(1e9),
    };
    let frame = LaplaceFrame::new(&good).unwrap();
    // Its axes are made unit vectors, 45° apart.
    assert!((norm(frame.spin_pole()) - 1.0).abs() < 1e-15);
    assert!((degrees(frame.obliquity().value()) - 45.0).abs() < 1e-12);
    let cases = [
        (
            LaplaceFrameParts {
                spin_pole: [0.0; 3],
                ..good
            },
            BuildLaplaceFrameError::Axis,
        ),
        (
            LaplaceFrameParts {
                orbit_normal: [f64::NAN, 0.0, 1.0],
                ..good
            },
            BuildLaplaceFrameError::Axis,
        ),
        (
            LaplaceFrameParts { j2: -1e-3, ..good },
            BuildLaplaceFrameError::Quadrupole,
        ),
        (
            LaplaceFrameParts {
                equatorial_radius: Metres::new(0.0),
                ..good
            },
            BuildLaplaceFrameError::Size,
        ),
        (
            LaplaceFrameParts {
                host_mass: Kilograms::new(f64::INFINITY),
                ..good
            },
            BuildLaplaceFrameError::Size,
        ),
    ];
    for (parts, error) in cases {
        assert_eq!(LaplaceFrame::new(&parts), Err(error));
        let text = error.to_string();
        assert_eq!(text, text.to_lowercase());
        assert!(!text.ends_with('.'));
    }
}

#[test]
fn a_band_moon_is_unstable_on_its_local_plane() {
    let frame = jupiter_frame(85.0, 5.2);
    let rl = frame.laplace_radius([]);
    let local = frame.local(rl, PoleSense::Spin);
    assert!(local.unstable_at(rl));
    assert!(!local.unstable_at(Metres::new(rl.value() * 0.5)));
    assert!(
        !frame
            .local(rl, PoleSense::Spin)
            .unstable_at(Metres::new(rl.value() * 2.0))
    );
    assert_eq!(local.sense(), PoleSense::Spin);
    assert_same_bits(local.laplace_radius().value(), rl.value());
    assert!((local.obliquity().value() - deg(85.0).value()).abs() < 1e-12);
}
