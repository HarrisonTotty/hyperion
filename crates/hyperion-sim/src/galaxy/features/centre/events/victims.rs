//! The victims of the central black hole's tidal disruptions (plan 09, P09.T30.b).
//!
//! Every disruption in the source horizon ([`SourceHorizon`], `[−(H + L), +H]`) has a victim: a
//! star of the centre's living classes on a parabolic orbit about the black hole, whose pericentre
//! passage is the disruption's time. It is a feature-level member of the centre's list, of class
//! [`ClassId::TIDAL_DISRUPTION_VICTIM`](crate::galaxy::catalogue_classes::ClassId::TIDAL_DISRUPTION_VICTIM),
//! and the first class in the list's order (plan 09, Design
//! note 16): the horizon's disruptions in time order (then by ID) take indices 1, 2, … after the
//! black hole's 0, so a class registered after them takes the indices after theirs and moves no
//! victim. The victims are not placed on the centre's grid, and the Kepler members' loss cone stays
//! empty (P09.T25): a victim is the brainstorm's list member "separate from the Kepler members".
//! Plan 03's `resolve` answers a victim's ID through
//! [`resolve_centre_member`](super::super::members::resolve_centre_member).
//!
//! # Draws
//!
//! A victim's draws are its disruption's own event stream from word 0
//! ([`EventKey::event_stream`](crate::rng::EventKey::event_stream)), where P09.T30.a draws none,
//! so no disruption's ID or time moves:
//!
//! - words 0–2 ([`ORBIT_WORDS`]): the orbit's orientation, the pericentre's direction (`cos θ` and
//!   the azimuth, isotropic) and the angle of the velocity about it;
//! - from word 3, attempt `k` at word `3 + 3k` ([`STAR_ATTEMPT_WORDS`]): a [`Mark`] that picks one
//!   of the centre's living classes by its expected count ([`CentreClass::expected`]), the star's
//!   age at the disruption, uniform in its class's age component (from
//!   [`MIN_STAR_AGE`] at the youngest), and its initial mass from the galaxy's mass function in the
//!   class's range below the turn-off at that age, as a centre member's star is drawn (P09.T27). An
//!   attempt whose range is empty at its age is passed over; [`STAR_ATTEMPTS`] are read at most.
//!
//! The star is plan 06's at that age, with the median of its draws ([`StarDraws::median`]): its
//! own draws would come from streams keyed by its ID, and the victim takes none but its event's.
//! A living class's star is below the turn-off, so every victim is on the main sequence, before it
//! in the youngest components, or, below 0.1 M☉, on plan 06's cooling fits.
//!
//! # The outcome
//!
//! A star is swallowed whole when the black hole's mass is at or above its Hills mass, where its
//! tidal radius `r_t = R★ (M• ÷ M★)^⅓` equals `2 r_g = 2 G M• ÷ c²` (Stone and Metzger 2016, MNRAS
//! 455, 859, §2.2), `M_H = (R★ c² ÷ 2G)^{3⁄2} M★^{−½}`: 1.143 × 10⁸ M☉ for a 1 M☉, 1 R☉ star
//! ([`hills_mass`]). Stone and Metzger's rule near their equation 33 (§4.2), "Eq. 27 if
//! `M• < M_Hills(M★)`, but is 0 otherwise", is a test on the star, so it is taken per victim and
//! recorded as its [`DisruptionOutcome`], never dropped: every disruption keeps its ID, its time
//! and its victim. At the Milky Way's 4.3 × 10⁶ M☉ every star the centre holds is disrupted (an M
//! dwarf of 0.1 M☉ and 0.12 R☉ has a Hills mass near 1.5 × 10⁷ M☉). Only a disrupted victim has a
//! tail ([`luminosity`](super::luminosity)).
//!
//! # The orbit
//!
//! The victim's orbit is a parabola about the black hole alone, `μ = G M•` (lean, 2026-10-29; plan
//! 09's Risks, "P09.T30.b as built"): the brainstorm's "near-parabolic" orbit, read as the
//! parabola that the theory of tidal disruption takes for a star from the loss cone, whose
//! binding energy is far below its debris's spread (Rees 1988; Stone and Metzger 2016, §2.2). Its
//! pericentre is its tidal radius for a disrupted victim (the penetration factor β = 1) and the
//! black hole's Schwarzschild radius `2 r_g` for one swallowed whole, whose tidal radius lies
//! inside it ([`TidalDisruptionVictim::pericentre`]), so the parabola is slower than light
//! everywhere before the event. The state at the event is held exactly, and a state at any other
//! time is that one propagated ([`KeplerOrbit::propagate`]).
//!
//! A victim exists until its disruption, when it passes its pericentre, and from then on is "no
//! such system" ([`TidalDisruptionVictim::state_at`]). Its record's epoch position and velocity
//! are its parabola's at the epoch: a victim disrupted before the epoch has them only formally
//! (some 60 ly out for the horizon's oldest), since it no longer exists there. Its motion is its
//! own and not plan 09's Kepler regime ([`position_velocity_at`](crate::galaxy::motion::position_velocity_at),
//! which takes μ from the mass inside the epoch radius): a caller asks the victim.
//!
//! # Reach
//!
//! Before its disruption a victim moves at most at the escape speed `√(2 G M• ÷ r)` from the black
//! hole alone, so `t_d − t` before the event it lies within its pericentre plus the radial plunge
//! [`full_scan_radius`] about `G M•` of the black hole ([`TidalDisruptionVictim::reach_at`]): the
//! victims' speed bound, which the centre's range-query source reads to pass over victims that
//! cannot reach a sphere. Within the clock window a victim that still exists has its disruption
//! within 2H, under about 2.3 ly from the black hole at Milky Way values (2.29 ly).

use std::error::Error;
use std::fmt;

use crate::coords::{GalacticPosition, GalacticVelocity};
use crate::events::{EventId, TimeWindow};
use crate::galaxy::features::centre::classes::{CentreClass, CentreClassKind, age_components};
use crate::galaxy::features::centre::members::{CentreMemberRecord, centre_composition};
use crate::galaxy::features::centre::{BuildCentreError, CentreModel};
use crate::galaxy::imf::MassFunction;
use crate::galaxy::motion::{KeplerOrbit, MotionState, StateAt, full_scan_radius};
use crate::galaxy::placement::{SystemOrigin, SystemRecord};
use crate::galaxy::{Galaxy, PointLy, Population};
use crate::id::{CentreMemberId, MemberSlot, SystemId};
use crate::math;
use crate::rng::{Mark, Seed, Stream};
use crate::stellar::draws::StarDraws;
use crate::stellar::sse::{main_sequence_state, turn_off_mass};
use crate::stellar::{Composition, Phase, StarState, evolve};
use crate::time::{SourceHorizon, UniverseTime};
use crate::units::consts::{GM_SUN, METRES_PER_LIGHT_YEAR, SOLAR_RADIUS_M, SPEED_OF_LIGHT};
use crate::units::consts::{METRES_PER_SECOND_PER_KILOMETRE_PER_SECOND, SECONDS_PER_JULIAN_YEAR};
use crate::units::{
    GravitationalParameter, LightYears, Metres, Seconds, SolarMasses, SolarRadii, Years,
};

use super::disruptions::{
    BuildTidalDisruptionModelError, TidalDisruption, TidalDisruptionModel, disruption_rate,
    tidal_disruptions,
};
use super::luminosity::DisruptionTail;

/// The words of a disruption's event stream its victim's orbit takes, words 0–2: the
/// pericentre's direction (two uniforms) and the angle of the velocity about it (one).
pub const ORBIT_WORDS: u64 = 3;

/// The words each attempt at a victim's star takes, from word [`ORBIT_WORDS`]: the class's mark,
/// the age's uniform and the initial mass's uniform.
pub const STAR_ATTEMPT_WORDS: u64 = 3;

/// The most attempts at a victim's star, 64: an attempt fails only where its class's range lies
/// above the turn-off at its age, a few classes at the edges of their bands that hold under 10⁻³
/// of the living stars, so the 64th is never reached (a debug assertion says so).
pub const STAR_ATTEMPTS: u32 = 64;

/// The youngest age a victim's star is drawn at, 10⁴ years: the floor at which a centre member's
/// turn-off is read (P09.T27), which the continuous component's span from −H is cut at.
pub const MIN_STAR_AGE: Years = Years::new(1e4);

/// The relative margin a victim's reach is widened by, 10⁻⁹: it covers the rounding of the closed
/// form and of the propagation, some 10⁻¹² relative.
const REACH_MARGIN: f64 = 1e-9;

/// What became of a disruption's victim (module documentation): the outcome recorded on the
/// event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DisruptionOutcome {
    /// The star was torn apart at its tidal radius, outside the black hole's horizon: its debris
    /// falls back and makes a tail.
    Disrupted,
    /// The black hole's mass is at or above the star's Hills mass, so the star crossed the horizon
    /// whole: no debris and no tail (Stone and Metzger 2016, §2.2 and near equation 33, §4.2).
    SwallowedWhole,
}

/// The tidal radius `r_t = R★ (M• ÷ M★)^⅓` of a star of present mass `star_mass` and radius
/// `star_radius` about a black hole of `black_hole_mass` (Stone and Metzger 2016, §2.2): where the
/// hole's tide at pericentre matches the star's self-gravity.
///
/// # Examples
///
/// The Sun about the Milky Way's black hole, inside an astronomical unit:
///
/// ```
/// use hyperion_sim::galaxy::features::centre::events::victims::tidal_radius;
/// use hyperion_sim::units::{AstronomicalUnits, SolarMasses, SolarRadii};
///
/// let r_t = tidal_radius(SolarMasses::new(4.3e6), SolarMasses::new(1.0), SolarRadii::new(1.0));
/// assert!((AstronomicalUnits::from(r_t).value() - 0.756).abs() < 0.001);
/// ```
#[must_use]
pub fn tidal_radius(
    black_hole_mass: SolarMasses,
    star_mass: SolarMasses,
    star_radius: SolarRadii,
) -> Metres {
    Metres::new(
        star_radius.value()
            * SOLAR_RADIUS_M
            * math::cbrt(black_hole_mass.value() / star_mass.value()),
    )
}

/// The Schwarzschild radius `2 r_g = 2 G M ÷ c²` of a black hole of `mass`, with the IAU 2015
/// nominal `GM☉` ([`GM_SUN`]).
#[must_use]
pub fn schwarzschild_radius(mass: SolarMasses) -> Metres {
    Metres::new(2.0 * GM_SUN * mass.value() / (SPEED_OF_LIGHT * SPEED_OF_LIGHT))
}

/// The Hills mass of a star of present mass `star_mass` and radius `star_radius`: the black hole's
/// mass at which its tidal radius equals `2 r_g`, `M_H = (R★ c² ÷ 2G)^{3⁄2} M★^{−½}` (Stone and
/// Metzger 2016, §2.2), written as `G M_H = (R★ c² ÷ 2)^{3⁄2} (G M★)^{−½}` with the IAU 2015
/// nominal `R☉` and `GM☉`.
///
/// # Examples
///
/// The plan's figure, 1.14 × 10⁸ M☉ for a 1 M☉, 1 R☉ star:
///
/// ```
/// use hyperion_sim::galaxy::features::centre::events::victims::hills_mass;
/// use hyperion_sim::units::{SolarMasses, SolarRadii};
///
/// let m_h = hills_mass(SolarMasses::new(1.0), SolarRadii::new(1.0)).value();
/// assert!((m_h / 1.143e8 - 1.0).abs() < 1e-3, "{m_h:e}");
/// ```
#[must_use]
pub fn hills_mass(star_mass: SolarMasses, star_radius: SolarRadii) -> SolarMasses {
    let half = 0.5 * star_radius.value() * SOLAR_RADIUS_M * SPEED_OF_LIGHT * SPEED_OF_LIGHT;
    let gm = half * half.sqrt() / (GM_SUN * star_mass.value()).sqrt();
    SolarMasses::new(gm / GM_SUN)
}

/// The outcome for a star of present mass `star_mass` and radius `star_radius` that enters the
/// loss cone of a black hole of `black_hole_mass`: swallowed whole at or above its
/// [`hills_mass`], disrupted below it (module documentation).
#[must_use]
pub fn outcome_of(
    black_hole_mass: SolarMasses,
    star_mass: SolarMasses,
    star_radius: SolarRadii,
) -> DisruptionOutcome {
    if black_hole_mass.value() >= hills_mass(star_mass, star_radius).value() {
        DisruptionOutcome::SwallowedWhole
    } else {
        DisruptionOutcome::Disrupted
    }
}

/// One disruption's victim (module documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct TidalDisruptionVictim {
    disruption: TidalDisruption,
    index: u16,
    member: CentreMemberRecord,
    star_mass: SolarMasses,
    star_radius: SolarRadii,
    star_phase: Phase,
    black_hole_mass: SolarMasses,
    outcome: DisruptionOutcome,
    /// The orbit's state at the disruption, at pericentre.
    at_pericentre: KeplerOrbit,
    tail: Option<DisruptionTail>,
}

impl TidalDisruptionVictim {
    /// Its member ID, `0xF000_0007_0000_0000` plus its index.
    ///
    /// # Panics
    ///
    /// Never: a victim's index is 1–65,535, which the centre's list index holds.
    #[must_use]
    pub fn id(&self) -> CentreMemberId {
        CentreMemberId::new(MemberSlot::FeatureLevel { index: self.index })
            .expect("a victim's index is under the centre's 65,536")
    }

    /// Its index on the centre's list, 1 upward.
    #[must_use]
    pub const fn index(&self) -> u16 {
        self.index
    }

    /// Its disruption: its event's ID and time.
    #[must_use]
    pub const fn disruption(&self) -> TidalDisruption {
        self.disruption
    }

    /// The instant of its disruption, its pericentre passage.
    #[must_use]
    pub const fn time(&self) -> UniverseTime {
        self.disruption.time()
    }

    /// Its member record: its plan 03 record, its class, its composition and its velocity at the
    /// epoch.
    #[must_use]
    pub const fn member(&self) -> &CentreMemberRecord {
        &self.member
    }

    /// Its plan 03 record, of origin `CentreMember`.
    #[must_use]
    pub const fn record(&self) -> &SystemRecord {
        self.member.record()
    }

    /// The living class its star was drawn from.
    ///
    /// # Panics
    ///
    /// Never: a victim's member record always holds its class.
    #[must_use]
    pub fn class(&self) -> &CentreClass {
        self.member
            .class()
            .expect("a victim's member record holds its class")
    }

    /// Its star's present mass at the disruption, M☉.
    #[must_use]
    pub const fn star_mass(&self) -> SolarMasses {
        self.star_mass
    }

    /// Its star's radius at the disruption, nominal solar radii.
    #[must_use]
    pub const fn star_radius(&self) -> SolarRadii {
        self.star_radius
    }

    /// Its star's phase at the disruption.
    #[must_use]
    pub const fn star_phase(&self) -> Phase {
        self.star_phase
    }

    /// The black hole's mass, M☉.
    #[must_use]
    pub const fn black_hole_mass(&self) -> SolarMasses {
        self.black_hole_mass
    }

    /// Its outcome, recorded on its disruption.
    #[must_use]
    pub const fn outcome(&self) -> DisruptionOutcome {
        self.outcome
    }

    /// Its star's tidal radius about the black hole ([`tidal_radius`]).
    #[must_use]
    pub fn tidal_radius(&self) -> Metres {
        tidal_radius(self.black_hole_mass, self.star_mass, self.star_radius)
    }

    /// Its star's Hills mass ([`hills_mass`]).
    #[must_use]
    pub fn hills_mass(&self) -> SolarMasses {
        hills_mass(self.star_mass, self.star_radius)
    }

    /// Its orbit's pericentre distance from the black hole: its tidal radius if disrupted, the
    /// Schwarzschild radius if swallowed whole (module documentation).
    #[must_use]
    pub fn pericentre(&self) -> Metres {
        self.at_pericentre.distance()
    }

    /// Its orbit at its disruption, at pericentre: a parabola about the black hole alone.
    #[must_use]
    pub const fn orbit_at_disruption(&self) -> &KeplerOrbit {
        &self.at_pericentre
    }

    /// Its tail, the light of its debris falling back, or `None` for a star swallowed whole.
    #[must_use]
    pub const fn tail(&self) -> Option<&DisruptionTail> {
        self.tail.as_ref()
    }

    /// Where it is at `t` and how it moves there: on its parabola up to its disruption, at
    /// pericentre at the disruption itself, and [`StateAt::NoSuchSystem`] after it (module
    /// documentation). Whether its star is born yet at `t` is its record's question
    /// ([`SystemRecord::existence_at`]).
    ///
    /// # Panics
    ///
    /// Never: a parabola about the black hole is under 10⁴ ly from it over any span the clock
    /// holds, which the galactic frame addresses.
    #[must_use]
    pub fn state_at(&self, t: UniverseTime) -> StateAt {
        if t > self.time() {
            return StateAt::NoSuchSystem;
        }
        StateAt::Exists(state_of(&self.orbit_at(t)))
    }

    /// The orbit's state at `t`, before or at the disruption.
    #[must_use]
    fn orbit_at(&self, t: UniverseTime) -> KeplerOrbit {
        if t == self.time() {
            return self.at_pericentre;
        }
        self.at_pericentre.propagate(span_seconds(self.time(), t))
    }

    /// A bound on its distance from the black hole at `t`, or `None` after its disruption, where
    /// it is no system: its pericentre plus the radial plunge about `G M•` over the span to its
    /// disruption ([`full_scan_radius`]), widened by 10⁻⁹ (module documentation, "Reach").
    #[must_use]
    pub fn reach_at(&self, t: UniverseTime) -> Option<LightYears> {
        if t > self.time() {
            return None;
        }
        let mu = self.at_pericentre.gravitational_parameter();
        let plunge = full_scan_radius(mu, span_seconds(t, self.time()));
        let pericentre = LightYears::from(self.pericentre());
        Some((pericentre + plunge) * (1.0 + REACH_MARGIN))
    }
}

/// `later − earlier`, s.
///
/// # Panics
///
/// Never: two instants of the clock are within its range of each other.
#[must_use]
fn span_seconds(earlier: UniverseTime, later: UniverseTime) -> Seconds {
    Seconds::new(
        later
            .checked_since(earlier)
            .expect("two instants of the clock are within its span of each other")
            .as_seconds_f64(),
    )
}

/// The motion state of an orbit about the black hole at the galactic origin.
///
/// # Panics
///
/// Never for a victim's orbit: it is under 10⁴ ly from the origin.
#[must_use]
fn state_of(orbit: &KeplerOrbit) -> MotionState {
    let position = GalacticPosition::ORIGIN
        .translated(orbit.displacement())
        .expect("a victim's parabola stays within the galactic frame's addressable range");
    MotionState::new(position, orbit.velocity())
}

/// A victim's star: its living class, its attempt, its initial mass, its age at the disruption
/// and its state there.
#[derive(Debug, Clone)]
struct VictimStar {
    class: CentreClass,
    attempt: u32,
    initial_mass: SolarMasses,
    age: Years,
    state: StarState,
}

/// The victims of a galaxy's central black hole, every disruption's in the source horizon, in
/// list order (module documentation).
///
/// # Examples
///
/// ```no_run
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::features::centre::events::victims::{CentreVictims, DisruptionOutcome};
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::time::UniverseTime;
///
/// let galaxy = Galaxy::from_params(Seed::new(3), GalaxyParams::milky_way_like())?;
/// let victims = CentreVictims::of(&galaxy)?;
/// // The Milky Way's black hole disrupts every star it takes, some 27 in the horizon.
/// assert!(victims.victims().iter().all(|v| v.outcome() == DisruptionOutcome::Disrupted));
/// // The ones still on their way in at the epoch: one in ten galaxies has one.
/// let coming = victims.at(UniverseTime::EPOCH).count();
/// assert!(coming < 5);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct CentreVictims {
    model: TidalDisruptionModel,
    victims: Vec<TidalDisruptionVictim>,
}

impl CentreVictims {
    /// The victims of `galaxy`'s black hole, whose centre is `centre`.
    ///
    /// # Errors
    ///
    /// As [`TidalDisruptionModel::of`], which no galaxy the builder allows reaches.
    pub fn new(
        galaxy: &Galaxy,
        centre: &CentreModel,
    ) -> Result<Self, BuildTidalDisruptionModelError> {
        Self::from_parts(
            galaxy.seed(),
            galaxy.params().black_hole().mass(),
            centre,
            galaxy.mass_function(),
        )
    }

    /// The victims of a black hole of `black_hole_mass` in the universe of `seed`, drawn from the
    /// living classes of `centre` with initial masses on `mass_function`: what [`new`](Self::new)
    /// builds from a galaxy's seed, black hole, centre and mass function, so that a test can sweep
    /// seeds and masses about one centre.
    ///
    /// # Errors
    ///
    /// As [`TidalDisruptionModel::new`].
    pub fn from_parts(
        seed: Seed,
        black_hole_mass: SolarMasses,
        centre: &CentreModel,
        mass_function: &dyn MassFunction,
    ) -> Result<Self, BuildTidalDisruptionModelError> {
        let model = TidalDisruptionModel::new(seed, black_hole_mass)?;
        let (classes, weights) = living_classes(centre);
        let victims = (1..=u16::MAX)
            .zip(horizon_disruptions(&model))
            .map(|(index, disruption)| {
                victim_of(&model, &classes, &weights, mass_function, disruption, index)
            })
            .collect();
        Ok(Self { model, victims })
    }

    /// The victim of `galaxy`'s black hole at list index `index`, whose centre is `centre`, drawn
    /// alone: what [`new`](Self::new)'s list holds there, without drawing the others, for plan
    /// 03's `resolve`. `None` for index 0 (the black hole) or past the victims.
    ///
    /// # Errors
    ///
    /// As [`new`](Self::new).
    pub fn one(
        galaxy: &Galaxy,
        centre: &CentreModel,
        index: u16,
    ) -> Result<Option<TidalDisruptionVictim>, BuildTidalDisruptionModelError> {
        let model = TidalDisruptionModel::of(galaxy)?;
        let Some(disruption) = index
            .checked_sub(1)
            .and_then(|i| horizon_disruptions(&model).into_iter().nth(usize::from(i)))
        else {
            return Ok(None);
        };
        let (classes, weights) = living_classes(centre);
        Ok(Some(victim_of(
            &model,
            &classes,
            &weights,
            galaxy.mass_function(),
            disruption,
            index,
        )))
    }

    /// The victims of `galaxy`'s black hole, from the centre the galaxy holds
    /// ([`Galaxy::centre`]).
    ///
    /// # Errors
    ///
    /// [`BuildCentreVictimsError`] if the centre or the disruptions cannot be built, which no
    /// galaxy the builder allows has.
    pub fn of(galaxy: &Galaxy) -> Result<Self, BuildCentreVictimsError> {
        let centre = galaxy.centre().map_err(BuildCentreVictimsError::Centre)?;
        Self::new(galaxy, centre).map_err(BuildCentreVictimsError::Disruptions)
    }

    /// The disruptions they come from.
    #[must_use]
    pub const fn model(&self) -> &TidalDisruptionModel {
        &self.model
    }

    /// Every victim, in list order: by disruption time, then by event ID.
    #[must_use]
    pub fn victims(&self) -> &[TidalDisruptionVictim] {
        &self.victims
    }

    /// The victim at list index `index`, or `None` for index 0 (the black hole) or past the
    /// victims.
    #[must_use]
    pub fn victim(&self, index: u16) -> Option<&TidalDisruptionVictim> {
        let i = usize::from(index.checked_sub(1)?);
        self.victims.get(i)
    }

    /// How many victims the list holds: the disruptions in the source horizon.
    ///
    /// # Panics
    ///
    /// Never: the victims are numbered by `u16` indices.
    #[must_use]
    pub fn count(&self) -> u16 {
        u16::try_from(self.victims.len()).expect("the victims are numbered by u16 indices")
    }

    /// The victim of the disruption `id` names, which carries its outcome, or `None` if `id` names
    /// no disruption of the source horizon: the outcome as the event's (plan 09, P09.T30.b).
    #[must_use]
    pub fn of_disruption(&self, id: EventId) -> Option<&TidalDisruptionVictim> {
        self.victims.iter().find(|v| v.disruption.id() == id)
    }

    /// The victims that may exist at `t`: those whose disruption is not before it, in list order.
    pub fn at(&self, t: UniverseTime) -> impl Iterator<Item = &TidalDisruptionVictim> + '_ {
        self.victims.iter().filter(move |v| v.time() >= t)
    }
}

/// The victims could not be listed for a galaxy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildCentreVictimsError {
    /// The galaxy's centre could not be built ([`Galaxy::centre`]).
    Centre(BuildCentreError),
    /// Its disruptions could not be ([`TidalDisruptionModel::of`]).
    Disruptions(BuildTidalDisruptionModelError),
}

impl fmt::Display for BuildCentreVictimsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Centre(e) => write!(f, "the galaxy's centre has no victims: {e}"),
            Self::Disruptions(e) => write!(f, "the galaxy's centre has no victims: {e}"),
        }
    }
}

impl Error for BuildCentreVictimsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Centre(e) => Some(e),
            Self::Disruptions(e) => Some(e),
        }
    }
}

/// The victims a black hole of `black_hole_mass` expects in the source horizon: P09.T30.a's rate
/// ([`disruption_rate`]) times the horizon's 264,144 years, 27.31 at the Milky Way's 4.30 × 10⁶ M☉
/// and 125 at 10⁵ M☉. The centre's list spares its indices for them
/// ([`check_feature_list_headroom`](crate::galaxy::features::members::check_feature_list_headroom)).
///
/// # Panics
///
/// In debug builds, unless the mass is finite and positive ([`disruption_rate`]).
#[must_use]
pub fn expected_victims(black_hole_mass: SolarMasses) -> f64 {
    let span = SourceHorizon::END
        .checked_since(SourceHorizon::START)
        .expect("the horizon is on the clock")
        .as_seconds_f64();
    disruption_rate(black_hole_mass).value() * span
}

/// The disruptions of `model` in the source horizon, in list order: by time, then by ID.
#[must_use]
fn horizon_disruptions(model: &TidalDisruptionModel) -> Vec<TidalDisruption> {
    let mut disruptions = Vec::new();
    tidal_disruptions(model, horizon(), &mut disruptions);
    debug_assert!(
        disruptions.len() < usize::from(u16::MAX),
        "{} disruptions in the source horizon, past the centre's list index",
        disruptions.len()
    );
    disruptions
}

/// The source horizon as a window, `[−(H + L), +H]` with both ends in, as
/// [`SourceHorizon::contains`] has it.
///
/// # Panics
///
/// Never: the horizon's end follows its start.
#[must_use]
fn horizon() -> TimeWindow {
    let end = SourceHorizon::END
        .checked_add(crate::time::Span::new(0, 1).expect("1 ns is a span"))
        .expect("+H plus a nanosecond is on the clock");
    TimeWindow::new(SourceHorizon::START, end).expect("the horizon's end follows its start")
}

/// The centre's living classes with members, in the classes' order, with their expected counts.
#[must_use]
fn living_classes(centre: &CentreModel) -> (Vec<CentreClass>, Vec<f64>) {
    let classes: Vec<CentreClass> = centre
        .classes()
        .classes()
        .iter()
        .filter(|c| c.kind == CentreClassKind::Living && c.expected > 0.0)
        .copied()
        .collect();
    let weights = classes.iter().map(|c| c.expected).collect();
    (classes, weights)
}

/// The victim of `disruption` in `model`, at list index `index`, its star drawn from `classes` by
/// `weights` (module documentation).
#[must_use]
fn victim_of(
    model: &TidalDisruptionModel,
    classes: &[CentreClass],
    weights: &[f64],
    mass_function: &dyn MassFunction,
    disruption: TidalDisruption,
    index: u16,
) -> TidalDisruptionVictim {
    let black_hole_mass = model.black_hole_mass();
    let word = disruption.id().word();
    let mut stream = model.series().key().event_stream(word.bin(), word.number());
    let orientation = [stream.uniform(), stream.uniform(), stream.uniform()];
    let composition = centre_composition();
    let star = draw_star(mass_function, &mut stream, classes, weights, &composition);
    let (star_mass, star_radius) = (star.state.mass(), star.state.radius());
    let outcome = outcome_of(black_hole_mass, star_mass, star_radius);
    let pericentre = match outcome {
        DisruptionOutcome::Disrupted => tidal_radius(black_hole_mass, star_mass, star_radius),
        DisruptionOutcome::SwallowedWhole => schwarzschild_radius(black_hole_mass),
    };
    let mu = GravitationalParameter::from_solar_masses(black_hole_mass);
    let at_pericentre = parabola_at_pericentre(mu, pericentre, orientation);
    let at_epoch = if disruption.time() == UniverseTime::EPOCH {
        at_pericentre
    } else {
        at_pericentre.propagate(span_seconds(disruption.time(), UniverseTime::EPOCH))
    };
    let years_since = UniverseTime::EPOCH
        .checked_since(disruption.time())
        .expect("the horizon is on the clock")
        .as_seconds_f64()
        / SECONDS_PER_JULIAN_YEAR;
    let id = SystemId::from(
        CentreMemberId::new(MemberSlot::FeatureLevel { index })
            .expect("a victim's index is under the centre's 65,536"),
    );
    let record = SystemRecord::from_parts(
        id,
        *state_of(&at_epoch).position(),
        SystemOrigin::CentreMember {
            attempt: u16::try_from(star.attempt).expect("an attempt below 64 fits in u16"),
        },
        Population::NuclearDisc,
        star.initial_mass,
        Years::new(star.age.value() + years_since),
    );
    let velocity_km_s = at_epoch
        .velocity()
        .metres_per_second()
        .map(|c| c / METRES_PER_SECOND_PER_KILOMETRE_PER_SECOND);
    let member = CentreMemberRecord::from_parts(record, star.class, composition, velocity_km_s);
    let tail = match outcome {
        DisruptionOutcome::Disrupted => Some(DisruptionTail::new(
            disruption.time(),
            black_hole_mass,
            star_mass,
            star_radius,
        )),
        DisruptionOutcome::SwallowedWhole => None,
    };
    TidalDisruptionVictim {
        disruption,
        index,
        member,
        star_mass,
        star_radius,
        star_phase: star.state.phase(),
        black_hole_mass,
        outcome,
        at_pericentre,
        tail,
    }
}

/// A parabola about `mu` at its pericentre `pericentre` from the black hole, oriented by three
/// uniforms (module documentation): the pericentre's direction, isotropic, from `cos θ = 2u₀ − 1`
/// and the azimuth `2π u₁`, and the velocity, perpendicular to it at the parabola's speed
/// `√(2μ ÷ q)`, at the angle `2π u₂` about it from the meridian's tangent.
///
/// # Panics
///
/// Never: the state is finite, off the black hole and not radial.
#[must_use]
fn parabola_at_pericentre(
    mu: GravitationalParameter,
    pericentre: Metres,
    u: [f64; 3],
) -> KeplerOrbit {
    let cos_theta = 2.0 * u[0] - 1.0;
    let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
    let (sin_phi, cos_phi) = math::sin_cos(core::f64::consts::TAU * u[1]);
    let radial = [sin_theta * cos_phi, sin_theta * sin_phi, cos_theta];
    // The meridian's and the parallel's unit tangents at that direction: with the radial one, an
    // orthonormal basis at every direction, the poles included (where the azimuth names one).
    let north = [cos_theta * cos_phi, cos_theta * sin_phi, -sin_theta];
    let east = [-sin_phi, cos_phi, 0.0];
    let (sin_angle, cos_angle) = math::sin_cos(core::f64::consts::TAU * u[2]);
    let q = pericentre.value();
    let speed = (2.0 * mu.value() / q).sqrt();
    let velocity = [0, 1, 2].map(|i| speed * (cos_angle * north[i] + sin_angle * east[i]));
    let r_ly = q / METRES_PER_LIGHT_YEAR;
    let position = PointLy::new(r_ly * radial[0], r_ly * radial[1], r_ly * radial[2]);
    KeplerOrbit::from_state(position, GalacticVelocity::new(velocity), mu)
        .expect("a parabola at a positive pericentre is finite, off the black hole and not radial")
}

/// A victim's star, from the attempts on `stream` after its orbit's words (module documentation):
/// its class picked by `weights` among `classes`, its age at the disruption and its initial mass.
fn draw_star(
    imf: &dyn MassFunction,
    stream: &mut Stream,
    classes: &[CentreClass],
    weights: &[f64],
    composition: &Composition,
) -> VictimStar {
    let total = weights.iter().fold(0.0, |sum, &w| sum + w);
    let components = age_components();
    let mut last = None;
    for attempt in 0..STAR_ATTEMPTS {
        stream.seek(ORBIT_WORDS + STAR_ATTEMPT_WORDS * u64::from(attempt));
        let mark: Mark = stream.mark();
        let class = classes[mark
            .pick_weighted(weights, total)
            .expect("the weights summed in order are their own bound, so a class is picked")];
        let component = components[usize::from(class.component)];
        let youngest = component.youngest.value().max(MIN_STAR_AGE.value());
        let age = Years::new(youngest + stream.uniform() * (component.oldest.value() - youngest));
        let u = stream.uniform();
        let (lo, hi) = (
            class.initial_mass_range.0.value(),
            class
                .initial_mass_range
                .1
                .value()
                .min(turn_off_mass(age, composition).value()),
        );
        if hi > lo {
            let initial_mass = SolarMasses::new(imf.quantile_in(lo, hi, u));
            return star(class, attempt, initial_mass, age, composition);
        }
        last = Some((age, u));
    }
    debug_assert!(
        false,
        "a victim's star exhausted its {STAR_ATTEMPTS} attempts"
    );
    // Never reached in practice (`STAR_ATTEMPTS`): the first class, the oldest of band A, is
    // alive at every age, so the victim takes its star at the last attempt's age and quantile.
    let (age, u) = last.expect("an attempt was made");
    let class = classes[0];
    let (lo, hi) = (
        class.initial_mass_range.0.value(),
        class.initial_mass_range.1.value(),
    );
    star(
        class,
        STAR_ATTEMPTS - 1,
        SolarMasses::new(imf.quantile_in(lo, hi, u)),
        age,
        composition,
    )
}

/// A victim's star of `class` at `attempt`, of `initial_mass` at `age`: plan 06's state there, with
/// the median draws (module documentation).
#[must_use]
fn star(
    class: CentreClass,
    attempt: u32,
    initial_mass: SolarMasses,
    age: Years,
    composition: &Composition,
) -> VictimStar {
    let draws = StarDraws::median();
    // The main sequence's closed forms where they apply, the track's state bit for bit (P06.T38.b).
    let state = main_sequence_state(initial_mass, composition, &draws, age)
        .unwrap_or_else(|| evolve(initial_mass, composition, &draws, age));
    VictimStar {
        class,
        attempt,
        initial_mass,
        age,
        state,
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;
    use hyperion_testkit::order::assert_order_independent;

    use super::*;
    use crate::GENERATOR_VERSION;
    use crate::galaxy::features::centre::members::{
        CentrePlacement, resolve_centre_member, resolve_centre_member_with,
    };
    use crate::galaxy::features::centre::testing::{milky_way_centre, milky_way_galaxy};
    use crate::galaxy::imf::MassBand;
    use crate::galaxy::placement::{ResolveSystemError, resolve};
    use crate::units::AstronomicalUnits;

    /// The Milky Way fixture's victims.
    fn fixture() -> CentreVictims {
        CentreVictims::new(milky_way_galaxy(), milky_way_centre()).unwrap()
    }

    /// The victims of a black hole of `mass` M☉ about the fixture's centre, for each of `n` seeds of
    /// family `0x0930_b000_0000_0000`.
    fn sweep(mass: f64, n: u64) -> Vec<TidalDisruptionVictim> {
        let galaxy = milky_way_galaxy();
        (0..n)
            .flat_map(|i| {
                CentreVictims::from_parts(
                    Seed::new(0x0930_b000_0000_0000 | i),
                    SolarMasses::new(mass),
                    milky_way_centre(),
                    galaxy.mass_function(),
                )
                .unwrap()
                .victims
            })
            .collect()
    }

    fn length(v: [f64; 3]) -> f64 {
        (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
    }

    fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
        a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
    }

    /// The Hills mass of a 1 M☉, 1 R☉ star is the plan's 1.14 × 10⁸ M☉ (1.1433 × 10⁸ with the IAU
    /// 2015 nominal values), where its tidal radius is the Schwarzschild radius; the outcome turns
    /// there.
    #[test]
    fn a_sun_like_star_s_hills_mass_is_1_14e8_solar_masses() {
        let (m, r) = (SolarMasses::new(1.0), SolarRadii::new(1.0));
        let m_h = hills_mass(m, r);
        eprintln!(
            "Hills mass of a 1 M_sun, 1 R_sun star: {:.6e} M_sun",
            m_h.value()
        );
        assert!((m_h.value() / 1.1433e8 - 1.0).abs() < 1e-4, "{m_h:?}");
        let ratio = tidal_radius(m_h, m, r).value() / schwarzschild_radius(m_h).value();
        assert!((ratio - 1.0).abs() < 1e-12, "{ratio}");
        let below = SolarMasses::new(m_h.value() * (1.0 - 1e-9));
        assert_eq!(outcome_of(below, m, r), DisruptionOutcome::Disrupted);
        assert_eq!(outcome_of(m_h, m, r), DisruptionOutcome::SwallowedWhole);
        // The Sun about the Milky Way's black hole: 0.756 au.
        let r_t = AstronomicalUnits::from(tidal_radius(SolarMasses::new(4.3e6), m, r)).value();
        assert!((r_t - 0.7561).abs() < 1e-3, "{r_t} au");
    }

    /// The record's epoch state is on `v`'s parabola about `mu`: the same angular momentum, and no
    /// orbital energy against the kinetic energy at pericentre.
    fn assert_on_its_parabola(v: &TidalDisruptionVictim, mu: GravitationalParameter) {
        let orbit = v.orbit_at_disruption();
        let epoch = KeplerOrbit::from_state(
            PointLy::from(v.record().epoch_position()),
            v.member().velocity(),
            mu,
        )
        .unwrap();
        let (h0, h1) = (
            orbit.angular_momentum_m2_per_s(),
            epoch.angular_momentum_m2_per_s(),
        );
        let dh = length([h1[0] - h0[0], h1[1] - h0[1], h1[2] - h0[2]]);
        // Tens of light-years out for the horizon's oldest, where a propagation's error grows
        // with (r ÷ q)² (the motion module's "Units and precision"): 2 × 10⁻⁹ at worst.
        assert!(dh < 1e-6 * length(h0), "{:?}: {dh}", v.id());
        // The energy's rounding is set at pericentre, where v² is 2μ ÷ q.
        let v2 = dot(
            orbit.velocity().metres_per_second(),
            orbit.velocity().metres_per_second(),
        );
        assert!(
            epoch.specific_energy_j_per_kg().abs() < 1e-12 * v2,
            "{:?}",
            v.id()
        );
    }

    /// P09.T30.b: every disruption in the source horizon has a victim, in its order; each victim
    /// resolves from its ID through plan 03's `resolve`, is at pericentre at its disruption (its
    /// radial velocity zero there) and no system after it, and its record's epoch state lies on
    /// its parabola.
    #[test]
    fn a_victim_resolves_from_its_id_and_is_at_pericentre_at_its_disruption() {
        let galaxy = milky_way_galaxy();
        let victims = fixture();
        let mut disruptions = Vec::new();
        tidal_disruptions(victims.model(), horizon(), &mut disruptions);
        let listed: Vec<TidalDisruption> = victims
            .victims()
            .iter()
            .map(TidalDisruptionVictim::disruption)
            .collect();
        assert_eq!(listed, disruptions);
        eprintln!(
            "the fixture's horizon holds {} victims (27.31 expected)",
            victims.count()
        );
        assert!((10..50).contains(&victims.count()), "{}", victims.count());
        let mu = GravitationalParameter::from_solar_masses(galaxy.params().black_hole().mass());
        for (i, v) in (1_u16..).zip(victims.victims()) {
            assert_eq!(v.index(), i);
            assert_eq!(victims.victim(i), Some(v));
            assert_eq!(
                CentreVictims::one(galaxy, milky_way_centre(), i)
                    .unwrap()
                    .as_ref(),
                Some(v)
            );
            assert_eq!(victims.of_disruption(v.disruption().id()), Some(v));
            let id = v.id();
            assert_eq!(SystemId::from(id), v.record().id());
            assert_eq!(resolve(galaxy, SystemId::from(id)), Ok(*v.record()));
            assert_eq!(resolve_centre_member(galaxy, id), Ok(*v.member()));
            // At pericentre at the disruption.
            let StateAt::Exists(at) = v.state_at(v.time()) else {
                panic!("{id:?} exists at its disruption");
            };
            // The orbit's own state is the pericentre's, and its galactic position is it to the
            // frame's resolution near the origin, a few metres.
            let orbit = v.orbit_at_disruption();
            assert_eq!(orbit.distance(), v.pericentre());
            let r = at.position().distance_to(&GalacticPosition::ORIGIN).value();
            assert!(
                (r - v.pericentre().value()).abs() < 1e-6 * r,
                "{id:?}: {r} m"
            );
            let (p, u) = (
                orbit.displacement().metres(),
                orbit.velocity().metres_per_second(),
            );
            assert!(dot(p, u).abs() < 1e-12 * length(p) * length(u), "{id:?}");
            assert!((orbit.eccentricity() - 1.0).abs() < 1e-12, "{id:?}");
            // No system after it; on its way in before it.
            let later = v
                .time()
                .checked_add(crate::time::Span::new(0, 1).unwrap())
                .unwrap();
            assert_eq!(v.state_at(later), StateAt::NoSuchSystem);
            let year_before = v
                .time()
                .checked_sub(crate::time::Span::from_julian_years(1).unwrap())
                .unwrap();
            let StateAt::Exists(before) = v.state_at(year_before) else {
                panic!("{id:?} exists a year before its disruption");
            };
            let r_before = before
                .position()
                .distance_to(&GalacticPosition::ORIGIN)
                .value();
            assert!(r_before > 100.0 * v.pericentre().value(), "{id:?}");
            assert_on_its_parabola(v, mu);
            assert_eq!(
                v.record().layer(),
                crate::galaxy::placement::layer_for_initial_mass(v.record().primary_initial_mass())
                    .unwrap()
            );
        }
        assert_eq!(victims.victim(0), None);
        let past = victims.count() + 1;
        assert_eq!(victims.victim(past), None);
        for index in [0, past] {
            assert_eq!(
                CentreVictims::one(galaxy, milky_way_centre(), index),
                Ok(None)
            );
        }
        let id = CentreMemberId::new(MemberSlot::FeatureLevel { index: past }).unwrap();
        assert_eq!(
            resolve(galaxy, SystemId::from(id)),
            Err(ResolveSystemError::NoSuchSystem)
        );
    }

    /// A victim's draws are its disruption's event stream from word 0: its pericentre's direction
    /// is words 0–1's and its first attempt's class word 3's mark, and no disruption's ID or time
    /// moves (they are T30.a's listing, above).
    #[test]
    fn a_victim_draws_from_word_0_of_its_disruption_s_stream() {
        let victims = fixture();
        let (classes, weights) = living_classes(milky_way_centre());
        let total = weights.iter().fold(0.0, |s, &w| s + w);
        for v in victims.victims() {
            let word = v.disruption().id().word();
            let mut stream = victims
                .model()
                .series()
                .key()
                .event_stream(word.bin(), word.number());
            let (u0, u1) = (stream.uniform(), stream.uniform());
            let p = v.orbit_at_disruption().displacement().metres();
            let along = p.map(|c| c / length(p));
            assert!((along[2] - (2.0 * u0 - 1.0)).abs() < 1e-12, "{:?}", v.id());
            let azimuth = math::atan2(along[1], along[0]).rem_euclid(core::f64::consts::TAU);
            assert!(
                (azimuth - core::f64::consts::TAU * u1).abs() < 1e-9,
                "{:?}: {azimuth} against {}",
                v.id(),
                core::f64::consts::TAU * u1
            );
            if let SystemOrigin::CentreMember { attempt: 0 } = v.record().origin() {
                stream.seek(ORBIT_WORDS);
                let picked = stream.mark().pick_weighted(&weights, total).unwrap();
                assert_eq!(&classes[picked], v.class(), "{:?}", v.id());
            }
        }
    }

    /// P09.T30.b: at Milky Way values every main-sequence victim is disrupted. Every victim of a
    /// living class is on the main sequence, before it in the youngest components, or below
    /// 0.1 M☉ on plan 06's cooling fits; over 200 seeds about the fixture's centre all are
    /// disrupted, and so is a main-sequence star of any mass from 0.1 to 150 M☉ from its arrival
    /// to its turn-off.
    #[test]
    fn at_milky_way_values_every_main_sequence_victim_is_disrupted() {
        let mass = milky_way_galaxy().params().black_hole().mass();
        let all = sweep(mass.value(), 200);
        let (mut main, mut pre, mut cooling, mut least) = (0, 0, 0, f64::INFINITY);
        for v in &all {
            match v.star_phase() {
                Phase::MainSequence => main += 1,
                Phase::PreMainSequence => pre += 1,
                Phase::Substellar if v.record().primary_initial_mass().value() < 0.1 => {
                    cooling += 1;
                }
                phase => panic!("{:?}: a victim of a living class in {phase:?}", v.id()),
            }
            assert_eq!(v.outcome(), DisruptionOutcome::Disrupted, "{:?}", v.id());
            assert!(v.tail().is_some());
            least = least.min(v.hills_mass().value() / mass.value());
        }
        eprintln!(
            "{} victims over 200 seeds: {main} on the main sequence, {pre} before it, {cooling} \
             below 0.1 M_sun on the cooling fits; the least Hills mass is {least:.3} times the \
             black hole's",
            all.len()
        );
        assert!(all.len() > 4_000, "{}", all.len());
        let composition = centre_composition();
        let draws = StarDraws::median();
        for k in 0..=64 {
            let m0 = SolarMasses::new(0.08 * math::powf(150.0 / 0.08, f64::from(k) / 64.0));
            if m0.value() < 0.1 {
                continue;
            }
            let lifetime = crate::stellar::lifetime(m0, &composition, &draws).value();
            for share in [0.01, 0.1, 0.5, 0.9, 0.99] {
                let age = Years::new(share * lifetime);
                if let Some(state) = main_sequence_state(m0, &composition, &draws, age) {
                    assert_eq!(
                        outcome_of(mass, state.mass(), state.radius()),
                        DisruptionOutcome::Disrupted,
                        "{m0:?} at {age:?}"
                    );
                }
            }
        }
    }

    /// Above a star's Hills mass it is swallowed whole: at the builder's heaviest black hole, 1.36 ×
    /// 10⁸ M☉, most victims are, each at the Schwarzschild radius and with no tail, and the
    /// heaviest are still disrupted, at their tidal radius and with a tail.
    #[test]
    fn above_its_hills_mass_a_victim_is_swallowed_whole() {
        let mass = SolarMasses::new(1.36e8);
        let all = sweep(mass.value(), 40);
        let swallowed = all
            .iter()
            .filter(|v| v.outcome() == DisruptionOutcome::SwallowedWhole)
            .count();
        eprintln!(
            "{swallowed} of {} victims swallowed whole at 1.36e8 M_sun",
            all.len()
        );
        assert!(swallowed > all.len() / 2, "{swallowed} of {}", all.len());
        assert!(swallowed < all.len(), "every victim swallowed");
        for v in &all {
            assert_eq!(
                v.outcome(),
                outcome_of(mass, v.star_mass(), v.star_radius())
            );
            let (q, want) = match v.outcome() {
                DisruptionOutcome::Disrupted => {
                    assert!(v.tail().is_some());
                    (v.pericentre(), v.tidal_radius())
                }
                DisruptionOutcome::SwallowedWhole => {
                    assert!(v.tail().is_none());
                    (v.pericentre(), schwarzschild_radius(mass))
                }
            };
            assert!(
                (q.value() / want.value() - 1.0).abs() < 1e-12,
                "{:?}",
                v.id()
            );
            assert!(
                v.orbit_at_disruption().velocity().speed().value()
                    <= SPEED_OF_LIGHT * (1.0 + 1e-12)
            );
        }
    }

    /// P09.T30.b: the Kepler members' loss cone stays empty. No member of the cells about the black
    /// hole, in any band, is on an orbit through the loss cone (P09.T25's thinning), the victims
    /// sit on no cell, and resolving a victim's ID gives the victim, never a cell member.
    #[test]
    fn the_kepler_members_loss_cone_stays_empty() {
        let galaxy = milky_way_galaxy();
        let model = milky_way_centre();
        let placement = CentrePlacement::new(model);
        let cone = model.loss_cone();
        let mut members = 0;
        // The eight cells at the black hole, and on each level the owned cells beside it along the
        // axes, just outside the inner half the level below owns.
        let mut cells: Vec<(u8, [u8; 3])> = Vec::new();
        for x in 15..17_u8 {
            for y in 15..17_u8 {
                for z in 15..17_u8 {
                    cells.push((0, [x, y, z]));
                }
            }
        }
        for level in 1..6 {
            for along in [7, 24] {
                for (a, b) in [(15, 15), (15, 16), (16, 15), (16, 16)] {
                    cells.extend([
                        (level, [along, a, b]),
                        (level, [a, along, b]),
                        (level, [a, b, along]),
                    ]);
                }
            }
        }
        for (level, at) in cells {
            let cell = placement.grid().cell(level, at).unwrap();
            for band in MassBand::ALL {
                for m in placement.members_in_cell(galaxy, band, cell) {
                    let p = m.local_position();
                    assert!(
                        !cone.contains([p.x, p.y, p.z], m.velocity_km_s()),
                        "{:?} is on an orbit through the loss cone",
                        m.record().id()
                    );
                    assert!(matches!(
                        m.record().id().kind(),
                        crate::id::SystemIdKind::Centre(c)
                            if matches!(c.slot(), MemberSlot::InCell { .. })
                    ));
                    members += 1;
                }
            }
        }
        eprintln!("{members} members of the cells about the black hole, none in its loss cone");
        assert!(members > 2_000, "{members}");
        for v in fixture().victims() {
            assert!(matches!(
                v.id().slot(),
                MemberSlot::FeatureLevel { index } if index >= 1
            ));
            assert_eq!(
                resolve_centre_member_with(galaxy, model, v.id()),
                Ok(*v.member())
            );
        }
    }

    /// The victims are the same in any order of asking, on a shared centre or a fresh one, and two
    /// listings of one galaxy's are equal.
    #[test]
    fn the_victims_are_order_independent() {
        let galaxy = milky_way_galaxy();
        let model = milky_way_centre();
        let victims = fixture();
        let mut ids: Vec<CentreMemberId> = victims
            .victims()
            .iter()
            .map(TidalDisruptionVictim::id)
            .collect();
        ids.push(CentreMemberId::CENTRAL_BLACK_HOLE);
        ids.push(
            CentreMemberId::new(MemberSlot::FeatureLevel {
                index: victims.count() + 1,
            })
            .unwrap(),
        );
        assert_order_independent(&ids, |&id| resolve_centre_member_with(galaxy, model, id));
        assert_eq!(fixture(), victims);
        assert_eq!(CentreVictims::of(galaxy).unwrap(), victims);
        let fresh = CentreModel::new(galaxy).unwrap();
        assert_eq!(CentreVictims::new(galaxy, &fresh).unwrap(), victims);
    }

    /// Writes `v`'s lines: its disruption, class, star, outcome, pericentre and its record's epoch
    /// state.
    fn pin_victim(w: &mut GoldenWriter, v: &TidalDisruptionVictim) {
        let label = format!("{:016x}", SystemId::from(v.id()).raw());
        let class = v.class();
        w.line("");
        w.line(&format!(
            "{label} {} at {}: {:?} {:?} component {}, attempt {:?}, {:?}, {:?}",
            v.disruption().id(),
            v.time(),
            class.band,
            class.kind,
            class.component,
            v.record().origin(),
            v.star_phase(),
            v.outcome()
        ));
        w.f64(
            &format!("{label}.initial_mass"),
            v.record().primary_initial_mass().value(),
        );
        w.f64(
            &format!("{label}.age_at_epoch"),
            v.record().age_at_epoch().value(),
        );
        w.f64(&format!("{label}.star_mass"), v.star_mass().value());
        w.f64(&format!("{label}.star_radius"), v.star_radius().value());
        w.f64(&format!("{label}.pericentre_m"), v.pericentre().value());
        let p = PointLy::from(v.record().epoch_position());
        for (axis, c) in ["x", "y", "z"].iter().zip([p.x, p.y, p.z]) {
            w.f64(&format!("{label}.epoch_{axis}"), c);
        }
        for (axis, c) in ["vx", "vy", "vz"].iter().zip(v.member().velocity_km_s()) {
            w.f64(&format!("{label}.epoch_{axis}"), c);
        }
    }

    /// Pinned: the fixture's victims, each with its disruption, class, star, outcome, pericentre
    /// and the record's epoch state, as float bits; the last one's state a year and a thousand
    /// years before its disruption; the victims of the first seed of family
    /// `0x0930_b000_0000_0000` with both outcomes at 1.36 × 10⁸ M☉; and the first victim inbound
    /// at the epoch of family `0x0930_c000_0000_0000`, about the fixture's centre.
    #[test]
    fn centre_victims_are_pinned() {
        let galaxy = milky_way_galaxy();
        let victims = fixture();
        let mut w = GoldenWriter::new();
        w.header(GENERATOR_VERSION.get());
        w.f64(
            "Hills mass of a 1 M_sun, 1 R_sun star (M_sun)",
            hills_mass(SolarMasses::new(1.0), SolarRadii::new(1.0)).value(),
        );
        w.line(&format!(
            "{} victims in the source horizon",
            victims.count()
        ));
        for v in victims.victims() {
            pin_victim(&mut w, v);
        }
        let last = victims.victims().last().unwrap();
        for years in [1, 1_000] {
            let t = last
                .time()
                .checked_sub(crate::time::Span::from_julian_years(years).unwrap())
                .unwrap();
            let StateAt::Exists(state) = last.state_at(t) else {
                panic!("the last victim exists before its disruption");
            };
            w.line("");
            w.line(&format!("the last victim {years} yr before its disruption"));
            let p = PointLy::from(state.position());
            for (axis, c) in ["x", "y", "z"].iter().zip([p.x, p.y, p.z]) {
                w.f64(&format!("  {axis} (ly)"), c);
            }
            let velocity = state.velocity().metres_per_second();
            for (axis, c) in ["vx", "vy", "vz"].iter().zip(velocity) {
                w.f64(&format!("  {axis} (m/s)"), c);
            }
        }
        let of = |family: u64, mass: f64, i: u64| {
            CentreVictims::from_parts(
                Seed::new(family | i),
                SolarMasses::new(mass),
                milky_way_centre(),
                galaxy.mass_function(),
            )
            .unwrap()
        };
        let has = |v: &CentreVictims, outcome| v.victims().iter().any(|v| v.outcome() == outcome);
        let (i, both) = (0..100)
            .map(|i| (i, of(0x0930_b000_0000_0000, 1.36e8, i)))
            .find(|(_, v)| {
                has(v, DisruptionOutcome::Disrupted) && has(v, DisruptionOutcome::SwallowedWhole)
            })
            .expect("a seed in a hundred has both outcomes at 1.36e8 M_sun");
        w.line("");
        w.line(&format!(
            "seed {:#018x} at 1.36e8 M_sun: {} victims",
            0x0930_b000_0000_0000_u64 | i,
            both.count()
        ));
        for v in both.victims() {
            pin_victim(&mut w, v);
        }
        let mass = galaxy.params().black_hole().mass().value();
        let (i, inbound) = (0..1_000)
            .map(|i| (i, of(0x0930_c000_0000_0000, mass, i)))
            .find(|(_, v)| v.at(UniverseTime::EPOCH).count() > 0)
            .expect("a seed in a thousand has a victim inbound at the epoch");
        w.line("");
        w.line(&format!(
            "seed {:#018x}: the first victim inbound at the epoch",
            0x0930_c000_0000_0000_u64 | i
        ));
        pin_victim(&mut w, inbound.at(UniverseTime::EPOCH).next().unwrap());
        golden!("galaxy/features/centre_victims", w.as_str());
    }
}
