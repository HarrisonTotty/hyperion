//! A placed planet's orbit: its eccentricity, its inclination to its host's plane and its angles,
//! and the host's plane itself (plan 14, P14.T8.d).
//!
//! # The laws
//!
//! - **Eccentricity**, from the group's [`EccentricityLaw`] (P14.T5), by inversion of one rank:
//!   Rayleigh with σ = 0.04 for cold compact chains, half-normal with σ = 0.3 for the dynamically
//!   hot variant, Rayleigh with σ = 0.05 for the Solar-like and terrestrial groups, and Kipping's
//!   (2013) Betas for giants. Each law is truncated at the planet's own limit, the largest
//!   eccentricity whose periapsis and apoapsis stay inside the zone and outside twice the host's
//!   Roche limit (see [`super`]), by drawing its rank inside the truncated range, so that the law's
//!   shape is kept and nothing piles up at the limit.
//! - **Inclination to the host's plane**: Rayleigh with σ = 1.5° for cold chains (Fabrycky et al.
//!   2014), otherwise σᵢ = σₑ ÷ 2 radians, where σₑ is the Rayleigh scale with the law's own
//!   mean square eccentricity (for a Rayleigh law its σ): the equipartition of random velocities
//!   that makes ⟨i²⟩ = ⟨e²⟩ ÷ 4.
//! - **The host's plane**: one isotropic direction per orbit host ([`PlaneDraws`]), or a given
//!   plane, a close binary's own, with no draw ([`HostPlane`]).
//! - **Angles**: the node on the host's plane, the argument of periapsis and the mean anomaly at
//!   the epoch are uniform.
//!
//! # Sources, re-checked
//!
//! - Xie et al. (2016, PNAS 113, 11431, abstract and SI eq. S1) find Kepler's multi-transiting
//!   systems "on nearly circular (ē = 0.04 (+0.03 −0.04)) orbits" and the singles at ē ≈ 0.3, both
//!   as the means of Rayleigh laws, so the multis' σ is ē ÷ √(π ÷ 2) ≈ 0.032 (0–0.056). Van Eylen
//!   et al. (2019, AJ 157, 61, §3.2.2 and Table 3) fit Rayleigh laws of σ = 0.061 (+0.010 −0.012)
//!   to multis and 0.24 ± 0.04 to singles, and half-Gaussians of σ = 0.083 and 0.32 ± 0.06. Plan
//!   14's 0.04 for cold chains lies inside Xie et al.'s range and below Van Eylen et al.'s, and its
//!   half-normal 0.3 for the hot variant is Van Eylen et al.'s singles' 0.32 ± 0.06. Both are
//!   [`template`](crate::planetary::architecture::template)'s constants.
//! - Fabrycky et al. (2014, ApJ 790, 146, §5.2 and Fig. 7): "The typical mutual inclination lies
//!   firmly in the range 1.0°–2.2°", as the width δ of a Rayleigh law, with a best fit of 1.8°.
//!   Plan 14's 1.5° ([`COLD_MUTUAL_INCLINATION_DEG`]) lies inside it.
//! - Kipping (2013, MNRAS 434, L51, Tables 1 and 2): Beta(0.867, 3.03) for 396 radial-velocity
//!   planets, and Beta(0.697, 3.27) and Beta(1.12, 3.09) inside and beyond the median period of
//!   382.3 days, as the templates have them.
//!
//! # Draws
//!
//! On [`tags::PLANET_ORBIT`], keyed by the planet's [`BodyId`](crate::id::BodyId): word 0 the
//! eccentricity's rank, word 1 the inclination's, and words 2, 3 and 4 the node, the argument of
//! periapsis and the mean anomaly, each uniform ([`OrbitDraws`]); words 5–7 are reserved. On
//! [`tags::PLANET_PLANE`], keyed by the system's ID, orbit host h reads words 4h and 4h + 1, the
//! cosine of its plane's inclination and its node ([`PLANE_WORDS_PER_HOST`]).

use core::f64::consts::{PI, SQRT_2, TAU};

use crate::id::SystemId;
use crate::math;
use crate::orbit::{BuildOrbitError, Orientation};
use crate::planetary::architecture::template::EccentricityLaw;
use crate::planetary::index::BodyIndex;
use crate::rng::{ObjectKey, Seed, Stream, tags};
use crate::stellar::draws::UnitUniform;
use crate::units::consts::RADIANS_PER_DEGREE;
use crate::units::{Days, Radians};

/// The width of the Rayleigh law of a cold chain's inclinations to its host's plane: 1.5°
/// (Fabrycky et al. 2014, §5.2: 1.0°–2.2°, best fit 1.8°).
pub const COLD_MUTUAL_INCLINATION_DEG: f64 = 1.5;

/// The largest eccentricity a planet is placed on: 0.99, a numerical guard well beyond Kipping's
/// (2013) Betas (under 10⁻⁶ of their mass lies above it) that keeps every orbit bound.
pub const ECCENTRICITY_CAP: f64 = 0.99;

/// Words of the [`tags::PLANET_PLANE`] stream that one orbit host owns: host h reads words 4h
/// (the cosine of its plane's inclination, as a uniform) and 4h + 1 (its node); 4h + 2 and 4h + 3
/// are reserved.
pub const PLANE_WORDS_PER_HOST: u64 = 4;

/// The continued fraction's fixed number of terms in [`regularised_incomplete_beta`]: 64.
///
/// Every shape the templates use converges in under 30 (Kipping's Betas have a + b ≤ 4.3), and
/// a fixed count makes every platform run the same operations.
const BETA_FRACTION_TERMS: u32 = 64;

/// Halley steps of [`inverse_regularised_incomplete_beta`]: 10, as Press et al. (2007) take at
/// most, from their approximate start.
const BETA_INVERSE_STEPS: u32 = 10;

/// The random variates of one planet's orbit, as drawn from [`tags::PLANET_ORBIT`] by
/// [`OrbitDraws::for_planet`], or given explicitly by a test or a tool.
///
/// The fields are plain variates with no invariant between them, so they are public, as the
/// disc's [`DiscDraws`](crate::planetary::disc::DiscDraws) are.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrbitDraws {
    /// The eccentricity's rank in its law. Word 0.
    pub eccentricity: UnitUniform,
    /// The inclination's rank in its law. Word 1.
    pub inclination: UnitUniform,
    /// The ascending node on the host's plane, rad, in `[0, 2π)`. Word 2.
    pub node: Radians,
    /// The argument of periapsis from that node, rad, in `[0, 2π)`. Word 3.
    pub periapsis: Radians,
    /// The mean anomaly at the epoch, rad, in `[0, 2π)`. Word 4.
    pub mean_anomaly: Radians,
}

impl OrbitDraws {
    /// Every rank at its median and every angle zero.
    pub const MEDIAN: Self = Self {
        eccentricity: UnitUniform::HALF,
        inclination: UnitUniform::HALF,
        node: Radians::ZERO,
        periapsis: Radians::ZERO,
        mean_anomaly: Radians::ZERO,
    };

    /// The draws of `planet` of `system`, in the universe of `seed`: words 0–4 of the planet's
    /// own [`tags::PLANET_ORBIT`] stream.
    ///
    /// # Panics
    ///
    /// Never: an open uniform is a rank.
    #[must_use]
    pub fn for_planet(seed: Seed, system: SystemId, planet: BodyIndex) -> Self {
        let mut stream = Stream::open(
            seed,
            tags::PLANET_ORBIT,
            ObjectKey::from(planet.body_id(system)),
        );
        let mut rank = || {
            UnitUniform::new(stream.uniform_open())
                .expect("an open uniform lies strictly between 0 and 1")
        };
        let eccentricity = rank();
        let inclination = rank();
        let mut angle = || Radians::new(TAU * stream.uniform());
        Self {
            eccentricity,
            inclination,
            node: angle(),
            periapsis: angle(),
            mean_anomaly: angle(),
        }
    }
}

/// A plane through an orbit host, by its pole: the inclination of its normal to galactic north and
/// the longitude of its ascending node on the galactic x–y plane, in the orbit module's
/// conventions ([`Orientation`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SystemPlane {
    inclination: Radians,
    node: Radians,
}

impl SystemPlane {
    /// The plane of inclination `inclination` and node `node`.
    ///
    /// # Errors
    ///
    /// As [`Orientation::new`]: an inclination outside `[0, π]` or a node that is not finite.
    pub fn new(inclination: Radians, node: Radians) -> Result<Self, BuildOrbitError> {
        let checked = Orientation::new(inclination, node, Radians::ZERO)?;
        Ok(Self {
            inclination: checked.inclination(),
            node: checked.ascending_node(),
        })
    }

    /// The plane of an orbit: a close binary's, which its circumbinary planets share
    /// (P14.T8.d).
    #[must_use]
    pub const fn of_orbit(orientation: &Orientation) -> Self {
        Self {
            inclination: orientation.inclination(),
            node: orientation.ascending_node(),
        }
    }

    /// The inclination of the plane's normal to galactic north, rad, in `[0, π]`.
    #[must_use]
    pub const fn inclination(&self) -> Radians {
        self.inclination
    }

    /// The longitude of the plane's ascending node, rad, in `[0, 2π)`.
    #[must_use]
    pub const fn node(&self) -> Radians {
        self.node
    }

    /// The plane's frame: the unit vector along its ascending node, the one a quarter-turn ahead
    /// in the plane, and its normal, along the system frame's axes.
    #[must_use]
    pub fn frame(&self) -> [[f64; 3]; 3] {
        let (sin_i, cos_i) = math::sin_cos(self.inclination.value());
        let (sin_node, cos_node) = math::sin_cos(self.node.value());
        [
            [cos_node, sin_node, 0.0],
            [-sin_node * cos_i, cos_node * cos_i, sin_i],
            [sin_i * sin_node, -sin_i * cos_node, cos_i],
        ]
    }
}

/// An orbit host's plane as the placer takes it (P14.T8.d).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HostPlane {
    /// One isotropic draw on [`tags::PLANET_PLANE`] at the host's number ([`PlaneDraws`]).
    Isotropic,
    /// A given plane, with no draw: a close binary's, for its circumbinary zone.
    Aligned(SystemPlane),
}

/// An orbit host's isotropic plane, as drawn from [`tags::PLANET_PLANE`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlaneDraws {
    /// The cosine of the inclination, uniform in `(−1, 1)`. Word 4h.
    pub cos_inclination: f64,
    /// The node, rad, in `[0, 2π)`. Word 4h + 1.
    pub node: Radians,
}

impl PlaneDraws {
    /// The draws of orbit host number `host` of `system`, in the universe of `seed`.
    #[must_use]
    pub fn for_host(seed: Seed, system: SystemId, host: u8) -> Self {
        let mut stream = Stream::open(seed, tags::PLANET_PLANE, ObjectKey::from(system));
        stream.seek(u64::from(host) * PLANE_WORDS_PER_HOST);
        let cos_inclination = 1.0 - 2.0 * stream.uniform_open();
        let node = Radians::new(TAU * stream.uniform());
        Self {
            cos_inclination,
            node,
        }
    }

    /// The plane these draws give.
    #[must_use]
    pub fn plane(&self) -> SystemPlane {
        SystemPlane {
            inclination: Radians::new(math::acos(self.cos_inclination.clamp(-1.0, 1.0))),
            node: self.node,
        }
    }
}

/// The plane of host `host` of `system` under `plane`: the drawn one, or the one given.
#[must_use]
pub fn host_plane(seed: Seed, system: SystemId, host: u8, plane: HostPlane) -> SystemPlane {
    match plane {
        HostPlane::Isotropic => PlaneDraws::for_host(seed, system, host).plane(),
        HostPlane::Aligned(plane) => plane,
    }
}

/// One of the three shapes of eccentricity law, once a Kipping split has been resolved by period.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Shape {
    Rayleigh(f64),
    HalfNormal(f64),
    Beta(f64, f64),
}

/// The shape `law` takes for a body of period `period`.
#[must_use]
fn shape(law: EccentricityLaw, period: Days) -> Shape {
    match law {
        EccentricityLaw::Rayleigh { sigma } => Shape::Rayleigh(sigma),
        EccentricityLaw::HalfNormal { sigma } => Shape::HalfNormal(sigma),
        EccentricityLaw::Beta { a, b } => Shape::Beta(a, b),
        EccentricityLaw::BetaByPeriod {
            split,
            short_a,
            short_b,
            long_a,
            long_b,
        } => {
            if period < split {
                Shape::Beta(short_a, short_b)
            } else {
                Shape::Beta(long_a, long_b)
            }
        }
    }
}

impl Shape {
    /// The cumulative distribution at `e`.
    fn cdf(self, e: f64) -> f64 {
        match self {
            Self::Rayleigh(sigma) => -math::exp_m1(-(e * e) / (2.0 * sigma * sigma)),
            Self::HalfNormal(sigma) => math::erf(e / (sigma * SQRT_2)),
            Self::Beta(a, b) => regularised_incomplete_beta(a, b, e),
        }
    }

    /// The eccentricity at cumulative probability `p`, 0 < p < 1.
    fn quantile(self, p: f64) -> f64 {
        match self {
            Self::Rayleigh(sigma) => sigma * (-2.0 * math::ln_1p(-p)).sqrt(),
            Self::HalfNormal(sigma) => sigma * math::normal_quantile(0.5 + 0.5 * p),
            Self::Beta(a, b) => inverse_regularised_incomplete_beta(a, b, p),
        }
    }

    /// The Rayleigh scale with the same mean square: σ for a Rayleigh law, whose ⟨e²⟩ is 2σ².
    fn rayleigh_scale(self) -> f64 {
        match self {
            Self::Rayleigh(sigma) => sigma,
            // A half-normal law of scale σ has ⟨e²⟩ = σ².
            Self::HalfNormal(sigma) => sigma / SQRT_2,
            Self::Beta(a, b) => {
                let mean_square = a * (a + 1.0) / ((a + b) * (a + b + 1.0));
                (mean_square / 2.0).sqrt()
            }
        }
    }
}

/// The eccentricity at rank `rank` of `law` for a body of period `period`, truncated at `limit`:
/// the quantile of `rank` × F(`limit`), so that the law keeps its shape below the limit
/// (P14.T8.d). A limit of zero or less gives a circular orbit.
///
/// # Examples
///
/// Kipping's (2013) Beta for radial-velocity planets has its median near 0.18; truncated at 0.1,
/// the same rank gives a smaller eccentricity, still below the limit.
///
/// ```
/// use hyperion_sim::planetary::architecture::template::EccentricityLaw;
/// use hyperion_sim::planetary::placement::classes::orbits::eccentricity;
/// use hyperion_sim::stellar::draws::UnitUniform;
/// use hyperion_sim::units::Days;
///
/// let kipping = EccentricityLaw::Beta { a: 0.867, b: 3.03 };
/// let median = UnitUniform::HALF;
/// let e = eccentricity(kipping, median, Days::new(300.0), 0.99);
/// assert!((0.17..0.19).contains(&e));
/// let held = eccentricity(kipping, median, Days::new(300.0), 0.1);
/// assert!(held < 0.1 && held < e);
/// ```
#[must_use]
pub fn eccentricity(law: EccentricityLaw, rank: UnitUniform, period: Days, limit: f64) -> f64 {
    let limit = limit.min(ECCENTRICITY_CAP);
    if limit <= 0.0 {
        return 0.0;
    }
    let shape = shape(law, period);
    let p = rank.value() * shape.cdf(limit);
    if p <= 0.0 {
        return 0.0;
    }
    shape.quantile(p).clamp(0.0, limit)
}

/// The largest eccentricity `rank` can give under `law` at any period and any limit: the
/// untruncated quantile, of the larger of a Kipping split's two Betas, held to
/// [`ECCENTRICITY_CAP`]. The placer spaces a planet for this before its period is known.
#[must_use]
pub fn upper_eccentricity(law: EccentricityLaw, rank: UnitUniform) -> f64 {
    let at = |period: f64| eccentricity(law, rank, Days::new(period), ECCENTRICITY_CAP);
    match law {
        EccentricityLaw::BetaByPeriod { split, .. } => {
            let short = at(split.value() * 0.5);
            let long = at(split.value() * 2.0);
            short.max(long)
        }
        EccentricityLaw::Rayleigh { .. }
        | EccentricityLaw::HalfNormal { .. }
        | EccentricityLaw::Beta { .. } => at(1.0),
    }
}

/// The width σᵢ of the Rayleigh law of a planet's inclination to its host's plane, rad: 1.5°
/// for a cold chain (Fabrycky et al. 2014), and otherwise half the Rayleigh scale with the same
/// mean square eccentricity as `law` at period `period` (P14.T8.d).
#[must_use]
pub fn inclination_width(law: EccentricityLaw, period: Days, cold_chain: bool) -> f64 {
    if cold_chain {
        COLD_MUTUAL_INCLINATION_DEG * RADIANS_PER_DEGREE
    } else {
        shape(law, period).rayleigh_scale() / 2.0
    }
}

/// The inclination to its host's plane at rank `rank` of a Rayleigh law of width `width` rad,
/// truncated at π.
#[must_use]
pub fn mutual_inclination(width: f64, rank: UnitUniform) -> Radians {
    if width <= 0.0 {
        return Radians::ZERO;
    }
    let law = Shape::Rayleigh(width);
    let p = rank.value() * law.cdf(PI);
    Radians::new(law.quantile(p).clamp(0.0, PI))
}

/// The orientation, in the system frame, of an orbit inclined by `inclination` to `plane`, with
/// its ascending node on that plane at `node` from the plane's own node and its periapsis
/// `periapsis` beyond it.
///
/// # Errors
///
/// As [`Orientation::new`], which cannot fail for finite angles: the inclination it is given
/// comes from an arccosine, in `[0, π]`.
pub fn orientation_in_plane(
    plane: &SystemPlane,
    inclination: Radians,
    node: Radians,
    periapsis: Radians,
) -> Result<Orientation, BuildOrbitError> {
    let local = Orientation::new(inclination, node, periapsis)?;
    let [towards, beyond, pole] = plane.frame();
    let to_system = |x: [f64; 3]| {
        [
            x[0] * towards[0] + x[1] * beyond[0] + x[2] * pole[0],
            x[0] * towards[1] + x[1] * beyond[1] + x[2] * pole[1],
            x[0] * towards[2] + x[1] * beyond[2] + x[2] * pole[2],
        ]
    };
    let apse = to_system(local.periapsis_direction());
    let normal = to_system(local.normal());
    let sin_i = math::hypot(normal[0], normal[1]);
    let tilt = math::atan2(sin_i, normal[2]);
    if sin_i > 1e-12 {
        let node = math::atan2(normal[0], -normal[1]);
        let (sin_node, cos_node) = math::sin_cos(node);
        // The normal crossed with the node's direction: a quarter-turn past the node in the
        // direction of motion.
        let ahead = [
            -normal[2] * sin_node,
            normal[2] * cos_node,
            normal[0] * sin_node - normal[1] * cos_node,
        ];
        let along = apse[0] * cos_node + apse[1] * sin_node;
        let across = apse[0] * ahead[0] + apse[1] * ahead[1] + apse[2] * ahead[2];
        Orientation::new(
            Radians::new(tilt),
            Radians::new(node),
            Radians::new(math::atan2(across, along)),
        )
    } else if normal[2] > 0.0 {
        // In the galactic plane, prograde: only the node plus the argument places the periapsis.
        Orientation::new(
            Radians::ZERO,
            Radians::ZERO,
            Radians::new(math::atan2(apse[1], apse[0])),
        )
    } else {
        Orientation::new(
            Radians::new(PI),
            Radians::ZERO,
            Radians::new(math::atan2(-apse[1], apse[0])),
        )
    }
}

/// The regularised incomplete beta function Iₓ(a, b), the Beta law's cumulative distribution,
/// by Lentz's continued fraction on a fixed number of terms (Press et al. 2007, Numerical Recipes,
/// 3rd ed., §6.4), using the symmetry Iₓ(a, b) = 1 − I₁₋ₓ(b, a) where the fraction converges
/// slowly.
///
/// # Panics
///
/// In debug builds, unless `a` and `b` are positive.
#[must_use]
pub fn regularised_incomplete_beta(a: f64, b: f64, x: f64) -> f64 {
    debug_assert!(a > 0.0 && b > 0.0, "Beta shapes are positive, got {a}, {b}");
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let ln_front = a * math::ln(x) + b * math::ln_1p(-x) - ln_beta(a, b);
    if x < (a + 1.0) / (a + b + 2.0) {
        math::exp(ln_front) * beta_fraction(a, b, x) / a
    } else {
        1.0 - math::exp(ln_front) * beta_fraction(b, a, 1.0 - x) / b
    }
}

/// ln B(a, b) = ln Γ(a) + ln Γ(b) − ln Γ(a + b).
#[must_use]
fn ln_beta(a: f64, b: f64) -> f64 {
    math::ln_gamma(a) + math::ln_gamma(b) - math::ln_gamma(a + b)
}

/// The continued fraction of the incomplete beta function, by the modified Lentz method on
/// [`BETA_FRACTION_TERMS`] terms.
#[must_use]
fn beta_fraction(a: f64, b: f64, x: f64) -> f64 {
    const TINY: f64 = 1e-300;
    let guard = |value: f64| if value.abs() < TINY { TINY } else { value };
    let (sum, plus, minus) = (a + b, a + 1.0, a - 1.0);
    let mut numerator = 1.0;
    let mut denominator = 1.0 / guard(1.0 - sum * x / plus);
    let mut fraction = denominator;
    for term in 1..=BETA_FRACTION_TERMS {
        let term = f64::from(term);
        let twice = 2.0 * term;
        let even = term * (b - term) * x / ((minus + twice) * (a + twice));
        denominator = 1.0 / guard(1.0 + even * denominator);
        numerator = guard(1.0 + even / numerator);
        fraction *= denominator * numerator;
        let odd = -(a + term) * (sum + term) * x / ((a + twice) * (plus + twice));
        denominator = 1.0 / guard(1.0 + odd * denominator);
        numerator = guard(1.0 + odd / numerator);
        fraction *= denominator * numerator;
    }
    fraction
}

/// The x at which Iₓ(a, b) = `p`: Press et al.'s (2007, Numerical Recipes, 3rd ed., §6.14.11)
/// `invbetai`, an approximate start refined by [`BETA_INVERSE_STEPS`] Halley steps, each kept
/// inside `(0, 1)`.
///
/// # Panics
///
/// In debug builds, unless `a` and `b` are positive.
#[must_use]
pub fn inverse_regularised_incomplete_beta(a: f64, b: f64, p: f64) -> f64 {
    debug_assert!(a > 0.0 && b > 0.0, "Beta shapes are positive, got {a}, {b}");
    if p <= 0.0 {
        return 0.0;
    }
    if p >= 1.0 {
        return 1.0;
    }
    let mut x = if a >= 1.0 && b >= 1.0 {
        let tail = if p < 0.5 { p } else { 1.0 - p };
        let root = (-2.0 * math::ln(tail)).sqrt();
        let mut normal =
            (2.307_53 + root * 0.270_61) / (1.0 + root * (0.992_29 + root * 0.044_81)) - root;
        if p < 0.5 {
            normal = -normal;
        }
        let lambda = (normal * normal - 3.0) / 6.0;
        let harmonic = 2.0 / (1.0 / (2.0 * a - 1.0) + 1.0 / (2.0 * b - 1.0));
        let width = (normal * (lambda + harmonic).sqrt() / harmonic)
            - (1.0 / (2.0 * b - 1.0) - 1.0 / (2.0 * a - 1.0))
                * (lambda + 5.0 / 6.0 - 2.0 / (3.0 * harmonic));
        a / (a + b * math::exp(2.0 * width))
    } else {
        let lower = math::exp(a * math::ln(a / (a + b))) / a;
        let upper = math::exp(b * math::ln(b / (a + b))) / b;
        let total = lower + upper;
        if p < lower / total {
            math::powf(a * total * p, 1.0 / a)
        } else {
            1.0 - math::powf(b * total * (1.0 - p), 1.0 / b)
        }
    };
    let ln_b = ln_beta(a, b);
    for _ in 0..BETA_INVERSE_STEPS {
        if x <= 0.0 || x >= 1.0 {
            break;
        }
        let miss = regularised_incomplete_beta(a, b, x) - p;
        let density = math::exp((a - 1.0) * math::ln(x) + (b - 1.0) * math::ln_1p(-x) - ln_b);
        let newton = miss / density;
        let step =
            newton / (1.0 - 0.5 * (newton * ((a - 1.0) / x - (b - 1.0) / (1.0 - x))).min(1.0));
        let next = x - step;
        x = if next <= 0.0 {
            0.5 * x
        } else if next >= 1.0 {
            f64::midpoint(x, 1.0)
        } else {
            next
        };
    }
    x
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::order::assert_order_independent;
    use hyperion_testkit::stats::{ALPHA, assert_p_value, ks_one_sample};

    use super::*;
    use crate::coords::{CellSize, GenCell};
    use crate::id::Layer;
    use crate::planetary::index::{BodySlot, BodySub};

    fn system(index: u32) -> SystemId {
        let cell = GenCell::new(CellSize::Ly8, [4, -2, 1]).unwrap();
        SystemId::from_parts(Layer::A, cell, index).unwrap()
    }

    fn planet(slot: u8) -> BodyIndex {
        BodyIndex::new(BodySlot::Planet(slot), BodySub::Primary).unwrap()
    }

    fn rank(u: f64) -> UnitUniform {
        UnitUniform::new(u).unwrap()
    }

    #[test]
    fn the_incomplete_beta_function_meets_its_closed_forms() {
        for x in [1e-6, 0.01, 0.2, 0.5, 0.77, 0.999] {
            // I_x(1, b) = 1 − (1 − x)^b and I_x(a, 1) = x^a.
            for s in [0.7, 1.0, 2.5, 3.27] {
                let one_b = 1.0 - math::powf(1.0 - x, s);
                assert!((regularised_incomplete_beta(1.0, s, x) - one_b).abs() < 1e-13);
                let a_one = math::powf(x, s);
                assert!((regularised_incomplete_beta(s, 1.0, x) - a_one).abs() < 1e-13);
            }
            // I_x(2, 2) = x² (3 − 2x).
            let two_two = x * x * (3.0 - 2.0 * x);
            assert!((regularised_incomplete_beta(2.0, 2.0, x) - two_two).abs() < 1e-13);
        }
    }

    /// Simpson's rule on the Beta density, with the singular end integrated in closed form.
    fn numerical_beta_cdf(a: f64, b: f64, x: f64) -> f64 {
        // With s = t^a, ∫₀ˣ t^(a−1) (1 − t)^(b−1) dt = (1 ÷ a) ∫₀^(x^a) (1 − s^(1/a))^(b−1) ds,
        // whose integrand is smooth.
        let panels = 20_000_u32;
        let end = math::powf(x, a);
        let width = end / f64::from(panels);
        let smooth = |s: f64| math::powf(1.0 - math::powf(s, 1.0 / a), b - 1.0);
        let mut sum = smooth(0.0) + smooth(end);
        for k in 1..panels {
            sum += if k % 2 == 1 { 4.0 } else { 2.0 } * smooth(width * f64::from(k));
        }
        sum * width / 3.0 / a / math::exp(ln_beta(a, b))
    }

    #[test]
    fn kipping_s_betas_match_the_integrated_density() {
        for (a, b) in [(0.867, 3.03), (0.697, 3.27), (1.12, 3.09)] {
            for x in [0.02, 0.1, 0.3, 0.6, 0.9] {
                let closed = regularised_incomplete_beta(a, b, x);
                let numeric = numerical_beta_cdf(a, b, x);
                assert!(
                    (closed - numeric).abs() < 1e-9,
                    "{a}, {b} at {x}: {closed} {numeric}"
                );
            }
        }
    }

    #[test]
    fn the_inverse_returns_the_rank_it_was_given() {
        for (a, b) in [(0.867, 3.03), (0.697, 3.27), (1.12, 3.09), (2.0, 5.0)] {
            for p in [1e-9, 1e-4, 0.05, 0.3, 0.5, 0.8, 0.99, 1.0 - 1e-9] {
                let x = inverse_regularised_incomplete_beta(a, b, p);
                let back = regularised_incomplete_beta(a, b, x);
                assert!(
                    (back - p).abs() < 1e-12 * p.max(1e-3),
                    "{a}, {b} at {p}: x {x}, back {back}"
                );
            }
        }
    }

    #[test]
    fn eccentricities_follow_their_laws() {
        let laws = [
            EccentricityLaw::Rayleigh { sigma: 0.04 },
            EccentricityLaw::HalfNormal { sigma: 0.3 },
            EccentricityLaw::Beta { a: 0.867, b: 3.03 },
        ];
        for law in laws {
            let shape = shape(law, Days::new(10.0));
            let mut samples: Vec<f64> = (0..20_000)
                .map(|i| {
                    let draws = OrbitDraws::for_planet(Seed::new(5), system(i), planet(1));
                    eccentricity(law, draws.eccentricity, Days::new(10.0), ECCENTRICITY_CAP)
                })
                .collect();
            let cap = shape.cdf(ECCENTRICITY_CAP);
            let ks = ks_one_sample(&mut samples, |e| shape.cdf(e) / cap);
            assert_p_value(&format!("{law:?}"), ks.p_value, ALPHA);
        }
    }

    #[test]
    fn a_truncated_law_keeps_its_shape_below_the_limit() {
        let law = EccentricityLaw::HalfNormal { sigma: 0.3 };
        let limit = 0.2;
        let shape = shape(law, Days::new(10.0));
        let mut samples: Vec<f64> = (0..20_000)
            .map(|i| {
                let draws = OrbitDraws::for_planet(Seed::new(6), system(i), planet(3));
                eccentricity(law, draws.eccentricity, Days::new(10.0), limit)
            })
            .collect();
        assert!(samples.iter().all(|&e| (0.0..=limit).contains(&e)));
        let ks = ks_one_sample(&mut samples, |e| shape.cdf(e) / shape.cdf(limit));
        assert_p_value("truncated half-normal", ks.p_value, ALPHA);
        assert_same_bits(eccentricity(law, rank(0.7), Days::new(1.0), 0.0), 0.0);
    }

    #[test]
    fn kipping_s_split_follows_the_period() {
        let law = EccentricityLaw::BetaByPeriod {
            split: Days::new(382.3),
            short_a: 0.697,
            short_b: 3.27,
            long_a: 1.12,
            long_b: 3.09,
        };
        let r = rank(0.6);
        let short = eccentricity(law, r, Days::new(100.0), ECCENTRICITY_CAP);
        let long = eccentricity(law, r, Days::new(1_000.0), ECCENTRICITY_CAP);
        let truncated = |a: f64, b: f64| {
            let p = 0.6 * regularised_incomplete_beta(a, b, ECCENTRICITY_CAP);
            inverse_regularised_incomplete_beta(a, b, p)
        };
        let (expect_short, expect_long) = (truncated(0.697, 3.27), truncated(1.12, 3.09));
        assert!((short - expect_short).abs() < 1e-15 && (long - expect_long).abs() < 1e-15);
        assert!((upper_eccentricity(law, r) - short.max(long)).abs() < 1e-15);
    }

    #[test]
    fn inclinations_are_rayleigh_with_the_equipartition_width() {
        let cold = inclination_width(
            EccentricityLaw::Rayleigh { sigma: 0.04 },
            Days::new(10.0),
            true,
        );
        assert!((cold - 1.5 * PI / 180.0).abs() < 1e-15);
        let warm = inclination_width(
            EccentricityLaw::Rayleigh { sigma: 0.05 },
            Days::new(10.0),
            false,
        );
        assert!((warm - 0.025).abs() < 1e-15);
        // A half-normal law of scale 0.3 has the mean square of a Rayleigh law of 0.3 ÷ √2.
        let hot = inclination_width(
            EccentricityLaw::HalfNormal { sigma: 0.3 },
            Days::new(10.0),
            false,
        );
        assert!((hot - 0.3 / SQRT_2 / 2.0).abs() < 1e-15);
        let mut samples: Vec<f64> = (0..20_000)
            .map(|i| {
                let draws = OrbitDraws::for_planet(Seed::new(7), system(i), planet(2));
                mutual_inclination(cold, draws.inclination).value()
            })
            .collect();
        let law = Shape::Rayleigh(cold);
        let ks = ks_one_sample(&mut samples, |i| law.cdf(i));
        assert_p_value("cold inclinations", ks.p_value, ALPHA);
    }

    #[test]
    fn isotropic_planes_have_uniform_cosines_and_nodes() {
        let (mut cosines, mut nodes): (Vec<f64>, Vec<f64>) = (0..20_000)
            .map(|i| {
                let draws = PlaneDraws::for_host(Seed::new(8), system(i), 0);
                (draws.cos_inclination, draws.node.value())
            })
            .unzip();
        let ks = ks_one_sample(&mut cosines, |c| f64::midpoint(c, 1.0));
        assert_p_value("cos i", ks.p_value, ALPHA);
        let ks = ks_one_sample(&mut nodes, |n| n / TAU);
        assert_p_value("node", ks.p_value, ALPHA);
        // Hosts read their own words.
        let host = |h: u8| PlaneDraws::for_host(Seed::new(8), system(1), h);
        assert_ne!(host(0), host(1));
        let mut stream = Stream::open(Seed::new(8), tags::PLANET_PLANE, ObjectKey::from(system(1)));
        stream.seek(3 * PLANE_WORDS_PER_HOST);
        assert!((host(3).cos_inclination - (1.0 - 2.0 * stream.uniform_open())).abs() < 1e-300);
    }

    fn close(a: [f64; 3], b: [f64; 3]) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-12)
    }

    #[test]
    fn an_orbit_in_a_tilted_plane_keeps_its_inclination_to_that_plane() {
        let plane = SystemPlane::new(Radians::new(1.1), Radians::new(2.3)).unwrap();
        let [_, _, pole] = plane.frame();
        for (i, node, argument) in [
            (0.0, 0.0, 0.0),
            (0.3, 1.0, 2.0),
            (0.05, 4.0, 5.5),
            (2.9, 0.2, 0.1),
        ] {
            let orientation = orientation_in_plane(
                &plane,
                Radians::new(i),
                Radians::new(node),
                Radians::new(argument),
            )
            .unwrap();
            let normal = orientation.normal();
            let cos = normal[0] * pole[0] + normal[1] * pole[1] + normal[2] * pole[2];
            assert!((math::acos(cos.clamp(-1.0, 1.0)) - i).abs() < 1e-10, "{i}");
            // The periapsis is the local one carried into the system frame.
            let local =
                Orientation::new(Radians::new(i), Radians::new(node), Radians::new(argument))
                    .unwrap()
                    .periapsis_direction();
            let [towards, beyond, up] = plane.frame();
            let expected =
                [0, 1, 2].map(|k| local[0] * towards[k] + local[1] * beyond[k] + local[2] * up[k]);
            assert!(close(orientation.periapsis_direction(), expected), "{i}");
        }
        // A plane at the galactic plane leaves an orbit's angles as they are.
        let flat = SystemPlane::new(Radians::ZERO, Radians::ZERO).unwrap();
        let o = orientation_in_plane(
            &flat,
            Radians::new(0.4),
            Radians::new(1.0),
            Radians::new(2.0),
        )
        .unwrap();
        assert!((o.inclination().value() - 0.4).abs() < 1e-14);
        assert!((o.ascending_node().value() - 1.0).abs() < 1e-14);
        assert!((o.argument_of_periapsis().value() - 2.0).abs() < 1e-13);
        // An orbit in the plane of a binary shares the binary's pole.
        let binary =
            Orientation::new(Radians::new(0.8), Radians::new(3.0), Radians::new(1.0)).unwrap();
        let shared = SystemPlane::of_orbit(&binary);
        let o = orientation_in_plane(&shared, Radians::ZERO, Radians::new(1.5), Radians::new(0.5))
            .unwrap();
        assert!(close(o.normal(), binary.normal()));
    }

    #[test]
    fn the_same_planet_draws_the_same_orbit_in_any_order() {
        let draw = |i: &u32| OrbitDraws::for_planet(Seed::new(9), system(*i), planet(4));
        assert_order_independent(&[1, 2, 3], draw);
        assert_ne!(
            OrbitDraws::for_planet(Seed::new(9), system(1), planet(4)),
            OrbitDraws::for_planet(Seed::new(9), system(1), planet(5))
        );
    }
}
