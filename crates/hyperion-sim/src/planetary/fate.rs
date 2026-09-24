//! What has become of a body at a time: plan 14's fate transform and the states it produces
//! (design notes 1, 11 and 12, P14.T28).
//!
//! A system is generated as it was born, and time enters only here: [`state_at`] turns a body's
//! primordial orbit into its state and orbit at a time, and is the only place that does. A body is
//! [`BodyState::NotYetFormed`] before its formation age (P14.T28.a,
//! [`hosts::young`](crate::planetary::hosts::young)), then [`BodyState::Present`] on an orbit that
//! circularises under tides (P14.T8.e) and widens as its host loses mass, until its host swallows
//! it (P14.T28.b) or a supernova unbinds or disrupts it (P14.T28.c,
//! [`hosts::evolved`](crate::planetary::hosts::evolved)). The state sequence of every body is a
//! prefix of not yet formed → present → destroyed or unbound, and its elements are continuous in
//! time except at a supernova.
//!
//! # The transform
//!
//! [`BodyFate::resolve`] fixes a body's history once, from its [`FateBody`] and its [`FateHost`]:
//!
//! 1. Its formation time: the clock time of its formation age on its host.
//! 2. Segments of its life between its host's sudden deaths. In each, the orbit is the segment's
//!    elements circularised to the host's age (the first segment only) and then expanded from the
//!    segment's reference mass to the mass the body orbits then, so a(t) = a₀ M₀ ÷ M(t).
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
//!
//! [`BodyFate::at`] then reads the history at any time, so every query of one body agrees with
//! every other, whatever their order: the history depends on the body and its host alone, never on
//! the time asked about. [`state_at`] is both in one call.
//!
//! # The seam to the generator
//!
//! `SystemContext` (P14.T1.d) and the placer (P14.T8) are being built beside this, so the transform
//! takes plain values, which P14.T30.b fills from them:
//!
//! - [`FateHost`]: the [`StarModel`]s of the stars the body orbits (ruling 34), from the context:
//!   one star for a planet of a star, and every star below the pair for a circumbinary planet,
//!   which orbits the pair's total mass. A companion's planets have their own star alone as host,
//!   so they see its mass loss and not their companion's: until P11.T4's post-explosion binary
//!   orbits, plan 14's T28.c treats a companion's planets as a single star's.
//! - [`FateBody`]: its [`Formation`] (drawn with the body from its seed, ID, mass and its disc's
//!   lifetime, P14.T30.a), its primordial elements about its host's initial mass, its mass, its
//!   bulk density for the Roche test, and T8.e's circularisation time.
//! - Moons go with their planet: a moon's state is its planet's (P14.T30.b).
//!
//! # In the vertical slice
//!
//! There is no kick law until P06.T19, so every [`StarModel::natal_kick`] is `None` and a
//! supernova applies a zero kick (ruling 33); the transform applies the kick as soon as the model
//! returns one, which moves output with P06.T19's bump. The protoplanetary disc body of T28.a
//! arrives with phase D's belts (T21), and T28.d–e (white dwarf pollution and second-generation
//! planets) are not in the slice.

use std::error::Error;
use std::fmt;

use crate::coords::SystemVelocity;
use crate::orbit::KeplerElements;
use crate::planetary::hosts::evolved::{
    Aftermath, Circularisation, circularised, circularised_axis, engulfment_reach, expanded,
    expanded_axis, first_engulfment, supernova,
};
use crate::planetary::hosts::young::Formation;
use crate::planetary::record::BodyOrbit;
use crate::stellar::StarState;
use crate::stellar::system::StarModel;
use crate::time::{ClockWindow, Span, UniverseTime};
use crate::units::consts::SECONDS_PER_JULIAN_YEAR;
use crate::units::{
    EarthMasses, GravitationalParameter, KilogramsPerCubicMetre, Metres, SolarMasses, Years,
};

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

/// What the fate transform reads of a body: its formation, its primordial orbit, its mass, its
/// bulk density and how its orbit circularises.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FateBody {
    formation: Formation,
    orbit: KeplerElements,
    mass: EarthMasses,
    density: KilogramsPerCubicMetre,
    circularisation: Circularisation,
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
        })
    }

    /// The same body, circularising as `circularisation` says (P14.T8.e).
    #[must_use]
    pub const fn with_circularisation(self, circularisation: Circularisation) -> Self {
        Self {
            circularisation,
            ..self
        }
    }

    /// When the body forms.
    #[must_use]
    pub const fn formation(&self) -> &Formation {
        &self.formation
    }

    /// The primordial orbit, about the host's initial mass.
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
}

impl<'a> FateHost<'a> {
    /// The single star `star`.
    #[must_use]
    pub fn star(star: &'a StarModel) -> Self {
        Self { stars: vec![star] }
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
        Ok(Self { stars })
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
}

/// A body's state and orbit at a time: what [`state_at`] and [`BodyFate::at`] return.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FateAt {
    state: BodyState,
    orbit: Option<KeplerElements>,
    valid_until: Option<UniverseTime>,
}

impl FateAt {
    /// The body's state.
    #[must_use]
    pub const fn state(&self) -> BodyState {
        self.state
    }

    /// The body's elements at the time, about the mass it orbits then, if it is present.
    ///
    /// They are the osculating elements of design note 11's closed forms: the mean anomaly at the
    /// epoch is kept as the orbit circularises and expands, and the period follows the axis.
    #[must_use]
    pub const fn orbit(&self) -> Option<&KeplerElements> {
        self.orbit.as_ref()
    }

    /// The next time the body's state changes, or its orbit steps (a supernova), if that is
    /// inside the clock window: its formation, its destruction or unbinding, a host star's sudden
    /// death. Between, the orbit changes only slowly, by the tides and the winds.
    #[must_use]
    pub const fn valid_until(&self) -> Option<UniverseTime> {
        self.valid_until
    }

    /// The record's orbit section: the elements and [`FateAt::valid_until`], if the body is
    /// present (P14.T34).
    #[must_use]
    pub fn body_orbit(&self) -> Option<BodyOrbit> {
        self.orbit
            .map(|elements| BodyOrbit::new(elements, self.valid_until))
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
    /// The remnant's natal kick, m s⁻¹ along the system frame's axes; zero until P06.T19.
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
/// // The Sun 13.5 Gyr after its birth, a white dwarf since 12.46 Gyr.
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
/// assert!((12.2e9..12.33e9).contains(&age), "engulfed at {age:e} yr");
/// assert_eq!(fate.at(UniverseTime::EPOCH).state(), BodyState::Destroyed { cause, at });
///
/// let earth = at_1_au(1.0, 5_513.0)?;
/// let now = BodyFate::resolve(&earth, &host).at(UniverseTime::EPOCH);
/// let orbit = now.orbit().ok_or("the Earth survives")?;
/// let au = orbit.semi_major_axis().value() / 1.495_978_707e11;
/// assert!((au - 1.924).abs() < 1e-3, "at {au} au");
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
}

impl<'a> BodyFate<'a> {
    /// The history of `body` on `host`.
    ///
    /// It reads the host's stars up to the end of the clock window, or to the last death where
    /// every star is dead by then; the cost is a few state evaluations per star, and for a body
    /// that an evolved host can reach, about 130 more for the engulfment search.
    ///
    /// A body whose formation age falls at or after one of its host stars' deaths never forms,
    /// since the death ends its disc; it needs a disc that outlives a star of over 13 M☉, which
    /// P06.T15.c's lifetimes make vanishingly rare.
    #[must_use]
    pub fn resolve(body: &'a FateBody, host: &'a FateHost<'a>) -> Self {
        let deaths: Vec<Option<StarDeath>> = host.stars.iter().map(|star| death_of(star)).collect();
        let formed_at = clock_time(host.first(), Years::from(body.formation.formed_at()))
            .filter(|formed| deaths.iter().flatten().all(|death| death.at > *formed));
        let mut fate = Self {
            body,
            host,
            deaths,
            formed_at,
            segments: Vec::new(),
            ending: None,
        };
        if let Some(formed) = formed_at {
            fate.segments.push(Segment {
                start: formed,
                orbit: body.orbit,
                reference: host.initial_mass(),
                circularises: true,
            });
            // A body that forms after the clock window, about a host alive then, has no history
            // the host's models can tell yet.
            if formed <= fate.known_until() {
                fate.live();
            }
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
        let Some(formed) = self.formed_at else {
            return FateAt {
                state: BodyState::NotYetFormed,
                orbit: None,
                valid_until: None,
            };
        };
        if t < formed {
            return FateAt {
                state: BodyState::NotYetFormed,
                orbit: None,
                valid_until: within(formed),
            };
        }
        if let Some(ending) = self.ending
            && ending.ended_at().is_some_and(|at| t >= at)
        {
            return FateAt {
                state: ending,
                orbit: None,
                valid_until: None,
            };
        }
        let index = self.segments.partition_point(|segment| segment.start <= t) - 1;
        let segment = &self.segments[index];
        let next = self.segments.get(index + 1).map_or_else(
            || self.ending.and_then(|ending| ending.ended_at()),
            |segment| Some(segment.start),
        );
        FateAt {
            state: BodyState::Present,
            orbit: Some(self.orbit_in(segment, t, self.host_mass(t))),
            valid_until: next.and_then(within),
        }
    }

    /// Follows the body from its formation, no later than [`BodyFate::known_until`], through its
    /// host's sudden deaths until it ends or the host's known history does.
    fn live(&mut self) {
        let mut sudden: Vec<(UniverseTime, usize)> = self
            .deaths
            .iter()
            .enumerate()
            .filter_map(|(k, death)| death.filter(|d| d.sudden).map(|d| (d.at, k)))
            .collect();
        sudden.sort_unstable();
        let last = self.known_until();
        let reach = engulfment_reach(self.body.mass);
        let mut pending = sudden.into_iter();
        loop {
            let segment = *self.segments.last().expect("a formed body has a segment");
            let next = pending.next();
            let end = next.map_or(last, |(at, _)| {
                at.checked_sub(Span::new(0, 1).expect("one nanosecond"))
                    .expect("a death after a formation is not the clock's first instant")
            });
            if let Some(at) = self.engulfment(&segment, end, reach) {
                self.ending = Some(BodyState::Destroyed {
                    cause: DestructionCause::Engulfed,
                    at,
                });
                return;
            }
            let Some((at, star)) = next else {
                return;
            };
            let (aftermath, reference) = self.explode(&segment, at, star);
            match aftermath {
                Aftermath::Bound(orbit) => self.segments.push(Segment {
                    start: at,
                    orbit,
                    reference,
                    circularises: false,
                }),
                Aftermath::Disrupted { orbit, at: when } => {
                    if let Some(orbit) = orbit {
                        self.segments.push(Segment {
                            start: at,
                            orbit,
                            reference,
                            circularises: false,
                        });
                    }
                    self.ending = Some(BodyState::Destroyed {
                        cause: DestructionCause::TidallyDisrupted,
                        at: when,
                    });
                    return;
                }
                Aftermath::Unbound => {
                    self.ending = Some(BodyState::Unbound { at });
                    return;
                }
            }
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
        for (k, model) in self.host.stars.iter().enumerate() {
            let (b, a) = if k == star {
                (death.before, death.after)
            } else {
                let m = star_mass(model, self.deaths[k], at);
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
        self.host
            .stars
            .iter()
            .zip(&self.deaths)
            .fold(SolarMasses::ZERO, |sum, (model, death)| {
                sum + star_mass(model, *death, t)
            })
    }

    /// The largest radius any of the host's stars has had by `t`, among those that had not died
    /// by the segment's start, m.
    fn largest_radius(&self, segment: &Segment, t: UniverseTime) -> f64 {
        self.host
            .stars
            .iter()
            .zip(&self.deaths)
            .filter(|(_, death)| death.is_none_or(|d| d.at > segment.start))
            .map(|(model, _)| Metres::from(model.max_radius_until(t)).value())
            .fold(0.0, f64::max)
    }
}

/// The state and orbit of `body` on `host` at `t`: [`BodyFate::resolve`] then [`BodyFate::at`],
/// the fate transform of P14.T28.
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
    let kick = star.natal_kick().map_or(SystemVelocity::ZERO, |kick| {
        let [x, y, z] = kick.direction().components();
        let speed = kick.speed().value();
        SystemVelocity::new([x * speed, y * speed, z * speed])
    });
    Some(StarDeath {
        at,
        sudden: death.kind().is_sudden(),
        before: progenitor.helium_core_mass() + progenitor.envelope_mass(),
        after: remnant.mass(),
        kick,
    })
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
            let state = living_state(model, t);
            if state.phase().is_remnant() {
                death.before
            } else {
                state.mass()
            }
        }
        Some(_) | None => living_state(model, t).mass(),
    }
}

/// The state of the host star `model` at `t`, a time at which a body of it exists.
fn living_state(model: &StarModel, t: UniverseTime) -> StarState {
    model
        .state_at(t)
        .expect("a host star has formed by the time its body has, 0.3 Myr at the earliest")
}

#[cfg(test)]
mod tests;
