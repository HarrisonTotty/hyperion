//! Roche limits, Hill spheres and satellite survival (plan 14, P14.T15).
//!
//! These bound where bodies can be: rings inside a planet's Roche limit, moons outside it and
//! inside the stability limit of the planet's Hill sphere, a disc's inner edge outside its star's
//! Roche limit, and the moons of a close-in planet no heavier than tides let survive (the
//! brainstorm's "Roche limits for rings, Hill spheres bounding moon orbits"). Every function is a
//! closed form; masses are in kilograms so that one function serves a star and its planet and a
//! planet and its moon alike.

use std::error::Error;
use std::fmt;

use crate::math;
use crate::planetary::params::SATELLITE_STABILITY_FRACTION;
use crate::units::consts::GRAVITATIONAL_CONSTANT;
use crate::units::{Kilograms, KilogramsPerCubicMetre, Metres, Seconds};

/// The fluid Roche limit's coefficient: 2.456.
///
/// A synchronously rotating fluid satellite, distorted by the tide into Chandrasekhar's (1969)
/// equilibrium ellipsoid, is torn apart inside 2.456 R (ρₚ ÷ ρₛ)^⅓. Tiscareno
/// (2013, "Planetary Rings", arXiv:1112.3305, eq. 1 and §1.2) writes the limit as
/// R (4π ρₚ ÷ γ ρₛ)^⅓ with γ ≈ 0.85 for that figure, which is 2.455.
pub const ROCHE_FLUID_COEFFICIENT: f64 = 2.456;

/// The rigid Roche limit's coefficient: 2^⅓ = 1.26.
///
/// A rigid spherical satellite held together by its own gravity alone loses loose material from
/// its surface inside R (2 ρₚ ÷ ρₛ)^⅓ (Kipping 2009, MNRAS 392, 181, eq. 9).
pub const ROCHE_RIGID_COEFFICIENT: f64 = 1.259_921_049_894_873_2;

/// How far a satellite's critical semi-major axis falls with the planet's eccentricity, prograde:
/// 1.0305 (Domingos, Winter and Yokoyama 2006, abstract; Rosario-Franco et al. 2020, Table 1,
/// 1.0305 ± 0.0612).
pub const PROGRADE_PLANET_ECCENTRICITY_TERM: f64 = 1.0305;

/// How far it falls with the satellite's own eccentricity, prograde: 0.2738 (Domingos et al.
/// 2006, abstract; Rosario-Franco et al. 2020, Table 1, 0.2738 ± 0.0240).
pub const PROGRADE_SATELLITE_ECCENTRICITY_TERM: f64 = 0.2738;

/// How far out a retrograde satellite on a circular orbit about a planet on a circular orbit stays
/// bound: 0.9309 of the Hill radius (Domingos et al. 2006, abstract; Namouni 2010, ApJ 719, L145,
/// quotes "f ≃ 0.93 for retrograde satellites").
pub const RETROGRADE_STABILITY_FRACTION: f64 = 0.9309;

/// The retrograde fit's planet-eccentricity term: 1.0764 (Domingos et al. 2006, abstract).
pub const RETROGRADE_PLANET_ECCENTRICITY_TERM: f64 = 1.0764;

/// The retrograde fit's satellite-eccentricity term: 0.9812 (Domingos et al. 2006, abstract).
pub const RETROGRADE_SATELLITE_ECCENTRICITY_TERM: f64 = 0.9812;

/// The fluid Roche limit of a body of density `satellite_density` about a primary of radius
/// `primary_radius` and mean density `primary_density`: 2.456 R (ρₚ ÷ ρₛ)^⅓
/// ([`ROCHE_FLUID_COEFFICIENT`]).
///
/// The radius and density stand for the primary's mass, R³ρ; give the radius that the density was
/// computed with. A ring of loose material lies inside this limit, and a moon that forms outside
/// it.
///
/// # Panics
///
/// In debug builds, if a density is not positive or the radius is negative.
///
/// # Examples
///
/// Saturn's main rings end inside the limit for porous ice:
///
/// ```
/// use hyperion_sim::planetary::derive::roche_limit_fluid;
/// use hyperion_sim::units::{KilogramsPerCubicMetre, Metres};
///
/// let saturn = Metres::new(58_232e3); // volumetric mean radius
/// let limit = roche_limit_fluid(
///     saturn,
///     KilogramsPerCubicMetre::new(687.0),
///     KilogramsPerCubicMetre::new(600.0),
/// );
/// let a_ring_edge = Metres::new(136_775e3);
/// assert!(limit > a_ring_edge);
/// ```
#[must_use]
pub fn roche_limit_fluid(
    primary_radius: Metres,
    primary_density: KilogramsPerCubicMetre,
    satellite_density: KilogramsPerCubicMetre,
) -> Metres {
    roche(
        ROCHE_FLUID_COEFFICIENT,
        primary_radius,
        primary_density,
        satellite_density,
    )
}

/// The rigid Roche limit of a body of density `satellite_density` about a primary of radius
/// `primary_radius` and mean density `primary_density`: 1.26 R (ρₚ ÷ ρₛ)^⅓
/// ([`ROCHE_RIGID_COEFFICIENT`]).
///
/// # Panics
///
/// In debug builds, if a density is not positive or the radius is negative.
#[must_use]
pub fn roche_limit_rigid(
    primary_radius: Metres,
    primary_density: KilogramsPerCubicMetre,
    satellite_density: KilogramsPerCubicMetre,
) -> Metres {
    roche(
        ROCHE_RIGID_COEFFICIENT,
        primary_radius,
        primary_density,
        satellite_density,
    )
}

/// `coefficient` R (ρₚ ÷ ρₛ)^⅓.
#[must_use]
fn roche(
    coefficient: f64,
    radius: Metres,
    primary: KilogramsPerCubicMetre,
    satellite: KilogramsPerCubicMetre,
) -> Metres {
    debug_assert!(radius.value() >= 0.0, "a radius is not negative");
    debug_assert!(
        primary.value() > 0.0 && satellite.value() > 0.0,
        "densities are positive"
    );
    Metres::new(coefficient * radius.value() * math::cbrt(primary.value() / satellite.value()))
}

/// The Hill radius at pericentre of a body of mass `mass` on an orbit of semi-major axis `a` and
/// eccentricity `e` about a primary of mass `primary_mass`: a (1 − e) (m ÷ 3M)^⅓.
///
/// Inside it the body's gravity dominates its primary's tide. With `e` = 0 it is the circular Hill
/// radius a (m ÷ 3M)^⅓ that [`satellite_stability_limit`] is measured in.
///
/// # Panics
///
/// In debug builds, if `e` is outside 0–1 or a mass is not positive.
#[must_use]
pub fn hill_radius(a: Metres, e: f64, mass: Kilograms, primary_mass: Kilograms) -> Metres {
    debug_assert!(
        (0.0..1.0).contains(&e),
        "a bound orbit's eccentricity, got {e}"
    );
    debug_assert!(mass.value() > 0.0 && primary_mass.value() > 0.0);
    Metres::new(a.value() * (1.0 - e) * math::cbrt(mass.value() / (3.0 * primary_mass.value())))
}

/// Which way a satellite goes round its planet, relative to the planet's orbit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum OrbitSense {
    /// The same way as the planet's orbit (inclination below 90°).
    Prograde,
    /// The opposite way (inclination above 90°), stable much farther out.
    Retrograde,
}

/// The largest semi-major axis at which a satellite stays bound to its planet: Domingos, Winter
/// and Yokoyama's (2006) fit of the critical semi-major axis (design note 14).
///
/// Prograde, [`SATELLITE_STABILITY_FRACTION`] (1 − 1.0305 eₚ − 0.2738 eₛ) of `hill`; retrograde,
/// 0.9309 (1 − 1.0764 eₚ − 0.9812 eₛ) of it; never below zero. `hill` is the planet's circular
/// Hill radius, aₚ (mₚ ÷ 3M★)^⅓, which is [`hill_radius`] with e = 0: the fit takes the planet's
/// eccentricity through its own term, and a pericentre Hill radius would count it twice.
///
/// # Panics
///
/// In debug builds, if an eccentricity is outside 0–1.
///
/// # Examples
///
/// The Moon lies well inside Earth's limit:
///
/// ```
/// use hyperion_sim::planetary::derive::{OrbitSense, hill_radius, satellite_stability_limit};
/// use hyperion_sim::units::{Kilograms, Metres};
/// use hyperion_sim::units::consts::{EARTH_MASS_KG, METRES_PER_AU, SOLAR_MASS_KG};
///
/// let hill = hill_radius(
///     Metres::new(METRES_PER_AU),
///     0.0,
///     Kilograms::new(EARTH_MASS_KG),
///     Kilograms::new(SOLAR_MASS_KG),
/// );
/// let limit = satellite_stability_limit(hill, 0.0167, 0.0549, OrbitSense::Prograde);
/// assert!(Metres::new(3.844e8) < limit);
/// ```
#[must_use]
pub fn satellite_stability_limit(
    hill: Metres,
    e_planet: f64,
    e_satellite: f64,
    sense: OrbitSense,
) -> Metres {
    debug_assert!((0.0..1.0).contains(&e_planet), "got e_p = {e_planet}");
    debug_assert!((0.0..1.0).contains(&e_satellite), "got e_s = {e_satellite}");
    let (fraction, planet_term, satellite_term) = match sense {
        OrbitSense::Prograde => (
            SATELLITE_STABILITY_FRACTION,
            PROGRADE_PLANET_ECCENTRICITY_TERM,
            PROGRADE_SATELLITE_ECCENTRICITY_TERM,
        ),
        OrbitSense::Retrograde => (
            RETROGRADE_STABILITY_FRACTION,
            RETROGRADE_PLANET_ECCENTRICITY_TERM,
            RETROGRADE_SATELLITE_ECCENTRICITY_TERM,
        ),
    };
    let reach = 1.0 - planet_term * e_planet - satellite_term * e_satellite;
    Metres::new(fraction * hill.value() * reach.max(0.0))
}

/// What the tides between a planet and its moon read of the planet: mass, radius, Love number k₂
/// and tidal quality factor Q.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TidalPlanet {
    mass: Kilograms,
    radius: Metres,
    love_number: f64,
    tidal_q: f64,
}

impl TidalPlanet {
    /// A planet of mass `mass`, radius `radius`, Love number `love_number` (k₂, about 0.5 for a
    /// gas giant and 0.3 for Earth) and tidal quality factor `tidal_q` (about 10⁵ for Jupiter and
    /// 10–500 for a rocky planet).
    ///
    /// # Errors
    ///
    /// The [`BuildTidalPlanetError`] naming the first input that is not positive and finite.
    pub fn new(
        mass: Kilograms,
        radius: Metres,
        love_number: f64,
        tidal_q: f64,
    ) -> Result<Self, BuildTidalPlanetError> {
        let positive = |x: f64| x.is_finite() && x > 0.0;
        if !positive(mass.value()) {
            return Err(BuildTidalPlanetError::MassNotPositive);
        }
        if !positive(radius.value()) {
            return Err(BuildTidalPlanetError::RadiusNotPositive);
        }
        if !positive(love_number) {
            return Err(BuildTidalPlanetError::LoveNumberNotPositive);
        }
        if !positive(tidal_q) {
            return Err(BuildTidalPlanetError::TidalQNotPositive);
        }
        Ok(Self {
            mass,
            radius,
            love_number,
            tidal_q,
        })
    }

    /// The planet's mass.
    #[must_use]
    pub const fn mass(&self) -> Kilograms {
        self.mass
    }

    /// The planet's radius.
    #[must_use]
    pub const fn radius(&self) -> Metres {
        self.radius
    }

    /// The planet's Love number k₂.
    #[must_use]
    pub const fn love_number(&self) -> f64 {
        self.love_number
    }

    /// The planet's tidal quality factor Q.
    #[must_use]
    pub const fn tidal_q(&self) -> f64 {
        self.tidal_q
    }
}

/// A [`TidalPlanet`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildTidalPlanetError {
    /// The mass was not positive and finite.
    MassNotPositive,
    /// The radius was not positive and finite.
    RadiusNotPositive,
    /// The Love number was not positive and finite.
    LoveNumberNotPositive,
    /// The tidal quality factor was not positive and finite.
    TidalQNotPositive,
}

impl fmt::Display for BuildTidalPlanetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::MassNotPositive => "a tidal planet's mass must be positive and finite",
            Self::RadiusNotPositive => "a tidal planet's radius must be positive and finite",
            Self::LoveNumberNotPositive => {
                "a tidal planet's love number must be positive and finite"
            }
            Self::TidalQNotPositive => {
                "a tidal planet's quality factor must be positive and finite"
            }
        })
    }
}

impl Error for BuildTidalPlanetError {}

/// The heaviest moon that tides let survive for `age` about `planet` if it began at `outermost`,
/// the farthest bound orbit: Barnes and O'Brien's (2002, ApJ 575, 1087) eq. 7 solved for the
/// moon's mass.
///
/// A moon's tidal migration takes T = (2 ÷ 13) (`a_crit`^(13/2) − Rₚ^(13/2)) Qₚ √(Mₚ ÷ G) ÷
/// (3 k₂ Mₘ Rₚ⁵) to cross from the planet's surface to the critical semi-major axis or back, so
/// Mₘ ≤ (2 ÷ 13) (`a_crit`^(13/2) − Rₚ^(13/2)) Qₚ √(Mₚ ÷ G) ÷ (3 k₂ T Rₚ⁵). With `a_crit` =
/// 0.36 `R_H` this is their eq. 8, and gives their 7 × 10⁻⁷ M⊕ for HD 209458 b. Zero if `outermost`
/// is inside the planet.
///
/// # Panics
///
/// In debug builds, if `age` is not positive.
#[must_use]
pub fn moon_mass_limit(outermost: Metres, planet: &TidalPlanet, age: Seconds) -> Kilograms {
    debug_assert!(age.value() > 0.0, "an age is positive");
    let radius = planet.radius.value();
    let a = outermost.value();
    if a <= radius {
        return Kilograms::ZERO;
    }
    let span = math::powf(a, 6.5) - math::powf(radius, 6.5);
    let radius_5 = math::powi(radius, 5);
    let numerator =
        2.0 / 13.0 * span * planet.tidal_q * (planet.mass.value() / GRAVITATIONAL_CONSTANT).sqrt();
    Kilograms::new(numerator / (3.0 * planet.love_number * age.value() * radius_5))
}

/// The heaviest moon a planet of `planet` on an orbit of semi-major axis `a` and eccentricity `e`
/// about a primary of mass `primary_mass` can keep for `age`, against the tides that brake the
/// planet's spin and make its moons migrate (Barnes and O'Brien 2002), which removes the moons of
/// close-in planets.
///
/// The moon is taken to start where it lives longest, at the prograde stability limit of design
/// note 14 ([`satellite_stability_limit`] of the planet's circular Hill radius, with a circular
/// moon), and [`moon_mass_limit`] gives the mass. That limit, 0.49 `R_H`, is the one the generator
/// places moons inside; Barnes and O'Brien took 0.36 `R_H`, and the mass goes nearly as its 13/2
/// power, so this bound is 7.4 times theirs, or more where the planet's radius is not negligible
/// against it (7.8 times for HD 209458 b).
///
/// # Panics
///
/// In debug builds, if `e` is outside 0–1, a mass is not positive or `age` is not positive.
///
/// # Examples
///
/// An Earth at 0.05 au of a Sun keeps no primordial moon heavier than 10⁻⁸ M⊕ for 5 Gyr, where at
/// 1 au it could keep one a hundred times the Moon's mass:
///
/// ```
/// use hyperion_sim::planetary::derive::limits::{TidalPlanet, maximum_surviving_moon_mass};
/// use hyperion_sim::units::consts::{
///     EARTH_MASS_KG, METRES_PER_AU, SECONDS_PER_GIGAYEAR, SOLAR_MASS_KG,
/// };
/// use hyperion_sim::units::{Kilograms, Metres, Seconds};
///
/// // k₂ = 0.3 and Q = 100, a rocky planet's.
/// let (mass, radius) = (Kilograms::new(EARTH_MASS_KG), Metres::new(6.371e6));
/// let earth = TidalPlanet::new(mass, radius, 0.3, 100.0)?;
/// let (sun, age) = (Kilograms::new(SOLAR_MASS_KG), Seconds::new(5.0 * SECONDS_PER_GIGAYEAR));
/// let close_in = Metres::new(0.05 * METRES_PER_AU);
/// let close = maximum_surviving_moon_mass(&earth, sun, close_in, 0.0, age);
/// assert!(close.value() / EARTH_MASS_KG < 1e-8);
/// let far = maximum_surviving_moon_mass(&earth, sun, Metres::new(METRES_PER_AU), 0.0167, age);
/// assert!(far.value() / EARTH_MASS_KG > 1.0);
/// # Ok::<(), hyperion_sim::planetary::derive::limits::BuildTidalPlanetError>(())
/// ```
#[must_use]
pub fn maximum_surviving_moon_mass(
    planet: &TidalPlanet,
    primary_mass: Kilograms,
    a: Metres,
    e: f64,
    age: Seconds,
) -> Kilograms {
    let hill = hill_radius(a, 0.0, planet.mass, primary_mass);
    let outermost = satellite_stability_limit(hill, e, 0.0, OrbitSense::Prograde);
    moon_mass_limit(outermost, planet, age)
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::units::consts::{
        EARTH_MASS_KG, GM_EARTH, GM_JUPITER, GM_SUN, JUPITER_MASS_KG, METRES_PER_AU,
        SECONDS_PER_GIGAYEAR, SOLAR_MASS_KG,
    };

    fn au(x: f64) -> Metres {
        Metres::new(x * METRES_PER_AU)
    }

    fn kg_of_gm(gm: f64) -> Kilograms {
        Kilograms::new(gm / GRAVITATIONAL_CONSTANT)
    }

    use OrbitSense::{Prograde as P, Retrograde as R};

    /// The planets whose moons are tested: name, semi-major axis (au), eccentricity and GM
    /// (m³ s⁻²), from JPL's approximate elements (Standish), the IAU 2015 nominal GM of Jupiter and
    /// Earth, and JPL's planetary GMs for the rest, Pluto's without Charon.
    const PLANETS: [(&str, f64, f64, f64); 7] = [
        ("Earth", 1.000_002_61, 0.016_711_23, GM_EARTH),
        ("Mars", 1.523_710_34, 0.093_394_10, 4.282_837e13),
        ("Jupiter", 5.202_887, 0.048_386_24, GM_JUPITER),
        ("Saturn", 9.536_675_94, 0.053_861_79, 3.793_118_7e16),
        ("Uranus", 19.189_164_64, 0.047_257_44, 5.793_939e15),
        ("Neptune", 30.069_922_76, 0.008_590_48, 6.836_529e15),
        ("Pluto", 39.482_116_75, 0.248_827_30, 8.696e11),
    ];

    /// The moons tested: planet, name, semi-major axis (km), eccentricity and sense, from JPL's
    /// mean elements of the planetary satellites (Solar System Dynamics, "Planetary Satellite Mean
    /// Elements", sep.html, fetched 2026-09-23); a moon inclined more than 90° is retrograde. They
    /// are the regular moons and the irregulars nearest their limits: all 187 moons of that table
    /// lie inside the fit, the nearest Jupiter's Aoede at 0.93 of its limit, then Megaclite,
    /// Cyllene, Geirrod and Thiazzi near 0.89.
    const MOONS: [(&str, &str, f64, f64, OrbitSense); 44] = [
        ("Earth", "Moon", 384_400.0, 0.0554, P),
        ("Mars", "Phobos", 9_375.0, 0.015, P),
        ("Mars", "Deimos", 23_457.0, 0.0, P),
        ("Jupiter", "Io", 421_800.0, 0.004, P),
        ("Jupiter", "Callisto", 1_882_700.0, 0.007, P),
        ("Jupiter", "Themisto", 7_397_000.0, 0.257, P),
        ("Jupiter", "Himalia", 11_439_000.0, 0.160, P),
        ("Jupiter", "Carpo", 17_039_500.0, 0.415, P),
        ("Jupiter", "Valetudo", 18_690_100.0, 0.217, P),
        ("Jupiter", "Ananke", 21_029_500.0, 0.238, R),
        ("Jupiter", "Pasiphae", 23_463_200.0, 0.412, R),
        ("Jupiter", "Megaclite", 23_640_100.0, 0.421, R),
        ("Jupiter", "Cyllene", 23_650_000.0, 0.421, R),
        ("Jupiter", "Sinope", 23_679_300.0, 0.262, R),
        ("Jupiter", "Aoede", 23_773_100.0, 0.437, R),
        ("Jupiter", "Kore", 24_203_300.0, 0.338, R),
        ("Saturn", "Mimas", 186_000.0, 0.020, P),
        ("Saturn", "Titan", 1_221_900.0, 0.029, P),
        ("Saturn", "Iapetus", 3_561_700.0, 0.028, P),
        ("Saturn", "Phoebe", 12_929_400.0, 0.164, R),
        ("Saturn", "Siarnaq", 17_881_100.0, 0.308, P),
        ("Saturn", "Gerd", 20_947_500.0, 0.517, R),
        ("Saturn", "Geirrod", 22_259_400.0, 0.539, R),
        ("Saturn", "Ymir", 22_955_600.0, 0.338, R),
        ("Saturn", "Thiazzi", 23_577_500.0, 0.511, R),
        ("Saturn", "Fornjot", 24_936_800.0, 0.213, R),
        ("Uranus", "Miranda", 129_846.0, 0.001, P),
        ("Uranus", "Oberon", 583_511.0, 0.002, P),
        ("Uranus", "Sycorax", 12_193_200.0, 0.520, R),
        ("Uranus", "Margaret", 14_425_000.0, 0.642, P),
        ("Uranus", "Ferdinand", 20_421_400.0, 0.395, R),
        ("Neptune", "Proteus", 117_600.0, 0.0, P),
        ("Neptune", "Triton", 354_800.0, 0.0, R),
        ("Neptune", "Nereid", 5_513_900.0, 0.751, P),
        ("Neptune", "Halimede", 16_590_500.0, 0.521, R),
        ("Neptune", "Sao", 22_239_900.0, 0.296, P),
        ("Neptune", "Laomedeia", 23_499_900.0, 0.419, P),
        ("Neptune", "Psamathe", 47_646_600.0, 0.413, R),
        ("Neptune", "Neso", 49_897_800.0, 0.455, R),
        ("Pluto", "Charon", 19_600.0, 0.0, P),
        ("Pluto", "Styx", 43_200.0, 0.025, P),
        ("Pluto", "Nix", 49_300.0, 0.015, P),
        ("Pluto", "Kerberos", 58_300.0, 0.010, P),
        ("Pluto", "Hydra", 65_200.0, 0.009, P),
    ];

    #[test]
    fn saturn_s_rings_lie_inside_its_fluid_roche_limit_for_porous_ice() {
        // Saturn's volumetric mean radius and the mean density it gives with Saturn's GM.
        let radius = 58_232e3;
        let density = 3.793_118_7e16
            / GRAVITATIONAL_CONSTANT
            / (4.0 / 3.0 * core::f64::consts::PI * radius * radius * radius);
        assert!((density - 687.0).abs() < 1.0, "{density}");
        let limit = roche_limit_fluid(
            Metres::new(radius),
            KilogramsPerCubicMetre::new(density),
            KilogramsPerCubicMetre::new(600.0),
        );
        let in_radii = limit.value() / radius;
        assert!((2.5..2.7).contains(&in_radii), "{in_radii} Saturn radii");
        // The A ring's outer edge, 2.27 equatorial radii of 60,268 km, is inside it.
        let a_ring: f64 = 136_775e3;
        assert!((a_ring / 60_268e3 - 2.27).abs() < 0.005);
        assert!(limit.value() > a_ring, "{} km", limit.value() / 1e3);
        // The rigid limit is the fluid one scaled by 1.26 ÷ 2.456.
        let rigid = roche_limit_rigid(
            Metres::new(radius),
            KilogramsPerCubicMetre::new(density),
            KilogramsPerCubicMetre::new(600.0),
        );
        assert!((rigid / limit - 1.26 / 2.456).abs() < 1e-3);
    }

    #[test]
    fn the_fluid_coefficient_is_tiscareno_s_form_with_chandrasekhar_s_figure() {
        let gamma = 0.85;
        let coefficient = math::cbrt(4.0 * core::f64::consts::PI / gamma);
        assert!(
            (coefficient - ROCHE_FLUID_COEFFICIENT).abs() < 2e-3,
            "{coefficient}"
        );
        assert!((math::cbrt(2.0) - ROCHE_RIGID_COEFFICIENT).abs() < 1e-15);
    }

    #[test]
    fn earth_s_hill_radius_is_one_and_a_half_million_kilometres() {
        let earth = kg_of_gm(GM_EARTH);
        let sun = kg_of_gm(GM_SUN);
        let hill = hill_radius(au(1.000_002_61), 0.016_711_23, earth, sun).value();
        assert!((hill / 1.5e9 - 1.0).abs() < 0.02, "{hill} m");
        let circular = hill_radius(au(1.000_002_61), 0.0, earth, sun).value();
        assert!((circular / hill - 1.0 / (1.0 - 0.016_711_23)).abs() < 1e-12);
        assert!((circular - 1.4966e9).abs() < 1e6, "{circular} m");
    }

    #[test]
    fn every_solar_system_moon_tested_lies_inside_its_stability_limit() {
        let sun = kg_of_gm(GM_SUN);
        for (planet, name, a_km, e, sense) in MOONS {
            let &(_, a_au, e_planet, gm) = PLANETS
                .iter()
                .find(|p| p.0 == planet)
                .expect("every moon's planet is listed");
            let hill = hill_radius(au(a_au), 0.0, kg_of_gm(gm), sun);
            let limit = satellite_stability_limit(hill, e_planet, e, sense);
            let a = a_km * 1e3;
            assert!(
                a < limit.value(),
                "{planet}'s {name} at {:.3} R_H is beyond its limit of {:.3} R_H",
                a / hill.value(),
                limit / hill
            );
        }
    }

    #[test]
    fn the_stability_limit_is_domingos_s_fit_and_never_negative() {
        let hill = Metres::new(1e10);
        let circular = satellite_stability_limit(hill, 0.0, 0.0, OrbitSense::Prograde);
        assert!((circular.value() - 0.4895e10).abs() < 1.0);
        let retro = satellite_stability_limit(hill, 0.0, 0.0, OrbitSense::Retrograde);
        assert!((retro.value() - 0.9309e10).abs() < 1.0);
        let eccentric = satellite_stability_limit(hill, 0.1, 0.2, OrbitSense::Prograde);
        let expected = 0.4895e10 * (1.0 - 1.0305 * 0.1 - 0.2738 * 0.2);
        assert!((eccentric.value() - expected).abs() < 1.0);
        let torn = satellite_stability_limit(hill, 0.9, 0.9, OrbitSense::Retrograde);
        assert_same_bits(torn.value(), 0.0);
    }

    #[test]
    fn barnes_and_o_brien_s_limit_for_hd_209458_b_is_reproduced() {
        // Their §4.1: M★ = 1.1 M☉, M_p = 0.69 M_J, R_p = 1.35 R_J, a_p = 0.0468 au, T = 5 Gyr,
        // k₂ = 0.51, Q_p = 10⁵, f = 0.36: no moon over 7 × 10⁻⁷ M⊕.
        let planet = TidalPlanet::new(
            Kilograms::new(0.69 * JUPITER_MASS_KG),
            Metres::new(1.35 * 7.1492e7),
            0.51,
            1e5,
        )
        .unwrap();
        let star = Kilograms::new(1.1 * SOLAR_MASS_KG);
        let hill = hill_radius(au(0.0468), 0.0, planet.mass(), star);
        let age = Seconds::new(5.0 * SECONDS_PER_GIGAYEAR);
        let theirs = moon_mass_limit(hill * 0.36, &planet, age).value() / EARTH_MASS_KG;
        // Their eq. 8 drops the planet's radius from eq. 7's a_crit^(13/2) − R_p^(13/2), which
        // here is 6% of the whole; restored, the figure is theirs.
        let radius_term = math::powf(planet.radius().value() / (hill.value() * 0.36), 6.5);
        let eq_8 = theirs / (1.0 - radius_term);
        assert!(
            (eq_8 / 7e-7 - 1.0).abs() < 0.01,
            "{eq_8} M⊕ by eq. 8, {theirs} by eq. 7"
        );
        assert!((0.05..0.07).contains(&radius_term), "{radius_term}");
        // At the stability limit the generator uses, 0.4895 R_H, the bound is 7.4 times heavier.
        let ours = maximum_surviving_moon_mass(&planet, star, au(0.0468), 0.0, age).value()
            / EARTH_MASS_KG;
        let ratio = ours / theirs;
        assert!((7.3..7.9).contains(&ratio), "{ratio}");
    }

    /// The moon mass limit of a planet of `mass` M⊕, radius `radius` m, Love number `k2` and
    /// quality factor `q` at `a_au` of a solar mass for 5 Gyr, in M⊕.
    fn moon_limit_m_earth(mass: f64, radius: f64, k2: f64, q: f64, a_au: f64) -> f64 {
        let planet = TidalPlanet::new(
            Kilograms::new(mass * EARTH_MASS_KG),
            Metres::new(radius),
            k2,
            q,
        )
        .unwrap();
        let age = Seconds::new(5.0 * SECONDS_PER_GIGAYEAR);
        let sun = Kilograms::new(SOLAR_MASS_KG);
        maximum_surviving_moon_mass(&planet, sun, au(a_au), 0.0, age).value() / EARTH_MASS_KG
    }

    #[test]
    fn a_rocky_planet_at_a_twentieth_of_an_au_keeps_no_moon_over_a_millionth_of_an_earth() {
        // An Earth and a super-Earth, k₂ = 0.3 and Q = 100.
        for (name, mass, radius) in [("Earth", 1.0, 6.371e6), ("super-Earth", 5.0, 1.1e7)] {
            let limit = moon_limit_m_earth(mass, radius, 0.3, 100.0, 0.05);
            assert!(limit < 1e-6, "{name}: {limit} M⊕");
        }
        // A Neptune (k₂ = 0.13, Q = 10⁴) keeps up to 4 × 10⁻⁶ M⊕ at the generator's stability
        // limit; plan 14's 10⁻⁶ holds for it only at Barnes and O'Brien's 0.36 R_H.
        let neptune = moon_limit_m_earth(17.15, 2.4622e7, 0.13, 1e4, 0.05);
        assert!((1e-6..1e-5).contains(&neptune), "Neptune: {neptune} M⊕");
        assert!(neptune / math::powf(0.4895 / 0.36, 6.5) < 1e-6);
        let sun = Kilograms::new(SOLAR_MASS_KG);
        let age = Seconds::new(5.0 * SECONDS_PER_GIGAYEAR);
        // Farther out the same Earth keeps a moon of the Moon's size for as long.
        let earth = TidalPlanet::new(
            Kilograms::new(EARTH_MASS_KG),
            Metres::new(6.371e6),
            0.3,
            100.0,
        )
        .unwrap();
        let at_1 = maximum_surviving_moon_mass(&earth, sun, au(1.0), 0.0167, age).value();
        assert!(at_1 / EARTH_MASS_KG > 0.0123, "{} M⊕", at_1 / EARTH_MASS_KG);
    }

    #[test]
    fn a_moon_limit_inside_the_planet_is_zero() {
        let planet = TidalPlanet::new(
            Kilograms::new(EARTH_MASS_KG),
            Metres::new(6.371e6),
            0.3,
            100.0,
        )
        .unwrap();
        let age = Seconds::new(1e16);
        assert_same_bits(moon_mass_limit(Metres::new(6e6), &planet, age).value(), 0.0);
    }

    #[test]
    fn tidal_planets_are_validated_once() {
        let (m, r) = (Kilograms::new(1e24), Metres::new(1e6));
        assert!(TidalPlanet::new(m, r, 0.3, 100.0).is_ok());
        assert_eq!(
            TidalPlanet::new(Kilograms::ZERO, r, 0.3, 100.0),
            Err(BuildTidalPlanetError::MassNotPositive)
        );
        assert_eq!(
            TidalPlanet::new(m, Metres::new(-1.0), 0.3, 100.0),
            Err(BuildTidalPlanetError::RadiusNotPositive)
        );
        assert_eq!(
            TidalPlanet::new(m, r, f64::NAN, 100.0),
            Err(BuildTidalPlanetError::LoveNumberNotPositive)
        );
        assert_eq!(
            TidalPlanet::new(m, r, 0.3, 0.0),
            Err(BuildTidalPlanetError::TidalQNotPositive)
        );
        let text = BuildTidalPlanetError::LoveNumberNotPositive.to_string();
        assert!(text.starts_with('a') && !text.ends_with('.'));
    }
}
