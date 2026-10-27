//! A belt part's small bodies as a population (plan 14, P14.T48.g; science-r09-craters.md F5.6):
//! the laws their orbits follow, the record's moments of those laws, and Öpik's mean encounter
//! speed between the part's crossers and a body at a semi-major axis, which the impactors read
//! (P14.T48.f's `U_b`², [`PopulationLaw::encounter_excitation`]).
//!
//! # The laws
//!
//! A part's small bodies follow its members' laws (P14.T21.c), with the population's own bounds:
//!
//! - **The semi-major axis** lies as the disc's solids lie over the annulus ([`AxisSpread`]):
//!   e^(−a ÷ `r_c`) per unit radius on either side of the snow line, each side holding its share of
//!   the solids.
//! - **The eccentricity** is Rayleigh in the part's width σₑ, truncated where the orbit would reach
//!   a planet's chaotic zone ([`EccentricityLaw::TruncatedRayleigh`]): its pericentre beyond the
//!   floor, the farthest reach of the zones of the planets inside the annulus or the disc's inner
//!   edge, and its apocentre inside the ceiling, the nearest zone beyond it, held to
//!   [`MEMBER_ECCENTRICITY_CAP`]. The members'
//!   own cap at the disc's outer edge is a placement guard, not a bound on the population, so the
//!   ceiling is a planet's alone.
//! - **A scattered component** beyond a giant is perihelion-anchored instead
//!   ([`EccentricityLaw::PerihelionAnchored`]): its perihelia lie uniform from `q_z` to
//!   [`PERIHELION_SPREAD`] × `q_z`, with `q_z` its floor, the outer edge of the outermost planet's
//!   chaotic zone, and e = 1 − q ÷ a. Tested against JPL's Small-Body Database (the `TNO` class,
//!   fetched 2026-10-26), it is the better law (decision-p14-t48g-opik §7): the scattered disc's
//!   median perihelion is flat at 35.1–36.0 au in every bin of a from Neptune's 2:1 at 47.7 au to
//!   150 au (2,390 bodies), where T21.c's Rayleigh law of width 0.3 puts it at 39–92 au; between
//!   47.7 and 100 au (2,146 bodies) its mean e is 0.399 against the observed 0.416 (the Rayleigh's
//!   0.263), and the Kolmogorov–Smirnov distance of the bodies' e at their a 0.244 (the
//!   Rayleigh's 0.459). The share of e in 0.2–0.6 alone favours the Rayleigh (0.678 and 0.933
//!   against the observed 0.778). A scattered component beyond a planet that is not a giant
//!   (`BELT_GIANT_MASS`) keeps the Rayleigh law, since only a giant scatters a disc.
//! - **The inclination** to the host's plane is Rayleigh in σᵢ, untruncated.
//!
//! The members keep T21.c's placement: only the population's moments and encounters read these
//! laws.
//!
//! # The moments
//!
//! The record's mean eccentricity of a part is its law's mean over the annulus, by a fixed-node
//! quadrature (Gauss–Legendre, 16 nodes on each piece of the annulus between the snow line and the
//! law's kinks, summed in order, with no draws): the truncated Rayleigh's mean at each a is
//! (σ√(π ÷ 2) erf(L ÷ σ√2) − L e^(−L² ÷ 2σ²)) ÷ (1 − e^(−L² ÷ 2σ²)) at the limit L, in place of
//! the untruncated σ√(π ÷ 2). The mean inclination, untruncated, stays σᵢ√(π ÷ 2).
//!
//! # The encounter speed
//!
//! `U_b`²(a) is the mean of Öpik's U² = 3 − 1 ÷ A − 2√(A(1 − e²)) cos i, with A = `a_p` ÷ a, in
//! units of the circular speed at a, over the population's orbits that cross a (q ≤ a ≤ Q),
//! weighted by Öpik's collision probability per unit time, τ²U ÷ (π sin i |`U_x`| `T_p`), with
//! `U_x`² = A (e² − (1 − 1 ÷ A)²) the radial part and `T_p` ∝ `a_p`^1.5 the crosser's period (Öpik
//! 1951, Proc. R. Irish Acad. 54A, 165; Wetherill 1967, J. Geophys. Res. 72, 2429; Bottke et al.
//! 1994, Icarus 107, 255). The singular 1 ÷ |`U_x`| at a tangent orbit is taken out by e² =
//! `e_min`² + s², and the nodes are Gauss–Legendre's. Without the weighting
//! (`Weighting::Orbits`), at equal semi-major axes and over untruncated laws it gives
//! science-r09-craters.md F5.6's 0.0586 and 0.408; Öpik's weighting gives 0.0453 and 0.3205 there
//! (decision-p14-t48g-opik).
//!
//! # The hold
//!
//! Below `BELT_POPULATION_VERSION` (24) the record's moments and the impactors' excitations are
//! version 23's bit for bit (`PopulationRules::Held`); the laws are built whatever the version,
//! and read only under `PopulationRules::Laws`.

use core::f64::consts::{PI, SQRT_2};
use core::fmt;
use std::error::Error;

use super::{BeltPart, MEMBER_ECCENTRICITY_CAP, widths};
use crate::math;
use crate::tables::gauss_legendre::{GL16_NODES, GL16_WEIGHTS};
use crate::units::{Metres, Radians};
use crate::{GENERATOR_VERSION, GeneratorVersion};

#[cfg(test)]
mod tests;

/// The generator version from which a belt's recorded moments and its impactors' encounter speeds
/// follow its population's laws (P14.T48.g): 24, a rider of the 23 → 24 batch (P14.T54.b) that
/// boards if it lands before P14.T54.b.i's wiring, and takes the next bump, its hold renumbered,
/// otherwise (decision-bump-plan-after-22).
///
/// Below it [`PopulationRules::in_force`] is [`PopulationRules::Held`], so that every belt's
/// recorded mean eccentricity and inclination and every impact velocity are version 23's bit for
/// bit. P14.T54.b deletes this hold, [`PopulationRules`] with its `Held` arms (the record's
/// [`BeltPart`] means, `hooks::figures::belt_excitation` and its cube, and the old
/// `LUNAR_ENCOUNTER_SPEED`), the tests' switch and its tripwire
/// (`the_belt_population_is_held_only_until_its_bump`).
pub(crate) const BELT_POPULATION_VERSION: GeneratorVersion = GeneratorVersion::new(24);

/// Whether the belts' moments and encounter speeds follow their populations' laws (the hold,
/// [`BELT_POPULATION_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum PopulationRules {
    /// Version 23's: the untruncated laws' σ√(π ÷ 2) on the record, and P14.T48.f's 2σₑ² + 2 sin²
    /// σᵢ for the impactors with its cube.
    Held,
    /// P14.T48.g's: the laws' quadrature means on the record, and Öpik's mean for the impactors.
    Laws,
}

impl PopulationRules {
    /// The rules this generator version gives: the laws from [`BELT_POPULATION_VERSION`], the
    /// untruncated moments below it; in the tests, the rules a test's switch sets on its thread
    /// ([`with_population_rules`]).
    #[must_use]
    pub(crate) fn in_force() -> Self {
        #[cfg(test)]
        if let Some(rules) = POPULATION_RULES.with(core::cell::Cell::get) {
            return rules;
        }
        if GENERATOR_VERSION >= BELT_POPULATION_VERSION {
            Self::Laws
        } else {
            Self::Held
        }
    }
}

#[cfg(test)]
thread_local! {
    /// The belt population's rules a test sets on its thread, `None` for the version's.
    static POPULATION_RULES: core::cell::Cell<Option<PopulationRules>> =
        const { core::cell::Cell::new(None) };
}

/// `run`'s result with the belt population's rules `rules` on this thread, for the tests to reach
/// 24's laws ahead of their bump; the version's rules are set back after, a panic in `run`
/// included. Its bump deletes it with the hold.
#[cfg(test)]
pub(crate) fn with_population_rules<R>(rules: PopulationRules, run: impl FnOnce() -> R) -> R {
    /// Sets the thread's rules back as it drops.
    struct Restore(Option<PopulationRules>);
    impl Drop for Restore {
        fn drop(&mut self) {
            POPULATION_RULES.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(POPULATION_RULES.with(|cell| cell.replace(Some(rules))));
    run()
}

/// How the crossers' encounters are counted in their mean
/// ([`PopulationLaw::encounter_excitation`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Weighting {
    /// By Öpik's collision probability per unit time: the mean over impacts.
    Collisions,
    /// Each crossing orbit once: the mean over the crossers.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the unweighted mean by which the tests check the kernel against F5.6 \
                      (decision-p14-t48g-opik §4)"
        )
    )]
    Orbits,
}

/// The weighting of the impactors' mean: Öpik's collision probability, the mean over impacts that
/// Bottke et al. 1994's main-belt mean of about 5.3 km/s is (from memory, its abstract unread;
/// Dahlgren 1998's whole-belt 5.28 km/s, as Cibulková et al. 2014, Icarus 241, 358, §4, quote
/// it). P14.T48.g's text, ruled by decision-p14-t48g-opik §1, with no gravitational focusing in
/// the weight, and the mean of U², not of U.
pub(crate) const ENCOUNTER_WEIGHTING: Weighting = Weighting::Collisions;

/// The spread of a scattered component's perihelia, as a multiple of its floor `q_z`: 1.2, so
/// that q lies uniform from `q_z` to 1.2 `q_z` and its mean is 1.1 `q_z`.
///
/// This module's figure, from JPL's Small-Body Database (the `TNO` class, fetched 2026-10-26):
/// the median perihelion of the bodies between Neptune's 2:1 at 47.7 au and 150 au lies at
/// 35.1–36.0 au in each of four bins of a (2,390 bodies), 1.07–1.10 times Neptune's chaotic zone's
/// outer edge, 32.66 au (P14.T21.a's 1.3 μ^(2⁄7) beyond its aphelion), which a mean of 1.1 `q_z`
/// matches (decision-p14-t48g-opik §7). The Kolmogorov–Smirnov distance of the bodies' e at their
/// a cannot choose between 1.2 and 1.3, which tie at 0.244, its floor the 24% of bodies with q
/// inside `q_z` (1.1 gives 0.48); over the bodies with q at or beyond `q_z` alone 1.3 fits better
/// (0.126 against 0.250), which the deferred spread scaled by the planet's Hill radius revisits.
pub const PERIHELION_SPREAD: f64 = 1.2;

/// How far the eccentricity and inclination integrals reach, in widths of their Rayleigh laws: 7,
/// beyond which a Rayleigh law holds e^(−24.5), 2 × 10⁻¹¹, of its bodies.
const RAYLEIGH_REACH: f64 = 7.0;

/// The relative distance inside an annulus's edges at which an encounter speed is read for a
/// target at or beyond an edge: 10⁻⁹, where the crossers' mean is its limit at the edge to far
/// below the quadrature's error.
const EDGE_NUDGE: f64 = 1e-9;

/// How a population's eccentricities lie at a semi-major axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EccentricityLaw {
    /// Rayleigh in the part's width, truncated where the orbit would reach the floor or the
    /// ceiling (P14.T21.c's members' law, without the disc's outer edge), held to
    /// [`MEMBER_ECCENTRICITY_CAP`].
    TruncatedRayleigh,
    /// Perihelia uniform from `q_z` to [`PERIHELION_SPREAD`] × `q_z`, with `q_z` the floor, and at
    /// most a, and e = 1 − q ÷ a: a scattered disc's.
    PerihelionAnchored,
}

/// How a population's semi-major axes lie over its annulus.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AxisSpread {
    /// Uniform in a.
    Uniform,
    /// As a disc's solids lie: e^(−a ÷ `r_c`) per unit radius on either side of the snow line,
    /// `inside_share` of them inside it (P14.T3's surface density, Σ ∝ e^(−r ÷ `r_c`) ÷ r, with
    /// the solids' share stepping up at the snow line).
    Solids {
        /// The disc's characteristic radius `r_c`, m; finite and positive.
        characteristic_radius: Metres,
        /// The disc's snow line, m; finite and positive.
        snow_line: Metres,
        /// The share of the annulus's solids inside the snow line, 0 to 1.
        inside_share: f64,
    },
}

/// What a [`PopulationLaw`] is built from.
///
/// Plain data with public fields: [`PopulationLaw::new`] validates them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PopulationParts {
    /// Which part of its belt it is, whose Rayleigh widths its laws read.
    pub part: BeltPart,
    /// How its eccentricities lie.
    pub eccentricity: EccentricityLaw,
    /// The annulus's inner edge, m; finite and positive.
    pub inner_edge: Metres,
    /// The annulus's outer edge, m; finite and beyond the inner edge.
    pub outer_edge: Metres,
    /// The least pericentre, m: the outer edge of the farthest-reaching chaotic zone of the
    /// planets inside the annulus, or the disc's inner edge; finite and non-negative, and positive
    /// for a perihelion-anchored law, whose perihelia it anchors. It lies at
    /// or inside the inner edge but where an inner planet's zone reaches into a Kuiper-like band
    /// (P14.T21.b checks only the outermost planet's), and the population is circular inside it.
    pub floor: Metres,
    /// The greatest apocentre, m: the inner edge of the nearest chaotic zone of the planets
    /// beyond the annulus; finite and positive, `None` with no planet beyond.
    pub ceiling: Option<Metres>,
    /// How its semi-major axes lie.
    pub spread: AxisSpread,
}

/// A belt part's small bodies' orbital laws (P14.T48.g; the [module](self) documentation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PopulationLaw {
    parts: PopulationParts,
}

/// Why [`PopulationLaw::new`] refused its parts, with the value it refused.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildPopulationLawError {
    /// The annulus's edges, inner and outer, are not finite, positive and in order.
    Edges(Metres, Metres),
    /// The floor is not finite and non-negative, or not positive under a perihelion-anchored
    /// law.
    Floor(Metres),
    /// The ceiling is not finite and positive.
    Ceiling(Metres),
    /// The solids' radius or snow line is not finite and positive, or their share inside the snow
    /// line is not in 0–1.
    Spread(AxisSpread),
}

impl fmt::Display for BuildPopulationLawError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Edges(inner, outer) => write!(
                f,
                "annulus edges {} m to {} m are not finite, positive and in order",
                inner.value(),
                outer.value()
            ),
            Self::Floor(floor) => write!(
                f,
                "pericentre floor {} m is out of range for its law",
                floor.value()
            ),
            Self::Ceiling(ceiling) => write!(
                f,
                "apocentre ceiling {} m is not finite and positive",
                ceiling.value()
            ),
            Self::Spread(spread) => write!(f, "axis spread {spread:?} is out of range"),
        }
    }
}

impl Error for BuildPopulationLawError {}

/// A belt's populations, its belt proper's and its scattered component's, as its impactors read
/// them (P14.T48.g).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BeltPopulation {
    /// The belt proper's.
    pub main: PopulationLaw,
    /// The scattered component's, if the belt has one.
    pub scattered: Option<PopulationLaw>,
}

impl BeltPopulation {
    /// The law of its part `part`; `None` for a scattered part it does not have.
    #[must_use]
    pub const fn part(&self, part: BeltPart) -> Option<&PopulationLaw> {
        match part {
            BeltPart::Main => Some(&self.main),
            BeltPart::Scattered => self.scattered.as_ref(),
        }
    }
}

/// The eccentricities at one semi-major axis.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Eccentricities {
    /// None: every orbit there is circular.
    Circular,
    /// Rayleigh of width `width` truncated at `limit`, whose share below the limit is `norm`.
    Rayleigh { width: f64, limit: f64, norm: f64 },
    /// Uniform over `low` to `high`.
    Uniform { low: f64, high: f64 },
}

impl Eccentricities {
    /// The mean eccentricity.
    fn mean(self) -> f64 {
        match self {
            Self::Circular => 0.0,
            Self::Rayleigh { width, limit, norm } => {
                let x = limit / width;
                if x * x < 1e-6 {
                    // The law's mean as the limit falls to zero, 2L ÷ 3, where the closed form
                    // cancels.
                    return 2.0 * limit / 3.0;
                }
                let whole = width * (PI / 2.0).sqrt() * math::erf(x / SQRT_2);
                (whole - limit * math::exp(-x * x / 2.0)) / norm
            }
            Self::Uniform { low, high } => f64::midpoint(low, high),
        }
    }

    /// The mean square eccentricity.
    fn mean_square(self) -> f64 {
        match self {
            Self::Circular => 0.0,
            Self::Rayleigh { width, limit, .. } => {
                let x = limit / width;
                if x * x < 1e-6 {
                    return limit * limit / 2.0;
                }
                2.0 * width * width - limit * limit / math::exp_m1(x * x / 2.0)
            }
            Self::Uniform { low, high } => (low * low + low * high + high * high) / 3.0,
        }
    }

    /// The support's upper end, beyond which the law holds nothing (a Rayleigh's within
    /// [`RAYLEIGH_REACH`] widths).
    fn reach(self) -> (f64, f64) {
        match self {
            Self::Circular => (0.0, 0.0),
            Self::Rayleigh { width, limit, .. } => (0.0, limit.min(RAYLEIGH_REACH * width)),
            Self::Uniform { low, high } => (low, high),
        }
    }

    /// The density at e inside the support.
    fn density(self, e: f64) -> f64 {
        match self {
            Self::Circular => 0.0,
            Self::Rayleigh { width, norm, .. } => {
                let w2 = width * width;
                e / w2 * math::exp(-e * e / (2.0 * w2)) / norm
            }
            Self::Uniform { low, high } => 1.0 / (high - low),
        }
    }
}

/// The inclination nodes of an encounter integral: each node's cos i, sin i and Rayleigh weight
/// (the quadrature's weight times the density).
struct InclinationNodes {
    cos: [f64; 16],
    sin: [f64; 16],
    weight: [f64; 16],
}

impl InclinationNodes {
    /// The nodes of a Rayleigh law of width `width` (rad), from 0 to [`RAYLEIGH_REACH`] widths or
    /// π.
    fn of(width: f64) -> Self {
        let reach = (RAYLEIGH_REACH * width).min(PI);
        let half = reach / 2.0;
        let w2 = width * width;
        let mut nodes = Self {
            cos: [0.0; 16],
            sin: [0.0; 16],
            weight: [0.0; 16],
        };
        for k in 0..16 {
            let i = half * (GL16_NODES[k] + 1.0);
            let (sin, cos) = math::sin_cos(i);
            nodes.cos[k] = cos;
            nodes.sin[k] = sin;
            nodes.weight[k] = GL16_WEIGHTS[k] * half * (i / w2) * math::exp(-i * i / (2.0 * w2));
        }
        nodes
    }
}

impl PopulationLaw {
    /// The law `parts` describe.
    ///
    /// # Errors
    ///
    /// - [`BuildPopulationLawError::Edges`] for edges that are not finite, positive and in
    ///   order, the outer beyond the inner;
    /// - [`BuildPopulationLawError::Floor`] for a floor that is not finite and non-negative, or
    ///   not positive under [`EccentricityLaw::PerihelionAnchored`];
    /// - [`BuildPopulationLawError::Ceiling`] for a ceiling that is not finite and positive;
    /// - [`BuildPopulationLawError::Spread`] for solids whose radius or snow line is not finite
    ///   and positive, or whose share inside the snow line is not in 0–1.
    ///
    /// # Examples
    ///
    /// The Solar main belt between Mars's chaotic zone and Jupiter's: truncation at the planets
    /// lowers its mean eccentricity below the untruncated law's 0.125.
    ///
    /// ```
    /// use hyperion_sim::planetary::belts::BeltPart;
    /// use hyperion_sim::planetary::belts::population::{
    ///     AxisSpread, EccentricityLaw, PopulationLaw, PopulationParts,
    /// };
    /// use hyperion_sim::units::Metres;
    /// use hyperion_sim::units::consts::METRES_PER_AU;
    ///
    /// let au = |x: f64| Metres::new(x * METRES_PER_AU);
    /// let belt = PopulationLaw::new(PopulationParts {
    ///     part: BeltPart::Main,
    ///     eccentricity: EccentricityLaw::TruncatedRayleigh,
    ///     inner_edge: au(2.065),
    ///     outer_edge: au(3.278),
    ///     floor: au(1.694),
    ///     ceiling: Some(au(4.024)),
    ///     spread: AxisSpread::Uniform,
    /// })?;
    /// let e = belt.mean_eccentricity();
    /// assert!((0.115..0.125).contains(&e));
    /// # Ok::<(), hyperion_sim::planetary::belts::population::BuildPopulationLawError>(())
    /// ```
    pub fn new(parts: PopulationParts) -> Result<Self, BuildPopulationLawError> {
        let (inner, outer) = (parts.inner_edge, parts.outer_edge);
        if !(inner.value().is_finite()
            && inner.value() > 0.0
            && outer.value().is_finite()
            && outer > inner)
        {
            return Err(BuildPopulationLawError::Edges(inner, outer));
        }
        let floor = parts.floor;
        let least = match parts.eccentricity {
            EccentricityLaw::TruncatedRayleigh => floor.value() >= 0.0,
            EccentricityLaw::PerihelionAnchored => floor.value() > 0.0,
        };
        if !(floor.value().is_finite() && least) {
            return Err(BuildPopulationLawError::Floor(floor));
        }
        if let Some(ceiling) = parts.ceiling
            && !(ceiling.value().is_finite() && ceiling.value() > 0.0)
        {
            return Err(BuildPopulationLawError::Ceiling(ceiling));
        }
        if let AxisSpread::Solids {
            characteristic_radius,
            snow_line,
            inside_share,
        } = parts.spread
        {
            let positive = |m: Metres| m.value().is_finite() && m.value() > 0.0;
            if !(positive(characteristic_radius)
                && positive(snow_line)
                && (0.0..=1.0).contains(&inside_share))
            {
                return Err(BuildPopulationLawError::Spread(parts.spread));
            }
        }
        Ok(Self { parts })
    }

    /// Which part of its belt it is.
    #[must_use]
    pub const fn part(&self) -> BeltPart {
        self.parts.part
    }

    /// The parts it was built from.
    #[must_use]
    pub const fn parts(&self) -> &PopulationParts {
        &self.parts
    }

    /// The truncated Rayleigh law's limit at a semi-major axis `a` (m): the eccentricity at which
    /// the pericentre reaches the floor or the apocentre the ceiling, held to 0–
    /// [`MEMBER_ECCENTRICITY_CAP`].
    #[must_use]
    fn rayleigh_limit(&self, a: f64) -> f64 {
        let from_floor = 1.0 - self.parts.floor.value() / a;
        let limit = self
            .parts
            .ceiling
            .map_or(from_floor, |c| from_floor.min(c.value() / a - 1.0));
        limit.clamp(0.0, MEMBER_ECCENTRICITY_CAP)
    }

    /// The eccentricities at a semi-major axis `a` (m).
    #[must_use]
    fn eccentricities(&self, a: f64) -> Eccentricities {
        let floor = self.parts.floor.value();
        match self.parts.eccentricity {
            EccentricityLaw::TruncatedRayleigh => {
                let width = widths(self.parts.part).0;
                let limit = self.rayleigh_limit(a);
                if limit <= 0.0 {
                    return Eccentricities::Circular;
                }
                let norm = -math::exp_m1(-(limit * limit) / (2.0 * width * width));
                Eccentricities::Rayleigh { width, limit, norm }
            }
            EccentricityLaw::PerihelionAnchored => {
                if a <= floor {
                    return Eccentricities::Circular;
                }
                let nearest = (PERIHELION_SPREAD * floor).min(a);
                Eccentricities::Uniform {
                    low: 1.0 - nearest / a,
                    high: 1.0 - floor / a,
                }
            }
        }
    }

    /// Where the eccentricities' law changes form over a, m: the truncated Rayleigh's limit where
    /// its floor's and ceiling's bounds cross and where it reaches the cap, and the anchored law's
    /// where its perihelia reach a.
    fn kinks(&self) -> [Option<f64>; 3] {
        let floor = self.parts.floor.value();
        match self.parts.eccentricity {
            EccentricityLaw::TruncatedRayleigh => [
                self.parts.ceiling.map(|c| f64::midpoint(floor, c.value())),
                Some(floor / (1.0 - MEMBER_ECCENTRICITY_CAP)),
                self.parts
                    .ceiling
                    .map(|c| c.value() / (1.0 + MEMBER_ECCENTRICITY_CAP)),
            ],
            EccentricityLaw::PerihelionAnchored => [Some(PERIHELION_SPREAD * floor), None, None],
        }
    }

    /// The semi-major axis nodes over the annulus, (a in m, weight), the weights the spread's
    /// density times the quadrature's: 16 Gauss–Legendre nodes on each piece between the edges,
    /// the snow line, the law's kinks and `extra`, in ascending order.
    fn axis_nodes(&self, extra: Option<f64>) -> Vec<(f64, f64)> {
        let (lo, hi) = (self.parts.inner_edge.value(), self.parts.outer_edge.value());
        let inside_open = |x: f64| x > lo && x < hi;
        let snow = match self.parts.spread {
            AxisSpread::Uniform => None,
            AxisSpread::Solids { snow_line, .. } => {
                Some(snow_line.value()).filter(|&s| inside_open(s))
            }
        };
        let mut cuts: Vec<f64> = Vec::with_capacity(7);
        cuts.push(lo);
        cuts.extend(snow);
        cuts.extend(
            self.kinks()
                .into_iter()
                .flatten()
                .filter(|&x| inside_open(x)),
        );
        cuts.extend(extra.filter(|&x| inside_open(x)));
        cuts.push(hi);
        cuts.sort_by(f64::total_cmp);
        cuts.dedup();

        // The density on each side of the snow line, normalised to that side's share.
        let side_density = |p: f64, q: f64, share: f64, rc: f64| {
            let span = -math::exp_m1(-(q - p) / rc);
            move |a: f64| share * math::exp(-(a - p) / rc) / (rc * span)
        };
        let density = |a: f64| -> f64 {
            match self.parts.spread {
                AxisSpread::Uniform => 1.0 / (hi - lo),
                AxisSpread::Solids {
                    characteristic_radius,
                    inside_share,
                    ..
                } => {
                    let rc = characteristic_radius.value();
                    match snow {
                        None => {
                            // The whole annulus lies on one side of the snow line.
                            side_density(lo, hi, 1.0, rc)(a)
                        }
                        Some(s) if a < s => side_density(lo, s, inside_share, rc)(a),
                        Some(s) => side_density(s, hi, 1.0 - inside_share, rc)(a),
                    }
                }
            }
        };
        let mut nodes = Vec::with_capacity(16 * (cuts.len() - 1));
        for piece in cuts.windows(2) {
            let (p, q) = (piece[0], piece[1]);
            let (mid, half) = (f64::midpoint(p, q), (q - p) / 2.0);
            for k in 0..16 {
                let a = mid + half * GL16_NODES[k];
                nodes.push((a, GL16_WEIGHTS[k] * half * density(a)));
            }
        }
        nodes
    }

    /// The population's mean eccentricity over its annulus (the [module](self) documentation's
    /// quadrature).
    #[must_use]
    pub fn mean_eccentricity(&self) -> f64 {
        self.axis_mean(Eccentricities::mean)
    }

    /// The population's mean square eccentricity ⟨e²⟩ over its annulus.
    #[must_use]
    pub fn mean_square_eccentricity(&self) -> f64 {
        self.axis_mean(Eccentricities::mean_square)
    }

    /// The population's mean inclination to its host's plane, rad: σᵢ√(π ÷ 2) of its untruncated
    /// Rayleigh law, as [`BeltPart::mean_inclination`] gives it.
    #[must_use]
    pub fn mean_inclination(&self) -> Radians {
        self.parts.part.mean_inclination()
    }

    /// The mean over the annulus of `moment` of the eccentricities at each a, by the axis nodes,
    /// over their summed weights.
    fn axis_mean(&self, moment: impl Fn(Eccentricities) -> f64) -> f64 {
        let (sum, weight) = self
            .axis_nodes(None)
            .into_iter()
            .fold((0.0, 0.0), |(sum, weight), (a, w)| {
                (sum + w * moment(self.eccentricities(a)), weight + w)
            });
        sum / weight
    }

    /// `U_b`² at a semi-major axis `a`: Öpik's mean U² over the population's orbits that cross a,
    /// weighted by their collision probability, dimensionless, finite and non-negative.
    ///
    /// U is in units of the circular speed at a, and the weight is Öpik's per unit time (the
    /// [module](self) documentation). A target at or beyond an edge of the annulus is read just
    /// inside it, at a relative 10⁻⁹ (`EDGE_NUDGE`). It is zero only where no orbit of the
    /// population crosses the target's, a population circular throughout: it then adds nothing to
    /// the target's own e² + sin² i.
    ///
    /// # Examples
    ///
    /// The Solar main belt's encounters at Ceres's 2.77 au, about 0.045, are slower than the
    /// untruncated widths' 2σₑ² + 2 sin² σᵢ = 0.059 that P14.T48.f takes: Öpik's weighting counts
    /// the slow, frequent encounters of the low-inclination crossers more often.
    ///
    /// ```
    /// use hyperion_sim::planetary::belts::BeltPart;
    /// use hyperion_sim::planetary::belts::population::{
    ///     AxisSpread, EccentricityLaw, PopulationLaw, PopulationParts,
    /// };
    /// use hyperion_sim::units::Metres;
    /// use hyperion_sim::units::consts::METRES_PER_AU;
    ///
    /// let au = |x: f64| Metres::new(x * METRES_PER_AU);
    /// let belt = PopulationLaw::new(PopulationParts {
    ///     part: BeltPart::Main,
    ///     eccentricity: EccentricityLaw::TruncatedRayleigh,
    ///     inner_edge: au(2.065),
    ///     outer_edge: au(3.278),
    ///     floor: au(1.694),
    ///     ceiling: Some(au(4.024)),
    ///     spread: AxisSpread::Uniform,
    /// })?;
    /// let u2 = belt.encounter_excitation(au(2.7675));
    /// assert!((0.040..0.050).contains(&u2));
    /// # Ok::<(), hyperion_sim::planetary::belts::population::BuildPopulationLawError>(())
    /// ```
    #[must_use]
    pub fn encounter_excitation(&self, a: Metres) -> f64 {
        self.encounter_mean(a, ENCOUNTER_WEIGHTING)
    }

    /// [`encounter_excitation`](Self::encounter_excitation) under the weighting `weighting`.
    #[must_use]
    pub(crate) fn encounter_mean(&self, a: Metres, weighting: Weighting) -> f64 {
        let (lo, hi) = (self.parts.inner_edge.value(), self.parts.outer_edge.value());
        let (near, far) = (lo * (1.0 + EDGE_NUDGE), hi * (1.0 - EDGE_NUDGE));
        // An annulus thinner than the nudge is read at its midpoint.
        let target = if near < far {
            a.value().clamp(near, far)
        } else {
            f64::midpoint(lo, hi)
        };
        let inclinations = InclinationNodes::of(widths(self.parts.part).1);
        let (mut sum, mut weight) = (0.0, 0.0);
        for (a_p, w_a) in self.axis_nodes(Some(target)) {
            let (s, w) = crossers_at(
                a_p / target,
                self.eccentricities(a_p),
                &inclinations,
                weighting,
            );
            sum += w_a * s;
            weight += w_a * w;
        }
        if weight > 0.0 { sum / weight } else { 0.0 }
    }
}

/// Öpik's U² summed over the crossers of semi-major axis `A` times the target's, with
/// eccentricities `eccentricities` and inclinations `inclinations`, under `weighting`: (Σ w U²,
/// Σ w), the eccentricity integral taken in s = √(e² − `e_min`²) from the least crossing
/// eccentricity `e_min` = |1 − 1 ÷ A|, which takes out Öpik's 1 ÷ |`U_x`|.
fn crossers_at(
    axis_ratio: f64,
    eccentricities: Eccentricities,
    inclinations: &InclinationNodes,
    weighting: Weighting,
) -> (f64, f64) {
    let big_a = axis_ratio;
    let e_min = (1.0 - 1.0 / big_a).abs();
    let (low, high) = eccentricities.reach();
    let from = low.max(e_min);
    if high <= from {
        return (0.0, 0.0);
    }
    let e_min2 = e_min * e_min;
    let (s_lo, s_hi) = ((from * from - e_min2).sqrt(), (high * high - e_min2).sqrt());
    let (mid, half) = (f64::midpoint(s_lo, s_hi), (s_hi - s_lo) / 2.0);
    let period = big_a * big_a.sqrt();
    let (mut sum, mut weight) = (0.0, 0.0);
    for k in 0..16 {
        let s = mid + half * GL16_NODES[k];
        let e = (e_min2 + s * s).sqrt();
        // f(e) de = f(e) (s ÷ e) ds; Öpik's 1 ÷ |U_x| = 1 ÷ (√A s), and the period's 1 ÷ A^1.5.
        let w_e = GL16_WEIGHTS[k]
            * half
            * eccentricities.density(e)
            * match weighting {
                Weighting::Collisions => 1.0 / (e * big_a.sqrt() * period),
                Weighting::Orbits => s / e,
            };
        let cross = 2.0 * (big_a * (1.0 - e * e)).sqrt();
        let base = 3.0 - 1.0 / big_a;
        for j in 0..16 {
            let u2 = (base - cross * inclinations.cos[j]).max(0.0);
            let w = w_e
                * inclinations.weight[j]
                * match weighting {
                    Weighting::Collisions => u2.sqrt() / inclinations.sin[j],
                    Weighting::Orbits => 1.0,
                };
            sum += w * u2;
            weight += w;
        }
    }
    (sum, weight)
}
