//! A range query's brief of a system, routed by whether its pair can have changed its primary
//! (plan 11, P11.T11.a) and by what that primary is (plan 06, P06.T38.e; rulings 89, 90 and 99).
//!
//! A range row's [`StellarBrief`] needs the primary's state at the query's time, as the system's
//! summary gives it ([`SystemStars::brief_at`]), and the system's star count, and nothing else of
//! the system: no companion's state, no orbit, no photometry. The [`BriefModel`] builds only that.
//!
//! **The gate** (P11.T11.a; `decision-observed-class-findings.md` §A). The model first decides
//! whether the primary is certainly its own single-star model at every time of the clock window.
//! It is if the system is single (plan 11's count-only draw,
//! [`draw_star_count`](crate::stellar::multiplicity::draw_star_count), is 1), or if no pair of two
//! stars in the system's first redraw attempt (attempt 0 of P11.T16's
//! [`hierarchy_bound`](crate::stellar::multiplicity::hierarchy_bound)) can change its stars or be
//! carved: then the generator keeps that attempt and the binary engine reaches no state of the
//! primary. For a grid system that is every such pair of the attempt, since a carved one would have
//! the system redrawn (P11.T7) and the primary might be in another pair at the attempt kept; for a
//! system the grid does not redraw, a feature member, only the pair the primary is in. A pair
//! passes where plan 11's census bound
//! ([`pair_light_bound`](crate::stellar::binary::pair_light_bound)) calls it `Detached` over the
//! window's ages and at most one of its stars may have left its main sequence by their end. A
//! certified primary takes the cheapest of three routes, which run no engine:
//!
//! - **The fate table** ([`BriefRoute::Table`]): a primary the fate table ([`FittedFates`]) finds
//!   dead through the whole clock window, by its death age's bound, is its remnant's closed form
//!   at the table's fate, and needs no track. A guard evaluates the brief across the table's error
//!   box and the window, and sends the star to the exact route wherever its kind or class could
//!   differ. Such a brief has the full system's kind and class exactly, and its luminosity and
//!   temperature within [`BriefModel::stated_error_at`] (ruling 90.4). It is asked first, since a
//!   dead star's main sequence would be built for nothing.
//! - **The main sequence without knots** ([`BriefRoute::MainSequence`]): a primary whose main
//!   sequence has no knots and lasts beyond the clock window's end is read from its main sequence
//!   alone, the build of [`main_sequence_state`](crate::stellar::sse::main_sequence_state)
//!   (P06.T38.b), which is its track's state bit for bit at every age of the window. About eight
//!   rows in ten near the Sun.
//! - **Exact** ([`BriefRoute::Exact`]): below 0.1 M☉, P06.T13's cooling fits; for a star the table
//!   found dead but whose guard failed, its exact remnant from a build to its death that keeps no
//!   other segment; and otherwise its [`StarModel`], as [`SystemStars::generate`] builds it.
//!
//! Every other system takes **the paired route** ([`BriefRoute::Paired`]): its [`SystemStars`],
//! binary engine included, whose brief is the summary's primary by construction, a redrawn
//! system's included.
//!
//! The star count is the first redraw attempt's: 1 for a system the count-only draw finds single
//! from its count words, and otherwise the count of the attempt the gate draws, which is
//! `draw_star_count`'s, so that no hierarchy is drawn twice and none for a single star. A
//! certified system keeps that attempt, so its count is the kept one; for the grid
//! systems that [`SystemStars::generate`] redraws because a pair fell into a carved class (P11.T7,
//! about 10⁻⁴ of them, every one on the paired route) it is the first attempt's, which may differ
//! from the kept attempt's (plan 11's Risks, "Deviations in P11.T7, as built"). The composition and
//! the primary's draws are the ones [`SystemStars::generate`] reads, from the same streams; a
//! main-sequence primary reads η alone, the one draw its state and class depend on.
//!
//! The exact, main-sequence and paired routes give the brief [`SystemStars::brief_at`] gives, bit
//! for bit, the paired route with the first attempt's count: a brief is `StellarBrief::of` the same
//! state, composition, draws and count. A [`BriefModel`] holds epoch state only and is the same
//! whatever was asked before, so a server may cache it by system.

use crate::galaxy::Galaxy;
use crate::galaxy::features::members::{FeatureInteriorCache, MemberRecord, resolve_member};
use crate::galaxy::placement::{SystemOrigin, SystemRecord};
use crate::id::SystemIdKind;
use crate::math;
use crate::rng::Mark;
use crate::stellar::Composition;
use crate::stellar::binary::carve::grid_redraws;
use crate::stellar::binary::pair_keeps_own_models;
use crate::stellar::classify::ClassExtras;
use crate::stellar::draws::{StandardNormal, StarDraws, StarDrawsParts, UnitUniform};
use crate::stellar::fates::{FateRoute, FittedFate, FittedFates, stripped_mark_matters};
use crate::stellar::multiplicity::{MultiplicityContext, StarIndex, drawn_single, first_attempt};
use crate::stellar::remnant::NeutronStar;
use crate::stellar::remnant::collapse::RemnantDraws;
use crate::stellar::remnant::wd_spectral::white_dwarf_type;
use crate::stellar::sse::{
    MAX_INITIAL_MASS, MIN_INITIAL_MASS, RemnantModel, Track, TrackOptions, remnant_of,
};
use crate::stellar::system::{
    GRID_ATTEMPT, StarModel, StellarBrief, SystemStars, draw_metallicity, grid_multiplicity,
    primary_draws, primary_eta, primary_rotation_draws,
};
use crate::stellar::{Phase, StarState};
use crate::time::{ClockWindow, UniverseTime};
use crate::units::{Kelvin, SolarMasses, Years};

/// Which way a [`BriefModel`] evaluates its primary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BriefRoute {
    /// A main sequence without knots that lasts beyond the clock window: its closed forms alone.
    MainSequence,
    /// A star dead through the whole clock window whose fate the fate table gives within its
    /// stated error, and whose kind and class the guard found the same at every corner of that
    /// error: its remnant's closed form ([`FittedFates`], P06.T38.d).
    Table,
    /// The primary's [`StarModel`]: the cooling fits below 0.1 M☉, and otherwise its track.
    Exact,
    /// A system whose pair may have changed its primary, or that the grid may redraw (P11.T11.a):
    /// its [`SystemStars`], binary engine included, read as its summary reads them.
    Paired,
}

/// The stated error of a table-routed brief at one time: the least and greatest log₁₀ L ÷ L☉
/// and `T_eff`, K, over the corners of the fate table's error box taken at [`STATED_FACTOR`]
/// times its bounds, widened by at least [`STATED_FLOOR_DEX`] and [`STATED_FLOOR_RELATIVE`],
/// between which the exact primary's lie (ruling 90.4). `None` for a remnant with no light.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StatedError {
    /// log₁₀ L ÷ L☉, least and greatest.
    pub log_luminosity: (f64, f64),
    /// `T_eff`, K, least and greatest.
    pub effective_temperature: (f64, f64),
}

/// What a range query's row needs of one grid system at any clock time: its primary as the
/// system's summary has it, by the cheapest of the module's routes, and its star count (plan 06,
/// P06.T38.e; plan 11, P11.T11.a).
///
/// It holds epoch state and nothing that depends on a time asked about, so it can be cached by
/// system and asked about any time of the clock window.
///
/// # Examples
///
/// The brief of each system of a cell at the solar circle has the kind and class of its
/// summary's primary, whatever the primary's companion has made of it, and is the full system's
/// brief bit for bit unless the fate table gave it:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::placement::{CellKey, generate_cell};
/// use hyperion_sim::id::Layer;
/// use hyperion_sim::stellar::brief::{BriefModel, BriefRoute};
/// use hyperion_sim::stellar::system::SystemStars;
/// use hyperion_sim::time::UniverseTime;
///
/// let galaxy = Galaxy::new(Seed::new(11));
/// let mut cell = Vec::new();
/// generate_cell(&galaxy, CellKey::new(Layer::C, [0, 812, 0])?, &mut cell);
/// for record in cell.iter().take(20) {
///     let model = BriefModel::new(&galaxy, record);
///     let brief = model.brief_at(UniverseTime::EPOCH).ok_or("formed")?;
///     let stars = SystemStars::generate(&galaxy, record);
///     let summary = stars.summary_at(UniverseTime::EPOCH);
///     let primary = summary.stars().first().ok_or("a primary")?;
///     assert_eq!(
///         (brief.kind(), brief.class().to_string()),
///         (primary.kind(), primary.classification().to_string())
///     );
///     // A system the grid redrew would count its first attempt's stars, which none of these is.
///     if model.route() != BriefRoute::Table {
///         assert_eq!(Some(brief), stars.brief_at(UniverseTime::EPOCH));
///     }
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct BriefModel {
    primary: Primary,
    star_count: u8,
}

/// A [`BriefModel`]'s primary.
#[derive(Debug, Clone, PartialEq)]
enum Primary {
    /// [`BriefRoute::MainSequence`]: the knot-free main sequence's one-segment track.
    MainSequence {
        track: Box<Track>,
        composition: Composition,
        /// The primary's η, the one draw its main sequence reads.
        eta: StandardNormal,
        /// Its rotation rank and fossil-field mark, which its peculiar class reads (P06.T25).
        rotation: (UnitUniform, Mark),
        age_at_epoch: Years,
    },
    /// [`BriefRoute::Table`]: the remnant's one-segment track, with the fate it was read from.
    Table {
        track: Box<Track>,
        fate: FittedFate,
        composition: Composition,
        draws: Box<StarDraws>,
        age_at_epoch: Years,
    },
    /// [`BriefRoute::Exact`] for a star dead through the whole window: its exact remnant's
    /// one-segment track, from a build that keeps nothing else.
    Remnant {
        track: Box<Track>,
        composition: Composition,
        draws: Box<StarDraws>,
        age_at_epoch: Years,
        /// The age at which the remnant formed, years since the onset of collapse.
        birth: f64,
    },
    /// [`BriefRoute::Exact`].
    Exact(Box<StarModel>),
    /// [`BriefRoute::Paired`]: the system's stars.
    Paired(Box<SystemStars>),
}

/// The factor on the fate table's validated bounds by which its stated error, and the margin by
/// which a star is taken as dead, exceed them: the bounds are validated, not proved (P06.T38.d's
/// test finds about one star in fifty past one of them), and three times them held every row
/// of the tests.
pub const STATED_FACTOR: f64 = 3.0;

/// The least relative margin past the fate table's death age at which a star is taken as dead
/// through the window: P06.T38.d's slow test found a 99 M☉ star's death age 10⁻³ off, six times
/// its cell's bound. A star dead for less than this share of its life takes the exact route.
pub const DEAD_MARGIN_FLOOR: f64 = 3e-3;

/// The least half-width of a stated error in log₁₀ L, dex: the table's error where its columns'
/// bounds all but cancel in the luminosity, as a cool white dwarf's do (P06.T38.e's slow test
/// found one 9 × 10⁻⁴ dex off). It is below the chart's and the readout's three figures.
pub const STATED_FLOOR_DEX: f64 = 1e-3;

/// The least relative half-width of a stated error in `T_eff`.
pub const STATED_FLOOR_RELATIVE: f64 = 1e-3;

/// The guard's shifts of the fate table's columns: the centre, then every corner of the error
/// box of the columns a route reads.
const WHITE_DWARF_CORNERS: [(f64, f64, f64); 9] = [
    (0.0, 0.0, 0.0),
    (-1.0, -1.0, -1.0),
    (-1.0, -1.0, 1.0),
    (-1.0, 1.0, -1.0),
    (-1.0, 1.0, 1.0),
    (1.0, -1.0, -1.0),
    (1.0, -1.0, 1.0),
    (1.0, 1.0, -1.0),
    (1.0, 1.0, 1.0),
];

/// An iron core's corners: its death age and its two cores; its remnant's kind is decided by the
/// cores alone.
const IRON_CORE_CORNERS: [(f64, f64, f64); 9] = WHITE_DWARF_CORNERS;

impl BriefModel {
    /// The brief model of the grid system `record` in `galaxy`: a star system's, or a free-floating
    /// brown dwarf's, a system of one (plan 13, P13.T5.a).
    ///
    /// # Panics
    ///
    /// - As [`SystemStars::generate`] does: for a record of another galaxy, and for a rogue
    ///   planet, whose mass is below the stellar stage's and which has no brief (plan 13,
    ///   P13.T5.d); a caller checks [`SystemRecord::kind`] first.
    /// - As [`pair_light_bound`](crate::stellar::binary::pair_light_bound), which the gate reads for
    ///   a multiple system, if a committed pair-light table cannot be read: a broken invariant,
    ///   which `hyperion-fit check` and the tests rule out.
    #[must_use]
    pub fn new(galaxy: &Galaxy, record: &SystemRecord) -> Self {
        Self::with_fates(galaxy, record, &FittedFates::generator())
    }

    /// [`BriefModel::new`] with the fate table `fates` rather than the generator's: for studying
    /// a candidate table on real rows before it is committed, as P06.T38.c's grid was chosen.
    ///
    /// # Panics
    ///
    /// As [`BriefModel::new`]: for a record of another galaxy, for a rogue planet, and if a
    /// committed pair-light table cannot be read.
    #[must_use]
    pub fn with_fates(galaxy: &Galaxy, record: &SystemRecord, fates: &FittedFates<'_>) -> Self {
        let composition = draw_metallicity(galaxy, record);
        Self::build(
            galaxy,
            record,
            (composition, grid_multiplicity(record)),
            fates,
        )
    }

    /// Whether [`BriefModel::new`] keeps the grid system `record`'s primary on one of its
    /// single-star routes, the module's gate (P11.T11.a), without building the model.
    ///
    /// It is true for a single system, or one no pair of whose first redraw attempt can change its
    /// stars or be carved, and is `BriefModel::new(galaxy, record).route() != BriefRoute::Paired`
    /// for every record `new` takes: for timing the gate alone. A rogue planet, which has no brief,
    /// is a single object, and is certified.
    ///
    /// # Panics
    ///
    /// As [`BriefModel::new`]: for a record of another galaxy, and if a committed pair-light table
    /// cannot be read.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::Seed;
    /// use hyperion_sim::galaxy::Galaxy;
    /// use hyperion_sim::galaxy::placement::{CellKey, generate_cell};
    /// use hyperion_sim::id::Layer;
    /// use hyperion_sim::stellar::brief::{BriefModel, BriefRoute};
    ///
    /// let galaxy = Galaxy::new(Seed::new(11));
    /// let mut cell = Vec::new();
    /// generate_cell(&galaxy, CellKey::new(Layer::C, [0, 812, 0])?, &mut cell);
    /// for record in cell.iter().take(10) {
    ///     let paired = BriefModel::new(&galaxy, record).route() == BriefRoute::Paired;
    ///     assert_eq!(BriefModel::certifies(&galaxy, record), !paired);
    /// }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn certifies(galaxy: &Galaxy, record: &SystemRecord) -> bool {
        let composition = draw_metallicity(galaxy, record);
        gate(galaxy, record, &composition, grid_multiplicity(record)).certified
    }

    /// The brief model of the feature member `member` in `galaxy` (plan 09, P09.T10): its
    /// [`MemberRecord::stars`]' brief by the module's routes, at the member's composition and under
    /// its [`multiplicity_context`](MemberRecord::multiplicity_context), where
    /// [`BriefModel::new`] would read a grid system's metallicity draw, which a member has no
    /// density component for.
    ///
    /// # Panics
    ///
    /// As [`MemberRecord::stars`] does, and as [`BriefModel::new`] if a committed pair-light table
    /// cannot be read.
    #[must_use]
    pub fn of_member(galaxy: &Galaxy, member: &MemberRecord) -> Self {
        Self::build(
            galaxy,
            member.record(),
            (*member.composition(), member.multiplicity_context()),
            &FittedFates::generator(),
        )
    }

    /// The brief model of `record`'s system in `galaxy`, whatever placed it, as
    /// [`stars_of`](crate::observe::stars_of) builds its stars: a grid system's
    /// [`BriefModel::new`], a feature member's [`BriefModel::of_member`], its feature's interior
    /// taken from `interiors`.
    ///
    /// # Panics
    ///
    /// - For a rogue planet, as [`BriefModel::new`], and for a member of the galactic centre,
    ///   whose stars wait for plan 09's centre composition.
    /// - If `record` is a feature member that does not resolve in `galaxy`, which only a record of
    ///   another galaxy can be.
    /// - As [`BriefModel::new`], if a committed pair-light table cannot be read.
    #[must_use]
    pub fn of_record(
        galaxy: &Galaxy,
        interiors: &dyn FeatureInteriorCache,
        record: &SystemRecord,
    ) -> Self {
        match record.origin() {
            SystemOrigin::Grid(_) => Self::new(galaxy, record),
            SystemOrigin::FeatureMember { .. } => {
                let SystemIdKind::FeatureMember(id) = record.id().kind() else {
                    unreachable!("a feature member's record is built with a member ID")
                };
                let member = resolve_member(galaxy, interiors, id)
                    .expect("a feature member's record resolves in the galaxy that placed it");
                Self::of_member(galaxy, &member)
            }
            SystemOrigin::CentreMember { .. } => {
                panic!(
                    "a galactic-centre member's stars are not generated yet: {:?}",
                    record.id()
                )
            }
        }
    }

    /// The model of `record` at `composition` under the multiplicity context `ctx`: its system's
    /// stars where the gate does not certify its primary, and otherwise its primary by the
    /// cheapest route `fates` allows, with its first attempt's star count either way.
    fn build(
        galaxy: &Galaxy,
        record: &SystemRecord,
        (composition, ctx): (Composition, MultiplicityContext),
        fates: &FittedFates<'_>,
    ) -> Self {
        let Gate {
            star_count,
            certified,
        } = gate(galaxy, record, &composition, ctx);
        if !certified {
            // `SystemStars::generate` for a grid record, `MemberRecord::stars` for a member: the
            // same composition and context, so the same system.
            let stars = SystemStars::generate_with(galaxy, record, &composition, ctx);
            return Self {
                primary: Primary::Paired(Box::new(stars)),
                star_count,
            };
        }
        let m0 = record.primary_initial_mass();
        let age_at_epoch = record.age_at_epoch();
        // The main sequence reads η alone of the primary's draws: its one stream, not all of
        // them (a quarter of a main-sequence row's instructions, P06.T38.e's measurement).
        let eta = primary_eta(galaxy, record);
        // A star the fate table finds dead through the whole window takes its remnant, if the
        // guard passes, and no track at all: asked first, since a dead star's main sequence
        // (with its winds' knots, for a massive one) would be built for nothing.
        let first = age_at(age_at_epoch, ClockWindow::START);
        let dead = fates.fate_fitted(m0, &composition, eta).filter(|fate| {
            let margin = (STATED_FACTOR * fate.bounds.0).max(DEAD_MARGIN_FLOOR);
            first > fate.death_age.value() * (1.0 + margin)
        });
        if let Some(fate) = dead {
            let draws = primary_draws(galaxy, record);
            let primary = table_primary(fate, composition, &draws, age_at_epoch, m0)
                .or_else(|| exact_remnant(m0, composition, &draws, age_at_epoch))
                .unwrap_or_else(|| exact(m0, composition, draws, age_at_epoch));
            return Self {
                primary,
                star_count,
            };
        }
        // The main sequence's class reads the rotation rank and fossil mark too (P06.T25).
        let rotation = primary_rotation_draws(galaxy, record);
        let on_main_sequence = (MIN_INITIAL_MASS..=MAX_INITIAL_MASS)
            .contains(&m0)
            .then(|| {
                Track::knot_free_main_sequence(
                    m0,
                    &composition,
                    &eta_draws(eta, rotation),
                    TrackOptions::default(),
                )
            })
            .flatten()
            // The main sequence must hold every age of the window: from the arrival on it
            // (P06.T15.b) at the window's start to the age at its end, as `StarModel` builds its
            // track to.
            .filter(|track| {
                track.built_from().value() <= first
                    && age_at(age_at_epoch, ClockWindow::END) < track.built_until().value()
            });
        let primary = if let Some(track) = on_main_sequence {
            Primary::MainSequence {
                track: Box::new(track),
                composition,
                eta,
                rotation,
                age_at_epoch,
            }
        } else {
            exact(m0, composition, primary_draws(galaxy, record), age_at_epoch)
        };
        Self {
            primary,
            star_count,
        }
    }

    /// Which way the primary is evaluated.
    #[must_use]
    pub const fn route(&self) -> BriefRoute {
        match self.primary {
            Primary::MainSequence { .. } => BriefRoute::MainSequence,
            Primary::Table { .. } => BriefRoute::Table,
            Primary::Remnant { .. } | Primary::Exact(_) => BriefRoute::Exact,
            Primary::Paired(_) => BriefRoute::Paired,
        }
    }

    /// How many stars the system has, the primary included, at its first redraw attempt: the kept
    /// count for every system the grid does not redraw (plan 11's Risks, "Deviations in P11.T7, as
    /// built").
    #[must_use]
    pub const fn star_count(&self) -> u8 {
        self.star_count
    }

    /// The system's brief at `t`, or `None` before its primary forms: [`SystemStars::brief_at`]'s,
    /// the summary's primary then, bit for bit on the exact, main-sequence and paired routes (the
    /// paired one with the first attempt's star count), and of its kind and class on the table's.
    ///
    /// # Panics
    ///
    /// In debug builds, for a `t` after the clock window's end, as [`StarModel::state_at`].
    #[must_use]
    pub fn brief_at(&self, t: UniverseTime) -> Option<StellarBrief> {
        match &self.primary {
            Primary::MainSequence {
                track,
                composition,
                eta,
                rotation,
                age_at_epoch,
            } => {
                let age = age_at(*age_at_epoch, t);
                (age > 0.0).then(|| {
                    let state = track.state_at(Years::new(age));
                    StellarBrief::of(
                        &state,
                        composition,
                        &eta_draws(*eta, *rotation),
                        ClassExtras::NONE,
                        self.star_count,
                    )
                })
            }
            Primary::Table {
                track,
                fate,
                composition,
                draws,
                age_at_epoch,
            } => Some(remnant_brief(
                track,
                composition,
                draws,
                age_at(*age_at_epoch, t),
                fate.death_age.value(),
                self.star_count,
            )),
            Primary::Remnant {
                track,
                composition,
                draws,
                age_at_epoch,
                birth,
            } => Some(remnant_brief(
                track,
                composition,
                draws,
                age_at(*age_at_epoch, t),
                *birth,
                self.star_count,
            )),
            Primary::Exact(model) => model.state_at(t).map(|state| {
                StellarBrief::of(
                    &state,
                    model.composition(),
                    model.draws(),
                    model.class_extras_at(&state, t),
                    self.star_count,
                )
            }),
            Primary::Paired(stars) => stars
                .brief_at(t)
                .map(|brief| brief.with_star_count(self.star_count)),
        }
    }

    /// The stated error of the brief at `t` for a table-routed primary (ruling 90.4): the range of
    /// its luminosity and temperature over the corners of the fate table's error box. `None` for
    /// every other route, whose briefs are exact, and for a remnant with no light.
    #[must_use]
    pub fn stated_error_at(&self, t: UniverseTime) -> Option<StatedError> {
        let Primary::Table {
            track,
            fate,
            draws,
            age_at_epoch,
            ..
        } = &self.primary
        else {
            return None;
        };
        let age = Years::new(age_at(*age_at_epoch, t));
        let (mut l, mut teff) = (
            (f64::INFINITY, f64::NEG_INFINITY),
            (f64::INFINITY, f64::NEG_INFINITY),
        );
        for &(t, a, b) in corners(fate.route) {
            let shift = (STATED_FACTOR * t, STATED_FACTOR * a, STATED_FACTOR * b);
            let state = track.remnant_state_at(fate.remnant(RemnantDraws::of(draws), shift), age);
            let lum = state.luminosity().value();
            if lum <= 0.0 {
                return None;
            }
            let log_l = math::log10(lum);
            let temp = state.effective_temperature().value();
            l = (l.0.min(log_l), l.1.max(log_l));
            teff = (teff.0.min(temp), teff.1.max(temp));
        }
        // Where the box's corners nearly agree, as an old white dwarf's luminosity does, the
        // floor of the interpolation's own error still stands.
        Some(StatedError {
            log_luminosity: (l.0 - STATED_FLOOR_DEX, l.1 + STATED_FLOOR_DEX),
            effective_temperature: (
                teff.0 * (1.0 - STATED_FLOOR_RELATIVE),
                teff.1 * (1.0 + STATED_FLOOR_RELATIVE),
            ),
        })
    }

    /// The bytes the model owns on the heap, beyond `size_of::<BriefModel>()`: what a server's
    /// byte-bounded cache charges it. A main-sequence model owns a few kilobytes; an exact one its
    /// [`StarModel`]'s track, tens of kilobytes for a dead star; a paired one its system's stars,
    /// each star's track and each pair's timeline ([`SystemStars::heap_bytes`]).
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        match &self.primary {
            Primary::MainSequence { track, .. }
            | Primary::Table { track, .. }
            | Primary::Remnant { track, .. } => size_of::<Track>() + track.heap_bytes(),
            Primary::Exact(model) => size_of::<StarModel>() + model.heap_bytes(),
            Primary::Paired(stars) => size_of::<SystemStars>() + stars.heap_bytes(),
        }
    }
}

/// The brief of the grid system `record` in `galaxy` at `t`, or `None` before it forms:
/// [`BriefModel::new`] then [`BriefModel::brief_at`], for a caller that keeps no model.
///
/// # Panics
///
/// As [`BriefModel::new`] and [`BriefModel::brief_at`], which include a pair-light table that
/// cannot be read.
#[must_use]
pub fn range_brief(
    galaxy: &Galaxy,
    record: &SystemRecord,
    t: UniverseTime,
) -> Option<StellarBrief> {
    BriefModel::new(galaxy, record).brief_at(t)
}

/// What the gate finds of a system: its first attempt's star count, and whether its primary is
/// certainly its own single-star model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Gate {
    /// How many stars the first redraw attempt draws, [`draw_star_count`]'s count.
    ///
    /// [`draw_star_count`]: crate::stellar::multiplicity::draw_star_count
    star_count: u8,
    /// Whether the primary is certainly its own single-star model through the clock window.
    certified: bool,
}

/// The module's gate (P11.T11.a; `decision-observed-class-findings.md` §A.2) for `record`'s
/// system at `composition` under `ctx`: whether its primary is certainly its own single-star model
/// at every time up to the clock window's end, and its first attempt's star count.
///
/// A system drawn single is, from its count words alone (`drawn_single`, the start of
/// [`draw_star_count`](crate::stellar::multiplicity::draw_star_count)). Otherwise its first
/// attempt is drawn once ([`first_attempt`]), for its count, which is `draw_star_count`'s, and for
/// its pairs of two stars, read over the system's ages from the window's start to its end, the last
/// its age at +H, to which `SystemStars` runs its pairs: each pair that could reach the primary must
/// leave its stars their own models and fall into no carved class ([`pair_keeps_own_models`]).
/// For a grid system that is every pair, since any carved one would have the system redrawn and
/// the primary might be in another pair at the attempt kept; then the generator keeps attempt 0,
/// whose count it is. For a system the grid does not redraw it is the pair the primary is in, if
/// any. The test of each pair holds at every age up to the window's end, so a time before the
/// window, as the tests ask, is certified too.
///
/// It is conservative, not exact: a refused system takes the paired route, which is right
/// whatever its pairs did. The tests check that every certified primary is its own model at
/// times across the window.
#[must_use]
fn gate(
    galaxy: &Galaxy,
    record: &SystemRecord,
    composition: &Composition,
    ctx: MultiplicityContext,
) -> Gate {
    if drawn_single(galaxy, record, Some(composition), ctx, GRID_ATTEMPT) {
        return Gate {
            star_count: 1,
            certified: true,
        };
    }
    let attempt = first_attempt(galaxy, record, composition, ctx);
    let age_at_epoch = record.age_at_epoch();
    let ages = Years::new(age_at(age_at_epoch, ClockWindow::START))
        ..=Years::new(age_at(age_at_epoch, ClockWindow::END));
    let redrawn = grid_redraws(record, ctx);
    let stars = attempt.stars();
    let certified = attempt
        .pairs()
        .iter()
        .filter(|pair| redrawn || pair.stars().contains(&StarIndex::PRIMARY))
        .all(|pair| {
            let [a, b] = pair
                .stars()
                .map(|star| stars[usize::from(star.get())].initial_mass());
            pair_keeps_own_models(a, b, pair.periastron(), composition, ages.clone())
        });
    Gate {
        star_count: attempt.hierarchy().star_count(),
        certified,
    }
}

/// Draws of η `eta`, the rotation rank and fossil mark `rotation`, and every other draw at its
/// median: what a main-sequence primary's track and class read of the primary's own draws. A main
/// sequence's builder reads η (and holds the remnant draws, which it reads only at a death);
/// [`classify`] reads the rotation and magnetism draws for a main-sequence star's peculiar class
/// (P06.T25) and the white dwarf marks for a white dwarf. So on the main sequence these give the
/// state and brief of [`StarDraws::for_star`]'s draws bit for bit, which the tests check route by
/// route.
///
/// [`classify`]: crate::stellar::classify::classify
#[must_use]
fn eta_draws(eta: StandardNormal, (rotation, magnetism): (UnitUniform, Mark)) -> StarDraws {
    StarDraws::from_parts(StarDrawsParts {
        eta,
        rotation,
        magnetism,
        ..StarDrawsParts::MEDIAN
    })
}

/// The guard's shifts for a fate of `route`.
#[must_use]
fn corners(route: FateRoute) -> &'static [(f64, f64, f64)] {
    match route {
        FateRoute::HeliumWhiteDwarf
        | FateRoute::CarbonOxygenWhiteDwarf
        | FateRoute::OxygenNeonWhiteDwarf
        | FateRoute::BridgedCarbonOxygenWhiteDwarf
        | FateRoute::BridgedOxygenNeonWhiteDwarf => &WHITE_DWARF_CORNERS,
        FateRoute::IronCore => &IRON_CORE_CORNERS,
        FateRoute::ElectronCapture | FateRoute::NoRemnant => &WHITE_DWARF_CORNERS[..1],
    }
}

/// Whether a neutron star's class, which its pulsar's age since formation decides, is the same
/// at both ends of the window for the death age at its centre and at [`STATED_FACTOR`] times its
/// bound either side; true for any other remnant.
#[must_use]
fn pulsar_class_holds(
    fate: &FittedFate,
    centre: RemnantModel,
    draws: &StarDraws,
    age_at_epoch: Years,
) -> bool {
    if centre.phase != Phase::NeutronStar {
        return true;
    }
    let star = NeutronStar::from_draws(draws);
    let t = fate.death_age.value();
    let class = |age: f64, birth: f64| star.state_at(Years::new((age - birth).max(0.0))).class();
    let first = age_at(age_at_epoch, ClockWindow::START);
    let written = class(first, t);
    [ClockWindow::START, ClockWindow::END]
        .into_iter()
        .all(|when| {
            let age = age_at(age_at_epoch, when);
            [-1.0, 0.0, 1.0]
                .into_iter()
                .all(|k| class(age, t * (1.0 + k * STATED_FACTOR * fate.bounds.0)) == written)
        })
}

/// The exact primary: its [`StarModel`].
#[must_use]
fn exact(
    m0: SolarMasses,
    composition: Composition,
    draws: StarDraws,
    age_at_epoch: Years,
) -> Primary {
    Primary::Exact(Box::new(
        StarModel::new(m0, composition, draws, age_at_epoch)
            .expect("a grid record's primary is a star or a brown dwarf, with a finite age"),
    ))
}

/// The exact primary of a star dead through the whole window, from its remnant alone: the
/// remnant a build to the death that keeps no other segment gives (`remnant_of`), which is its
/// full track's bit for bit, and so its brief is [`SystemStars::brief_at`]'s for a primary the
/// gate certifies. `None` if the star is alive at the window's start after all, which the table's
/// bound makes rare.
#[must_use]
fn exact_remnant(
    m0: SolarMasses,
    composition: Composition,
    draws: &StarDraws,
    age_at_epoch: Years,
) -> Option<Primary> {
    let m = if m0 > MAX_INITIAL_MASS {
        MAX_INITIAL_MASS
    } else {
        m0
    };
    let (remnant, _) = remnant_of(m, &composition, draws, TrackOptions::default());
    let birth = remnant.birth;
    (age_at(age_at_epoch, ClockWindow::START) >= birth).then(|| Primary::Remnant {
        track: Box::new(Track::remnant_only(
            &composition,
            draws.eta().value(),
            TrackOptions::default(),
            remnant,
        )),
        composition,
        draws: Box::new(draws.clone()),
        age_at_epoch,
        birth,
    })
}

/// What `classify` reads of a remnant's history at `age`, of a remnant formed at `birth` (years
/// since the onset of collapse): a neutron star's class from its pulsar state at its age since
/// formation, as `StarModel::class_extras_at` gives it (P06.T21.c).
#[must_use]
fn remnant_extras(state: &StarState, draws: &StarDraws, age: f64, birth: f64) -> ClassExtras {
    if state.phase() == Phase::NeutronStar {
        let pulsar = NeutronStar::from_draws(draws).state_at(Years::new((age - birth).max(0.0)));
        ClassExtras::neutron_star(pulsar.class())
    } else {
        ClassExtras::NONE
    }
}

/// The brief at `age` of a remnant-only `track` formed at `birth`, for a star of `composition`
/// and `draws` in a system of `star_count` stars: the remnant's state and its history's extras.
#[must_use]
fn remnant_brief(
    track: &Track,
    composition: &Composition,
    draws: &StarDraws,
    age: f64,
    birth: f64,
    star_count: u8,
) -> StellarBrief {
    let state = track.state_at(Years::new(age));
    let extras = remnant_extras(&state, draws, age, birth);
    StellarBrief::of(&state, composition, draws, extras, star_count)
}

/// The table-routed primary of `fate`, found dead through the whole clock window, of initial
/// mass `m0`, `composition`, `draws` and age at the epoch `age_at_epoch`, or `None` if it is not
/// the table's: its companion-stripped mark can move its fate, or the guard finds that its kind
/// or class could differ somewhere in the fate's error box over the window (P06.T38.e).
///
/// - A white dwarf's class is a function of its temperature and draws alone
///   ([`white_dwarf_type`]). The guard takes the temperature's range over the box and the window
///   to first order: its partial differences in the three columns at the window's start, and
///   the cooling across the window at the centre. The class must be the same at both ends of that
///   range and at the centre.
/// - An iron core's kind is its remnant's, which the remnant draws decide from its two cores: it
///   must be the same at every corner of their box. Its class follows its kind.
/// - Electron capture and no remnant have one kind and class whatever the columns.
///
/// Over a box this small the class is monotone in temperature, so equal classes at the ends of
/// the range are the class of every point inside it; the slow test checks that the exact
/// primary's is.
#[must_use]
fn table_primary(
    fate: FittedFate,
    composition: Composition,
    draws: &StarDraws,
    age_at_epoch: Years,
    m0: SolarMasses,
) -> Option<Primary> {
    if stripped_mark_matters(m0, &composition, draws) {
        return None;
    }
    let remnant_draws = RemnantDraws::of(draws);
    let centre = fate.remnant(remnant_draws, (0.0, 0.0, 0.0));
    let track = Track::remnant_only(
        &composition,
        draws.eta().value(),
        TrackOptions::default(),
        centre,
    );
    let passes = match fate.route {
        FateRoute::HeliumWhiteDwarf
        | FateRoute::CarbonOxygenWhiteDwarf
        | FateRoute::OxygenNeonWhiteDwarf
        | FateRoute::BridgedCarbonOxygenWhiteDwarf
        | FateRoute::BridgedOxygenNeonWhiteDwarf => {
            let (first, last) = (
                Years::new(age_at(age_at_epoch, ClockWindow::START)),
                Years::new(age_at(age_at_epoch, ClockWindow::END)),
            );
            let teff = |shift: (f64, f64, f64), age: Years| {
                track
                    .remnant_state_at(fate.remnant(remnant_draws, shift), age)
                    .effective_temperature()
                    .value()
            };
            let now = teff((0.0, 0.0, 0.0), first);
            let spread = [(1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (0.0, 0.0, 1.0)]
                .into_iter()
                .fold(0.0, |sum, shift| sum + (teff(shift, first) - now).abs());
            let later = teff((0.0, 0.0, 0.0), last);
            let class = |t: f64| white_dwarf_type(Kelvin::new(t), draws).to_string();
            let written = class(now);
            class(now + spread) == written && class(later - spread * later / now) == written
        }
        FateRoute::IronCore => {
            let phase = |shift| fate.remnant(remnant_draws, shift).phase;
            IRON_CORE_CORNERS
                .iter()
                .all(|&(_, a, b)| phase((0.0, a, b)) == centre.phase)
                && pulsar_class_holds(&fate, centre, draws, age_at_epoch)
        }
        FateRoute::ElectronCapture => pulsar_class_holds(&fate, centre, draws, age_at_epoch),
        FateRoute::NoRemnant => true,
    };
    passes.then(|| Primary::Table {
        track: Box::new(track),
        fate,
        composition,
        draws: Box::new(draws.clone()),
        age_at_epoch,
    })
}

/// A primary's age at `t`, Julian years, from its age at the epoch: [`StarModel::age_at`]'s
/// arithmetic.
#[must_use]
fn age_at(age_at_epoch: Years, t: UniverseTime) -> f64 {
    age_at_epoch.value() + t.since_epoch().as_julian_years_f64()
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::bits;
    use hyperion_testkit::lcg::Lcg;

    use super::*;
    use crate::Seed;
    use crate::coords::GalacticPosition;
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::placement::{CellKey, generate_cell};
    use crate::id::Layer;
    use crate::stellar::multiplicity::RedrawAttempt;
    use crate::stellar::multiplicity::testing::records_near;

    fn records(galaxy: &Galaxy) -> Vec<SystemRecord> {
        let mut all = Vec::new();
        let mut cell = Vec::new();
        for layer in [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E] {
            let size = i32::try_from(layer.cell_size_ly()).expect("small");
            for step in 0..3 {
                let key = CellKey::new(layer, [step, 26_000 / size, 0]).expect("a cell");
                generate_cell(galaxy, key, &mut cell);
                all.extend(cell.drain(..).take(12));
            }
        }
        all
    }

    fn years(y: i64) -> UniverseTime {
        UniverseTime::from_julian_years(y).expect("inside the clock")
    }

    /// `route`'s place in a tally of the four.
    fn route_index(route: BriefRoute) -> usize {
        match route {
            BriefRoute::MainSequence => 0,
            BriefRoute::Table => 1,
            BriefRoute::Exact => 2,
            BriefRoute::Paired => 3,
        }
    }

    /// The brief at `t` of `stars`' primary as its single-star self, in a system of `star_count`
    /// stars: what `SystemStars::brief_at` gave before P11.T11.a, and so every range row before
    /// it.
    fn single_star_brief(
        stars: &SystemStars,
        star_count: u8,
        t: UniverseTime,
    ) -> Option<StellarBrief> {
        let primary = stars.primary();
        let state = primary.state_at(t)?;
        Some(StellarBrief::of(
            &state,
            primary.composition(),
            primary.draws(),
            primary.class_extras_at(&state, t),
            star_count,
        ))
    }

    /// The exact, main-sequence and paired routes give the full system's brief bit for bit at
    /// times across the window (the paired one with the first attempt's count), the table's its
    /// kind and class with its luminosity near its stated error, and every route is taken.
    #[test]
    fn every_route_is_the_full_systems_brief() {
        let galaxy = Galaxy::new(Seed::new(0x6272_6965));
        let mut routes = [0_u32; 4];
        for record in records(&galaxy) {
            let model = BriefModel::new(&galaxy, &record);
            let stars = SystemStars::generate(&galaxy, &record);
            if stars.attempt() == RedrawAttempt::FIRST {
                assert_eq!(model.star_count(), stars.star_count(), "{record:?}");
            }
            routes[route_index(model.route())] += 1;
            if model.route() == BriefRoute::Table {
                for y in [-1_000, 0, 1_000] {
                    let t = years(y);
                    let (fitted, exact) = (model.brief_at(t).unwrap(), stars.brief_at(t).unwrap());
                    assert_eq!(
                        (fitted.kind(), fitted.class().to_string()),
                        (exact.kind(), exact.class().to_string()),
                        "{record:?}"
                    );
                    if let Some(stated) = model.stated_error_at(t) {
                        let log_l = exact.log_luminosity().unwrap().value();
                        let teff = exact.effective_temperature().value();
                        let (lo, hi) = stated.log_luminosity;
                        let (cold, hot) = stated.effective_temperature;
                        assert!(
                            (lo..=hi).contains(&log_l) && (cold..=hot).contains(&teff),
                            "{record:?}: {log_l}, {teff} outside {stated:?}"
                        );
                    }
                }
                continue;
            }
            for y in [-200_000, -1_000, -500, 0, 500, 1_000] {
                let (a, b) = (
                    model.brief_at(years(y)),
                    stars
                        .brief_at(years(y))
                        .map(|b| b.with_star_count(model.star_count())),
                );
                assert_eq!(a, b, "{record:?} at {y} yr");
                if let (Some(a), Some(b)) = (a, b) {
                    assert_eq!(
                        bits(a.effective_temperature().value()),
                        bits(b.effective_temperature().value())
                    );
                    assert_eq!(
                        a.log_luminosity().map(|l| bits(l.value())),
                        b.log_luminosity().map(|l| bits(l.value()))
                    );
                }
            }
        }
        assert!(routes.iter().all(|&n| n > 0), "{routes:?}");
    }

    /// A system not yet born has no brief, on either route, and has one once it is.
    #[test]
    fn a_system_not_yet_born_has_no_brief_on_either_route() {
        let galaxy = Galaxy::new(Seed::new(0x6272_6967));
        let template = records(&galaxy)[0];
        let mut routes = Vec::new();
        for mass in [0.5, 0.09, 12.0] {
            // Born 400 years after the epoch.
            let record = SystemRecord::from_parts(
                template.id(),
                *template.epoch_position(),
                template.origin(),
                template.population(),
                crate::units::SolarMasses::new(mass),
                Years::new(-400.0),
            );
            let model = BriefModel::new(&galaxy, &record);
            let stars = SystemStars::generate(&galaxy, &record);
            routes.push(model.route());
            for y in [-1_000, 0, 399] {
                assert_eq!(model.brief_at(years(y)), None, "{mass} M☉ at {y} yr");
                assert_eq!(stars.brief_at(years(y)), None);
            }
            let born = model.brief_at(years(500));
            assert!(born.is_some(), "{mass} M☉ once born");
            assert_eq!(born, stars.brief_at(years(500)));
        }
        // A star born within the window is a protostar at its start, on the exact path (P06.T15).
        assert_eq!(
            routes,
            [BriefRoute::Exact, BriefRoute::Exact, BriefRoute::Exact]
        );
    }

    /// The gate is conservative (P11.T11.a): over 400 systems of each stellar layer near the Sun
    /// and 150 in the bulge, every system it certifies keeps its first attempt, and its primary
    /// is its own model at both ends of the window, at the epoch, at a random time of the window
    /// and 200,000 years before it. So each certified row is the full system's brief, and is the
    /// single-star brief every range row was before P11.T11.a, bit for bit. Both answers are found
    /// among the multiples, and `certifies` is the route's.
    #[test]
    fn the_gate_is_conservative() {
        let galaxy = Galaxy::from_params(Seed::new(0x6761_7465), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral");
        let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 68.0]).expect("in the cube");
        let bulge = GalacticPosition::from_light_years([0.0, 2_000.0, 300.0]).expect("in the cube");
        let mut rng = Lcg::new(0x6761_7465);
        let (mut certified_multiples, mut paired) = (0_u32, 0_u32);
        for (place, at, n) in [("near the Sun", sun, 400), ("in the bulge", bulge, 150)] {
            for layer in [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E] {
                let (mut multiples, mut kept, mut layer_paired) = (0_u32, 0_u32, 0_u32);
                for record in records_near(&galaxy, layer, &at, n) {
                    let model = BriefModel::new(&galaxy, &record);
                    let certified = model.route() != BriefRoute::Paired;
                    assert_eq!(BriefModel::certifies(&galaxy, &record), certified);
                    multiples += u32::from(model.star_count() > 1);
                    if !certified {
                        paired += 1;
                        layer_paired += 1;
                        continue;
                    }
                    kept += u32::from(model.star_count() > 1);
                    let stars = SystemStars::generate(&galaxy, &record);
                    assert_eq!(stars.attempt(), RedrawAttempt::FIRST, "{record:?}");
                    assert_eq!(model.star_count(), stars.star_count(), "{record:?}");
                    certified_multiples += u32::from(model.star_count() > 1);
                    let random =
                        years(i64::try_from(rng.next_u64() % 2_001).expect("small") - 1_000);
                    for t in [
                        ClockWindow::START,
                        ClockWindow::END,
                        UniverseTime::EPOCH,
                        random,
                        years(-200_000),
                    ] {
                        assert_ne!(
                            stars.primary_is_its_own_at(t),
                            Some(false),
                            "{record:?} at {t}: certified, and its pair has changed it"
                        );
                        if model.route() == BriefRoute::Table {
                            continue;
                        }
                        let brief = model.brief_at(t);
                        assert_eq!(brief, stars.brief_at(t), "{record:?} at {t}");
                        assert_eq!(
                            brief,
                            single_star_brief(&stars, model.star_count(), t),
                            "{record:?} at {t}"
                        );
                    }
                }
                println!(
                    "layer {layer:?} {place}: {n} systems, {multiples} multiple; {kept} \
                     multiples certified, {layer_paired} paired"
                );
            }
        }
        assert!(
            certified_multiples > 100 && paired > 10,
            "{certified_multiples} certified multiples, {paired} paired"
        );
    }

    /// A model is the same built twice, and answers the same whatever was asked before, on every
    /// route, the paired one included.
    #[test]
    fn a_model_does_not_depend_on_what_was_asked_before() {
        let galaxy = Galaxy::new(Seed::new(0x6272_6966));
        let mut routes = [0_u32; 4];
        for record in &records(&galaxy) {
            let model = BriefModel::new(&galaxy, record);
            routes[route_index(model.route())] += 1;
            assert_eq!(model, BriefModel::new(&galaxy, record));
            let times = [years(700), years(-300), years(0)];
            let forward: Vec<_> = times.iter().map(|&t| model.brief_at(t)).collect();
            let backward: Vec<_> = times.iter().rev().map(|&t| model.brief_at(t)).collect();
            assert_eq!(forward, backward.into_iter().rev().collect::<Vec<_>>());
            assert_eq!(
                range_brief(&galaxy, record, years(0)),
                model.brief_at(years(0))
            );
        }
        assert!(routes.iter().all(|&n| n > 0), "{routes:?}");
    }
}
