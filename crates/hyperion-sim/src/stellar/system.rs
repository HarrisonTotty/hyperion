//! System assembly: a system's metallicity draw, its stars' models, and the summaries and briefs
//! the server sends for a system and for each row of a range query (plan 06, phase G).
//!
//! This is the only part of the stellar stage that reads plan 03's placement: everything else is a
//! pure function of mass, composition, the star's draws and age.
//!
//! [`draw_metallicity`] (P06.T3) gives every star of a grid system one [`Composition`]: its
//! \[Fe/H\] is drawn once per system from the distribution plan 02's fields give the system's own
//! density component at its place and age (brainstorm, "Fields": metallicity "falls with galactic
//! radius … and, beyond about 8 Gyr, with age, with scatter").
//!
//! [`StarModel`] (P06.T29.a) is one star at any clock time: its track, or below 0.1 M☉ its cooling
//! fits, and its remnant. It needs no record, so plan 14 builds its synthetic hosts with it, and by
//! ruling 34 of 2026-09-22 it is how plans 11 and 14 read every star, never a
//! [`Track`] directly.
//!
//! [`SystemStars`] (P06.T29.b, with plan 11's P11.T2.c) is a grid system's stars from its record:
//! its primary, body 0 of the system, and the companions of its [`SystemHierarchy`], each a
//! [`StarModel`] of the system's composition and age. Its [`SystemSummary`] is what the server's
//! `system_summary` answers from, and its [`StellarBrief`] what a range query's row carries.

use std::error::Error;
use std::fmt;

use crate::galaxy::fields::Component;
use crate::galaxy::placement::{Existence, SystemRecord};
use crate::galaxy::{Galaxy, PointLy};
use crate::id::BodyId;
use crate::math;
use crate::rng::{ObjectKey, Stream, tags};
use crate::stellar::classify::{ClassExtras, Classification, LuminosityClass, classify};
use crate::stellar::draws::{StandardNormal, StarDraws};
use crate::stellar::multiplicity::{
    MultiplicityContext, RedrawAttempt, SystemHierarchy, draw_hierarchy,
};
use crate::stellar::photometry::{absolute_magnitude_v, colour_b_v};
use crate::stellar::remnant::collapse::RemnantDraws;
use crate::stellar::remnant::{CompactRemnant, Death, DeathKind, NatalKick, StandardKickLaw};
use crate::stellar::sse::{self, Track, TrackOptions};
use crate::stellar::{Composition, ObjectKind, Phase, StarState, substellar};
use crate::time::{ClockWindow, Span, UniverseTime};
use crate::units::consts::SECONDS_PER_JULIAN_YEAR;
use crate::units::{
    Dex, HeliumExcess, Kelvin, Magnitudes, SolarLuminosities, SolarMasses, SolarRadii, Years,
};

/// The composition every star of the grid system `record` shares: \[Fe/H\] drawn from its density
/// component's distribution at its epoch position and age, and no helium excess.
///
/// The component is `record`'s own ([`SystemRecord::component`]), and its
/// [`metallicity`](Component::metallicity) at the epoch position, read as a
/// [`PointLy`], and at the age at the epoch returns the normal distribution of \[Fe/H\] for that
/// population or halo component, place and age (plan 02, P02.T7.e, with rulings 7 and 21 of
/// 2026-09-22). The system's \[Fe/H\] is its mean plus its standard deviation times one standard
/// normal: two words of the stream `system.metallicity`, keyed by the system's ID
/// ([`tags::SYSTEM_METALLICITY`]). So it depends on the seed, the ID and the fields alone, and
/// neither on what else was generated nor on the time asked about: a system's metallicity is fixed
/// at its birth.
///
/// - A system not yet born at the epoch (a negative age, down to −H in the populations that still
///   form stars) reads its distribution at age zero.
/// - The helium excess is zero for every grid system (plan 06, design note 5); only plan 09's
///   cluster members carry one, and they bring their own [`Composition`].
/// - The function is for grid records, whose origin is
///   [`SystemOrigin::Grid`](crate::galaxy::placement::SystemOrigin::Grid) and whose
///   [`component`](SystemRecord::component) is therefore `Some`. A record without a component,
///   which no origin of this generator version builds, fails a debug assertion and in release
///   reads the first component of its population.
///
/// # Panics
///
/// If the record's component does not index `galaxy`'s fields, as a record placed in a galaxy
/// with more components would not ([`Fields::component`](crate::galaxy::fields::Fields::component)).
/// In debug builds also if the record has no component.
///
/// # Examples
///
/// Two systems of one cell at the solar circle draw their abundances independently from the
/// same thin-disc distribution, which is solar at the Sun's radius with a scatter of 0.2 dex:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::placement::{CellKey, generate_cell};
/// use hyperion_sim::id::Layer;
/// use hyperion_sim::stellar::system::draw_metallicity;
///
/// let galaxy = Galaxy::new(Seed::new(11));
/// let key = CellKey::new(Layer::C, [0, 812, 0])?;
/// let mut cell = Vec::new();
/// generate_cell(&galaxy, key, &mut cell);
/// for record in cell.iter().take(2) {
///     let composition = draw_metallicity(&galaxy, record);
///     // Within five standard deviations of a mean that lies within a few tenths of solar.
///     assert!(composition.fe_h().value().abs() < 1.5);
///     // The same system always draws the same abundance.
///     assert_eq!(composition, draw_metallicity(&galaxy, record));
/// }
/// # Ok::<(), hyperion_sim::galaxy::placement::BuildCellKeyError>(())
/// ```
#[must_use]
pub fn draw_metallicity(galaxy: &Galaxy, record: &SystemRecord) -> Composition {
    let distribution = component_of(galaxy, record)
        .metallicity(&PointLy::from(record.epoch_position()), age_read(record));
    let mut stream = Stream::open(
        galaxy.seed(),
        tags::SYSTEM_METALLICITY,
        ObjectKey::from(record.id()),
    );
    let fe_h = stream.normal(distribution.mean().value(), distribution.sigma().value());
    Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO)
}

/// The age at which a system reads its component's metallicity: its age at the epoch, or zero for
/// a system not yet born then.
#[must_use]
fn age_read(record: &SystemRecord) -> Years {
    let age = record.age_at_epoch();
    if age.value() < 0.0 { Years::ZERO } else { age }
}

/// The density component whose laws a grid record follows: its own, or for a record without one,
/// which no origin of this generator version builds, the first component of its population.
#[must_use]
fn component_of<'g>(galaxy: &'g Galaxy, record: &SystemRecord) -> &'g Component {
    let fields = galaxy.fields();
    debug_assert!(
        record.component().is_some(),
        "the metallicity draw is for grid records, which carry a component: {:?}",
        record.id()
    );
    match record.component() {
        Some(id) => fields.component(id),
        None => fields
            .components()
            .iter()
            .find(|c| c.population() == record.population())
            .expect("every population has at least one density component"),
    }
}

/// The heaviest star a [`StarModel`] takes, 150 M☉: the upper end of plan 02's initial mass
/// function ([`MASS_BAND_EDGES`](crate::galaxy::imf::MASS_BAND_EDGES)).
pub const MAX_STAR_MASS: SolarMasses = SolarMasses::new(150.0);

/// A [`StarModel`] could not be built from its parts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildStarModelError {
    /// The initial mass is outside [`substellar::MIN_MASS`]–[`MAX_STAR_MASS`], 0.01–150 M☉, or not
    /// a number.
    MassOutsideRange(SolarMasses),
    /// The age at the epoch is not finite.
    AgeNotFinite(Years),
}

impl fmt::Display for BuildStarModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MassOutsideRange(m) => {
                write!(f, "initial mass {} M_sun is outside 0.01-150", m.value())
            }
            Self::AgeNotFinite(age) => {
                write!(f, "age at the epoch {} yr is not finite", age.value())
            }
        }
    }
}

impl Error for BuildStarModelError {}

/// One star at any clock time: its evolution from its initial mass, composition and draws, its
/// age at the epoch, and its remnant (plan 06, P06.T29.a).
///
/// A star of 0.1 M☉ or more follows its [`Track`], built to its age at the end of the clock window
/// (the epoch + H, [`ClockWindow::END`]) and so completed to its death if it is dead by then
/// (design note 19). A lighter object follows P06.T13's cooling fits
/// ([`substellar::cooling`], ruling 33), whose luminosity and radius fall with age, and never
/// dies. Every question takes a [`UniverseTime`] and reads the star at its age then, the age at
/// the epoch plus the time since it, as [`SystemRecord::age_at`] does (design note 23).
///
/// The remnant stage reads only the star's remnant draws (`star.remnant.*`), its provisional
/// companion-stripped mark (`star.stripped`) and its kick draws (`star.kick.*`): the remnant of an
/// iron core's collapse is redrawn from them on the built track (`Track::fate_with`), and the
/// natal kick drawn by the generator's law
/// ([`StandardKickLaw::natal_kick`](crate::stellar::remnant::StandardKickLaw::natal_kick),
/// P06.T19), which is the step plan 08's kick loop repeats with later attempts' draws, costing a
/// remnant and a kick and never a track. The death it reports carries the stripped mark
/// ([`Stripping::Companion`](crate::stellar::remnant::Stripping::Companion)) where it is set; the
/// track itself is the single star's (plan 06, design note 11).
///
/// Until P06.T14 the formulae stop at 100 M☉ (`sse::MAX_INITIAL_MASS`), so a star of 100–150 M☉
/// is evolved as one of 100 M☉ and keeps its own initial mass.
///
/// # Examples
///
/// A host star for plan 14: the Sun at 4.57 Gyr, and whether it can have swallowed a planet at
/// 0.1 AU yet.
///
/// ```
/// use hyperion_sim::stellar::draws::StarDraws;
/// use hyperion_sim::stellar::system::StarModel;
/// use hyperion_sim::stellar::{Composition, Phase};
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::{SolarMasses, Years};
///
/// let sun = StarModel::new(
///     SolarMasses::new(1.0),
///     Composition::SOLAR,
///     StarDraws::median(),
///     Years::new(4.57e9),
/// )?;
/// let today = sun.state_at(UniverseTime::EPOCH).ok_or("the Sun has formed")?;
/// assert_eq!(today.phase(), Phase::MainSequence);
/// // 0.1 AU is 21.5 R☉, which the Sun has never reached.
/// assert!(sun.max_radius_until(UniverseTime::EPOCH).value() < 21.5);
/// // The Sun outlives the clock window, so its death is found by building the rest of its life.
/// assert!(sun.lifetime().ok_or("a star dies")?.value() > 1.1e10);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct StarModel {
    initial_mass: SolarMasses,
    composition: Composition,
    draws: StarDraws,
    age_at_epoch: Years,
    evolution: Evolution,
    remnant: Option<RemnantStage>,
}

/// How a [`StarModel`] evolves.
#[derive(Debug, Clone, PartialEq)]
enum Evolution {
    /// From 0.1 M☉: the track, to the end of the clock window or the star's death.
    Track(Box<Track>),
    /// Below 0.1 M☉: P06.T13's cooling fits.
    Cooling,
}

/// What the remnant stage decides from a built track and the star's remnant, stripped and kick
/// draws: the death and the remnant, and the natal kick.
#[derive(Debug, Clone, Copy, PartialEq)]
struct RemnantStage {
    death: Death,
    remnant: CompactRemnant,
    natal_kick: Option<NatalKick>,
}

impl RemnantStage {
    /// The stage of a star that died `death` leaving `remnant`, with its `draws`: the death with
    /// the provisional companion-stripped mark applied, and the kick of the generator's law
    /// (P06.T19), which read `star.stripped` and `star.kick.*`.
    #[must_use]
    fn new(death: Death, remnant: CompactRemnant, draws: &StarDraws) -> Self {
        let law = StandardKickLaw::default();
        let death = law.with_stripped_mark(death, draws);
        Self {
            death,
            remnant,
            natal_kick: law.natal_kick(&death, &remnant, draws),
        }
    }
}

/// The remnant stage of a star whose `track` has reached its death, from its `draws`, or `None`
/// if the track has not: the death and remnant that `track` gives the remnant draws
/// (`star.remnant.*`), and the natal kick from `star.stripped` and `star.kick.*`.
#[must_use]
fn remnant_stage(track: &Track, draws: &StarDraws) -> Option<RemnantStage> {
    let fate = track.fate_with(RemnantDraws::of(draws))?;
    Some(RemnantStage::new(fate.death, fate.remnant, draws))
}

impl StarModel {
    /// The star of initial mass `m0`, `composition` and `draws` whose age at the epoch is
    /// `age_at_epoch` (Julian years since its onset of collapse, negative for a star that forms
    /// after the epoch).
    ///
    /// # Errors
    ///
    /// [`BuildStarModelError::MassOutsideRange`] if `m0` is outside 0.01–150 M☉ or not a number,
    /// and [`BuildStarModelError::AgeNotFinite`] if `age_at_epoch` is not finite.
    pub fn new(
        m0: SolarMasses,
        composition: Composition,
        draws: StarDraws,
        age_at_epoch: Years,
    ) -> Result<Self, BuildStarModelError> {
        if !(substellar::MIN_MASS.value()..=MAX_STAR_MASS.value()).contains(&m0.value()) {
            return Err(BuildStarModelError::MassOutsideRange(m0));
        }
        if !age_at_epoch.value().is_finite() {
            return Err(BuildStarModelError::AgeNotFinite(age_at_epoch));
        }
        let mut model = Self {
            initial_mass: m0,
            composition,
            draws,
            age_at_epoch,
            evolution: Evolution::Cooling,
            remnant: None,
        };
        if m0 >= sse::MIN_INITIAL_MASS {
            let end = model.age_at(ClockWindow::END).value();
            let track = Track::to_age(
                model.track_mass(),
                &model.composition,
                &model.draws,
                Years::new(if end > 0.0 { end } else { 0.0 }),
            );
            model.remnant = remnant_stage(&track, &model.draws);
            model.evolution = Evolution::Track(Box::new(track));
        }
        Ok(model)
    }

    /// The initial mass, M☉, as given.
    #[must_use]
    pub const fn initial_mass(&self) -> SolarMasses {
        self.initial_mass
    }

    /// The composition.
    #[must_use]
    pub const fn composition(&self) -> &Composition {
        &self.composition
    }

    /// The star's fixed draws.
    #[must_use]
    pub const fn draws(&self) -> &StarDraws {
        &self.draws
    }

    /// The age at the epoch, Julian years since the onset of collapse.
    #[must_use]
    pub const fn age_at_epoch(&self) -> Years {
        self.age_at_epoch
    }

    /// The star's age at `t`: its age at the epoch plus the time from the epoch to `t`, in the same
    /// arithmetic as [`SystemRecord::age_at`].
    #[must_use]
    pub fn age_at(&self, t: UniverseTime) -> Years {
        Years::new(self.age_at_epoch.value() + t.since_epoch().as_julian_years_f64())
    }

    /// The star's state at `t`, or `None` if it has not formed by then: a star exists once its age
    /// is positive, as [`SystemRecord::existence_at`] says of a system.
    ///
    /// `t` may lie anywhere before the end of the clock window, +H; after it only for a star that
    /// has died by then, since the track of one still living is built no further.
    ///
    /// # Panics
    ///
    /// In debug builds, if `t` is after the end of the clock window and the star lives past it.
    #[must_use]
    pub fn state_at(&self, t: UniverseTime) -> Option<StarState> {
        let age = self.age_at(t);
        if age.value() <= 0.0 {
            return None;
        }
        Some(match &self.evolution {
            Evolution::Track(track) => track.state_at(age),
            Evolution::Cooling => self.cooling_at(age),
        })
    }

    /// The age at which the star dies, Julian years since its onset of collapse, or `None` for an
    /// object below 0.1 M☉, which never dies.
    ///
    /// For a star that outlives the clock window the model holds no death, and this builds the
    /// rest of its life to find it: the cost of a whole track (a millisecond or two for an evolved
    /// star; plan 06's Risks, T10.c–e). It is [`Track::lifetime`] of the full track, bit for bit.
    #[must_use]
    pub fn lifetime(&self) -> Option<Years> {
        self.death().map(|death| death.age())
    }

    /// How and when the star dies, and what it was at its last living instant, or `None` for an
    /// object below 0.1 M☉, at the cost [`StarModel::lifetime`] states.
    #[must_use]
    pub fn death(&self) -> Option<Death> {
        self.fate().map(|stage| stage.death)
    }

    /// The remnant the star leaves or will leave, or `None` for an object below 0.1 M☉, at the
    /// cost [`StarModel::lifetime`] states.
    #[must_use]
    pub fn remnant(&self) -> Option<CompactRemnant> {
        self.fate().map(|stage| stage.remnant)
    }

    /// The largest radius the star has had up to `t`: non-decreasing in `t`, never below the
    /// radius at `t`, and zero before the star forms (plans 11 and 14).
    ///
    /// Below 0.1 M☉ the radius falls monotonically with age, so the largest is the first the fits
    /// give (their 1 Myr state).
    ///
    /// # Panics
    ///
    /// In debug builds, as [`StarModel::state_at`].
    #[must_use]
    pub fn max_radius_until(&self, t: UniverseTime) -> SolarRadii {
        let age = self.age_at(t);
        if age.value() <= 0.0 {
            return SolarRadii::ZERO;
        }
        match &self.evolution {
            Evolution::Track(track) => track.max_radius_until(age),
            Evolution::Cooling => self.cooling_at(Years::ZERO).radius(),
        }
    }

    /// The largest luminosity the star has had up to `t`, in the same way as
    /// [`StarModel::max_radius_until`] (plan 14).
    ///
    /// # Panics
    ///
    /// In debug builds, as [`StarModel::state_at`].
    #[must_use]
    pub fn max_luminosity_until(&self, t: UniverseTime) -> SolarLuminosities {
        let age = self.age_at(t);
        if age.value() <= 0.0 {
            return SolarLuminosities::ZERO;
        }
        match &self.evolution {
            Evolution::Track(track) => track.max_luminosity_until(age),
            Evolution::Cooling => self.cooling_at(Years::ZERO).luminosity(),
        }
    }

    /// The natal kick of the star's remnant (P06.T19's law): `None` for a star alive at the end of
    /// the clock window, for an object below 0.1 M☉, and where the star left nothing. A white
    /// dwarf's kick of about 1 km/s is its own, which a planet's orbit does not feel (ruling 62.6).
    #[must_use]
    pub fn natal_kick(&self) -> Option<NatalKick> {
        self.remnant.and_then(|stage| stage.natal_kick)
    }

    /// The bytes the model owns on the heap, beyond `size_of::<StarModel>()`: its boxed track and
    /// what the track owns, and nothing below 0.1 M☉ (for [`SystemStars::heap_bytes`]).
    #[must_use]
    pub(crate) fn heap_bytes(&self) -> usize {
        match &self.evolution {
            Evolution::Track(track) => size_of::<Track>() + track.heap_bytes(),
            Evolution::Cooling => 0,
        }
    }

    /// The initial mass the track is built for: the star's own, held to the formulae's 100 M☉
    /// until P06.T14.
    #[must_use]
    fn track_mass(&self) -> SolarMasses {
        if self.initial_mass > sse::MAX_INITIAL_MASS {
            sse::MAX_INITIAL_MASS
        } else {
            self.initial_mass
        }
    }

    /// The cooling fits' state at `age`, for an object below 0.1 M☉.
    #[must_use]
    fn cooling_at(&self, age: Years) -> StarState {
        substellar::cooling(self.initial_mass, age, &self.composition)
            .expect("a mass of 0.01-0.1 M_sun at a finite non-negative age is inside the fits")
    }

    /// The remnant stage: the model's own if its track reached the death, otherwise the rest of
    /// the star's life built to find it; `None` below 0.1 M☉.
    #[must_use]
    fn fate(&self) -> Option<RemnantStage> {
        match &self.evolution {
            Evolution::Cooling => None,
            Evolution::Track(_) => Some(self.remnant.unwrap_or_else(|| {
                let fate = sse::fate_of(
                    self.track_mass(),
                    &self.composition,
                    &self.draws,
                    TrackOptions::default(),
                );
                RemnantStage::new(fate.death, fate.remnant, &self.draws)
            })),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// P06.T29.b: a grid system's stars.

/// Whether a system exists at a clock time (plan 06's Provides): plan 03's
/// [`Existence`] in the stellar stage's terms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SystemExistence {
    /// The system's age at that time is zero or negative: nothing has formed yet, which the
    /// consoles read as `NOT YET FORMED` (ruling 34 of 2026-09-22).
    NotYetBorn,
    /// The system exists.
    Exists,
}

impl From<Existence> for SystemExistence {
    fn from(existence: Existence) -> Self {
        match existence {
            Existence::NoSystemYet => Self::NotYetBorn,
            Existence::Exists => Self::Exists,
        }
    }
}

/// When a system's primary dies on the universe clock (plan 06's Provides).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClockDeath {
    /// At this clock time, T = lifetime − age at the epoch, in this way. T is before the epoch
    /// for a star already dead then.
    At(UniverseTime, DeathKind),
    /// Beyond what the clock can hold: T's whole seconds do not fit an `i64` (about ±2.9 × 10¹¹
    /// years), or the object never dies, as one below 0.1 M☉ does not.
    BeyondClockRange,
    /// The system was a remnant already when it was born. No grid system is (a grid record's age
    /// at the epoch starts from its star's formation); plan 09's and plan 10's records of other
    /// origins may be.
    AlreadyRemnantAtBirth,
}

/// A star's summary at one clock time (plan 06, P06.T29.b): which body of its system it is, its
/// state, what kind of object it is, its class and absolute magnitudes, its remnant once it is
/// dead, and its death if that falls inside the clock window.
///
/// Variability (P06.T26), rotation and magnetism (T25), a planetary nebula (T16), the active
/// events (T28) and plan 11's binary class (P11.T5) are added by their tasks; this generator
/// version computes none of them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StarSummary {
    body: BodyId,
    state: StarState,
    kind: ObjectKind,
    classification: Classification,
    absolute_magnitude_v: Option<Magnitudes>,
    colour_b_v: Option<Magnitudes>,
    remnant: Option<CompactRemnant>,
    death_in_window: Option<(UniverseTime, DeathKind)>,
}

impl StarSummary {
    /// Which body of its system the star is: its body index is its
    /// [`StarIndex`](crate::stellar::multiplicity::StarIndex) in the system's hierarchy, 0 for the
    /// primary (plan 11, design note 5).
    #[must_use]
    pub const fn body(&self) -> BodyId {
        self.body
    }

    /// The star's state.
    #[must_use]
    pub const fn state(&self) -> &StarState {
        &self.state
    }

    /// What kind of object the star is ([`object_kind`]).
    #[must_use]
    pub const fn kind(&self) -> ObjectKind {
        self.kind
    }

    /// The star's MK or remnant class (P06.T23, T20.b).
    #[must_use]
    pub const fn classification(&self) -> Classification {
        self.classification
    }

    /// The absolute visual magnitude, `M_V`, where the tables of P06.T23.a reach (none for a remnant
    /// or an object cooler than L5).
    #[must_use]
    pub const fn absolute_magnitude_v(&self) -> Option<Magnitudes> {
        self.absolute_magnitude_v
    }

    /// The colour B − V, where the tables reach (none for an object with no luminosity or cooler
    /// than M9).
    #[must_use]
    pub const fn colour_b_v(&self) -> Option<Magnitudes> {
        self.colour_b_v
    }

    /// The remnant, once the star has died: its kind and mass.
    #[must_use]
    pub const fn remnant(&self) -> Option<CompactRemnant> {
        self.remnant
    }

    /// The star's death, its clock time and kind, if it falls inside the clock window [−H, +H]
    /// (the client's `DIES IN 312 yr`).
    #[must_use]
    pub const fn death_in_window(&self) -> Option<(UniverseTime, DeathKind)> {
        self.death_in_window
    }
}

/// A system's summary at one clock time: whether it exists, its composition, its stars, and the
/// hierarchy of orbits that holds them, of which there are none before it is born (plan 06's
/// Provides, with plan 11's P11.T2.c).
#[derive(Debug, Clone, PartialEq)]
pub struct SystemSummary {
    time: UniverseTime,
    existence: SystemExistence,
    composition: Composition,
    stars: Vec<StarSummary>,
    hierarchy: Option<SystemHierarchy>,
}

impl SystemSummary {
    /// The clock time summarised.
    #[must_use]
    pub const fn time(&self) -> UniverseTime {
        self.time
    }

    /// Whether the system exists then.
    #[must_use]
    pub const fn existence(&self) -> SystemExistence {
        self.existence
    }

    /// The composition all its stars share.
    #[must_use]
    pub const fn composition(&self) -> &Composition {
        &self.composition
    }

    /// Its stars by body index, primary first; empty before the system is born.
    #[must_use]
    pub fn stars(&self) -> &[StarSummary] {
        &self.stars
    }

    /// The hierarchy of its stars at the time summarised, or `None` before it is born: plan 11's
    /// `HierarchySummary`.
    ///
    /// Until plan 11's binary engine (P11.T4) gives a pair a state at each time, a pair is two
    /// single stars on the orbit drawn at the system's formation (ruling 33 of 2026-09-22), so the
    /// hierarchy at any time the system exists is [`SystemStars::hierarchy`]. Its stars are listed
    /// in the same body order as [`SystemSummary::stars`].
    #[must_use]
    pub const fn hierarchy(&self) -> Option<&SystemHierarchy> {
        self.hierarchy.as_ref()
    }
}

/// What a range query's row says of a system (plan 06's Provides): cheap, the primary's state and
/// classification only, and how many stars the system has (plan 11's Provides).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StellarBrief {
    kind: ObjectKind,
    class: Classification,
    log_luminosity: Option<Dex>,
    effective_temperature: Kelvin,
    star_count: u8,
}

impl StellarBrief {
    /// The brief of a primary in `state`, of `composition` and `draws`, in a system of
    /// `star_count` stars: its class, its kind, log L and `T_eff`. [`SystemStars::brief_at`] and
    /// the range brief ([`crate::stellar::brief`]) both make it here, so that a brief is the same
    /// function of the state whichever built it.
    #[must_use]
    pub(crate) fn of(
        state: &StarState,
        composition: &Composition,
        draws: &StarDraws,
        star_count: u8,
    ) -> Self {
        let class = classify(state, composition, draws, &ClassExtras::NONE);
        let luminosity = state.luminosity().value();
        Self {
            kind: object_kind(state, &class, composition),
            class,
            log_luminosity: (luminosity > 0.0).then(|| Dex::new(math::log10(luminosity))),
            effective_temperature: state.effective_temperature(),
            star_count,
        }
    }

    /// How many stars the system has, the primary included: 1 to 1 +
    /// [`MAX_COMPANIONS`](crate::stellar::multiplicity::MAX_COMPANIONS).
    #[must_use]
    pub const fn star_count(&self) -> u8 {
        self.star_count
    }

    /// What kind of object the primary is.
    #[must_use]
    pub const fn kind(&self) -> ObjectKind {
        self.kind
    }

    /// Its class.
    #[must_use]
    pub const fn class(&self) -> Classification {
        self.class
    }

    /// log₁₀ of its luminosity in L☉, or `None` for an object with none (a black hole, or
    /// nothing), whose logarithm no consumer may take (ruling 40 of 2026-09-22).
    #[must_use]
    pub const fn log_luminosity(&self) -> Option<Dex> {
        self.log_luminosity
    }

    /// Its effective temperature, K; zero for an object with no luminosity.
    #[must_use]
    pub const fn effective_temperature(&self) -> Kelvin {
        self.effective_temperature
    }
}

/// What kind of object a star of `state`, `classification` and `composition` is (plan 06, design
/// note 17), from its phase and, for a star burning hydrogen or helium in a shell or core, its
/// luminosity class:
///
/// - V and the subdwarf classes are dwarfs, IV subgiants, III and II giants, and Ib to Ia⁺
///   supergiants;
/// - a naked helium star (HPT types 7–9) is a Wolf-Rayet star above P06.T24.a's luminosity floor
///   of 10⁴·⁹ L☉ × (Z ÷ 0.02)^−0.4, where Z is the metal fraction the formulae see, and a hot
///   subdwarf below it; the Wolf-Rayet rule's other half, a hydrogen-rich star nearly stripped
///   (`WNh`), and the floor's recorded source are T24.a's;
/// - an object on P06.T13's cooling fits is substellar below the hydrogen-burning limit
///   ([`substellar::hydrogen_burning_limit`]) and a dwarf above it;
/// - protostars, pre-main-sequence stars and the remnants are their phases, and a post-AGB star is
///   classed as a living star by its luminosity class until P06.T24 gives it a class of its own.
#[must_use]
pub fn object_kind(
    state: &StarState,
    classification: &Classification,
    composition: &Composition,
) -> ObjectKind {
    match state.phase() {
        Phase::Protostar => ObjectKind::Protostar,
        Phase::PreMainSequence => ObjectKind::PreMainSequence,
        Phase::HeliumWhiteDwarf | Phase::CarbonOxygenWhiteDwarf | Phase::OxygenNeonWhiteDwarf => {
            ObjectKind::WhiteDwarf
        }
        Phase::NeutronStar => ObjectKind::NeutronStar,
        Phase::BlackHole => ObjectKind::BlackHole,
        Phase::NoRemnant => ObjectKind::NoRemnant,
        Phase::HeliumMainSequence | Phase::HeliumHertzsprungGap | Phase::HeliumGiantBranch => {
            let floor = WOLF_RAYET_FLOOR_L_SUN
                * math::powf(composition.z_fit().value() / 0.02, WOLF_RAYET_FLOOR_Z_SLOPE);
            if state.luminosity().value() > floor {
                ObjectKind::WolfRayet
            } else {
                ObjectKind::HotSubdwarf
            }
        }
        Phase::Substellar if state.mass() < substellar::hydrogen_burning_limit(composition) => {
            ObjectKind::Substellar
        }
        Phase::Substellar
        | Phase::MainSequence
        | Phase::HertzsprungGap
        | Phase::FirstGiantBranch
        | Phase::CoreHeliumBurning
        | Phase::EarlyAgb
        | Phase::ThermallyPulsingAgb
        | Phase::PostAgb => match classification.luminosity_class() {
            Some(
                LuminosityClass::Dwarf
                | LuminosityClass::Subdwarf
                | LuminosityClass::ExtremeSubdwarf,
            )
            | None => ObjectKind::Dwarf,
            Some(LuminosityClass::Subgiant) => ObjectKind::Subgiant,
            Some(LuminosityClass::Giant | LuminosityClass::BrightGiant) => ObjectKind::Giant,
            Some(
                LuminosityClass::LessLuminousSupergiant
                | LuminosityClass::Supergiant
                | LuminosityClass::LuminousSupergiant
                | LuminosityClass::Hypergiant,
            ) => ObjectKind::Supergiant,
        },
    }
}

/// P06.T24.a's luminosity floor of a Wolf-Rayet star at Z = 0.02, 10⁴·⁹ L☉.
const WOLF_RAYET_FLOOR_L_SUN: f64 = 79_432.823_472_428_15;

/// The floor's slope in Z ÷ 0.02, −0.4 (P06.T24.a).
const WOLF_RAYET_FLOOR_Z_SLOPE: f64 = -0.4;

/// The redraw attempt a grid system's companions are drawn at: the first, [`RedrawAttempt::FIRST`].
///
/// **A named seam** (plan 11, P11.T2.c). The grid redraws a system whose binary falls into a
/// catalogue class or explodes as a Type Ia (P11.T6–T8, plan 08's `SystemRecord::mark_attempt`),
/// and then reads a later attempt here, for the hierarchy and for each companion's own draws alike
/// ([`StarDraws::for_attempt`]), since both are blocks of [`ATTEMPT_WORDS`](crate::stellar::draws::ATTEMPT_WORDS)
/// words. No record of this generator version is redrawn.
pub(crate) const GRID_ATTEMPT: RedrawAttempt = RedrawAttempt::FIRST;

/// A grid system's stars from its record (plan 06, P06.T29.b, with plan 11's P11.T2.c): its
/// primary, body 0 of the system, its companions, and the hierarchy of orbits that holds them.
///
/// [`SystemStars::generate`] takes three steps for the primary, because plan 08's P08.T12.c
/// repeats only the last: the primary's draws ([`StarDraws::for_star`] of body 0; plan 08 makes it
/// `for_attempt` of the record's mark attempt), its track, and the remnant stage on that track
/// ([`StarModel`]). With no kick constraint, which is every record of this generator version, the
/// last runs once. The companions and their orbits are plan 11's [`draw_hierarchy`], and each
/// companion is a [`StarModel`] of its slot's initial mass, on its own body's draws, with the
/// system's composition and age. The primary never depends on the companions: its model is plan
/// 06's, bit for bit, whatever the hierarchy.
///
/// Until plan 11's binary engine (P11.T4) a pair is two single stars on an orbit (ruling 33 of
/// 2026-09-22): each star evolves alone, and no star's evolution reads its companion.
///
/// # Examples
///
/// The first system of a cell at the solar circle, as the `SYSTEM` display summarises it:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::placement::{CellKey, generate_cell};
/// use hyperion_sim::id::Layer;
/// use hyperion_sim::stellar::system::{SystemExistence, SystemStars};
/// use hyperion_sim::time::UniverseTime;
///
/// let galaxy = Galaxy::new(Seed::new(11));
/// let mut cell = Vec::new();
/// generate_cell(&galaxy, CellKey::new(Layer::C, [0, 812, 0])?, &mut cell);
/// let record = cell.first().ok_or("the cell has systems")?;
/// let stars = SystemStars::generate(&galaxy, record);
/// let summary = stars.summary_at(UniverseTime::EPOCH);
/// if summary.existence() == SystemExistence::Exists {
///     // Every star of the system, by body index: the primary first.
///     assert_eq!(summary.stars().len(), usize::from(stars.star_count()));
///     let primary = &summary.stars()[0];
///     println!("{} {:?}", primary.classification(), primary.kind());
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct SystemStars {
    record: SystemRecord,
    hierarchy: SystemHierarchy,
    stars: Vec<StarModel>,
}

impl SystemStars {
    /// The stars of the grid system `record` in `galaxy`, with the model's own multiplicity:
    /// [`SystemStars::generate_in`] with [`MultiplicityContext::Free`].
    ///
    /// # Panics
    ///
    /// As [`draw_metallicity`] does, for a record of another galaxy.
    #[must_use]
    pub fn generate(galaxy: &Galaxy, record: &SystemRecord) -> Self {
        Self::generate_in(galaxy, record, MultiplicityContext::Free)
    }

    /// The stars of the system `record` in `galaxy` under the multiplicity context `ctx` (plan 11,
    /// P11.T2.c): its [`draw_metallicity`], the hierarchy [`draw_hierarchy`] draws for `ctx`, the
    /// primary's [`StarModel`] at the record's initial mass and age on its own draws, and each
    /// companion's at its slot's initial mass and the same age, on the draws of its own body
    /// ([`StarDraws::for_attempt`] at the system's attempt, 0 for every grid record).
    ///
    /// Grid systems take [`MultiplicityContext::Free`] ([`SystemStars::generate`]); plan 09's
    /// cluster members will pass `ForcedMultiple`, and `ForcedSingle` gives the primary alone, which
    /// is exactly plan 06's single-star system.
    ///
    /// # Panics
    ///
    /// As [`draw_metallicity`] does, for a record of another galaxy.
    #[must_use]
    pub fn generate_in(galaxy: &Galaxy, record: &SystemRecord, ctx: MultiplicityContext) -> Self {
        let composition = draw_metallicity(galaxy, record);
        let hierarchy = draw_hierarchy(galaxy, record, ctx, GRID_ATTEMPT);
        let age = record.age_at_epoch();
        let mut stars = Vec::with_capacity(hierarchy.stars().len());
        stars.push(
            StarModel::new(
                record.primary_initial_mass(),
                composition,
                primary_draws(galaxy, record),
                age,
            )
            .expect("a grid record's primary is of 0.08-150 M_sun with a finite age"),
        );
        stars.extend(hierarchy.stars().iter().skip(1).map(|slot| {
            let draws =
                StarDraws::for_attempt(galaxy.seed(), slot.body(), u32::from(GRID_ATTEMPT.get()));
            StarModel::new(slot.initial_mass(), composition, draws, age)
                .expect("a companion is of 0.08 M_sun up to its primary's mass, with a finite age")
        }));
        Self {
            record: *record,
            hierarchy,
            stars,
        }
    }

    /// The system's record.
    #[must_use]
    pub const fn record(&self) -> &SystemRecord {
        &self.record
    }

    /// The system's stars by body index, primary first: what plan 14's `SystemContext` holds
    /// (ruling 34). Star k is the hierarchy's star k.
    #[must_use]
    pub fn stars(&self) -> &[StarModel] {
        &self.stars
    }

    /// The primary star.
    #[must_use]
    pub fn primary(&self) -> &StarModel {
        &self.stars[0]
    }

    /// The hierarchy of the system's stars and the orbits that hold them (plan 11, P11.T2.a–b).
    #[must_use]
    pub const fn hierarchy(&self) -> &SystemHierarchy {
        &self.hierarchy
    }

    /// How many stars the system has, the primary included: 1 to 1 +
    /// [`MAX_COMPANIONS`](crate::stellar::multiplicity::MAX_COMPANIONS).
    #[must_use]
    pub fn star_count(&self) -> u8 {
        self.hierarchy.star_count()
    }

    /// The bytes the system's stars own on the heap, beyond `size_of::<SystemStars>()`: each
    /// star's model and track and the hierarchy's lists, by capacity.
    ///
    /// It is what the server charges a cached system against its byte budget (plan 06, P06.T34;
    /// plan 04, design note 23), as [`Galaxy::heap_bytes`] is for a galaxy. A system of one living
    /// dwarf owns a few kilobytes; nothing generated reads it.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        self.stars.iter().fold(
            self.stars.capacity() * size_of::<StarModel>() + self.hierarchy.heap_bytes(),
            |bytes, star| bytes + star.heap_bytes(),
        )
    }

    /// The system at `t`: whether it exists ([`SystemRecord::existence_at`]), its composition,
    /// each star's [`StarSummary`] at its age then ([`SystemRecord::age_at`]), and its hierarchy;
    /// no star and no hierarchy before the system is born.
    ///
    /// # Panics
    ///
    /// In debug builds, as [`StarModel::state_at`], for a `t` after the clock window's end.
    #[must_use]
    pub fn summary_at(&self, t: UniverseTime) -> SystemSummary {
        let existence = SystemExistence::from(self.record.existence_at(t));
        let (stars, hierarchy) = match existence {
            SystemExistence::NotYetBorn => (Vec::new(), None),
            SystemExistence::Exists => (
                self.stars
                    .iter()
                    .zip(self.hierarchy.stars())
                    .filter_map(|(star, slot)| star_summary(star, slot.body(), t))
                    .collect(),
                Some(self.hierarchy.clone()),
            ),
        };
        SystemSummary {
            time: t,
            existence,
            composition: *self.primary().composition(),
            stars,
            hierarchy,
        }
    }

    /// The system's brief at `t`, for a range query's row, or `None` before the system is born:
    /// its primary's kind, class, luminosity and temperature, with no photometry and no death, and
    /// how many stars it has.
    ///
    /// # Panics
    ///
    /// In debug builds, as [`StarModel::state_at`].
    #[must_use]
    pub fn brief_at(&self, t: UniverseTime) -> Option<StellarBrief> {
        let primary = self.primary();
        let state = primary.state_at(t)?;
        Some(StellarBrief::of(
            &state,
            primary.composition(),
            primary.draws(),
            self.star_count(),
        ))
    }

    /// When the primary dies on the clock: T = lifetime − age at the epoch.
    ///
    /// It costs the rest of the star's life where the star outlives the clock window
    /// ([`StarModel::lifetime`]).
    #[must_use]
    pub fn death_time(&self) -> ClockDeath {
        let primary = self.primary();
        let Some(death) = primary.death() else {
            return ClockDeath::BeyondClockRange;
        };
        clock_death(primary.age_at_epoch(), death)
    }

    /// The primary remnant's natal kick, as [`StarModel::natal_kick`] gives it (P06.T19).
    #[must_use]
    pub fn natal_kick(&self) -> Option<NatalKick> {
        self.primary().natal_kick()
    }

    /// The clock interval in which the primary is a luminous blue variable: `None` until
    /// P06.T24.a builds the criterion and `Track::window_where` (plan 09's catalogue class reads
    /// it).
    #[must_use]
    pub fn lbv_window(&self) -> Option<(UniverseTime, UniverseTime)> {
        None
    }
}

/// The primary's draws: body 0's at attempt 0, [`StarDraws::for_star`].
///
/// **A named seam** (plan 06, P06.T29.b; plan 11, P11.T2.c's second): plan 08's P08.T12.c makes it
/// `for_attempt` of the record's `mark_attempt()`, which does not exist before then.
#[must_use]
pub(crate) fn primary_draws(galaxy: &Galaxy, record: &SystemRecord) -> StarDraws {
    StarDraws::for_star(galaxy.seed(), BodyId::new(record.id(), 0))
}

/// The primary's η alone: [`primary_draws`]'s [`eta`](StarDraws::eta), bit for bit, from its one
/// stream, for the range brief of a main-sequence primary (P06.T38.e).
///
/// **The same seam as [`primary_draws`]:** plan 08's P08.T12.c changes both to the record's
/// `mark_attempt()` together.
#[must_use]
pub(crate) fn primary_eta(galaxy: &Galaxy, record: &SystemRecord) -> StandardNormal {
    StarDraws::eta_for_attempt(galaxy.seed(), BodyId::new(record.id(), 0), 0)
}

/// The clock time of `death` for a star whose age at the epoch is `age_at_epoch`.
#[must_use]
fn clock_death(age_at_epoch: Years, death: Death) -> ClockDeath {
    let years = death.age().value() - age_at_epoch.value();
    match Span::from_seconds_f64(years * SECONDS_PER_JULIAN_YEAR)
        .and_then(|span| UniverseTime::EPOCH.checked_add(span))
    {
        Some(t) => ClockDeath::At(t, death.kind()),
        None => ClockDeath::BeyondClockRange,
    }
}

/// The summary of `star`, the system's body `body`, at `t`, or `None` if it has not formed by then.
#[must_use]
fn star_summary(star: &StarModel, body: BodyId, t: UniverseTime) -> Option<StarSummary> {
    let state = star.state_at(t)?;
    let classification = classify(&state, star.composition(), star.draws(), &ClassExtras::NONE);
    let remnant = if state.phase().is_remnant() {
        star.remnant()
    } else {
        None
    };
    // A death inside the window is always one the model holds, because its track is built to
    // the window's end: nothing further is built for it.
    let death_in_window =
        star.remnant.and_then(
            |stage| match clock_death(star.age_at_epoch(), stage.death) {
                ClockDeath::At(when, kind) if ClockWindow::contains(when) => Some((when, kind)),
                ClockDeath::At(..)
                | ClockDeath::BeyondClockRange
                | ClockDeath::AlreadyRemnantAtBirth => None,
            },
        );
    Some(StarSummary {
        body,
        kind: object_kind(&state, &classification, star.composition()),
        classification,
        absolute_magnitude_v: absolute_magnitude_v(&state),
        colour_b_v: colour_b_v(state.effective_temperature()),
        remnant,
        death_in_window,
        state,
    })
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::coords::GalacticPosition;
    use crate::galaxy::Population;
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::placement::{CellKey, SystemOrigin};
    use crate::id::Layer;
    use crate::rng::Seed;
    use crate::units::SolarMasses;

    const SEED: u64 = 0x0600_0003_5eed_0001;

    fn galaxy() -> Galaxy {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral")
    }

    /// A grid record of the young thin disc (component 0) at the Sun-like point with `age`.
    fn young_disc_record(galaxy: &Galaxy, index: u32, age: f64) -> SystemRecord {
        let key = CellKey::new(Layer::C, [0, 812, 0]).unwrap();
        SystemRecord::from_parts(
            key.candidate_id(index).unwrap(),
            GalacticPosition::from_light_years([0.0, 26_000.0, 30.0]).unwrap(),
            SystemOrigin::Grid(galaxy.fields().component_id(0).unwrap()),
            Population::YoungThinDisc,
            SolarMasses::new(1.0),
            Years::new(age),
        )
    }

    #[test]
    fn the_abundance_is_the_components_mean_plus_its_sigma_times_one_normal() {
        let galaxy = galaxy();
        let record = young_disc_record(&galaxy, 5, 2.0e7);
        let expected = galaxy
            .fields()
            .component(record.component().unwrap())
            .metallicity(&PointLy::new(0.0, 26_000.0, 30.0), Years::new(2.0e7));
        let mut stream = Stream::open(
            galaxy.seed(),
            tags::SYSTEM_METALLICITY,
            ObjectKey::from(record.id()),
        );
        let z = stream.standard_normal();
        let drawn = draw_metallicity(&galaxy, &record);
        assert_same_bits(
            drawn.fe_h().value(),
            expected.mean().value() + expected.sigma().value() * z,
        );
        assert_same_bits(
            drawn.z().value(),
            Composition::from_fe_h(drawn.fe_h(), HeliumExcess::ZERO)
                .z()
                .value(),
        );
        assert_same_bits(drawn.helium_excess().value(), 0.0);
    }

    #[test]
    fn a_system_not_yet_born_reads_its_distribution_at_age_zero() {
        let galaxy = galaxy();
        let unborn = young_disc_record(&galaxy, 9, -3.0e7);
        let newborn = young_disc_record(&galaxy, 9, 0.0);
        assert_eq!(age_read(&unborn), Years::ZERO);
        // Every law of this generator version is flat below 8 Gyr, so the draw itself cannot show
        // the rule yet; `age_read` above is what holds it.
        assert_eq!(
            draw_metallicity(&galaxy, &unborn),
            draw_metallicity(&galaxy, &newborn)
        );
        // An old system reads its own age.
        let old = young_disc_record(&galaxy, 9, 9.5e9);
        assert_same_bits(age_read(&old).value(), 9.5e9);
    }

    #[test]
    fn systems_draw_independently_and_the_same_system_draws_the_same_abundance() {
        let galaxy = galaxy();
        let records: Vec<SystemRecord> = (0..32)
            .map(|i| young_disc_record(&galaxy, i, 1.0e7))
            .collect();
        let draws: Vec<f64> = records
            .iter()
            .map(|r| draw_metallicity(&galaxy, r).fe_h().value())
            .collect();
        for (i, a) in draws.iter().enumerate() {
            for b in &draws[..i] {
                assert!(
                    a.total_cmp(b).is_ne(),
                    "two systems drew the same abundance"
                );
            }
        }
        hyperion_testkit::order::assert_order_independent(&records, |r| {
            draw_metallicity(&galaxy, r)
        });
    }

    // -----------------------------------------------------------------------------------------
    // P06.T29.a: `StarModel`.

    use crate::stellar::Phase;
    use crate::stellar::draws::{StandardNormal, StarDrawsParts};
    use crate::stellar::remnant::{DeathKind, RemnantKind};
    use crate::time::Span;

    /// Stars across the backbone and around it: (initial mass, [Fe/H], age at the epoch).
    const STARS: [(f64, f64, f64); 12] = [
        (0.3, 0.0, 8.0e9),
        (1.0, 0.0, 4.57e9),
        (1.0, -1.2, 1.2e10),
        (2.0, 0.0, 1.2e9),
        (2.0, 0.3, 5.0e9),
        (5.0, -0.5, 1.2e8),
        (8.25, 0.0, 6.0e7),
        (20.0, 0.0, 8.0e6),
        (20.0, 0.0, 5.0e7),
        (40.0, -2.0, 1.0e9),
        (100.0, 0.0, 2.0e6),
        (0.05, 0.0, 3.0e9),
    ];

    fn star_draws(i: usize) -> StarDraws {
        let mut s = Stream::open(
            Seed::new(0x0629_a000 + u64::try_from(i).unwrap()),
            tags::SELFTEST_STREAM,
            ObjectKey::galaxy(),
        );
        StarDraws::from_parts(StarDrawsParts {
            eta: StandardNormal::new(s.standard_normal()).unwrap(),
            remnant_type: s.mark(),
            remnant_fallback: s.mark(),
            remnant_mass: StandardNormal::new(s.standard_normal()).unwrap(),
            ..StarDrawsParts::MEDIAN
        })
    }

    fn comp(fe_h: f64) -> Composition {
        Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO)
    }

    fn model(i: usize) -> StarModel {
        let (m, fe_h, age) = STARS[i];
        StarModel::new(
            SolarMasses::new(m),
            comp(fe_h),
            star_draws(i),
            Years::new(age),
        )
        .unwrap()
    }

    fn years(y: i64) -> UniverseTime {
        UniverseTime::from_julian_years(y).unwrap()
    }

    #[test]
    fn the_state_at_the_epoch_is_the_tracks_at_the_age_at_the_epoch_bit_for_bit() {
        for (i, &(m, fe_h, age)) in STARS.iter().enumerate() {
            let star = model(i);
            let state = star.state_at(UniverseTime::EPOCH).expect("formed");
            let expected = if m < 0.1 {
                substellar::cooling(SolarMasses::new(m), Years::new(age), &comp(fe_h)).unwrap()
            } else {
                Track::to_age(
                    SolarMasses::new(m),
                    &comp(fe_h),
                    &star_draws(i),
                    Years::new(age),
                )
                .state_at(Years::new(age))
            };
            assert_eq!(state, expected, "{m} M☉ at {age} yr");
            hyperion_testkit::float::assert_same_bits(
                state.luminosity().value(),
                expected.luminosity().value(),
            );
        }
    }

    /// A star dead at the epoch has its whole track and its remnant, which are the full track's;
    /// a living one finds its death by building the rest of its life, which is the full track's
    /// too, bit for bit.
    #[test]
    fn a_star_dead_at_the_epoch_has_its_full_track_and_a_remnant() {
        for (i, &(m, fe_h, age)) in STARS.iter().enumerate() {
            let star = model(i);
            if m < 0.1 {
                assert_eq!(star.lifetime(), None);
                assert_eq!(star.death(), None);
                assert_eq!(star.remnant(), None);
                continue;
            }
            let full = Track::full(SolarMasses::new(m), &comp(fe_h), &star_draws(i));
            let life = full.lifetime().unwrap();
            let lifetime = star.lifetime().unwrap();
            hyperion_testkit::float::assert_same_bits(lifetime.value(), life.value());
            assert_eq!(star.death(), full.death(), "{m} M☉");
            assert_eq!(star.remnant(), full.remnant(), "{m} M☉");
            let dead = age > life.value();
            assert_eq!(star.remnant.is_some(), dead || age + 1_000.0 > life.value());
            let now = star.state_at(UniverseTime::EPOCH).unwrap();
            assert_eq!(
                now.phase().is_remnant(),
                dead,
                "{m} M☉ at {age}: {:?}",
                now.phase()
            );
            if dead {
                assert_eq!(now, full.state_at(Years::new(age)));
                let Evolution::Track(track) = &star.evolution else {
                    panic!("a star of {m} M☉ has a track");
                };
                assert!(track.built_until().value().is_infinite());
            }
        }
    }

    #[test]
    fn a_model_does_not_depend_on_what_was_asked_before() {
        let times: Vec<UniverseTime> = [-1_000_i64, -999, -250, -1, 0, 1, 17, 500, 999, 1_000]
            .into_iter()
            .map(years)
            .collect();
        for i in 0..STARS.len() {
            let star = model(i);
            hyperion_testkit::order::assert_order_independent(&times, |t| {
                (
                    star.state_at(*t),
                    star.max_radius_until(*t),
                    star.max_luminosity_until(*t),
                )
            });
        }
        let indices: Vec<usize> = (0..STARS.len()).collect();
        hyperion_testkit::order::assert_order_independent(&indices, |&i| model(i));
        assert_eq!(model(3), model(3));
    }

    /// The largest radius and luminosity so far never fall across the window and never lie below
    /// the state's, for every kind of star, and a substellar object's are its youngest state's.
    #[test]
    fn the_largest_radius_and_luminosity_so_far_never_fall() {
        for i in 0..STARS.len() {
            let star = model(i);
            let (mut r, mut l) = (0.0, 0.0);
            for y in (-1_000..=1_000).step_by(50) {
                let t = years(y);
                let (rm, lm) = (
                    star.max_radius_until(t).value(),
                    star.max_luminosity_until(t).value(),
                );
                assert!(rm >= r && lm >= l, "star {i} at {y}");
                let now = star.state_at(t).unwrap();
                assert!(rm >= now.radius().value() && lm >= now.luminosity().value());
                (r, l) = (rm, lm);
            }
        }
        let brown = model(11);
        let youngest =
            substellar::cooling(SolarMasses::new(0.05), Years::ZERO, &comp(0.0)).unwrap();
        assert_eq!(
            brown.max_radius_until(UniverseTime::EPOCH),
            youngest.radius()
        );
        assert_eq!(
            brown.state_at(UniverseTime::EPOCH).unwrap().phase(),
            Phase::Substellar
        );
    }

    /// A star whose onset of collapse is 500 years after the epoch has no state and no radius
    /// until then, and exists after it.
    #[test]
    fn a_star_not_yet_formed_has_no_state() {
        let star = StarModel::new(
            SolarMasses::new(1.0),
            Composition::SOLAR,
            StarDraws::median(),
            Years::new(-500.0),
        )
        .unwrap();
        assert_eq!(star.state_at(UniverseTime::EPOCH), None);
        assert_eq!(star.state_at(years(500)), None);
        assert_eq!(star.max_radius_until(years(400)), SolarRadii::ZERO);
        assert_eq!(
            star.max_luminosity_until(years(400)),
            SolarLuminosities::ZERO
        );
        let later = star.state_at(years(600)).unwrap();
        assert!((later.age().value() - 100.0).abs() < 1e-9);
    }

    #[test]
    fn a_model_refuses_masses_and_ages_outside_its_range() {
        let build = |m: f64, age: f64| {
            StarModel::new(
                SolarMasses::new(m),
                Composition::SOLAR,
                StarDraws::median(),
                Years::new(age),
            )
        };
        for m in [0.005, 150.5, f64::NAN, -1.0] {
            assert!(
                matches!(build(m, 1e9), Err(BuildStarModelError::MassOutsideRange(_))),
                "{m}"
            );
        }
        assert!(matches!(
            build(1.0, f64::INFINITY),
            Err(BuildStarModelError::AgeNotFinite(_))
        ));
        assert!(build(0.01, 1e9).is_ok() && build(150.0, 1e6).is_ok());
        assert_eq!(
            build(0.005, 1e9).unwrap_err().to_string(),
            "initial mass 0.005 M_sun is outside 0.01-150"
        );
    }

    /// Until P06.T14 a star above 100 M☉ is evolved as one of 100 M☉, and keeps its own mass.
    #[test]
    fn a_star_above_a_hundred_solar_masses_is_evolved_at_a_hundred() {
        let (heavy, limit) = (
            StarModel::new(
                SolarMasses::new(130.0),
                Composition::SOLAR,
                StarDraws::median(),
                Years::new(1e6),
            )
            .unwrap(),
            StarModel::new(
                SolarMasses::new(100.0),
                Composition::SOLAR,
                StarDraws::median(),
                Years::new(1e6),
            )
            .unwrap(),
        );
        assert_eq!(heavy.initial_mass(), SolarMasses::new(130.0));
        assert_eq!(
            heavy.state_at(UniverseTime::EPOCH),
            limit.state_at(UniverseTime::EPOCH)
        );
    }

    /// The remnant stage of a dead star is its track's for its own draws, with the stripped mark
    /// applied and the kick law's kick, and on the same track other remnant draws redraw only the
    /// remnant and the kind of death, never its age, and other kick draws only the kick: what
    /// plan 08's attempts repeat.
    #[test]
    fn the_remnant_stage_redraws_only_the_remnant() {
        let star = model(8);
        let Evolution::Track(track) = &star.evolution else {
            panic!("a 20 M☉ star has a track");
        };
        let own = star.remnant.expect("dead at 50 Myr");
        let law = StandardKickLaw::default();
        let death = track.death().expect("dead at 50 Myr");
        assert_eq!(own.death, law.with_stripped_mark(death, star.draws()));
        assert_eq!(own.death.kind(), death.kind());
        assert_same_bits(own.death.age().value(), death.age().value());
        assert_eq!(Some(own.remnant), track.remnant());
        let kick = star
            .natal_kick()
            .expect("a collapse leaves a kicked remnant");
        assert_eq!(
            Some(kick),
            law.natal_kick(&own.death, &own.remnant, star.draws())
        );
        let turned = StarDraws::from_parts(StarDrawsParts {
            kick_direction: crate::coords::UnitVector::X,
            kick_low: [crate::stellar::draws::StandardNormal::new(0.6).unwrap(); 3],
            ..star.draws().parts().clone()
        });
        let stage = remnant_stage(track, &turned).unwrap();
        assert_eq!((stage.death, stage.remnant), (own.death, own.remnant));
        assert_ne!(
            stage.natal_kick,
            Some(kick),
            "the kick draws redraw the kick"
        );
        let mut kinds = Vec::new();
        for word in [0_u64, u64::MAX / 3, u64::MAX / 3 * 2, u64::MAX] {
            let draws = StarDraws::from_parts(StarDrawsParts {
                remnant_type: crate::rng::Mark::from_word(word),
                remnant_fallback: crate::rng::Mark::from_word(word),
                ..star.draws().parts().clone()
            });
            let stage = remnant_stage(track, &draws).unwrap();
            hyperion_testkit::float::assert_same_bits(
                stage.death.age().value(),
                own.death.age().value(),
            );
            assert_eq!(stage.death.progenitor(), own.death.progenitor());
            kinds.push(stage.remnant.kind());
            if stage.remnant.kind() == RemnantKind::BlackHole {
                assert!(matches!(
                    stage.death.kind(),
                    DeathKind::DirectCollapse | DeathKind::CoreCollapse { .. }
                ));
            }
        }
        assert!(
            kinds.contains(&RemnantKind::NeutronStar) && kinds.contains(&RemnantKind::BlackHole)
        );
    }

    /// A living state of `phase` with mass `m`, luminosity `l` and radius `r` (solar units).
    fn living(phase: Phase, m: f64, l: f64, r: f64) -> StarState {
        StarState::new(crate::stellar::StarStateParts {
            phase,
            age: Years::new(1e7),
            mass: SolarMasses::new(m),
            core_mass: SolarMasses::new(0.5 * m),
            luminosity: SolarLuminosities::new(l),
            radius: SolarRadii::new(r),
            mass_loss_rate: crate::units::SolarMassesPerYear::ZERO,
            phase_fraction: 0.5,
        })
    }

    fn kind_of(state: &StarState, composition: &Composition) -> ObjectKind {
        let class = classify(state, composition, &StarDraws::median(), &ClassExtras::NONE);
        object_kind(state, &class, composition)
    }

    /// The kind follows the phase and the luminosity class: the Sun is a dwarf, Arcturus a giant,
    /// Betelgeuse a supergiant, a luminous naked helium star a Wolf-Rayet star and a faint one a
    /// hot subdwarf, and a cooling-fit object a brown dwarf below the hydrogen-burning limit.
    #[test]
    fn the_kind_follows_the_phase_and_the_luminosity_class() {
        let solar = Composition::SOLAR;
        assert_eq!(
            kind_of(&living(Phase::MainSequence, 1.0, 1.0, 1.0), &solar),
            ObjectKind::Dwarf
        );
        assert_eq!(
            kind_of(&living(Phase::FirstGiantBranch, 1.1, 170.0, 25.4), &solar),
            ObjectKind::Giant
        );
        assert_eq!(
            kind_of(
                &living(Phase::CoreHeliumBurning, 18.0, 1.1e5, 760.0),
                &solar
            ),
            ObjectKind::Supergiant
        );
        // The floor is 10^4.9 L☉ at Z = 0.02 and 10^4.9 × 10^0.4 at Z = 0.002.
        let hot = |l: f64| living(Phase::HeliumMainSequence, 10.0, l, 0.8);
        assert_eq!(kind_of(&hot(1.0e5), &solar), ObjectKind::WolfRayet);
        assert_eq!(kind_of(&hot(6.0e4), &solar), ObjectKind::HotSubdwarf);
        let poor = comp(-1.0);
        assert_eq!(kind_of(&hot(1.5e5), &poor), ObjectKind::HotSubdwarf);
        assert_eq!(kind_of(&hot(2.5e5), &poor), ObjectKind::WolfRayet);
        let cool = |m: f64| {
            substellar::cooling(SolarMasses::new(m), Years::new(5e9), &solar).expect("in the fits")
        };
        assert_eq!(kind_of(&cool(0.05), &solar), ObjectKind::Substellar);
        assert_eq!(kind_of(&cool(0.09), &solar), ObjectKind::Dwarf);
    }

    /// The brief is the summary's primary, in brief, and a black hole is a black hole.
    #[test]
    fn the_brief_is_the_summary_in_brief() {
        let galaxy = galaxy();
        let record = young_disc_record(&galaxy, 7, 3.0e9);
        let stars = SystemStars::generate(&galaxy, &record);
        let summary = stars.summary_at(UniverseTime::EPOCH);
        let brief = stars.brief_at(UniverseTime::EPOCH).unwrap();
        let primary = &summary.stars()[0];
        assert_eq!(brief.kind(), primary.kind());
        assert_eq!(brief.class(), primary.classification());
        assert_eq!(brief.star_count(), stars.star_count());
        assert_eq!(summary.stars().len(), usize::from(stars.star_count()));
        assert_eq!(
            brief.effective_temperature(),
            primary.state().effective_temperature()
        );
        assert!(
            (brief.log_luminosity().unwrap().value()
                - math::log10(primary.state().luminosity().value()))
            .abs()
                < 1e-15
        );
        let dark = StarState::new(crate::stellar::StarStateParts {
            phase: Phase::BlackHole,
            age: Years::new(1e8),
            mass: SolarMasses::new(8.0),
            core_mass: SolarMasses::new(8.0),
            luminosity: SolarLuminosities::ZERO,
            radius: SolarRadii::new(3.4e-5),
            mass_loss_rate: crate::units::SolarMassesPerYear::ZERO,
            phase_fraction: 0.0,
        });
        assert_eq!(kind_of(&dark, &Composition::SOLAR), ObjectKind::BlackHole);
    }

    /// A system's heap bytes are its stars' models and tracks and its hierarchy's lists, so a
    /// multiple system owns more than its primary alone, and a star below 0.1 M☉ owns no track.
    #[test]
    fn a_systems_heap_bytes_count_every_stars_track() {
        let galaxy = galaxy();
        let record = (0..256)
            .map(|i| young_disc_record(&galaxy, i, 2.0e9))
            .find(|r| SystemStars::generate(&galaxy, r).star_count() > 1)
            .expect("a multiple system among 256 Sun-like primaries");
        let multiple = SystemStars::generate(&galaxy, &record);
        let single = SystemStars::generate_in(&galaxy, &record, MultiplicityContext::ForcedSingle);
        let Evolution::Track(track) = &single.primary().evolution else {
            panic!("a star of 1 M☉ has a track");
        };
        assert_eq!(
            single.heap_bytes(),
            single.stars.capacity() * size_of::<StarModel>()
                + size_of::<Track>()
                + track.heap_bytes()
                + single.hierarchy().heap_bytes()
        );
        assert!(track.heap_bytes() > 0);
        assert!(multiple.heap_bytes() > single.heap_bytes());
        let brown = StarModel::new(
            SolarMasses::new(0.05),
            Composition::SOLAR,
            StarDraws::median(),
            Years::new(1e9),
        )
        .unwrap();
        assert_eq!(brown.heap_bytes(), 0);
    }

    /// The primary's draws, which the hierarchy's stripped-mark conditioning reads (P11.T2.a), and
    /// the companions' are at one attempt: when plan 08's `mark_attempt` moves one seam, this fails
    /// until the other moves with it.
    #[test]
    fn the_primary_and_its_companions_are_drawn_at_the_grid_attempt() {
        let galaxy = galaxy();
        let record = young_disc_record(&galaxy, 11, 1.0e8);
        let primary = BodyId::new(record.id(), 0);
        assert_eq!(
            primary_draws(&galaxy, &record),
            StarDraws::for_attempt(galaxy.seed(), primary, u32::from(GRID_ATTEMPT.get()))
        );
        assert_eq!(
            primary_draws(&galaxy, &record).stripped(),
            StarDraws::for_star(galaxy.seed(), primary).stripped(),
            "the mark `draw_hierarchy` conditions on is the primary's own"
        );
    }

    /// `age_at` is `SystemRecord::age_at`'s arithmetic.
    #[test]
    fn the_age_at_a_time_is_the_records() {
        let galaxy = galaxy();
        let record = young_disc_record(&galaxy, 3, 2.5e7);
        let star = StarModel::new(
            SolarMasses::new(1.0),
            Composition::SOLAR,
            StarDraws::median(),
            record.age_at_epoch(),
        )
        .unwrap();
        for t in [
            UniverseTime::EPOCH,
            years(-731),
            years(1_000),
            UniverseTime::EPOCH
                .checked_add(Span::new(12_345, 678_901_234).unwrap())
                .unwrap(),
        ] {
            assert_eq!(star.age_at(t), record.age_at(t));
        }
    }
}
