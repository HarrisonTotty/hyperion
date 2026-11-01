//! The recurrent classes' walk (plan 12, P12.T5.c): the events a catalogue class's hosts repeat,
//! each in the window its light arrives in.
//!
//! # The classes
//!
//! Plan 12's T5.c walks these, each through [`scan_plan`] and [`scan_cell`]:
//!
//! - the magnetars of the core collapses' entries (plan 06, P06.T28.c), walked over
//!   [`ClassId::CORE_COLLAPSE`] (main's ruling of 2026-10-30; below);
//! - the accreting white dwarfs, fast and slow, their novae, and the X-ray binaries, their
//!   transients, X-ray bursts and Be/X outbursts, through plan 11's scan marks alone (P11.T8.a–b,
//!   P11.T9);
//! - the luminous blue variables of plan 09's class (P09.T43), plan 06's giant eruptions
//!   (P06.T28.e; [`giant_eruption_scan_cell`]).
//!
//! A class with no recurrent kind plans
//! and scans nothing. Each class is walked over its cells within the reach of its recurrent kinds
//! ([`recurrent_scan_plan`], the magnetars' [`magnetar_scan_plan`]), not widened by Design note 5's
//! 1%, as T5.b's walk is not, and [`recurrent_detections`] merges a sphere's cells for any of them.
//!
//! # The magnetars
//!
//! A core collapse's neutron star born above the quantum critical field `B_Q` is a magnetar for
//! 10¹⁹ G yr ÷ `B_Q` − 10¹⁹ G yr ÷ B₀ after its birth, under [`MAGNETAR_ACTIVE_SPAN`] (227,273
//! years), and only then has short bursts and giant flares (plan 06's
//! [`magnetar`](crate::stellar::events::magnetar) module). The walk ([`magnetar_scan_cell`], over
//! the cells of [`magnetar_scan_plan`]):
//!
//! 1. The reach is [`kind_reach`] of [`TransientKind::MagnetarBurst`] for the query's sensor, not
//!    widened by Design note 5's 1%, and the cells are the class's within it ([`cells_in_sphere`],
//!    padded by the class grid's 4,096 ly, which holds every entry and its remnant at every time
//!    the lookup serves).
//! 2. Each entry's host ([`MagnetarClass::magnetar_host`]): none for an entry whose neutron star,
//!    if it left one, was born at or below `B_Q`.
//! 3. The emission interval. The remnant moves on a straight line from its site at T, so the light
//!    that reaches the observer at t left it at the light cone's exact root on that line
//!    ([`retarded_exact_linear`]). The interval runs from the root for `t₁` less twice the longest
//!    event (Design note 6's D, 3 s; the root is exact, so no ε) to the root for `t₂`, each widened
//!    by a day for rounding, and is cut to the host's own active span, T to T + its span, each
//!    widened by a day: a host whose span misses it is dropped before its star is built
//!    (P12.T5.a.2's prefilter, by each host's span rather than 227,273 years for all).
//! 4. Its star ([`StarRecipe`], plan 06's `StarModel`), its [`MagnetarModel`], and its bursts and
//!    giant flares with onsets in the interval.
//! 5. Each event's arrival, τ + |x(τ) − `x_o`| ÷ c, x(τ) the remnant's position at the onset τ: an
//!    [`Onset`] in the window, or [`InProgress`] if its light, onset to end, is arriving at `t₁`.
//! 6. Its flux in 2–10 keV, the only band it gives light in (plan 06 models no optical light): a
//!    top hat at its mean luminosity (the row's [`LightCurveSource::MagnetarBurst`]) times the
//!    band's share of the event's whole output, which plan 06's energies are (main's ruling of
//!    2026-10-30): [`MAGNETAR_BURST_XRAY_SHARE`] for a short burst, and
//!    [`MAGNETAR_GIANT_FLARE_XRAY_SHARE`] for a giant flare's spike. The distance is the onset's.
//!    The line of sight is plan 07's from the remnant's position at the start of the Julian year
//!    (since the epoch) that holds the onset, marched once per host and year, and only for a year
//!    with an event over the limit unextinguished: it depends on the event alone, never on the
//!    window, so a detection's flux is the same in any window that finds it.
//!
//! A sphere's detections are its cells', each through [`magnetar_scan_cell`] on its own, merged by
//! arrival and then by event ([`magnetar_detections`]), so a background scan's chunks of cells merge
//! to one walk (Design note 8). A magnetar's events are keyed by its star's body, so a detection is
//! the same event whichever query finds it.
//!
//! The route is complete for observers within 34,871 ly whatever an entry's window; beyond that an
//! entry with a short window can leave the class while its magnetar is still active, a recorded
//! finding (plan 12's Risks, "Deferred findings", from P12.T5.a.2).
//!
//! [`Onset`]: DetectionPhase::Onset
//! [`InProgress`]: DetectionPhase::InProgress
//! [`LightCurveSource::MagnetarBurst`]: super::LightCurveSource::MagnetarBurst

use std::error::Error;
use std::fmt;

use crate::coords::GalacticPosition;
use crate::events::TimeWindow;
use crate::galaxy::Galaxy;
use crate::galaxy::catalogue_classes::binary::{AccretingWdFast, AccretingWdSlow, XrayBinaries};
use crate::galaxy::catalogue_classes::core_collapse::{CoreCollapseEntry, CoreCollapseProcess};
use crate::galaxy::catalogue_classes::lbv::LbvProcess;
use crate::galaxy::catalogue_classes::{
    CatalogueCellKey, CatalogueClassCell, CatalogueMotion, ClassId, ClassProcess, cells_in_sphere,
    entries_when, occupied_cells,
};
use crate::galaxy::gas::extinction::Sightline;
use crate::galaxy::placement::SystemRecord;
use crate::galaxy::query::QuerySphere;
use crate::id::{BodyId, EventId, SystemId};
use crate::observe::{Drift, Observer, Retardation, bearing, light_time, retarded_exact_linear};
use crate::stellar::Composition;
use crate::stellar::draws::StarDraws;
use crate::stellar::events::magnetar::{
    Burst, FIELD_DECAY_PRODUCT_GAUSS_YEARS, GiantFlare, MAX_BURST_DURATION, MagnetarModel,
    MagnetarSeries,
};
use crate::stellar::remnant::neutron_star::QUANTUM_CRITICAL_FIELD;
use crate::stellar::substellar;
use crate::stellar::system::{MAX_STAR_MASS, StarModel, primary_draws};
use crate::time::{Span, UniverseTime};
use crate::units::consts::{SECONDS_PER_JULIAN_YEAR, SPEED_OF_LIGHT};
use crate::units::{Gauss, LightYears, Metres, SolarMasses, Watts, Years};

use super::arriving::{ArrivingLight, DetectionLight};
use super::cut;
use super::reach::{flux, inverse_square_flux, kind_reach, transmission};
use super::registry::{Route, TransientKind};
use super::{
    AlertBand, AlertContext, AlertQuery, BAND_COUNT, Detection, DetectionPhase, SensorHorizon,
};

mod binaries;
mod eruptions;
mod lit;

use binaries::{nova_scan_cell, xray_binary_scan_cell};
pub use eruptions::{
    BuildEruptionHostError, EruptionClass, EruptionHost, giant_eruption_scan_cell,
};

/// The longest a magnetar stays one after its neutron star's birth, Julian years: 10¹⁹ G yr ÷
/// `B_Q`, 227,272.7, which a field born far above the critical field approaches (plan 06's
/// [`FIELD_DECAY_PRODUCT_GAUSS_YEARS`] and [`QUANTUM_CRITICAL_FIELD`]).
pub const MAGNETAR_ACTIVE_SPAN: Years =
    Years::new(FIELD_DECAY_PRODUCT_GAUSS_YEARS / QUANTUM_CRITICAL_FIELD.value());

/// The 2–10 keV share of a magnetar's short burst's whole output: 0.106, two blackbodies of kT = 5
/// and 10 keV of equal fluence ([`band_share`](super::band_share) of each, averaged). Israel et al.
/// (2008, ApJ 685, 1114, §2.1 and Table 3) fit two blackbodies of 4.8–6.2 and 9–12 keV, and Lin et
/// al. (2011, ApJ 739, 87, §4.1.2) two of 3–7 and 10–20 keV, equal in fluence: 0.07–0.12 for the
/// former's ends. Stored at full precision and recomputed by a test. Part of the generator version.
pub const MAGNETAR_BURST_XRAY_SHARE: f64 = 0.106_281_566_995_235_85;

/// The share of a luminous blue variable's giant eruption's own light in each band, in
/// [`AlertBand::ALL`]'s order: a 5,000 K blackbody's through each band's top hat
/// ([`band_share`](super::band_share)), none in radio or X-rays (P12.T5.c).
///
/// The light echoes of η Carinae's Great Eruption show an early G-type supergiant's spectrum at
/// its peak (Prieto et al. 2014, ApJL 787, L8, read at source in its abstract), some 5,000 K:
/// Rest et al. (2012, Nature 482, 375, "Light echoes reveal an unexpectedly cool η Carinae during
/// its nineteenth-century Great Eruption"; the figure recalled, its title read at source) took it
/// for a G2–G5 supergiant's, cooler than the 7,000 K or more an opaque wind was thought to show
/// (Davidson 1987, ApJ 317, 760; recalled, not read at source). Plan 06's eruptions are
/// bolometric, so the share puts 0.094 of their light in V. Held to `band_share`'s to 10⁻¹² by a
/// test, with the bits of the integral they were first stored from. Part of the generator
/// version.
pub const GIANT_ERUPTION_SHARES: [f64; BAND_COUNT] = [
    0.038_245_858_899_365_98,
    0.081_650_643_015_884_41,
    0.094_176_500_995_914_4,
    0.173_555_117_530_767_24,
    0.141_966_794_944_053_5,
    0.101_178_469_007_729_46,
    0.053_720_796_674_658_33,
    0.031_696_535_695_516_76,
    0.001_670_013_988_742_865_3,
    0.0,
    0.0,
];

/// The 2–10 keV share of a giant flare's spike's whole output: 0.0036, from Frederiks et al.'s
/// (2007, Astron. Lett. 33, 1) cut-off power law for SGR 1806−20's spike, peaking near 850 keV,
/// over 1–10,000 keV (computed by plan 12's research from the fit, which its science check read at
/// source). Palmer et al.'s (2005) fit gives 5 × 10⁻⁴ and Hurley et al.'s (2005) 175 keV blackbody
/// 10⁻⁵, so this is the largest of the three. Part of the generator version.
pub const MAGNETAR_GIANT_FLARE_XRAY_SHARE: f64 = 0.0036;

/// What builds a host's star: plan 06's `StarModel` of its primary's initial mass, composition,
/// draws and age at the epoch.
#[derive(Debug, Clone, PartialEq)]
pub struct StarRecipe {
    mass: SolarMasses,
    composition: Composition,
    draws: StarDraws,
    age_at_epoch: Years,
}

impl StarRecipe {
    /// The star of initial mass `mass`, 0.01–150 M☉, at `composition`, with `draws`, `age_at_epoch`
    /// old at the epoch: its age then since its birth, Julian years, negative for a star born after
    /// the epoch.
    ///
    /// # Errors
    ///
    /// [`BuildStarModelError`](crate::stellar::system::BuildStarModelError), as plan 06's
    /// `StarModel::new` gives it, for a mass outside 0.01–150 M☉ or an age that is not finite.
    pub fn new(
        mass: SolarMasses,
        composition: Composition,
        draws: StarDraws,
        age_at_epoch: Years,
    ) -> Result<Self, crate::stellar::system::BuildStarModelError> {
        use crate::stellar::system::BuildStarModelError;
        if !(substellar::MIN_MASS.value()..=MAX_STAR_MASS.value()).contains(&mass.value()) {
            return Err(BuildStarModelError::MassOutsideRange(mass));
        }
        if !age_at_epoch.value().is_finite() {
            return Err(BuildStarModelError::AgeNotFinite(age_at_epoch));
        }
        Ok(Self {
            mass,
            composition,
            draws,
            age_at_epoch,
        })
    }

    /// The primary of a catalogue system's `record` at `composition`, as the stellar stage builds
    /// it: its draws at attempt 0 ([`primary_draws`]).
    ///
    /// # Panics
    ///
    /// Never: a record's primary is of 0.08–150 M☉ with a finite age (`SystemRecord::from_parts`).
    #[must_use]
    fn of_record(galaxy: &Galaxy, record: &SystemRecord, composition: Composition) -> Self {
        Self::new(
            record.primary_initial_mass(),
            composition,
            primary_draws(galaxy, record),
            record.age_at_epoch(),
        )
        .expect("a record's primary is a star of a finite age")
    }

    /// The star, its track evolved: milliseconds of work.
    ///
    /// # Panics
    ///
    /// Never: the recipe's mass and age were checked when it was built, as `StarModel::new`
    /// checks them.
    #[must_use]
    pub fn to_star(&self) -> StarModel {
        StarModel::new(
            self.mass,
            self.composition,
            self.draws.clone(),
            self.age_at_epoch,
        )
        .expect("a recipe's mass and age are checked when it is built")
    }
}

/// One entry's magnetar, as the walk reads it: its system and star, its neutron star's birth and
/// birth field, and how its remnant moves (P12.T5.c).
#[derive(Debug, Clone, PartialEq)]
pub struct MagnetarHost {
    host: SystemId,
    birth: UniverseTime,
    birth_field: Gauss,
    motion: CatalogueMotion,
    star: StarRecipe,
}

/// A [`MagnetarHost`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildMagnetarHostError {
    /// The birth field is at or below the critical field, or not a number: the neutron star is
    /// never a magnetar.
    FieldNotAboveCritical(Gauss),
    /// The remnant or its progenitor moves at or above light's speed, or at no speed a number.
    NotSlowerThanLight,
}

impl fmt::Display for BuildMagnetarHostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FieldNotAboveCritical(field) => write!(
                f,
                "a magnetar's birth field of {} G is not above the critical field",
                field.value()
            ),
            Self::NotSlowerThanLight => f.write_str("a magnetar's host moves no slower than light"),
        }
    }
}

impl Error for BuildMagnetarHostError {}

impl MagnetarHost {
    /// The magnetar of system `host`'s primary, `star`, whose neutron star is born at `birth` with
    /// the field `birth_field`, G, which `star`'s neutron star must have, its remnant moving by
    /// `motion` (galactic frame) from then on.
    ///
    /// # Errors
    ///
    /// [`BuildMagnetarHostError::FieldNotAboveCritical`] for a field at or below
    /// `B_Q`, 4.4 × 10¹³ G, or not a number, and [`BuildMagnetarHostError::NotSlowerThanLight`] for
    /// a motion whose speed before or after its break is not below light's.
    pub fn new(
        host: SystemId,
        birth: UniverseTime,
        birth_field: Gauss,
        motion: CatalogueMotion,
        star: StarRecipe,
    ) -> Result<Self, BuildMagnetarHostError> {
        if !(birth_field > QUANTUM_CRITICAL_FIELD && birth_field.value().is_finite()) {
            return Err(BuildMagnetarHostError::FieldNotAboveCritical(birth_field));
        }
        let before = motion
            .at()
            .checked_sub(Span::from_seconds(1))
            .unwrap_or(motion.at());
        let slower = [before, motion.at()]
            .map(|t| motion.velocity_at(t).speed().value())
            .iter()
            .all(|&speed| speed < SPEED_OF_LIGHT);
        if !slower {
            return Err(BuildMagnetarHostError::NotSlowerThanLight);
        }
        Ok(Self {
            host,
            birth,
            birth_field,
            motion,
            star,
        })
    }

    /// The system.
    #[must_use]
    pub const fn host(&self) -> SystemId {
        self.host
    }

    /// The body whose events the magnetar's are: the system's primary.
    #[must_use]
    pub fn body(&self) -> BodyId {
        BodyId::new(self.host, 0)
    }

    /// The neutron star's birth, the entry's T.
    #[must_use]
    pub const fn birth(&self) -> UniverseTime {
        self.birth
    }

    /// The neutron star's field at birth, G, above `B_Q`.
    #[must_use]
    pub const fn birth_field(&self) -> Gauss {
        self.birth_field
    }

    /// How the remnant moves.
    #[must_use]
    pub const fn motion(&self) -> &CatalogueMotion {
        &self.motion
    }

    /// What builds the star.
    #[must_use]
    pub const fn star(&self) -> &StarRecipe {
        &self.star
    }

    /// The span after its birth in which it can be a magnetar: 10¹⁹ G yr ÷ `B_Q` − 10¹⁹ G yr ÷ B₀,
    /// Julian years, positive and under [`MAGNETAR_ACTIVE_SPAN`].
    #[must_use]
    pub fn active_span(&self) -> Years {
        Years::new(
            MAGNETAR_ACTIVE_SPAN.value()
                - FIELD_DECAY_PRODUCT_GAUSS_YEARS / self.birth_field.value(),
        )
    }
}

/// A class whose entries' neutron stars may be magnetars, as the walk reads it (P12.T5.c).
///
/// Implemented for plan 09's core collapses ([`CoreCollapseProcess`]); a test's class implements
/// it the same way.
pub trait MagnetarClass: ClassProcess {
    /// The magnetar `entry` leaves, or `None` for an entry whose neutron star, if it left one, was
    /// born at or below the critical field.
    #[must_use]
    fn magnetar_host(&self, galaxy: &Galaxy, entry: &Self::Entry) -> Option<MagnetarHost>;
}

impl MagnetarClass for CoreCollapseProcess {
    /// The entry's neutron star ([`SupernovaEntry::pulsar`]), born at T, and its primary as the
    /// stellar stage builds it, whose neutron star that is.
    ///
    /// [`SupernovaEntry::pulsar`]: crate::galaxy::catalogue_classes::supernova::SupernovaEntry::pulsar
    fn magnetar_host(&self, galaxy: &Galaxy, entry: &CoreCollapseEntry) -> Option<MagnetarHost> {
        let pulsar = entry.supernova().pulsar()?;
        // A field at or below the critical field is no magnetar's; the class's motions are below
        // light's speed (plan 08's draws, P09.T33.a's cut), so nothing else is refused.
        MagnetarHost::new(
            entry.record().id(),
            entry.supernova().explosion(),
            pulsar.birth_field(),
            *entry.motion(),
            StarRecipe::of_record(galaxy, entry.record(), *entry.composition()),
        )
        .ok()
    }
}

/// Lends the magnetar walk a class cell's hosts, generating them if it has not got them (P12.T5.c).
///
/// As [`OneShotCells`](super::OneShotCells) lends the one-shot walk its events: a class's cell is a
/// pure function of the galaxy and its key and costs milliseconds (P09.T33.a), so a caller that
/// walks the same cells again keeps them. An implementation must lend exactly what `generate`
/// returns. The walk names a cell by its key alone, which holds its class, so a cache serves one
/// galaxy and one process per class, as the production lookup has. Nothing the walk returns then
/// depends on what the cache held.
///
/// # Examples
///
/// ```
/// use std::collections::BTreeMap;
///
/// use hyperion_sim::alerts::{MagnetarCells, MagnetarHost};
/// use hyperion_sim::galaxy::catalogue_classes::CatalogueCellKey;
///
/// /// Keeps every cell it is asked for, for one galaxy.
/// #[derive(Debug, Default)]
/// struct KeepHosts(BTreeMap<CatalogueCellKey, Vec<MagnetarHost>>);
///
/// impl MagnetarCells for KeepHosts {
///     fn with_hosts(
///         &mut self,
///         key: CatalogueCellKey,
///         generate: &mut dyn FnMut() -> Vec<MagnetarHost>,
///         visit: &mut dyn FnMut(&[MagnetarHost]),
///     ) {
///         visit(self.0.entry(key).or_insert_with(generate));
///     }
/// }
/// # let _ = KeepHosts::default();
/// ```
pub trait MagnetarCells {
    /// Calls `visit` with the hosts of the cell `key`, which `generate` computes.
    fn with_hosts(
        &mut self,
        key: CatalogueCellKey,
        generate: &mut dyn FnMut() -> Vec<MagnetarHost>,
        visit: &mut dyn FnMut(&[MagnetarHost]),
    );
}

/// The cache that keeps nothing: every cell is generated for its visit and dropped.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NoMagnetarCells;

impl MagnetarCells for NoMagnetarCells {
    fn with_hosts(
        &mut self,
        _key: CatalogueCellKey,
        generate: &mut dyn FnMut() -> Vec<MagnetarHost>,
        visit: &mut dyn FnMut(&[MagnetarHost]),
    ) {
        visit(&generate());
    }
}

/// The cells a background scan of `class` walks for `query` in `_galaxy`, in key order: for the
/// core collapses, their magnetars' ([`magnetar_scan_plan`]); for the accreting white dwarfs,
/// the X-ray binaries and the luminous blue variables, their cells within the reach of their
/// recurrent kinds ([`recurrent_scan_plan`]); none for a class the query does not ask about or a
/// class with no recurrent kind walked here.
///
/// The galaxy is the plan's Provides': no class's cells need it.
///
/// # Panics
///
/// As [`recurrent_scan_plan`].
#[must_use]
pub fn scan_plan(_galaxy: &Galaxy, class: ClassId, query: &AlertQuery) -> Vec<CatalogueCellKey> {
    if class == ClassId::CORE_COLLAPSE {
        magnetar_scan_plan(class, query)
    } else if is_walked(class) {
        recurrent_scan_plan(class, query)
    } else {
        Vec::new()
    }
}

/// Whether [`scan_cell`] walks `class` through plan 11's scan marks.
#[must_use]
fn is_walked(class: ClassId) -> bool {
    [
        ClassId::ACCRETING_WHITE_DWARF,
        ClassId::ACCRETING_WHITE_DWARF_SLOW,
        ClassId::XRAY_BINARY,
        ClassId::LUMINOUS_BLUE_VARIABLE,
    ]
    .contains(&class)
}

/// Appends to `out` the recurrent events of `class`'s hosts in `cell` that `query`'s sensor sees in
/// its window, sorted by arrival and then by event: for the core collapses, their magnetars'
/// ([`magnetar_scan_cell`] with plan 09's process); for the accreting white dwarfs, their novae
/// through their scan marks, for the X-ray binaries, their outbursts and bursts, the same way,
/// and for the luminous blue variables, their giant eruptions
/// ([`giant_eruption_scan_cell`]), each with its class's process; nothing for the other classes
/// (module documentation). A pure function of one catalogue cell, which a background scan runs in
/// chunks (Design note 8).
///
/// # Panics
///
/// As the class's walk does.
///
/// # Examples
///
/// The magnetars an X-ray sensor near the Sun sees flare in a year, from the core collapses' cells
/// within its reach, here some 2,000 ly at a bright limit: a background scan walks the plan's cells
/// in chunks and merges them.
///
/// ```no_run
/// use hyperion_sim::Seed;
/// use hyperion_sim::alerts::{
///     AlertBand, AlertContext, AlertQuery, BandLimits, SensorHorizon, scan_cell, scan_plan,
/// };
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::events::TimeWindow;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::catalogue_classes::ClassId;
/// use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
/// use hyperion_sim::galaxy::gas::noise::NoiseCache;
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::WattsPerSquareMetre;
///
/// let galaxy = Galaxy::new(Seed::new(9));
/// let w = WattsPerSquareMetre::new;
/// let xray = BandLimits::new(w(0.04), w(0.4))?;
/// let sensor = SensorHorizon::new([(AlertBand::XRay, xray)])?;
/// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 20.0]).ok_or("in the cube")?;
/// let year = TimeWindow::new(UniverseTime::EPOCH, UniverseTime::from_julian_years(1).ok_or("on the clock")?)?;
/// let query = AlertQuery::builder(sun, year, sensor).build()?;
/// let mut noise = NoiseCache::with_capacity(4_096);
/// let mut context = AlertContext::new(&mut noise, &NoModifiers);
/// let mut seen = Vec::new();
/// for cell in scan_plan(&galaxy, ClassId::CORE_COLLAPSE, &query) {
///     scan_cell(&galaxy, &mut context, ClassId::CORE_COLLAPSE, cell, &query, &mut seen);
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn scan_cell(
    galaxy: &Galaxy,
    ctx: &mut AlertContext<'_>,
    class: ClassId,
    cell: CatalogueCellKey,
    query: &AlertQuery,
    out: &mut Vec<Detection>,
) {
    if cell.class() != class {
        return;
    }
    if class == ClassId::CORE_COLLAPSE {
        let process = CoreCollapseProcess::new(galaxy);
        magnetar_scan_cell(galaxy, ctx, query, &process, cell, out);
    } else if let Some(process) = RecurrentProcess::of(galaxy, class) {
        process.scan_cell(galaxy, ctx, cell, query, out);
    }
}

/// The process of a class walked through its recurrent events alone: both accreting white dwarf
/// classes, the X-ray binaries and the luminous blue variables. A scan of many cells builds it once
/// and lends it to each cell's walk ([`scan_cell`](Self::scan_cell)), as [`scan_cell`] would build
/// it for every cell (plan 12, P12.T8.b.2). The core collapses' magnetars are walked with their
/// own process ([`magnetar_scan_cell`]).
#[derive(Debug, Clone, PartialEq)]
#[expect(
    clippy::large_enum_variant,
    reason = "one is built a walk and lent to every cell, never held in bulk"
)]
pub enum RecurrentProcess {
    /// The fast accreting white dwarfs, their novae.
    AccretingWdFast(AccretingWdFast),
    /// The slow accreting white dwarfs, their novae.
    AccretingWdSlow(AccretingWdSlow),
    /// The X-ray binaries, their outbursts and bursts.
    XrayBinaries(XrayBinaries),
    /// The luminous blue variables, their giant eruptions.
    LuminousBlueVariables(LbvProcess),
}

impl RecurrentProcess {
    /// `class`'s process in `galaxy`, or `None` for a class not walked through its recurrent
    /// events alone.
    #[must_use]
    pub fn of(galaxy: &Galaxy, class: ClassId) -> Option<Self> {
        match class {
            ClassId::ACCRETING_WHITE_DWARF => {
                Some(Self::AccretingWdFast(AccretingWdFast::new(galaxy)))
            }
            ClassId::ACCRETING_WHITE_DWARF_SLOW => {
                Some(Self::AccretingWdSlow(AccretingWdSlow::new(galaxy)))
            }
            ClassId::XRAY_BINARY => Some(Self::XrayBinaries(XrayBinaries::new(galaxy))),
            ClassId::LUMINOUS_BLUE_VARIABLE => {
                Some(Self::LuminousBlueVariables(LbvProcess::new(galaxy)))
            }
            _ => None,
        }
    }

    /// The class.
    #[must_use]
    pub fn class(&self) -> ClassId {
        match self {
            Self::AccretingWdFast(process) => process.class(),
            Self::AccretingWdSlow(process) => process.class(),
            Self::XrayBinaries(process) => process.class(),
            Self::LuminousBlueVariables(process) => process.class(),
        }
    }

    /// Each of `cells` that draws any candidate, with its count ([`occupied_cells`]): on these
    /// alone the walk can find a host, since each class's scan draws its candidates from the same
    /// bound and count.
    ///
    /// # Panics
    ///
    /// As [`occupied_cells`].
    #[must_use]
    pub fn occupied_cells(
        &self,
        galaxy: &Galaxy,
        cells: impl IntoIterator<Item = CatalogueCellKey>,
    ) -> Vec<(CatalogueCellKey, u32)> {
        match self {
            Self::AccretingWdFast(process) => occupied_cells(galaxy, process, cells),
            Self::AccretingWdSlow(process) => occupied_cells(galaxy, process, cells),
            Self::XrayBinaries(process) => occupied_cells(galaxy, process, cells),
            Self::LuminousBlueVariables(process) => occupied_cells(galaxy, process, cells),
        }
    }

    /// Appends to `out` the recurrent events of the class's hosts in `cell` that `query`'s sensor
    /// sees in its window, sorted by arrival and then by event: [`scan_cell`]'s, with this
    /// process.
    ///
    /// # Panics
    ///
    /// As the class's walk does.
    pub fn scan_cell(
        &self,
        galaxy: &Galaxy,
        ctx: &mut AlertContext<'_>,
        cell: CatalogueCellKey,
        query: &AlertQuery,
        out: &mut Vec<Detection>,
    ) {
        match self {
            Self::AccretingWdFast(process) => {
                nova_scan_cell(galaxy, ctx, query, process.hosts(), cell, out);
            }
            Self::AccretingWdSlow(process) => {
                nova_scan_cell(galaxy, ctx, query, process.hosts(), cell, out);
            }
            Self::XrayBinaries(process) => {
                xray_binary_scan_cell(galaxy, ctx, query, process, cell, out);
            }
            Self::LuminousBlueVariables(process) => {
                giant_eruption_scan_cell(galaxy, ctx, query, process, cell, out);
            }
        }
    }
}

/// The detections of every recurrent event of `class`'s hosts that `query`'s sensor sees in its
/// window, appended to `out` by arrival and then by event: [`scan_cell`] over each cell of
/// [`scan_plan`], merged, as a background scan's chunks merge (Design note 8).
///
/// # Panics
///
/// As [`scan_cell`].
///
/// # Examples
///
/// The novae a visual sensor at the Sun-like point sees begin over a decade, to twelfth magnitude
/// in V: a nova on a heavy white dwarf outshines that across the cube, so the walk takes every cell
/// of the fast hosts' class, and the dust decides which are seen.
///
/// ```no_run
/// use hyperion_sim::Seed;
/// use hyperion_sim::alerts::{
///     AlertBand, AlertContext, AlertQuery, BandLimits, DetectionPhase, SensorHorizon,
///     recurrent_detections,
/// };
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::events::TimeWindow;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::catalogue_classes::ClassId;
/// use hyperion_sim::galaxy::gas::ccm::Band;
/// use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
/// use hyperion_sim::galaxy::gas::noise::NoiseCache;
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::WattsPerSquareMetre;
///
/// let galaxy = Galaxy::new(Seed::new(9));
/// // V = 12: Vega's 3.631 × 10⁻⁸ W m⁻² µm⁻¹ through the V band's 0.084 µm, 10^4.8 fainter.
/// let limit = WattsPerSquareMetre::new(3.631e-8 * 0.084 * 1.584_893e-5);
/// let v = BandLimits::new(limit, limit)?;
/// let sensor = SensorHorizon::new([(AlertBand::Photometric(Band::V), v)])?;
/// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 20.0]).ok_or("in the cube")?;
/// let decade = TimeWindow::new(UniverseTime::EPOCH, UniverseTime::from_julian_years(10).ok_or("on the clock")?)?;
/// let query = AlertQuery::builder(sun, decade, sensor).build()?;
/// let mut noise = NoiseCache::with_capacity(4_096);
/// let mut context = AlertContext::new(&mut noise, &NoModifiers);
/// let mut novae = Vec::new();
/// recurrent_detections(&galaxy, &mut context, ClassId::ACCRETING_WHITE_DWARF, &query, &mut novae);
/// let begun = novae.iter().filter(|d| d.phase() == DetectionPhase::Onset).count();
/// # let _ = begun;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn recurrent_detections(
    galaxy: &Galaxy,
    ctx: &mut AlertContext<'_>,
    class: ClassId,
    query: &AlertQuery,
    out: &mut Vec<Detection>,
) {
    let start = out.len();
    for cell in scan_plan(galaxy, class, query) {
        scan_cell(galaxy, ctx, class, cell, query, out);
    }
    sort_detections(&mut out[start..]);
}

/// How far `sensor` can see the events of `class`'s hosts that the recurrent walk finds: the
/// largest [`kind_reach`] of the recurrent kinds whose hosts are the class's, with no extinction
/// (Design note 5); zero for a class with none.
///
/// # Examples
///
/// ```
/// use hyperion_sim::alerts::{AlertBand, BandLimits, SensorHorizon, recurrent_reach};
/// use hyperion_sim::galaxy::catalogue_classes::ClassId;
/// use hyperion_sim::units::WattsPerSquareMetre;
///
/// let limit = WattsPerSquareMetre::new(1e-6);
/// let xray = SensorHorizon::new([(AlertBand::XRay, BandLimits::new(limit, limit)?)])?;
/// // An X-ray transient at a 15 M☉ black hole's Eddington luminosity, 1.0282 × 10³² W in
/// // 2–10 keV, falls to a microwatt per square metre at some 300 ly.
/// let transients = recurrent_reach(ClassId::XRAY_BINARY, &xray).value();
/// assert!((290.0..310.0).contains(&transients));
/// // A nova gives no X-rays.
/// assert_eq!(recurrent_reach(ClassId::ACCRETING_WHITE_DWARF, &xray).value(), 0.0);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn recurrent_reach(class: ClassId, sensor: &SensorHorizon) -> LightYears {
    TransientKind::ALL
        .into_iter()
        .filter(|kind| {
            let row = kind.row();
            row.route() == Route::Recurrent && row.classes().contains(class)
        })
        .map(|kind| kind_reach(kind, sensor))
        .fold(
            LightYears::ZERO,
            |far, reach| if reach > far { reach } else { far },
        )
}

/// The cells of `class`'s grid whose hosts' recurrent events `query`'s sensor can see: the class's
/// cells within [`recurrent_reach`], in key order; none if the query does not ask about the class
/// or the sensor sees none of its events. Every host stays within the class grid's 4,096 ly of its
/// cell at every time the lookup serves (plan 09's rule 5), so no host whose event's light could
/// reach the sensor's limit is left out.
///
/// # Panics
///
/// If the reach is not finite, which [`kind_reach`] never gives.
#[must_use]
pub fn recurrent_scan_plan(class: ClassId, query: &AlertQuery) -> Vec<CatalogueCellKey> {
    if !query.classes().contains(class) {
        return Vec::new();
    }
    let reach = recurrent_reach(class, query.sensor());
    if reach <= LightYears::ZERO {
        return Vec::new();
    }
    let sphere = QuerySphere::new(
        *query.observer_position(),
        reach,
        query.window().start(),
        LightYears::ZERO,
    )
    .expect("a reach is a positive length within the cube's diagonal");
    cells_in_sphere(class, &sphere).collect()
}

/// The cells of `class`'s grid whose entries' magnetars `query`'s sensor can see: the class's cells
/// within [`kind_reach`] of [`TransientKind::MagnetarBurst`], in key order; none if the query does
/// not ask about the class or the sensor sees no magnetar.
///
/// # Panics
///
/// If the reach is not finite, which [`kind_reach`] never gives.
#[must_use]
pub fn magnetar_scan_plan(class: ClassId, query: &AlertQuery) -> Vec<CatalogueCellKey> {
    if !query.classes().contains(class) {
        return Vec::new();
    }
    let reach = kind_reach(TransientKind::MagnetarBurst, query.sensor());
    if reach <= LightYears::ZERO {
        return Vec::new();
    }
    let sphere = QuerySphere::new(
        *query.observer_position(),
        reach,
        query.window().start(),
        LightYears::ZERO,
    )
    .expect("a reach is a positive length within the cube's diagonal");
    cells_in_sphere(class, &sphere).collect()
}

/// The detections of every magnetar of `process`'s class that `query`'s sensor sees in its window
/// (module documentation), appended to `out` by arrival and then by event: each cell of
/// [`magnetar_scan_plan`] walked whole, its hosts taken from the context's [`MagnetarCells`],
/// merged. [`magnetar_scan_cell`] over the same cells gives the same detections, each to the bit,
/// building only the hosts the window can see (P12.T8.b.2).
///
/// # Panics
///
/// As [`magnetar_scan_cell`].
pub fn magnetar_detections<P: MagnetarClass>(
    galaxy: &Galaxy,
    ctx: &mut AlertContext<'_>,
    query: &AlertQuery,
    process: &P,
    out: &mut Vec<Detection>,
) {
    let start = out.len();
    if query.classes().contains(process.class()) {
        let observer = *query.observer_position();
        for cell in magnetar_scan_plan(process.class(), query) {
            let mut active = Vec::new();
            ctx.magnetar_hosts(
                cell,
                &mut || hosts_of(galaxy, process, cell),
                &mut |hosts| {
                    active.extend(hosts.iter().filter_map(|host| {
                        emission_of(host, &observer, query).map(|interval| (host.clone(), interval))
                    }));
                },
            );
            hosts_detections(galaxy, ctx, query, process.class(), active, out);
        }
    }
    sort_detections(&mut out[start..]);
}

/// Appends to `out` the detections of the magnetars of `process`'s cell `cell` that `query`'s
/// sensor sees in its window, sorted by arrival and then by event (module documentation). Nothing
/// for a cell of another class, or one the query does not ask about.
///
/// It builds only the entries whose death, read first, can make a magnetar that lights the window
/// from the cell (the magnetar cut, plan 12, P12.T8.b.2), some one in fifteen of a cell's, and so
/// never takes the cell from the context's [`MagnetarCells`], which hold whole cells. Over every
/// cell of [`magnetar_scan_plan`] the detections are [`magnetar_detections`]' own, each to the bit.
///
/// # Panics
///
/// As [`CatalogueClassCell::generate`] and the class's process do; and if a host's star leaves no
/// magnetar, which a host whose birth field is not its star's can claim, a broken invariant of its
/// class.
pub fn magnetar_scan_cell<P: MagnetarClass>(
    galaxy: &Galaxy,
    ctx: &mut AlertContext<'_>,
    query: &AlertQuery,
    process: &P,
    cell: CatalogueCellKey,
    out: &mut Vec<Detection>,
) {
    let class = process.class();
    if cell.class() != class || !query.classes().contains(class) {
        return;
    }
    let observer = *query.observer_position();
    let active: Vec<(MagnetarHost, TimeWindow)> =
        entries_when(galaxy, process, cell, cut::magnetars(query, &cell))
            .iter()
            .filter_map(|entry| process.magnetar_host(galaxy, entry.value()))
            .filter_map(|host| {
                emission_of(&host, &observer, query).map(|interval| (host, interval))
            })
            .collect();
    let start = out.len();
    hosts_detections(galaxy, ctx, query, class, active, out);
    sort_detections(&mut out[start..]);
}

/// Appends to `out` the detections of each of `active`'s hosts of `class` over its emission
/// interval, a cell's, unsorted.
fn hosts_detections(
    galaxy: &Galaxy,
    ctx: &mut AlertContext<'_>,
    query: &AlertQuery,
    class: ClassId,
    active: Vec<(MagnetarHost, TimeWindow)>,
    out: &mut Vec<Detection>,
) {
    let mut scratch = Scratch::default();
    for (host, interval) in active {
        host_detections(
            galaxy,
            ctx,
            query,
            class,
            &host,
            interval,
            &mut scratch,
            out,
        );
    }
}

/// Sorts detections by arrival and then by event, the [`AlertResult`](super::AlertResult)'s
/// order.
fn sort_detections(detections: &mut [Detection]) {
    detections.sort_by(|a, b| {
        a.arrival()
            .cmp(&b.arrival())
            .then_with(|| a.event().cmp(&b.event()))
    });
}

/// The hosts of `process`'s cell `key` in `galaxy`, in index order.
#[must_use]
fn hosts_of<P: MagnetarClass>(
    galaxy: &Galaxy,
    process: &P,
    key: CatalogueCellKey,
) -> Vec<MagnetarHost> {
    CatalogueClassCell::generate(galaxy, process, key)
        .entries()
        .iter()
        .filter_map(|entry| process.magnetar_host(galaxy, entry.value()))
        .collect()
}

/// A day, the emission interval's margin for rounding.
const DAY: Span = Span::from_seconds(86_400);

/// The interval `host`'s events' onsets are listed over for `query` at `observer` (module
/// documentation, step 3), or `None` if its active span misses it, so that no star is built for
/// it.
///
/// # Panics
///
/// Never: an [`AlertQuery`]'s observer is in the root cube and its window on the clock, and a host
/// moves below light's speed, which [`MagnetarHost::new`] checks.
#[must_use]
fn emission_of(
    host: &MagnetarHost,
    observer: &GalacticPosition,
    query: &AlertQuery,
) -> Option<TimeWindow> {
    let window = query.window();
    let motion = host.motion();
    let line = Drift::through(motion.point(), motion.at(), motion.velocity_at(motion.at()));
    let root = |t: UniverseTime| {
        let at = Observer::new(*observer, t)
            .expect("an alert query's observer is in the root cube and its window on the clock");
        retarded_exact_linear(&at, line.epoch_position(), &line.velocity()).emitted()
    };
    let longest = MAX_BURST_DURATION
        .checked_add(MAX_BURST_DURATION)
        .and_then(|span| span.checked_add(DAY))
        .expect("seconds and a day are a span");
    let from = root(window.start())
        .checked_sub(longest)
        .expect("the source horizon is far from the clock's end");
    let to = root(window.end())
        .checked_add(DAY)
        .expect("the window's end is on the clock");
    let span = Span::from_seconds_f64(host.active_span().value() * SECONDS_PER_JULIAN_YEAR)
        .expect("under 227,273 years is a span");
    let quiet = host
        .birth()
        .checked_add(span)
        .and_then(|t| t.checked_add(DAY))
        .expect("a magnetar's active span ends on the clock");
    let born = host
        .birth()
        .checked_sub(DAY)
        .expect("a death is on the clock");
    TimeWindow::new(from.max(born), to.min(quiet))
        .ok()
        .filter(|interval| !interval.is_empty())
}

/// One of a magnetar's events, as the walk evaluates it: its ID, onset, end and light in 2–10 keV.
#[derive(Debug, Clone, Copy)]
struct Outburst {
    id: EventId,
    onset: UniverseTime,
    end: UniverseTime,
    xray: Watts,
}

impl Outburst {
    #[must_use]
    fn of_burst(burst: &Burst) -> Self {
        Self {
            id: burst.id(),
            onset: burst.onset(),
            end: burst.end(),
            xray: Watts::new(burst.mean_luminosity_watts() * MAGNETAR_BURST_XRAY_SHARE),
        }
    }

    #[must_use]
    fn of_giant_flare(flare: &GiantFlare) -> Self {
        Self {
            id: flare.id(),
            onset: flare.onset(),
            end: flare.end(),
            xray: Watts::new(flare.mean_luminosity_watts() * MAGNETAR_GIANT_FLARE_XRAY_SHARE),
        }
    }
}

/// The walk's buffers, kept across a cell's hosts for their allocations.
#[derive(Debug, Default)]
struct Scratch {
    bursts: Vec<Burst>,
    flares: Vec<GiantFlare>,
    outbursts: Vec<Outburst>,
}

/// Seconds in a Julian year, as an integer: the line of sight's year.
const SECONDS_PER_JULIAN_YEAR_I64: i64 = 31_557_600;

/// The first instant of the Julian year, counted from the epoch, that holds `t`.
///
/// # Panics
///
/// Never: a year's start inside the clock's range is on it.
#[must_use]
fn year_start(t: UniverseTime) -> UniverseTime {
    let year = t.seconds().div_euclid(SECONDS_PER_JULIAN_YEAR_I64);
    UniverseTime::new(year * SECONDS_PER_JULIAN_YEAR_I64, 0)
        .expect("a year's start is on the clock")
}

/// Appends to `out` the detections of `host`'s events over `interval` (module documentation, steps
/// 4–6).
#[expect(
    clippy::too_many_arguments,
    reason = "the walk's query, host and buffers, each its own; a private step of the walk"
)]
fn host_detections(
    galaxy: &Galaxy,
    ctx: &mut AlertContext<'_>,
    query: &AlertQuery,
    class: ClassId,
    host: &MagnetarHost,
    interval: TimeWindow,
    scratch: &mut Scratch,
    out: &mut Vec<Detection>,
) {
    let Some(limits) = query.sensor().limits(AlertBand::XRay) else {
        return;
    };
    let star = host.star().to_star();
    let model = MagnetarModel::of(&star)
        .expect("a host's birth field is its star's neutron star's, above the critical field");
    let series = MagnetarSeries::new(galaxy.seed(), host.body());
    let Scratch {
        bursts,
        flares,
        outbursts,
    } = scratch;
    bursts.clear();
    flares.clear();
    outbursts.clear();
    model.bursts_in(&series, interval, bursts);
    model.giant_flares_in(&series, interval, flares);
    outbursts.extend(bursts.iter().map(Outburst::of_burst));
    outbursts.extend(flares.iter().map(Outburst::of_giant_flare));
    outbursts.sort_by(|a, b| a.onset.cmp(&b.onset).then_with(|| a.id.cmp(&b.id)));
    let observer = query.observer_position();
    let window = query.window();
    let motion = host.motion();
    let mut line: Option<(UniverseTime, Sightline)> = None;
    for outburst in outbursts.iter() {
        let site = motion.position_at(outburst.onset);
        let distance = site.distance_to(observer);
        let arrival = arrival_of(outburst.onset, distance);
        let phase = if window.contains(arrival) {
            DetectionPhase::Onset
        } else if arrival < window.start()
            && window.start()
                < arrival_of(
                    outburst.end,
                    motion.position_at(outburst.end).distance_to(observer),
                )
        {
            DetectionPhase::InProgress
        } else {
            continue;
        };
        if !inverse_square_flux(outburst.xray, distance).is_some_and(|bare| bare >= limits.detect())
        {
            continue;
        }
        let year = year_start(outburst.onset);
        if line.as_ref().is_none_or(|(at, _)| *at != year) {
            let from = motion.position_at(year);
            line = Some((year, ctx.sightline(galaxy, &from, observer)));
        }
        let Some((_, sightline)) = line.as_ref() else {
            continue;
        };
        let Some(seen) = flux(outburst.xray, distance, sightline, AlertBand::XRay) else {
            continue;
        };
        if seen < limits.detect() {
            continue;
        }
        let Some(towards) = bearing(observer, &site) else {
            continue;
        };
        let retardation = Retardation::of_emission(
            observer,
            outburst.onset,
            site,
            motion.velocity_at(outburst.onset),
            &motion.position_at(arrival),
        );
        // A top hat at the detection's distance and line of sight (research's ruling of
        // 2026-10-31): its flux at any time of its light is `seen`.
        let light = ArrivingLight::magnetar(
            (outburst.xray, outburst.onset, outburst.end),
            distance,
            transmission(sightline, AlertBand::XRay),
        );
        out.push(Detection::new(
            outburst.id,
            host.host(),
            Some(class),
            TransientKind::MagnetarBurst,
            phase,
            AlertBand::XRay,
            seen,
            towards,
            retardation,
            seen >= limits.resolve(),
            DetectionLight::Lit(light),
        ));
    }
}

/// When light that leaves `distance` from the observer at `emitted` arrives.
///
/// # Panics
///
/// If the arrival falls off the clock, which no light time across the cube can make it do.
#[must_use]
fn arrival_of(emitted: UniverseTime, distance: Metres) -> UniverseTime {
    emitted
        .checked_add(light_time(distance))
        .expect("a light time across the cube keeps a time on the clock")
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::super::light::KILOELECTRONVOLTS_PER_KELVIN;
    use super::*;
    use crate::Seed;
    use crate::alerts::{BandLimits, ClassSet, SensorHorizon, band_share};
    use crate::galaxy::gas::modifiers::NoModifiers;
    use crate::galaxy::gas::noise::NoiseCache;
    use crate::units::{Kelvin, WattsPerSquareMetre};

    /// The giant eruptions' shares are a 5,000 K blackbody's through each band's top hat, to the
    /// bit, none in radio or X-rays, 0.094 in V.
    #[test]
    fn the_eruption_shares_are_a_5000_k_blackbodys() {
        for (band, &stored) in AlertBand::ALL.iter().zip(&GIANT_ERUPTION_SHARES) {
            let share = match band {
                AlertBand::Photometric(_) => band_share(Kelvin::new(5_000.0), *band),
                AlertBand::XRay => 0.0,
            };
            assert_same_bits(stored, share);
        }
        let v = AlertBand::Photometric(crate::galaxy::gas::ccm::Band::V).index();
        assert!((GIANT_ERUPTION_SHARES[v] - 0.094).abs() < 5e-4);
    }

    /// The bursts' share is the mean of two blackbodies' at kT = 5 and 10 keV, to the bit, inside
    /// the 0.066–0.12 Israel et al.'s ends give; the giant flares' is under it, as their spikes are
    /// far harder.
    #[test]
    fn the_burst_share_is_two_blackbodies_of_equal_fluence() {
        let kt = |kev: f64| Kelvin::new(kev / KILOELECTRONVOLTS_PER_KELVIN);
        let share = |kev| band_share(kt(kev), AlertBand::XRay);
        assert_same_bits(
            MAGNETAR_BURST_XRAY_SHARE,
            f64::midpoint(share(5.0), share(10.0)),
        );
        assert!((MAGNETAR_BURST_XRAY_SHARE - 0.106_3).abs() < 1e-4);
        let (soft, hard) = (
            f64::midpoint(share(4.8), share(9.0)),
            f64::midpoint(share(6.2), share(12.0)),
        );
        assert!(hard < MAGNETAR_BURST_XRAY_SHARE && MAGNETAR_BURST_XRAY_SHARE < soft);
        assert!((soft - 0.12).abs() < 0.005 && (hard - 0.066).abs() < 0.005);
        const { assert!(MAGNETAR_GIANT_FLARE_XRAY_SHARE < 0.004) };
    }

    /// A magnetar is active for under 227,273 years: 10¹⁹ G yr ÷ `B_Q` − 10¹⁹ G yr ÷ B₀, 217,273
    /// years for a field born at 10¹⁵ G, and none at or below `B_Q`.
    #[test]
    fn a_magnetar_is_active_for_under_227_273_years() {
        assert!((MAGNETAR_ACTIVE_SPAN.value() - 227_272.73).abs() < 0.01);
        let site = GalacticPosition::from_light_years([0.0, 26_000.0, 20.0]).unwrap();
        let system = SystemId::from_raw(1).unwrap();
        let recipe = StarRecipe::new(
            SolarMasses::new(15.0),
            Composition::SOLAR,
            StarDraws::for_star(Seed::new(1), BodyId::new(system, 0)),
            Years::new(1e7),
        )
        .unwrap();
        let host = |field: f64, velocity: [f64; 3]| {
            let velocity = crate::coords::GalacticVelocity::new(velocity);
            MagnetarHost::new(
                system,
                UniverseTime::EPOCH,
                Gauss::new(field),
                CatalogueMotion::straight(site, velocity),
                recipe.clone(),
            )
        };
        let still = [0.0; 3];
        assert!((host(1e15, still).unwrap().active_span().value() - 217_272.73).abs() < 0.01);
        for field in [QUANTUM_CRITICAL_FIELD.value(), 1e12, f64::NAN] {
            assert!(matches!(
                host(field, still),
                Err(BuildMagnetarHostError::FieldNotAboveCritical(_))
            ));
        }
        assert_eq!(
            host(1e15, [SPEED_OF_LIGHT, 0.0, 0.0]),
            Err(BuildMagnetarHostError::NotSlowerThanLight)
        );
        assert!(matches!(
            StarRecipe::new(
                SolarMasses::new(200.0),
                Composition::SOLAR,
                StarDraws::for_star(Seed::new(1), BodyId::new(system, 0)),
                Years::new(1e7),
            ),
            Err(crate::stellar::system::BuildStarModelError::MassOutsideRange(_))
        ));
    }

    /// The core collapses' magnetar is the entry's own: its host's star is the stellar stage's
    /// primary of the entry's system, whose neutron star is the entry's and dies at its T; and an
    /// entry is a host exactly when that neutron star is born above the critical field.
    #[test]
    fn a_core_collapses_magnetar_is_its_entrys_own() {
        let galaxy = Galaxy::new(Seed::new(9));
        let process = CoreCollapseProcess::new(&galaxy);
        let (mut pulsars, mut magnetars) = (0, 0);
        for y in 46..54 {
            for x in -2..2 {
                let key = CatalogueCellKey::new(ClassId::CORE_COLLAPSE, [x, y, 0]).unwrap();
                for entry in CatalogueClassCell::generate(&galaxy, &process, key).entries() {
                    let entry = entry.value();
                    let Some(pulsar) = entry.supernova().pulsar() else {
                        assert!(process.magnetar_host(&galaxy, entry).is_none());
                        continue;
                    };
                    pulsars += 1;
                    let recipe =
                        StarRecipe::of_record(&galaxy, entry.record(), *entry.composition());
                    let star = recipe.to_star();
                    assert_eq!(star.neutron_star(), Some(pulsar));
                    assert_eq!(&star, entry.stars(&galaxy).primary());
                    let host = process.magnetar_host(&galaxy, entry);
                    assert_eq!(
                        host.is_some(),
                        pulsar.birth_field() > QUANTUM_CRITICAL_FIELD,
                        "{:?}",
                        entry.record().id()
                    );
                    if let Some(host) = host {
                        magnetars += 1;
                        let model = MagnetarModel::of(&star).unwrap();
                        let gap = model
                            .birth()
                            .checked_since(host.birth())
                            .unwrap_or_else(|| host.birth().checked_since(model.birth()).unwrap());
                        assert!(gap.as_seconds_f64() < 1.0, "{gap}");
                        assert_eq!(host.body(), BodyId::new(entry.record().id(), 0));
                    }
                }
            }
        }
        assert!(
            pulsars > 10 && magnetars > 0,
            "{pulsars} pulsars, {magnetars} magnetars"
        );
    }

    /// `scan_plan` and `scan_cell` walk exactly the classes that host a recurrent kind of the
    /// registry, each of whose rows is built: a class added to a recurrent row is walked, or this
    /// fails.
    #[test]
    fn the_walked_classes_are_the_registrys_recurrent_ones() {
        for class in crate::galaxy::catalogue_classes::ALL_CLASSES {
            let hosted = TransientKind::ALL.into_iter().any(|kind| {
                let row = kind.row();
                row.route() == Route::Recurrent && row.classes().contains(class)
            });
            let walked = class == ClassId::CORE_COLLAPSE || is_walked(class);
            assert_eq!(walked, hosted, "{class}");
        }
        for kind in TransientKind::ALL {
            if kind.row().route() == Route::Recurrent {
                assert_eq!(kind.row().pending(), None, "{kind:?}");
            }
        }
    }

    /// The classes with no recurrent kind walked here, and those whose recurrent kinds a sensor
    /// cannot see, plan and scan nothing: an X-ray sensor sees no nova and no giant eruption, but
    /// the X-ray binaries' transients; a query that does not ask about a class plans nothing for
    /// it.
    #[test]
    fn classes_without_a_walked_kind_the_sensor_sees_scan_nothing() {
        let galaxy = Galaxy::new(Seed::new(9));
        let w = WattsPerSquareMetre::new;
        let xray = BandLimits::new(w(0.04), w(0.4)).unwrap();
        let sensor = SensorHorizon::new([(AlertBand::XRay, xray)]).unwrap();
        let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 20.0]).unwrap();
        let year = TimeWindow::new(
            UniverseTime::EPOCH,
            UniverseTime::from_julian_years(1).unwrap(),
        )
        .unwrap();
        let query = AlertQuery::builder(sun, year, sensor.clone())
            .build()
            .unwrap();
        let mut noise = NoiseCache::with_capacity(64);
        let mut context = AlertContext::new(&mut noise, &NoModifiers);
        for class in [
            ClassId::ACCRETING_WHITE_DWARF,
            ClassId::ACCRETING_WHITE_DWARF_SLOW,
            ClassId::LUMINOUS_BLUE_VARIABLE,
            ClassId::TYPE_IA,
            ClassId::STELLAR_MERGER,
        ] {
            assert_eq!(scan_plan(&galaxy, class, &query), [], "{class}");
            let cell = CatalogueCellKey::containing(class, &sun).unwrap();
            let mut out = Vec::new();
            scan_cell(&galaxy, &mut context, class, cell, &query, &mut out);
            assert_eq!(out, [], "{class}");
        }
        for class in [ClassId::CORE_COLLAPSE, ClassId::XRAY_BINARY] {
            assert_ne!(scan_plan(&galaxy, class, &query).len(), 0, "{class}");
        }
        assert_eq!(
            recurrent_scan_plan(ClassId::CORE_COLLAPSE, &query),
            magnetar_scan_plan(ClassId::CORE_COLLAPSE, &query)
        );
        let others = AlertQuery::builder(sun, year, sensor)
            .classes(ClassSet::of(ClassId::TYPE_IA))
            .build()
            .unwrap();
        for class in [ClassId::CORE_COLLAPSE, ClassId::XRAY_BINARY] {
            assert_eq!(scan_plan(&galaxy, class, &others), [], "{class}");
        }
    }
}
