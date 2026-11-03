//! What has become of a body at a time: plan 14's fate transform and the states it produces
//! (design notes 1, 11 and 12, P14.T28).
//!
//! A system is generated as it was born, and time enters only here: [`state_at`] turns a body's
//! primordial orbit into its state and orbit at a time, and is the only place that does. A body is
//! [`BodyState::NotYetFormed`] before its formation age (P14.T28.a,
//! [`hosts::young`](crate::planetary::hosts::young)), then [`BodyState::Present`] on an orbit that
//! circularises under tides (P14.T8.e) and widens as its host loses mass, until its host swallows
//! it (P14.T28.b), a supernova unbinds or disrupts it (P14.T28.c,
//! [`hosts::evolved`](crate::planetary::hosts::evolved)), or a neighbour whose orbit the
//! supernova made cross its own ejects it or merges with it (ruling 71). The state sequence of
//! every body is a prefix of not yet formed → present → destroyed or unbound, and its elements are
//! continuous in time except at a supernova.
//!
//! # The transform
//!
//! [`BodyFate::resolve_all`] fixes the histories of one host's bodies once, from their
//! [`FateBody`]s and their [`FateHost`], and [`BodyFate::resolve`] one body's alone there:
//!
//! 1. Its formation time: the clock time of its formation age on its host.
//! 2. Segments of its life between its host's sudden deaths. In each, the orbit is the segment's
//!    elements circularised to the host's age (the first segment only) and then expanded from the
//!    segment's reference mass to the mass the body orbits then, so a(t) = a₀ M₀ ÷ M(t), the
//!    adiabatic limit of isotropic mass loss: a M constant (Veras et al. 2011, MNRAS 417, 2104,
//!    eq. 18) and e constant over an orbit (eq. 17), valid while Ψ = (Ṁ ÷ M) ÷ n ≪ 1 (eq. 15).
//! 3. In each segment, the first time the body's semi-major axis is inside the engulfment reach
//!    times the largest radius of the host's stars that have not died by the segment's start
//!    ([`hosts::evolved::engulfment_reach`](crate::planetary::hosts::evolved::engulfment_reach)),
//!    found by a scan and a bisection of fixed points: the body is `Destroyed { Engulfed }` then.
//! 4. At each sudden death of a host star (a core collapse, an electron capture, a direct collapse,
//!    a pair-instability or thermonuclear disruption), the body's state vector on the orbit just
//!    before, the host's mass dropping from the progenitor's to the remnant's, and the remnant's
//!    natal kick: a new segment on the elements that follow, `Unbound`, or
//!    `Destroyed { TidallyDisrupted }` at its next pericentre if that is inside the remnant's
//!    Roche limit. A white dwarf's birth is a star's envelope lost by winds, slow against the
//!    orbit, so it is part of the expansion and no event.
//! 5. Right after each sudden death, the scattering step (P14.T28.c, ruling 71): the bodies it
//!    left bound are walked in slot order until no two orbits cross or come within 2√3 mutual Hill
//!    radii, and each such pair loses its lighter body, `Unbound` where the heavier's Safronov
//!    number is 1 or more, the heavier taking the pair's binding energy with an eccentricity drawn
//!    from Ford and Rasio's Table 1 (rulings 75.3 and 80, [`ScatterDraws`]), and otherwise
//!    `Destroyed { Collided }` and merged into the heavier, which takes both masses and the pair's
//!    mass-weighted angular momentum and energy (Ford and Rasio 2008; Petrovich et al. 2014).
//!    This is the one step in which a body's history depends on another's; pairs that come
//!    within 2√3 Hill radii under slow mass loss stay as they are (ruling 71.2), since their
//!    instability takes up to gigayears.
//!
//! [`BodyFate::at`] then reads the history at any time, so every query of one body agrees with
//! every other, whatever their order: the history depends on the host's bodies and the host alone,
//! never on the time asked about. [`state_at`] is both in one call, for a body alone on its host.
//!
//! # The seam to the generator
//!
//! `SystemContext` (P14.T1.d) and the placer (P14.T8) are being built beside this, so the transform
//! takes plain values, which P14.T30.b fills from them:
//!
//! - [`FateHost`]: the [`StarModel`]s of the stars the body orbits (ruling 34), from the context:
//!   one star for a planet of a star, and every star below the pair for a circumbinary planet,
//!   which orbits the pair's total mass. A companion's planets have their own star alone as host,
//!   so they see its mass loss and not their companion's. Below generator version 27 each star is
//!   a single star; from it (P14.T28.g, `BINARY_HOST_FATE_VERSION`) a star of a pair the binary
//!   engine ran is read from its pair's timeline (`FateHost::evolved`), as the next section says.
//!
//! # Hosts of a pair the engine ran
//!
//! From generator version 27 (P14.T28.g; research-binary-hosts R3–R5) a paired star's mass, which
//! a body's orbit widens or shrinks with by a M = constant (Veras et al. 2011, eq. 18), and its
//! largest radius, which engulfs, are its pair's timeline's, and the pair's own events replace its
//! stars' single-star deaths as the host's sudden events:
//!
//! - a common envelope (Hurley, Tout and Pols 2002, MNRAS 329, 897, §2.7.1), and the coalescence
//!   that ends a pair without one (their §§2.6.4–2.6.6), engulfs a planet of either star
//!   (`Destroyed { Engulfed }`): the envelope engulfs the binary's orbit (Paczyński 1976; Ivanova
//!   et al. 2013, A&ARv 21, 59, §1), and every S-type orbit lies inside 0.43 of it (Holman and
//!   Wiegert 1999, AJ 117, 621, eq. 1);
//! - for a circumbinary planet the same events are a sudden loss of the pair's mass with no kick,
//!   the supernova's step (lean, 2026-11-02: BSE ejects the envelope at once, and over about a year
//!   a planet at 3–100 au is in Veras et al.'s non-adiabatic regime); a planet inside its reach of
//!   the larger star before the envelope is engulfed by the search, as before any death;
//! - a supernova of a star moves its own planets as a single star's does, with the remnant's natal
//!   kick, and its companion's not at all; a circumbinary planet takes the pair's mass loss with
//!   the pair's recoil (their eq. A14), and is `Unbound` where the explosion, or a white dwarf's
//!   birth, unbinds the pair (lean, 2026-11-02: the stars separate at about their orbital speed);
//! - a host the engine leaves with no mass unbinds its planets, by the step.
//! - [`FateBody`]: its [`Formation`] (drawn with the body from its seed, ID, mass and its disc's
//!   lifetime, P14.T30.a), its primordial elements about its host's initial mass, its mass, its
//!   bulk density for the Roche test, and T8.e's circularisation time.
//! - Moons go with their planet: a moon's state is its planet's (P14.T30.b).
//!
//! # In the vertical slice
//!
//! The kick is [`StarModel::natal_kick`], P06.T19's law, along the galactic axes, which the
//! system frame's are parallel to; T28.c first applied a zero kick, since there was no law
//! (ruling 33), and P06.T19 lifted it. A white dwarf's own kick of about 1 km/s is not applied,
//! since its birth is no sudden death (ruling 62.6). The protoplanetary disc body of T28.a
//! arrives with phase D's belts (T21), and T28.d–e (white dwarf pollution and second-generation
//! planets) are not in the slice.

use std::error::Error;
use std::f64::consts::TAU;
use std::fmt;

use crate::Seed;
use crate::coords::SystemVelocity;
use crate::id::BodyId;
use crate::orbit::KeplerElements;
use crate::planetary::context::SystemContext;
use crate::planetary::drift::{DriftingOrbit, EvolvingLaw, composite_excess, drift_cell};
use crate::planetary::hosts::evolved::{
    Aftermath, Circularisation, circularised, circularised_axis, engulfment_reach, expanded,
    expanded_axis, first_crossing, first_engulfment, supernova,
};
use crate::planetary::hosts::young::Formation;
use crate::planetary::placement::OrbitZone;
use crate::planetary::placement::zones::{BinaryHostZonesRules, PairBound, ZONE_EDGE_TOLERANCE};
use crate::planetary::record::BodyOrbit;
use crate::stellar::Phase;
use crate::stellar::binary::{BinaryTimeline, Component as PairComponent, SegmentKind};
use crate::stellar::remnant::NatalKick;
use crate::stellar::system::{StarModel, pair_of};
use crate::time::{ClockWindow, SourceHorizon, Span, UniverseTime};
use crate::units::consts::SECONDS_PER_JULIAN_YEAR;
use crate::units::{
    EarthMasses, GravitationalParameter, KilogramsPerCubicMetre, Metres, SolarMasses, Years,
};
use crate::{GENERATOR_VERSION, GeneratorVersion};

/// The generator version from which the fate transform reads a star of a pair the binary engine
/// ran from its pair's timeline (P14.T28.g; the [module documentation](self)): 27, held with the
/// 27 train.
///
/// Below it [`BinaryHostFateRules::in_force`] is [`BinaryHostFateRules::SingleModels`]: every
/// host star is its own single-star model, so that a planet of a star whose companion swallowed
/// it in a common envelope survives about the mass of both, and every figure is version 26's bit
/// for bit. 27's bump deletes this hold, [`BinaryHostFateRules`] with its `SingleModels` arm, the
/// tests' switch and its tripwire (`the_binary_host_fate_is_held_only_until_the_27_bump`).
pub(crate) const BINARY_HOST_FATE_VERSION: GeneratorVersion = GeneratorVersion::new(27);

/// Where the fate transform reads a paired star (its hold, [`BINARY_HOST_FATE_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum BinaryHostFateRules {
    /// Version 26's and before: every star's own single-star model, with its own deaths.
    SingleModels,
    /// Version 27's: a star of a pair the engine ran from its pair's timeline, with the pair's
    /// events.
    Engine,
}

impl BinaryHostFateRules {
    /// The rules this generator version gives: the engine's from [`BINARY_HOST_FATE_VERSION`],
    /// the single models' below it; in the tests, the rules a test's switch sets on its thread
    /// ([`with_binary_host_fate`]).
    #[must_use]
    pub(crate) fn in_force() -> Self {
        #[cfg(test)]
        if let Some(rules) = BINARY_HOST_FATE_RULES.with(core::cell::Cell::get) {
            return rules;
        }
        if GENERATOR_VERSION >= BINARY_HOST_FATE_VERSION {
            Self::Engine
        } else {
            Self::SingleModels
        }
    }
}

#[cfg(test)]
thread_local! {
    /// The paired hosts' fate rules a test sets on its thread, `None` for the version's.
    static BINARY_HOST_FATE_RULES: core::cell::Cell<Option<BinaryHostFateRules>> =
        const { core::cell::Cell::new(None) };
}

/// `run`'s result with the paired hosts' fate rules `rules` on this thread, for the tests to reach
/// the engine's fates ahead of the bump; the version's rules are set back after, a panic in `run`
/// included. 27's bump deletes it with the hold.
#[cfg(test)]
pub(crate) fn with_binary_host_fate<R>(rules: BinaryHostFateRules, run: impl FnOnce() -> R) -> R {
    /// Sets the thread's rules back as it drops.
    struct Restore(Option<BinaryHostFateRules>);
    impl Drop for Restore {
        fn drop(&mut self) {
            BINARY_HOST_FATE_RULES.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(BINARY_HOST_FATE_RULES.with(|cell| cell.replace(Some(rules))));
    run()
}

/// A body's state at a time: a prefix of not yet formed → present → destroyed or unbound
/// (P14.T28's test).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BodyState {
    /// The body has not formed yet: a giant before its formation age, a small planet before its
    /// disc's lifetime, or any body of a system not yet born (design note 12).
    NotYetFormed,
    /// The body exists and orbits as its elements say.
    Present,
    /// The body was destroyed at `at`.
    Destroyed {
        /// What destroyed it.
        cause: DestructionCause,
        /// When.
        at: UniverseTime,
    },
    /// The body was unbound from its system at `at`, and is no longer tracked (design note 11:
    /// the rogue-planet layer counts escapers statistically).
    Unbound {
        /// When.
        at: UniverseTime,
    },
}

impl BodyState {
    /// When the body stopped being present, for a destroyed or unbound body.
    #[must_use]
    pub const fn ended_at(&self) -> Option<UniverseTime> {
        match self {
            Self::Destroyed { at, .. } | Self::Unbound { at } => Some(*at),
            Self::NotYetFormed | Self::Present => None,
        }
    }
}

/// What destroyed a body (design notes 11 and 12).
///
/// A moon destroyed with its planet takes its planet's cause.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DestructionCause {
    /// A protoplanetary disc dispersed at the end of its lifetime (P14.T28.a).
    Dispersed,
    /// The host expanded over the body's orbit: a planet is engulfed once its semi-major axis is
    /// inside 1.0–1.8 host radii, from rocky to Jovian (Mustill and Villaver 2012; P14.T28.b,
    /// ruling 62).
    Engulfed,
    /// The body's pericentre fell inside its primary's Roche limit, as after a supernova that
    /// leaves a planet on a plunging orbit about the remnant (P14.T28.c).
    TidallyDisrupted,
    /// The body collided with a heavier neighbour whose orbit its own crossed after a supernova,
    /// and merged into it (P14.T28.c's scattering step, ruling 71): the heavier's Safronov number
    /// was under 1, too small to eject it (Ford and Rasio 2008; `scatter`).
    Collided,
}

/// A [`FateBody`] could not be built from the values given.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildFateBodyError {
    /// The body's mass was not finite and positive.
    MassNotPositive(EarthMasses),
    /// The body's bulk density was not finite and positive.
    DensityNotPositive(KilogramsPerCubicMetre),
}

impl fmt::Display for BuildFateBodyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MassNotPositive(mass) => {
                write!(
                    f,
                    "body mass {} M_earth is not finite and positive",
                    mass.value()
                )
            }
            Self::DensityNotPositive(density) => write!(
                f,
                "body density {} kg/m^3 is not finite and positive",
                density.value()
            ),
        }
    }
}

impl Error for BuildFateBodyError {}

/// Words of a body's `planet.scatter` stream that each ejection it survives reads or reserves
/// (ruling 80, [`ScatterDraws`]): its eccentricity's rank and its phase's, then two reserved.
pub const SCATTER_WORDS_PER_EJECTION: u64 = 4;

/// Where the draws a body takes from each ejection it survives come from (P14.T28.c, ruling 80):
/// the rank of its new eccentricity and its phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScatterDraws {
    /// Every draw at its median: the eccentricity at its law's median and the survivor at its
    /// pericentre. For a body built by hand, as in a test.
    Median,
    /// The body's own `planet.scatter` stream in the universe of `seed`
    /// ([`SCATTER_WORDS_PER_EJECTION`] words an ejection), which the generator gives every planet.
    Stream {
        /// The universe.
        seed: Seed,
        /// The body.
        body: BodyId,
    },
}

/// Which generation of its host's bodies a body is (design note 11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Generation {
    /// Formed in its host's primordial disc: it never forms at or after one of its host stars'
    /// deaths, and its orbit is about the host's initial mass.
    Primordial,
    /// Formed after its host's death, in a fallback disc about the remnant (P14.T28.e): its orbit
    /// is about the host's mass at its formation, and only later deaths reach it.
    Second,
}

/// What the fate transform reads of a body: its formation, its primordial orbit, its mass, its
/// bulk density, how its orbit circularises, the draws of the ejections it survives, and its
/// generation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FateBody {
    formation: Formation,
    orbit: KeplerElements,
    mass: EarthMasses,
    density: KilogramsPerCubicMetre,
    circularisation: Circularisation,
    scatter: ScatterDraws,
    generation: Generation,
}

impl FateBody {
    /// A body that forms as `formation` says on the primordial `orbit`, of `mass` and bulk
    /// `density`, with no circularisation until
    /// [`with_circularisation`](Self::with_circularisation).
    ///
    /// `orbit` is the orbit at birth, about the host's initial mass: its star's, or the sum of a
    /// pair's for a circumbinary body ([`FateHost`]), as the placer builds it (P14.T8). The mass
    /// sets the engulfment reach, and the density the Roche limit after a supernova: the
    /// generator passes the derived body's (P14.T16.a).
    ///
    /// # Errors
    ///
    /// [`BuildFateBodyError::MassNotPositive`] and [`BuildFateBodyError::DensityNotPositive`]
    /// unless the mass and the density are finite and positive.
    pub fn new(
        formation: Formation,
        orbit: KeplerElements,
        mass: EarthMasses,
        density: KilogramsPerCubicMetre,
    ) -> Result<Self, BuildFateBodyError> {
        if !(mass.value().is_finite() && mass.value() > 0.0) {
            return Err(BuildFateBodyError::MassNotPositive(mass));
        }
        if !(density.value().is_finite() && density.value() > 0.0) {
            return Err(BuildFateBodyError::DensityNotPositive(density));
        }
        Ok(Self {
            formation,
            orbit,
            mass,
            density,
            circularisation: Circularisation::NONE,
            scatter: ScatterDraws::Median,
            generation: Generation::Primordial,
        })
    }

    /// The same body as a second-generation one (P14.T28.e): formed when `formation` says, after
    /// its host's death, on `orbit` about the host's mass then, its remnant's.
    #[must_use]
    pub const fn second_generation(self) -> Self {
        Self {
            generation: Generation::Second,
            ..self
        }
    }

    /// Which generation of its host's bodies it is.
    #[must_use]
    pub const fn generation(&self) -> Generation {
        self.generation
    }

    /// The same body, circularising as `circularisation` says (P14.T8.e).
    #[must_use]
    pub const fn with_circularisation(self, circularisation: Circularisation) -> Self {
        Self {
            circularisation,
            ..self
        }
    }

    /// The same body, taking the draws of the ejections it survives from `scatter` (ruling 80).
    #[must_use]
    pub const fn with_scatter_draws(self, scatter: ScatterDraws) -> Self {
        Self { scatter, ..self }
    }

    /// Where the draws of the ejections it survives come from.
    #[must_use]
    pub const fn scatter_draws(&self) -> ScatterDraws {
        self.scatter
    }

    /// When the body forms.
    #[must_use]
    pub const fn formation(&self) -> &Formation {
        &self.formation
    }

    /// The orbit it forms on: about the host's initial mass for a primordial body, and about the
    /// host's mass at its formation for a second-generation one.
    #[must_use]
    pub const fn orbit(&self) -> &KeplerElements {
        &self.orbit
    }

    /// The mass, M⊕.
    #[must_use]
    pub const fn mass(&self) -> EarthMasses {
        self.mass
    }

    /// The bulk density, kg m⁻³.
    #[must_use]
    pub const fn density(&self) -> KilogramsPerCubicMetre {
        self.density
    }

    /// How the orbit circularises.
    #[must_use]
    pub const fn circularisation(&self) -> Circularisation {
        self.circularisation
    }
}

/// A [`FateHost`] could not be built from the stars given.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildFateHostError {
    /// No star was given.
    NoStars,
    /// Two of the stars have different ages at the epoch, which the stars of one system never do.
    NotCoeval {
        /// The first star's age at the epoch, yr.
        first: Years,
        /// The other's.
        other: Years,
    },
}

impl fmt::Display for BuildFateHostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoStars => f.write_str("a host has no stars"),
            Self::NotCoeval { first, other } => write!(
                f,
                "host stars aged {} yr and {} yr at the epoch are not coeval",
                first.value(),
                other.value()
            ),
        }
    }
}

impl Error for BuildFateHostError {}

/// The stars a body orbits, as the fate transform reads them: plan 06's [`StarModel`]s (ruling
/// 34).
///
/// One star for a circumstellar body, and every star below the pair it orbits for a circumbinary
/// one, which is treated as orbiting the stars' total mass (P14.T28.c). The stars' order is the
/// order their masses are summed in, which P14.T30.b makes their body-index order.
#[derive(Debug, Clone, PartialEq)]
pub struct FateHost<'a> {
    stars: Vec<&'a StarModel>,
    /// For each of `stars`, in order, the engine's run of it (P14.T28.g): its pair, by its index
    /// in `timelines`, and its component there; `None` for a star the engine ran in no pair, and
    /// for every star of a host built from its models alone.
    paired: Vec<Option<(usize, PairComponent)>>,
    /// The timelines of the pairs `paired` names.
    timelines: Vec<&'a BinaryTimeline>,
    /// The pairs' sudden events the host's bodies feel, by time ([`PairEvent`]).
    events: Vec<PairEvent>,
    /// The zone a pair the engine ran bounds, with how ([`ZoneBound`], P14.T9.d); `None` for a
    /// host built from its models alone, and for a zone no such pair bounds.
    bound: Option<ZoneBound>,
    /// The first time one of the host's stars leaves the system, a supernova having released it
    /// (P11.T10), which unbinds the host's bodies (P14.T9.d): a star's planets go with it, and a
    /// circumbinary planet of the broken pair is left unbound. `None` for a host built from its
    /// models alone, and below the zones' hold at 27.
    departure: Option<UniverseTime>,
}

/// A zone that a pair the binary engine ran bounds at its evaluated state (P14.T9.d): the zone at
/// birth, its pair by its index in the host's timelines, how the pair bounds it, and the clock
/// times of the pair's timeline's knots ([`BinaryTimeline::knot_ages`]), ascending, at which the
/// zone's search looks.
#[derive(Debug, Clone, PartialEq)]
struct ZoneBound {
    zone: OrbitZone,
    pair: usize,
    bound: PairBound,
    knots: Vec<UniverseTime>,
}

/// A sudden event of a pair the binary engine ran, as a host's bodies feel it (P14.T28.g; the
/// [module documentation](self)).
#[derive(Debug, Clone, Copy, PartialEq)]
struct PairEvent {
    /// The event's clock time.
    at: UniverseTime,
    /// Its age on its pair's timeline, Julian years: the start of the segment it opens, or the
    /// supernova's own.
    age: f64,
    /// Its pair, by its index in the host's timelines.
    pair: usize,
    /// What it does to the host's bodies.
    effect: PairEffect,
}

/// What a [`PairEvent`] does to a host's bodies.
#[derive(Debug, Clone, Copy, PartialEq)]
enum PairEffect {
    /// Every body is `Destroyed { Engulfed }`: a star's planets at its pair's common envelope or
    /// coalescence.
    Engulf,
    /// Every body takes the sudden step from the host's mass just before the event to just after
    /// it, its centre moving off at `kick`, m s⁻¹ along the system frame's axes: a circumbinary
    /// body at a common envelope or a coalescence (no kick) and at a supernova that leaves its pair
    /// bound (the pair's recoil), and a star's planets at its own supernova (its natal kick).
    Step {
        /// The velocity the host's centre takes.
        kick: SystemVelocity,
    },
    /// Every body is `Unbound`: a circumbinary body when its pair is unbound.
    Unbind,
}

/// Which of a pair's stars a host holds, for its events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PairShare {
    /// One star, the host's own: an S-type zone.
    Star(PairComponent),
    /// Both: a circumbinary zone, or one above it.
    Both,
}

impl<'a> FateHost<'a> {
    /// The single star `star`.
    #[must_use]
    pub fn star(star: &'a StarModel) -> Self {
        Self {
            stars: vec![star],
            paired: vec![None],
            timelines: Vec::new(),
            events: Vec::new(),
            bound: None,
            departure: None,
        }
    }

    /// The stars `stars`, which a circumbinary body orbits together.
    ///
    /// # Errors
    ///
    /// [`BuildFateHostError::NoStars`] if there are none, and [`BuildFateHostError::NotCoeval`]
    /// if two of them differ in age at the epoch.
    pub fn stars(
        stars: impl IntoIterator<Item = &'a StarModel>,
    ) -> Result<Self, BuildFateHostError> {
        let stars: Vec<&'a StarModel> = stars.into_iter().collect();
        let first = stars
            .first()
            .ok_or(BuildFateHostError::NoStars)?
            .age_at_epoch();
        if let Some(other) = stars
            .iter()
            .map(|star| star.age_at_epoch())
            .find(|age| !age.total_cmp(&first).is_eq())
        {
            return Err(BuildFateHostError::NotCoeval { first, other });
        }
        Ok(Self {
            paired: vec![None; stars.len()],
            stars,
            timelines: Vec::new(),
            events: Vec::new(),
            bound: None,
            departure: None,
        })
    }

    /// The stars the bodies of `zone`, a zone of `ctx`, orbit: [`FateHost::stars`] of the zone's
    /// members in body order, and from generator version 27 (P14.T28.g) each star of a pair the
    /// binary engine ran read from its pair's timeline, with the pair's events in place of its
    /// single-star deaths (the [module documentation](self)).
    ///
    /// # Panics
    ///
    /// If `zone` is not a zone of `ctx`'s stars.
    #[must_use]
    pub(crate) fn evolved(ctx: &'a SystemContext, zone: &OrbitZone) -> Self {
        let stars = ctx.stars();
        let mut host = Self::stars(zone.members().map(|member| &stars[usize::from(member)]))
            .expect("a zone has a component, and a system's stars are coeval");
        match BinaryHostZonesRules::in_force() {
            BinaryHostZonesRules::Birth => {}
            BinaryHostZonesRules::Evaluated => {
                host.departure = zone
                    .members()
                    .filter_map(|member| ctx.departure(usize::from(member)))
                    .filter_map(|age| clock_time(host.stars[0], age))
                    .min();
            }
        }
        match BinaryHostFateRules::in_force() {
            BinaryHostFateRules::SingleModels => return host,
            BinaryHostFateRules::Engine => {}
        }
        let members: Vec<u8> = zone.members().collect();
        for (k, &member) in members.iter().enumerate() {
            let Some((pair, component)) = pair_of(ctx.pairs(), member) else {
                continue;
            };
            let timeline = pair.timeline();
            let index = host
                .timelines
                .iter()
                .position(|&t| core::ptr::eq(t, timeline))
                .unwrap_or_else(|| {
                    let both = pair.stars().iter().all(|s| members.contains(&s.get()));
                    let share = if both {
                        PairShare::Both
                    } else {
                        PairShare::Star(component)
                    };
                    host.timelines.push(timeline);
                    let index = host.timelines.len() - 1;
                    let first = host.stars[0];
                    host.events
                        .extend(pair_events(timeline, first, index, share));
                    index
                });
            host.paired[k] = Some((index, component));
        }
        // By time, and at one time in the order found: the pairs' depth-first order, each pair's
        // events by their kind's order in `pair_events`.
        host.events.sort_by_key(|event| event.at);
        // A star's own zone, which its pair bounds from outside, and the pair's own zone.
        let bound = match *host.paired.as_slice() {
            [Some((pair, component))] => Some((pair, PairBound::Star(component))),
            [Some((first, _)), Some((second, _))] if first == second => {
                Some((first, PairBound::Pair))
            }
            _ => None,
        };
        host.bound = bound.and_then(|(pair, bound)| match BinaryHostZonesRules::in_force() {
            BinaryHostZonesRules::Birth => None,
            BinaryHostZonesRules::Evaluated => {
                let timeline = host.timelines[pair];
                let first = host.stars[0];
                let mut knots: Vec<UniverseTime> = timeline
                    .knot_ages(0.0, timeline.until().value())
                    .into_iter()
                    .filter_map(|age| clock_time(first, Years::new(age)))
                    .collect();
                knots.dedup();
                Some(ZoneBound {
                    zone: *zone,
                    pair,
                    bound,
                    knots,
                })
            }
        });
        host
    }

    /// The host's mass at `t`, M☉, as [`BodyFate::host_mass`] reads it: its stars' masses then,
    /// a paired star's from its pair's timeline.
    ///
    /// # Panics
    ///
    /// As [`BodyFate::host_mass`], at a time before the host has formed.
    #[must_use]
    pub(crate) fn mass_at(&self, t: UniverseTime) -> SolarMasses {
        self.stars.iter().zip(&self.paired).zip(self.deaths()).fold(
            SolarMasses::ZERO,
            |sum, ((model, paired), death)| {
                sum + match paired {
                    Some((pair, component)) => self.paired_mass(*pair, *component, t),
                    None => star_mass(model, death, t),
                }
            },
        )
    }

    /// The limits at `t` of `zone`, the zone at birth whose bodies the host holds, m (P14.T9.d):
    /// its birth limits widened as its bodies' orbits are, by its mass at birth over
    /// [`mass_at`](Self::mass_at) `t`, and where a pair the engine ran bounds it and the zones'
    /// hold is lifted, intersected with Holman and Wiegert's fit on the pair's state then
    /// ([`OrbitZone::limits_at`]).
    ///
    /// # Panics
    ///
    /// As [`mass_at`](Self::mass_at).
    #[must_use]
    pub(crate) fn limits_at(
        &self,
        zone: &OrbitZone,
        t: UniverseTime,
    ) -> (Option<Metres>, Option<Metres>) {
        let mass = self.mass_at(t).value();
        let widening = if mass > 0.0 {
            zone.host_mass().value() / mass
        } else {
            1.0
        };
        let birth = (
            zone.inner().map(|r| r * widening),
            zone.outer().map(|r| r * widening),
        );
        match (BinaryHostZonesRules::in_force(), &self.bound) {
            (BinaryHostZonesRules::Evaluated, Some(bound)) => {
                let age = Years::new(self.pair_age(bound.pair, t));
                let state = self.timelines[bound.pair].state_at(age);
                zone.limits_at(&state, bound.bound, widening)
            }
            (BinaryHostZonesRules::Evaluated, None) | (BinaryHostZonesRules::Birth, _) => birth,
        }
    }

    /// The stars, in the order given.
    #[must_use]
    pub fn members(&self) -> &[&'a StarModel] {
        &self.stars
    }

    /// The stars' total initial mass, summed in their order: the mass a body's primordial orbit is
    /// about.
    #[must_use]
    fn initial_mass(&self) -> SolarMasses {
        self.stars
            .iter()
            .fold(SolarMasses::ZERO, |sum, star| sum + star.initial_mass())
    }

    /// The first star, whose age is every star's.
    #[must_use]
    fn first(&self) -> &'a StarModel {
        self.stars[0]
    }

    /// The age on the timeline of pair `pair` at `t`, Julian years: its stars' age then, held on
    /// the side of each of the pair's events that `t` is on, so that the clock's rounding of an
    /// event's age never reads the state across it, and inside the ages the timeline was run to.
    #[must_use]
    fn pair_age(&self, pair: usize, t: UniverseTime) -> f64 {
        let mut age = self.first().age_at(t).value();
        for event in self.events.iter().filter(|event| event.pair == pair) {
            age = if t >= event.at {
                age.max(event.age)
            } else {
                age.min(event.age.next_down())
            };
        }
        age.clamp(0.0, self.timelines[pair].until().value())
    }

    /// The mass at `t` of the star the engine ran as `component` of pair `pair`, M☉: its state's
    /// on the pair's timeline then.
    #[must_use]
    fn paired_mass(&self, pair: usize, component: PairComponent, t: UniverseTime) -> SolarMasses {
        let age = Years::new(self.pair_age(pair, t));
        self.timelines[pair].state_at(age).stars()[component.index()].mass()
    }

    /// The largest radius by `t` of the star the engine ran as `component` of pair `pair`, m.
    #[must_use]
    fn paired_radius(&self, pair: usize, component: PairComponent, t: UniverseTime) -> f64 {
        let age = Years::new(self.pair_age(pair, t));
        Metres::from(self.timelines[pair].max_radius_until(component, age)).value()
    }

    /// Each star's death, if it has died by the end of the clock window ([`death_of`]): none for a
    /// star the engine ran in a pair, whose pair's events stand in for its deaths (P14.T28.g).
    #[must_use]
    fn deaths(&self) -> Vec<Option<StarDeath>> {
        self.stars
            .iter()
            .zip(&self.paired)
            .map(|(star, paired)| match paired {
                Some(_) => None,
                None => death_of(star),
            })
            .collect()
    }

    /// Whether the star the engine ran as `component` of pair `pair` has not exploded by `t`.
    #[must_use]
    fn paired_lives_at(&self, pair: usize, component: PairComponent, t: UniverseTime) -> bool {
        self.timelines[pair].supernovae().iter().all(|sn| {
            sn.component() != component
                || clock_time(self.first(), sn.age()).is_none_or(|at| at > t)
        })
    }
}

/// The events of `timeline`, the host's pair `pair`, that a host holding `share` of its stars
/// feels (P14.T28.g; the [module documentation](self)), each at its clock time on `first`'s clock,
/// in the timeline's order: its segments' events, then its supernovae. An event off the clock is
/// left out.
#[must_use]
fn pair_events(
    timeline: &BinaryTimeline,
    first: &StarModel,
    pair: usize,
    share: PairShare,
) -> Vec<PairEvent> {
    let merged = timeline.merger_age().map(Years::value);
    let mut events = Vec::new();
    let mut push = |age: f64, effect: PairEffect| {
        if let Some(at) = clock_time(first, Years::new(age)) {
            events.push(PairEvent {
                at,
                age,
                pair,
                effect,
            });
        }
    };
    let segments = timeline.segments();
    for (before, segment) in segments.iter().zip(segments.iter().skip(1)) {
        let effect = match (segment.kind(), share) {
            // A common envelope that ends in a coalescence is one event, the envelope's.
            (SegmentKind::Merged, _) if before.kind() == SegmentKind::CommonEnvelope => None,
            (SegmentKind::CommonEnvelope | SegmentKind::Merged, PairShare::Star(_)) => {
                Some(PairEffect::Engulf)
            }
            (SegmentKind::CommonEnvelope | SegmentKind::Merged, PairShare::Both) => {
                Some(PairEffect::Step {
                    kick: SystemVelocity::ZERO,
                })
            }
            (SegmentKind::Disrupted { .. }, PairShare::Both) => Some(PairEffect::Unbind),
            (SegmentKind::Disrupted { .. }, PairShare::Star(_))
            | (
                SegmentKind::Detached | SegmentKind::StableTransfer { .. } | SegmentKind::Contact,
                _,
            ) => None,
        };
        if let Some(effect) = effect {
            push(segment.start().value(), effect);
        }
    }
    for sn in timeline.supernovae() {
        let natal = sn.kick().map_or(SystemVelocity::ZERO, kick_velocity);
        let effect = match share {
            PairShare::Star(own) if sn.component() == own => Some(PairEffect::Step { kick: natal }),
            PairShare::Star(_) => None,
            // A coalesced pair's product explodes as a single star, with its own kick.
            PairShare::Both if merged.is_some_and(|age| age <= sn.age().value()) => {
                Some(PairEffect::Step { kick: natal })
            }
            PairShare::Both => Some(
                sn.recoil()
                    .map_or(PairEffect::Unbind, |kick| PairEffect::Step { kick }),
            ),
        };
        if let Some(effect) = effect {
            push(sn.age().value(), effect);
        }
    }
    events
}

/// A host's sudden change, by where the host keeps it: a single star's sudden death, by the star's
/// index, or a pair's event, by its index in the host's events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Sudden {
    /// The sudden death of the host's star of this index.
    Death(usize),
    /// The host's pair event of this index.
    Event(usize),
    /// A host star's leaving the system ([`FateHost`]'s `departure`).
    Departure,
}

/// A body's state and orbit at a time: what [`state_at`] and [`BodyFate::at`] return.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FateAt {
    state: BodyState,
    orbit: Option<KeplerElements>,
    trajectory: Option<DriftingOrbit>,
    valid_until: Option<UniverseTime>,
    changes_at: Option<UniverseTime>,
    mass: EarthMasses,
}

impl FateAt {
    /// The body's state.
    #[must_use]
    pub const fn state(&self) -> BodyState {
        self.state
    }

    /// The body's elements at the time, about the mass it orbits then, if it is present.
    ///
    /// They are the osculating elements of design note 11's closed forms, with the segment's mean
    /// anomaly at the epoch kept as the orbit circularises and expands, and the period from the
    /// axis: what the derivation reads. Positions follow [`trajectory`](Self::trajectory),
    /// whose phase is the integral of the mean motion (P14.T45.a).
    #[must_use]
    pub const fn orbit(&self) -> Option<&KeplerElements> {
        self.orbit.as_ref()
    }

    /// How the body moves about its host at the time, if it is present: its elements, and for
    /// an orbit that evolves the model of its drift cell (P14.T45.a), which every position is
    /// taken from.
    #[must_use]
    pub const fn trajectory(&self) -> Option<&DriftingOrbit> {
        self.trajectory.as_ref()
    }

    /// The time the record's orbit holds until, if inside the clock window: the next change of
    /// the body's state or segment ([`changes_at`](Self::changes_at)), or for an evolving orbit
    /// the end of its drift cell if earlier (P14.T45.a).
    #[must_use]
    pub const fn valid_until(&self) -> Option<UniverseTime> {
        self.valid_until
    }

    /// The next time the body's state changes, or its orbit steps (a supernova), if that is
    /// inside the clock window, or wherever it falls for a present body at a time before the
    /// window's start, where drifting records hold until their cell's end and the light-time solve
    /// reads them: its formation, its destruction or unbinding, a host star's sudden death.
    /// Between, the orbit changes only slowly, by the tides and the winds, which
    /// [`trajectory`](Self::trajectory) follows. A moon's record holds until its planet's next
    /// change, not its planet's drift cell's end. Before the window's start this can be stated
    /// where [`valid_until`](Self::valid_until) is not: a fixed orbit's record states no
    /// `valid_until` outside the window, but its moons' drifting records still end at the change.
    #[must_use]
    pub const fn changes_at(&self) -> Option<UniverseTime> {
        self.changes_at
    }

    /// The body's mass at the time, M⊕: its own, plus that of every neighbour that collided with
    /// it and merged into it by then (ruling 71). A body not yet formed has its own mass, and a
    /// body gone the mass it had when it went.
    #[must_use]
    pub const fn mass(&self) -> EarthMasses {
        self.mass
    }

    /// The record's orbit section: the trajectory's elements and drift and
    /// [`FateAt::valid_until`], if the body is present (P14.T34, P14.T45.a).
    #[must_use]
    pub fn body_orbit(&self) -> Option<BodyOrbit> {
        self.trajectory.map(|trajectory| {
            BodyOrbit::new(*trajectory.elements(), self.valid_until)
                .with_drift(trajectory.drift().copied())
        })
    }
}

/// A segment's circularisation and expansion as an evolving law (P14.T45.a): its elements at a
/// time are [`BodyFate::orbit_in`]'s about the host's mass then, held at their value just before
/// the segment's next change, and its phase excess the integral of the mean motion's excess from
/// the anchor, by the 16-point Gauss–Legendre rule on each interval between the law's break
/// points ([`composite_excess`], P14.T45.d).
struct SegmentLaw<'s, 'a> {
    fate: &'s BodyFate<'a>,
    segment: &'s Segment,
    /// The last instant of the segment the law is read at: a nanosecond before its next change.
    last: UniverseTime,
    anchor: UniverseTime,
    anchored: KeplerElements,
    n_anchor_rad_per_s: f64,
    /// The times strictly inside the segment, ascending and without repeats, at which the mean
    /// motion may change its slope ([`BodyFate::breaks`]).
    breaks: Vec<UniverseTime>,
}

impl EvolvingLaw for SegmentLaw<'_, '_> {
    fn anchored(&self) -> &KeplerElements {
        &self.anchored
    }

    fn shape_at(&self, t: UniverseTime) -> KeplerElements {
        let t = t.min(self.last);
        self.fate.orbit_in(self.segment, t, self.fate.host_mass(t))
    }

    fn phase_excess(&self, t: UniverseTime) -> f64 {
        self.local_excess(self.anchor, t, self.n_anchor_rad_per_s)
    }

    fn local_excess(&self, from: UniverseTime, to: UniverseTime, n_from_rad_per_s: f64) -> f64 {
        composite_excess(from, to, n_from_rad_per_s, &self.breaks, |at| {
            TAU / self.shape_at(at).period().value()
        })
    }

    fn breaks_around(&self, t: UniverseTime) -> (Option<UniverseTime>, Option<UniverseTime>) {
        let after = self.breaks.partition_point(|&at| at <= t);
        (
            after.checked_sub(1).map(|before| self.breaks[before]),
            self.breaks.get(after).copied(),
        )
    }
}

/// How and when a host star dies, if it has died by the end of the clock window.
#[derive(Debug, Clone, Copy, PartialEq)]
struct StarDeath {
    /// The clock time of the death.
    at: UniverseTime,
    /// Whether the death is sudden, an explosion or a collapse, and not a white dwarf's birth.
    sudden: bool,
    /// The star's mass at its last living instant, M☉.
    before: SolarMasses,
    /// The remnant's mass, M☉; zero where there is none.
    after: SolarMasses,
    /// The remnant's natal kick, m s⁻¹ along the system frame's axes (P06.T19's law); zero where
    /// the model has none.
    kick: SystemVelocity,
}

/// One stretch of a body's life between its host's sudden deaths.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Segment {
    /// When it starts: the body's formation, or a supernova.
    start: UniverseTime,
    /// The elements it starts from, about `reference`.
    orbit: KeplerElements,
    /// The mass `orbit` is about, M☉.
    reference: SolarMasses,
    /// Whether the orbit circularises, which only the primordial one does.
    circularises: bool,
    /// The body's mass through the segment, M⊕: its own, or with the neighbours merged into it
    /// at the segment's start ([`scatter`]).
    mass: EarthMasses,
}

/// A body's whole history on its host: when it forms, its orbit between its host's sudden deaths,
/// and how and when it ends, fixed once by [`BodyFate::resolve`] and read at any time by
/// [`BodyFate::at`] (P14.T28).
///
/// # Examples
///
/// A Jupiter and an Earth at 1 au about the Sun of plan 06's track: the Jupiter's tides drag it
/// into the Sun as it climbs the red-giant branch, some 7.7 Gyr from now, and the Earth, whose
/// reach is only just beyond the photosphere, survives to orbit the white dwarf at 1.92 au.
///
/// ```
/// use hyperion_sim::orbit::{Eccentricity, KeplerElements, Orientation};
/// use hyperion_sim::planetary::fate::{BodyFate, BodyState, DestructionCause, FateBody, FateHost};
/// use hyperion_sim::planetary::hosts::young::{Formation, FormationDraws};
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::stellar::draws::StarDraws;
/// use hyperion_sim::stellar::system::StarModel;
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::{
///     AstronomicalUnits, EarthMasses, GravitationalParameter, KilogramsPerCubicMetre, Megayears,
///     Metres, Radians, SolarMasses, Years,
/// };
///
/// // The Sun 13.5 Gyr after the onset of its collapse, a white dwarf since 12.50 Gyr.
/// let sun = StarModel::new(
///     SolarMasses::new(1.0),
///     Composition::SOLAR,
///     StarDraws::median(),
///     Years::new(13.5e9),
/// )?;
/// let host = FateHost::star(&sun);
/// let at_1_au = |mass: f64, density: f64| -> Result<FateBody, Box<dyn std::error::Error>> {
///     let mass = EarthMasses::new(mass);
///     Ok(FateBody::new(
///         Formation::from_draws(mass, Megayears::new(2.0), &FormationDraws::MEDIAN)?,
///         KeplerElements::from_semi_major_axis(
///             Metres::from(AstronomicalUnits::new(1.0)),
///             GravitationalParameter::from_solar_masses(SolarMasses::new(1.0)),
///             Eccentricity::CIRCULAR,
///             Orientation::new(Radians::ZERO, Radians::ZERO, Radians::ZERO)?,
///             Radians::ZERO,
///         )?,
///         mass,
///         KilogramsPerCubicMetre::new(density),
///     )?)
/// };
///
/// let jupiter = at_1_au(317.8, 1_326.0)?;
/// let fate = BodyFate::resolve(&jupiter, &host);
/// let Some(BodyState::Destroyed { cause, at }) = fate.ending() else {
///     panic!("a Jupiter at 1 au does not survive the Sun");
/// };
/// assert_eq!(cause, DestructionCause::Engulfed);
/// let age = 13.5e9 + at.since_epoch().as_julian_years_f64();
/// assert!((12.24e9..12.37e9).contains(&age), "engulfed at {age:e} yr");
/// assert_eq!(fate.at(UniverseTime::EPOCH).state(), BodyState::Destroyed { cause, at });
///
/// let earth = at_1_au(1.0, 5_513.0)?;
/// let now = BodyFate::resolve(&earth, &host).at(UniverseTime::EPOCH);
/// let orbit = now.orbit().ok_or("the Earth survives")?;
/// let au = orbit.semi_major_axis().value() / 1.495_978_707e11;
/// assert!((au - 1.953).abs() < 1e-3, "at {au} au");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct BodyFate<'a> {
    body: &'a FateBody,
    host: &'a FateHost<'a>,
    deaths: Vec<Option<StarDeath>>,
    formed_at: Option<UniverseTime>,
    segments: Vec<Segment>,
    ending: Option<BodyState>,
    /// How many ejections the body has survived so far, which picks the words of its next draw.
    recoils: u64,
}

impl<'a> BodyFate<'a> {
    /// The history of `body` on `host`, alone there: [`BodyFate::resolve_all`] of `body` alone.
    ///
    /// It reads the host's stars up to the end of the clock window, or to the last death where
    /// every star is dead by then; the cost is a few state evaluations per star, and for a body
    /// that an evolved host can reach, about 130 more for the engulfment search.
    ///
    /// A body whose formation age falls at or after one of its host stars' deaths never forms,
    /// since the death ends its disc; it needs a disc that outlives a star of over 13 M☉, which
    /// P06.T15.c's lifetimes make vanishingly rare.
    ///
    /// # Panics
    ///
    /// Never: one body has one history.
    #[must_use]
    pub fn resolve(body: &'a FateBody, host: &'a FateHost<'a>) -> Self {
        let mut fates = Self::resolve_all(&[body], host);
        fates.pop().expect("one body has one history")
    }

    /// The histories of `bodies`, every body of one host in slot order, on `host`: each as
    /// [`BodyFate::resolve`] would have it alone, but for the scattering step after each of the
    /// host's sudden deaths (P14.T28.c, ruling 71, `scatter`).
    ///
    /// A supernova gives each survivor its own eccentricity, and orbits that cross, or come within
    /// 2√3 mutual Hill radii, scatter within a few orbits. So at each sudden death the bodies it
    /// leaves bound are walked in slot order until no such pair remains, and each pair found
    /// loses its lighter body, ejected or merged into the heavier. Nothing else couples one body's
    /// history to another's, so a host without a sudden death gives every body its history alone,
    /// bit for bit, and so does a body the step leaves untouched.
    ///
    /// The result is in the order given.
    ///
    /// # Examples
    ///
    /// Two giants on circular orbits 300 and 400 au from a 20 M☉ star: the black hole's birth
    /// sheds a fifth of the mass, both orbits take e = 0.22 and cross, and the lighter is ejected.
    ///
    /// ```
    /// use hyperion_sim::orbit::{Eccentricity, KeplerElements, Orientation};
    /// use hyperion_sim::planetary::fate::{BodyFate, BodyState, FateBody, FateHost};
    /// use hyperion_sim::planetary::hosts::young::{Formation, FormationDraws};
    /// use hyperion_sim::stellar::Composition;
    /// use hyperion_sim::stellar::draws::StarDraws;
    /// use hyperion_sim::stellar::system::StarModel;
    /// use hyperion_sim::time::UniverseTime;
    /// use hyperion_sim::units::{
    ///     AstronomicalUnits, EarthMasses, GravitationalParameter, KilogramsPerCubicMetre,
    ///     Megayears, Metres, Radians, SolarMasses, Years,
    /// };
    ///
    /// let star = StarModel::new(
    ///     SolarMasses::new(20.0),
    ///     Composition::SOLAR,
    ///     StarDraws::median(),
    ///     Years::new(3e7),
    /// )?;
    /// let host = FateHost::star(&star);
    /// let jupiter = |a_au: f64, mass: f64| -> Result<FateBody, Box<dyn std::error::Error>> {
    ///     let mass = EarthMasses::new(mass);
    ///     Ok(FateBody::new(
    ///         Formation::from_draws(mass, Megayears::new(0.3), &FormationDraws::MEDIAN)?,
    ///         KeplerElements::from_semi_major_axis(
    ///             Metres::from(AstronomicalUnits::new(a_au)),
    ///             GravitationalParameter::from_solar_masses(SolarMasses::new(20.0)),
    ///             Eccentricity::CIRCULAR,
    ///             Orientation::new(Radians::ZERO, Radians::ZERO, Radians::ZERO)?,
    ///             Radians::new(a_au / 100.0),
    ///         )?,
    ///         mass,
    ///         KilogramsPerCubicMetre::new(1_326.0),
    ///     )?)
    /// };
    /// let (inner, outer) = (jupiter(300.0, 317.8)?, jupiter(400.0, 100.0)?);
    ///
    /// // Alone, each would survive the supernova.
    /// for body in [&inner, &outer] {
    ///     let alone = BodyFate::resolve(body, &host).at(UniverseTime::EPOCH);
    ///     assert_eq!(alone.state(), BodyState::Present);
    /// }
    /// // Together, the lighter is ejected, and the heavier takes the pair's binding energy: its
    /// // orbit shrinks and grows more eccentric.
    /// let fates = BodyFate::resolve_all(&[&inner, &outer], &host);
    /// assert!(matches!(fates[1].ending(), Some(BodyState::Unbound { .. })));
    /// let alone = BodyFate::resolve(&inner, &host).at(UniverseTime::EPOCH);
    /// let before = alone.orbit().ok_or("bound")?;
    /// let after = fates[0].at(UniverseTime::EPOCH);
    /// let after = after.orbit().ok_or("the survivor is bound")?;
    /// assert!(after.semi_major_axis() < before.semi_major_axis());
    /// assert!(after.eccentricity().value() > before.eccentricity().value());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn resolve_all(bodies: &[&'a FateBody], host: &'a FateHost<'a>) -> Vec<Self> {
        let deaths = host.deaths();
        let mut sudden: Vec<(UniverseTime, Sudden)> = deaths
            .iter()
            .enumerate()
            .filter_map(|(k, death)| death.filter(|d| d.sudden).map(|d| (d.at, Sudden::Death(k))))
            .chain(
                host.events
                    .iter()
                    .enumerate()
                    .map(|(k, event)| (event.at, Sudden::Event(k))),
            )
            .chain(host.departure.map(|at| (at, Sudden::Departure)))
            .collect();
        sudden.sort_unstable();
        let mut fates: Vec<Self> = bodies
            .iter()
            .map(|&body| Self::begin(body, host, deaths.clone()))
            .collect();
        for &death in &sudden {
            for fate in &mut fates {
                fate.live_to(Some(death));
            }
            let passes = scatter::scatter(&mut fates, death.0);
            debug_assert!(passes < fates.len().max(1), "each pass removes a body");
        }
        for fate in &mut fates {
            fate.live_to(None);
        }
        fates
    }

    /// The history of `bodies[index]` among `bodies`, every body of one host in slot order:
    /// [`BodyFate::resolve_all`]'s, resolving the others only where the host has a sudden death
    /// for the scattering step to follow.
    ///
    /// # Panics
    ///
    /// If `index` is out of `bodies`' range.
    #[must_use]
    pub fn resolve_among(bodies: &[&'a FateBody], index: usize, host: &'a FateHost<'a>) -> Self {
        let body = bodies[index];
        let sudden = !host.events.is_empty()
            || host.departure.is_some()
            || host
                .deaths()
                .iter()
                .any(|death| death.is_some_and(|death| death.sudden));
        if sudden {
            Self::resolve_all(bodies, host).swap_remove(index)
        } else {
            Self::resolve(body, host)
        }
    }

    /// The body's formation and its first segment, before any of its life is followed.
    ///
    /// A primordial body never forms at or after a host star's death, which ends its disc; a
    /// second-generation one forms after its host's death, and its first segment is about the
    /// host's mass then, without circularisation (P14.T28.e).
    fn begin(body: &'a FateBody, host: &'a FateHost<'a>, deaths: Vec<Option<StarDeath>>) -> Self {
        let formed_at =
            clock_time(host.first(), Years::from(body.formation.formed_at())).filter(|formed| {
                match body.generation {
                    Generation::Primordial => {
                        deaths.iter().flatten().all(|death| death.at > *formed)
                            && host.events.iter().all(|event| event.at > *formed)
                            && host.departure.is_none_or(|at| at > *formed)
                    }
                    Generation::Second => true,
                }
            });
        let mut fate = Self {
            body,
            host,
            deaths,
            formed_at,
            segments: Vec::new(),
            ending: None,
            recoils: 0,
        };
        if let Some(formed) = formed_at {
            let (reference, circularises) = match body.generation {
                Generation::Primordial => (host.initial_mass(), true),
                Generation::Second => (fate.host_mass(formed), false),
            };
            fate.segments.push(Segment {
                start: formed,
                orbit: body.orbit,
                reference,
                circularises,
                mass: body.mass,
            });
        }
        fate
    }

    /// When the body forms, or `None` if it never does.
    #[must_use]
    pub const fn formed_at(&self) -> Option<UniverseTime> {
        self.formed_at
    }

    /// How the body ends, if it does before the host's history known to the transform ends: a
    /// [`BodyState::Destroyed`] or [`BodyState::Unbound`] with its time.
    #[must_use]
    pub const fn ending(&self) -> Option<BodyState> {
        self.ending
    }

    /// The body's state and orbit at `t`.
    ///
    /// # Panics
    ///
    /// In debug builds, if `t` is after the end of the clock window and the body is present then
    /// about a star that lives past it, whose track is built no further ([`StarModel::state_at`]).
    #[must_use]
    pub fn at(&self, t: UniverseTime) -> FateAt {
        let within = |time: UniverseTime| ClockWindow::contains(time).then_some(time);
        let absent = |state, valid_until, mass| FateAt {
            state,
            orbit: None,
            trajectory: None,
            valid_until,
            changes_at: valid_until,
            mass,
        };
        let Some(formed) = self.formed_at else {
            return absent(BodyState::NotYetFormed, None, self.body.mass);
        };
        if t < formed {
            return absent(BodyState::NotYetFormed, within(formed), self.body.mass);
        }
        if let Some(ending) = self.ending
            && ending.ended_at().is_some_and(|at| t >= at)
        {
            return absent(
                ending,
                None,
                self.segments.last().map_or(self.body.mass, |s| s.mass),
            );
        }
        let (segment, next) = self.segment_at(t);
        let orbit = self.orbit_in(segment, t, self.host_mass(t));
        let (trajectory, valid_until) = match self.law(segment, next, t) {
            Some(law) => {
                let cell = drift_cell(&law, t, segment.start, next);
                (cell.orbit, cell.holds_until(t, next))
            }
            None => (DriftingOrbit::fixed(orbit), next.and_then(within)),
        };
        FateAt {
            state: BodyState::Present,
            orbit: Some(orbit),
            trajectory: Some(trajectory),
            valid_until,
            // Before `START` a moon's drifting record holds until its planet's change there too,
            // as `holds_until` states a cell's end there (RM1 validation, 2026-10-02).
            changes_at: next.filter(|&next| t < ClockWindow::START || ClockWindow::contains(next)),
            mass: segment.mass,
        }
    }

    /// The body's orbit at `t`, [`BodyFate::at`]'s bit for bit, without the trajectory and the
    /// validity it also finds; `None` where the body is not present then. P14.T48.b's water window
    /// reads it at past ages.
    ///
    /// # Panics
    ///
    /// In debug builds, as [`BodyFate::at`].
    #[must_use]
    pub(crate) fn orbit_at(&self, t: UniverseTime) -> Option<KeplerElements> {
        let formed = self.formed_at?;
        let ended = self
            .ending
            .and_then(|ending| ending.ended_at())
            .is_some_and(|at| t >= at);
        if t < formed || ended {
            return None;
        }
        let (segment, _) = self.segment_at(t);
        Some(self.orbit_in(segment, t, self.host_mass(t)))
    }

    /// The segment holding `t`, a time at or after the body's formation, and the segment's next
    /// change: the next segment's start, or the body's ending.
    #[must_use]
    fn segment_at(&self, t: UniverseTime) -> (&Segment, Option<UniverseTime>) {
        let index = self.segments.partition_point(|segment| segment.start <= t) - 1;
        let next = self.segments.get(index + 1).map_or_else(
            || self.ending.and_then(|ending| ending.ended_at()),
            |segment| Some(segment.start),
        );
        (&self.segments[index], next)
    }

    /// The law `segment`'s elements follow until `until`, its next change if any, if the body's
    /// position at `t` follows it (P14.T45.a, P14.T45.d); `None` for an orbit that holds.
    ///
    /// The orbit evolves where its elements differ between the two ends of the segment's part of
    /// the source horizon, [`SourceHorizon::START`] to `END`: the circularised axis and
    /// eccentricity only fall, and a host's mass only falls, so equal elements there are equal
    /// throughout. The light-time solve reaches back to the horizon's start, so an orbit that
    /// changed only before the window still has its phase there from the integral. Inside the
    /// window and after it, an orbit that does not change over the segment's part of the window
    /// takes a fixed record, which equals the law's there: its mean motion is the anchor's, so its
    /// phase excess is zero. The anchor its phase is counted from is the epoch clamped to the
    /// segment's part of the horizon: the epoch where the segment holds it, the segment's nearer
    /// end in the window otherwise, and for a segment that ends before the window its end.
    #[must_use]
    fn law<'s>(
        &'s self,
        segment: &'s Segment,
        until: Option<UniverseTime>,
        t: UniverseTime,
    ) -> Option<SegmentLaw<'s, 'a>> {
        let one_nanosecond = Span::new(0, 1).expect("0 s and 1 ns is a normalised span");
        let last = until.map_or(ClockWindow::END, |until| {
            until
                .checked_sub(one_nanosecond)
                .expect("a change after a segment's start is not the clock's first instant")
        });
        let high = last.min(ClockWindow::END);
        let low = if t < ClockWindow::START {
            segment.start.max(SourceHorizon::START)
        } else {
            segment.start.max(ClockWindow::START)
        };
        if high < low {
            return None;
        }
        // Exactly equal elements at both ends, compared by total order: equal bits mean nothing
        // changes to the resolution of the law itself, which is what decides between a fixed
        // record and a drifting one.
        let first = self.orbit_in(segment, low, self.host_mass(low));
        let last_orbit = self.orbit_in(segment, high, self.host_mass(high));
        let same = |a: f64, b: f64| a.total_cmp(&b).is_eq();
        if same(
            first.semi_major_axis().value(),
            last_orbit.semi_major_axis().value(),
        ) && same(
            first.eccentricity().value(),
            last_orbit.eccentricity().value(),
        ) && same(
            first.gravitational_parameter().value(),
            last_orbit.gravitational_parameter().value(),
        ) {
            return None;
        }
        let horizon_low = segment.start.max(SourceHorizon::START);
        let anchor = UniverseTime::EPOCH.clamp(horizon_low.min(high), high);
        let anchored = self.orbit_in(segment, anchor, self.host_mass(anchor));
        Some(SegmentLaw {
            fate: self,
            segment,
            last,
            anchor,
            n_anchor_rad_per_s: TAU / anchored.period().value(),
            anchored,
            breaks: self.breaks(segment, last),
        })
    }

    /// The times strictly between `segment`'s start and `last`, ascending and without repeats,
    /// at which the mean motion of its law may change its slope (P14.T45.d): each host star's
    /// track-segment boundaries and knot ages ([`StarModel::mass_breaks`]), the host's sudden
    /// deaths, and for a circularising orbit its eccentricity floor's corners
    /// ([`corners`](crate::planetary::hosts::evolved::EccentricityFloor::corners)).
    #[must_use]
    fn breaks(&self, segment: &Segment, last: UniverseTime) -> Vec<UniverseTime> {
        let inside = |at: &UniverseTime| segment.start < *at && *at < last;
        let mut breaks: Vec<UniverseTime> = Vec::new();
        for (star, paired) in self.host.stars.iter().zip(&self.host.paired) {
            breaks.extend(star.mass_breaks(segment.start, last));
            // A paired star's mass is its timeline's, which turns at the timeline's segments.
            if let Some((pair, _)) = paired {
                breaks.extend(
                    self.host.timelines[*pair]
                        .segments()
                        .iter()
                        .filter_map(|s| clock_time(self.host.first(), s.start()))
                        .filter(inside),
                );
            }
        }
        breaks.extend(
            self.deaths
                .iter()
                .flatten()
                .filter(|death| death.sudden)
                .map(|death| death.at)
                .filter(inside),
        );
        breaks.extend(self.host.events.iter().map(|event| event.at).filter(inside));
        let circularisation = self.body.circularisation;
        let e0 = segment.orbit.eccentricity().value();
        if segment.circularises && circularisation.timescale().is_some() && e0 > 0.0 {
            breaks.extend(
                circularisation
                    .floor()
                    .corners(e0)
                    .into_iter()
                    .flatten()
                    .filter_map(|age| clock_time(self.host.first(), Years::new(age)))
                    .filter(inside),
            );
        }
        breaks.sort_unstable();
        breaks.dedup();
        breaks
    }

    /// Follows the body on its last segment up to the sudden death `next` of one of its host
    /// stars, and through it, or with `None` to the end of its host's known history
    /// ([`BodyFate::known_until`]): the engulfment search on the segment, then the death's
    /// aftermath. A body not formed, not formed by then, or gone already is left as it is.
    fn live_to(&mut self, next: Option<(UniverseTime, Sudden)>) {
        let (Some(formed), None) = (self.formed_at, self.ending) else {
            return;
        };
        // A body that forms after the clock window, about a host alive then, has no history the
        // host's models can tell yet.
        if formed > self.known_until() {
            return;
        }
        // A pair's event or a star's leaving before a body forms, which only a second-generation
        // body outlives, is none of its history.
        if let Some((at, Sudden::Event(_) | Sudden::Departure)) = next
            && at <= formed
        {
            return;
        }
        let segment = *self.segments.last().expect("a formed body has a segment");
        let end = next.map_or(self.known_until(), |(at, _)| {
            at.checked_sub(Span::new(0, 1).expect("0 s and 1 ns is a normalised span"))
                .expect("a death after a formation is not the clock's first instant")
        });
        let engulfed = self.engulfment(&segment, end, engulfment_reach(segment.mass));
        // From P14.T9.d's hold at 27, the first time the body leaves its zone at its pair's
        // evaluated state, if that comes before any engulfment.
        let ending = match (engulfed, self.crossing(&segment, end)) {
            (Some(engulfed), Some(crossed)) if crossed < engulfed => {
                Some(BodyState::Unbound { at: crossed })
            }
            (Some(at), _) => Some(BodyState::Destroyed {
                cause: DestructionCause::Engulfed,
                at,
            }),
            (None, Some(at)) => Some(BodyState::Unbound { at }),
            (None, None) => None,
        };
        if ending.is_some() {
            self.ending = ending;
            return;
        }
        let Some((at, sudden)) = next else {
            return;
        };
        let (aftermath, reference) = match sudden {
            Sudden::Death(star) => self.explode(&segment, at, star),
            Sudden::Departure => {
                self.ending = Some(BodyState::Unbound { at });
                return;
            }
            Sudden::Event(k) => {
                let event = self.host.events[k];
                match event.effect {
                    PairEffect::Engulf => {
                        self.ending = Some(BodyState::Destroyed {
                            cause: DestructionCause::Engulfed,
                            at,
                        });
                        return;
                    }
                    PairEffect::Unbind => {
                        self.ending = Some(BodyState::Unbound { at });
                        return;
                    }
                    PairEffect::Step { kick } => match self.step(&segment, event, kick) {
                        Some(stepped) => stepped,
                        None => return,
                    },
                }
            }
        };
        let after = |orbit| Segment {
            start: at,
            orbit,
            reference,
            circularises: false,
            mass: segment.mass,
        };
        match aftermath {
            Aftermath::Bound(orbit) => self.segments.push(after(orbit)),
            Aftermath::Disrupted { orbit, at: when } => {
                if let Some(orbit) = orbit {
                    self.segments.push(after(orbit));
                }
                self.ending = Some(BodyState::Destroyed {
                    cause: DestructionCause::TidallyDisrupted,
                    at: when,
                });
            }
            Aftermath::Unbound => self.ending = Some(BodyState::Unbound { at }),
        }
    }

    /// The last time the host's stars are known: the end of the clock window, or the last death
    /// if every star has died by then, after which nothing changes.
    fn known_until(&self) -> UniverseTime {
        self.deaths
            .iter()
            .try_fold(None, |latest: Option<UniverseTime>, death| {
                death.map(|d| Some(latest.map_or(d.at, |l| l.max(d.at))))
            })
            .flatten()
            .unwrap_or(ClockWindow::END)
    }

    /// The first time in `segment` up to `end` at which the body is engulfed, if any.
    fn engulfment(&self, segment: &Segment, end: UniverseTime, reach: f64) -> Option<UniverseTime> {
        // The orbit only shrinks by circularisation and only widens by mass loss, and the largest
        // radius only grows, so the circularised axis at the end against the largest radius then
        // bounds the clearance at every earlier time: most bodies are clear on that alone.
        let floor = self.base_axis(segment, end).value();
        if floor >= reach * self.largest_radius(segment, end) {
            return None;
        }
        first_engulfment(segment.start, end, |t| {
            self.axis_in(segment, t, self.host_mass(t)).value()
                - reach * self.largest_radius(segment, t)
        })
    }

    /// What the sudden death of host star `star` at `at` leaves of the body on `segment`, and the
    /// mass the body orbits afterwards.
    fn explode(
        &self,
        segment: &Segment,
        at: UniverseTime,
        star: usize,
    ) -> (Aftermath, SolarMasses) {
        let death = self.deaths[star].expect("a star that explodes has a death");
        let (mut before, mut after) = (SolarMasses::ZERO, SolarMasses::ZERO);
        for k in 0..self.host.stars.len() {
            let (b, a) = if k == star {
                (death.before, death.after)
            } else {
                let m = self.member_mass(k, at);
                (m, m)
            };
            before = before + b;
            after = after + a;
        }
        let orbit = self.orbit_in(segment, at, before);
        let mu_per_solar_mass =
            self.body.orbit.gravitational_parameter().value() / self.host.initial_mass().value();
        let mu_after = GravitationalParameter::new(mu_per_solar_mass * after.value());
        // A circumbinary body sees the pair's barycentre move off at the remnant's share of the
        // kick; the pair's own recoil from the mass it lost waits for P11.T4's binaries.
        let share = if after.value() > 0.0 {
            death.after.value() / after.value()
        } else {
            0.0
        };
        let k = death.kick.metres_per_second();
        let kick = SystemVelocity::new([k[0] * share, k[1] * share, k[2] * share]);
        (
            supernova(&orbit, at, mu_after, kick, self.body.density),
            after,
        )
    }

    /// The segment's elements at `t` before expansion: circularised to the host's age then, for
    /// the primordial orbit.
    fn base_orbit(&self, segment: &Segment, t: UniverseTime) -> KeplerElements {
        if segment.circularises {
            circularised(
                &segment.orbit,
                self.body.circularisation,
                self.host.first().age_at(t),
            )
        } else {
            segment.orbit
        }
    }

    /// The segment's elements at `t` about the mass `mass`.
    fn orbit_in(&self, segment: &Segment, t: UniverseTime, mass: SolarMasses) -> KeplerElements {
        expanded(&self.base_orbit(segment, t), segment.reference, mass)
    }

    /// The semi-major axis of [`BodyFate::base_orbit`], bit for bit.
    fn base_axis(&self, segment: &Segment, t: UniverseTime) -> Metres {
        if segment.circularises {
            circularised_axis(
                &segment.orbit,
                self.body.circularisation,
                self.host.first().age_at(t),
            )
        } else {
            segment.orbit.semi_major_axis()
        }
    }

    /// The semi-major axis of [`BodyFate::orbit_in`], bit for bit, without building the orbit.
    fn axis_in(&self, segment: &Segment, t: UniverseTime, mass: SolarMasses) -> Metres {
        expanded_axis(self.base_axis(segment, t), segment.reference, mass)
    }

    /// The mass the body orbits at `t`, M☉: its host stars' masses then, summed in their order,
    /// the mass [`BodyFate::at`]'s elements are about (the derivation's primary, P14.T30.b).
    ///
    /// # Panics
    ///
    /// If a host star has not formed by `t`: the body's host has formed by the time it is
    /// present, which is when the question has an answer. In debug builds also as
    /// [`BodyFate::at`].
    #[must_use]
    pub fn host_mass(&self, t: UniverseTime) -> SolarMasses {
        (0..self.host.stars.len()).fold(SolarMasses::ZERO, |sum, k| sum + self.member_mass(k, t))
    }

    /// The mass of host star `k` at `t`, M☉: its pair's timeline's for a star the engine ran in a
    /// pair (P14.T28.g), and otherwise [`star_mass`]'s.
    #[must_use]
    fn member_mass(&self, k: usize, t: UniverseTime) -> SolarMasses {
        match self.host.paired[k] {
            Some((pair, component)) => self.host.paired_mass(pair, component, t),
            None => star_mass(self.host.stars[k], self.deaths[k], t),
        }
    }

    /// The largest radius any of the host's stars has had by `t`, among those that had not died
    /// by the segment's start, m: a paired star's from its pair's timeline (P14.T28.g), not
    /// counted once it has exploded.
    fn largest_radius(&self, segment: &Segment, t: UniverseTime) -> f64 {
        self.host
            .stars
            .iter()
            .zip(&self.deaths)
            .zip(&self.host.paired)
            .filter(|((_, death), paired)| {
                death.is_none_or(|d| d.at > segment.start)
                    && paired.is_none_or(|(pair, component)| {
                        self.host.paired_lives_at(pair, component, segment.start)
                    })
            })
            .map(|((model, _), paired)| match paired {
                Some((pair, component)) => self.host.paired_radius(*pair, *component, t),
                None => Metres::from(model.max_radius_until(t)).value(),
            })
            .fold(0.0, f64::max)
    }

    /// The first time in `segment` up to `end` at which the body's orbit leaves its zone at its
    /// pair's evaluated state (P14.T9.d, [`FateHost::limits_at`]), its apocentre beyond the outer
    /// limit or its pericentre inside the inner by more than [`ZONE_EDGE_TOLERANCE`] of it, if
    /// any: `None` below the zones' hold at 27 and for a zone no pair the engine ran bounds.
    ///
    /// The scan's points are the segment's ends and every knot of the pair's timeline between
    /// them ([`BinaryTimeline::knot_ages`]), where the pair's orbit or its stars' masses may
    /// turn; the first crossing is bisected to within a second ([`first_crossing`]).
    fn crossing(&self, segment: &Segment, end: UniverseTime) -> Option<UniverseTime> {
        match BinaryHostZonesRules::in_force() {
            BinaryHostZonesRules::Birth => return None,
            BinaryHostZonesRules::Evaluated => {}
        }
        let bound = self.host.bound.as_ref()?;
        let mut points = vec![segment.start];
        if end > segment.start {
            let from = bound.knots.partition_point(|&t| t <= segment.start);
            let to = bound.knots.partition_point(|&t| t < end);
            points.extend_from_slice(&bound.knots[from..to.max(from)]);
            points.push(end);
        }
        first_crossing(&points, |t| {
            self.zone_clearance(segment, t).unwrap_or(f64::INFINITY)
        })
    }

    /// How far inside its zone at its pair's evaluated state the body's orbit on `segment` lies
    /// at `t`, m: the least of the outer limit less its apocentre and its pericentre less the inner
    /// limit, each limit eased by [`ZONE_EDGE_TOLERANCE`] of it (the outer outwards, the inner
    /// inwards) so that a body on a limit counts inside, infinite where neither is set; `None` for
    /// a zone no pair the engine ran bounds, and a host with no mass then.
    fn zone_clearance(&self, segment: &Segment, t: UniverseTime) -> Option<f64> {
        let bound = self.host.bound.as_ref()?;
        let state =
            self.host.timelines[bound.pair].state_at(Years::new(self.host.pair_age(bound.pair, t)));
        // Every star of a bounded zone is its pair's, so its mass is `host_mass`'s from this state,
        // summed in the same order.
        let mass = self
            .host
            .paired
            .iter()
            .flatten()
            .fold(SolarMasses::ZERO, |sum, &(_, component)| {
                sum + state.stars()[component.index()].mass()
            });
        if mass.value() <= 0.0 {
            return None;
        }
        let widening = bound.zone.host_mass().value() / mass.value();
        let (inner, outer) = bound.zone.limits_at(&state, bound.bound, widening);
        let orbit = self.orbit_in(segment, t, mass);
        let outside = outer.map_or(f64::INFINITY, |r| {
            r.value() * (1.0 + ZONE_EDGE_TOLERANCE) - orbit.apoapsis().value()
        });
        let inside = inner.map_or(f64::INFINITY, |r| {
            orbit.periapsis().value() - r.value() * (1.0 - ZONE_EDGE_TOLERANCE)
        });
        Some(outside.min(inside))
    }

    /// How far inside its zone at its pair's evaluated state the body lies at `t`, m, as the
    /// fate's zone search reads it (P14.T9.d): `None` where the body is not present then, for a
    /// zone no pair the engine ran bounds, and for a host built from its models alone. The census
    /// of the binary hosts reads it.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn zone_clearance_at(&self, t: UniverseTime) -> Option<f64> {
        if self.at(t).state() != BodyState::Present {
            return None;
        }
        let (segment, _) = self.segment_at(t);
        self.zone_clearance(segment, t)
    }

    /// What pair event `event` leaves of the body on `segment`, with the mass the body orbits
    /// afterwards, its host's centre moving off at `kick`, the pair's share of it where the host
    /// holds other stars too (P14.T28.g): the supernova's step from the host's mass just before the
    /// event to just after it, or `None` where neither the mass nor the centre moves, which leaves
    /// the segment as it is.
    fn step(
        &self,
        segment: &Segment,
        event: PairEvent,
        kick: SystemVelocity,
    ) -> Option<(Aftermath, SolarMasses)> {
        let (mut before, mut after, mut paired_after) =
            (SolarMasses::ZERO, SolarMasses::ZERO, SolarMasses::ZERO);
        for k in 0..self.host.stars.len() {
            let (b, a) = match self.host.paired[k] {
                Some((pair, component)) if pair == event.pair => {
                    let timeline = self.host.timelines[pair];
                    let mass = |age: f64| {
                        timeline.state_at(Years::new(age)).stars()[component.index()].mass()
                    };
                    let a = mass(event.age);
                    paired_after = paired_after + a;
                    (mass(event.age.next_down().max(0.0)), a)
                }
                Some(_) | None => {
                    let m = self.member_mass(k, event.at);
                    (m, m)
                }
            };
            before = before + b;
            after = after + a;
        }
        let k = kick.metres_per_second();
        let still = k.iter().all(|v| v.total_cmp(&0.0).is_eq());
        if still && before.total_cmp(&after).is_eq() {
            return None;
        }
        let orbit = self.orbit_in(segment, event.at, before);
        let mu_per_solar_mass =
            self.body.orbit.gravitational_parameter().value() / self.host.initial_mass().value();
        let mu_after = GravitationalParameter::new(mu_per_solar_mass * after.value());
        let share = if after.value() > 0.0 {
            paired_after.value() / after.value()
        } else {
            0.0
        };
        let kick = SystemVelocity::new([k[0] * share, k[1] * share, k[2] * share]);
        Some((
            supernova(&orbit, event.at, mu_after, kick, self.body.density),
            after,
        ))
    }
}

/// The state and orbit of `body`, alone on `host`, at `t`: [`BodyFate::resolve`] then
/// [`BodyFate::at`], the fate transform of P14.T28. A body among others on its host takes
/// [`BodyFate::resolve_all`] or [`BodyFate::resolve_among`], for the scattering after a supernova.
///
/// # Panics
///
/// As [`BodyFate::at`].
#[must_use]
pub fn state_at(body: &FateBody, host: &FateHost<'_>, t: UniverseTime) -> FateAt {
    BodyFate::resolve(body, host).at(t)
}

/// The death of `star`, if it has died by the end of the clock window, when its model holds it at
/// no cost.
fn death_of(star: &StarModel) -> Option<StarDeath> {
    if !star
        .state_at(ClockWindow::END)
        .is_some_and(|state| state.phase().is_remnant())
    {
        return None;
    }
    let death = star.death()?;
    let remnant = star.remnant()?;
    let at = clock_time(star, death.age())?;
    let progenitor = death.progenitor();
    let kick = star
        .natal_kick()
        .map_or(SystemVelocity::ZERO, kick_velocity);
    Some(StarDeath {
        at,
        sudden: death.kind().is_sudden(),
        before: progenitor.helium_core_mass() + progenitor.envelope_mass(),
        after: remnant.mass(),
        kick,
    })
}

/// The velocity a natal kick gives its remnant, m s⁻¹ along the system frame's axes, which are the
/// galactic axes' (the vertical slice's note).
#[must_use]
fn kick_velocity(kick: NatalKick) -> SystemVelocity {
    let [x, y, z] = kick.direction().components();
    let speed = kick.speed().value();
    SystemVelocity::new([x * speed, y * speed, z * speed])
}

/// The clock time at which `star` is `age` old, in the arithmetic of plan 06's clock deaths, or
/// `None` beyond the clock.
fn clock_time(star: &StarModel, age: Years) -> Option<UniverseTime> {
    let years = age.value() - star.age_at_epoch().value();
    Span::from_seconds_f64(years * SECONDS_PER_JULIAN_YEAR)
        .and_then(|span| UniverseTime::EPOCH.checked_add(span))
}

/// The mass of the host star `model` at `t`, M☉: its track's, then its remnant's from a sudden
/// death on.
///
/// Just before a sudden death, where the track's age and the clock's round apart, the progenitor's
/// mass stands in for a remnant's the track already gives.
fn star_mass(model: &StarModel, death: Option<StarDeath>, t: UniverseTime) -> SolarMasses {
    match death {
        Some(death) if death.sudden && t >= death.at => death.after,
        Some(death) if death.sudden => {
            let (phase, mass) = living_mass(model, t);
            if phase.is_remnant() {
                death.before
            } else {
                mass
            }
        }
        Some(_) | None => living_mass(model, t).1,
    }
}

/// The phase and the mass of the host star `model` at `t`, a time at which a body of it exists:
/// the mass read smoothly in time ([`StarModel::phase_and_mass_at`], P14.T45.d).
fn living_mass(model: &StarModel, t: UniverseTime) -> (Phase, SolarMasses) {
    model
        .phase_and_mass_at(t)
        .expect("a host star has formed by the time its body has, 0.3 Myr at the earliest")
}

#[cfg(test)]
impl BodyFate<'_> {
    /// For P14.T45.d's tests: the phase excess at `t` of the law the body follows then, rad, by
    /// the 16-point rule and by the 32-point rule on the same intervals; the phase ∫ n dt gained
    /// from the anchor, rad, whose magnitude the rounding allowance scales; and the along-track
    /// factor a √((1 + e) ÷ (1 − e)) at `t`, m. `None` unless the body is present at `t` on an
    /// orbit that evolves.
    pub(crate) fn quadrature_at(&self, t: UniverseTime) -> Option<[f64; 4]> {
        use crate::orbit::seconds_between;
        use crate::planetary::drift::composite_excess_by;
        use crate::tables::gauss_legendre::{GL32_NODES, GL32_WEIGHTS};
        if self.at(t).state() != BodyState::Present {
            return None;
        }
        let (segment, next) = self.segment_at(t);
        let law = self.law(segment, next, t)?;
        let n = |at: UniverseTime| TAU / law.shape_at(at).period().value();
        let n_anchor = law.n_anchor_rad_per_s;
        let by16 = law.phase_excess(t);
        let by32 = composite_excess_by(
            (&GL32_NODES, &GL32_WEIGHTS),
            law.anchor,
            t,
            n_anchor,
            &law.breaks,
            n,
        );
        let gained = by16 + n_anchor * seconds_between(law.anchor, t);
        let shape = law.shape_at(t);
        let e = shape.eccentricity().value();
        let along = shape.semi_major_axis().value() * ((1.0 + e) / (1.0 - e)).sqrt();
        Some([by16, by32, gained, along])
    }
}

mod scatter;

#[cfg(test)]
mod binary_host_tests;
#[cfg(test)]
mod tests;
