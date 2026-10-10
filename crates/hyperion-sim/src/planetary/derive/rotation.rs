//! Rotation and tides (plan 14, P14.T14.a–b): a body's primordial spin, how long its primary's
//! tides take to lock it, and its rotation angle as a closed form of time.
//!
//! # The spin (P14.T14.a)
//!
//! Each planet, moon and dwarf planet draws four words on its own `planet.spin` stream
//! ([`SpinDraws`]): the ranks of its primordial period and of its obliquity, the azimuth of its
//! pole about its orbit's normal, and its rotation phase at the epoch.
//!
//! - The primordial period is log-normal about 10 hours for a giant and 15 hours for every other
//!   class ([`SpinFamily`], [`primordial_period`]), with a scatter of
//!   [`PRIMORDIAL_PERIOD_SCATTER_DEX`], and never shorter than the body's break-up period
//!   ([`breakup_period`]), which the assembly takes as a floor.
//! - The obliquity, the angle between the spin axis and the orbit's normal, is isotropic for a
//!   body that had a giant impact (one with a giant-impact moon, P14.T18) and Rayleigh with scale
//!   [`QUIET_OBLIQUITY_SCALE`] otherwise ([`ObliquityLaw`], [`obliquity`]).
//!
//! # Locking (P14.T14.b)
//!
//! The despinning time is Gladman et al.'s (1996, Icarus 122, 166, eq. 9),
//! τ = ω a⁶ I Q ÷ (3 G M² k₂ R⁵), with ω the primordial spin rate, a the semi-major axis, M the
//! primary's mass (the star's for a planet, the planet's for a moon), I = α m R² the body's moment
//! of inertia and k₂ and Q its tides ([`tides`], P14.T14.d), as P14.T15's limits take them: a
//! rocky body's for the rocky and icy classes, a gas giant's for gas giants, and for a body under
//! a hydrogen and helium envelope, a sub-Neptune or an ice giant, its fluid Love number, the
//! core's tide seen from the envelope's top ([`love_number_under_envelope`]), with
//! Q = 1.03 × 10⁴ ([`ENVELOPED_TIDAL_Q`]).
//!
//! P14.T14.d is held until the 21 → 22 batch's bump (plan 14, "Order and parallelism"): below
//! version 22 the generator gives every class its earlier pair, a rocky body's to every class
//! with a surface and a giant's to the giants, so that the batch's goldens move once, at its bump.
//!
//! The spin rate falls (or rises) linearly with the system's age from ω to the locked rate over τ
//! and stays there: synchronous, the orbit's mean motion n, or 1.5 n in the 3:2 state for an orbit
//! more eccentric than [`SPIN_ORBIT_RESONANCE_ECCENTRICITY`] ([`SpinOrbitResonance`]). The
//! rotation angle ([`RotationLaw::angle_at`]) is the integral of that rate, a closed form in each
//! regime, and the locked regime's angle is tied to the orbit so that the prime meridian faces the
//! primary at pericentre ([`frames`](crate::planetary::frames), design note 23).
//!
//! The spin itself, the pole and the prime meridian are in
//! [`frames`](crate::planetary::frames); this module holds the laws, each a pure function of plain
//! quantities, testable on Solar System values without a generator.

use core::f64::consts::{PI, TAU};
use std::error::Error;
use std::fmt;

use crate::id::BodyId;
use crate::math;
use crate::orbit::KeplerElements;
use crate::planetary::derive::PlanetClass;
use crate::planetary::derive::composition::MassFractions;
use crate::planetary::derive::radius::{CoreComposition, EARTH_CORE_MASS_FRACTION, radius_zeng};
use crate::planetary::frames::{BodyFixedFrame, FrameSpin};
use crate::planetary::params::{
    ENVELOPE_LOVE_COEFFICIENT, ENVELOPED_CORE_LOVE_NUMBER, ENVELOPED_CORE_WATER_SOFTENING,
    ENVELOPED_MOMENT_OF_INERTIA, ENVELOPED_TIDAL_Q, GAS_GIANT_MOMENT_OF_INERTIA, GIANT_LOVE_NUMBER,
    GIANT_PRIMORDIAL_PERIOD, GIANT_TIDAL_Q, ICY_MOMENT_OF_INERTIA, JUPITER_HEAVY_ELEMENT_FRACTION,
    PRIMORDIAL_PERIOD_SCATTER_DEX, QUIET_OBLIQUITY_SCALE, ROCKY_LOVE_NUMBER,
    ROCKY_MOMENT_OF_INERTIA, ROCKY_PRIMORDIAL_PERIOD, ROCKY_TIDAL_Q, SATURN_HEAVY_ELEMENT_FRACTION,
    SATURN_LIKE_MOMENT_OF_INERTIA, SPIN_ORBIT_RESONANCE_ECCENTRICITY,
};
use crate::rng::{ObjectKey, Stream, tags};
use crate::stellar::draws::UnitUniform;
use crate::time::{Span, UniverseTime};
use crate::units::consts::GRAVITATIONAL_CONSTANT;
use crate::units::{EarthMasses, EarthRadii, Kilograms, Metres, Radians, Seconds, Years};
use crate::{GENERATOR_VERSION, GeneratorVersion, Seed};

/// Words of [`tags::PLANET_SPIN`] a body reads or reserves: words 0–3, [`SpinDraws`], then four
/// reserved.
pub const SPIN_WORDS: u64 = 8;

/// A body's draws on its own [`tags::PLANET_SPIN`] stream (P14.T14.a).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpinDraws {
    period_rank: UnitUniform,
    obliquity_rank: UnitUniform,
    pole_azimuth: Radians,
    phase: Radians,
}

impl SpinDraws {
    /// The median draws: both ranks one half, and the pole's azimuth and the phase zero.
    pub const MEDIAN: Self = Self {
        period_rank: UnitUniform::HALF,
        obliquity_rank: UnitUniform::HALF,
        pole_azimuth: Radians::ZERO,
        phase: Radians::ZERO,
    };

    /// Draws of the ranks and angles given, for a test or a tool; each angle is reduced into
    /// `[0, 2π)`, and one that is not finite is taken as zero.
    #[must_use]
    pub fn new(
        period_rank: UnitUniform,
        obliquity_rank: UnitUniform,
        pole_azimuth: Radians,
        phase: Radians,
    ) -> Self {
        Self {
            period_rank,
            obliquity_rank,
            pole_azimuth: Radians::new(reduce(pole_azimuth.value())),
            phase: Radians::new(reduce(phase.value())),
        }
    }

    /// The draws of `body` in the universe of `seed`: words 0 and 1 of its
    /// [`tags::PLANET_SPIN`] stream, the ranks of its period and obliquity, open uniforms; words 2
    /// and 3, its pole's azimuth and its phase at the epoch, uniform on `[0, 2π)`.
    ///
    /// # Panics
    ///
    /// Never: an open uniform lies strictly between 0 and 1.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::Seed;
    /// use hyperion_sim::id::{BodyId, SystemId};
    /// use hyperion_sim::planetary::derive::rotation::SpinDraws;
    ///
    /// let system = SystemId::from_raw(0x0200_0800_2000_0000)?;
    /// let (b, c) = (BodyId::new(system, 0x0100), BodyId::new(system, 0x0200));
    /// assert_eq!(SpinDraws::for_body(Seed::new(7), b), SpinDraws::for_body(Seed::new(7), b));
    /// assert_ne!(SpinDraws::for_body(Seed::new(7), b), SpinDraws::for_body(Seed::new(7), c));
    /// # Ok::<(), hyperion_sim::id::DecodeSystemIdError>(())
    /// ```
    #[must_use]
    pub fn for_body(seed: Seed, body: BodyId) -> Self {
        let mut stream = Stream::open(seed, tags::PLANET_SPIN, ObjectKey::from(body));
        let rank = |u: f64| UnitUniform::new(u).expect("an open uniform lies strictly in (0, 1)");
        let period_rank = rank(stream.uniform_open());
        let obliquity_rank = rank(stream.uniform_open());
        let pole_azimuth = Radians::new(TAU * stream.uniform());
        let phase = Radians::new(TAU * stream.uniform());
        Self {
            period_rank,
            obliquity_rank,
            pole_azimuth,
            phase,
        }
    }

    /// The rank of the primordial period.
    #[must_use]
    pub const fn period_rank(&self) -> UnitUniform {
        self.period_rank
    }

    /// The rank of the obliquity.
    #[must_use]
    pub const fn obliquity_rank(&self) -> UnitUniform {
        self.obliquity_rank
    }

    /// The azimuth of the pole about the orbit's normal, rad, in `[0, 2π)`, from the orbit's
    /// ascending node.
    #[must_use]
    pub const fn pole_azimuth(&self) -> Radians {
        self.pole_azimuth
    }

    /// The rotation phase at the epoch, rad, in `[0, 2π)`: the prime meridian's angle then, for a
    /// body not yet locked.
    #[must_use]
    pub const fn phase(&self) -> Radians {
        self.phase
    }
}

/// Which primordial period law a body follows (P14.T14.a).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SpinFamily {
    /// Every class with a surface: log-normal about [`ROCKY_PRIMORDIAL_PERIOD`].
    Rocky,
    /// The ice and gas giants: log-normal about [`GIANT_PRIMORDIAL_PERIOD`].
    Giant,
}

impl SpinFamily {
    /// The family of a body of class `class`: [`Giant`](Self::Giant) for the classes without a
    /// surface, the ice and gas giants.
    #[must_use]
    pub const fn of(class: PlanetClass) -> Self {
        if class.has_surface() {
            Self::Rocky
        } else {
            Self::Giant
        }
    }

    /// The family's median primordial period.
    #[must_use]
    pub const fn median_period(self) -> Seconds {
        match self {
            Self::Rocky => ROCKY_PRIMORDIAL_PERIOD,
            Self::Giant => GIANT_PRIMORDIAL_PERIOD,
        }
    }
}

/// The primordial rotation period of a body of family `family` at rank `rank` (P14.T14.a):
/// log-normal, P = `P_med` × 10^(σ Φ⁻¹(rank)) with σ [`PRIMORDIAL_PERIOD_SCATTER_DEX`].
///
/// The law alone: the assembly floors it at the body's [`breakup_period`].
///
/// # Examples
///
/// ```
/// use hyperion_sim::planetary::derive::rotation::{SpinFamily, primordial_period};
/// use hyperion_sim::stellar::draws::UnitUniform;
///
/// let giant = primordial_period(SpinFamily::Giant, UnitUniform::HALF);
/// assert!((giant.value() - 36_000.0).abs() < 1e-6);
/// ```
#[must_use]
pub fn primordial_period(family: SpinFamily, rank: UnitUniform) -> Seconds {
    let z = math::normal_quantile(rank.value());
    Seconds::new(family.median_period().value() * math::exp10(PRIMORDIAL_PERIOD_SCATTER_DEX * z))
}

/// The shortest rotation period a body of mass `mass` and radius `radius` can hold, at which its
/// equator orbits: 2π √(R³ ÷ G M).
///
/// A rigid sphere's; a real body flattens and sheds mass somewhat before it, so this is a floor,
/// not a target.
#[must_use]
pub fn breakup_period(mass: Kilograms, radius: Metres) -> Seconds {
    let r = radius.value();
    Seconds::new(TAU * (r * r * r / (GRAVITATIONAL_CONSTANT * mass.value())).sqrt())
}

/// How a body's obliquity is drawn (P14.T14.a).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ObliquityLaw {
    /// Isotropic: the cosine of the obliquity is uniform on `[−1, 1]`, for a body that had a
    /// giant impact.
    Isotropic,
    /// Rayleigh with scale [`QUIET_OBLIQUITY_SCALE`], for every other body.
    Rayleigh,
}

/// The obliquity at rank `rank` under `law`, rad, in `[0, π]` (P14.T14.a).
///
/// - Isotropic: ε = arccos(1 − 2u), whose distribution is (1 − cos ε) ÷ 2.
/// - Rayleigh: ε = σ √(−2 ln(1 − u)), whose distribution is 1 − exp(−ε² ÷ 2σ²), held at π, which
///   a scale of 10° reaches with a probability of e⁻¹⁶².
///
/// # Examples
///
/// ```
/// use hyperion_sim::planetary::derive::rotation::{ObliquityLaw, obliquity};
/// use hyperion_sim::stellar::draws::UnitUniform;
///
/// let quiet = obliquity(ObliquityLaw::Rayleigh, UnitUniform::HALF);
/// let tumbled = obliquity(ObliquityLaw::Isotropic, UnitUniform::HALF);
/// assert!(quiet.value().to_degrees() < 15.0);
/// assert!((tumbled.value() - core::f64::consts::FRAC_PI_2).abs() < 1e-12);
/// ```
#[must_use]
pub fn obliquity(law: ObliquityLaw, rank: UnitUniform) -> Radians {
    let u = rank.value();
    Radians::new(match law {
        ObliquityLaw::Isotropic => math::acos((1.0 - 2.0 * u).clamp(-1.0, 1.0)),
        ObliquityLaw::Rayleigh => {
            let sigma = Radians::from(QUIET_OBLIQUITY_SCALE).value();
            (sigma * (-2.0 * math::ln_1p(-u)).sqrt()).min(PI)
        }
    })
}

/// What tides act on: a body's mass, radius, moment-of-inertia factor, Love number k₂ and tidal
/// quality factor Q (P14.T14.b).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpinningBody {
    mass: Kilograms,
    radius: Metres,
    moment_factor: f64,
    love_number: f64,
    tidal_q: f64,
}

impl SpinningBody {
    /// A body of the values given.
    ///
    /// # Errors
    ///
    /// [`BuildSpinningBodyError`] naming the first value that is not positive and finite.
    pub fn new(
        mass: Kilograms,
        radius: Metres,
        moment_factor: f64,
        love_number: f64,
        tidal_q: f64,
    ) -> Result<Self, BuildSpinningBodyError> {
        let positive = |x: f64| x.is_finite() && x > 0.0;
        let checks = [
            (mass.value(), BuildSpinningBodyError::MassNotPositive),
            (radius.value(), BuildSpinningBodyError::RadiusNotPositive),
            (
                moment_factor,
                BuildSpinningBodyError::MomentFactorNotPositive,
            ),
            (love_number, BuildSpinningBodyError::LoveNumberNotPositive),
            (tidal_q, BuildSpinningBodyError::TidalQNotPositive),
        ];
        if let Some(&(_, error)) = checks.iter().find(|(x, _)| !positive(*x)) {
            return Err(error);
        }
        Ok(Self {
            mass,
            radius,
            moment_factor,
            love_number,
            tidal_q,
        })
    }

    /// A body of mass `mass`, radius `radius`, class `class` and mass fractions `fractions`,
    /// with its moment of inertia ([`moment_of_inertia_factor`]) and its tides ([`tides`],
    /// P14.T14.d).
    ///
    /// Until the 21 → 22 batch's bump the tides are the class's earlier pair: a rocky body's
    /// k₂ = 0.3 and Q = 100 for every class with a surface, a giant's 0.4 and 10⁵ otherwise
    /// (P14.T14.b's pairs, [`ROCKY_LOVE_NUMBER`] and [`ROCKY_TIDAL_Q`], [`GIANT_LOVE_NUMBER`] and
    /// [`GIANT_TIDAL_Q`]; see the [module](self) documentation).
    ///
    /// # Errors
    ///
    /// As [`new`](Self::new), for a mass or radius that is not positive and finite.
    pub fn of_class(
        mass: Kilograms,
        radius: Metres,
        class: PlanetClass,
        fractions: &MassFractions,
    ) -> Result<Self, BuildSpinningBodyError> {
        let (love_number, tidal_q) = tides_in_force(
            class,
            EarthMasses::from(mass),
            EarthRadii::from(radius),
            fractions,
        );
        Self::new(
            mass,
            radius,
            moment_of_inertia_factor(class, fractions),
            love_number,
            tidal_q,
        )
    }

    /// The mass.
    #[must_use]
    pub const fn mass(&self) -> Kilograms {
        self.mass
    }

    /// The radius.
    #[must_use]
    pub const fn radius(&self) -> Metres {
        self.radius
    }

    /// The moment of inertia in units of M R².
    #[must_use]
    pub const fn moment_factor(&self) -> f64 {
        self.moment_factor
    }

    /// The Love number k₂.
    #[must_use]
    pub const fn love_number(&self) -> f64 {
        self.love_number
    }

    /// The tidal quality factor Q.
    #[must_use]
    pub const fn tidal_q(&self) -> f64 {
        self.tidal_q
    }
}

/// A [`SpinningBody`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildSpinningBodyError {
    /// The mass was not positive and finite.
    MassNotPositive,
    /// The radius was not positive and finite.
    RadiusNotPositive,
    /// The moment-of-inertia factor was not positive and finite.
    MomentFactorNotPositive,
    /// The Love number was not positive and finite.
    LoveNumberNotPositive,
    /// The tidal quality factor was not positive and finite.
    TidalQNotPositive,
}

impl fmt::Display for BuildSpinningBodyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::MassNotPositive => "a spinning body's mass must be positive and finite",
            Self::RadiusNotPositive => "a spinning body's radius must be positive and finite",
            Self::MomentFactorNotPositive => {
                "a spinning body's moment of inertia must be positive and finite"
            }
            Self::LoveNumberNotPositive => {
                "a spinning body's love number must be positive and finite"
            }
            Self::TidalQNotPositive => "a spinning body's tidal q must be positive and finite",
        })
    }
}

impl Error for BuildSpinningBodyError {}

/// The moment of inertia C ÷ M R² of a body of class `class` and mass fractions `fractions`, the
/// one factor that serves both its locking and its flattening (P14.T14.b, P14.T46.a):
/// [`ROCKY_MOMENT_OF_INERTIA`], [`ICY_MOMENT_OF_INERTIA`], [`ENVELOPED_MOMENT_OF_INERTIA`] for
/// sub-Neptunes and ice giants, and for a gas giant a blend in its heavy-element fraction
/// Z = 1 − envelope: [`GAS_GIANT_MOMENT_OF_INERTIA`] at or below
/// [`JUPITER_HEAVY_ELEMENT_FRACTION`], [`SATURN_LIKE_MOMENT_OF_INERTIA`] at or above
/// [`SATURN_HEAVY_ELEMENT_FRACTION`], linear between, so that two neighbouring giants differ by
/// no step (decision-p14-phase-j, 1). Only a gas giant reads `fractions`.
///
/// Z is a function of mass as built (Thorngren et al.'s heavy elements), so the blend runs over
/// about 0.30–1 Jupiter mass; it is written in Z so that it follows if composition gains scatter.
#[must_use]
pub fn moment_of_inertia_factor(class: PlanetClass, fractions: &MassFractions) -> f64 {
    match class {
        PlanetClass::Rocky => ROCKY_MOMENT_OF_INERTIA,
        PlanetClass::Icy => ICY_MOMENT_OF_INERTIA,
        PlanetClass::SubNeptune | PlanetClass::IceGiant => ENVELOPED_MOMENT_OF_INERTIA,
        PlanetClass::GasGiant => {
            let z = 1.0 - fractions.envelope();
            let share = ((z - JUPITER_HEAVY_ELEMENT_FRACTION)
                / (SATURN_HEAVY_ELEMENT_FRACTION - JUPITER_HEAVY_ELEMENT_FRACTION))
                .clamp(0.0, 1.0);
            GAS_GIANT_MOMENT_OF_INERTIA
                + (SATURN_LIKE_MOMENT_OF_INERTIA - GAS_GIANT_MOMENT_OF_INERTIA) * share
        }
    }
}

/// The Love number k₂ and tidal quality factor Q of a body of class `class`, mass `mass`, radius
/// `radius` and mass fractions `fractions` (P14.T14.d, decision-backlog-1): what every reader of a
/// body's tides takes, [`SpinningBody::of_class`] for its locking time (P14.T14.b) and
/// [`derive_body`](crate::planetary::derive::derive_body) for its moon limit (P14.T15).
///
/// - `Rocky` and `Icy`: [`ROCKY_LOVE_NUMBER`] and [`ROCKY_TIDAL_Q`], 0.3 and 100.
/// - `GasGiant`: [`GIANT_LOVE_NUMBER`] and [`GIANT_TIDAL_Q`], 0.4 and 10⁵.
/// - `SubNeptune` and `IceGiant`: [`enveloped_love_number`] and [`ENVELOPED_TIDAL_Q`],
///   1.03 × 10⁴. Under a hydrogen and helium envelope the tide is raised on the core, seen from
///   the envelope's top, and the body dissipates as the ice giants do, not as a solid rocky body.
///   The two classes are one structure here, split by mass alone at
///   [`ICE_GIANT_MASS`](crate::planetary::params::ICE_GIANT_MASS), so they share one function and
///   their tides take no step there.
///
/// Only the enveloped classes read `mass`, `radius` and `fractions`. Both numbers are
/// dimensionless: Q is 100, 1.03 × 10⁴ or 10⁵, and k₂ is positive for every class a body can take.
/// An enveloped class holds at least
/// [`THIN_ENVELOPE_FRACTION`](crate::planetary::params::THIN_ENVELOPE_FRACTION) of envelope on a
/// core of more than half its mass, so its core's fractional radius α is positive. An enveloped
/// body's modified quality factor Q′ = 3Q ÷ 2k₂ runs from about 2.5 × 10⁴ under a 0.1% envelope to
/// about 5 × 10⁵, and is 1–4.6 × 10⁵ under 7–25% of envelope. It is 1.6 × 10⁵ for the generated
/// Uranus and 1.3 × 10⁵ for Neptune.
///
/// Held until the 21 → 22 batch's bump: below generator version 22 the generator gives every
/// class its earlier pair instead (see the [module](self) documentation).
#[must_use]
pub fn tides(
    class: PlanetClass,
    mass: EarthMasses,
    radius: EarthRadii,
    fractions: &MassFractions,
) -> (f64, f64) {
    match class {
        PlanetClass::Rocky | PlanetClass::Icy => (ROCKY_LOVE_NUMBER, ROCKY_TIDAL_Q),
        PlanetClass::SubNeptune | PlanetClass::IceGiant => (
            enveloped_love_number(mass, radius, fractions),
            ENVELOPED_TIDAL_Q,
        ),
        PlanetClass::GasGiant => (GIANT_LOVE_NUMBER, GIANT_TIDAL_Q),
    }
}

/// The generator version from which every reader of a body's tides takes [`tides`]: 22, the
/// 21 → 22 batch's bump (P14.T14.d; plan 14, "Order and parallelism", the bump plan).
///
/// P14.T14.d is built first in that batch, whose goldens move once, at its bump in P14.T48.e.
/// Below it [`tides_in_force`] gives every class its earlier pair, so that no golden moves before
/// the batch's wiring, and from it [`tides`]. The bump removes this hold
/// (`the_envelope_s_tides_are_held_only_until_the_batch_s_bump`).
pub(crate) const ENVELOPED_TIDES_VERSION: GeneratorVersion = GeneratorVersion::new(22);

/// The k₂ and Q the generator gives a body of class `class`, mass `mass`, radius `radius` and mass
/// fractions `fractions` (P14.T14.d, held): [`tides`] from [`ENVELOPED_TIDES_VERSION`], and below
/// it the class's earlier pair, a rocky body's for `Rocky`, `Icy` and `SubNeptune` and a giant's
/// for `IceGiant` and `GasGiant` (P14.T14.b; P14.T16.a as built), bit for bit what version 21
/// gives.
#[must_use]
pub(crate) fn tides_in_force(
    class: PlanetClass,
    mass: EarthMasses,
    radius: EarthRadii,
    fractions: &MassFractions,
) -> (f64, f64) {
    if GENERATOR_VERSION >= ENVELOPED_TIDES_VERSION {
        tides(class, mass, radius, fractions)
    } else {
        match class {
            PlanetClass::Rocky | PlanetClass::Icy | PlanetClass::SubNeptune => {
                (ROCKY_LOVE_NUMBER, ROCKY_TIDAL_Q)
            }
            PlanetClass::IceGiant | PlanetClass::GasGiant => (GIANT_LOVE_NUMBER, GIANT_TIDAL_Q),
        }
    }
}

/// The fluid Love number k₂ of a body of mass `mass` and radius `radius` under a hydrogen and
/// helium envelope, with mass fractions `fractions` (P14.T14.d): [`love_number_under_envelope`]
/// at α = `R_core` ÷ R, of its envelope fraction f and of the water fraction beneath it,
/// w = water ÷ (1 − f).
///
/// `R_core` is [`radius_zeng`] of the mass beneath the envelope, M (1 − f), and of the core read
/// from the fractions: iron ÷ (iron + rock) of iron in its rock and iron, or Earth's
/// [`EARTH_CORE_MASS_FRACTION`] if it holds neither, and w of water. That is the core
/// [`radius_with_envelope`](crate::planetary::derive::envelope::radius_with_envelope) lays the
/// envelope on, so α is the generator's own. It is held at 1 at most, and is 0, as then is k₂, for
/// a body with nothing beneath its envelope or a radius that is not positive and finite. Zeng et
/// al.'s curves end at 32 M⊕, and a heavier core's radius extrapolates their last interval.
#[must_use]
pub fn enveloped_love_number(
    mass: EarthMasses,
    radius: EarthRadii,
    fractions: &MassFractions,
) -> f64 {
    let (alpha, water) = core_beneath_envelope(mass, radius, fractions);
    love_number_under_envelope(alpha, fractions.envelope(), water)
}

/// The core of fractional radius α = `R_core` ÷ R beneath a body's envelope, and its water mass
/// fraction w, as [`enveloped_love_number`] reads them.
#[must_use]
fn core_beneath_envelope(
    mass: EarthMasses,
    radius: EarthRadii,
    fractions: &MassFractions,
) -> (f64, f64) {
    let beneath = 1.0 - fractions.envelope();
    let core_mass = mass.value() * beneath;
    let r = radius.value();
    let measurable = core_mass > 0.0 && core_mass.is_finite() && r > 0.0 && r.is_finite();
    if !measurable {
        return (0.0, 0.0);
    }
    let water = (fractions.water() / beneath).clamp(0.0, 1.0);
    let heavy = fractions.iron() + fractions.rock();
    let iron_share = if heavy > 0.0 {
        fractions.iron() / heavy
    } else {
        EARTH_CORE_MASS_FRACTION
    };
    let core = CoreComposition::from_fractions(iron_share, water);
    let alpha = radius_zeng(EarthMasses::new(core_mass), core).value() / r;
    (alpha.min(1.0), water)
}

/// The fluid Love number k₂ of a body whose core, of fractional radius `alpha` = `R_core` ÷ R and
/// water mass fraction `core_water_fraction`, lies under a hydrogen and helium envelope of
/// `envelope_fraction` of its mass, each in 0–1 (P14.T14.d; decision-backlog-1 as
/// science-p14-tides refits it): k₂ = 0.9 (1 − 0.39 w) α⁵ + 0.62 f α, dimensionless, at most 1.21
/// for an envelope of less than half the mass, and positive for any envelope on a core of positive
/// radius.
///
/// - The first term is the core's own fluid tide seen from the envelope's top. Under a massless
///   envelope a core of Love number `k_core` gives exactly `k_core` α⁵ at the outer radius, since
///   the core's induced potential falls as r⁻³ and the tide grows as r² (decision-backlog-1,
///   §1.2). [`ENVELOPED_CORE_LOVE_NUMBER`] is `k_core` and [`ENVELOPED_CORE_WATER_SOFTENING`] the
///   softening a water layer gives it.
/// - The second term, [`ENVELOPE_LOVE_COEFFICIENT`] f α, is the envelope's own response, which
///   matters from a few per cent of the mass: linear in the envelope's mass, and weaker the further
///   an envelope spreads above its core.
///
/// The law is this plan's fit to science-p14-tides's integration of Clairaut's equation in Radau's
/// form, checked against the degree-2 potential equation, which give the same linear, hydrostatic
/// k₂ as the Tₙ equation of Zharkov and Trubitsyn (1978) that Kramm et al. (2011, A&A 528, A18, §2,
/// eq. 3) use. It integrated the generator's own bodies: Seager et al.'s (2007) iron, `MgSiO₃` and
/// water, layered by the fractions (water on Earth-like rock and iron) and laid at
/// [`radius_zeng`]'s radius, under n = 1 and n = 2 polytropes of hydrogen and helium reaching
/// [`radius_with_envelope`](crate::planetary::derive::envelope::radius_with_envelope)'s radius,
/// over 1.6–80 M⊕, envelopes of 0.1–45%, 0.1–1,000 F⊕ and 1–10 Gyr. Against the geometric mean of
/// the two envelopes, for the bodies the generator makes (within 2σ of Chen and Kipping's radius),
/// it lies within 0.92–1.15 below 2% of envelope, 0.93–1.05 from 2% to 5%, and 0.79–1.16 beyond,
/// where the envelope's equation of state alone spans a factor of up to 3.3. For seventeen
/// generated bodies, the Solar System's ice giants among them, it lies within 0.92–1.05. A
/// homogeneous fluid body's k₂ is 1.5 and an n = 1 polytrope's 0.52 (Kramm et al. 2011, §2.2).
/// GJ 436b's metal-free envelope models give 0.02–0.2 (Nettelmann et al. 2010, A&A 523, A26), and
/// its models by interior 0.055–0.160 (Padovan et al. 2018, A&A 620, A178, §4.1), where this law
/// gives 0.067 for the generator's GJ 436b.
///
/// The water term is fitted at the one water fraction the composition solve lays an envelope on,
/// [`OUTER_WATER_CAP`](crate::planetary::params::OUTER_WATER_CAP). The integration's softening is
/// not linear in w (a core 25% water takes 0.67–0.72 of a dry core's k₂, less than one 53.9% water
/// takes), so another water fraction needs a refit. The core's own k₂ also falls with its mass,
/// from 0.99 at 1.6 M⊕ to 0.74 at 30 M⊕ dry, which the constant 0.9 averages.
///
/// # Panics
///
/// In debug builds, if an argument lies outside 0–1 or is NaN.
///
/// # Examples
///
/// The generated Uranus, a core 54% water at 0.618 of its radius under an envelope of 8.2% of its
/// mass, takes k₂ = 0.0955, so that with Q = 1.03 × 10⁴ its modified quality factor
/// Q′ = 3Q ÷ 2k₂ is 1.6 × 10⁵, inside Tittemore and Wisdom's 1.6–5.6 × 10⁵:
///
/// ```
/// use hyperion_sim::planetary::derive::rotation::love_number_under_envelope;
/// use hyperion_sim::planetary::params::ENVELOPED_TIDAL_Q;
///
/// let k2 = love_number_under_envelope(0.618, 0.082, 0.539);
/// assert!((k2 - 0.0955).abs() < 0.001);
/// let q_prime = 3.0 * ENVELOPED_TIDAL_Q / (2.0 * k2);
/// assert!((1.6e5..5.6e5).contains(&q_prime));
/// ```
#[must_use]
pub fn love_number_under_envelope(
    alpha: f64,
    envelope_fraction: f64,
    core_water_fraction: f64,
) -> f64 {
    for (name, x) in [
        ("α", alpha),
        ("an envelope fraction", envelope_fraction),
        ("a water fraction", core_water_fraction),
    ] {
        debug_assert!((0.0..=1.0).contains(&x), "{name} lies in 0 to 1, got {x}");
    }
    let core = ENVELOPED_CORE_LOVE_NUMBER
        * (1.0 - ENVELOPED_CORE_WATER_SOFTENING * core_water_fraction)
        * math::powi(alpha, 5);
    core + ENVELOPE_LOVE_COEFFICIENT * envelope_fraction * alpha
}

/// The time the tides of a primary of mass `primary_mass` take to despin `body`, spinning with
/// period `spin_period` on an orbit of semi-major axis `a` (P14.T14.b): Gladman et al.'s (1996,
/// Icarus 122, 166, eq. 9) τ = ω a⁶ I Q ÷ (3 G M² k₂ R⁵), with ω = 2π ÷ `spin_period` and
/// I = α m R².
///
/// # Examples
///
/// The Moon, from a 15-hour day at its present distance, locks within a few million years (2.6
/// Myr with a rocky body's k₂ and Q):
///
/// ```
/// use hyperion_sim::planetary::derive::rotation::{SpinningBody, tidal_locking_time};
/// use hyperion_sim::planetary::params::{ROCKY_LOVE_NUMBER, ROCKY_MOMENT_OF_INERTIA, ROCKY_TIDAL_Q};
/// use hyperion_sim::units::consts::EARTH_MASS_KG;
/// use hyperion_sim::units::{Kilograms, Metres, Seconds};
///
/// let moon = SpinningBody::new(
///     Kilograms::new(7.346e22),
///     Metres::new(1.7374e6),
///     ROCKY_MOMENT_OF_INERTIA,
///     ROCKY_LOVE_NUMBER,
///     ROCKY_TIDAL_Q,
/// )?;
/// let tau = tidal_locking_time(&moon, Seconds::new(54_000.0), Metres::new(3.844e8), Kilograms::new(EARTH_MASS_KG));
/// assert!(tau.value() < 1e7 * 3.156e7);
/// # Ok::<(), hyperion_sim::planetary::derive::rotation::BuildSpinningBodyError>(())
/// ```
#[must_use]
pub fn tidal_locking_time(
    body: &SpinningBody,
    spin_period: Seconds,
    a: Metres,
    primary_mass: Kilograms,
) -> Seconds {
    let omega = TAU / spin_period.value();
    let (r, a, m_p) = (body.radius.value(), a.value(), primary_mass.value());
    let inertia = body.moment_factor * body.mass.value() * r * r;
    Seconds::new(
        omega * math::powi(a, 6) * inertia * body.tidal_q
            / (3.0 * GRAVITATIONAL_CONSTANT * m_p * m_p * body.love_number * math::powi(r, 5)),
    )
}

/// The rotation a locked body settles into (P14.T14.b).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SpinOrbitResonance {
    /// One rotation per orbit: synchronous, as the Moon.
    Synchronous,
    /// Three rotations per two orbits, as Mercury.
    ThreeToTwo,
}

impl SpinOrbitResonance {
    /// The state of a body locked on an orbit of eccentricity `e`: 3:2 above
    /// [`SPIN_ORBIT_RESONANCE_ECCENTRICITY`], synchronous at or below it.
    #[must_use]
    pub fn for_eccentricity(e: f64) -> Self {
        if e > SPIN_ORBIT_RESONANCE_ECCENTRICITY {
            Self::ThreeToTwo
        } else {
            Self::Synchronous
        }
    }

    /// Rotations and orbits per cycle: (1, 1) or (3, 2).
    #[must_use]
    pub const fn ratio(self) -> (u32, u32) {
        match self {
            Self::Synchronous => (1, 1),
            Self::ThreeToTwo => (3, 2),
        }
    }
}

/// Whether a body is locked at a time, and how (P14.T14.b).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SpinState {
    /// Spinning down (or up) towards the locked rate.
    Despinning,
    /// Locked, in the resonance given.
    Locked(SpinOrbitResonance),
}

/// What [`RotationLaw::new`] reads: the spin, its tides' time, the system's age and the orbit it
/// locks to (P14.T14.b).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RotationInputs {
    /// The primordial rotation period, positive and finite.
    pub primordial_period: Seconds,
    /// The despinning time τ ([`tidal_locking_time`]), from the system's birth; positive, or
    /// infinite for a body that never locks.
    pub locking_time: Seconds,
    /// The system's age at the epoch.
    pub age_at_epoch: Years,
    /// The body's orbit about its primary, the one its rotation locks to.
    pub orbit: KeplerElements,
    /// The prime meridian's angle at which it faces the primary at pericentre, rad: the direction
    /// to the primary then, measured about the pole as the rotation angle is
    /// ([`frames`](crate::planetary::frames)).
    pub sub_primary_angle: Radians,
    /// The rotation angle at the epoch of a body not locked by then (the drawn phase), rad.
    pub phase_at_epoch: Radians,
}

/// A body's rotation angle as a closed form of time (P14.T14.b): the angle W(t) of its prime
/// meridian about its pole, and its spin rate.
///
/// In the system's age s, the rate is ω(s) = ω₀ + (`ω_L` − ω₀) s ÷ τ until the locking age τ and
/// `ω_L` after, `ω_L` the orbit's mean motion n, or 1.5 n in the 3:2 state. The angle is anchored where it
/// is known exactly:
///
/// - **Locked**, from τ on: W = `W_p` + p `M_q`(t) for a p:q state, where `W_p` is the angle facing
///   the primary at pericentre and `M_q` the mean anomaly of an orbit of q periods, both reduced
///   exactly from the clock ([`KeplerElements::mean_anomaly_at`]), so that the prime meridian
///   faces the primary at every pericentre of a synchronous body, and of a 3:2 body at every other
///   one, facing away at the rest, as Mercury's does.
/// - **Before τ**, for a body locked by the epoch: the locked angle at τ less the integral of the
///   rate from the time to τ, so the angle is continuous at τ.
/// - **Before τ**, for a body not locked at the epoch: the drawn phase at the epoch plus the
///   integral of the rate since, plus the phase δ ∈ `[−π, π)` that the capture into the resonance
///   takes up, spread as δ (Δ ÷ (τ − `s_e`))² from the epoch to τ, so the angle is the drawn phase at
///   the epoch and continuous at τ, and the rate differs from the law's by 2δΔ ÷ (τ − `s_e`)², under
///   10⁻¹⁵ rad s⁻¹ for a lock more than a year off.
///
/// A body whose τ is not finite or lies beyond the clock's range never locks, and keeps the drawn
/// phase and the law's rate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RotationLaw {
    initial_rate: f64,
    locked_rate: f64,
    age_at_epoch: f64,
    locking_age: f64,
    resonance: SpinOrbitResonance,
    locks_at: Option<UniverseTime>,
    clock: KeplerElements,
    sub_primary_angle: f64,
    phase_at_epoch: f64,
    capture_phase: f64,
}

impl RotationLaw {
    /// The rotation law of `inputs`.
    ///
    /// # Errors
    ///
    /// [`BuildRotationLawError::PeriodNotPositive`] for a primordial period that is not positive
    /// and finite, and [`BuildRotationLawError::LockingTimeNotPositive`] for a locking time that
    /// is not positive (an infinite one is allowed).
    ///
    /// # Panics
    ///
    /// Never: the doubled period of a valid orbit is a valid orbit's.
    pub fn new(inputs: &RotationInputs) -> Result<Self, BuildRotationLawError> {
        let period = inputs.primordial_period.value();
        if !(period.is_finite() && period > 0.0) {
            return Err(BuildRotationLawError::PeriodNotPositive);
        }
        let tau = inputs.locking_time.value();
        if tau.is_nan() || tau <= 0.0 {
            return Err(BuildRotationLawError::LockingTimeNotPositive);
        }
        let orbit = inputs.orbit;
        let resonance = SpinOrbitResonance::for_eccentricity(orbit.eccentricity().value());
        let (p, q) = resonance.ratio();
        let n = TAU / orbit.period().value();
        let clock = if q == 1 {
            orbit
        } else {
            KeplerElements::from_period(
                Seconds::new(orbit.period().value() * f64::from(q)),
                orbit.gravitational_parameter(),
                orbit.eccentricity(),
                *orbit.orientation(),
                Radians::new(orbit.mean_anomaly_at_epoch().value() / f64::from(q)),
            )
            .expect("a valid orbit's period, doubled, is a valid orbit's")
        };
        let age_at_epoch = Seconds::from(inputs.age_at_epoch).value();
        let locks_at = if tau.is_finite() {
            Span::from_seconds_f64(tau - age_at_epoch)
                .and_then(|span| UniverseTime::EPOCH.checked_add(span))
        } else {
            None
        };
        let mut law = Self {
            initial_rate: TAU / period,
            locked_rate: n * f64::from(p) / f64::from(q),
            age_at_epoch,
            locking_age: tau,
            resonance,
            locks_at,
            clock,
            sub_primary_angle: inputs.sub_primary_angle.value(),
            phase_at_epoch: reduce(inputs.phase_at_epoch.value()),
            capture_phase: 0.0,
        };
        if let Some(at) = locks_at {
            let d = tau - age_at_epoch;
            if d > 0.0 {
                let free = law.phase_at_epoch + law.swept(0.0, d);
                law.capture_phase = centred(law.locked_angle(at) - free);
            }
        }
        Ok(law)
    }

    /// The system age at which the body locks, the locking time τ; infinite for one that never
    /// does.
    #[must_use]
    pub const fn locking_age(&self) -> Seconds {
        Seconds::new(self.locking_age)
    }

    /// When the body locks, if within the clock's range.
    #[must_use]
    pub const fn locks_at(&self) -> Option<UniverseTime> {
        self.locks_at
    }

    /// The state the body locks into.
    #[must_use]
    pub const fn resonance(&self) -> SpinOrbitResonance {
        self.resonance
    }

    /// The primordial spin rate ω₀, rad s⁻¹.
    #[must_use]
    pub const fn initial_rate(&self) -> f64 {
        self.initial_rate
    }

    /// The locked spin rate, rad s⁻¹: n, or 1.5 n in the 3:2 state.
    #[must_use]
    pub const fn locked_rate(&self) -> f64 {
        self.locked_rate
    }

    /// Every parameter [`angle_at`](Self::angle_at) reads (P14.T46.b), so that the wire carries
    /// them and a client evaluates W(t) by the same closed forms (see [`RotationLawParts`]).
    #[must_use]
    pub fn parts(&self) -> RotationLawParts {
        RotationLawParts {
            initial_rate: self.initial_rate,
            locked_rate: self.locked_rate,
            age_at_epoch: Seconds::new(self.age_at_epoch),
            locking_age: self
                .locking_age
                .is_finite()
                .then_some(Seconds::new(self.locking_age)),
            locks_at: self.locks_at,
            resonance: self.resonance,
            clock_period: self.clock.period(),
            clock_mean_anomaly_at_epoch: self.clock.mean_anomaly_at_epoch(),
            sub_primary_angle: Radians::new(self.sub_primary_angle),
            phase_at_epoch: Radians::new(self.phase_at_epoch),
            capture_phase: Radians::new(self.capture_phase),
        }
    }

    /// Whether the body is locked at `t`.
    #[must_use]
    pub fn state_at(&self, t: UniverseTime) -> SpinState {
        match self.locks_at {
            Some(at) if t >= at => SpinState::Locked(self.resonance),
            Some(_) | None => SpinState::Despinning,
        }
    }

    /// The spin rate at `t`, rad s⁻¹, from the law (without the capture's phase, see the type's
    /// documentation).
    #[must_use]
    pub fn rate_at(&self, t: UniverseTime) -> f64 {
        match self.state_at(t) {
            SpinState::Locked(_) => self.locked_rate,
            SpinState::Despinning => self.rate_at_age(self.age_at_epoch + offset(t)),
        }
    }

    /// The rotation angle W at `t`, rad, in `[0, 2π)`.
    #[must_use]
    pub fn angle_at(&self, t: UniverseTime) -> Radians {
        let dt = offset(t);
        let angle = match self.locks_at {
            None => self.phase_at_epoch + self.swept(0.0, dt),
            Some(at) if t >= at => self.locked_angle(t),
            Some(at) => {
                let d = self.locking_age - self.age_at_epoch;
                if d > 0.0 {
                    let share = dt.max(0.0) / d;
                    self.phase_at_epoch + self.swept(0.0, dt) + self.capture_phase * share * share
                } else {
                    self.locked_angle(at) - self.swept(dt, d)
                }
            }
        };
        Radians::new(reduce(angle))
    }

    /// The locked angle at `t`: `W_p` + p `M_q`(t).
    fn locked_angle(&self, t: UniverseTime) -> f64 {
        let (p, _) = self.resonance.ratio();
        self.sub_primary_angle + f64::from(p) * self.clock.mean_anomaly_at(t).value()
    }

    /// The spin rate at system age `s` seconds on the law, before any lock.
    fn rate_at_age(&self, s: f64) -> f64 {
        if self.locking_age.is_finite() {
            let share = (s / self.locking_age).clamp(0.0, 1.0);
            self.initial_rate + (self.locked_rate - self.initial_rate) * share
        } else {
            self.initial_rate
        }
    }

    /// The angle the law's rate sweeps from `from` to `to` seconds after the epoch, both before
    /// the lock: ω(`s_e` + from) Δ + (`ω_L` − ω₀) Δ² ÷ 2τ, exact for a linear rate.
    fn swept(&self, from: f64, to: f64) -> f64 {
        let span = to - from;
        let start = self.rate_at_age(self.age_at_epoch + from);
        if self.locking_age.is_finite() {
            start * span
                + (self.locked_rate - self.initial_rate) * span * span / (2.0 * self.locking_age)
        } else {
            start * span
        }
    }
}

/// The parameters of a [`RotationLaw`], as [`RotationLaw::parts`] gives them (P14.T46.b): what a
/// client needs to evaluate the rotation angle at any time without the orbit.
///
/// With s the seconds from the epoch to t, d = τ − `s_e` and Δ = max(s, 0):
///
/// - **Locked**, at or after `locks_at`: W = `sub_primary_angle` + p M(t), with p 1 or 3 for the
///   [`resonance`](Self::resonance), and M(t) the clock's mean anomaly,
///   `clock_mean_anomaly_at_epoch` + 2π s ÷ `clock_period`, the fraction of a period taken from
///   the clock's whole seconds modulo the period before it rounds
///   ([`KeplerElements::mean_anomaly_at`]), reduced into `[0, 2π)`.
/// - **No lock in the clock's range** (`locks_at` `None`): W = `phase_at_epoch` + the swept angle from 0 to s.
/// - **Before the lock**, d > 0: W = `phase_at_epoch` + the swept angle from 0 to s +
///   `capture_phase` (Δ ÷ d)².
/// - **Before the lock**, d ≤ 0 (a body locked by the epoch, read before its lock): W = the
///   locked angle at `locks_at` less the swept angle from s to d.
///
/// The swept angle from a to b is ω(`s_e` + a) (b − a) + (`ω_L` − ω₀) (b − a)² ÷ 2τ, with
/// ω(x) = ω₀ + (`ω_L` − ω₀) clamp(x ÷ τ, 0, 1), or ω₀ (b − a) where `locking_age` is `None`. W is
/// reduced into `[0, 2π)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RotationLawParts {
    /// The primordial spin rate ω₀, rad s⁻¹.
    pub initial_rate: f64,
    /// The locked spin rate `ω_L`, rad s⁻¹: n, or 1.5 n in the 3:2 state.
    pub locked_rate: f64,
    /// The system's age at the epoch, `s_e`.
    pub age_at_epoch: Seconds,
    /// The system age at which the body locks, τ; `None` for a body that never does.
    pub locking_age: Option<Seconds>,
    /// When the body locks, if within the clock's range.
    pub locks_at: Option<UniverseTime>,
    /// The state the body locks into.
    pub resonance: SpinOrbitResonance,
    /// The period of the locked angle's clock: the orbit's for a synchronous body, two orbits for
    /// a 3:2 one.
    pub clock_period: Seconds,
    /// The clock's mean anomaly at the epoch, rad.
    pub clock_mean_anomaly_at_epoch: Radians,
    /// `W_p`, the angle at which the prime meridian faces the primary at pericentre, rad.
    pub sub_primary_angle: Radians,
    /// The drawn rotation angle at the epoch, rad, in `[0, 2π)`.
    pub phase_at_epoch: Radians,
    /// δ, the phase the capture into the resonance takes up before the lock, rad, in `[−π, π)`.
    pub capture_phase: Radians,
}

/// A [`RotationLaw`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildRotationLawError {
    /// The primordial period was not positive and finite.
    PeriodNotPositive,
    /// The locking time was not positive.
    LockingTimeNotPositive,
}

impl fmt::Display for BuildRotationLawError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::PeriodNotPositive => "a primordial rotation period must be positive and finite",
            Self::LockingTimeNotPositive => "a locking time must be positive",
        })
    }
}

impl Error for BuildRotationLawError {}

/// What [`BodyRotation::derive`] reads of a body (P14.T14): its spin draws, how its obliquity is
/// drawn, its class, mass and radius, its orbit about its primary and the primary's mass, and its
/// system's age at the epoch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpinInputs {
    /// The body's draws on `planet.spin` ([`SpinDraws::for_body`]).
    pub draws: SpinDraws,
    /// Isotropic for a body that had a giant impact, Rayleigh otherwise.
    pub obliquity_law: ObliquityLaw,
    /// The body's class, which sets its period law, moment of inertia and tides.
    pub class: PlanetClass,
    /// The body's mass fractions, which set a gas giant's moment of inertia
    /// ([`moment_of_inertia_factor`]) and the tides of a body under an envelope ([`tides`]).
    pub fractions: MassFractions,
    /// The body's mass.
    pub mass: Kilograms,
    /// The body's radius.
    pub radius: Metres,
    /// The body's orbit about its primary, the one its rotation locks to.
    pub orbit: KeplerElements,
    /// The primary's mass: the star's or pair's for a planet, the planet's for a moon.
    pub primary_mass: Kilograms,
    /// The system's age at the epoch.
    pub age_at_epoch: Years,
}

/// A body's rotation (P14.T14): its spin as drawn, its locking time, and its body-fixed frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BodyRotation {
    obliquity_law: ObliquityLaw,
    primordial_period: Seconds,
    obliquity: Radians,
    locking_time: Seconds,
    frame: BodyFixedFrame,
}

impl BodyRotation {
    /// The rotation of the body `inputs` describe (P14.T14.a–c): its primordial period
    /// ([`primordial_period`], floored at its [`breakup_period`]), its obliquity ([`obliquity`]),
    /// its locking time ([`tidal_locking_time`] of that period at the tides and moment of inertia
    /// [`SpinningBody::of_class`] gives it) and its frame ([`BodyFixedFrame::new`]).
    ///
    /// # Errors
    ///
    /// [`DeriveRotationError::Body`] for a mass or radius that is not positive and finite, and
    /// [`DeriveRotationError::Law`] for a locking time that is not positive, which a primary of
    /// positive mass never gives.
    pub fn derive(inputs: &SpinInputs) -> Result<Self, DeriveRotationError> {
        let body =
            SpinningBody::of_class(inputs.mass, inputs.radius, inputs.class, &inputs.fractions)?;
        let drawn = primordial_period(SpinFamily::of(inputs.class), inputs.draws.period_rank);
        let floor = breakup_period(inputs.mass, inputs.radius);
        let period = Seconds::new(drawn.value().max(floor.value()));
        let tilt = obliquity(inputs.obliquity_law, inputs.draws.obliquity_rank);
        let locking_time = tidal_locking_time(
            &body,
            period,
            inputs.orbit.semi_major_axis(),
            inputs.primary_mass,
        );
        let frame = BodyFixedFrame::new(
            &inputs.orbit,
            &FrameSpin {
                obliquity: tilt,
                pole_azimuth: inputs.draws.pole_azimuth,
                primordial_period: period,
                locking_time,
                phase_at_epoch: inputs.draws.phase,
            },
            inputs.age_at_epoch,
        )?;
        Ok(Self {
            obliquity_law: inputs.obliquity_law,
            primordial_period: period,
            obliquity: tilt,
            locking_time,
            frame,
        })
    }

    /// How the obliquity was drawn.
    #[must_use]
    pub const fn obliquity_law(&self) -> ObliquityLaw {
        self.obliquity_law
    }

    /// The primordial rotation period, at least the break-up period.
    #[must_use]
    pub const fn primordial_period(&self) -> Seconds {
        self.primordial_period
    }

    /// The obliquity, rad, in `[0, π]`: the angle between the spin axis and the orbit's normal.
    #[must_use]
    pub const fn obliquity(&self) -> Radians {
        self.obliquity
    }

    /// The despinning time τ from the system's birth.
    #[must_use]
    pub const fn locking_time(&self) -> Seconds {
        self.locking_time
    }

    /// The body-fixed frame.
    #[must_use]
    pub const fn frame(&self) -> &BodyFixedFrame {
        &self.frame
    }
}

/// [`BodyRotation::derive`] could not derive a rotation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeriveRotationError {
    /// The body's mass or radius was not positive and finite.
    Body(BuildSpinningBodyError),
    /// The rotation law could not be built.
    Law(BuildRotationLawError),
}

impl fmt::Display for DeriveRotationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Body(_) => f.write_str("a spinning body could not be built"),
            Self::Law(_) => f.write_str("a rotation law could not be built"),
        }
    }
}

impl Error for DeriveRotationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Body(e) => Some(e),
            Self::Law(e) => Some(e),
        }
    }
}

impl From<BuildSpinningBodyError> for DeriveRotationError {
    fn from(e: BuildSpinningBodyError) -> Self {
        Self::Body(e)
    }
}

impl From<BuildRotationLawError> for DeriveRotationError {
    fn from(e: BuildRotationLawError) -> Self {
        Self::Law(e)
    }
}

/// The seconds from the epoch to `t`, as closed forms take them.
fn offset(t: UniverseTime) -> f64 {
    t.since_epoch().as_seconds_f64()
}

/// `x` reduced into `[0, 2π)`; zero for a value that is not finite.
fn reduce(x: f64) -> f64 {
    if !x.is_finite() {
        return 0.0;
    }
    let r = x.rem_euclid(TAU);
    if r < TAU { r } else { 0.0 }
}

/// `x` reduced into `[−π, π)`.
fn centred(x: f64) -> f64 {
    let r = reduce(x + PI) - PI;
    if r < PI { r } else { -PI }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::stats::{ALPHA, assert_p_value, ks_one_sample, normal_cdf};

    use super::*;
    use crate::id::SystemId;
    use crate::orbit::{Eccentricity, Orientation};
    use crate::planetary::derive::composition::{
        SnowLineSide, giant_composition, giant_heavy_elements,
    };
    use crate::units::GravitationalParameter;
    use crate::units::consts::{
        EARTH_MASS_KG, EARTH_RADIUS_M, JUPITER_MASS_KG, METRES_PER_AU, SECONDS_PER_JULIAN_YEAR,
        SOLAR_MASS_KG,
    };

    const MYR: f64 = 1e6 * SECONDS_PER_JULIAN_YEAR;
    const GYR: f64 = 1e9 * SECONDS_PER_JULIAN_YEAR;

    /// A solid body's fractions, which no class but a gas giant reads.
    const SOLID: MassFractions = MassFractions::solid(0.3, 0.7, 0.0);

    fn body(mass: f64, radius: f64, class: PlanetClass) -> SpinningBody {
        SpinningBody::of_class(Kilograms::new(mass), Metres::new(radius), class, &SOLID).unwrap()
    }

    fn lock(b: &SpinningBody, a: f64, primary: f64) -> f64 {
        tidal_locking_time(
            b,
            ROCKY_PRIMORDIAL_PERIOD,
            Metres::new(a),
            Kilograms::new(primary),
        )
        .value()
    }

    fn orbit(a: f64, e: f64, primary: f64, orientation: Orientation, m0: f64) -> KeplerElements {
        KeplerElements::from_semi_major_axis(
            Metres::new(a),
            GravitationalParameter::new(GRAVITATIONAL_CONSTANT * primary),
            Eccentricity::new(e).unwrap(),
            orientation,
            Radians::new(m0),
        )
        .unwrap()
    }

    fn flat() -> Orientation {
        Orientation::new(Radians::ZERO, Radians::ZERO, Radians::ZERO).unwrap()
    }

    fn draws(n: u64) -> Vec<SpinDraws> {
        let system = SystemId::from_raw(0x0200_0800_2000_0000).unwrap();
        (0..n)
            .map(|i| {
                let id = BodyId::new(system, u16::try_from(i % 0xFFFF).unwrap());
                let seed = Seed::new(0x5714 + i / 0xFFFF);
                SpinDraws::for_body(seed, id)
            })
            .collect()
    }

    /// (a) 10⁵ primordial periods follow their log-normal laws.
    #[test]
    fn primordial_periods_are_log_normal() {
        let sample = draws(100_000);
        for family in [SpinFamily::Rocky, SpinFamily::Giant] {
            let median = family.median_period().value();
            let mut periods: Vec<f64> = sample
                .iter()
                .map(|d| primordial_period(family, d.period_rank()).value())
                .collect();
            let ks = ks_one_sample(&mut periods, |p| {
                normal_cdf(math::log10(p / median) / PRIMORDIAL_PERIOD_SCATTER_DEX)
            });
            assert_p_value(&format!("{family:?} periods"), ks.p_value, ALPHA);
        }
    }

    /// (a) 10⁵ obliquities follow their laws, isotropic and Rayleigh.
    #[test]
    fn obliquities_follow_their_laws() {
        let sample = draws(100_000);
        let mut isotropic: Vec<f64> = sample
            .iter()
            .map(|d| obliquity(ObliquityLaw::Isotropic, d.obliquity_rank()).value())
            .collect();
        let ks = ks_one_sample(&mut isotropic, |e| (1.0 - math::cos(e)) / 2.0);
        assert_p_value("isotropic obliquity", ks.p_value, ALPHA);
        let sigma = Radians::from(QUIET_OBLIQUITY_SCALE).value();
        let mut quiet: Vec<f64> = sample
            .iter()
            .map(|d| obliquity(ObliquityLaw::Rayleigh, d.obliquity_rank()).value())
            .collect();
        let ks = ks_one_sample(&mut quiet, |e| {
            1.0 - math::exp(-e * e / (2.0 * sigma * sigma))
        });
        assert_p_value("Rayleigh obliquity", ks.p_value, ALPHA);
        let mut azimuths: Vec<f64> = sample.iter().map(|d| d.pole_azimuth().value()).collect();
        let ks = ks_one_sample(&mut azimuths, |x| (x / TAU).clamp(0.0, 1.0));
        assert_p_value("pole azimuth", ks.p_value, ALPHA);
    }

    /// (b) The Moon, Io and Titan lock in under 100 Myr.
    #[test]
    fn the_regular_moons_lock_within_a_hundred_million_years() {
        let moon = body(7.346e22, 1.7374e6, PlanetClass::Rocky);
        let io = body(8.932e22, 1.8216e6, PlanetClass::Rocky);
        let titan = body(1.3452e23, 2.5747e6, PlanetClass::Icy);
        let saturn = 5.683e26;
        for (name, tau) in [
            ("Moon", lock(&moon, 3.844e8, EARTH_MASS_KG)),
            ("Io", lock(&io, 4.217e8, 1.898e27)),
            ("Titan", lock(&titan, 1.2219e9, saturn)),
        ] {
            assert!(tau < 100.0 * MYR, "{name} locks at {} Myr", tau / MYR);
        }
    }

    /// (b) Earth and Mars do not lock in 10 Gyr; Mercury does, into 3:2; a planet in the habitable
    /// zone of a 0.2 M☉ star locks in under 1 Gyr.
    #[test]
    fn the_terrestrial_planets_lock_where_they_should() {
        let sun = SOLAR_MASS_KG;
        let earth = body(EARTH_MASS_KG, EARTH_RADIUS_M, PlanetClass::Rocky);
        let mars = body(6.417e23, 3.3895e6, PlanetClass::Rocky);
        let mercury = body(3.301e23, 2.4397e6, PlanetClass::Rocky);
        assert!(lock(&earth, METRES_PER_AU, sun) > 10.0 * GYR);
        assert!(lock(&mars, 1.5237 * METRES_PER_AU, sun) > 10.0 * GYR);
        let mercury_tau = lock(&mercury, 0.3871 * METRES_PER_AU, sun);
        assert!(
            mercury_tau < 4.57 * GYR,
            "Mercury locks at {} Gyr",
            mercury_tau / GYR
        );
        assert_eq!(
            SpinOrbitResonance::for_eccentricity(0.2056),
            SpinOrbitResonance::ThreeToTwo
        );
        // L ≈ 0.0044 L☉ for 0.2 M☉ (the habitable zone's middle at about 0.066 au).
        let m_dwarf = 0.2 * sun;
        let a = (0.0044_f64).sqrt() * METRES_PER_AU;
        assert!(lock(&earth, a, m_dwarf) < GYR);
        // A giant's tides are weaker still: Jupiter never locks.
        let jupiter = body(JUPITER_MASS_KG, 7.1492e7, PlanetClass::GasGiant);
        assert!(lock(&jupiter, 5.2 * METRES_PER_AU, sun) > 10.0 * GYR);
    }

    /// The fractions of a giant of `earth_masses` by Thorngren et al.'s heavy elements (0.3–13
    /// `M_J`, the fit's range).
    fn giant(earth_masses: f64) -> MassFractions {
        giant_composition(EarthMasses::new(earth_masses), SnowLineSide::Beyond)
            .unwrap()
            .fractions()
    }

    /// The fractions of a body of heavy-element fraction `z` under a hydrogen and helium envelope.
    fn heavy(z: f64) -> MassFractions {
        MassFractions::of(CoreComposition::new(0.3, 0.5).unwrap(), 1.0 - z)
    }

    /// P14.T46.a (a): the heavy-element fractions of Jupiter and Saturn are those of
    /// `giant_heavy_elements`, to 10⁻¹².
    #[test]
    fn the_blend_s_heavy_element_fractions_follow_thorngren() {
        let jupiter = EarthMasses::from(crate::units::JupiterMasses::new(1.0));
        let z_j = giant_heavy_elements(jupiter).value() / jupiter.value();
        let z_s = giant_heavy_elements(EarthMasses::new(95.16)).value() / 95.16;
        assert!(
            (z_j - JUPITER_HEAVY_ELEMENT_FRACTION).abs() < 1e-12,
            "{z_j}"
        );
        assert!((z_s - SATURN_HEAVY_ELEMENT_FRACTION).abs() < 1e-12, "{z_s}");
    }

    /// P14.T46.a (a): a gas giant's factor is 0.25 at 1 `M_J`, 0.21 at Saturn's Z, continuous
    /// and monotone in Z between; the rocky, icy and enveloped constants are unchanged.
    ///
    /// Saturn's 95.16 M⊕ lies just below the heavy-element fit's 0.3 `M_J`, so its Z is set
    /// directly; the fit's own giants run from 0.3 `M_J`, where the factor is within 10⁻³ of 0.21.
    #[test]
    fn one_moment_of_inertia_blends_jupiter_to_saturn() {
        let by_z = |z: f64| moment_of_inertia_factor(PlanetClass::GasGiant, &heavy(z));
        let by_mass = |m: f64| moment_of_inertia_factor(PlanetClass::GasGiant, &giant(m));
        let jupiter = EarthMasses::from(crate::units::JupiterMasses::new(1.0)).value();
        assert!(
            (by_mass(jupiter) - 0.25).abs() < 1e-9,
            "{}",
            by_mass(jupiter)
        );
        assert!((by_mass(3.0 * jupiter) - 0.25).abs() < 1e-15);
        assert!((by_z(SATURN_HEAVY_ELEMENT_FRACTION) - 0.21).abs() < 1e-12);
        assert!((by_z(0.5) - 0.21).abs() < 1e-15);
        assert!(
            (by_mass(0.3 * jupiter) - 0.21).abs() < 1e-3,
            "{}",
            by_mass(0.3 * jupiter)
        );
        let mut previous = by_z(0.0);
        for i in 1..=1_000 {
            let z = f64::from(i) * 1e-3;
            let f = by_z(z);
            assert!(f <= previous, "not monotone at Z {z}");
            assert!(previous - f < 1e-3, "a step of {} at Z {z}", previous - f);
            previous = f;
        }
        let mut previous = by_mass(0.3 * jupiter);
        let mut m = 0.3 * jupiter;
        while m < 2.0 * jupiter {
            m *= 1.001;
            let f = by_mass(m);
            assert!(f >= previous, "not monotone at {m} M⊕");
            assert!(f - previous < 2e-4, "a step of {} at {m} M⊕", f - previous);
            previous = f;
        }
        for (class, expected) in [
            (PlanetClass::Rocky, 0.33),
            (PlanetClass::Icy, 0.34),
            (PlanetClass::SubNeptune, 0.23),
            (PlanetClass::IceGiant, 0.23),
        ] {
            assert!((moment_of_inertia_factor(class, &SOLID) - expected).abs() < 1e-15);
            assert!((moment_of_inertia_factor(class, &heavy(0.29)) - expected).abs() < 1e-15);
        }
    }

    fn law(tau: f64, age: f64, e: f64) -> RotationLaw {
        RotationLaw::new(&RotationInputs {
            primordial_period: Seconds::new(54_000.0),
            locking_time: Seconds::new(tau),
            age_at_epoch: Years::from(Seconds::new(age)),
            orbit: orbit(0.05 * METRES_PER_AU, e, 0.3 * SOLAR_MASS_KG, flat(), 1.0),
            sub_primary_angle: Radians::new(2.0),
            phase_at_epoch: Radians::new(0.5),
        })
        .unwrap()
    }

    fn at(seconds: f64) -> UniverseTime {
        UniverseTime::EPOCH
            .checked_add(Span::from_seconds_f64(seconds).unwrap())
            .unwrap()
    }

    /// W at `s` seconds from the epoch, from `parts` alone, by a plain re-implementation of the
    /// closed forms [`RotationLawParts`] documents: what a client's twin computes.
    fn angle_from_parts(parts: &RotationLawParts, s: f64) -> f64 {
        let (w0, wl) = (parts.initial_rate, parts.locked_rate);
        let se = parts.age_at_epoch.value();
        let tau = parts.locking_age.map(Seconds::value);
        let rate = |x: f64| tau.map_or(w0, |tau| w0 + (wl - w0) * (x / tau).clamp(0.0, 1.0));
        let swept = |a: f64, b: f64| {
            let span = b - a;
            rate(se + a) * span + tau.map_or(0.0, |tau| (wl - w0) * span * span / (2.0 * tau))
        };
        let p = match parts.resonance {
            SpinOrbitResonance::Synchronous => 1.0,
            SpinOrbitResonance::ThreeToTwo => 3.0,
        };
        let locked = |s: f64| {
            let period = parts.clock_period.value();
            let m = parts.clock_mean_anomaly_at_epoch.value() + TAU * s.rem_euclid(period) / period;
            parts.sub_primary_angle.value() + p * m.rem_euclid(TAU)
        };
        let w = match parts.locks_at.map(|t| t.since_epoch().as_seconds_f64()) {
            None => parts.phase_at_epoch.value() + swept(0.0, s),
            Some(lock) if s >= lock => locked(s),
            Some(lock) => {
                let d = tau.expect("a body that locks has a locking age") - se;
                if d > 0.0 {
                    let share = s.max(0.0) / d;
                    parts.phase_at_epoch.value()
                        + swept(0.0, s)
                        + parts.capture_phase.value() * share * share
                } else {
                    locked(lock) - swept(s, d)
                }
            }
        };
        w.rem_euclid(TAU)
    }

    /// P14.T46.b (b): W from [`RotationLaw::parts`] by a plain re-implementation equals
    /// [`RotationLaw::angle_at`] to 10⁻⁹ rad at five times either side of a lock inside the window
    /// and across the epoch, on a despinning, a synchronous and a 3:2 body (Mercury's eccentricity).
    #[test]
    fn the_law_s_parts_reproduce_its_angle() {
        let year = SECONDS_PER_JULIAN_YEAR;
        let age = 1e9 * year;
        let laws = [
            ("despinning", law(f64::INFINITY, age, 0.02)),
            (
                "despinning, lock beyond the window",
                law(age + 1e9 * year, age, 0.02),
            ),
            ("synchronous, ahead", law(age + 30.0 * year, age, 0.02)),
            ("synchronous, behind", law(age - 30.0 * year, age, 0.02)),
            ("3:2, ahead", law(age + 30.0 * year, age, 0.2056)),
            ("3:2, behind", law(age - 30.0 * year, age, 0.2056)),
        ];
        let around_epoch = [-year, -86_400.0, -3_600.0, 0.0, 3_600.0, 86_400.0, year];
        for (name, law) in laws {
            let parts = law.parts();
            let mut times: Vec<f64> = around_epoch.to_vec();
            if let Some(lock) = law.locks_at() {
                let lock = lock.since_epoch().as_seconds_f64();
                for k in [1.0, 10.0, 1e3, 1e5, 1e7] {
                    times.extend([lock - k, lock + k]);
                }
            }
            for s in times {
                let expected = law.angle_at(at(s)).value();
                let twin = angle_from_parts(&parts, s);
                assert!(
                    centred(twin - expected).abs() < 1e-9,
                    "{name} at {s} s: {twin} against {expected}"
                );
            }
        }
    }

    /// The angle's step over `dt` at `t`, less what the rate sweeps, as a wrapped difference.
    fn jump(law: &RotationLaw, t: f64, dt: f64) -> f64 {
        let before = law.angle_at(at(t - dt)).value();
        let after = law.angle_at(at(t + dt)).value();
        let expected = law.rate_at(at(t)) * 2.0 * dt;
        centred(after - before - expected).abs()
    }

    /// (b) The rotation angle is continuous across the locking time, whether it falls after the
    /// epoch or before it.
    #[test]
    fn the_rotation_angle_is_continuous_across_the_lock() {
        let year = SECONDS_PER_JULIAN_YEAR;
        for (e, resonance) in [
            (0.02, SpinOrbitResonance::Synchronous),
            (0.2, SpinOrbitResonance::ThreeToTwo),
        ] {
            let age = 1e9 * year;
            for d in [30.0 * year, -30.0 * year] {
                let law = law(age + d, age, e);
                assert_eq!(law.resonance(), resonance);
                let lock = law.locks_at().unwrap().since_epoch().as_seconds_f64();
                assert!((lock - d).abs() < 1.0);
                assert_eq!(law.state_at(at(lock + 1.0)), SpinState::Locked(resonance));
                assert_eq!(law.state_at(at(lock - 1.0)), SpinState::Despinning);
                assert!(
                    jump(&law, lock, 0.5) < 1e-6,
                    "{e} {d}: {}",
                    jump(&law, lock, 0.5)
                );
                assert!(jump(&law, 0.0, 0.5) < 1e-6);
            }
        }
    }

    /// A body not locked at the epoch has its drawn phase then, and one locked by it the locked
    /// angle.
    #[test]
    fn the_epoch_angle_is_the_drawn_phase_until_the_lock() {
        let year = SECONDS_PER_JULIAN_YEAR;
        let free = law(2e9 * year, 1e9 * year, 0.02);
        assert!((free.angle_at(UniverseTime::EPOCH).value() - 0.5).abs() < 1e-12);
        let never = law(f64::INFINITY, 1e9 * year, 0.02);
        assert_eq!(never.locks_at(), None);
        assert!((never.angle_at(UniverseTime::EPOCH).value() - 0.5).abs() < 1e-12);
        let locked = law(1e8 * year, 1e9 * year, 0.02);
        let expected = reduce(2.0 + locked.clock.mean_anomaly_at(UniverseTime::EPOCH).value());
        assert!((locked.angle_at(UniverseTime::EPOCH).value() - expected).abs() < 1e-12);
    }

    #[test]
    fn invalid_inputs_are_refused() {
        assert_eq!(
            SpinningBody::new(Kilograms::new(1.0), Metres::new(0.0), 0.3, 0.3, 100.0),
            Err(BuildSpinningBodyError::RadiusNotPositive)
        );
        let mut inputs = RotationInputs {
            primordial_period: Seconds::new(0.0),
            locking_time: Seconds::new(1.0),
            age_at_epoch: Years::new(1.0),
            orbit: orbit(METRES_PER_AU, 0.0, SOLAR_MASS_KG, flat(), 0.0),
            sub_primary_angle: Radians::ZERO,
            phase_at_epoch: Radians::ZERO,
        };
        assert_eq!(
            RotationLaw::new(&inputs),
            Err(BuildRotationLawError::PeriodNotPositive)
        );
        inputs.primordial_period = Seconds::new(1.0);
        inputs.locking_time = Seconds::new(-1.0);
        assert_eq!(
            RotationLaw::new(&inputs),
            Err(BuildRotationLawError::LockingTimeNotPositive)
        );
    }

    #[test]
    fn breakup_is_hours_for_the_earth() {
        let p = breakup_period(Kilograms::new(EARTH_MASS_KG), Metres::new(EARTH_RADIUS_M));
        assert!((p.value() / 3_600.0 - 1.41).abs() < 0.01);
    }

    /// science-p14-tides's integration (its §7's thirteen rows): generated bodies, laid at
    /// `radius_zeng`'s core radius under `radius_with_envelope`'s radius, and the edges of the
    /// generated range. Each row is α = `R_core` ÷ R, the envelope fraction f, the water fraction w
    /// beneath it, and the fluid k₂ under an n = 1 and an n = 2 polytrope of hydrogen and helium.
    const INTEGRATED: [(f64, f64, f64, f64, f64); 13] = [
        (0.8323, 0.001, 0.0, 0.3867, 0.3866), // 2.4 M⊕, 0.1%, 10 F⊕: law/mean 0.931
        (0.9200, 0.0011, 0.539, 0.4652, 0.4652), // close_binary AB g: 1.008
        (0.8295, 0.0083, 0.0, 0.3419, 0.3413), // hierarchical_triple C g: 1.047
        (0.6915, 0.0193, 0.0, 0.1492, 0.1470), // wide_binary A b: 1.017
        (0.4662, 0.0188, 0.0, 0.0281, 0.0255), // filler_c A b: 0.943
        (0.5672, 0.0207, 0.0, 0.0637, 0.0609), // hot_sub_neptune: 0.965
        (0.6737, 0.0492, 0.0, 0.1426, 0.1369), // subgiant A c: 1.041
        (0.6661, 0.0636, 0.539, 0.1318, 0.1243), // Neptune: 0.933
        (0.6183, 0.0820, 0.539, 0.1091, 0.0985), // Uranus: 0.923
        (0.4575, 0.0982, 0.0, 0.0516, 0.0374), // hierarchical_triple A c: 1.045
        (0.5301, 0.1386, 0.539, 0.0904, 0.0704), // hierarchical_triple C i: 0.944
        (0.3835, 0.2, 0.0, 0.0676, 0.0367),   // 13 M⊕, 20%, 10 F⊕: 1.105
        (0.4391, 0.3, 0.539, 0.1249, 0.0752), // 30 M⊕, 30%, 0.1 F⊕, icy: 0.962
    ];

    /// P14.T14.d (science-p14-tides): the law lies within 10% of the geometric mean of the n = 1
    /// and n = 2 envelopes' k₂ for f ≤ 5%, and within 15% beyond, where the envelope's equation of
    /// state alone spans a factor of up to 3.3.
    #[test]
    fn the_love_number_under_an_envelope_reproduces_the_integration() {
        for (alpha, f, w, n1, n2) in INTEGRATED {
            let k2 = love_number_under_envelope(alpha, f, w);
            let mean = (n1 * n2).sqrt();
            let band = if f <= 0.05 { 0.10 } else { 0.15 };
            assert!(
                (k2 / mean - 1.0).abs() <= band,
                "α {alpha}, f {f}, w {w}: {k2} against {mean} ({n1}, {n2})"
            );
        }
    }

    /// P14.T14.d: under a massless envelope the law is the core's own tide seen from the outer
    /// radius, 0.9 (1 − 0.39 w) α⁵, exactly.
    #[test]
    fn a_massless_envelope_leaves_the_core_s_tide_exactly() {
        for alpha in [0.0, 0.3, 0.618, 0.9, 1.0] {
            for w in [0.0, 0.25, 0.539, 1.0] {
                let core = 0.9 * (1.0 - 0.39 * w) * math::powi(alpha, 5);
                assert_same_bits(love_number_under_envelope(alpha, 0.0, w), core);
            }
        }
    }

    /// P14.T14.d (science-p14-tides): both terms scale with α, so a core of no radius, or a body
    /// whose core cannot be measured, takes k₂ = 0 under any envelope.
    #[test]
    fn an_unmeasurable_core_takes_no_tide() {
        assert_same_bits(love_number_under_envelope(0.0, 0.1, 0.539), 0.0);
        let unmeasurable =
            enveloped_love_number(EarthMasses::new(5.0), EarthRadii::new(0.0), &sub_neptune());
        assert_same_bits(unmeasurable, 0.0);
    }

    /// P14.T14.d: over 1.5–100 M⊕, envelopes of 0.1–50% and water of 0–54% beneath them, at the
    /// generator's own radii (Lopez and Fortney's at 10 F⊕ and 5 Gyr), α is the core's fraction of
    /// the radius that `radius_with_envelope` lays the envelope on, in (0, 1], and k₂ lies in
    /// (0, 1.5), a homogeneous body's.
    #[test]
    fn alpha_and_the_love_number_stay_physical_at_the_generator_s_radii() {
        use crate::planetary::derive::envelope::radius_with_envelope;
        use crate::units::{EarthFluxes, Gigayears};
        let mut checked = 0;
        for m in [1.5, 2.0, 3.0, 5.0, 8.0, 10.0, 15.0, 20.0, 30.0, 50.0, 100.0] {
            for f in [1e-3, 3e-3, 0.01, 0.02, 0.05, 0.1, 0.2, 0.35, 0.5] {
                for w in [0.0, 0.1, 0.25, 0.4, 0.54] {
                    for cmf in [0.15, EARTH_CORE_MASS_FRACTION, 0.6] {
                        let core = CoreComposition::new(cmf, w).unwrap();
                        let mass = EarthMasses::new(m);
                        let fractions = MassFractions::of(core, f);
                        let radius = radius_with_envelope(
                            mass,
                            core,
                            f,
                            EarthFluxes::new(10.0),
                            Gigayears::new(5.0),
                        );
                        let (alpha, water) = core_beneath_envelope(mass, radius, &fractions);
                        let laid = radius_zeng(EarthMasses::new(m * (1.0 - f)), core).value();
                        assert!(
                            (alpha * radius.value() / laid - 1.0).abs() < 1e-12,
                            "{m} M⊕, f {f}, w {w}, cmf {cmf}: α {alpha}"
                        );
                        assert!((water - w).abs() < 1e-12, "{water} against {w}");
                        assert!(alpha > 0.0 && alpha <= 1.0, "{m} M⊕, f {f}: α {alpha}");
                        let k2 = enveloped_love_number(mass, radius, &fractions);
                        assert!(k2 > 0.0 && k2 < 1.5, "{m} M⊕, f {f}, w {w}: k₂ {k2}");
                        checked += 1;
                    }
                }
            }
        }
        assert_eq!(checked, 11 * 9 * 5 * 3);
    }

    /// The fractions of P14.T14.d's test sub-Neptune: iron 0.318 and rock 0.662 under an envelope
    /// of 2%.
    fn sub_neptune() -> MassFractions {
        // Iron is what the rock and the envelope leave: 1 − 0.662 − 0.02 = 0.318.
        MassFractions::of(CoreComposition::new(1.0 - 0.662 / 0.98, 0.0).unwrap(), 0.02)
    }

    /// P14.T14.d: the solid classes keep a rocky body's pair and gas giants a giant's; a
    /// sub-Neptune and an ice giant of the same mass, radius and fractions take the same tides,
    /// the envelope's Love number with Q = 1.03 × 10⁴.
    #[test]
    fn solid_classes_and_gas_giants_keep_their_pairs_and_enveloped_classes_share_one_tide() {
        let (mass, radius) = (EarthMasses::new(5.0), EarthRadii::new(2.4));
        let fractions = sub_neptune();
        let pair = |class| tides(class, mass, radius, &fractions);
        for (class, (k2, q)) in [
            (PlanetClass::Rocky, (0.3, 100.0)),
            (PlanetClass::Icy, (0.3, 100.0)),
            (PlanetClass::GasGiant, (0.4, 1e5)),
        ] {
            let (love, quality) = pair(class);
            assert_same_bits(love, k2);
            assert_same_bits(quality, q);
        }
        let (sub_neptune, ice_giant) = (pair(PlanetClass::SubNeptune), pair(PlanetClass::IceGiant));
        assert_same_bits(sub_neptune.0, ice_giant.0);
        assert_same_bits(sub_neptune.1, ice_giant.1);
        assert_same_bits(sub_neptune.1, 1.03e4);
        assert_same_bits(
            sub_neptune.0,
            enveloped_love_number(mass, radius, &fractions),
        );
        // science-p14-tides's 5 M⊕, 2% body: k₂ 0.105 (integrated 0.109–0.112), so
        // Q′ = 3Q ÷ 2k₂ about 1.5 × 10⁵.
        assert!((0.09..0.13).contains(&sub_neptune.0), "{}", sub_neptune.0);
    }

    /// A body of class `class` with [`tides`] and its one moment of inertia: what
    /// [`SpinningBody::of_class`] gives from the 21 → 22 batch's bump.
    fn with_its_tides(
        mass: f64,
        radius: f64,
        class: PlanetClass,
        fractions: &MassFractions,
    ) -> SpinningBody {
        let (mass, radius) = (EarthMasses::new(mass), EarthRadii::new(radius));
        let (k2, q) = tides(class, mass, radius, fractions);
        SpinningBody::new(
            Kilograms::from(mass),
            Metres::from(radius),
            moment_of_inertia_factor(class, fractions),
            k2,
            q,
        )
        .unwrap()
    }

    /// P14.T14.d: from a 15-hour spin, a 5 M⊕ sub-Neptune of 2% envelope at 2.4 R⊕ locks within
    /// 100 Myr at 0.1 au of a Sun and within 1 Gyr at 0.07 au of a 0.2 M☉ star, and does not lock
    /// within 10 Gyr at 0.35 au of a Sun, where a rocky body's tides would lock it within 100 Myr.
    #[test]
    fn a_sub_neptune_locks_only_close_in() {
        let fractions = sub_neptune();
        let planet = with_its_tides(5.0, 2.4, PlanetClass::SubNeptune, &fractions);
        let au = METRES_PER_AU;
        let close = lock(&planet, 0.1 * au, SOLAR_MASS_KG);
        assert!(close < 100.0 * MYR, "{} Myr at 0.1 au", close / MYR);
        let m_dwarf = lock(&planet, 0.07 * au, 0.2 * SOLAR_MASS_KG);
        assert!(m_dwarf < GYR, "{} Gyr at 0.07 au of 0.2 M☉", m_dwarf / GYR);
        let far = lock(&planet, 0.35 * au, SOLAR_MASS_KG);
        assert!(far > 10.0 * GYR, "{} Gyr at 0.35 au", far / GYR);
        let rocky = SpinningBody::new(
            planet.mass(),
            planet.radius(),
            planet.moment_factor(),
            ROCKY_LOVE_NUMBER,
            ROCKY_TIDAL_Q,
        )
        .unwrap();
        let rocky_far = lock(&rocky, 0.35 * au, SOLAR_MASS_KG);
        assert!(rocky_far < 100.0 * MYR, "{} Myr", rocky_far / MYR);
    }

    /// P14.T14.d is held until the 21 → 22 batch's bump, so that the batch's goldens move once:
    /// until then [`SpinningBody::of_class`] takes [`tides_in_force`], which gives every class
    /// version 21's pair. The bump removes the hold, and this test with it.
    #[test]
    fn the_envelope_s_tides_are_held_only_until_the_batch_s_bump() {
        assert!(
            GENERATOR_VERSION < ENVELOPED_TIDES_VERSION,
            "the 21 → 22 bump applies `tides` to every reader: call `tides` where \
             `tides_in_force` is called, and remove it, `ENVELOPED_TIDES_VERSION` and this test \
             (plan 14, P14.T14.d as built)"
        );
        let (mass, radius) = (EarthMasses::new(5.0), EarthRadii::new(2.4));
        let fractions = sub_neptune();
        let rocky = (ROCKY_LOVE_NUMBER, ROCKY_TIDAL_Q);
        let giant = (GIANT_LOVE_NUMBER, GIANT_TIDAL_Q);
        for (class, (k2, q)) in [
            (PlanetClass::Rocky, rocky),
            (PlanetClass::Icy, rocky),
            (PlanetClass::SubNeptune, rocky),
            (PlanetClass::IceGiant, giant),
            (PlanetClass::GasGiant, giant),
        ] {
            let (love, quality) = tides_in_force(class, mass, radius, &fractions);
            assert_same_bits(love, k2);
            assert_same_bits(quality, q);
            let held = SpinningBody::of_class(
                Kilograms::from(mass),
                Metres::from(radius),
                class,
                &fractions,
            )
            .unwrap();
            assert_same_bits(held.love_number(), k2);
            assert_same_bits(held.tidal_q(), q);
            assert_same_bits(
                held.moment_factor(),
                moment_of_inertia_factor(class, &fractions),
            );
        }
    }
}
