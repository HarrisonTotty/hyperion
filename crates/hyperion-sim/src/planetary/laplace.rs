//! The classical Laplace surface on which a planet's moons and rings lie (plan 14, P14.T56.a;
//! decision-p14-moons §1), pure.
//!
//! # The surface
//!
//! A satellite's orbit precesses about its **local Laplace plane**: under the parent's oblateness
//! close in, about the parent's equator, and under its host's tide far out, about the parent's
//! orbital plane. Tremaine, Touma and Namouni (2009, AJ 137, 3706; arXiv:0809.0237, §3, eqs. 22–23)
//! give the circular equilibria between the two. On the **classical** surface, the one with
//! φ → 0 as a → 0 and φ → φ⊙ as a → ∞ (their §3.1.2), the plane's pole lies in the plane of the
//! parent's spin axis ŝ and its orbit normal n̂, at the angle φ from ŝ towards n̂ given by
//!
//! tan 2φ = sin 2ε′ ÷ (cos 2ε′ + 2 (`r_L` ÷ a)⁵)   ([`laplace_angle`]),
//!
//! their eq. 23, a⁵ sin 2(φ − φ⊙) + 2 `r_L`⁵ sin 2φ = 0, solved for φ. Here ε′ = min(ε, π − ε) is
//! the acute angle between the two axes: J₂ and the stellar tide each depend on an axis, not on
//! its sense (their eq. 21 is even in the orbit's angular momentum), and their solutions above
//! 90° follow by (φ, φ⊙) → (π − φ, π − φ⊙). The pole is placed on the spin's side
//! ([`laplace_pole`]), so that a regular moon of a parent spinning retrograde orbits prograde
//! about its spin, as Uranus's do. Goldreich (1965, AJ 70, 5) shows that close satellites follow
//! the precessing equator; Ward (1981, Icarus 46, 97) puts Iapetus's plane between Saturn's
//! equator and its orbit.
//!
//! # The Laplace radius
//!
//! `r_L`⁵ = J₂′R² `a_p`³ (1 − `e_p`²)^{3/2} `M_p` ÷ `M_*` (their eq. 24; [`laplace_radius`]),
//! with `a_p` and `e_p` the parent's orbit and `M_*` the mass it orbits (a pair's, for a
//! circumbinary parent). The quadrupole J₂′R² = J₂R² + ½ Σ (`m_i` ÷ `M_p`) `a_i`² adds every moon inside the one in question
//! (their eq. 4; [`effective_j2r2`]): Titan's moves Iapetus's plane from about 23° to 16°.
//!
//! # The parent's J₂
//!
//! J₂ is the first-order hydrostatic value of the parent's spin ([`hydrostatic_j2`]): Clairaut's
//! f = (3 ÷ 2) J₂ + q ÷ 2 with the Darwin–Radau flattening f = (5 ÷ 2) q ÷ (1 + ξ²), ξ =
//! (5 ÷ 2)(1 − (3 ÷ 2) C ÷ M a²) ([`darwin_radau_flattening`]), gives J₂ = `k_f` q ÷ 3 with the
//! fluid Love number `k_f` = (4 − ξ²) ÷ (1 + ξ²) ([`fluid_love_number`]; Gao and Stevenson 2013,
//! Icarus 226, 1185, eqs. 2–3; Ragazzo 2020, São Paulo J. Math. Sci. 14, 1, eq. 2.74), with
//! q = ω² a³ ÷ G M and C taken at the equatorial radius a, as the figure takes them. A parent
//! locked 1:1 adds the static tide, J₂ = (5 ÷ 6) `k_f` q (J₂ ÷ C₂₂ = 10 ÷ 3; Gao and Stevenson
//! 2013, §2.1), the factor [`SYNCHRONOUS_TIDAL_FACTOR`] that its figure takes (Dermott 1979, Icarus 37,
//! 575). J₂ is taken from the spin whatever the figure's law: a body below the hydrostatic size
//! still has its spin's quadrupole, and an irregular shape's own is not modelled.
//!
//! `k_f` is positive only for C ÷ M a² above 2 ÷ 15; every class's moment of inertia lies at
//! 0.21–0.34. The first-order relation is good to about ±6% for the giants (Jupiter's at its
//! measured 0.264 comes out 6% high, Saturn's 1.5% low), and `r_L` goes as J₂^{1/5}; at the gas
//! giant's 0.25 Jupiter's comes out 0.0135, 8% under Juno's 0.014 697, so its `r_L` is 2% short.
//!
//! # The eccentricity band
//!
//! Above an obliquity of 68.875°, circular orbits on the classical surface are unstable to
//! eccentricity in a band about `r_L` (Tremaine et al. 2009, §3.1.2, p. 11: "restricted to
//! semimajor axes between about 0.9 and 1.25 times the Laplace radius"; their eq. 34 puts the
//! outer edge at 1.171 at most). The test ([`eccentricity_unstable`]) is the linear criterion of
//! decision-p14-moons §1.2, unstable if and only if 0 < 4 + γ < 10 sin²θ with
//! γ = (2 ÷ 3)(`r_L` ÷ a)⁵ (9 cos²φ − 3) − 2 cos²θ and θ = ε′ − φ, the plane's angle from n̂. It
//! is exactly their condition: linearised on the circular equilibrium (their eq. 25, second
//! line), λ² = (9 ε⊙² ÷ 16)(C² − 2 − g)(4C² − 3 + g) with C = cos θ and g = (`r_L` ÷ a)⁵
//! (3 cos²φ − 1), and the criterion says both factors are negative, the only way λ² > 0 (both
//! positive would need C² > 1). The tests check it against the sign of their eq. 34, eq. 33 with
//! eq. 22 substituted; the arXiv text of eq. 33 needs its 6 sin 2φ sin 2(φ − φ⊙) term's sign
//! turned to reduce to eq. 34. A regular moon inside the band is dropped with every regular moon
//! beyond it, and a ring that reaches into it is not made
//! ([`satellites`](crate::planetary::satellites), [`rings`](crate::planetary::rings)).
//!
//! # The hold
//!
//! Every moon and ring lies on this surface from [`MOONS_VERSION`]; below it they keep version
//! 22's planes, the parent's orbital plane, bit for bit ([`SatellitePlanes::in_force`]).
//!
//! [`darwin_radau_flattening`]: crate::planetary::derive::figure::darwin_radau_flattening

use core::f64::consts::FRAC_PI_2;
use std::error::Error;
use std::fmt;

use crate::GENERATOR_VERSION;
use crate::math;
use crate::orbit::KeplerElements;
use crate::planetary::derive::figure::rotational_parameter;
use crate::planetary::derive::rotation::{SpinOrbitResonance, SpinState};
use crate::planetary::moons::MOONS_VERSION;
use crate::planetary::params::SYNCHRONOUS_TIDAL_FACTOR;
use crate::units::consts::RADIANS_PER_DEGREE;
use crate::units::{Kilograms, Metres, Radians, Years};

/// The obliquity ε′ below which no circular orbit on the classical surface is unstable to
/// eccentricity, with a margin: 68.8°, under Tremaine et al.'s (2009, §3.1.2) 68.875°, which the
/// criterion reproduces (`the_band_begins_at_68_875_degrees`). [`eccentricity_band`] looks for no
/// band below it.
pub(crate) const BAND_FREE_OBLIQUITY_DEG: f64 = 68.8;

/// Where [`eccentricity_band`] looks for the band's edges, in a ÷ `r_L`: 0.85–1.30, about
/// Tremaine et al.'s (2009, p. 11) "between about 0.9 and 1.25". The band of every obliquity lies
/// at 0.922–1.171 and is one interval (`the_band_is_one_interval_inside_its_scan`).
pub(crate) const BAND_SCAN: (f64, f64) = (0.85, 1.30);

/// The steps of [`eccentricity_band`]'s scan of [`BAND_SCAN`]: 450, so 0.001 `r_L` apart, finer
/// than the band is wide 0.01° above its onset.
const BAND_SCAN_STEPS: u32 = 450;

/// The halvings by which [`eccentricity_band`] refines each edge from its scan: 40, to about
/// 10⁻¹⁵ `r_L`.
const BAND_BISECTIONS: u32 = 40;

/// Which planes a generated system's satellites are referred to (P14.T56.a, held).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SatellitePlanes {
    /// Version 22's: every moon's elements and every ring about the parent's orbital plane.
    OrbitalPlane,
    /// Every moon and ring on its parent's classical Laplace surface (this module).
    LaplaceSurface,
}

impl SatellitePlanes {
    /// The planes the generator version in force takes: [`LaplaceSurface`](Self::LaplaceSurface)
    /// from [`MOONS_VERSION`], and [`OrbitalPlane`](Self::OrbitalPlane) below it, so that every
    /// value below it is version 22's, bit for bit.
    #[must_use]
    pub const fn in_force() -> Self {
        if GENERATOR_VERSION.get() >= MOONS_VERSION.get() {
            Self::LaplaceSurface
        } else {
            Self::OrbitalPlane
        }
    }
}

/// The fluid Love number `k_f` = (4 − ξ²) ÷ (1 + ξ²) of a body of moment of inertia `moment_factor`
/// = C ÷ M a², with ξ = (5 ÷ 2)(1 − (3 ÷ 2) C ÷ M a²): the Darwin–Radau relation
/// ([`darwin_radau_flattening`]) inverted through the hydrostatic f = (1 + `k_f`) q ÷ 2 (see the
/// [module](self) documentation). 3 ÷ 2 for a uniform body (C = 0.4), and smaller the more
/// centrally condensed.
///
/// [`darwin_radau_flattening`]: crate::planetary::derive::figure::darwin_radau_flattening
#[must_use]
pub fn fluid_love_number(moment_factor: f64) -> f64 {
    let xi = 2.5 * (1.0 - 1.5 * moment_factor);
    let xi2 = xi * xi;
    (4.0 - xi2) / (1.0 + xi2)
}

/// The hydrostatic J₂ of a body of moment of inertia `moment_factor` = C ÷ M a², spinning at
/// `rate_rad_s` with equatorial radius `equatorial_radius` and mass `mass`, in the spin state `state`:
/// `k_f` q ÷ 3 ([`fluid_love_number`], [`rotational_parameter`]), times
/// [`SYNCHRONOUS_TIDAL_FACTOR`] for a body locked 1:1 (see the [module](self) documentation). Dimensionless, referred to
/// `equatorial_radius`.
///
/// # Examples
///
/// Jupiter, at its 9.925 h sidereal day, 71,492 km and the gas giant's 0.25, comes out 8% under
/// Juno's 0.014 697 (Durante et al. 2020), as the [module](self) documentation says:
///
/// ```
/// use hyperion_sim::planetary::derive::rotation::SpinState;
/// use hyperion_sim::planetary::laplace::hydrostatic_j2;
/// use hyperion_sim::units::{Kilograms, Metres};
///
/// let rate = core::f64::consts::TAU / (9.925 * 3_600.0);
/// let (radius, mass) = (Metres::new(71_492e3), Kilograms::new(1.898e27));
/// let j2 = hydrostatic_j2(0.25, rate, radius, mass, SpinState::Despinning);
/// assert!((j2 / 0.014_697 - 0.92).abs() < 0.01, "{j2}");
/// ```
#[must_use]
pub fn hydrostatic_j2(
    moment_factor: f64,
    rate_rad_s: f64,
    equatorial_radius: Metres,
    mass: Kilograms,
    state: SpinState,
) -> f64 {
    let q = rotational_parameter(rate_rad_s, equatorial_radius, mass);
    let spin = fluid_love_number(moment_factor) * q / 3.0;
    match state {
        SpinState::Locked(SpinOrbitResonance::Synchronous) => spin * SYNCHRONOUS_TIDAL_FACTOR,
        SpinState::Despinning | SpinState::Locked(SpinOrbitResonance::ThreeToTwo) => spin,
    }
}

/// One moon inside another, as its quadrupole adds to its parent's ([`effective_j2r2`]): its mass
/// over its parent's and its semi-major axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InnerMoon {
    /// The moon's mass over its parent's, `m_i` ÷ `M_p`.
    pub mass_ratio: f64,
    /// Its semi-major axis about the parent.
    pub semi_major_axis: Metres,
}

/// The effective quadrupole J₂′R², m², of a parent of J₂ `j2` referred to `radius`, with the moons
/// `inner` inside the orbit in question: J₂R² + ½ Σ (`m_i` ÷ `M_p`) `a_i`² (Tremaine et al. 2009,
/// eq. 4), summed in the order given.
#[must_use]
pub fn effective_j2r2(j2: f64, radius: Metres, inner: impl IntoIterator<Item = InnerMoon>) -> f64 {
    let r = radius.value();
    inner.into_iter().fold(j2 * r * r, |sum, moon| {
        let a = moon.semi_major_axis.value();
        sum + 0.5 * moon.mass_ratio * a * a
    })
}

/// The stellar tide's factor of the Laplace radius, `a_p`³ (1 − `e_p`²)^{3/2} `M_p` ÷ `M_*`, m³,
/// of a parent of mass `parent_mass` on `parent_orbit` about a host of mass `host_mass`.
#[must_use]
fn tide_factor(parent_orbit: &KeplerElements, parent_mass: Kilograms, host_mass: Kilograms) -> f64 {
    let a = parent_orbit.semi_major_axis().value();
    let e = parent_orbit.eccentricity().value();
    let one_minus_e2 = 1.0 - e * e;
    a * a * a * (one_minus_e2 * one_minus_e2.sqrt()) * (parent_mass.value() / host_mass.value())
}

/// The Laplace radius `r_L` of a parent of effective quadrupole `j2r2_m2` ([`effective_j2r2`]), m²,
/// and mass `parent_mass`, on `parent_orbit` about a host of mass `host_mass`:
/// `r_L`⁵ = J₂′R² `a_p`³ (1 − `e_p`²)^{3/2} `M_p` ÷ `M_*` (Tremaine et al. 2009, eq. 24), where the
/// parent's oblateness and its host's tide precess an orbit equally fast.
///
/// # Examples
///
/// The Earth's is about 8.4 of its radii (Tremaine et al. 2009, §4.1), so the Moon, at 60, lies
/// on a plane close to the ecliptic:
///
/// ```
/// use hyperion_sim::orbit::{Eccentricity, KeplerElements, Orientation};
/// use hyperion_sim::planetary::laplace::{effective_j2r2, laplace_radius};
/// use hyperion_sim::units::consts::METRES_PER_AU;
/// use hyperion_sim::units::{GravitationalParameter, Kilograms, Metres, Radians, SolarMasses};
///
/// let mu = GravitationalParameter::from_solar_masses(SolarMasses::new(1.0));
/// let plane = Orientation::new(Radians::ZERO, Radians::ZERO, Radians::ZERO)?;
/// let orbit = KeplerElements::from_semi_major_axis(
///     Metres::new(METRES_PER_AU), mu, Eccentricity::new(0.0167)?, plane, Radians::ZERO)?;
/// let radius = Metres::new(6_378.137e3);
/// let j2r2 = effective_j2r2(1.082_63e-3, radius, []);
/// let rl = laplace_radius(j2r2, &orbit, Kilograms::new(5.972e24), Kilograms::new(1.989e30));
/// assert!((rl.value() / radius.value() - 8.4).abs() < 0.1);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn laplace_radius(
    j2r2_m2: f64,
    parent_orbit: &KeplerElements,
    parent_mass: Kilograms,
    host_mass: Kilograms,
) -> Metres {
    radius_of(j2r2_m2, tide_factor(parent_orbit, parent_mass, host_mass))
}

/// (J₂′R² × the tide's factor)^{1/5}: [`laplace_radius`]'s last step, shared with
/// [`LaplaceFrame::laplace_radius`] so that both give the same bits.
#[must_use]
fn radius_of(j2r2_m2: f64, tide_m3: f64) -> Metres {
    Metres::new(math::powf(j2r2_m2 * tide_m3, 0.2))
}

/// ε′ = min(ε, π − ε), the acute angle between two axes `obliquity` apart, rad, in [0, π ÷ 2].
#[must_use]
fn acute(obliquity: Radians) -> f64 {
    let e = obliquity.value();
    if e > FRAC_PI_2 {
        core::f64::consts::PI - e
    } else {
        e
    }
}

/// (`r_L` ÷ a)⁵ at `a_over_rl` = a ÷ `r_L`.
#[must_use]
fn fifth_inverse(a_over_rl: f64) -> f64 {
    let x = 1.0 / a_over_rl;
    let x2 = x * x;
    x2 * x2 * x
}

/// φ, rad, of an acute angle `acute` between the axes, at `a_over_rl`.
#[must_use]
fn angle_of(acute: f64, a_over_rl: f64) -> f64 {
    let (sin_2e, cos_2e) = math::sin_cos(2.0 * acute);
    0.5 * math::atan2(sin_2e, cos_2e + 2.0 * fifth_inverse(a_over_rl))
}

/// The tilt φ of the local Laplace plane from the parent's equator, rad, in [0, ε′], at
/// `a_over_rl` = a ÷ `r_L` about a parent of obliquity `obliquity`:
/// tan 2φ = sin 2ε′ ÷ (cos 2ε′ + 2 (`r_L` ÷ a)⁵) on the classical branch (see the [module](self)
/// documentation), with ε′ = min(ε, π − ε).
///
/// # Examples
///
/// Titan, at 20.3 Saturn radii against a Laplace radius of about 36, at Saturn's 26.73°:
///
/// ```
/// use hyperion_sim::planetary::laplace::laplace_angle;
/// use hyperion_sim::units::{Degrees, Radians};
///
/// let phi = laplace_angle(Radians::from(Degrees::new(26.73)), 20.25 / 36.3);
/// assert!((Degrees::from(phi).value() - 0.61).abs() < 0.02);
/// ```
#[must_use]
pub fn laplace_angle(obliquity: Radians, a_over_rl: f64) -> Radians {
    Radians::new(angle_of(acute(obliquity), a_over_rl))
}

/// The pole of the local Laplace plane at semi-major axis `a` about a parent spinning about
/// `spin_pole` on an orbit of normal `orbit_normal`, both unit vectors along the galactic axes,
/// whose Laplace radius there is `laplace_radius` (see the [module](self) documentation): a unit
/// vector in the plane of the two axes, [`laplace_angle`] from the spin pole towards whichever
/// sense of the orbit's normal lies within 90° of it. So it is on the spin's side: the spin pole
/// itself well inside `r_L`, and the orbit's normal, or for a parent spinning retrograde its
/// opposite, well beyond it.
///
/// # Examples
///
/// The Moon's plane, at 60 Earth radii against a Laplace radius of 8.4, lies within 0.01° of the
/// ecliptic, not on the equator 23.44° away:
///
/// ```
/// use hyperion_sim::math;
/// use hyperion_sim::planetary::laplace::laplace_pole;
/// use hyperion_sim::units::{Degrees, Metres, Radians};
///
/// let (sin, cos) = math::sin_cos(Radians::from(Degrees::new(23.44)).value());
/// let (spin, ecliptic) = ([sin, 0.0, cos], [0.0, 0.0, 1.0]);
/// let pole = laplace_pole(spin, ecliptic, Metres::new(8.4), Metres::new(60.3));
/// assert!(pole[2] > math::cos(Radians::from(Degrees::new(0.01)).value()));
/// ```
#[must_use]
pub fn laplace_pole(
    spin_pole: [f64; 3],
    orbit_normal: [f64; 3],
    laplace_radius: Metres,
    a: Metres,
) -> [f64; 3] {
    let s = unit(spin_pole);
    let normal = unit(orbit_normal);
    let towards = if dot(s, normal) < 0.0 {
        normal.map(|x| -x)
    } else {
        normal
    };
    let sin_e = norm(cross(s, towards));
    if sin_e <= 0.0 {
        // The axes are one: every plane is the equator.
        return s;
    }
    let e = math::atan2(sin_e, dot(s, towards));
    let phi = angle_of(e, a.value() / laplace_radius.value());
    // The rotation of `s` by φ towards `towards`, in their plane: a spherical interpolation.
    let (from_s, from_n) = (math::sin(e - phi) / sin_e, math::sin(phi) / sin_e);
    unit([
        from_s * s[0] + from_n * towards[0],
        from_s * s[1] + from_n * towards[1],
        from_s * s[2] + from_n * towards[2],
    ])
}

/// Whether a circular orbit at `a_over_rl` = a ÷ `r_L` on the classical Laplace surface of a parent
/// of obliquity `obliquity` is unstable to eccentricity (see the [module](self) documentation):
/// 0 < 4 + γ < 10 sin²θ, with γ = (2 ÷ 3)(`r_L` ÷ a)⁵ (9 cos²φ − 3) − 2 cos²θ and θ = ε′ − φ.
///
/// # Examples
///
/// About a parent on its side, an orbit at the Laplace radius is unstable and one at half of it
/// is not; below 68.875° none is:
///
/// ```
/// use hyperion_sim::planetary::laplace::eccentricity_unstable;
/// use hyperion_sim::units::{Degrees, Radians};
///
/// let side = Radians::from(Degrees::new(90.0));
/// assert!(eccentricity_unstable(side, 1.0));
/// assert!(!eccentricity_unstable(side, 0.5));
/// assert!(!eccentricity_unstable(Radians::from(Degrees::new(60.0)), 1.0));
/// ```
#[must_use]
pub fn eccentricity_unstable(obliquity: Radians, a_over_rl: f64) -> bool {
    let e = acute(obliquity);
    let phi = angle_of(e, a_over_rl);
    let (equator, orbit) = (math::cos(phi), math::cos(e - phi));
    let (to_equator, to_orbit) = (equator * equator, orbit * orbit);
    let gamma = (2.0 / 3.0) * fifth_inverse(a_over_rl) * (9.0 * to_equator - 3.0) - 2.0 * to_orbit;
    let growth = 4.0 + gamma;
    0.0 < growth && growth < 10.0 * (1.0 - to_orbit)
}

/// The eccentricity band of a parent of obliquity `obliquity`, its edges in a ÷ `r_L`, inner first;
/// `None` below [`BAND_FREE_OBLIQUITY_DEG`] and wherever [`eccentricity_unstable`] holds nowhere in
/// [`BAND_SCAN`]. Each edge is found by a fixed scan of the interval and a fixed number of
/// halvings, so it is the same bits on every platform.
#[must_use]
pub(crate) fn eccentricity_band(obliquity: Radians) -> Option<(f64, f64)> {
    if acute(obliquity) < BAND_FREE_OBLIQUITY_DEG * RADIANS_PER_DEGREE {
        return None;
    }
    let (lo, hi) = BAND_SCAN;
    let step = (hi - lo) / f64::from(BAND_SCAN_STEPS);
    let at = |k: u32| lo + step * f64::from(k);
    let unstable = |x: f64| eccentricity_unstable(obliquity, x);
    let first = (0..=BAND_SCAN_STEPS).find(|&k| unstable(at(k)))?;
    let last = (first..=BAND_SCAN_STEPS)
        .take_while(|&k| unstable(at(k)))
        .last()
        .unwrap_or(first);
    // Each edge lies between a stable point and an unstable one of the scan; the scan's ends are
    // stable (the band lies inside it), so both neighbours exist.
    let inner = edge(&unstable, at(first.saturating_sub(1)), at(first));
    let outer = edge(&unstable, at((last + 1).min(BAND_SCAN_STEPS)), at(last));
    Some((inner, outer))
}

/// The boundary between `stable`, where `unstable` is false, and `inside`, where it holds, by
/// [`BAND_BISECTIONS`] halvings: the point of the last halving's unstable end.
#[must_use]
fn edge(unstable: &impl Fn(f64) -> bool, stable: f64, inside: f64) -> f64 {
    let (mut out, mut inn) = (stable, inside);
    for _ in 0..BAND_BISECTIONS {
        let mid = f64::midpoint(out, inn);
        if unstable(mid) {
            inn = mid;
        } else {
            out = mid;
        }
    }
    inn
}

/// Whether an annulus from `inner` to `outer` in a ÷ `r_L` reaches into the eccentricity band of a
/// parent of obliquity `obliquity` ([`eccentricity_band`]).
#[must_use]
pub(crate) fn reaches_band(obliquity: Radians, inner: f64, outer: f64) -> bool {
    if outer < BAND_SCAN.0 {
        return false;
    }
    eccentricity_band(obliquity).is_some_and(|(lo, hi)| inner <= hi && outer >= lo)
}

/// What a parent's [`LaplaceFrame`] is built from (P14.T56.a), read once at the parent's time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LaplaceFrameParts {
    /// The spin pole, along the galactic axes: its rotation frame's.
    pub spin_pole: [f64; 3],
    /// Its orbit's normal, along the galactic axes, of the orbit its rotation was derived on.
    pub orbit_normal: [f64; 3],
    /// Its hydrostatic J₂ ([`hydrostatic_j2`]), referred to `equatorial_radius`.
    pub j2: f64,
    /// Its equatorial radius, its figure's.
    pub equatorial_radius: Metres,
    /// Its orbit about its host then.
    pub parent_orbit: KeplerElements,
    /// Its mass then.
    pub parent_mass: Kilograms,
    /// The mass it orbits: its star's, or its pair's for a circumbinary parent.
    pub host_mass: Kilograms,
    /// Its system's age then, at which the moons' semi-major axes are read.
    pub read_at: Years,
}

/// A parent's satellites' frame (P14.T56.a): its spin pole and orbit normal, its own quadrupole
/// and its host's tide, read once at the parent's time, from which each moon's and ring's local
/// Laplace plane follows ([`LocalLaplace`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LaplaceFrame {
    spin_pole: [f64; 3],
    orbit_normal: [f64; 3],
    obliquity: Radians,
    j2: f64,
    equatorial_radius: Metres,
    tide: f64,
    parent_mass: Kilograms,
    read_at: Years,
}

impl LaplaceFrame {
    /// The frame `parts` describe.
    ///
    /// # Errors
    ///
    /// [`BuildLaplaceFrameError`] for an axis that is not a finite, non-zero vector, a J₂ that is
    /// negative or not finite, or a radius, a mass or an orbit's tide that is not positive and
    /// finite.
    pub(crate) fn new(parts: &LaplaceFrameParts) -> Result<Self, BuildLaplaceFrameError> {
        let axis = |v: [f64; 3]| {
            let length = norm(v);
            (length.is_finite() && length > 0.0)
                .then(|| unit(v))
                .ok_or(BuildLaplaceFrameError::Axis)
        };
        let (spin_pole, orbit_normal) = (axis(parts.spin_pole)?, axis(parts.orbit_normal)?);
        if !(parts.j2.is_finite() && parts.j2 >= 0.0) {
            return Err(BuildLaplaceFrameError::Quadrupole);
        }
        let positive = |x: f64| x.is_finite() && x > 0.0;
        if !(positive(parts.equatorial_radius.value())
            && positive(parts.parent_mass.value())
            && positive(parts.host_mass.value()))
        {
            return Err(BuildLaplaceFrameError::Size);
        }
        let tide = tide_factor(&parts.parent_orbit, parts.parent_mass, parts.host_mass);
        if !positive(tide) {
            return Err(BuildLaplaceFrameError::Size);
        }
        let cos = dot(spin_pole, orbit_normal);
        let obliquity = Radians::new(math::atan2(norm(cross(spin_pole, orbit_normal)), cos));
        Ok(Self {
            spin_pole,
            orbit_normal,
            obliquity,
            j2: parts.j2,
            equatorial_radius: parts.equatorial_radius,
            tide,
            parent_mass: parts.parent_mass,
            read_at: parts.read_at,
        })
    }

    /// The spin pole, a unit vector along the galactic axes.
    #[must_use]
    pub const fn spin_pole(&self) -> [f64; 3] {
        self.spin_pole
    }

    /// The orbit's normal, a unit vector along the galactic axes.
    #[must_use]
    pub const fn orbit_normal(&self) -> [f64; 3] {
        self.orbit_normal
    }

    /// The obliquity, rad, in [0, π]: the angle between the spin pole and the orbit's normal.
    #[must_use]
    pub const fn obliquity(&self) -> Radians {
        self.obliquity
    }

    /// The parent's own J₂, referred to [`equatorial_radius`](Self::equatorial_radius).
    #[must_use]
    pub const fn j2(&self) -> f64 {
        self.j2
    }

    /// The equatorial radius J₂ is referred to.
    #[must_use]
    pub const fn equatorial_radius(&self) -> Metres {
        self.equatorial_radius
    }

    /// The parent's mass, which the inner moons' masses are taken over.
    #[must_use]
    pub const fn parent_mass(&self) -> Kilograms {
        self.parent_mass
    }

    /// The system's age at which the frame and the moons' semi-major axes are read.
    #[must_use]
    pub const fn read_at(&self) -> Years {
        self.read_at
    }

    /// The Laplace radius of an orbit with the moons `inner` inside it ([`effective_j2r2`],
    /// [`laplace_radius`]).
    #[must_use]
    pub fn laplace_radius(&self, inner: impl IntoIterator<Item = InnerMoon>) -> Metres {
        radius_of(
            effective_j2r2(self.j2, self.equatorial_radius, inner),
            self.tide,
        )
    }

    /// The local Laplace plane of an orbit whose Laplace radius is `laplace_radius`, on the side
    /// `sense` gives.
    #[must_use]
    pub(crate) const fn local(&self, laplace_radius: Metres, sense: PoleSense) -> LocalLaplace {
        LocalLaplace {
            spin_pole: self.spin_pole,
            orbit_normal: self.orbit_normal,
            obliquity: self.obliquity,
            laplace_radius,
            sense,
        }
    }
}

/// [`LaplaceFrame::new`] could not build a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum BuildLaplaceFrameError {
    /// The spin pole or the orbit's normal was not a finite, non-zero vector.
    Axis,
    /// J₂ was negative or not finite.
    Quadrupole,
    /// The radius, a mass or the orbit's tide was not positive and finite.
    Size,
}

impl fmt::Display for BuildLaplaceFrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Axis => "a laplace frame's axes must be finite and non-zero",
            Self::Quadrupole => "a laplace frame's j2 must be finite and not negative",
            Self::Size => "a laplace frame's radius, masses and orbit must be positive and finite",
        })
    }
}

impl Error for BuildLaplaceFrameError {}

/// Which sense of its local Laplace plane's pole an orbit's elements are measured from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PoleSense {
    /// The spin's side ([`laplace_pole`]): a regular or giant-impact moon's, which forms
    /// prograde about its parent's spin, and a ring's.
    Spin,
    /// The orbit normal's side: a capture's, whose sense, prograde or retrograde, is drawn about
    /// its parent's orbit, as its stability limit reads it (P14.T15, P14.T19). It differs from
    /// [`Spin`](Self::Spin) only about a parent spinning retrograde.
    Orbit,
}

/// One orbit's local Laplace plane (P14.T56.a): its parent's axes, its own Laplace radius, and the
/// side its pole is taken on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocalLaplace {
    spin_pole: [f64; 3],
    orbit_normal: [f64; 3],
    obliquity: Radians,
    laplace_radius: Metres,
    sense: PoleSense,
}

impl LocalLaplace {
    /// The Laplace radius, with the moons inside the orbit counted.
    #[must_use]
    pub const fn laplace_radius(&self) -> Metres {
        self.laplace_radius
    }

    /// The side the pole is taken on.
    #[must_use]
    pub const fn sense(&self) -> PoleSense {
        self.sense
    }

    /// The parent's obliquity, rad, in [0, π].
    #[must_use]
    pub const fn obliquity(&self) -> Radians {
        self.obliquity
    }

    /// The plane's pole at semi-major axis `a`, a unit vector along the galactic axes:
    /// [`laplace_pole`], turned to the orbit normal's side for [`PoleSense::Orbit`].
    #[must_use]
    pub fn pole_at(&self, a: Metres) -> [f64; 3] {
        let pole = laplace_pole(self.spin_pole, self.orbit_normal, self.laplace_radius, a);
        match self.sense {
            PoleSense::Orbit if dot(pole, self.orbit_normal) < 0.0 => pole.map(|x| -x),
            PoleSense::Spin | PoleSense::Orbit => pole,
        }
    }

    /// Whether a circular orbit at `a` on the plane is unstable to eccentricity
    /// ([`eccentricity_unstable`]).
    #[must_use]
    pub fn unstable_at(&self, a: Metres) -> bool {
        eccentricity_unstable(self.obliquity, a.value() / self.laplace_radius.value())
    }
}

#[must_use]
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[must_use]
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[must_use]
fn norm(v: [f64; 3]) -> f64 {
    dot(v, v).sqrt()
}

#[must_use]
fn unit(v: [f64; 3]) -> [f64; 3] {
    let length = norm(v);
    [v[0] / length, v[1] / length, v[2] / length]
}

#[cfg(test)]
mod tests;
