//! Hill spacing and the stability floor between neighbouring planets (plan 14, P14.T6.a; design
//! note 7).
//!
//! Neighbours are spaced in mutual Hill radii, `R_H` = ((m₁ + m₂) ÷ 3M★)^⅓ (a₁ + a₂) ÷ 2, the unit
//! in which the brainstorm centres spacing "on the observed 14–20" and floors it "at about 10–12".
//! A pair satisfies the floor when both of design note 7's conditions hold: its spacing is at
//! least [`spacing_floor`] (10 rising to 12 with eccentricity for two small planets, 7 when one is
//! a giant), and the inner planet's apocentre and the outer's pericentre are at least 2√3 mutual
//! Hill radii apart (Gladman 1993), which makes "no overlapping orbits" a theorem of the generator.
//! The spacing draw that uses these is P14.T6.b.

use crate::math;
use crate::planetary::params::{
    HILL_STABLE_GAP, SPACING_FLOOR_ECCENTRICITY_SLOPE, SPACING_FLOOR_GIANT,
    SPACING_FLOOR_SMALL_CIRCULAR, SPACING_FLOOR_SMALL_ECCENTRIC, SPACING_GIANT_MASS,
};
use crate::units::consts::{EARTH_MASS_KG, SOLAR_MASS_KG};
use crate::units::{EarthMasses, Metres, SolarMasses};

/// The largest Δχ at which [`next_semi_major_axis`] still places a neighbour: 0.9. Beyond it the
/// next orbit would lie more than 19 times farther out, and there is no room for another planet.
pub const MAX_SPACING_STEP: f64 = 0.9;

/// A pair's Hill factor χ = ½ ((m₁ + m₂) ÷ 3M★)^⅓, dimensionless: the pair's mutual Hill radius is
/// χ (a₁ + a₂).
///
/// A type of its own, so that it cannot be passed where a spacing is meant.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct HillFactor(f64);

impl HillFactor {
    /// The factor χ.
    #[must_use]
    pub const fn value(self) -> f64 {
        self.0
    }
}

/// The Hill factor χ = ½ ((m₁ + m₂) ÷ 3M★)^⅓ of two planets of masses `m1` and `m2` about a host
/// of mass `m_host`.
///
/// # Panics
///
/// In debug builds, if the host's mass is not positive or a planet's is negative.
#[must_use]
pub fn mutual_hill_factor(m1: EarthMasses, m2: EarthMasses, m_host: SolarMasses) -> HillFactor {
    debug_assert!(m_host.value() > 0.0, "a host's mass is positive");
    debug_assert!(
        m1.value() >= 0.0 && m2.value() >= 0.0,
        "planet masses are not negative"
    );
    let ratio = (m1.value() + m2.value()) * EARTH_MASS_KG / (3.0 * m_host.value() * SOLAR_MASS_KG);
    HillFactor(0.5 * math::cbrt(ratio))
}

/// The mutual Hill radius of two planets of masses `m1` and `m2` at semi-major axes `a1` and `a2`
/// about a host of mass `m_host`: ((m₁ + m₂) ÷ 3M★)^⅓ × (a₁ + a₂) ÷ 2.
///
/// # Panics
///
/// In debug builds, if the host's mass is not positive or a planet's is negative.
///
/// # Examples
///
/// Jupiter and Saturn are 7.9 mutual Hill radii apart:
///
/// ```
/// use hyperion_sim::planetary::placement::mutual_hill_radius;
/// use hyperion_sim::units::{AstronomicalUnits, EarthMasses, Metres, SolarMasses};
///
/// let au = |x: f64| Metres::from(AstronomicalUnits::new(x));
/// let (a1, a2) = (au(5.2029), au(9.5367));
/// let (jupiter, saturn) = (EarthMasses::new(317.8), EarthMasses::new(95.16));
/// let hill = mutual_hill_radius(jupiter, saturn, SolarMasses::new(1.0), a1, a2);
/// let spacing = (a2 - a1) / hill;
/// assert!((spacing - 7.9).abs() < 0.05);
/// ```
#[must_use]
pub fn mutual_hill_radius(
    m1: EarthMasses,
    m2: EarthMasses,
    m_host: SolarMasses,
    a1: Metres,
    a2: Metres,
) -> Metres {
    // ((m₁ + m₂) ÷ 3M★)^⅓ × (a₁ + a₂) ÷ 2 is 2χ × (a₁ + a₂) ÷ 2.
    Metres::new(mutual_hill_factor(m1, m2, m_host).value() * (a1.value() + a2.value()))
}

/// The semi-major axis of the neighbour outside `a1` at a spacing of `delta` mutual Hill radii,
/// for the pair's factor `chi` ([`mutual_hill_factor`]): a₁ (1 + Δχ) ÷ (1 − Δχ).
///
/// It inverts [`mutual_hill_radius`]: the spacing of the result is `delta`. `None` when Δχ is at
/// least [`MAX_SPACING_STEP`], where there is no room for another planet.
///
/// # Panics
///
/// In debug builds, if `delta` or χ is negative or not finite.
///
/// # Examples
///
/// Placing an Earth-mass neighbour 20 mutual Hill radii outside an Earth at 1 au:
///
/// ```
/// use hyperion_sim::planetary::placement::{
///     mutual_hill_factor, mutual_hill_radius, next_semi_major_axis,
/// };
/// use hyperion_sim::units::{AstronomicalUnits, EarthMasses, Metres, SolarMasses};
///
/// let (earth, sun) = (EarthMasses::new(1.0), SolarMasses::new(1.0));
/// let a1 = Metres::from(AstronomicalUnits::new(1.0));
/// let chi = mutual_hill_factor(earth, earth, sun);
/// let a2 = next_semi_major_axis(a1, 20.0, chi).expect("room for a neighbour");
/// let spacing = (a2 - a1) / mutual_hill_radius(earth, earth, sun, a1, a2);
/// assert!((spacing - 20.0).abs() < 1e-12);
/// assert!((AstronomicalUnits::from(a2).value() - 1.29).abs() < 0.01);
/// ```
#[must_use]
pub fn next_semi_major_axis(a1: Metres, delta: f64, chi: HillFactor) -> Option<Metres> {
    let chi = chi.value();
    debug_assert!(delta.is_finite() && delta >= 0.0, "a spacing, got {delta}");
    debug_assert!(chi.is_finite() && chi >= 0.0, "a Hill factor, got {chi}");
    let step = delta * chi;
    (step < MAX_SPACING_STEP).then(|| Metres::new(a1.value() * (1.0 + step) / (1.0 - step)))
}

/// The least spacing, in mutual Hill radii, at which two neighbours of masses `m1` and `m2` and
/// eccentricities `e1` and `e2` are placed (design note 7).
///
/// For two planets under [`SPACING_GIANT_MASS`] (0.1 `M_J`), the measured floor of Kepler's small
/// planets: 10 for circular orbits, rising by 80 per unit of mean eccentricity to at most 12 (Pu
/// and Wu 2015). For a pair with a giant, 7 (Chambers et al. 1996; Marzari and Weidenschilling
/// 2002), because the small planets' floor would forbid Jupiter and Saturn, 7.9 apart.
///
/// # Panics
///
/// In debug builds, if an eccentricity is outside 0–1.
#[must_use]
pub fn spacing_floor(m1: EarthMasses, m2: EarthMasses, e1: f64, e2: f64) -> f64 {
    debug_assert!((0.0..1.0).contains(&e1) && (0.0..1.0).contains(&e2));
    let giant = EarthMasses::from(SPACING_GIANT_MASS);
    if m1 < giant && m2 < giant {
        let mean_e = f64::midpoint(e1, e2);
        (SPACING_FLOOR_SMALL_CIRCULAR + SPACING_FLOOR_ECCENTRICITY_SLOPE * mean_e)
            .min(SPACING_FLOOR_SMALL_ECCENTRIC)
    } else {
        SPACING_FLOOR_GIANT
    }
}

/// A planet as the spacing floor sees it: its mass and the size and shape of its orbit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Neighbour {
    mass: EarthMasses,
    semi_major_axis: Metres,
    eccentricity: f64,
}

impl Neighbour {
    /// A planet of mass `mass` on an orbit of semi-major axis `semi_major_axis` and eccentricity
    /// `eccentricity`.
    ///
    /// # Panics
    ///
    /// In debug builds, if the mass or the semi-major axis is not positive, or the eccentricity
    /// is outside 0–1.
    #[must_use]
    pub fn new(mass: EarthMasses, semi_major_axis: Metres, eccentricity: f64) -> Self {
        debug_assert!(mass.value() > 0.0 && semi_major_axis.value() > 0.0);
        debug_assert!((0.0..1.0).contains(&eccentricity), "got e = {eccentricity}");
        Self {
            mass,
            semi_major_axis,
            eccentricity,
        }
    }

    /// The planet's mass.
    #[must_use]
    pub const fn mass(&self) -> EarthMasses {
        self.mass
    }

    /// The orbit's semi-major axis.
    #[must_use]
    pub const fn semi_major_axis(&self) -> Metres {
        self.semi_major_axis
    }

    /// The orbit's eccentricity.
    #[must_use]
    pub const fn eccentricity(&self) -> f64 {
        self.eccentricity
    }
}

/// Whether `inner` and `outer`, neighbours about a host of mass `m_host`, satisfy both of design
/// note 7's conditions: a spacing of at least [`spacing_floor`] mutual Hill radii, and at least
/// [`HILL_STABLE_GAP`] (2√3) mutual Hill radii between the inner apocentre and the outer
/// pericentre (Gladman 1993). False if `outer` is not outside `inner`.
///
/// # Panics
///
/// In debug builds, if the host's mass is not positive.
///
/// # Examples
///
/// Jupiter and Saturn clear the giants' floor of 7; two Jupiters 1.3 au apart do not:
///
/// ```
/// use hyperion_sim::planetary::placement::{Neighbour, satisfies_floor};
/// use hyperion_sim::units::{AstronomicalUnits, EarthMasses, Metres, SolarMasses};
///
/// let au = |x: f64| Metres::from(AstronomicalUnits::new(x));
/// let sun = SolarMasses::new(1.0);
/// let jupiter = Neighbour::new(EarthMasses::new(317.8), au(5.203), 0.048);
/// let saturn = Neighbour::new(EarthMasses::new(95.16), au(9.537), 0.054);
/// assert!(satisfies_floor(&jupiter, &saturn, sun));
/// let twin = Neighbour::new(EarthMasses::new(317.8), au(6.5), 0.0);
/// assert!(!satisfies_floor(&jupiter, &twin, sun));
/// ```
#[must_use]
pub fn satisfies_floor(inner: &Neighbour, outer: &Neighbour, m_host: SolarMasses) -> bool {
    let (a1, a2) = (inner.semi_major_axis, outer.semi_major_axis);
    if a2 <= a1 {
        return false;
    }
    let hill = mutual_hill_radius(inner.mass, outer.mass, m_host, a1, a2).value();
    let spacing = (a2.value() - a1.value()) / hill;
    let floor = spacing_floor(
        inner.mass,
        outer.mass,
        inner.eccentricity,
        outer.eccentricity,
    );
    let apocentre = a1.value() * (1.0 + inner.eccentricity);
    let pericentre = a2.value() * (1.0 - outer.eccentricity);
    spacing >= floor && pericentre - apocentre >= HILL_STABLE_GAP * hill
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::lcg::Lcg;

    use super::*;
    use crate::units::AstronomicalUnits;
    use crate::units::consts::{GM_EARTH, GM_JUPITER, GM_SUN, GRAVITATIONAL_CONSTANT};

    fn au(x: f64) -> Metres {
        Metres::from(AstronomicalUnits::new(x))
    }

    fn m_earth_of_gm(gm: f64) -> EarthMasses {
        EarthMasses::new(gm / GM_EARTH)
    }

    /// The eight planets: semi-major axis (au), eccentricity (JPL's approximate elements,
    /// Standish) and GM (m³ s⁻²).
    fn planets() -> [(&'static str, Neighbour); 8] {
        let p = |a: f64, e: f64, gm: f64| Neighbour::new(m_earth_of_gm(gm), au(a), e);
        [
            ("Mercury", p(0.387_099_27, 0.205_635_93, 2.203_2e13)),
            ("Venus", p(0.723_335_66, 0.006_776_72, 3.248_59e14)),
            ("Earth", p(1.000_002_61, 0.016_711_23, GM_EARTH)),
            ("Mars", p(1.523_710_34, 0.093_394_10, 4.282_837e13)),
            ("Jupiter", p(5.202_887, 0.048_386_24, GM_JUPITER)),
            ("Saturn", p(9.536_675_94, 0.053_861_79, 3.793_118_7e16)),
            ("Uranus", p(19.189_164_64, 0.047_257_44, 5.793_939e15)),
            ("Neptune", p(30.069_922_76, 0.008_590_48, 6.836_529e15)),
        ]
    }

    fn sun() -> SolarMasses {
        SolarMasses::new(GM_SUN / GRAVITATIONAL_CONSTANT / SOLAR_MASS_KG)
    }

    fn spacing(inner: &Neighbour, outer: &Neighbour, host: SolarMasses) -> f64 {
        let (a1, a2) = (inner.semi_major_axis(), outer.semi_major_axis());
        (a2 - a1) / mutual_hill_radius(inner.mass(), outer.mass(), host, a1, a2)
    }

    #[test]
    fn the_solar_system_satisfies_the_floor_pairwise() {
        let planets = planets();
        for (i, (inner_name, inner)) in planets.iter().enumerate() {
            for (outer_name, outer) in &planets[i + 1..] {
                assert!(
                    satisfies_floor(inner, outer, sun()),
                    "{inner_name} and {outer_name}: {:.2} mutual Hill radii",
                    spacing(inner, outer, sun())
                );
            }
        }
        // Jupiter and Saturn at 7.9, against the giants' floor of 7.
        let (jupiter, saturn) = (&planets[4].1, &planets[5].1);
        let js = spacing(jupiter, saturn, sun());
        assert!((js - 7.9).abs() < 0.05, "{js}");
        assert!((spacing_floor(jupiter.mass(), saturn.mass(), 0.05, 0.05) - 7.0).abs() < 1e-15);
        // Uranus and Neptune are both under 0.1 M_J, so the small planets' floor applies.
        let (uranus, neptune) = (&planets[6].1, &planets[7].1);
        assert!(spacing_floor(uranus.mass(), neptune.mass(), 0.0, 0.0) >= 10.0);
    }

    #[test]
    fn two_jupiters_at_five_point_two_and_six_point_five_au_fail() {
        let jupiter = m_earth_of_gm(GM_JUPITER);
        let inner = Neighbour::new(jupiter, au(5.2), 0.0);
        let outer = Neighbour::new(jupiter, au(6.5), 0.0);
        assert!(!satisfies_floor(&inner, &outer, sun()));
        assert!(spacing(&inner, &outer, sun()) < 3.0);
    }

    #[test]
    fn kepler_11_b_and_c_fail_narrowly() {
        // Lissauer et al. (2013, ApJ 770, 131, Tables 3 and 4): M★ = 0.961 M☉; b 1.9 M⊕ at 0.091 au,
        // e = 0.045; c 2.9 M⊕ at 0.107 au, e = 0.026.
        let host = SolarMasses::new(0.961);
        let b = Neighbour::new(EarthMasses::new(1.9), au(0.091), 0.045);
        let c = Neighbour::new(EarthMasses::new(2.9), au(0.107), 0.026);
        let bc = spacing(&b, &c, host);
        assert!((bc - 9.4).abs() < 0.1, "{bc}");
        assert!(!satisfies_floor(&b, &c, host));
        // Short of even the circular floor of 10.
        let circular_c = Neighbour::new(c.mass(), c.semi_major_axis(), 0.0);
        let circular_b = Neighbour::new(b.mass(), b.semi_major_axis(), 0.0);
        assert!(!satisfies_floor(&circular_b, &circular_c, host));
    }

    #[test]
    fn next_semi_major_axis_inverts_the_mutual_hill_radius() {
        let mut rng = Lcg::new(0x5eed_0014_0006);
        for _ in 0..10_000 {
            let a1 = au(0.01 + 50.0 * rng.next_f64());
            let m1 = EarthMasses::new(0.01 + 3_000.0 * rng.next_f64());
            let m2 = EarthMasses::new(0.01 + 3_000.0 * rng.next_f64());
            let host = SolarMasses::new(0.08 + 3.0 * rng.next_f64());
            let delta = 2.0 + 40.0 * rng.next_f64();
            let chi = mutual_hill_factor(m1, m2, host);
            let Some(a2) = next_semi_major_axis(a1, delta, chi) else {
                assert!(delta * chi.value() >= MAX_SPACING_STEP);
                continue;
            };
            let back = (a2 - a1) / mutual_hill_radius(m1, m2, host, a1, a2);
            assert!(
                (back / delta - 1.0).abs() < 1e-12,
                "{delta} came back as {back}"
            );
        }
    }

    #[test]
    fn there_is_no_room_past_the_largest_step() {
        assert_eq!(
            next_semi_major_axis(au(1.0), 10.0, HillFactor(0.0901)),
            None
        );
        assert_eq!(
            next_semi_major_axis(au(1.0), 1.0, HillFactor(MAX_SPACING_STEP)),
            None
        );
        let a = next_semi_major_axis(au(1.0), 10.0, HillFactor(0.0899)).unwrap();
        assert!((AstronomicalUnits::from(a).value() - 1.899 / 0.101).abs() < 1e-9);
    }

    #[test]
    fn the_small_planets_floor_rises_with_eccentricity_to_twelve() {
        let small = EarthMasses::new(5.0);
        assert!((spacing_floor(small, small, 0.0, 0.0) - 10.0).abs() < 1e-15);
        assert!((spacing_floor(small, small, 0.01, 0.01) - 10.8).abs() < 1e-12);
        assert!((spacing_floor(small, small, 0.02, 0.02) - 11.6).abs() < 1e-12);
        // Pu and Wu's 12 at σₑ = 0.02, a mean eccentricity of 0.025.
        assert!((spacing_floor(small, small, 0.025, 0.025) - 12.0).abs() < 1e-12);
        assert!((spacing_floor(small, small, 0.3, 0.1) - 12.0).abs() < 1e-15);
        // 0.1 M_J is the boundary: a pair with a planet at or above it takes the giants' floor.
        let giant = EarthMasses::from(SPACING_GIANT_MASS);
        assert!((giant.value() - 31.78).abs() < 0.01, "{}", giant.value());
        assert!((spacing_floor(small, giant, 0.0, 0.0) - 7.0).abs() < 1e-15);
        assert!((spacing_floor(giant, small, 0.3, 0.3) - 7.0).abs() < 1e-15);
    }

    #[test]
    fn overlapping_orbits_never_satisfy_the_floor() {
        // Widely spaced in Hill radii but crossing: the inner apocentre is at 1.7 au.
        let inner = Neighbour::new(EarthMasses::new(1.0), au(1.0), 0.7);
        let outer = Neighbour::new(EarthMasses::new(1.0), au(1.6), 0.0);
        assert!(spacing(&inner, &outer, sun()) > 12.0);
        assert!(!satisfies_floor(&inner, &outer, sun()));
        // The same orbits in the wrong order, or the same orbit twice.
        assert!(!satisfies_floor(&outer, &inner, sun()));
        assert!(!satisfies_floor(&inner, &inner, sun()));
    }

    #[test]
    fn the_gap_at_closest_approach_is_two_root_three_mutual_hill_radii() {
        assert!((HILL_STABLE_GAP - 2.0 * 3.0_f64.sqrt()).abs() < 1e-15);
        // Two giants at a spacing of 8, just clear of the floor of 7, with eccentricities that
        // bring them to 2√3 mutual Hill radii at closest approach, and just past it.
        let jupiter = m_earth_of_gm(GM_JUPITER);
        let a1 = au(5.0);
        let chi = mutual_hill_factor(jupiter, jupiter, sun());
        let a2 = next_semi_major_axis(a1, 8.0, chi).unwrap();
        let hill = mutual_hill_radius(jupiter, jupiter, sun(), a1, a2).value();
        let e_at_gap =
            (a2.value() - a1.value() - HILL_STABLE_GAP * hill) / (a1.value() + a2.value());
        let pair = |e: f64| {
            satisfies_floor(
                &Neighbour::new(jupiter, a1, e),
                &Neighbour::new(jupiter, a2, e),
                sun(),
            )
        };
        assert!(pair(e_at_gap * (1.0 - 1e-9)));
        assert!(!pair(e_at_gap * (1.0 + 1e-9)));
    }
}
