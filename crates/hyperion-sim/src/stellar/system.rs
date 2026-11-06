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

use std::borrow::Cow;
use std::error::Error;
use std::fmt;
use std::sync::Arc;

use crate::GENERATOR_VERSION;
use crate::coords::SystemVelocity;
use crate::events::{EventSeries, tags as event_tags};
use crate::galaxy::displaced::PlacementClass;
use crate::galaxy::fields::Component;
use crate::galaxy::placement::{
    Existence, KickConstraint, SystemKind, SystemOrigin, SystemRecord, is_helium_donor,
};
use crate::galaxy::{Galaxy, PointLy};
use crate::id::{BodyId, SystemId};
use crate::math;
use crate::orbit::KeplerElements;
use crate::rng::{Mark, ObjectKey, Seed, Stream, tags};
use crate::stellar::binary::Component as PairComponent;
use crate::stellar::binary::collapses::{OuterOrbit, PointLoss, outer_orbit_after};
use crate::stellar::binary::ia::{IaExplosionMark, marked_explosion};
use crate::stellar::binary::{
    BinaryClass, BinaryInput, BinaryState, BinaryTimeline, CarvedClass, PooledIaEvent, SegmentKind,
    SupernovaRecord, can_interact_with_tracks, carved_class, evolve_with_tracks,
};
use crate::stellar::classify::{
    ClassExtras, Classification, DredgeUp, HeliumSurface, LuminosityClass, PeculiarClass,
    SpectralType, chemical_class, classify_under, young_class,
};
use crate::stellar::draws::{StandardNormal, StarDraws, UnitUniform};
use crate::stellar::events::thermal_pulse::ThermalPulseModel;
use crate::stellar::events::{EventEffects, StarEvent, StarEvents};
#[cfg(doc)]
use crate::stellar::multiplicity::draw_hierarchy_of_composition;
use crate::stellar::multiplicity::{
    HierarchyNode, MultiplicityContext, NodeIndex, RedrawAttempt, SlotKind, StarIndex,
    SystemHierarchy, draw_hierarchy_of_primary,
};
use crate::stellar::nebula::{self, PlanetaryNebula};
use crate::stellar::photometry::{absolute_magnitude_v_under, colour_b_v_under};
use crate::stellar::remnant::collapse::RemnantDraws;
use crate::stellar::remnant::{
    BlackHole, CompactRemnant, Death, DeathKind, NatalKick, NeutronStar, PulsarState, RemnantKind,
    StandardKickLaw, mark_can_apply, stripped_by_companion, stripped_mark_applied_under,
};
use crate::stellar::rotation::{self, Activity, Magnetism, Rotation};
use crate::stellar::rules::StellarRules;
use crate::stellar::sse::{self, PhasePredicate, Track, TrackOptions};
use crate::stellar::variability::{Variability, VariabilityInputs, variability};
use crate::stellar::{Composition, ObjectKind, Phase, StarState, substellar};
use crate::tables::binary::IA_YIELD;
use crate::time::{ClockWindow, Span, UniverseTime};
use crate::units::consts::SECONDS_PER_JULIAN_YEAR;
use crate::units::{
    Dex, HeliumExcess, Kelvin, KilometresPerSecond, Magnitudes, MetresPerSecond, SolarLuminosities,
    SolarMasses, SolarRadii, Years,
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
/// - Plan 08's conditional marks (P08.T12.c) move the place and the age for the records of layers
///   D and E they draw: the place is the record's [`formation_site`](SystemRecord::formation_site),
///   the epoch position for a field system and its birth source's reference site for a displaced
///   one, and the age is the metallicity age the marks stored, where they stored one (a living
///   layer-E record's and a runaway's or a walkaway's), at which placement drew the same
///   composition (Design note 19). A dead one, retained or a displaced remnant, draws none: its
///   composition is its birth source's reference composition
///   (`SystemRecord::progenitor_composition`), at which the class table weighed its fate. Every
///   other record reads its epoch position and age, as before them.
/// - The helium excess is zero for every grid system (plan 06, design note 5); only plan 09's
///   cluster members carry one, and they bring their own [`Composition`].
/// - The function is for grid records, whose origin is
///   [`SystemOrigin::Grid`] and whose
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
    if let Some(composition) = record.progenitor_composition(galaxy) {
        return composition;
    }
    let age = record.metallicity_age().unwrap_or_else(|| age_read(record));
    metallicity_of(
        galaxy,
        record.id(),
        component_of(galaxy, record),
        &record.formation_site(galaxy),
        age,
    )
}

/// The composition of the system `id` of `component`, formed at `site` and read at `age`:
/// \[Fe/H\] drawn from the component's distribution there and then, on two words of
/// `system.metallicity`, and no helium excess ([`draw_metallicity`]'s draw, which plan 08's
/// conditional marks make at the place and age they choose, P08.T12.c).
#[must_use]
pub(crate) fn metallicity_of(
    galaxy: &Galaxy,
    id: SystemId,
    component: &Component,
    site: &PointLy,
    age: Years,
) -> Composition {
    let distribution = component.metallicity(site, age);
    let mut stream = Stream::open(galaxy.seed(), tags::SYSTEM_METALLICITY, ObjectKey::from(id));
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
/// The formulae cover 0.1–150 M☉ (`sse::MAX_INITIAL_MASS`, P06.T14), the whole range of the
/// initial mass.
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
    /// The stellar stage's rules its track and its remnant stage read (`stellar::rules`): the
    /// version's, or those of the galaxy it was generated in.
    rules: StellarRules,
}

/// How a [`StarModel`] evolves.
#[derive(Debug, Clone, PartialEq)]
enum Evolution {
    /// From 0.1 M☉: the track, to the end of the clock window or the star's death, shared with the
    /// binary engine's timeline of the star's pair (P11.T11), which reads the same track.
    Track(Arc<Track>),
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
    /// The stage of a star of initial mass `m0` and composition `comp` that died `death` leaving
    /// `remnant`, with its `draws`: the death with the companion-stripped mark applied, and the
    /// kick of the generator's law under `rules` (P06.T19), which read `star.stripped` and
    /// `star.kick.*`.
    #[must_use]
    fn new(
        death: Death,
        remnant: CompactRemnant,
        draws: &StarDraws,
        (m0, comp): (SolarMasses, &Composition),
        rules: StellarRules,
    ) -> Self {
        let law = StandardKickLaw::under(rules);
        let death = stripped_mark_applied_under(death, draws, (m0, comp), rules);
        Self {
            death,
            remnant,
            natal_kick: law.natal_kick(&death, &remnant, draws),
        }
    }
}

/// The speed, m/s, nearest `speed` (km/s, the range's edge or inside it) that `constraint` holds
/// once read back in km/s: the constraint is in km/s and a kick in m/s, whose round trip is exact
/// only to an ulp or so, so the m/s value is stepped by ulps towards the range's inside.
#[must_use]
fn metres_held_by(speed: KilometresPerSecond, constraint: &KickConstraint) -> f64 {
    let lo = constraint.speed().0.value();
    let mut metres = MetresPerSecond::from(speed).value();
    for _ in 0..64 {
        if constraint.holds(KilometresPerSecond::from(MetresPerSecond::new(metres))) {
            break;
        }
        metres = if speed.value() <= lo {
            metres.next_up()
        } else {
            metres.next_down()
        };
    }
    metres
}

/// Whether `stage` has a natal kick whose speed `constraint` holds.
fn kick_holds(stage: Option<&RemnantStage>, constraint: &KickConstraint) -> bool {
    stage
        .and_then(|stage| stage.natal_kick)
        .is_some_and(|kick| constraint.holds(KilometresPerSecond::from(kick.speed())))
}

impl RemnantStage {
    /// This stage with its natal kick's speed moved to the nearest speed `constraint` holds, its
    /// direction and mode kept: how the kick loop ends at its cap (Design note 17). A stage with
    /// no kick is returned as it is.
    #[must_use]
    fn with_kick_scaled_into(self, constraint: &KickConstraint) -> Self {
        let Some(kick) = self.natal_kick else {
            return self;
        };
        let (lo, hi) = constraint.speed();
        let speed = KilometresPerSecond::from(kick.speed()).value();
        let target = if speed < lo.value() {
            lo.value()
        } else if speed >= hi.value() {
            hi.value()
        } else {
            speed
        };
        let metres = metres_held_by(KilometresPerSecond::new(target), constraint);
        Self {
            natal_kick: Some(NatalKick::new(
                MetresPerSecond::new(metres),
                kick.direction(),
                kick.mode(),
            )),
            ..self
        }
    }
}

/// The natal kick of a star of track mass `m0` and composition `comp` that died as `fate` says,
/// with its `draws`, under `rules`: the remnant stage's ([`RemnantStage::new`]), so the kick a
/// model built on the same track gives ([`StarModel::natal_kick`]) where no kick loop redraws it,
/// bit for bit; read without building the star (P09.T37's cold cost: a listed core collapse's
/// bound from its kick).
#[must_use]
pub(crate) fn natal_kick_of(
    fate: &sse::Fate,
    draws: &StarDraws,
    (m0, comp): (SolarMasses, &Composition),
    rules: StellarRules,
) -> Option<NatalKick> {
    RemnantStage::new(fate.death, fate.remnant, draws, (m0, comp), rules).natal_kick
}

/// The remnant stage of a star whose `track` has reached its death, from its `draws`, or `None`
/// if the track has not: the death and remnant that `track` gives the remnant draws
/// (`star.remnant.*`), and the natal kick from `star.stripped` and `star.kick.*` under `rules`.
#[must_use]
fn remnant_stage(track: &Track, draws: &StarDraws, rules: StellarRules) -> Option<RemnantStage> {
    let fate = track.fate_with(RemnantDraws::of(draws))?;
    Some(RemnantStage::new(
        fate.death,
        fate.remnant,
        draws,
        (track.initial_mass(), track.composition()),
        rules,
    ))
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
        Self::new_under(
            m0,
            composition,
            draws,
            age_at_epoch,
            StellarRules::of_version(GENERATOR_VERSION),
        )
    }

    /// [`new`](Self::new) under the stellar stage's `rules` in place of the generator version's:
    /// what a star generated in a galaxy is built with, from `galaxy.rules().stellar`
    /// (`stellar::rules`).
    ///
    /// # Errors
    ///
    /// As [`new`](Self::new).
    pub(crate) fn new_under(
        m0: SolarMasses,
        composition: Composition,
        draws: StarDraws,
        age_at_epoch: Years,
        rules: StellarRules,
    ) -> Result<Self, BuildStarModelError> {
        check_star_inputs(m0, age_at_epoch)?;
        let mut model = Self {
            initial_mass: m0,
            composition,
            draws,
            age_at_epoch,
            evolution: Evolution::Cooling,
            remnant: None,
            rules,
        };
        if m0 >= sse::MIN_INITIAL_MASS {
            let end = model.age_at(ClockWindow::END).value();
            let track = Track::to_age_with(
                model.track_mass(),
                &model.composition,
                &model.draws,
                model.track_options(),
                Years::new(if end > 0.0 { end } else { 0.0 }),
            );
            model.remnant = remnant_stage(&track, &model.draws, model.rules);
            model.evolution = Evolution::Track(Arc::new(track));
        }
        Ok(model)
    }

    /// [`new_under`](Self::new_under) of a star of 0.1 M☉ or more that dies by the clock window's
    /// end, on `track`, its whole track ([`sse::track_under`] of its track mass, `composition`,
    /// `draws` and `rules`) built already: the model `new_under` builds, bit for bit, since the
    /// track that builds to an age at or past the star's death is the whole track, without
    /// building it again (plan 09, the core-collapse class's test on its candidate's track,
    /// P09.T37's bench).
    ///
    /// # Errors
    ///
    /// As [`new`](Self::new).
    ///
    /// # Panics
    ///
    /// In debug builds, if `track` is not of the star's track mass, composition and options, or
    /// does not end in a death by the clock window's end.
    pub(crate) fn on_track_under(
        m0: SolarMasses,
        composition: Composition,
        draws: StarDraws,
        age_at_epoch: Years,
        rules: StellarRules,
        track: Arc<Track>,
    ) -> Result<Self, BuildStarModelError> {
        check_star_inputs(m0, age_at_epoch)?;
        let mut model = Self {
            initial_mass: m0,
            composition,
            draws,
            age_at_epoch,
            evolution: Evolution::Cooling,
            remnant: None,
            rules,
        };
        debug_assert!(
            track
                .initial_mass()
                .value()
                .total_cmp(&model.track_mass().value())
                .is_eq()
                && track.composition() == &model.composition
                && track.options() == model.track_options(),
            "a star's model is built on its own track"
        );
        debug_assert!(
            track
                .lifetime()
                .is_some_and(|life| life.value() <= model.age_at(ClockWindow::END).value()),
            "a model is built on a whole track only where its star dies by the window's end"
        );
        model.remnant = remnant_stage(&track, &model.draws, model.rules);
        model.evolution = Evolution::Track(track);
        Ok(model)
    }

    /// A helium white dwarf of mass `m`, `age_at_epoch` since it became one, under `rules`: plan
    /// 11's naked helium star below the lightest that burns helium, a helium white dwarf from its
    /// start (`Track::helium_star`). Plan 09's hypervelocity survivors of the slow band are such
    /// donors (P09.T34.b), of 0.15–0.28 M☉, under the lightest helium star's 0.31–0.35 M☉.
    ///
    /// # Errors
    ///
    /// As [`new`](Self::new).
    pub(crate) fn helium_white_dwarf_under(
        m: SolarMasses,
        composition: Composition,
        draws: StarDraws,
        age_at_epoch: Years,
        rules: StellarRules,
    ) -> Result<Self, BuildStarModelError> {
        check_star_inputs(m, age_at_epoch)?;
        let mut model = Self {
            initial_mass: m,
            composition,
            draws,
            age_at_epoch,
            evolution: Evolution::Cooling,
            remnant: None,
            rules,
        };
        let end = model.age_at(ClockWindow::END).value();
        let track = Track::helium_star(
            m,
            &model.composition,
            &model.draws,
            Years::new(if end > 0.0 { end } else { 0.0 }),
        );
        debug_assert!(
            track.is_remnant_from_its_start(),
            "a helium white dwarf of {} M☉ is lighter than the lightest helium star that burns",
            m.value()
        );
        model.remnant = remnant_stage(&track, &model.draws, model.rules);
        model.evolution = Evolution::Track(Arc::new(track));
        Ok(model)
    }

    /// The stellar stage's rules the star was built under (`stellar::rules`): the version's, or
    /// those of the galaxy it was generated in.
    #[must_use]
    pub(crate) const fn rules(&self) -> StellarRules {
        self.rules
    }

    /// This star with its remnant stage drawn again until the natal kick holds `constraint`: the
    /// kick loop of [`primary_model`], on the attempts of `star` (in the universe of `seed`) after
    /// `first`, and the attempts it made. A star whose track has not reached its death, or that has
    /// none, is returned as it is.
    #[must_use]
    fn with_kick_in(
        mut self,
        constraint: &KickConstraint,
        (seed, star): (Seed, BodyId),
        first: u32,
    ) -> (Self, u32) {
        if !matches!(self.evolution, Evolution::Track(_))
            || self.remnant.is_none()
            || kick_holds(self.remnant.as_ref(), constraint)
        {
            return (self, 1);
        }
        let Evolution::Track(track) = std::mem::replace(&mut self.evolution, Evolution::Cooling)
        else {
            unreachable!("matched above")
        };
        let (m0, composition) = (track.initial_mass(), *track.composition());
        // Where the built track read the companion-stripped mark, the mark decided its death (the
        // band where the single star's and the companion-stripped star's electron-capture windows
        // disagree), so every attempt keeps the track's; everywhere else the mark is redrawn with
        // the remnant's and the kick's draws (ruling 93.2, amended 2026-11-01).
        let held_mark = track.companion_mark_read().map(|_| self.draws.stripped());
        let (mut kept, mut attempts) = (None, KICK_LOOP_CAP);
        for k in 1..KICK_LOOP_CAP {
            let draws = self
                .draws
                .clone()
                .with_remnant_draws_of(seed, star, first + k);
            let draws = match held_mark {
                Some(mark) => draws.with_stripped(mark),
                None => draws,
            };
            let fate = track.fate_with(RemnantDraws::of(&draws));
            let stage = fate.map(|fate| {
                RemnantStage::new(
                    fate.death,
                    fate.remnant,
                    &draws,
                    (m0, &composition),
                    self.rules,
                )
            });
            let held = kick_holds(stage.as_ref(), constraint);
            kept = Some((draws, fate, stage));
            if held {
                attempts = k + 1;
                break;
            }
        }
        let (draws, fate, stage) = kept.expect("the cap allows more than one attempt");
        self.remnant = if attempts < KICK_LOOP_CAP {
            stage
        } else {
            stage.map(|stage| stage.with_kick_scaled_into(constraint))
        };
        self.draws = draws;
        self.evolution = Evolution::Track(match fate {
            Some(fate) => Arc::new(Arc::unwrap_or_clone(track).with_fate(fate)),
            None => track,
        });
        (self, attempts)
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

    /// The star's phase and mass at `t`, or `None` if it has not formed by then: those of
    /// [`StarModel::state_at`], but with a track's mass read at its age at the epoch and the years
    /// since as two parts ([`Track::mass_at_split`]), so that it changes smoothly in time rather
    /// than in steps of one unit in the last place of a gigayear age (plan 14, P14.T45.d). It is
    /// the mass a planet's orbit widens with.
    ///
    /// # Panics
    ///
    /// As [`StarModel::state_at`].
    #[must_use]
    pub(crate) fn phase_and_mass_at(&self, t: UniverseTime) -> Option<(Phase, SolarMasses)> {
        let years = t.since_epoch().as_julian_years_f64();
        if self.age_at_epoch.value() + years <= 0.0 {
            return None;
        }
        Some(match &self.evolution {
            Evolution::Track(track) => {
                let (phase, mass) = track.mass_at_split(self.age_at_epoch.value(), years);
                (phase, SolarMasses::new(mass))
            }
            Evolution::Cooling => {
                let state = self.cooling_at(self.age_at(t));
                (state.phase(), state.mass())
            }
        })
    }

    /// The clock times strictly between `from` and `to` at which the star's mass may change its
    /// slope, ascending and without repeats: its track's segment boundaries and knot ages
    /// ([`Track::mass_breaks`]), and none for an object below 0.1 M☉, whose cooling fits keep its
    /// mass (plan 14, P14.T45.d).
    ///
    /// An age maps to the clock time `EPOCH` + (age − [`StarModel::age_at_epoch`]) ×
    /// 31,557,600 s, floored to the nanosecond ([`Span::from_seconds_f64`]), the arithmetic of the
    /// fate transform's clock deaths. [`StarModel::age_at`] of that time can differ from the age
    /// by the rounding of an age of gigayears, up to half a minute at 8–16 Gyr, so the break falls
    /// that close to the kink: a panel that meets a kink a time δ off integrates it with an error
    /// of about Δṅ δ², nothing at that scale. An age off the clock gives no break.
    #[must_use]
    pub(crate) fn mass_breaks(&self, from: UniverseTime, to: UniverseTime) -> Vec<UniverseTime> {
        let Evolution::Track(track) = &self.evolution else {
            return Vec::new();
        };
        let (from_age, to_age) = (self.age_at(from).value(), self.age_at(to).value());
        let mut breaks: Vec<UniverseTime> = track
            .mass_breaks(from_age, to_age)
            .filter_map(|age| {
                Span::from_seconds_f64((age - self.age_at_epoch.value()) * SECONDS_PER_JULIAN_YEAR)
                    .and_then(|span| UniverseTime::EPOCH.checked_add(span))
            })
            .filter(|&t| from < t && t < to)
            .collect();
        breaks.sort_unstable();
        breaks.dedup();
        breaks
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

    /// The neutron star the star leaves, with its birth spin and field (P06.T21), or `None` if it
    /// leaves none by the end of the clock window.
    #[must_use]
    pub fn neutron_star(&self) -> Option<NeutronStar> {
        self.remnant
            .filter(|stage| stage.remnant.kind() == RemnantKind::NeutronStar)
            .map(|_| NeutronStar::from_draws(&self.draws))
    }

    /// The black hole the star leaves, with its spin (P06.T22), or `None` if it leaves none by the
    /// end of the clock window.
    #[must_use]
    pub fn black_hole(&self) -> Option<BlackHole> {
        self.remnant
            .filter(|stage| stage.remnant.kind() == RemnantKind::BlackHole)
            .map(|stage| BlackHole::from_draws(stage.remnant.mass(), &self.draws))
    }

    /// The time since the star died, at `t`: `None` while it lives, for an object below 0.1 M☉,
    /// and for a star that dies after the clock window.
    #[must_use]
    pub fn remnant_age_at(&self, t: UniverseTime) -> Option<Years> {
        let stage = self.remnant?;
        let since = self.age_at(t).value() - stage.death.age().value();
        (since >= 0.0).then(|| Years::new(since))
    }

    /// The pulsar the star's neutron star is at `t` (P06.T21.b–c), or `None` if it is not a
    /// neutron star then.
    #[must_use]
    pub fn pulsar_at(&self, t: UniverseTime) -> Option<PulsarState> {
        let age = self.remnant_age_at(t)?;
        self.neutron_star().map(|ns| ns.state_at(age))
    }

    /// The pulse phase of the star's neutron star at `t`, cycles in [0, 1) (P06.T21.d), or `None`
    /// if it is not a neutron star then.
    #[must_use]
    pub fn pulsar_phase_at(&self, t: UniverseTime) -> Option<f64> {
        self.remnant_age_at(t)?;
        let stage = self.remnant?;
        let age_at_epoch = Years::new(self.age_at_epoch.value() - stage.death.age().value());
        let clock = self.neutron_star()?.pulse_clock(age_at_epoch)?;
        Some(clock.phase_at(t))
    }

    /// The age of the star's remnant at the epoch, Julian years since the star died: negative for
    /// a star that dies after the epoch, and `None` for an object below 0.1 M☉ and for a star
    /// whose track, built to the segment that holds its age at +H, does not reach its death (a
    /// death after +H within that segment still gives an age).
    ///
    /// It is the age [`StarModel::pulsar_phase_at`] hands to the pulse clock
    /// ([`NeutronStar::pulse_clock`]), in the same arithmetic, so that a reader of the clock's
    /// reference state, such as the glitches of P06.T28.b, refers to the same instant.
    #[must_use]
    pub fn remnant_age_at_epoch(&self) -> Option<Years> {
        let stage = self.remnant?;
        Some(Years::new(
            self.age_at_epoch.value() - stage.death.age().value(),
        ))
    }

    /// The planetary nebula the star lights at `t` (P06.T16.b), or `None` if it has not formed
    /// then, lights none, or is below 0.1 M☉, which never reaches the asymptotic giant branch.
    #[must_use]
    pub fn planetary_nebula_at(&self, t: UniverseTime) -> Option<PlanetaryNebula> {
        let age = self.age_at(t);
        if age.value() <= 0.0 {
            return None;
        }
        match &self.evolution {
            Evolution::Track(track) => nebula::planetary_nebula(track, age),
            Evolution::Cooling => None,
        }
    }

    /// The star's rotation at `t` (P06.T25), or `None` if it has not formed then or its phase is
    /// not one [`rotation::rotation`] models. An evolved star reads its state at the end of its
    /// main sequence from its track.
    #[must_use]
    pub fn rotation_at(&self, t: UniverseTime) -> Option<Rotation> {
        let state = self.state_at(t)?;
        self.rotation_of(&state)
    }

    /// The rotation of the star in `state`, one of its own states.
    #[must_use]
    fn rotation_of(&self, state: &StarState) -> Option<Rotation> {
        let terminal = match &self.evolution {
            Evolution::Track(track) if !matches!(state.phase(), Phase::MainSequence) => track
                .main_sequence_end()
                .filter(|end| *end <= state.age())
                .map(|end| track.state_at(end)),
            Evolution::Track(_) | Evolution::Cooling => None,
        };
        rotation::rotation_under(
            state,
            &self.composition,
            &self.draws,
            terminal.as_ref(),
            self.rules,
        )
    }

    /// How the star varies at `t` (P06.T26), as its summary gives it, or `None` if it does not vary
    /// then or has not formed.
    ///
    /// # Panics
    ///
    /// In debug builds, as [`StarModel::state_at`].
    #[must_use]
    pub fn variability_at(&self, t: UniverseTime) -> Option<Variability> {
        let state = self.state_at(t)?;
        // No variability reads an S or carbon star's class, so the pulses' series is not needed.
        let classification = classify_under(
            &state,
            &self.composition,
            &self.draws,
            self.class_extras_at(&state, t, Pulses::Unopened),
            self.rules,
        );
        let spin = self.rotation_of(&state);
        let activity = rotation::activity(&state, spin.as_ref());
        variability(&VariabilityInputs {
            state: &state,
            initial_mass: self.initial_mass,
            composition: &self.composition,
            classification: &classification,
            rotation: spin.as_ref(),
            activity: activity.as_ref(),
        })
    }

    /// What [`classify`] reads of the star's history in `state`, its state at `t`: a neutron
    /// star's class from its pulsar state (P06.T21.c), a naked helium star's surface from the
    /// mass it has lost since it became one (P06.T24.a), and under its rules' P06.T24.b a young
    /// star's class and, where `pulses` are opened, an AGB star's S or carbon class
    /// ([`peculiar_extras`]).
    ///
    /// With [`Pulses::Unopened`] an AGB star takes no S or carbon class: the pulses' series is the
    /// star's own, which [`StarModel`] cannot open (it holds no seed or ID).
    #[must_use]
    pub(crate) fn class_extras_at(
        &self,
        state: &StarState,
        t: UniverseTime,
        pulses: Pulses<'_>,
    ) -> ClassExtras {
        let extras = self.history_extras_at(state, t);
        peculiar_extras(
            extras,
            || {
                young_class(
                    state.phase(),
                    self.initial_mass,
                    self.age_at(t),
                    &self.draws,
                )
            },
            || {
                if state.phase() != Phase::ThermallyPulsingAgb {
                    return None;
                }
                let dredge_up = DredgeUp::of(self.initial_mass, &self.composition, self.track()?)?;
                let n = match pulses {
                    Pulses::Opened(streams) => {
                        let series = streams.series(event_tags::STAR_THERMAL_PULSE);
                        ThermalPulseModel::of(self, &series)?.pulse_number_at(t)?
                    }
                    Pulses::Built(model) => model?.pulse_number_at(t)?,
                    Pulses::Unopened => return None,
                };
                chemical_class(dredge_up.c_to_o(n))
            },
        )
    }

    /// [`class_extras_at`](Self::class_extras_at)'s extras of version 24: a neutron star's class
    /// and a naked helium star's surface.
    #[must_use]
    fn history_extras_at(&self, state: &StarState, t: UniverseTime) -> ClassExtras {
        match state.phase() {
            Phase::NeutronStar => self.pulsar_at(t).map_or(ClassExtras::NONE, |pulsar| {
                ClassExtras::neutron_star(pulsar.class())
            }),
            Phase::HeliumMainSequence | Phase::HeliumHertzsprungGap | Phase::HeliumGiantBranch => {
                match &self.evolution {
                    Evolution::Track(track) => track
                        .helium_star_entry_mass()
                        .map_or(ClassExtras::NONE, |entry| {
                            ClassExtras::helium_star(HeliumSurface::of(entry, state.mass()))
                        }),
                    Evolution::Cooling => ClassExtras::NONE,
                }
            }
            Phase::Protostar
            | Phase::PreMainSequence
            | Phase::MainSequence
            | Phase::HertzsprungGap
            | Phase::FirstGiantBranch
            | Phase::CoreHeliumBurning
            | Phase::EarlyAgb
            | Phase::ThermallyPulsingAgb
            | Phase::PostAgb
            | Phase::HeliumWhiteDwarf
            | Phase::CarbonOxygenWhiteDwarf
            | Phase::OxygenNeonWhiteDwarf
            | Phase::BlackHole
            | Phase::NoRemnant
            | Phase::Substellar => ClassExtras::NONE,
        }
    }

    /// The clock interval in which the star is a luminous blue variable
    /// ([`Track::window_where`] with [`PhasePredicate::Lbv`], P06.T24.a), within the part of its
    /// life its track is built for (to +H, or its death), or `None` if it is not one there or the
    /// interval lies beyond the clock's range.
    #[must_use]
    pub fn lbv_window(&self) -> Option<(UniverseTime, UniverseTime)> {
        let Evolution::Track(track) = &self.evolution else {
            return None;
        };
        let window = track.window_where(PhasePredicate::Lbv)?;
        let at = |age: Years| {
            let years = age.value() - self.age_at_epoch.value();
            Span::from_seconds_f64(years * SECONDS_PER_JULIAN_YEAR)
                .and_then(|span| UniverseTime::EPOCH.checked_add(span))
        };
        Some((at(window.start())?, at(window.end())?))
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

    /// The star's track, or `None` below 0.1 M☉: what P06.T28.f's pulse clock reads its thermally
    /// pulsing AGB from.
    #[must_use]
    pub(crate) fn track(&self) -> Option<&Track> {
        match &self.evolution {
            Evolution::Track(track) => Some(track),
            Evolution::Cooling => None,
        }
    }

    /// The star's track, shared, or `None` below 0.1 M☉: what the binary engine takes instead of
    /// building it again (P11.T11, ruling 111.5).
    #[must_use]
    pub(crate) fn shared_track(&self) -> Option<Arc<Track>> {
        match &self.evolution {
            Evolution::Track(track) => Some(Arc::clone(track)),
            Evolution::Cooling => None,
        }
    }

    /// The initial mass the track is built for: the star's own, held to the formulae's 150 M☉
    /// (P06.T14), [`track_mass_of`] its initial mass.
    #[must_use]
    fn track_mass(&self) -> SolarMasses {
        track_mass_of(self.initial_mass)
    }

    /// The options its track is built with: the generator's, under the model's stellar rules,
    /// whose helium table its track reads (P15.T7.b's hold).
    #[must_use]
    fn track_options(&self) -> TrackOptions {
        TrackOptions::default().with_rules(self.rules)
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
                    self.track_options(),
                );
                RemnantStage::new(
                    fate.death,
                    fate.remnant,
                    &self.draws,
                    (self.track_mass(), &self.composition),
                    self.rules,
                )
            })),
        }
    }
}

/// [`StarModel::new_under`]'s checks of a star's initial mass `m0` and its age at the epoch.
///
/// # Errors
///
/// As [`StarModel::new`].
fn check_star_inputs(m0: SolarMasses, age_at_epoch: Years) -> Result<(), BuildStarModelError> {
    if !(substellar::MIN_MASS.value()..=MAX_STAR_MASS.value()).contains(&m0.value()) {
        return Err(BuildStarModelError::MassOutsideRange(m0));
    }
    if !age_at_epoch.value().is_finite() {
        return Err(BuildStarModelError::AgeNotFinite(age_at_epoch));
    }
    Ok(())
}

/// A star's remnant, decided without its track: the [`remnant`](StarModel::remnant) of the star
/// [`StarModel::new_under`] builds of the same arguments, and what its
/// [`natal_kick`](StarModel::natal_kick) is judged to be, bit for bit, from builds that keep no
/// segment (P09.T10's member draws, which ask it of every attempt of a remnant class).
///
/// The model's track reaches the death if it dies by the end of the clock window, in the segment
/// holding it at the latest, and its remnant stage is then the death that track gives
/// (`sse::fate_to_age`, which stops where it stops); otherwise the remnant is the rest of its
/// life's (`sse::fate_of`) and it has no kick yet. A draw reads the kick only of an attempt whose
/// remnant is of the kind it wants, so the kick, with the companion-stripped mark it reads, is found
/// when asked.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RemnantFate {
    death: Death,
    remnant: CompactRemnant,
    /// Whether the model's track reaches the death, which gives the remnant its kick.
    kicked: bool,
    track_mass: SolarMasses,
    composition: Composition,
    draws: StarDraws,
    rules: StellarRules,
}

impl RemnantFate {
    /// The remnant of the star of initial mass `m0`, `composition` and `draws` whose age at the
    /// epoch is `age_at_epoch`, under the stellar stage's `rules`, or `None` for an object below
    /// 0.1 M☉, which leaves none.
    ///
    /// # Errors
    ///
    /// As [`StarModel::new`].
    pub(crate) fn of(
        m0: SolarMasses,
        composition: Composition,
        draws: StarDraws,
        age_at_epoch: Years,
        rules: StellarRules,
    ) -> Result<Option<Self>, BuildStarModelError> {
        check_star_inputs(m0, age_at_epoch)?;
        if m0 < sse::MIN_INITIAL_MASS {
            return Ok(None);
        }
        // `StarModel::new_under`'s track, to the star's age at the end of the clock window.
        let end = age_at_epoch.value() + ClockWindow::END.since_epoch().as_julian_years_f64();
        let track_mass = track_mass_of(m0);
        let options = TrackOptions::default().with_rules(rules);
        let reached = sse::fate_to_age(
            track_mass,
            &composition,
            &draws,
            options,
            Years::new(if end > 0.0 { end } else { 0.0 }),
        );
        let (fate, kicked) = match reached {
            Some(fate) => (fate, true),
            None => (
                sse::fate_of(track_mass, &composition, &draws, options),
                false,
            ),
        };
        Ok(Some(Self {
            death: fate.death,
            remnant: fate.remnant,
            kicked,
            track_mass,
            composition,
            draws,
            rules,
        }))
    }

    /// The remnant the star leaves or will leave: [`StarModel::remnant`].
    #[must_use]
    pub(crate) const fn remnant(&self) -> CompactRemnant {
        self.remnant
    }

    /// Whether [`StarModel::natal_kick`] is `None` or `fits` it: `Option::is_none_or` of the kick,
    /// which is `None` for a star alive at the end of the clock window and where it leaves nothing.
    ///
    /// The companion-stripped mark is read against its share only where the kicks of the death with
    /// and without it ([`RemnantStage::new`]) are judged apart, since the share's exact path costs
    /// about 3.5 ms for a star with a helium excess (P09.T10's members of the second population).
    #[must_use]
    pub(crate) fn kick_is_none_or(&self, fits: impl Fn(&NatalKick) -> bool) -> bool {
        if !self.kicked {
            return true;
        }
        let law = StandardKickLaw::under(self.rules);
        let judged = |death: &Death| {
            law.natal_kick(death, &self.remnant, &self.draws)
                .is_none_or(|kick| fits(&kick))
        };
        let unmarked = judged(&self.death);
        if !mark_can_apply(&self.death) || unmarked == judged(&stripped_by_companion(self.death)) {
            return unmarked;
        }
        let death = stripped_mark_applied_under(
            self.death,
            &self.draws,
            (self.track_mass, &self.composition),
            self.rules,
        );
        judged(&death)
    }
}

/// Where a star's own event streams are opened: its galaxy's seed and its body (plan 06, P06.T24.b
/// and T28.g). [`StarModel`] holds neither, so [`SystemStars`] keeps its galaxy's seed and hands
/// each star its body's streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct StarStreams {
    seed: Seed,
    body: BodyId,
}

impl StarStreams {
    /// The streams of `body` in the universe of `seed`.
    #[must_use]
    pub(crate) const fn new(seed: Seed, body: BodyId) -> Self {
        Self { seed, body }
    }

    /// The universe's seed.
    #[must_use]
    pub(crate) const fn seed(self) -> Seed {
        self.seed
    }

    /// The star's body.
    #[must_use]
    pub(crate) const fn body(self) -> BodyId {
        self.body
    }

    /// The star's event series of `tag`, `EventSeries::new(seed, tag, body.into())`.
    #[must_use]
    pub(crate) fn series(self, tag: crate::events::EventTag) -> EventSeries {
        EventSeries::new(self.seed, tag, self.body.into())
    }
}

/// Whether a star's own thermal pulses are opened for its class (P06.T24.b): an S or carbon star's
/// C/O reads the pulse number on the star's own series.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Pulses<'a> {
    /// Opened from these streams.
    Opened(StarStreams),
    /// Built already, by the summary's events (P06.T28.g builds each kind's model once): the
    /// star's pulses, or `None` for a star that does not pulse.
    Built(Option<&'a ThermalPulseModel<'a>>),
    /// Not opened: no S or carbon class is given.
    Unopened,
}

/// `extras` with P06.T24.b's class: the young star's class `young` gives, else the S or carbon
/// class `chemistry` gives. [`StarModel::class_extras_at`] and a paired star's summary both read
/// it.
#[must_use]
fn peculiar_extras(
    extras: ClassExtras,
    young: impl FnOnce() -> Option<PeculiarClass>,
    chemistry: impl FnOnce() -> Option<PeculiarClass>,
) -> ClassExtras {
    young()
        .or_else(chemistry)
        .map_or(extras, |class| extras.with_peculiar(class))
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
/// A remnant's detail (a neutron star's pulsar state, P06.T21; a black hole's spin, T22), a
/// living star's rotation, magnetism and activity (T25), its variability (T26.a–c) and the
/// planetary nebula it lights (T16.b) are here. Plan 11's binary class (P11.T5) is the class of
/// the pair the binary engine ran the star in (P11.T11). From version 26 its active events and
/// the light they make (P06.T28.g) are here too, beside the track's state, which they never
/// change.
#[derive(Debug, Clone, PartialEq)]
pub struct StarSummary {
    body: BodyId,
    state: StarState,
    kind: ObjectKind,
    classification: Classification,
    absolute_magnitude_v: Option<Magnitudes>,
    colour_b_v: Option<Magnitudes>,
    remnant: Option<CompactRemnant>,
    remnant_detail: Option<RemnantDetail>,
    rotation: Option<Rotation>,
    magnetism: Option<Magnetism>,
    activity: Option<Activity>,
    variability: Option<Variability>,
    planetary_nebula: Option<PlanetaryNebula>,
    death_in_window: Option<(UniverseTime, DeathKind)>,
    binary_class: BinaryClass,
    active_events: Option<Vec<StarEvent>>,
    event_effects: Option<EventEffects>,
}

/// What a remnant's summary says beyond its kind and mass: a neutron star's pulsar state
/// (P06.T21) or a black hole's spin (P06.T22). A white dwarf's type is its classification.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RemnantDetail {
    /// A neutron star as a pulsar at the summary's time.
    NeutronStar(PulsarState),
    /// A black hole.
    BlackHole(BlackHole),
}

impl StarSummary {
    /// Which body of its system the star is: its body index is its
    /// [`StarIndex`] in the system's hierarchy, 0 for the
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

    /// A neutron star's pulsar state or a black hole's spin, once the star has left one.
    #[must_use]
    pub const fn remnant_detail(&self) -> Option<RemnantDetail> {
        self.remnant_detail
    }

    /// The star's rotation (P06.T25), where it is modelled: every living star from the protostar
    /// to the asymptotic giant branch. A neutron star's spin is its pulsar's.
    #[must_use]
    pub const fn rotation(&self) -> Option<Rotation> {
        self.rotation
    }

    /// The star's surface field (P06.T25): a fossil field, or a cool dwarf's dynamo field.
    #[must_use]
    pub const fn magnetism(&self) -> Option<Magnetism> {
        self.magnetism
    }

    /// A cool dwarf's magnetic activity (P06.T25), from its Rossby number.
    #[must_use]
    pub const fn activity(&self) -> Option<Activity> {
        self.activity
    }

    /// How the star's light varies (P06.T26.a–c), or `None` if it does not.
    #[must_use]
    pub const fn variability(&self) -> Option<Variability> {
        self.variability
    }

    /// The planetary nebula the star lights (P06.T16.b), or `None` if it lights none.
    #[must_use]
    pub const fn planetary_nebula(&self) -> Option<PlanetaryNebula> {
        self.planetary_nebula
    }

    /// The star's death, its clock time and kind, if it falls inside the clock window [−H, +H]
    /// (the client's `DIES IN 312 yr`).
    ///
    /// It is the star's own death ([`StarModel::death`]) where the star is its single-star self at
    /// the time summarised, and `None` for a star whose pair has changed it (P11.T11), whose death
    /// the binary engine decides and records without its kind, unless its pair explodes it at the
    /// model's own death age (a massive primary, whose death design note 16 pins).
    #[must_use]
    pub const fn death_in_window(&self) -> Option<(UniverseTime, DeathKind)> {
        self.death_in_window
    }

    /// The class of the pair the binary engine ran the star in, at the time summarised (plan 11,
    /// P11.T5 and T11): [`BinaryClass::None`] for a star in no such pair, a brown dwarf, a star
    /// merged into its companion, and a star its own death left nothing of.
    #[must_use]
    pub const fn binary_class(&self) -> BinaryClass {
        self.binary_class
    }

    /// The star's events running at the time summarised, by onset, then kind, then ID
    /// ([`StarEvents::at`], P06.T28.g): empty where none runs, and `None` where they are not
    /// composed, for a star its pair has changed, whose events would be its own single-star
    /// model's (the lane's lean, 2026-10-31).
    #[must_use]
    pub fn active_events(&self) -> Option<&[StarEvent]> {
        self.active_events.as_deref()
    }

    /// The light and spin the star's active events make at the time summarised
    /// ([`EventEffects`], P06.T28.g), or `None` where none changes them or they are not composed.
    ///
    /// [`state`](Self::state) stays the track's (design note 14), as does the brief.
    #[must_use]
    pub const fn event_effects(&self) -> Option<EventEffects> {
        self.event_effects
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
    /// The brief of a primary in `state`, of `composition` and `draws`, with the `extras` its
    /// history gives (a neutron star's class), in a system of `star_count` stars, classified under
    /// the stellar stage's `rules`: its class, its kind, log L and `T_eff`.
    /// [`SystemStars::brief_at`] and the range brief ([`crate::stellar::brief`]) both make it here,
    /// so that a brief is the same function of the state whichever built it.
    #[must_use]
    pub(crate) fn of(
        state: &StarState,
        composition: &Composition,
        draws: &StarDraws,
        extras: ClassExtras,
        (star_count, rules): (u8, StellarRules),
    ) -> Self {
        let class = classify_under(state, composition, draws, extras, rules);
        let luminosity = state.luminosity().value();
        Self {
            kind: object_kind(state, &class, composition),
            class,
            log_luminosity: (luminosity > 0.0).then(|| Dex::new(math::log10(luminosity))),
            effective_temperature: state.effective_temperature(),
            star_count,
        }
    }

    /// The same brief in a system of `star_count` stars: the range brief's paired route, which
    /// reads the kept system's primary and reports the first attempt's count (P11.T11.a; plan 11's
    /// Risks, "Deviations in P11.T7, as built").
    #[must_use]
    pub(crate) const fn with_star_count(self, star_count: u8) -> Self {
        Self { star_count, ..self }
    }

    /// How many stars the system has, the primary included, with a bound brown dwarf (plan 11,
    /// P11.T2.d): 1 to 2 + [`MAX_COMPANIONS`](crate::stellar::multiplicity::MAX_COMPANIONS).
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
/// - a star classified as a Wolf-Rayet star (P06.T24.a: hot, above the luminosity floor of
///   10⁴·⁹ L☉ × (Z ÷ 0.02)^−0.4, and a naked helium star or a nearly stripped or near-Eddington
///   hydrogen-rich one, `WNh`) is a Wolf-Rayet star, a hot subdwarf (a naked helium star below
///   the floor, from 20,000 K, of up to 1.7 M☉) a hot subdwarf, and a stripped helium star (one
///   heavier than that, P06.T24.c) a stripped helium star; any other naked helium star goes by its
///   luminosity class;
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
        _ if matches!(classification.spectral_type(), SpectralType::WolfRayet(_)) => {
            ObjectKind::WolfRayet
        }
        _ if classification.peculiar_class() == Some(PeculiarClass::HotSubdwarf) => {
            ObjectKind::HotSubdwarf
        }
        _ if classification.peculiar_class() == Some(PeculiarClass::StrippedHeliumStar) => {
            ObjectKind::StrippedHeliumStar
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
        | Phase::HeliumMainSequence
        | Phase::HeliumHertzsprungGap
        | Phase::HeliumGiantBranch
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

/// The redraw attempt a grid system's companions are first drawn at: [`RedrawAttempt::FIRST`].
///
/// **A named seam** (plan 11, P11.T2.c and T7). [`SystemStars::generate`] redraws a grid system
/// one of whose pairs falls into a carved catalogue class ([`carved_class`], P11.T7; exploding as
/// a Type Ia is P11.T6's) and reads the later attempt it keeps for the hierarchy and for each
/// companion's own draws alike ([`StarDraws::for_attempt`]), since both are blocks of
/// [`ATTEMPT_WORDS`](crate::stellar::draws::ATTEMPT_WORDS) words
/// ([`SystemStars::attempt`]). The range brief's count-only draw
/// ([`draw_star_count`](crate::stellar::multiplicity::draw_star_count)), which runs no engine,
/// reads this one, so for the few redrawn systems, about 10⁻⁴, its count is attempt 0's (plan 11's
/// Risks, "Deviations in P11.T7, as built").
pub(crate) const GRID_ATTEMPT: RedrawAttempt = RedrawAttempt::FIRST;

/// What a system shows at one time ([`SystemStars::shown_at`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Shown {
    /// Every star of the drawn hierarchy.
    Drawn,
    /// The primary and star 1 on the drawn pair `node`, which is all a supernova left of the
    /// system by unbinding its other pair.
    Pair(NodeIndex),
    /// The part of the drawn hierarchy under `node`, of `stars` stars, three or more, the primary
    /// and the stars that follow it: all a supernova left of the system by unbinding an orbit
    /// above it (P11.T10's fix, [`SystemHierarchy::part`]).
    Part {
        /// The part's node in the drawn hierarchy, on the primary's inner chain.
        node: NodeIndex,
        /// Its stars.
        stars: u8,
    },
    /// The primary alone.
    Alone,
}

impl Shown {
    /// Whether star `index` is shown.
    #[must_use]
    pub(crate) const fn holds(self, index: u8) -> bool {
        match self {
            Self::Drawn => true,
            Self::Pair(_) => index <= 1,
            Self::Part { stars, .. } => index < stars,
            Self::Alone => index == 0,
        }
    }
}

/// The summary at `t` of the single stars `stars`, each a slot of `hierarchy` by body index, of a
/// synthetic host in the universe of `seed` whose existence then is `existence`: what
/// [`SystemStars::summary_at`] gives a system of stars no pair the binary engine ran, for the
/// planet viewer's star (plan R15), which a [`SystemContext`](crate::planetary::SystemContext)'s
/// builder makes and no [`SystemStars`] holds.
///
/// # Panics
///
/// If `stars` is empty, and in debug builds as [`StarModel::state_at`].
#[must_use]
pub(crate) fn synthetic_summary_at(
    seed: Seed,
    stars: &[StarModel],
    hierarchy: &SystemHierarchy,
    existence: Existence,
    t: UniverseTime,
) -> SystemSummary {
    let existence = SystemExistence::from(existence);
    let composition = *stars
        .first()
        .expect("a synthetic host has a star")
        .composition();
    let (stars, hierarchy) = match existence {
        SystemExistence::NotYetBorn => (Vec::new(), None),
        SystemExistence::Exists => (
            stars
                .iter()
                .zip(hierarchy.stars())
                .filter_map(|(star, slot)| {
                    star_summary_with(star, StarStreams::new(seed, slot.body()), t, None)
                })
                .collect(),
            Some(hierarchy.clone()),
        ),
    };
    SystemSummary {
        time: t,
        existence,
        composition,
        stars,
        hierarchy,
    }
}

/// The first supernova of `timeline` that unbinds the pair, if the pair had one by the age it was
/// run to: a record of an unbound pair whose next segment is [`SegmentKind::Disrupted`] by its
/// star (BSE appendix A1). A merged product's collapse, unbound for want of a companion, is not
/// one, nor is a white dwarf's birth that sheds enough mass to unbind a wide pair (P11.T4.j): plan
/// 08 places only the stars a core collapse releases (`runaway::RELEASING_MIN_MASS`).
#[must_use]
pub(crate) fn disrupting_supernova(timeline: &BinaryTimeline) -> Option<&SupernovaRecord> {
    let segments = timeline.segments();
    timeline.supernovae().iter().find(|sn| {
        // The explosion opens its segment at its own age, the last segment starting at or before it.
        let at = segments
            .partition_point(|s| s.start() <= sn.age())
            .saturating_sub(1);
        !sn.bound() && segments[at].kind() == SegmentKind::Disrupted { by: sn.component() }
    })
}

/// One pair of two stars of a system that the binary engine ran (plan 11, P11.T11): its node in
/// the hierarchy, its two stars, inner member first, and its timeline from zero age to the
/// system's age at +H ([`evolve`](crate::stellar::binary::evolve)).
#[derive(Debug, Clone, PartialEq)]
pub struct PairTimeline {
    node: NodeIndex,
    stars: [StarIndex; 2],
    timeline: BinaryTimeline,
}

impl PairTimeline {
    /// The pair's node in the system's hierarchy.
    #[must_use]
    pub const fn node(&self) -> NodeIndex {
        self.node
    }

    /// Its two stars: the inner member, the engine's primary, then the outer member.
    #[must_use]
    pub const fn stars(&self) -> [StarIndex; 2] {
        self.stars
    }

    /// Its timeline.
    #[must_use]
    pub const fn timeline(&self) -> &BinaryTimeline {
        &self.timeline
    }

    /// Which of the engine's components `star` is, if it is one of the pair's.
    #[must_use]
    fn component_of(&self, star: StarIndex) -> Option<PairComponent> {
        self.stars
            .iter()
            .position(|&s| s == star)
            .map(PairComponent::of_index)
    }

    /// The age at `t` at which the timeline is read, its inner member's age then from `stars`, the
    /// system's models by body index, or `None` if the system is not born then or `t` lies beyond
    /// the ages the timeline was run to.
    #[must_use]
    pub(crate) fn age_at(&self, stars: &[StarModel], t: UniverseTime) -> Option<Years> {
        let age = stars[usize::from(self.stars[0].get())].age_at(t);
        (age.value() > 0.0 && age <= self.timeline.until()).then_some(age)
    }
}

/// The pair among `pairs` that holds star `index`, with the engine's component the star is there,
/// if the engine ran it in one: the lookup [`SystemStars`]' paired states take, which plan 14's
/// context shares (P14.T1.e).
#[must_use]
pub(crate) fn pair_of(pairs: &[PairTimeline], index: u8) -> Option<(&PairTimeline, PairComponent)> {
    pairs.iter().find_map(|pair| {
        let star = pair.stars.iter().copied().find(|s| s.get() == index)?;
        pair.component_of(star).map(|c| (pair, c))
    })
}

/// A pair's state at one clock time (plan 11, P11.T11): its node, its orbit then, and its class.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PairState {
    node: NodeIndex,
    orbit: Option<KeplerElements>,
    class: BinaryClass,
}

impl PairState {
    /// The pair's node in the system's hierarchy.
    #[must_use]
    pub const fn node(&self) -> NodeIndex {
        self.node
    }

    /// The pair's relative orbit then: the binary engine's for a pair it ran, `None` once that
    /// pair has merged or been unbound, and the drawn orbit for every other pair, whose members
    /// the engine does not touch.
    #[must_use]
    pub const fn orbit(&self) -> Option<&KeplerElements> {
        self.orbit.as_ref()
    }

    /// The pair's binary class then ([`BinaryTimeline::class_at`]): [`BinaryClass::None`] for a
    /// pair the engine did not run.
    #[must_use]
    pub const fn class(&self) -> BinaryClass {
        self.class
    }
}

/// A system's state at one clock time (plan 11, P11.T11; [`SystemStars::state_at`]): every star's,
/// every pair's, and the whole system's luminosity and mass.
#[derive(Debug, Clone, PartialEq)]
pub struct SystemState {
    time: UniverseTime,
    stars: Vec<StarState>,
    pairs: Vec<PairState>,
    luminosity: SolarLuminosities,
    mass: SolarMasses,
}

impl SystemState {
    /// The clock time.
    #[must_use]
    pub const fn time(&self) -> UniverseTime {
        self.time
    }

    /// Every star's state, by body index, primary first: from its pair's timeline where the
    /// engine ran one, and otherwise its own ([`StarModel::state_at`]). A star merged into its
    /// companion is plan 06's `NoRemnant`.
    #[must_use]
    pub fn stars(&self) -> &[StarState] {
        &self.stars
    }

    /// Every pair's state, depth first.
    #[must_use]
    pub fn pairs(&self) -> &[PairState] {
        &self.pairs
    }

    /// The combined luminosity of the stars, L☉, summed in body order.
    #[must_use]
    pub const fn luminosity(&self) -> SolarLuminosities {
        self.luminosity
    }

    /// The system's mass, M☉: its stars' and remnants' masses then, summed in body order.
    #[must_use]
    pub const fn mass(&self) -> SolarMasses {
        self.mass
    }
}

/// A grid system's stars from its record (plan 06, P06.T29.b, with plan 11's P11.T2.c): its
/// primary, body 0 of the system, its companions, and the hierarchy of orbits that holds them.
///
/// [`SystemStars::generate`] takes three steps for the primary, because plan 08's P08.T12.c
/// repeats only the last: the primary's draws ([`StarDraws::for_star`] of body 0; plan 08 makes it
/// `for_attempt` of the record's mark attempt), its track, and the remnant stage on that track
/// ([`StarModel`]). With no kick constraint, which is every record of this generator version, the
/// last runs once. The companions and their orbits are plan 11's
/// [`draw_hierarchy`](crate::stellar::multiplicity::draw_hierarchy), and each companion is a
/// [`StarModel`] of its slot's initial mass, on its own body's draws, with the system's
/// composition and age. The primary never depends on the companions: its model is plan 06's, bit
/// for bit, whatever the hierarchy.
///
/// Every pair of two stars that can interact by +H or holds a remnant by then is run through plan
/// 11's binary engine once, from zero age to the system's age at +H (P11.T11,
/// [`evolve`](crate::stellar::binary::evolve)), on the tracks the star models hold (ruling
/// 111.5), and its timeline is kept ([`SystemStars::pairs`]). Any other pair is two single stars
/// on their orbit, in no class, and is not run. Each [`StarModel`] stays the star's single-star
/// self, and [`SystemStars::state_at`], [`SystemStars::summary_at`] and [`SystemStars::brief_at`]
/// read a paired star from its pair's timeline. A pair with a member that is itself a pair, and a
/// brown dwarf, never enter the engine.
///
/// A grid system one of whose pairs falls into a carved catalogue class at any age of the source
/// horizon ([`carved_class`], P11.T7) is redrawn, on the next [`RedrawAttempt`] of the hierarchy
/// and of the companions' draws, so that the grid holds exactly the binaries of its marks
/// conditioned on not being in a class (plan 11, design notes 8 and 9). The primary is never
/// redrawn.
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
    pairs: Vec<PairTimeline>,
    /// The outer orbits its pairs' supernovae unbind (P11.T10's fix), by age.
    releases: Vec<OuterRelease>,
    attempt: RedrawAttempt,
    /// The seed of the galaxy the stars were generated in, from which each star's own event
    /// streams are opened (P06.T24.b's pulses, T28.g's events), so that the summary and the brief
    /// need no galaxy (the lane's lean, 2026-10-31).
    seed: Seed,
}

impl SystemStars {
    /// The stars of the grid system `record` in `galaxy`, with the model's own multiplicity:
    /// [`SystemStars::generate_in`] with [`grid_multiplicity`]'s context, which is
    /// [`MultiplicityContext::Free`] for a star and `ForcedSingle` for a free-floating brown dwarf.
    ///
    /// A brown dwarf (plan 13, P13.T5.a) takes the stellar stage as a star does: its metallicity
    /// draw, P06.T13's cooling fits at its age plus the clock time ([`substellar::cooling`]), and
    /// its classification, with no companion.
    ///
    /// # Panics
    ///
    /// - As [`draw_metallicity`] does, for a record of another galaxy.
    /// - For a rogue planet, whose mass is below the fits' 0.01 M☉: its state is plan 14's (plan
    ///   13, P13.T5.d), and it has no stars.
    #[must_use]
    pub fn generate(galaxy: &Galaxy, record: &SystemRecord) -> Self {
        Self::generate_in(galaxy, record, grid_multiplicity(record))
    }

    /// The stars of the system `record` in `galaxy` under the multiplicity context `ctx` (plan 11,
    /// P11.T2.c): its [`draw_metallicity`], the hierarchy
    /// [`draw_hierarchy`](crate::stellar::multiplicity::draw_hierarchy) draws for `ctx`, the
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
        Self::generate_with(galaxy, record, &draw_metallicity(galaxy, record), ctx)
    }

    /// The stars of `record` with the composition `composition`, under the multiplicity context
    /// `ctx`: [`generate_in`](Self::generate_in) without its metallicity draw, which reads the
    /// record's density component. A feature member has none and brings its cluster's
    /// composition instead (plan 09, P09.T10); its primary is read at the attempt its origin
    /// carries, and its hierarchy judges the primary's stripped mark at `composition`
    /// ([`draw_hierarchy_of_composition`]). For a grid record with its own drawn composition it is
    /// `generate_in`, bit for bit.
    ///
    /// # Panics
    ///
    /// If the record's primary is not of 0.01–150 M☉ (a star, or a brown dwarf from 0.0124 M☉) or
    /// its age is not finite.
    #[must_use]
    pub fn generate_with(
        galaxy: &Galaxy,
        record: &SystemRecord,
        composition: &Composition,
        ctx: MultiplicityContext,
    ) -> Self {
        let (primary, _) = primary_model(galaxy, record, *composition);
        Self::generate_of_primary(galaxy, record, primary, ctx)
    }

    /// [`generate_with`](Self::generate_with) from the primary's model, `primary`, which must be
    /// [`primary_model`]'s at the system's composition: for a caller that has built it already,
    /// the range brief of a record whose class constrains its kick (P08.T12.c), so that its track
    /// and kick loop are not built twice.
    #[must_use]
    pub(crate) fn generate_of_primary(
        galaxy: &Galaxy,
        record: &SystemRecord,
        primary: StarModel,
        ctx: MultiplicityContext,
    ) -> Self {
        let carves = crate::stellar::binary::carve::redraws(galaxy, record, ctx);
        let mut attempt = GRID_ATTEMPT;
        loop {
            let drawn = Self::at_attempt(galaxy, record, &primary, ctx, attempt);
            if !carves || (drawn.carved_pair().is_none() && drawn.exploded_pair().is_none()) {
                // The carve-out takes no recent Type Ia from the grid, so a kept grid system has
                // none at or before +H (plan 09, P09.T35's requirement of plan 11's P11.T6). The
                // hook is the redraw's own test, so the branch's condition holds it; it guards the
                // requirement against an edit of either.
                debug_assert!(
                    !carves || !crate::galaxy::displaced::site::keeps_a_recent_type_ia(&drawn),
                    "a grid system keeps a Type Ia at or before +H, which plan 11's binaries \
                     redraw: {:?}",
                    record.id()
                );
                return drawn;
            }
            let Some(next) = attempt.next() else {
                return Self::after_last_attempt(galaxy.seed(), record, primary, attempt);
            };
            attempt = next;
        }
    }

    /// The system of `record` at `composition` under `ctx` at redraw attempt `attempt` alone, with
    /// no redraw after it: a marked feature member's, whose conditional draw chose the attempt
    /// (P11.T8.f).
    ///
    /// # Panics
    ///
    /// As [`generate_with`](Self::generate_with).
    #[must_use]
    pub(crate) fn generate_at(
        galaxy: &Galaxy,
        record: &SystemRecord,
        composition: &Composition,
        ctx: MultiplicityContext,
        attempt: RedrawAttempt,
    ) -> Self {
        let (primary, _) = primary_model(galaxy, record, *composition);
        Self::at_attempt(galaxy, record, &primary, ctx, attempt)
    }

    /// The system at redraw attempt `attempt`: `primary`, the hierarchy
    /// [`draw_hierarchy_of_composition`] draws then at the primary's composition, each companion's
    /// model on its own body's draws of that attempt, and each pair of two stars run through the
    /// engine.
    #[must_use]
    pub(crate) fn at_attempt(
        galaxy: &Galaxy,
        record: &SystemRecord,
        primary: &StarModel,
        ctx: MultiplicityContext,
        attempt: RedrawAttempt,
    ) -> Self {
        // `exploded_pair` reads P11.T6's arm from the primary's rules, so they are the pairs'.
        debug_assert_eq!(
            primary.rules(),
            galaxy.rules().stellar,
            "the primary is built under the galaxy's stellar rules"
        );
        let (hierarchy, stars) = models_at(galaxy, record, primary, ctx, attempt);
        let pairs = run_pairs(&hierarchy, &stars, galaxy.rules().stellar);
        let releases = outer_releases(&hierarchy, &stars, &pairs);
        Self {
            record: *record,
            hierarchy,
            stars,
            pairs,
            releases,
            attempt,
            seed: galaxy.seed(),
        }
    }

    /// What the grid keeps of a system whose [`MAX_REDRAWS`](crate::stellar::multiplicity::MAX_REDRAWS)
    /// attempts all carve: the primary alone, at the last attempt, in the universe of `seed`.
    ///
    /// Design note 9 keeps the last hierarchy with its innermost period moved out of the
    /// interacting range; a single star is never in a binary class, so the grid's half of the
    /// complementarity holds exactly here too. At a carve probability of 10⁻⁴ a system would need
    /// eight carved attempts in a row, which none of the galaxy's 10¹¹ does in expectation (plan
    /// 11's Risks, "Deviations in P11.T7, as built").
    #[must_use]
    fn after_last_attempt(
        seed: Seed,
        record: &SystemRecord,
        primary: StarModel,
        attempt: RedrawAttempt,
    ) -> Self {
        Self {
            record: *record,
            hierarchy: SystemHierarchy::single(
                record.id(),
                record.primary_initial_mass(),
                SlotKind::Star,
            ),
            stars: vec![primary],
            pairs: Vec::new(),
            releases: Vec::new(),
            attempt,
            seed,
        }
    }

    /// The stars of a catalogue entry of a binary class (plan 11, P11.T8.a) in the universe of
    /// `seed`: `record`'s system of the two stars of `hierarchy`, as `stars` model them on their
    /// own, and the pair's `timeline`, which the class builds from its marks
    /// ([`BinaryTimeline::from_marks`](crate::stellar::binary::BinaryTimeline::from_marks)). The
    /// timeline gives both stars' states wherever the system reads them, as an evolved timeline
    /// does a grid pair's; the entry's history is its models'.
    ///
    /// # Panics
    ///
    /// If `hierarchy` is not one pair of two stars, or `stars` does not hold a model for each.
    #[must_use]
    pub(crate) fn of_marked_pair(
        seed: Seed,
        record: SystemRecord,
        hierarchy: SystemHierarchy,
        stars: Vec<StarModel>,
        timeline: BinaryTimeline,
    ) -> Self {
        assert_eq!(hierarchy.star_count(), 2, "a marked pair is two stars");
        assert_eq!(stars.len(), 2, "a marked pair's stars each have a model");
        let (node, _) = hierarchy
            .pairs()
            .next()
            .expect("two stars of a hierarchy are a pair");
        let second = StarIndex::from_body(1).expect("body 1 is a star's");
        Self {
            record,
            hierarchy,
            stars,
            pairs: vec![PairTimeline {
                node,
                stars: [StarIndex::PRIMARY, second],
                timeline,
            }],
            // A pair of two stars has no orbit above it.
            releases: Vec::new(),
            attempt: GRID_ATTEMPT,
            seed,
        }
    }

    /// The first pair, depth first, whose timeline falls into a carved class at any age of the
    /// source horizon, with that class.
    #[must_use]
    pub fn carved_pair(&self) -> Option<(NodeIndex, CarvedClass)> {
        let age = self.record.age_at_epoch();
        self.pairs
            .iter()
            .find_map(|pair| carved_class(&pair.timeline, age).map(|class| (pair.node, class)))
    }

    /// The first pair, depth first, whose pooled Type Ia candidate its explosion mark explodes,
    /// with that candidate (plan 11, P11.T6): an explosion at or before +H, the age the pairs are
    /// run to. The mark is the pair's at the system's attempt ([`IaExplosionMark`], read from the
    /// pair's key), against plan 15's fitted [`IA_YIELD`]. `None` for a system with no pooled
    /// candidate.
    ///
    /// A grid system redraws an attempt for which it is `Some`, as for a carved pair
    /// ([`carved_pair`](Self::carved_pair)), so that no grid system keeps a Type Ia by +H: plan
    /// 09's class holds every one (design notes 8 and 9; P09.T35's requirement).
    ///
    /// # Panics
    ///
    /// Never: each pair the engine ran is a pair node of the hierarchy, which has a key.
    #[must_use]
    pub fn exploded_pair(&self) -> Option<(NodeIndex, PooledIaEvent)> {
        self.pairs.iter().find_map(|pair| {
            // The mark is read only for a pair with a candidate: most pairs have none.
            pair.timeline.pooled_ia()?;
            let key = self
                .hierarchy
                .pair_key(pair.node)
                .expect("a run pair's node is a pair of the hierarchy, which has a key");
            let mark = IaExplosionMark::of(self.seed, key, self.attempt);
            marked_explosion(&pair.timeline, mark, &IA_YIELD).map(|event| (pair.node, event))
        })
    }

    /// The redraw attempt the system's companions were drawn at (plan 11, design note 9): the
    /// first for nearly every system, a later one for a grid system whose earlier attempts fell
    /// into a carved class or exploded as a Type Ia
    /// ([`exploded_pair`](Self::exploded_pair)).
    #[must_use]
    pub const fn attempt(&self) -> RedrawAttempt {
        self.attempt
    }

    /// The pairs of two stars the binary engine ran, depth first (P11.T11).
    #[must_use]
    pub fn pairs(&self) -> &[PairTimeline] {
        &self.pairs
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

    /// How many stars the system has, the primary included, with a bound brown dwarf (plan 11,
    /// P11.T2.d): 1 to 2 + [`MAX_COMPANIONS`](crate::stellar::multiplicity::MAX_COMPANIONS).
    #[must_use]
    pub fn star_count(&self) -> u8 {
        self.hierarchy.star_count()
    }

    /// The event streams of the system's star `body`.
    #[must_use]
    pub(crate) const fn streams(&self, body: BodyId) -> StarStreams {
        StarStreams::new(self.seed, body)
    }

    /// The bytes the system's stars own on the heap, beyond `size_of::<SystemStars>()`: each
    /// star's model and track, the hierarchy's lists and each pair's timeline, by capacity. A
    /// track the star's pair shares is counted once, with the star.
    ///
    /// It is what the server charges a cached system against its byte budget (plan 06, P06.T34;
    /// plan 04, design note 23), as [`Galaxy::heap_bytes`] is for a galaxy. A system of one living
    /// dwarf owns a few kilobytes; nothing generated reads it.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        let stars = self.stars.iter().fold(
            self.stars.capacity() * size_of::<StarModel>() + self.hierarchy.heap_bytes(),
            |bytes, star| bytes + star.heap_bytes(),
        );
        self.pairs.iter().fold(
            stars
                + self.pairs.capacity() * size_of::<PairTimeline>()
                + self.releases.capacity() * size_of::<OuterRelease>(),
            |bytes, pair| bytes + pair.timeline.heap_bytes(),
        )
    }

    /// The system at `t`: whether it exists ([`SystemRecord::existence_at`]), its composition,
    /// each star's [`StarSummary`] at its age then ([`SystemRecord::age_at`]), and its hierarchy;
    /// no star and no hierarchy before the system is born.
    ///
    /// A star of a pair the engine ran is summarised from its pair's state then, with the pair's
    /// class ([`StarSummary::binary_class`]); the hierarchy is the drawn one, and
    /// [`SystemStars::state_at`] gives the pairs' orbits then. From version 27 a system that a
    /// supernova has unbound holds only the stars and the hierarchy it still holds then
    /// (`shown_at`; plan 11, P11.T10).
    ///
    /// From version 26 each star's summary holds its events running at `t` and the light they
    /// make (P06.T28.g; [`StarSummary::active_events`]). The summary depends on the system and `t`
    /// alone: it takes no list of past events, and is the same whatever was asked before.
    ///
    /// # Panics
    ///
    /// In debug builds, as [`StarModel::state_at`], for a `t` after the clock window's end.
    #[must_use]
    pub fn summary_at(&self, t: UniverseTime) -> SystemSummary {
        let existence = SystemExistence::from(self.record.existence_at(t));
        let (stars, hierarchy) = match existence {
            SystemExistence::NotYetBorn => (Vec::new(), None),
            SystemExistence::Exists => {
                let shown = self.shown_at(t);
                (
                    (0_u8..)
                        .zip(self.stars.iter().zip(self.hierarchy.stars()))
                        .filter(|&(index, _)| shown.holds(index))
                        .filter_map(|(index, (star, slot))| {
                            let paired = self.paired_state(index, t);
                            star_summary_with(star, self.streams(slot.body()), t, paired)
                        })
                        .collect(),
                    Some(self.shown_hierarchy(shown).into_owned()),
                )
            }
        };
        SystemSummary {
            time: t,
            existence,
            composition: *self.primary().composition(),
            stars,
            hierarchy,
        }
    }

    /// The system's brief at `t`, for a range query's row, or `None` before the system is born or
    /// its primary forms: the kind, class, luminosity and temperature of the primary that
    /// [`SystemStars::summary_at`] has then, from its pair's state where a pair the engine ran has
    /// changed it (P11.T11), with no photometry and no death, and the kept attempt's star count
    /// ([`SystemStars::star_count`]).
    ///
    /// It is the one brief of a system (plan 11, P11.T11.a): a row and the summary asked of one
    /// system at one time read one state, so a primary its companion has stripped is the stripped
    /// star in both, never its single-star track's supergiant. Observed mode reports it at the
    /// emitted time (plan 12; [`crate::observe::observe_hit`]), and the range brief
    /// ([`crate::stellar::brief`]) gives it at the query's time, the primary's single-star model
    /// where that is certainly the primary's state and this brief otherwise, with the first
    /// attempt's star count (plan 11's Risks, "Deviations in P11.T7, as built"). From version 27
    /// the count is that of the stars the system shows then (`shown_at`).
    ///
    /// # Panics
    ///
    /// In debug builds, as [`StarModel::state_at`].
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::Seed;
    /// use hyperion_sim::galaxy::Galaxy;
    /// use hyperion_sim::galaxy::placement::{CellKey, generate_cell};
    /// use hyperion_sim::id::Layer;
    /// use hyperion_sim::stellar::system::SystemStars;
    /// use hyperion_sim::time::UniverseTime;
    ///
    /// let galaxy = Galaxy::new(Seed::new(11));
    /// let mut cell = Vec::new();
    /// generate_cell(&galaxy, CellKey::new(Layer::C, [0, 812, 0])?, &mut cell);
    /// for record in cell.iter().take(10) {
    ///     let stars = SystemStars::generate(&galaxy, record);
    ///     let brief = stars.brief_at(UniverseTime::EPOCH).ok_or("formed")?;
    ///     let summary = stars.summary_at(UniverseTime::EPOCH);
    ///     let primary = summary.stars().first().ok_or("a primary")?;
    ///     // The brief is the summary's primary, whatever its pair has made of it.
    ///     assert_eq!((brief.kind(), brief.class()), (primary.kind(), primary.classification()));
    ///     assert_eq!(brief.star_count(), stars.star_count());
    /// }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn brief_at(&self, t: UniverseTime) -> Option<StellarBrief> {
        if SystemExistence::from(self.record.existence_at(t)) == SystemExistence::NotYetBorn {
            return None;
        }
        let primary = self.primary();
        let paired = self.paired_state(0, t);
        let (state, extras) = match summarised(primary, t, paired.as_ref())? {
            Summarised::Own(state) => {
                let streams = self.streams(self.hierarchy.star(StarIndex::PRIMARY).body());
                let extras = primary.class_extras_at(&state, t, Pulses::Opened(streams));
                (state, extras)
            }
            Summarised::Paired(paired) => (paired.state, paired.class_extras_of(primary)),
        };
        let count = match self.shown_at(t) {
            Shown::Drawn => self.star_count(),
            Shown::Pair(_) => 2,
            Shown::Part { stars, .. } => stars,
            Shown::Alone => 1,
        };
        Some(StellarBrief::of(
            &state,
            primary.composition(),
            primary.draws(),
            extras,
            (count, primary.rules),
        ))
    }

    /// Whether the summary's primary at `t` is its own single-star model's state (`Summarised::Own`),
    /// or `None` before it forms: what the range brief's gate certifies of every row it keeps on
    /// its single-star routes (P11.T11.a).
    #[cfg(test)]
    #[must_use]
    pub(crate) fn primary_is_its_own_at(&self, t: UniverseTime) -> Option<bool> {
        let paired = self.paired_state(0, t);
        summarised(self.primary(), t, paired.as_ref())
            .map(|summarised| matches!(summarised, Summarised::Own(_)))
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

    /// The clock interval in which the primary is a luminous blue variable
    /// ([`StarModel::lbv_window`], P06.T24.a), which plan 09's catalogue class reads.
    #[must_use]
    pub fn lbv_window(&self) -> Option<(UniverseTime, UniverseTime)> {
        self.primary().lbv_window()
    }

    /// The system at `t` (plan 11, P11.T11), or `None` before it is born: every star's state, from
    /// its pair's timeline where the engine ran one and otherwise from its own model, every pair's
    /// orbit and class then, and the system's combined luminosity and mass.
    ///
    /// From version 27 they are those of the stars and pairs the system shows then, each star by
    /// its body index and each pair by its node in the hierarchy it shows then (plan 11, P11.T10).
    ///
    /// Continuous in `t` across the clock window except where a star changes phase, a pair
    /// begins a new segment of its timeline, or a star dies.
    ///
    /// # Panics
    ///
    /// In debug builds, as [`StarModel::state_at`], for a `t` after the clock window's end.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::Seed;
    /// use hyperion_sim::galaxy::Galaxy;
    /// use hyperion_sim::galaxy::placement::{CellKey, generate_cell};
    /// use hyperion_sim::id::Layer;
    /// use hyperion_sim::stellar::system::SystemStars;
    /// use hyperion_sim::time::UniverseTime;
    ///
    /// let galaxy = Galaxy::new(Seed::new(11));
    /// let mut cell = Vec::new();
    /// generate_cell(&galaxy, CellKey::new(Layer::C, [0, 812, 0])?, &mut cell);
    /// let stars = SystemStars::generate(&galaxy, cell.first().ok_or("the cell has systems")?);
    /// if let Some(now) = stars.state_at(UniverseTime::EPOCH) {
    ///     // The system's light is at least its primary's.
    ///     assert!(now.luminosity() >= now.stars()[0].luminosity());
    ///     assert_eq!(now.stars().len(), usize::from(stars.star_count()));
    /// }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn state_at(&self, t: UniverseTime) -> Option<SystemState> {
        if SystemExistence::from(self.record.existence_at(t)) == SystemExistence::NotYetBorn {
            return None;
        }
        let shown = self.shown_at(t);
        let stars: Vec<StarState> = (0_u8..)
            .zip(&self.stars)
            .filter(|&(index, _)| shown.holds(index))
            .filter_map(|(index, star)| {
                self.paired_state(index, t)
                    .map(|paired| paired.state)
                    .or_else(|| star.state_at(t))
            })
            .collect();
        let pairs = self
            .hierarchy
            .pairs()
            .filter(|&(node, _)| match shown {
                Shown::Drawn => true,
                Shown::Pair(kept) => node == kept,
                Shown::Part { node: kept, stars } => {
                    node >= kept && stars_under(&self.hierarchy, node) >> stars == 0
                }
                Shown::Alone => false,
            })
            .map(|(node, orbit)| {
                // Each pair by its node in the shown hierarchy: the shown pair is the root of its
                // own (`hierarchy_at`), and a shown part's nodes are renumbered from its root.
                let shown_node = match shown {
                    Shown::Drawn | Shown::Alone => node,
                    Shown::Pair(_) => NodeIndex::ROOT,
                    Shown::Part { node: kept, .. } => node.in_part(kept),
                };
                match self
                    .pair_timeline(node)
                    .and_then(|pair| self.pair_state_at(pair, t))
                {
                    Some(state) => PairState {
                        node: shown_node,
                        orbit: state.0.orbit().copied(),
                        class: state.1,
                    },
                    None => PairState {
                        node: shown_node,
                        orbit: Some(*orbit),
                        class: BinaryClass::None,
                    },
                }
            })
            .collect();
        let luminosity = stars
            .iter()
            .fold(0.0, |sum, s| sum + s.luminosity().value());
        let mass = stars.iter().fold(0.0, |sum, s| sum + s.mass().value());
        Some(SystemState {
            time: t,
            stars,
            pairs,
            luminosity: SolarLuminosities::new(luminosity),
            mass: SolarMasses::new(mass),
        })
    }

    /// The state at `t` of the pair `pair` if the engine ran it and the system exists then, or
    /// `None`.
    #[must_use]
    pub fn binary_state_at(&self, pair: NodeIndex, t: UniverseTime) -> Option<BinaryState> {
        let timeline = self.pair_timeline(pair)?;
        self.pair_state_at(timeline, t).map(|(state, _)| state)
    }

    /// The system's mass at `t`, M☉ ([`SystemState::mass`]); zero before it is born.
    #[must_use]
    pub fn system_mass_at(&self, t: UniverseTime) -> SolarMasses {
        self.state_at(t)
            .map_or(SolarMasses::ZERO, |state| state.mass())
    }

    /// The recoil of the pair holding the primary after its supernovae, while the pair stays bound
    /// ([`BinaryTimeline::recoil`]): `None` for a primary in no pair `run_pairs` ran, or one whose
    /// pair had no supernova or was unbound. A pair the pre-test passes over is run for its
    /// remnant, and its collapses' recoils count (P11.T4.l).
    #[must_use]
    pub fn recoil(&self) -> Option<SystemVelocity> {
        self.pairs
            .iter()
            .find(|pair| pair.stars[0] == StarIndex::PRIMARY)
            .and_then(|pair| pair.timeline.recoil())
    }

    /// What the system shows at `t` (plan 11, P11.T10).
    ///
    /// While no supernova has unbound one of its pairs by the primary's age then, it is the drawn
    /// hierarchy ([`Shown::Drawn`]). From the first supernova that
    /// unbinds the pair holding the primary ([`disrupting_supernova`]) it is the primary alone, its
    /// remnant where the primary exploded, whatever else the hierarchy holds: the released star
    /// leaves at up to its orbital speed, above an outer companion's by the square root of the
    /// orbits' ratio of sizes, so that, for the wide outer orbits of most hierarchies, the outer
    /// stars are left behind as well (near the stability limit, a ratio of about 3, the outer star
    /// may stay; a lean of 2026-11-01). From one that unbinds another pair it is what is left
    /// without that pair's two stars: the primary's own pair, stars 0 and 1, where they are all
    /// that is left ([`Shown::Pair`]), and otherwise the primary alone (the lean of 2026-11-01:
    /// a hierarchy left with gaps in its body indices, such as `((0, (1, 2)), 3)` without its
    /// inner pair, has no shape of its own, and its outer star has lost the pair's mass from its
    /// orbit).
    ///
    /// Under P11.T10's fix an outer orbit a supernova of a pair below it unbinds
    /// ([`OuterRelease`]) takes from the system its member that does not hold the primary, or both
    /// where neither does ([`SystemStars::released_by`]). What is left is shown as above, the primary's own pair where
    /// stars 0 and 1 are all that is left, and otherwise the largest drawn part of the hierarchy
    /// on the primary's inner chain that it holds ([`Shown::Part`] for three stars or more; the
    /// primary alone if none).
    ///
    /// A supernova that unbinds a pair releases the other star at most at its orbital speed and
    /// the remnant at its kick (Tauris and Takens 1998, A&A 330, 1047; recalled, not read at
    /// source), the new orbit and the speeds the engine keeps being Hurley, Tout and Pols's (2002)
    /// appendix A1's at the explosion. The released star is plan 08's independent object where it
    /// is of 2.5 M☉ or more, a walkaway or the supernova-released share of the runaways
    /// ([`RunawayModel`](crate::galaxy::displaced::runaway::RunawayModel)), placed elsewhere, and
    /// left out where it is lighter, "under 1% of the dwarfs" (brainstorm, "Displaced objects:
    /// kicks and runaways"): a grid system that went on showing it would count it twice. The
    /// remnant a companion leaves is not, a death the catalogue does not list (design note 16). A
    /// white dwarf's birth that unbinds a wide pair releases no star plan 08 places elsewhere, and
    /// is shown as before.
    #[must_use]
    pub(crate) fn shown_at(&self, t: UniverseTime) -> Shown {
        self.shown_at_age(self.primary().age_at(t))
    }

    /// For each star, by body index, the age of the system, Julian years, from which
    /// [`shown_at`](Self::shown_at) no longer shows it, if it leaves the system by the ages its
    /// pairs were run to: a star that a supernova released, whose planets go with it, as plan 14
    /// reads them (P14.T9.d). What the system shows changes only at a disrupting supernova's age
    /// and an outer orbit's release, so each star's is the first of those ages at which it is not
    /// shown.
    #[must_use]
    pub(crate) fn departures(&self) -> Vec<Option<Years>> {
        let mut ages: Vec<Years> = self
            .pairs
            .iter()
            .filter_map(|pair| disrupting_supernova(&pair.timeline).map(SupernovaRecord::age))
            .chain(self.releases.iter().map(|release| release.age))
            .collect();
        ages.sort_by(Years::total_cmp);
        let mut departures = vec![None; self.stars.len()];
        for age in ages {
            let shown = self.shown_at_age(age);
            for (index, departure) in (0_u8..).zip(&mut departures) {
                if departure.is_none() && !shown.holds(index) {
                    *departure = Some(age);
                }
            }
        }
        departures
    }

    /// [`shown_at`](Self::shown_at) at the primary's age `age`, Julian years.
    #[must_use]
    fn shown_at_age(&self, age: Years) -> Shown {
        // A bit for each star still held: 16 stars at most (`StarIndex`).
        let all = (1_u32 << self.stars.len()) - 1;
        let mut held = all;
        for pair in &self.pairs {
            let Some(sn) = disrupting_supernova(&pair.timeline) else {
                continue;
            };
            if sn.age() > age {
                continue;
            }
            if pair.stars.contains(&StarIndex::PRIMARY) {
                return Shown::Alone;
            }
            for star in pair.stars {
                held &= !(1_u32 << star.get());
            }
        }
        for release in self.releases.iter().filter(|r| r.age <= age) {
            held &= !self.released_by(release);
        }
        if held == all {
            return Shown::Drawn;
        }
        let primary_pair = self.hierarchy.pairs().find_map(|(node, _)| {
            let HierarchyNode::Pair { inner, outer, .. } = *self.hierarchy.node(node) else {
                unreachable!("pairs are pairs");
            };
            matches!(
                (*self.hierarchy.node(inner), *self.hierarchy.node(outer)),
                (HierarchyNode::Star(a), HierarchyNode::Star(b))
                    if a == StarIndex::PRIMARY && b.get() == 1
            )
            .then_some(node)
        });
        match primary_pair {
            Some(node) if held == 0b11 => return Shown::Pair(node),
            Some(_) | None => {}
        }
        // Only an outer orbit's release leaves a part of three stars or more, or one beside
        // a star the shown hierarchy cannot hold.
        if self.releases.is_empty() {
            return Shown::Alone;
        }
        self.shown_part(held)
    }

    /// The stars, a bit for each, that the outer orbit `release` unbinds takes from the system
    /// ([`SystemStars::shown_at`]): its member that does not hold the primary, where one does, and
    /// otherwise both.
    #[must_use]
    pub(crate) fn released_by(&self, release: &OuterRelease) -> u32 {
        let HierarchyNode::Pair { inner, outer, .. } = *self.hierarchy.node(release.node) else {
            unreachable!("a release is of a pair's orbit, as `outer_releases` makes it");
        };
        let [a, b] = [inner, outer].map(|n| stars_under(&self.hierarchy, n));
        if a & 1 == 1 {
            b
        } else if b & 1 == 1 {
            a
        } else {
            a | b
        }
    }

    /// What the system shows of the stars `held`, a bit for each, the primary among them, once an
    /// outer orbit's release has taken the others ([`SystemStars::shown_at`]): the drawn part
    /// under the first node of the primary's inner chain, from the root down, all of whose stars
    /// are held. Where the held stars are such a part, it is all of them. Otherwise a star the
    /// shown hierarchy cannot hold without a gap in its bodies is left out with the rest (a lean of
    /// 2026-11-01), as `outer_releases` does where the primary's member escapes an orbit but not
    /// the one above it.
    #[must_use]
    fn shown_part(&self, held: u32) -> Shown {
        let mut node = NodeIndex::ROOT;
        loop {
            let under = stars_under(&self.hierarchy, node);
            match *self.hierarchy.node(node) {
                HierarchyNode::Star(_) => return Shown::Alone,
                HierarchyNode::Pair { .. } if under & !held == 0 => {
                    return if under == 0b11 {
                        Shown::Pair(node)
                    } else {
                        Shown::Part {
                            node,
                            stars: u8::try_from(under.count_ones()).expect("16 stars at most"),
                        }
                    };
                }
                HierarchyNode::Pair { inner, .. } => node = inner,
            }
        }
    }

    /// The outer orbits its pairs' supernovae unbind, by age (P11.T10's fix).
    #[cfg(test)]
    #[must_use]
    pub(crate) fn releases(&self) -> &[OuterRelease] {
        &self.releases
    }

    /// The hierarchy the system shows at `t` (plan 11, P11.T10).
    ///
    /// It is the drawn one ([`SystemStars::hierarchy`]), and from version 27, once a supernova has
    /// unbound one of its pairs, what that has left of it: the stars and pairs of
    /// [`SystemStars::state_at`] are its bodies and nodes. A caller that walks the stars of the
    /// state at `t`, as the sky's census does their positions, walks this one.
    #[must_use]
    pub(crate) fn hierarchy_at(&self, t: UniverseTime) -> Cow<'_, SystemHierarchy> {
        self.shown_hierarchy(self.shown_at(t))
    }

    /// The hierarchy of what `shown` shows: the drawn one, the drawn pair of the primary and star
    /// 1 on its drawn orbit, or the primary alone.
    #[must_use]
    fn shown_hierarchy(&self, shown: Shown) -> Cow<'_, SystemHierarchy> {
        let primary = self.hierarchy.star(StarIndex::PRIMARY);
        Cow::Owned(match shown {
            Shown::Drawn => return Cow::Borrowed(&self.hierarchy),
            Shown::Pair(node) => {
                let HierarchyNode::Pair { orbit, .. } = *self.hierarchy.node(node) else {
                    unreachable!("a shown pair is a pair of the hierarchy");
                };
                let second = self
                    .hierarchy
                    .star(StarIndex::from_body(1).expect("body 1 is a star's"));
                SystemHierarchy::binary_on(
                    self.record.id(),
                    primary.initial_mass(),
                    (second.initial_mass(), second.kind()),
                    orbit,
                )
            }
            Shown::Part { node, .. } => self.hierarchy.part(node),
            Shown::Alone => {
                SystemHierarchy::single(self.record.id(), primary.initial_mass(), primary.kind())
            }
        })
    }

    /// The timeline of the pair at node `node`, if the engine ran it.
    #[must_use]
    fn pair_timeline(&self, node: NodeIndex) -> Option<&PairTimeline> {
        self.pairs.iter().find(|pair| pair.node == node)
    }

    /// The state and class at `t` of `pair`, or `None` if the system is not born then or `t`
    /// lies beyond the ages its timeline was run to.
    #[must_use]
    fn pair_state_at(
        &self,
        pair: &PairTimeline,
        t: UniverseTime,
    ) -> Option<(BinaryState, BinaryClass)> {
        let age = pair.age_at(&self.stars, t)?;
        Some((pair.timeline.state_at(age), pair.timeline.class_at(age)))
    }

    /// Star `index`'s state and class at `t` from its pair's timeline, if the engine ran it in a
    /// pair and `t` lies inside that timeline.
    #[must_use]
    fn paired_state(&self, index: u8, t: UniverseTime) -> Option<PairedStar> {
        let (pair, component) = pair_of(&self.pairs, index)?;
        let (state, class) = self.pair_state_at(pair, t)?;
        let own = state.stars()[component.index()];
        // A star merged into its companion, or one its own death left nothing of (a pair
        // instability, which disrupts the pair: P11.T4.e's label), has no class of its own.
        let gone = own.phase() == Phase::NoRemnant;
        let pulsar = (own.phase() == Phase::NeutronStar)
            .then(|| {
                let age = self.stars[usize::from(index)].age_at(t);
                pair.timeline
                    .class_context(age)
                    .pulsar(component)
                    .map(|p| *p.state())
            })
            .flatten();
        // Only a naked helium star has one, and only it pays for the walk.
        let helium_entry = matches!(
            own.phase(),
            Phase::HeliumMainSequence | Phase::HeliumHertzsprungGap | Phase::HeliumGiantBranch
        )
        .then(|| pair.timeline.helium_star_entry_mass(component, state.age()))
        .flatten();
        let model = &self.stars[usize::from(index)];
        let own_death = model.remnant.is_some_and(|stage| {
            let at = stage.death.age().value();
            pair.timeline
                .supernovae()
                .iter()
                .any(|sn| sn.component() == component && (sn.age().value() - at).abs() <= 1e-9 * at)
        });
        Some(PairedStar {
            state: own,
            class: if gone { BinaryClass::None } else { class },
            pulsar,
            helium_entry,
            own_death,
        })
    }
}

/// The hierarchy `record`'s system draws under `ctx` at `attempt`, with `primary` and each
/// companion's single-star model on its own body's draws of that attempt, and no binary engine.
#[must_use]
fn models_at(
    galaxy: &Galaxy,
    record: &SystemRecord,
    primary: &StarModel,
    ctx: MultiplicityContext,
    attempt: RedrawAttempt,
) -> (SystemHierarchy, Vec<StarModel>) {
    let composition = *primary.composition();
    let hierarchy = draw_hierarchy_of_primary(galaxy, record, primary, ctx, attempt);
    let age = record.age_at_epoch();
    let mut stars = Vec::with_capacity(hierarchy.stars().len());
    stars.push(primary.clone());
    stars.extend(hierarchy.stars().iter().skip(1).map(|slot| {
        let draws = StarDraws::for_attempt(galaxy.seed(), slot.body(), u32::from(attempt.get()));
        StarModel::new_under(
            slot.initial_mass(),
            composition,
            draws,
            age,
            galaxy.rules().stellar,
        )
        .expect("a companion is of 13 Jupiter masses up to its primary's mass, with a finite age")
    }));
    (hierarchy, stars)
}

/// The single-star models of the grid system `record`'s stars at the first redraw attempt,
/// primary first, without the binary engine: what a range row's brief read before P11.T11.a (its
/// first attempt's star count), at a certified range row's cost, for plan 12's lens masses
/// (P12.T4). For the few systems
/// the grid redraws (P11.T7) they are the first attempt's stars, and a pair's mass transfer is
/// not in them (plan 11's Risks, "Deviations in P11.T11, as built"). The primary is
/// [`primary_model`]'s, so a record whose class constrains its kick (plan 08, P08.T12.c) lends
/// the lens the remnant of the kick loop's kept attempt, the one `SystemStars` keeps.
///
/// # Panics
///
/// As [`SystemStars::generate`].
#[must_use]
pub(crate) fn first_attempt_models(galaxy: &Galaxy, record: &SystemRecord) -> Vec<StarModel> {
    let (primary, _) = primary_model(galaxy, record, draw_metallicity(galaxy, record));
    models_at(
        galaxy,
        record,
        &primary,
        grid_multiplicity(record),
        GRID_ATTEMPT,
    )
    .1
}

/// What a star's pair says of it at one time: its state, its pair's class, a neutron star's
/// pulsar, recycled by what it accreted, and a naked helium star's mass when it became one.
#[derive(Debug, Clone, Copy, PartialEq)]
struct PairedStar {
    state: StarState,
    class: BinaryClass,
    pulsar: Option<PulsarState>,
    /// The star's mass, M☉, when its pair's history made it a naked helium star in the helium-star
    /// life it is in, if it is in one ([`BinaryTimeline::helium_star_entry_mass`], P11.T11.b).
    helium_entry: Option<SolarMasses>,
    /// Whether the pair's timeline explodes the star at its own model's death age, as design note
    /// 16 pins a massive primary's death, so that the model's death stands.
    own_death: bool,
}

impl PairedStar {
    /// The class extras its pair's history gives `star`, the model it is the pair's word on: those
    /// of [`class_extras`](Self::class_extras), and under the star's rules a young star's class
    /// from its own initial mass and disc ([`peculiar_extras`]). A star its pair has changed takes
    /// no S or carbon class: its track's pulses are not the ones its pair gives it (the lane's
    /// lean, 2026-10-31).
    #[must_use]
    fn class_extras_of(&self, star: &StarModel) -> ClassExtras {
        peculiar_extras(
            self.class_extras(),
            || {
                young_class(
                    self.state.phase(),
                    star.initial_mass(),
                    self.state.age(),
                    star.draws(),
                )
            },
            || None,
        )
    }

    /// The class extras its pair's history gives it: a neutron star's recycled pulsar's class, and
    /// a naked helium star's surface from the mass it has lost since it became one, as a single
    /// star's track gives it ([`StarModel::class_extras_at`]; P11.T11.b), so that a helium star
    /// its pair has stripped turns WC as a single one does.
    #[must_use]
    fn class_extras(&self) -> ClassExtras {
        match self.state.phase() {
            Phase::NeutronStar => self.pulsar.map_or(ClassExtras::NONE, |pulsar| {
                ClassExtras::neutron_star(pulsar.class())
            }),
            Phase::HeliumMainSequence | Phase::HeliumHertzsprungGap | Phase::HeliumGiantBranch => {
                self.helium_entry.map_or(ClassExtras::NONE, |entry| {
                    ClassExtras::helium_star(HeliumSurface::of(entry, self.state.mass()))
                })
            }
            Phase::Protostar
            | Phase::PreMainSequence
            | Phase::MainSequence
            | Phase::HertzsprungGap
            | Phase::FirstGiantBranch
            | Phase::CoreHeliumBurning
            | Phase::EarlyAgb
            | Phase::ThermallyPulsingAgb
            | Phase::PostAgb
            | Phase::HeliumWhiteDwarf
            | Phase::CarbonOxygenWhiteDwarf
            | Phase::OxygenNeonWhiteDwarf
            | Phase::BlackHole
            | Phase::NoRemnant
            | Phase::Substellar => ClassExtras::NONE,
        }
    }
}

/// Where a star's summary at one time comes from ([`summarised`]).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Summarised<'a> {
    /// Its own model's state then: it is in no pair the engine ran, or its pair has not changed it.
    Own(StarState),
    /// Its pair's word on it, whose state is not its own model's: its pair has changed it.
    Paired(&'a PairedStar),
}

/// Where the summary of `star` at `t` comes from, given `paired`, its pair's word on it then: from
/// its pair where the engine ran it in a pair that has changed it (P11.T11), and otherwise from its
/// own model. `None` if the star has not formed by `t`.
///
/// It is the one place that chooses, so that [`star_summary_with`], the summary's star, and
/// [`SystemStars::brief_at`], the range row's and observed mode's (plan 12), read one state at one
/// time.
#[must_use]
fn summarised<'a>(
    star: &StarModel,
    t: UniverseTime,
    paired: Option<&'a PairedStar>,
) -> Option<Summarised<'a>> {
    let own = star.state_at(t)?;
    Some(match paired {
        Some(paired) if paired.state != own => Summarised::Paired(paired),
        Some(_) | None => Summarised::Own(own),
    })
}

/// Every pair of two stars of `hierarchy` that can interact by +H or holds a remnant by then, run
/// through the binary engine on the tracks of `stars` (P11.T11): from zero age to the pair's age at
/// the end of the clock window, +H, so that its timeline covers the source horizon. A pair the
/// pre-test passes over that holds a remnant is run too: where a star of it collapses by then the
/// engine runs it from the start, so that the collapse acts on its orbit, with its record (P11.T10's
/// fix), and otherwise it is its two models on the drawn orbit. Any other pair is two single stars
/// on their orbit, in no class, whose timeline would be one detached segment of its models, so it
/// is not run. A pair with a brown dwarf, or with a member that is itself a pair, is not run
/// either. Each pair runs under the stellar stage's `rules`, those of the galaxy the stars were
/// generated in.
#[must_use]
fn run_pairs(
    hierarchy: &SystemHierarchy,
    stars: &[StarModel],
    rules: StellarRules,
) -> Vec<PairTimeline> {
    hierarchy
        .pairs()
        .filter_map(|(node, orbit)| {
            let (input, [a, b]) = star_pair_input(hierarchy, stars, node, orbit, rules)?;
            let [first, second] = [a, b].map(|s| &stars[usize::from(s.get())]);
            let until = Years::new(first.age_at(ClockWindow::END).value().max(0.0));
            let tracks = [first.shared_track(), second.shared_track()];
            // A pair that cannot interact by +H is two single stars on their orbit, and with no
            // remnant by then it can be in no class: its timeline would be its models'.
            let dead = [first, second]
                .iter()
                .any(|s| s.remnant_age_at(ClockWindow::END).is_some());
            if !dead && !can_interact_with_tracks(&input, until, tracks.clone()) {
                return None;
            }
            let timeline = evolve_with_tracks(&input, until, tracks);
            Some(PairTimeline {
                node,
                stars: [a, b],
                timeline,
            })
        })
        .collect()
}

/// The binary engine's input for `hierarchy`'s pair `node` on `orbit`, of two stars of `stars`
/// (none with a brown dwarf or a member that is itself a pair), with the two stars, under the
/// stellar stage's `rules` ([`run_pairs`]).
#[must_use]
fn star_pair_input(
    hierarchy: &SystemHierarchy,
    stars: &[StarModel],
    node: NodeIndex,
    orbit: &KeplerElements,
    rules: StellarRules,
) -> Option<(BinaryInput, [StarIndex; 2])> {
    let HierarchyNode::Pair { inner, outer, .. } = *hierarchy.node(node) else {
        unreachable!("pairs are pairs");
    };
    let (HierarchyNode::Star(a), HierarchyNode::Star(b)) =
        (*hierarchy.node(inner), *hierarchy.node(outer))
    else {
        return None;
    };
    if [a, b]
        .iter()
        .any(|&s| hierarchy.star(s).kind() == SlotKind::BrownDwarf)
    {
        return None;
    }
    let [first, second] = [a, b].map(|s| &stars[usize::from(s.get())]);
    let input = BinaryInput::new(
        first.initial_mass(),
        second.initial_mass(),
        *first.composition(),
        *orbit,
        [first.draws().clone(), second.draws().clone()],
        first.age_at_epoch(),
    )
    .expect("a pair's stars are of 0.08-150 M_sun with a finite age")
    .with_rules(rules);
    Some((input, [a, b]))
}

/// The pairs of two stars of grid record `record`'s first attempt under `ctx`, each with its key
/// (`SystemHierarchy::pair_key`, which its Type Ia explosion mark reads), run through the binary
/// engine to the pair's age `until` rather than to +H: plan 15's census of the engine's Type Ia
/// pool over a Hubble time (P15.T9.b, `stellar::binary::testing::ia_pool_census_in`). The stars
/// are [`SystemStars::generate_at`]'s at the first attempt, and a pair that cannot interact by
/// `until` is left out, as its timeline would pool no candidate.
#[cfg(any(test, feature = "testing"))]
#[must_use]
pub(crate) fn first_attempt_pairs_to(
    galaxy: &Galaxy,
    record: &SystemRecord,
    ctx: MultiplicityContext,
    until: Years,
) -> Vec<(BodyId, BinaryTimeline)> {
    let composition = draw_metallicity(galaxy, record);
    let (primary, _) = primary_model(galaxy, record, composition);
    let (hierarchy, stars) = models_at(galaxy, record, &primary, ctx, RedrawAttempt::FIRST);
    let rules = galaxy.rules().stellar;
    hierarchy
        .pairs()
        .filter_map(|(node, orbit)| {
            let (input, [a, b]) = star_pair_input(&hierarchy, &stars, node, orbit, rules)?;
            let tracks = [a, b].map(|s| stars[usize::from(s.get())].shared_track());
            if !can_interact_with_tracks(&input, until, tracks.clone()) {
                return None;
            }
            let key = hierarchy
                .pair_key(node)
                .expect("a pair of the hierarchy has a key");
            Some((key, evolve_with_tracks(&input, until, tracks)))
        })
        .collect()
}

/// An orbit of a system's hierarchy that a supernova of a pair below it unbinds (P11.T10's fix,
/// `collapses.rs`): its node, the system's age then, and each member's velocity
/// after in the frame of the orbit's barycentre before, inner member first.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct OuterRelease {
    node: NodeIndex,
    age: Years,
    velocities: [SystemVelocity; 2],
}

impl OuterRelease {
    /// The unbound orbit's node.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn node(&self) -> NodeIndex {
        self.node
    }

    /// The system's age at the supernova that unbinds it.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn age(&self) -> Years {
        self.age
    }

    /// Each member's velocity after, inner member first, in the frame of the orbit's barycentre
    /// before.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn velocities(&self) -> [SystemVelocity; 2] {
        self.velocities
    }
}

/// The stars under `node` of `hierarchy`, a bit for each (16 stars at most, `StarIndex`).
#[must_use]
pub(crate) fn stars_under(hierarchy: &SystemHierarchy, node: NodeIndex) -> u32 {
    match *hierarchy.node(node) {
        HierarchyNode::Star(star) => 1_u32 << star.get(),
        HierarchyNode::Pair { inner, outer, .. } => {
            stars_under(hierarchy, inner) | stars_under(hierarchy, outer)
        }
    }
}

/// One supernova of a run pair, as it acts on the orbit above the pair: the system's age, the
/// orbit's node, and the pair as a point mass ([`PointLoss`]).
#[derive(Debug, Clone, Copy)]
struct OuterLoss {
    age: Years,
    node: NodeIndex,
    loss: PointLoss,
}

/// What [`outer_releases`] reads of a system: its hierarchy, each node's parent, its stars and its
/// run pairs.
struct Outer<'a> {
    hierarchy: &'a SystemHierarchy,
    /// Each node's parent, and which member of it the node is: 0 the inner, 1 the outer.
    parents: Vec<(NodeIndex, NodeIndex, usize)>,
    stars: &'a [StarModel],
    pairs: &'a [PairTimeline],
}

impl Outer<'_> {
    /// The parent of `child`, and which member of it `child` is, or `None` for the root.
    #[must_use]
    fn parent_of(&self, child: NodeIndex) -> Option<(NodeIndex, usize)> {
        self.parents
            .iter()
            .find(|&&(c, _, _)| c == child)
            .map(|&(_, parent, member)| (parent, member))
    }

    /// The mass of star `k` at the system's age `age`, M☉: its pair's word on it where the engine
    /// ran it to then, and otherwise its own model's.
    #[must_use]
    fn star_mass(&self, k: u8, age: Years) -> f64 {
        let paired = self.pairs.iter().find_map(|pair| {
            pair.component_of(StarIndex::from_body(k)?)
                .map(|c| (pair, c))
        });
        match paired {
            Some((pair, c)) if age <= pair.timeline.until() => pair.timeline.state_at(age).stars()
                [c.index()]
            .mass()
            .value(),
            Some(_) | None => {
                let star = &self.stars[usize::from(k)];
                star.state_at(on_the_clock(star.age_at_epoch(), age))
                    .map_or(0.0, |state| state.mass().value())
            }
        }
    }

    /// The mass of the stars under `node` at the system's age `age`, M☉, summed in body order.
    #[must_use]
    fn mass_under(&self, node: NodeIndex, age: Years) -> f64 {
        let under = stars_under(self.hierarchy, node);
        (0..16_u8)
            .filter(|k| under & (1_u32 << k) != 0)
            .map(|k| self.star_mass(k, age))
            .sum()
    }

    /// Applies `first`, a supernova below an orbit, to that orbit and the ones above it as
    /// [`outer_releases`] says, pushing each orbit it unbinds onto `releases`, which holds the
    /// earlier supernovae's.
    fn apply(&self, first: OuterLoss, releases: &mut Vec<OuterRelease>) {
        let OuterLoss {
            age,
            mut node,
            mut loss,
        } = first;
        let hierarchy = self.hierarchy;
        let t = on_the_clock(self.stars[0].age_at_epoch(), age);
        while !releases.iter().any(|r| r.node == node) {
            let HierarchyNode::Pair {
                inner,
                outer: other_member,
                orbit,
            } = *hierarchy.node(node)
            else {
                unreachable!("an outer orbit is a pair's");
            };
            let other = if loss.member == 0 {
                other_member
            } else {
                inner
            };
            loss.before_msun[1 - loss.member] = self.mass_under(other, age);
            let initial = hierarchy.node_mass(node).value();
            match outer_orbit_after(&orbit, t, initial, loss) {
                OuterOrbit::Unbound(velocities) => {
                    releases.push(OuterRelease {
                        node,
                        age,
                        velocities,
                    });
                    // The primary's member leaves as a point mass of its own, at its velocity,
                    // which the orbit above takes as its kick and may be unbound by in turn.
                    let held = [inner, other_member]
                        .iter()
                        .position(|&n| stars_under(hierarchy, n) & 1 == 1);
                    let (Some(side), Some((parent, member))) = (held, self.parent_of(node)) else {
                        break;
                    };
                    let before: f64 = loss.before_msun.iter().sum();
                    let left = if loss.member == side {
                        loss.after_msun
                    } else {
                        loss.before_msun[side]
                    };
                    let mut before_msun = [0.0; 2];
                    before_msun[member] = before;
                    (node, loss) = (
                        parent,
                        PointLoss {
                            member,
                            before_msun,
                            after_msun: left,
                            kick: velocities[side],
                        },
                    );
                }
                OuterOrbit::Bound(recoil) => {
                    let Some((parent, member)) = self.parent_of(node) else {
                        break;
                    };
                    let before: f64 = loss.before_msun.iter().sum();
                    let lost = loss.before_msun[loss.member] - loss.after_msun;
                    let mut before_msun = [0.0; 2];
                    before_msun[member] = before;
                    (node, loss) = (
                        parent,
                        PointLoss {
                            member,
                            before_msun,
                            after_msun: before - lost,
                            kick: recoil,
                        },
                    );
                }
            }
        }
    }

    /// The supernovae of `pair` that act on the orbit above it ([`outer_releases`]), in its
    /// records' order: none for a pair at the root.
    #[must_use]
    fn losses(&self, pair: &PairTimeline) -> Vec<OuterLoss> {
        let Some((node, member)) = self.parent_of(pair.node) else {
            return Vec::new();
        };
        let mut losses = Vec::new();
        for sn in pair.timeline.supernovae() {
            let before = pair
                .timeline
                .state_at(Years::new(sn.age().value() * (1.0 - 1e-9)));
            let exploding = sn.component().index();
            let alone = before.stars()[1 - exploding].phase() == Phase::NoRemnant;
            let kick = match (before.orbit().is_some(), sn.bound(), alone) {
                (true, true, _) => sn.velocities()[0],
                // A merged product's collapse, all the pair has left.
                (false, _, true) => sn.velocities()[exploding],
                // The pair disrupted now or before.
                (true, false, _) | (false, _, false) => break,
            };
            let mass = before.total_mass().value();
            let lost = before.stars()[exploding].mass().value() - sn.remnant().mass().value();
            let mut before_msun = [0.0; 2];
            before_msun[member] = mass;
            losses.push(OuterLoss {
                age: sn.age(),
                node,
                loss: PointLoss {
                    member,
                    before_msun,
                    after_msun: mass - if lost > 0.0 { lost } else { 0.0 },
                    kick,
                },
            });
        }
        losses
    }
}

/// The clock time at which a system `age_at_epoch` old at the epoch is `age` old, for an age by
/// its age at +H ([`clock_time_of_death`]).
#[must_use]
fn on_the_clock(age_at_epoch: Years, age: Years) -> UniverseTime {
    clock_time_of_death(age_at_epoch, age)
        .expect("an age by +H lies within the clock's ±2.9 × 10¹¹ years of the epoch")
}

/// The outer orbits of `hierarchy` that the supernovae of its run pairs `pairs` unbind, by age
/// (P11.T10's fix, `collapses.rs`).
///
/// Each supernova of a pair, while the pair is bound just before it or is all its merged product,
/// acts on the orbit of the pair's parent node with the node's other member, the pair a point mass
/// ([`outer_orbit_after`]): its stars' masses just before the explosion, those less what the
/// exploding star lost just after it, and its barycentre's recoil, the record's velocity for a
/// pair that stays bound and the exploding star's own for a product alone. An orbit it leaves
/// bound acts so in turn, with its own masses and recoil, on the orbit of its parent, and so on
/// up. An orbit it unbinds that holds the primary leaves its primary's member to go on alone, a
/// point mass of that member's mass at its velocity after, which the orbit above takes as its
/// loss and kick, and so on up while each comes out unbound. A pair's disruption ends its
/// supernovae's effect on the orbits above (the system shows it, [`disrupting_supernova`]), and an
/// orbit unbound takes no later supernova. Every other member's
/// mass is its stars' at the explosion, each from its pair's timeline where the engine ran it, and
/// otherwise from its own model.
#[must_use]
fn outer_releases(
    hierarchy: &SystemHierarchy,
    stars: &[StarModel],
    pairs: &[PairTimeline],
) -> Vec<OuterRelease> {
    if pairs.is_empty() {
        return Vec::new();
    }
    let parents = hierarchy
        .pairs()
        .flat_map(|(node, _)| {
            let HierarchyNode::Pair { inner, outer, .. } = *hierarchy.node(node) else {
                unreachable!("pairs are pairs");
            };
            [(inner, node, 0), (outer, node, 1)]
        })
        .collect();
    let outer = Outer {
        hierarchy,
        parents,
        stars,
        pairs,
    };
    let mut losses: Vec<OuterLoss> = pairs.iter().flat_map(|p| outer.losses(p)).collect();
    // In age order, a pair's own in its records' order, so that an orbit takes its first.
    debug_assert!(
        losses.iter().all(|l| l.age.value().is_finite()),
        "a supernova's age is finite"
    );
    losses.sort_by(|a, b| a.age.value().total_cmp(&b.age.value()));
    let mut releases: Vec<OuterRelease> = Vec::new();
    for loss in losses {
        outer.apply(loss, &mut releases);
    }
    releases
}

/// The multiplicity context of the grid record `record`: [`MultiplicityContext::Free`] for a
/// star, and `ForcedSingle` for a free-floating brown dwarf or rogue planet, which are single
/// (plan 13's Risks: "Brown dwarfs are single"; plan 11's multiplicity is not applied to them),
/// and for a member of one of plan 08's displaced classes (P08.T12.c): a runaway or a walkaway is
/// single by construction, and a remnant its kick carried from its birth site travels alone
/// (brainstorm, "Displaced objects: kicks and runaways"); a retained remnant, kicked below a
/// quarter of the circular speed, keeps its companions.
///
/// Public as a seam: a caller that draws a grid record's hierarchy itself, as plan 14's hosts of
/// P14.T27 will for a brown dwarf, takes the same context [`SystemStars::generate`] does.
#[must_use]
pub fn grid_multiplicity(record: &SystemRecord) -> MultiplicityContext {
    match record.kind() {
        SystemKind::Stellar => match record.placement_class() {
            PlacementClass::Displaced { .. } => MultiplicityContext::ForcedSingle,
            PlacementClass::Alive | PlacementClass::Retained { .. } => MultiplicityContext::Free,
        },
        SystemKind::BrownDwarf | SystemKind::RoguePlanet => MultiplicityContext::ForcedSingle,
    }
}

/// The attempts plan 08's kick loop makes before it keeps the last, its kick scaled into the range
/// (Design note 17). The mean over displaced systems is about the number of speed bins (9.3
/// galaxy-wide); 2,081 of 10⁷ reach the cap, against Design note 17's none, a finding pinned by
/// P08.T12.c's test.
pub(crate) const KICK_LOOP_CAP: u32 = 4_096;

/// The primary's model of the grid system `record` in `galaxy` at `composition`, and the attempts
/// plan 08's kick loop made for it (P08.T12.c, Design note 17): 1 where the record constrains no
/// kick, which is every record of the version's galaxies. A hypervelocity survivor whose donor is a
/// helium white dwarf is that white dwarf, of the record's mass and age
/// ([`StarModel::helium_white_dwarf_under`]; plan 09, P09.T34.b).
///
/// Where the record has a [`kick_constraint`](SystemRecord::kick_constraint), the remnant stage
/// is repeated on the one track the record's draws build, with the remnant's, the stripped mark's
/// and the kick's draws of the attempts after its mark attempt
/// ([`StarDraws::with_remnant_draws_of`]; the stripped mark kept where the track's build read it,
/// ruling 93.2 as amended 2026-11-01), until the natal kick's speed falls in the range; after
/// [`KICK_LOOP_CAP`] attempts the last is kept with its kick scaled into the range. So an attempt
/// costs a remnant and a kick and never a track, and the record's kick lies in its bin, which,
/// given its mass, is exact.
///
/// It is the one reading of the kept attempt: every reader of a constrained record's primary
/// takes its star from here, so that each sees the star `SystemStars` keeps (P08.T12.c). The
/// system stage, the range brief's single-star routes
/// ([`BriefModel`](crate::stellar::brief::BriefModel)) and the lens masses
/// ([`first_attempt_models`]) build it, and plan 11's hierarchy reads its stripped mark
/// ([`primary_kept_stripped_mark`]), from the model where its caller holds one.
///
/// # Panics
///
/// If the record's primary is not of 0.01–150 M☉ or its age is not finite, as
/// [`SystemStars::generate_with`] does.
#[must_use]
pub(crate) fn primary_model(
    galaxy: &Galaxy,
    record: &SystemRecord,
    composition: Composition,
) -> (StarModel, u32) {
    if is_helium_donor(record) {
        let model = StarModel::helium_white_dwarf_under(
            record.primary_initial_mass(),
            composition,
            primary_draws(galaxy, record),
            record.age_at_epoch(),
            galaxy.rules().stellar,
        )
        .expect("a hypervelocity survivor's helium white dwarf has a mass and a finite age");
        return (model, 1);
    }
    let model = StarModel::new_under(
        record.primary_initial_mass(),
        composition,
        primary_draws(galaxy, record),
        record.age_at_epoch(),
        galaxy.rules().stellar,
    )
    .expect("a grid record's primary is a star or a brown dwarf, with a finite age");
    with_record_kick(galaxy, record, model)
}

/// [`primary_model`] of `record` at `composition` on `track`, the primary's whole track built
/// already ([`StarModel::on_track_under`]): the same model and attempts, bit for bit, for a
/// record whose primary is not a helium donor and dies by the clock window's end, its track not
/// built again (plan 09, the core-collapse class's test, P09.T37's bench).
///
/// # Panics
///
/// As [`primary_model`], and in debug builds as [`StarModel::on_track_under`], or if the record's
/// primary is a hypervelocity survivor's helium white dwarf.
#[must_use]
pub(crate) fn primary_model_on_track(
    galaxy: &Galaxy,
    record: &SystemRecord,
    composition: Composition,
    track: Arc<Track>,
) -> (StarModel, u32) {
    debug_assert!(
        !is_helium_donor(record),
        "a helium white dwarf has no whole track to be built on"
    );
    let model = StarModel::on_track_under(
        record.primary_initial_mass(),
        composition,
        primary_draws(galaxy, record),
        record.age_at_epoch(),
        galaxy.rules().stellar,
        track,
    )
    .expect("a record's primary is a star or a brown dwarf, with a finite age");
    with_record_kick(galaxy, record, model)
}

/// `model`, `record`'s primary's, with the kick loop [`primary_model`] runs where the record
/// constrains its kick, and the attempts it made: 1 where it constrains none.
#[must_use]
fn with_record_kick(galaxy: &Galaxy, record: &SystemRecord, model: StarModel) -> (StarModel, u32) {
    let Some(constraint) = record.kick_range(galaxy) else {
        return (model, 1);
    };
    model.with_kick_in(
        &constraint,
        (galaxy.seed(), BodyId::new(record.id(), 0)),
        record_attempt(record),
    )
}

/// The primary's companion-stripped mark at the kick loop's kept attempt, at `composition`: the
/// mark of [`primary_model`]'s star, which plan 11's hierarchy reads (P08.T12.c). For a record
/// that constrains no kick it is [`primary_draws`]'s, with no track built; for one that does it
/// builds the primary's track and runs the loop, so a caller that holds the model reads the mark
/// from it instead.
#[must_use]
pub(crate) fn primary_kept_stripped_mark(
    galaxy: &Galaxy,
    record: &SystemRecord,
    composition: &Composition,
) -> Mark {
    if record.kick_range(galaxy).is_none() {
        return primary_draws(galaxy, record).stripped();
    }
    primary_model(galaxy, record, *composition)
        .0
        .draws()
        .stripped()
}

/// The primary's draws: body 0's at the record's attempt, [`StarDraws::for_star`] at attempt 0.
///
/// **A named seam** (plan 06, P06.T29.b; plan 11, P11.T2.c's second): plan 08's P08.T12.c made it
/// `for_attempt` of a grid record's [`mark_attempt`](SystemRecord::mark_attempt).
#[must_use]
pub(crate) fn primary_draws(galaxy: &Galaxy, record: &SystemRecord) -> StarDraws {
    match record_attempt(record) {
        0 => StarDraws::for_star(galaxy.seed(), BodyId::new(record.id(), 0)),
        attempt => StarDraws::for_attempt(galaxy.seed(), BodyId::new(record.id(), 0), attempt),
    }
}

/// The attempt a record's primary is read at: a grid record's mark attempt (plan 08, P08.T12.c; 0
/// but where layer D's runaway reduction rejected a draw), 0 for a catalogue system (plan 09,
/// P09.T33.a), and for a feature member the attempt its class's conditional draw chose (plan 09,
/// P09.T10), which its origin carries.
#[must_use]
fn record_attempt(record: &SystemRecord) -> u32 {
    match record.origin() {
        SystemOrigin::Grid(_) => u32::from(record.mark_attempt()),
        SystemOrigin::CatalogueSystem(_) => 0,
        SystemOrigin::FeatureMember { attempt, .. }
        | SystemOrigin::CentreMember { attempt }
        | SystemOrigin::GlobalListMember { attempt } => u32::from(attempt),
    }
}

/// The primary's η alone: [`primary_draws`]'s [`eta`](StarDraws::eta), bit for bit, from its one
/// stream, for the range brief of a main-sequence primary (P06.T38.e).
///
/// **The same seam as [`primary_draws`]:** plan 08's P08.T12.c changes both to the record's
/// `mark_attempt()` together.
///
/// `pub` for `hyperion-sky` alone (the crate split, CS.T1); not part of the sim's API.
#[doc(hidden)]
#[must_use]
pub fn primary_eta(galaxy: &Galaxy, record: &SystemRecord) -> StandardNormal {
    StarDraws::eta_for_attempt(
        galaxy.seed(),
        BodyId::new(record.id(), 0),
        record_attempt(record),
    )
}

/// The primary's rotation rank and fossil-field mark alone: [`primary_draws`]'s, bit for bit, for
/// the range brief of a main-sequence primary, whose peculiar class reads them (P06.T25).
///
/// **The same seam as [`primary_draws`].**
#[must_use]
pub(crate) fn primary_rotation_draws(
    galaxy: &Galaxy,
    record: &SystemRecord,
) -> (UnitUniform, Mark) {
    StarDraws::rotation_for_attempt(
        galaxy.seed(),
        BodyId::new(record.id(), 0),
        record_attempt(record),
    )
}

/// The clock time of `death` for a star whose age at the epoch is `age_at_epoch`.
#[must_use]
fn clock_death(age_at_epoch: Years, death: Death) -> ClockDeath {
    match clock_time_of_death(age_at_epoch, death.age()) {
        Some(t) => ClockDeath::At(t, death.kind()),
        None => ClockDeath::BeyondClockRange,
    }
}

/// The clock time at which a star whose age at the epoch is `age_at_epoch` reaches the age
/// `death_age` (both Julian years since its onset of collapse): the epoch plus (`death_age` −
/// `age_at_epoch`) × 31,557,600 s, floored to the nanosecond, or `None` where the clock cannot
/// hold it (its whole seconds do not fit an `i64`, about ±2.9 × 10¹¹ years).
///
/// It is [`SystemStars::death_time`]'s conversion, which plan 08's interval predicate shares
/// (`galaxy::displaced::death_in_interval`, P08.T13), so that a death the predicate finds is the
/// stellar stage's to the nanosecond. The time is non-decreasing in `death_age`.
#[must_use]
pub(crate) fn clock_time_of_death(age_at_epoch: Years, death_age: Years) -> Option<UniverseTime> {
    let years = death_age.value() - age_at_epoch.value();
    Span::from_seconds_f64(years * SECONDS_PER_JULIAN_YEAR)
        .and_then(|span| UniverseTime::EPOCH.checked_add(span))
}

/// The initial mass a star of initial mass `initial_mass` has its track built for: its own, held to
/// the formulae's 150 M☉ (`sse::MAX_INITIAL_MASS`, P06.T14), as [`StarModel`] builds it, which
/// plan 08's interval predicate shares (P08.T13).
#[must_use]
pub(crate) fn track_mass_of(initial_mass: SolarMasses) -> SolarMasses {
    if initial_mass > sse::MAX_INITIAL_MASS {
        sse::MAX_INITIAL_MASS
    } else {
        initial_mass
    }
}

/// The summary of `star`, the system's body whose streams are `streams`, at `t`, or `None` if it
/// has not formed by then: from `paired`, its pair's word on it, where the engine ran it in a pair
/// and its pair has changed it, and otherwise [`star_summary`] with the pair's class
/// ([`summarised`] chooses).
#[must_use]
fn star_summary_with(
    star: &StarModel,
    streams: StarStreams,
    t: UniverseTime,
    paired: Option<PairedStar>,
) -> Option<StarSummary> {
    let body = streams.body();
    let paired = match summarised(star, t, paired.as_ref())? {
        Summarised::Own(_) => {
            return star_summary(star, streams, t).map(|summary| StarSummary {
                binary_class: paired.map_or(BinaryClass::None, |paired| paired.class),
                ..summary
            });
        }
        Summarised::Paired(paired) => paired,
    };
    let state = paired.state;
    let extras = paired.class_extras_of(star);
    let classification =
        classify_under(&state, star.composition(), star.draws(), extras, star.rules);
    let spin = rotation::rotation_under(&state, star.composition(), star.draws(), None, star.rules);
    let activity = rotation::activity(&state, spin.as_ref());
    let remnant = remnant_kind(state.phase()).map(|kind| {
        let mass = if kind == RemnantKind::None {
            SolarMasses::ZERO
        } else {
            state.mass()
        };
        CompactRemnant::new(kind, mass)
    });
    Some(StarSummary {
        body,
        kind: object_kind(&state, &classification, star.composition()),
        classification,
        absolute_magnitude_v: absolute_magnitude_v_under(&state, star.rules),
        colour_b_v: colour_b_v_under(state.effective_temperature(), star.rules),
        remnant,
        remnant_detail: if state.phase() == Phase::NeutronStar {
            paired.pulsar.map(RemnantDetail::NeutronStar)
        } else if state.phase() == Phase::BlackHole {
            Some(RemnantDetail::BlackHole(BlackHole::from_draws(
                state.mass(),
                star.draws(),
            )))
        } else {
            None
        },
        rotation: spin,
        magnetism: rotation::magnetism(&state, star.draws(), spin.as_ref()),
        activity,
        variability: variability(&VariabilityInputs {
            state: &state,
            initial_mass: star.initial_mass(),
            composition: star.composition(),
            classification: &classification,
            rotation: spin.as_ref(),
            activity: activity.as_ref(),
        }),
        // The star's own track's nebula (P06.T16.b) is not the one a pair that has changed its
        // evolution would light, and the engine models none: none until it does.
        planetary_nebula: None,
        death_in_window: if paired.own_death {
            own_death_in_window(star)
        } else {
            None
        },
        binary_class: paired.class,
        // The events are the star's own single-star model's, which its pair has replaced.
        active_events: None,
        event_effects: None,
        state,
    })
}

/// `star`'s own death, its clock time and kind, if it falls inside the clock window. A death inside
/// the window is always one the model holds, because its track is built to the window's end:
/// nothing further is built for it.
#[must_use]
fn own_death_in_window(star: &StarModel) -> Option<(UniverseTime, DeathKind)> {
    star.remnant.and_then(
        |stage| match clock_death(star.age_at_epoch(), stage.death) {
            ClockDeath::At(when, kind) if ClockWindow::contains(when) => Some((when, kind)),
            ClockDeath::At(..)
            | ClockDeath::BeyondClockRange
            | ClockDeath::AlreadyRemnantAtBirth => None,
        },
    )
}

/// The kind of remnant a star in `phase` is, or `None` for a living star.
#[must_use]
pub(crate) const fn remnant_kind(phase: Phase) -> Option<RemnantKind> {
    match phase {
        Phase::HeliumWhiteDwarf | Phase::CarbonOxygenWhiteDwarf | Phase::OxygenNeonWhiteDwarf => {
            Some(RemnantKind::WhiteDwarf)
        }
        Phase::NeutronStar => Some(RemnantKind::NeutronStar),
        Phase::BlackHole => Some(RemnantKind::BlackHole),
        Phase::NoRemnant => Some(RemnantKind::None),
        Phase::Protostar
        | Phase::PreMainSequence
        | Phase::MainSequence
        | Phase::HertzsprungGap
        | Phase::FirstGiantBranch
        | Phase::CoreHeliumBurning
        | Phase::EarlyAgb
        | Phase::ThermallyPulsingAgb
        | Phase::HeliumMainSequence
        | Phase::HeliumHertzsprungGap
        | Phase::HeliumGiantBranch
        | Phase::PostAgb
        | Phase::Substellar => None,
    }
}

/// The summary of `star`, the system's body whose streams are `streams`, at `t`, or `None` if it
/// has not formed by then.
#[must_use]
pub(crate) fn star_summary(
    star: &StarModel,
    streams: StarStreams,
    t: UniverseTime,
) -> Option<StarSummary> {
    let body = streams.body();
    let state = star.state_at(t)?;
    let events = StarEvents::of(star, streams.seed(), body);
    let classification = classify_under(
        &state,
        star.composition(),
        star.draws(),
        star.class_extras_at(&state, t, Pulses::Built(events.pulses())),
        star.rules,
    );
    let (active, event_effects) = events.at(t, &state);
    let active_events = Some(active);
    let spin = star.rotation_of(&state);
    let activity = rotation::activity(&state, spin.as_ref());
    let remnant = if state.phase().is_remnant() {
        star.remnant()
    } else {
        None
    };
    let death_in_window = own_death_in_window(star);
    Some(StarSummary {
        body,
        kind: object_kind(&state, &classification, star.composition()),
        classification,
        absolute_magnitude_v: absolute_magnitude_v_under(&state, star.rules),
        colour_b_v: colour_b_v_under(state.effective_temperature(), star.rules),
        remnant,
        remnant_detail: if state.phase() == Phase::NeutronStar {
            star.pulsar_at(t).map(RemnantDetail::NeutronStar)
        } else if state.phase() == Phase::BlackHole {
            star.black_hole().map(RemnantDetail::BlackHole)
        } else {
            None
        },
        rotation: spin,
        magnetism: rotation::magnetism(&state, star.draws(), spin.as_ref()),
        activity,
        variability: variability(&VariabilityInputs {
            state: &state,
            initial_mass: star.initial_mass(),
            composition: star.composition(),
            classification: &classification,
            rotation: spin.as_ref(),
            activity: activity.as_ref(),
        }),
        planetary_nebula: star.planetary_nebula_at(t),
        death_in_window,
        binary_class: BinaryClass::None,
        active_events,
        event_effects,
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
    use crate::galaxy::placement::CellKey;
    use crate::id::Layer;
    use crate::rng::Seed;
    use crate::stellar::classify::classify;
    use crate::units::SolarMasses;

    /// P08.T12.c: a kick moved to an edge of its speed bin, as the kick loop's cap moves one, is
    /// held by the bin once read back in km/s, at every bin's edges and at speed scales over the
    /// Milky Way's and the nuclear disc's, despite the km/s and m/s round trip's last ulp.
    #[test]
    fn a_kick_moved_to_its_bin_s_edge_lies_in_the_bin() {
        use crate::galaxy::displaced::SpeedBin;
        let mut v_ref = 100.0;
        while v_ref < 300.0 {
            for bin in SpeedBin::all() {
                let constraint = KickConstraint::of_bin(bin, KilometresPerSecond::new(v_ref), None);
                let (lo, hi) = constraint.speed();
                for edge in [lo, hi] {
                    if !edge.value().is_finite() {
                        continue;
                    }
                    let metres = metres_held_by(edge, &constraint);
                    assert!(
                        constraint.holds(KilometresPerSecond::from(MetresPerSecond::new(metres))),
                        "{v_ref} km/s, bin {}, edge {edge:?}",
                        bin.index()
                    );
                }
            }
            v_ref += 0.37;
        }
    }

    /// P08.T12.c: a track given its own fate is itself, bit for bit, and given the fate another
    /// attempt's remnant draws decide on it is the track those draws build, for a star outside the
    /// stripped window, where the stripped mark moves no track (`Track::with_fate`).
    #[test]
    fn a_track_with_another_attempt_s_fate_is_that_attempt_s_track() {
        let seed = Seed::new(0x0812_c000_0000_0001);
        let body = BodyId::new(
            CellKey::new(Layer::E, [0, 203, 0])
                .expect("in the cube")
                .candidate_id(7)
                .expect("under the capacity"),
            0,
        );
        let (m, comp) = (SolarMasses::new(15.0), Composition::SOLAR);
        let draws = StarDraws::for_star(seed, body);
        let track = Track::full(m, &comp, &draws);
        let own = track.fate_record().expect("a full track reaches its death");
        assert_eq!(track.clone().with_fate(own), track);
        let mut moved = 0;
        for k in 1..24 {
            let other = draws.clone().with_remnant_draws_of(seed, body, k);
            let fate = track
                .fate_with(RemnantDraws::of(&other))
                .expect("the track reaches its death");
            let built = Track::full(m, &comp, &other);
            moved += usize::from(fate.remnant != own.remnant);
            assert_eq!(track.clone().with_fate(fate), built, "attempt {k}");
        }
        assert!(moved > 0, "some attempt moves the remnant");
    }

    const SEED: u64 = 0x0600_0003_5eed_0001;

    /// The streams of `body` in the universe of [`SEED`].
    fn streams(body: BodyId) -> StarStreams {
        StarStreams::new(Seed::new(SEED), body)
    }

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

    /// The paired path of P06.T24.b's classes: a pre-main-sequence star its pair has changed takes
    /// the young class of its own initial mass and disc, and a star of the thermally pulsing AGB no
    /// S or carbon class (its track's pulses are not its pair's). No grid star its pair has changed
    /// is young, so this path is the one no grid system can show (`stellar/rules/classes_hold.rs`).
    #[test]
    fn a_paired_star_takes_a_young_class_and_no_carbon_class() {
        let paired = |state: StarState| PairedStar {
            state,
            class: BinaryClass::None,
            pulsar: None,
            helium_entry: None,
            own_death: false,
        };
        let model = |m: f64, age: f64, rules: StellarRules| {
            StarModel::new_under(
                SolarMasses::new(m),
                Composition::SOLAR,
                StarDraws::median(),
                Years::new(age),
                rules,
            )
            .unwrap()
        };
        let full = StellarRules::of_version(crate::GENERATOR_VERSION);
        let young = model(1.0, 2e6, full);
        let state = young.state_at(UniverseTime::EPOCH).unwrap();
        assert_eq!(
            paired(state).class_extras_of(&young),
            ClassExtras::NONE.with_peculiar(PeculiarClass::TTauri { classical: false })
        );
        let agb = model(3.0, 1.2e10, full);
        let clock = crate::stellar::events::thermal_pulse::ThermalPulseClock::of(&agb).unwrap();
        let half = clock
            .end()
            .checked_since(clock.start())
            .unwrap()
            .as_seconds_f64()
            / 2.0;
        let late = clock
            .start()
            .checked_add(Span::from_seconds_f64(half).unwrap())
            .unwrap();
        let state = agb.state_at(late).unwrap();
        assert_eq!(state.phase(), Phase::ThermallyPulsingAgb);
        assert_eq!(paired(state).class_extras_of(&agb), ClassExtras::NONE);
    }

    /// The paired path of P06.T28.g's hold: under every 26 hold a flaring dwarf its pair has
    /// changed (its pair's state a year older than its own) takes no events and no event light,
    /// while its own summary, at a time a flare runs, takes them.
    #[test]
    fn a_paired_star_takes_no_events() {
        let dwarf = StarModel::new_under(
            SolarMasses::new(0.2),
            Composition::SOLAR,
            StarDraws::median(),
            Years::new(3e8),
            StellarRules::of_version(crate::GENERATOR_VERSION),
        )
        .unwrap();
        let streams = streams(BodyId::new(test_system(), 0));
        let flaring = (0..24 * 30)
            .map(|h| {
                UniverseTime::EPOCH
                    .checked_add(Span::from_seconds(3_600 * h))
                    .unwrap()
            })
            .find(|&t| {
                star_summary(&dwarf, streams, t)
                    .and_then(|s| s.active_events().map(<[StarEvent]>::len))
                    .is_some_and(|n| n > 0)
            })
            .expect("a saturated dwarf flares within a month");
        let other = dwarf
            .state_at(UniverseTime::from_julian_years(1).unwrap())
            .unwrap();
        let paired = PairedStar {
            state: other,
            class: BinaryClass::None,
            pulsar: None,
            helium_entry: None,
            own_death: false,
        };
        let summary = star_summary_with(&dwarf, streams, flaring, Some(paired)).unwrap();
        assert_eq!(summary.state(), &other);
        assert_eq!(summary.active_events(), None);
        assert_eq!(summary.event_effects(), None);
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
            // The model's death carries the companion-stripped mark (P11.T1.d); the track's is
            // the single star's.
            let marked = full.death().map(|d| {
                StandardKickLaw::default().with_stripped_mark(
                    d,
                    &star_draws(i),
                    full.initial_mass(),
                    full.composition(),
                )
            });
            assert_eq!(star.death(), marked, "{m} M☉");
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

    /// A white dwarf a century past the AGB lights its nebula, and its summary carries the same
    /// one; long after, and before the star forms, and below 0.1 M☉, it lights none (P06.T16.b).
    #[test]
    fn a_young_white_dwarf_lights_its_nebula_and_its_summary_carries_it() {
        let at = |age: f64, m: f64| {
            StarModel::new(
                SolarMasses::new(m),
                Composition::SOLAR,
                StarDraws::median(),
                Years::new(age),
            )
            .unwrap()
        };
        let died = at(3.0e9, 2.0)
            .lifetime()
            .expect("a star of 2 M☉ dies")
            .value();
        let young = at(died + 100.0, 2.0);
        let shell = young
            .planetary_nebula_at(UniverseTime::EPOCH)
            .expect("a white dwarf a century old lights its nebula");
        assert!(shell.age().value() > 100.0, "{shell:?}");
        assert!(
            shell.radius().value() > 0.0 && shell.radius() < nebula::MAX_RADIUS,
            "{shell:?}"
        );
        let summary = star_summary(
            &young,
            streams(BodyId::new(test_system(), 0)),
            UniverseTime::EPOCH,
        )
        .expect("the white dwarf exists");
        assert_eq!(summary.planetary_nebula(), Some(shell));
        assert_eq!(
            at(died + 1.0e6, 2.0).planetary_nebula_at(UniverseTime::EPOCH),
            None
        );
        assert_eq!(
            at(-500.0, 2.0).planetary_nebula_at(UniverseTime::EPOCH),
            None
        );
        assert_eq!(
            at(5.0e9, 0.05).planetary_nebula_at(UniverseTime::EPOCH),
            None
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

    /// Since P06.T14 a star above 100 M☉ is evolved at its own mass, no longer as one of 100 M☉.
    #[test]
    fn a_star_above_a_hundred_solar_masses_is_evolved_at_its_own_mass() {
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
        assert!(
            heavy.state_at(UniverseTime::EPOCH).map(|s| s.luminosity())
                > limit.state_at(UniverseTime::EPOCH).map(|s| s.luminosity())
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
        assert_eq!(
            own.death,
            law.with_stripped_mark(
                death,
                star.draws(),
                track.initial_mass(),
                track.composition()
            )
        );
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
        let stage = remnant_stage(track, &turned, star.rules).unwrap();
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
            let stage = remnant_stage(track, &draws, star.rules).unwrap();
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
    /// Betelgeuse a supergiant, a luminous naked helium star a Wolf-Rayet star, a fainter one a
    /// stripped helium star and a faint one of up to 1.7 M☉ a hot subdwarf (P06.T24.c), and a
    /// cooling-fit object a brown dwarf below the hydrogen-burning limit.
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
        // A 10 M☉ helium star below the floor is no hot subdwarf (Götberg et al. 2018, A&A 615,
        // A78: subdwarf-like spectra only to 1.7 M☉), and a 0.5 M☉ one of 20 L☉ at 25,000 K is.
        assert_eq!(kind_of(&hot(6.0e4), &solar), ObjectKind::StrippedHeliumStar);
        let subdwarf = living(Phase::HeliumMainSequence, 0.5, 20.0, 0.24);
        assert_eq!(kind_of(&subdwarf, &solar), ObjectKind::HotSubdwarf);
        let poor = comp(-1.0);
        assert_eq!(kind_of(&hot(1.5e5), &poor), ObjectKind::StrippedHeliumStar);
        assert_eq!(kind_of(&hot(2.5e5), &poor), ObjectKind::WolfRayet);
        let cool = |m: f64| {
            substellar::cooling(SolarMasses::new(m), Years::new(5e9), &solar).expect("in the fits")
        };
        assert_eq!(kind_of(&cool(0.05), &solar), ObjectKind::Substellar);
        assert_eq!(kind_of(&cool(0.09), &solar), ObjectKind::Dwarf);
    }

    /// A system ID for tests that need a body and no record.
    fn test_system() -> crate::id::SystemId {
        let cell = crate::coords::GenCell::new(crate::coords::CellSize::Ly8, [1, 2, 3]).unwrap();
        crate::id::SystemId::from_parts(Layer::A, cell, 1).unwrap()
    }

    /// A neutron star dead at the epoch reads as its pulsar (P06.T21): the summary's detail is the
    /// model's pulsar state, its class is the pulsar's, the brief agrees, and its pulse phase is
    /// defined; a star that is still living has none of these.
    #[test]
    fn a_neutron_star_reads_as_its_pulsar() {
        let ns = StarModel::new(
            SolarMasses::new(15.0),
            Composition::SOLAR,
            StarDraws::median(),
            Years::new(2.0e7),
        )
        .unwrap();
        let t = UniverseTime::EPOCH;
        let summary = star_summary(&ns, streams(BodyId::new(test_system(), 0)), t).unwrap();
        assert_eq!(summary.state().phase(), Phase::NeutronStar);
        let pulsar = ns.pulsar_at(t).unwrap();
        assert_eq!(
            summary.remnant_detail(),
            Some(RemnantDetail::NeutronStar(pulsar))
        );
        let since = ns.remnant_age_at(t).unwrap();
        assert_eq!(pulsar, ns.neutron_star().unwrap().state_at(since));
        assert_eq!(
            summary.classification().spectral_type(),
            crate::stellar::classify::SpectralType::NeutronStar(pulsar.class())
        );
        let brief = StellarBrief::of(
            summary.state(),
            ns.composition(),
            ns.draws(),
            ns.class_extras_at(summary.state(), t, Pulses::Unopened),
            (1, ns.rules),
        );
        assert_eq!(brief.class(), summary.classification());
        assert!((0.0..1.0).contains(&ns.pulsar_phase_at(t).unwrap()));
        assert_eq!(summary.rotation(), None);
        let living = model(0);
        assert_eq!(living.pulsar_at(t), None);
        assert_eq!(living.pulsar_phase_at(t), None);
    }

    /// The remnant's age at the epoch (P06.T28.b) is the signed age the pulse clock is referred
    /// to: negative for a death after the epoch, `None` for a living star and for an object below
    /// 0.1 M☉, and the pulse phase built from it is `pulsar_phase_at`'s, bit for bit.
    #[test]
    fn the_remnant_age_at_the_epoch_is_the_pulse_clock_s() {
        let build = |mass: f64, age: f64| {
            StarModel::new(
                SolarMasses::new(mass),
                Composition::SOLAR,
                StarDraws::median(),
                Years::new(age),
            )
            .unwrap()
        };
        let life = build(15.0, 1e9).lifetime().unwrap().value();
        for since_death in [-500.0, 0.0, 1.1e4] {
            let star = build(15.0, life + since_death);
            let age = star.remnant_age_at_epoch().unwrap();
            assert!((age.value() - since_death).abs() < 1e-6, "{age:?}");
            let clock = star.neutron_star().unwrap().pulse_clock(age).unwrap();
            let after = |years: i64| {
                clock
                    .reference()
                    .max(UniverseTime::EPOCH)
                    .checked_add(Span::from_julian_years(years).unwrap())
                    .unwrap()
            };
            for t in [after(1), after(10), after(400)] {
                assert_same_bits(clock.phase_at(t), star.pulsar_phase_at(t).unwrap());
            }
        }
        assert_eq!(build(15.0, 1e6).remnant_age_at_epoch(), None);
        assert_eq!(build(0.05, 1e9).remnant_age_at_epoch(), None);
    }

    /// A black hole's detail is its spin from `star.bh.spin` (P06.T22), constant in time.
    #[test]
    fn a_black_hole_carries_its_spin() {
        let bh = StarModel::new(
            SolarMasses::new(40.0),
            Composition::SOLAR,
            StarDraws::from_parts(StarDrawsParts {
                bh_spin: StandardNormal::new(-1.5).unwrap(),
                ..StarDrawsParts::MEDIAN
            }),
            Years::new(2.0e7),
        )
        .unwrap();
        let body = BodyId::new(test_system(), 0);
        let now = star_summary(&bh, streams(body), UniverseTime::EPOCH).unwrap();
        let later = star_summary(&bh, streams(body), years(900)).unwrap();
        assert_eq!(now.state().phase(), Phase::BlackHole);
        let Some(RemnantDetail::BlackHole(hole)) = now.remnant_detail() else {
            panic!("{:?}", now.remnant_detail());
        };
        assert!((hole.spin() - 0.15).abs() < 1e-15);
        assert_eq!(hole.mass(), now.remnant().unwrap().mass());
        assert_eq!(later.remnant_detail(), now.remnant_detail());
        assert_eq!(later.state().luminosity(), SolarLuminosities::ZERO);
    }

    /// A star's rotation is continuous across the end of its main sequence (P06.T25): the evolved
    /// star keeps the angular momentum the main-sequence law gives it at the end, above and below
    /// the Kraft break.
    #[test]
    fn rotation_is_continuous_across_the_end_of_the_main_sequence() {
        for m in [1.0, 2.5] {
            let full = Track::full(
                SolarMasses::new(m),
                &Composition::SOLAR,
                &StarDraws::median(),
            );
            let end = full.main_sequence_end().unwrap().value();
            let spin = |age: f64| {
                StarModel::new(
                    SolarMasses::new(m),
                    Composition::SOLAR,
                    StarDraws::median(),
                    Years::new(age),
                )
                .unwrap()
                .rotation_at(UniverseTime::EPOCH)
                .unwrap()
            };
            let (before, after) = (spin(end - 1.0), spin(end + 1.0));
            let ratio = after.period().value() / before.period().value();
            assert!(
                (ratio - 1.0).abs() < 1e-3,
                "{m} M☉: {before:?} then {after:?}"
            );
            let giant = spin(end * 1.1);
            assert!(giant.period() > after.period(), "{m} M☉: {giant:?}");
        }
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

    // -----------------------------------------------------------------------------------------
    // P11.T11: the binary engine in the system stage.

    use crate::stellar::binary::{SegmentKind, evolve};
    use crate::stellar::classify::{WolfRayetSequence, carbon_shows_after};

    /// A grid record of the young thin disc at the Sun-like point, candidate `index`, with a
    /// primary of `mass` M☉ formed `age` years ago.
    fn record_of(galaxy: &Galaxy, index: u32, mass: f64, age: f64) -> SystemRecord {
        SystemRecord::from_parts(
            young_disc_record(galaxy, index, age).id(),
            GalacticPosition::from_light_years([0.0, 26_000.0, 30.0]).unwrap(),
            SystemOrigin::Grid(galaxy.fields().component_id(0).unwrap()),
            Population::YoungThinDisc,
            SolarMasses::new(mass),
            Years::new(age),
        )
    }

    /// Systems of 5 M☉ primaries 120 Myr old, past their main sequence, many of whose pairs
    /// interact.
    fn evolved_systems(galaxy: &Galaxy, n: u32) -> Vec<SystemStars> {
        (0..n)
            .map(|i| SystemStars::generate(galaxy, &record_of(galaxy, i, 5.0, 1.2e8)))
            .collect()
    }

    /// The input the engine takes for the pair at `node` of `stars`, and the age it runs to, or
    /// `None` for a node that is not a pair of two stars.
    fn pair_input(stars: &SystemStars, node: NodeIndex) -> Option<(BinaryInput, Years)> {
        let tree = stars.hierarchy();
        let HierarchyNode::Pair {
            inner,
            outer,
            orbit,
        } = *tree.node(node)
        else {
            return None;
        };
        let (HierarchyNode::Star(first), HierarchyNode::Star(second)) =
            (*tree.node(inner), *tree.node(outer))
        else {
            return None;
        };
        if [first, second]
            .iter()
            .any(|&s| tree.star(s).kind() != SlotKind::Star)
        {
            return None;
        }
        let [x, y] = [first, second].map(|s| &stars.stars()[usize::from(s.get())]);
        let input = BinaryInput::new(
            x.initial_mass(),
            y.initial_mass(),
            *x.composition(),
            orbit,
            [x.draws().clone(), y.draws().clone()],
            x.age_at_epoch(),
        )
        .unwrap();
        Some((input, x.age_at(ClockWindow::END)))
    }

    /// Checks that every pair of two stars of `stars` that can interact by +H or holds a remnant
    /// then, and no other, is run, with `evolve`'s timeline bit for bit, and that every other is
    /// one detached segment in no class; the pairs run and those that interacted.
    fn check_pairs(stars: &SystemStars) -> (usize, usize) {
        let (mut run, mut changed) = (0, 0);
        for (node, _) in stars.hierarchy().pairs() {
            let Some((input, until)) = pair_input(stars, node) else {
                assert!(stars.pairs().iter().all(|p| p.node() != node));
                continue;
            };
            let expected = evolve(&input, until);
            if let Some(pair) = stars.pairs().iter().find(|p| p.node() == node) {
                assert_same_bits(pair.timeline().until().value(), until.value());
                assert_eq!(pair.timeline(), &expected);
                run += 1;
                changed += usize::from(expected.segments().len() > 1);
            } else {
                {
                    assert_eq!(expected.segments().len(), 1, "{node:?}");
                    assert_eq!(expected.segments()[0].kind(), SegmentKind::Detached);
                    assert!(!crate::stellar::binary::can_interact(&input, until));
                    let age = stars.record().age_at_epoch();
                    if age.value() > 0.0 {
                        assert_eq!(
                            expected.class_at(age),
                            crate::stellar::binary::BinaryClass::None
                        );
                    }
                }
            }
        }
        (run, changed)
    }

    /// Every pair of two stars that can interact by +H or holds a remnant then, and no other, is
    /// run through the engine on its stars' own tracks, from zero age to its age at +H, and the
    /// timeline is `evolve`'s bit for bit: the tracks handed in save their builds and change
    /// nothing (ruling 111.5). The rest are two single stars on an orbit.
    #[test]
    fn every_pair_of_two_stars_is_run_on_its_stars_own_tracks() {
        let galaxy = galaxy();
        let (mut run, mut changed) = (0, 0);
        for stars in evolved_systems(&galaxy, 48) {
            let (r, c) = check_pairs(&stars);
            run += r;
            changed += c;
        }
        assert!(
            run > 10 && changed > 3,
            "{run} pairs run, {changed} interacting"
        );
    }

    /// The same for massive primaries, whose death design note 16 pins: alive at +H, so that the
    /// engine builds the full track itself, and dead by then, so that it takes the model's.
    #[test]
    fn massive_pairs_are_run_as_evolve_runs_them() {
        let galaxy = galaxy();
        let mut run = 0;
        for (mass, age) in [(12.0, 5.0e6), (12.0, 3.0e7), (30.0, 2.0e6), (30.0, 1.2e7)] {
            for i in 0..6 {
                let stars = SystemStars::generate(&galaxy, &record_of(&galaxy, 40 + i, mass, age));
                run += check_pairs(&stars).0;
            }
        }
        assert!(run > 4, "{run} massive pairs run");
    }

    fn is_helium_star(phase: Phase) -> bool {
        matches!(
            phase,
            Phase::HeliumMainSequence | Phase::HeliumHertzsprungGap | Phase::HeliumGiantBranch
        )
    }

    /// The time at which `stars`' stars are `age` years old.
    fn time_at_age(stars: &SystemStars, age: f64) -> UniverseTime {
        let since = (age - stars.primary().age_at_epoch().value())
            * crate::units::consts::SECONDS_PER_JULIAN_YEAR;
        UniverseTime::EPOCH
            .checked_add(Span::from_seconds_f64(since).expect("a finite span"))
            .expect("on the clock")
    }

    /// The ages, years, from the first to the last of 4,000 samples of `timeline` at which its
    /// primary is a naked helium star, or `None` if it never is one.
    fn helium_life(timeline: &BinaryTimeline) -> Option<(f64, f64)> {
        let until = timeline.until().value();
        let mut ages = (0..4_000).map(|k| until * (f64::from(k) + 0.5) / 4_000.0);
        let helium =
            |age: &f64| is_helium_star(timeline.state_at(Years::new(*age)).stars()[0].phase());
        Some((ages.find(helium)?, ages.rfind(helium)?))
    }

    /// The Wolf-Rayet sequences and masses of `seen`, in age order: WN while the mass is above
    /// `onset`, the mass at which the carbon shows, and WC below it (WO aside).
    fn assert_carbon_shows_at(seen: &[(WolfRayetSequence, f64)], onset: f64, what: &str) {
        for &(sequence, mass) in seen {
            match sequence {
                WolfRayetSequence::Nitrogen => assert!(mass >= onset, "{what}: WN at {mass} M☉"),
                WolfRayetSequence::Carbon => assert!(mass < onset, "{what}: WC at {mass} M☉"),
                WolfRayetSequence::Oxygen => {}
            }
        }
    }

    /// P11.T11.b: a naked helium star its pair has stripped carries the mass it had then, so its
    /// surface follows the single star's rule. Among the 60 M☉ primaries of reference systems
    /// 40–63, each one its pair has changed into a helium star is classified, in its summary, as
    /// `classify` gives its state with `HeliumSurface::of` its entry mass, which holds through its
    /// helium-star life; and one at least is WN early and WC once it has lost the share
    /// `carbon_shows_after` gives, at the mass at which a single helium star of the same entry mass
    /// on its own track turns WC too (Woosley 2019, ApJ 878, 49: stripped helium stars from about
    /// 9–11 M☉ turn WC). Before T11.b a paired helium star read WN for life.
    #[test]
    fn a_paired_helium_star_turns_wc_as_a_single_one_does() {
        let galaxy = galaxy();
        let (mut candidates, mut both) = (0, 0);
        for i in 0..24 {
            let stars = SystemStars::generate(&galaxy, &record_of(&galaxy, 40 + i, 60.0, 2.0e7));
            let Some(pair) = stars
                .pairs()
                .iter()
                .find(|p| p.stars()[0] == StarIndex::PRIMARY)
            else {
                continue;
            };
            let Some((first, last)) = helium_life(pair.timeline()) else {
                continue;
            };
            let primary = stars.primary();
            let (composition, draws) = (primary.composition(), primary.draws());
            let mut entry = None;
            let mut seen = Vec::new();
            for k in 0..400 {
                let t = time_at_age(&stars, first + (last - first) * f64::from(k) / 399.0);
                let paired = stars
                    .paired_state(0, t)
                    .expect("inside the pair's timeline");
                if !is_helium_star(paired.state.phase())
                    || primary.state_at(t) == Some(paired.state)
                {
                    continue;
                }
                let at_entry = paired
                    .helium_entry
                    .expect("a helium star its pair made carries its entry mass");
                assert_eq!(
                    *entry.get_or_insert(at_entry),
                    at_entry,
                    "one life, one entry"
                );
                let state = paired.state;
                let surface = HeliumSurface::of(at_entry, state.mass());
                let expected = classify(
                    &state,
                    composition,
                    draws,
                    &ClassExtras::helium_star(surface),
                );
                let summary = stars.summary_at(t);
                assert_eq!(
                    summary.stars()[0].classification(),
                    expected,
                    "{i} at {t:?}"
                );
                if let SpectralType::WolfRayet(wr) = expected.spectral_type() {
                    seen.push((wr.sequence(), state.mass().value()));
                }
            }
            let Some(entry) = entry else {
                continue;
            };
            candidates += 1;
            let onset = (1.0 - carbon_shows_after(entry)) * entry.value();
            assert_carbon_shows_at(&seen, onset, "paired");
            let has = |sequence| seen.iter().any(|&(s, _)| s == sequence);
            if !(has(WolfRayetSequence::Nitrogen) && has(WolfRayetSequence::Carbon)) {
                continue;
            }
            both += 1;
            // A single helium star of the same entry mass turns WC at the same mass.
            let single = Track::helium_star_full(entry, composition, draws);
            let death = single.lifetime().expect("dies").value();
            let mut alone = Vec::new();
            for k in 0..2_000 {
                let state = single.state_at(Years::new(death * (f64::from(k) + 0.5) / 2_000.0));
                if !is_helium_star(state.phase()) {
                    continue;
                }
                let surface = HeliumSurface::of(entry, state.mass());
                let class = classify(
                    &state,
                    composition,
                    draws,
                    &ClassExtras::helium_star(surface),
                );
                if let SpectralType::WolfRayet(wr) = class.spectral_type() {
                    alone.push((wr.sequence(), state.mass().value()));
                }
            }
            assert!(
                alone.iter().any(|&(s, _)| s == WolfRayetSequence::Carbon),
                "{alone:?}"
            );
            assert_carbon_shows_at(&alone, onset, "single");
        }
        assert!(
            candidates > 3 && both > 0,
            "{candidates} stripped, {both} WN then WC"
        );
    }

    /// P11.T11.b: a merger's product, whose mass the binary sets on its own track, becomes a naked
    /// helium star where that track strips it, inside a segment: its entry mass is its mass there,
    /// found by bisection, and it holds over the segments that follow, the hold before a pinned
    /// collapse among them (reference systems 40–63 of 30 and 60 M☉). Before it, no entry.
    #[test]
    fn a_merger_products_helium_entry_is_its_mass_where_its_track_strips_it() {
        let galaxy = galaxy();
        let (mut found, mut held) = (0, 0);
        for (mass, i) in (0..24).flat_map(|i| [(30.0, i), (60.0, i)]) {
            let stars = SystemStars::generate(&galaxy, &record_of(&galaxy, 40 + i, mass, 2.0e7));
            let Some(pair) = stars
                .pairs()
                .iter()
                .find(|p| p.stars()[0] == StarIndex::PRIMARY)
            else {
                continue;
            };
            let timeline = pair.timeline();
            let Some((first, last)) = helium_life(timeline) else {
                continue;
            };
            let star_at = |age: f64| timeline.state_at(Years::new(age)).stars()[0];
            // The onset to ten years: the first sample, walked back while it is a helium star.
            let mut onset = first;
            while is_helium_star(star_at(onset - 10.0).phase()) {
                onset -= 10.0;
            }
            let segment_of = |age: f64| {
                timeline
                    .segments()
                    .iter()
                    .rev()
                    .find(|s| s.start().value() <= age)
                    .expect("a segment from zero age")
            };
            let member = &segment_of(onset).members()[0];
            // A star whose mass the binary sets on a track not a helium star's at its start.
            let reshaped = member.form().starts_with("shaped")
                && member.track().is_some_and(|(track, _)| {
                    !is_helium_star(track.state_at(track.built_from()).phase())
                });
            if !reshaped {
                continue;
            }
            let entry_at =
                |age: f64| timeline.helium_star_entry_mass(PairComponent::Primary, Years::new(age));
            assert_eq!(entry_at(onset - 10.0), None, "{mass} M☉, {i}");
            let entry = entry_at(onset).expect("a helium star has an entry mass");
            let (after, before) = (
                star_at(onset).mass().value(),
                star_at(onset - 10.0).mass().value(),
            );
            assert!(
                (after..=before).contains(&entry.value()),
                "{mass} M☉, {i}: entry {entry:?} outside {after}–{before}"
            );
            for k in 0..200 {
                let age = onset + (last - onset) * f64::from(k) / 199.0;
                assert_eq!(entry_at(age), Some(entry), "{mass} M☉, {i} at {age}");
                held += usize::from(segment_of(age).members()[0].form() == "frozen");
            }
            found += 1;
        }
        assert!(
            found > 0 && held > 0,
            "{found} merger products, {held} held samples"
        );
    }

    /// `state_at` reads a paired star from its pair's timeline and any other from its own model,
    /// gives each pair its orbit and class then, and sums the stars' light and mass; the summary
    /// agrees star for star and gives each paired star its pair's class.
    #[test]
    fn the_state_reads_paired_stars_from_their_timeline_and_sums_the_system() {
        let galaxy = galaxy();
        let t = UniverseTime::EPOCH;
        let mut touched = 0;
        for stars in evolved_systems(&galaxy, 48) {
            let state = stars.state_at(t).expect("born 120 Myr ago");
            assert_eq!(state.stars().len(), usize::from(stars.star_count()));
            let summary = stars.summary_at(t);
            for (k, (model, now)) in stars.stars().iter().zip(state.stars()).enumerate() {
                let pair = stars
                    .pairs()
                    .iter()
                    .find(|p| p.stars().iter().any(|s| usize::from(s.get()) == k));
                let expected = if let Some(pair) = pair {
                    let c = pair
                        .stars()
                        .iter()
                        .position(|s| usize::from(s.get()) == k)
                        .unwrap();
                    let binary = stars.binary_state_at(pair.node(), t).unwrap();
                    assert_eq!(
                        summary.stars()[k].binary_class(),
                        if now.phase() == Phase::NoRemnant {
                            crate::stellar::binary::BinaryClass::None
                        } else {
                            pair.timeline().class_at(model.age_at(t))
                        }
                    );
                    binary.stars()[c]
                } else {
                    assert_eq!(
                        summary.stars()[k].binary_class(),
                        crate::stellar::binary::BinaryClass::None
                    );
                    model.state_at(t).unwrap()
                };
                assert_eq!(*now, expected);
                assert_eq!(summary.stars()[k].state(), now);
                touched += usize::from(model.state_at(t).unwrap() != *now);
            }
            let light = state
                .stars()
                .iter()
                .fold(0.0, |sum, s| sum + s.luminosity().value());
            assert_same_bits(state.luminosity().value(), light);
            assert_same_bits(stars.system_mass_at(t).value(), state.mass().value());
            for (pair_state, (node, orbit)) in state.pairs().iter().zip(stars.hierarchy().pairs()) {
                assert_eq!(pair_state.node(), node);
                match stars.binary_state_at(node, t) {
                    Some(binary) => assert_eq!(pair_state.orbit(), binary.orbit()),
                    None => assert_eq!(pair_state.orbit(), Some(orbit)),
                }
            }
        }
        assert!(touched > 0, "no star of 48 systems was changed by its pair");
        let unborn = SystemStars::generate(&galaxy, &record_of(&galaxy, 3, 1.0, -200.0));
        assert_eq!(unborn.state_at(t), None);
        assert_eq!(unborn.system_mass_at(t), SolarMasses::ZERO);
    }

    /// `state_at` is continuous in time across the clock window: between two times a second apart
    /// no star's luminosity or mass jumps by more than 1% unless its phase or its pair's segment
    /// changes between them (the listed events).
    #[test]
    fn the_state_is_continuous_across_the_window_except_at_listed_events() {
        let galaxy = galaxy();
        let second = Span::new(1, 0).unwrap();
        let window = 2_000_i64;
        for stars in evolved_systems(&galaxy, 16) {
            let kinds = |t: UniverseTime| -> Vec<Option<SegmentKind>> {
                stars
                    .pairs()
                    .iter()
                    .map(|p| stars.binary_state_at(p.node(), t).map(|s| s.kind()))
                    .collect()
            };
            for k in 0..=40 {
                let t = years(-1_000 + window * k / 41);
                let later = t.checked_add(second).unwrap();
                if kinds(t) != kinds(later) {
                    continue;
                }
                let (a, b) = (stars.state_at(t).unwrap(), stars.state_at(later).unwrap());
                for (x, y) in a.stars().iter().zip(b.stars()) {
                    if x.phase() != y.phase() {
                        continue;
                    }
                    for (p, q) in [
                        (x.luminosity().value(), y.luminosity().value()),
                        (x.mass().value(), y.mass().value()),
                    ] {
                        assert!(
                            (p - q).abs() <= 1e-2 * p.abs().max(1e-30),
                            "{p} then {q} at {t:?}"
                        );
                    }
                }
            }
        }
    }

    /// The primary's recoil is its pair's, and none for a single star.
    #[test]
    fn the_recoil_is_the_primarys_pairs() {
        let galaxy = galaxy();
        for stars in evolved_systems(&galaxy, 16) {
            let expected = stars
                .pairs()
                .iter()
                .find(|p| p.stars()[0] == StarIndex::PRIMARY)
                .and_then(|p| p.timeline().recoil());
            assert_eq!(stars.recoil(), expected);
        }
        let record = young_disc_record(&galaxy, 4, 1.0e9);
        let single = SystemStars::generate_in(&galaxy, &record, MultiplicityContext::ForcedSingle);
        assert_eq!(single.recoil(), None);
        assert_eq!(single.pairs(), []);
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

    /// A 2 M☉ star `before_death` years before its white dwarf's birth at the epoch.
    fn two_solar_masses_before_death(before_death: f64) -> StarModel {
        let old = StarModel::new(
            SolarMasses::new(2.0),
            Composition::SOLAR,
            StarDraws::median(),
            Years::new(5e9),
        )
        .unwrap();
        let death = old.lifetime().expect("a 2 M☉ star dies");
        StarModel::new(
            SolarMasses::new(2.0),
            Composition::SOLAR,
            StarDraws::median(),
            Years::new(death.value() - before_death),
        )
        .unwrap()
    }

    fn seconds_from_epoch(seconds: i64) -> UniverseTime {
        UniverseTime::EPOCH
            .checked_add(Span::from_seconds(seconds))
            .unwrap()
    }

    /// P14.T45.d: the split-age mass has `state_at`'s phase, its mass bit for bit at the epoch
    /// and for a remnant, and elsewhere differs from it by under one step of the summed age's
    /// staircase, Ṁ × one unit in the last place of the age, while falling every second.
    #[test]
    fn the_split_age_mass_is_the_state_s_without_its_staircase() {
        let star = two_solar_masses_before_death(2_775.0);
        let now = star.state_at(UniverseTime::EPOCH).unwrap();
        assert_eq!(now.phase(), Phase::ThermallyPulsingAgb);
        let (phase, mass) = star.phase_and_mass_at(UniverseTime::EPOCH).unwrap();
        assert_eq!(phase, now.phase());
        assert_same_bits(mass.value(), now.mass().value());
        let step = now.mass_loss_rate().value() * f64::EPSILON * star.age_at_epoch().value();
        let (mut previous, mut stairs) = (mass.value(), 0);
        let mut last_state = now.mass().value();
        for second in 1..=64 {
            let t = seconds_from_epoch(1_000_000 + second);
            let state = star.state_at(t).unwrap();
            let (phase, mass) = star.phase_and_mass_at(t).unwrap();
            assert_eq!(phase, state.phase());
            assert!(
                (mass.value() - state.mass().value()).abs() <= 2.0 * step,
                "{t}: {} against {}",
                mass.value(),
                state.mass().value()
            );
            assert!(
                mass.value() < previous,
                "{t}: the split mass falls every second"
            );
            stairs += u32::from(state.mass().value().total_cmp(&last_state).is_eq());
            previous = mass.value();
            last_state = state.mass().value();
        }
        // The summed age holds a time to about 7.5 s here, so its mass repeats.
        assert!(stairs > 32, "{stairs} repeats of the summed age's mass");
        let white_dwarf = two_solar_masses_before_death(-2e4);
        for t in [UniverseTime::EPOCH, seconds_from_epoch(123_456_789)] {
            let state = white_dwarf.state_at(t).unwrap();
            let (phase, mass) = white_dwarf.phase_and_mass_at(t).unwrap();
            assert_eq!(phase, state.phase());
            assert_same_bits(mass.value(), state.mass().value());
        }
    }

    /// P14.T45.d: a star's mass breaks in a span are its track's knot ages and segment boundaries
    /// there, mapped to the clock: strictly inside the span, ascending and without repeats.
    #[test]
    fn mass_breaks_are_the_track_s_inside_the_span_ascending() {
        // Late on the thermally pulsing AGB, whose knots crowd towards its end, over the ten
        // millennia to the end of the clock window.
        let star = two_solar_masses_before_death(2_775.0);
        let (from, to) = (
            seconds_from_epoch(-315_576_000_000),
            seconds_from_epoch(31_557_600_000),
        );
        let breaks = star.mass_breaks(from, to);
        let Evolution::Track(track) = &star.evolution else {
            panic!("a 2 M☉ star follows a track")
        };
        let ages: Vec<f64> = track
            .mass_breaks(star.age_at(from).value(), star.age_at(to).value())
            .collect();
        assert!(breaks.len() > 5, "{} breaks", breaks.len());
        assert!(breaks.len() <= ages.len());
        assert!(breaks.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(breaks.iter().all(|&b| from < b && b < to));
        // Each break is a knot's or a boundary's age, to the rounding of the age.
        for b in &breaks {
            let age = star.age_at(*b).value();
            let nearest = ages
                .iter()
                .map(|a| (a - age).abs())
                .fold(f64::INFINITY, f64::min);
            assert!(nearest <= 4.0 * f64::EPSILON * age, "{b}: {nearest} yr off");
        }
    }

    /// [`RemnantFate`] is the star model's remnant, and judges its kick as the model's would, bit
    /// for bit: over 0.05–120 M☉, at the solar composition and at a metal-poor one with a helium
    /// excess (whose stripped share takes the exact path), at ages where the star is young, has
    /// just died and is long dead, with the companion-stripped mark never set, always set and as
    /// drawn, for kicks judged against speeds from 0.5 to 300 km/s and a low-mode kick on a pair's
    /// systemic speed of 2 or 30 km/s (P09.T10's member draws ask it of every remnant attempt).
    #[test]
    fn a_remnant_fate_is_the_star_model_s_remnant_and_kick() {
        use crate::galaxy::displaced::binarity::NEVER_STRIPPED;
        use crate::stellar::draws::StarDrawsParts;
        use crate::stellar::remnant::KickMode;
        use crate::units::KilometresPerSecond;

        let rules = StellarRules::of_version(GENERATOR_VERSION);
        let helium = Composition::from_fe_h(Dex::new(-0.65), HeliumExcess::new(0.04));
        let masses = [0.05, 0.9, 2.5, 6.6, 7.4, 8.25, 9.0, 10.9, 15.0, 30.0, 120.0];
        let marks = [
            NEVER_STRIPPED,
            Mark::from_word(0),
            StarDrawsParts::MEDIAN.stripped,
        ];
        for comp in [Composition::SOLAR, helium] {
            for m0 in masses.map(SolarMasses::new) {
                for age in [3.0e7, 1.0e9, 1.28e10].map(Years::new) {
                    for (i, mark) in marks.into_iter().enumerate() {
                        let parts = StarDraws::for_attempt(
                            Seed::new(SEED),
                            BodyId::new(
                                crate::id::SystemId::from_raw(0x4200_2cb2_0000_0001).unwrap(),
                                0,
                            ),
                            u32::try_from(i).unwrap(),
                        )
                        .parts()
                        .clone();
                        let draws = StarDraws::from_parts(StarDrawsParts {
                            stripped: mark,
                            ..parts
                        });
                        let what = format!("{} M☉, {comp:?}, {age:?}, mark {mark:?}", m0.value());
                        let star =
                            StarModel::new_under(m0, comp, draws.clone(), age, rules).unwrap();
                        let fate = RemnantFate::of(m0, comp, draws, age, rules).unwrap();
                        assert_eq!(
                            fate.as_ref().map(RemnantFate::remnant),
                            star.remnant(),
                            "{what}"
                        );
                        let Some(fate) = fate else {
                            continue;
                        };
                        for (v_eff, systemic) in [0.5, 3.0, 18.6, 40.0, 300.0]
                            .into_iter()
                            .flat_map(|v| [(v, 2.0), (v, 30.0)])
                        {
                            let fits = |kick: &NatalKick| {
                                let judged = if kick.mode() == KickMode::Low {
                                    systemic
                                } else {
                                    KilometresPerSecond::from(kick.speed()).value()
                                };
                                judged < v_eff
                            };
                            assert_eq!(
                                fate.kick_is_none_or(fits),
                                star.natal_kick().is_none_or(|kick| fits(&kick)),
                                "{what}, {v_eff} km/s, {systemic} km/s"
                            );
                        }
                    }
                }
            }
        }
    }
}
