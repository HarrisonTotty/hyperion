//! The one-shot classes' walk (plan 12, P12.T5.b): the supernovae, luminous red novae and
//! kilonovae a sensor sees go off in its window, each the one event of a catalogue entry.
//!
//! Four classes are one-shot ([`OneShotClass`]): plan 09's core collapses and Type Ia, whose
//! entries explode ([`SupernovaEntry`]), and plan 11's stellar mergers and neutron-star mergers,
//! each of whose entries makes one event at its merger, a luminous red nova or a kilonova
//! (plan 11's [`BinarySystem::stellar_merger`] and [`BinarySystem::neutron_star_merger`] of the
//! entry's scan marks). A merger of a black hole that swallows its neutron star whole makes none.
//! Each event's light is a supernova template's or a binary curve's ([`OneShotLight`]).
//!
//! A one-shot entry makes one event, at its time T from its site (Design note 6). Its light reaches
//! an observer at rest at `t_a` = T + |`x_s`(T) − `x_o`| ÷ c, a closed form with no step on the
//! light cone, and the event belongs to the window `[t₁, t₂)` that holds `t_a`. So a window split
//! in two finds each onset exactly once.
//!
//! # The walk
//!
//! [`one_shot_detections`] takes one class, through its process ([`OneShotClass`]):
//!
//! 1. Its reach for the query's sensor: [`kind_reach`] of the kind its entries make. The class's
//!    census reads [`class_reach`](super::class_reach), which the core collapses' magnetars lift
//!    (P12.T5.c); the walk reads its own kind's.
//! 2. The class's cells that can hold an entry whose site lies within the reach of the observer
//!    ([`cells_in_sphere`], padded by the class grid's 4,096 ly). An entry stays within 4,096 ly of
//!    its cell at every time the lookup serves, from −(H + L) on (P09.T32, rule 5). No template
//!    gives light 228 years after its explosion (every term's exponential is then below the
//!    smallest `f64`), and no binary event once it is over, some 280 days at most
//!    ([`binary_luminosity`]), so an entry the sensor can see has its light arriving after −H −
//!    228 years, and with at most the cube's diagonal to cross it went off after −(H + L), when the
//!    rule holds it. A merger's T lies in the source horizon too. Design note 5's widening by
//!    `UNBOUND_PAD_SPEED` ÷ c is for hosts walked on their present positions; the walk reads each
//!    entry's site at T.
//! 3. Each entry's arrival. One that arrives in the window is an [`Onset`], seen at its peak in
//!    each band ([`OneShotLight::peaks`]); one that arrived before `t₁` is [`InProgress`] if its
//!    light at `t₁`, `t₁` − `t_a` after its onset reached the observer
//!    ([`OneShotLight::luminosities`]), is still above the limit. One that
//!    arrives at or after `t₂` is the next window's. So a window split in two finds each onset of
//!    the whole once, and its first half the whole's events in progress; the second half finds in
//!    progress the events that arrived before it and are still above the limit at its start,
//!    whether or not the first half saw them begin.
//! 4. The flux in each of the sensor's bands, through plan 07's line of sight from the site to the
//!    observer ([`AlertContext::sightline`], [`flux`]), which the walk marches only for an entry
//!    whose unextinguished flux reaches a band's limit. The event is detected in the band in which
//!    its flux stands highest over the detection limit, the first in [`AlertBand::ALL`]'s order
//!    on a tie, if it reaches the limit there.
//!
//! The walk is complete for any reach, the cube's diagonal included; whether a class is walked
//! interactively at all is the census's (P12.T5.d, Design note 7), and a class it sends to the
//! background is walked cell by cell ([`one_shot_scan_plan`], [`one_shot_scan_cell`]), which
//! merge to the same detections. A class's cells cost milliseconds each (P09.T33.a), so the
//! caller lends the walk a cache of them ([`OneShotCells`], [`AlertContext::with_one_shot_cells`]).
//!
//! # What a detection reports
//!
//! The entry's event, the same from any distance: a supernova's explosion
//! ([`SupernovaEntry::explosion_event`]), or a merger's red nova or kilonova (plan 11's
//! [`BinaryEvent::id`], bin 0 and number 0 of its tag on the system); its member 0 as the host,
//! its class and kind, T and `t_a`; the band and flux above; the bearing of its site from the
//! observer; the onset's light ([`Retardation`]: observed at `t_a`, emitted at T from the site, the
//! host then moving at its velocity from T, a supernova's remnant's); whether the flux reaches the
//! band's resolve limit; and the event's light in the band at every time ([`DetectionLight`],
//! P12.T8.b.1), through the same line of sight from the site.
//!
//! [`Onset`]: DetectionPhase::Onset
//! [`InProgress`]: DetectionPhase::InProgress
//! [`Retardation`]: crate::observe::Retardation

use std::ops::Range;

use crate::coords::GalacticPosition;
use crate::events::TimeWindow;
use crate::galaxy::Galaxy;
use crate::galaxy::catalogue_classes::binary::feature::ListedBinary;
use crate::galaxy::catalogue_classes::binary::{
    MergerEntry, NeutronStarMergers, NsmEntry, StellarMergers,
};
use crate::galaxy::catalogue_classes::core_collapse::{CoreCollapseEntry, CoreCollapseProcess};
use crate::galaxy::catalogue_classes::supernova::{
    SupernovaEntry, SupernovaKind, explosion_event_of,
};
use crate::galaxy::catalogue_classes::type_ia::{TypeIaEntry, TypeIaProcess};
use crate::galaxy::catalogue_classes::{
    CatalogueCellKey, CatalogueClassCell, CatalogueMotion, ClassId, ClassProcess, cells_in_sphere,
    entries_when_in,
};
use crate::galaxy::query::QuerySphere;
use crate::id::{EventId, SystemId};
use crate::observe::{Retardation, bearing, light_time};
use crate::stellar::binary::events::{BinaryEvent, BinarySystem, events_in};
use crate::stellar::binary::light_curves::BinaryLightCurve;
use crate::stellar::binary::scan::{scan_merger_cell_in, scan_nsm_cell_in};
use crate::time::{Span, UniverseTime};
use crate::units::{LightYears, Metres, Watts, WattsPerSquareMetre, Years};

use super::arriving::{ArrivingLight, DetectionLight};
use super::cut;
use super::light::{binary_luminosity, binary_peak, supernova_luminosities, supernova_peak};
use super::reach::{flux, inverse_square_flux, kind_reach, transmission};
use super::registry::TransientKind;
use super::{AlertBand, AlertContext, AlertQuery, BAND_COUNT, Detection, DetectionPhase};

/// A one-shot event's light (P12.T5.b): a supernova's template, or a merger's binary curve.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OneShotLight {
    /// A supernova of this kind: plan 09's template in each band ([`supernova_luminosity`]).
    ///
    /// [`supernova_luminosity`]: super::supernova_luminosity
    Supernova(SupernovaKind),
    /// A luminous red nova or a kilonova: plan 11's curve (P11.T9.a) in each band while the event
    /// is active ([`binary_luminosity`]).
    Binary(BinaryLightCurve),
}

impl OneShotLight {
    /// The light's peak in each band, in [`AlertBand::ALL`]'s order: [`supernova_peak`] or
    /// [`binary_peak`], which an onset reports.
    #[must_use]
    pub fn peaks(&self) -> [Watts; BAND_COUNT] {
        match self {
            Self::Supernova(kind) => AlertBand::ALL.map(|band| supernova_peak(*kind, band)),
            Self::Binary(curve) => AlertBand::ALL.map(|band| binary_peak(curve, band)),
        }
    }

    /// The light in each band `age` after its onset, in [`AlertBand::ALL`]'s order: a supernova's
    /// template at `age` in Julian years, bit for bit as
    /// [`supernova_luminosity`](super::supernova_luminosity) gives it band by band, or
    /// [`binary_luminosity`].
    #[must_use]
    pub fn luminosities(&self, age: Span) -> [Watts; BAND_COUNT] {
        match self {
            Self::Supernova(kind) => {
                supernova_luminosities(*kind, Years::new(age.as_julian_years_f64()))
            }
            Self::Binary(curve) => AlertBand::ALL.map(|band| binary_luminosity(curve, age, band)),
        }
    }
}

/// One entry's one-shot event, as the walk reads it: the event, its host, its light, when it went
/// off, and how the host moves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OneShotEvent {
    event: EventId,
    host: SystemId,
    light: OneShotLight,
    emitted: UniverseTime,
    motion: CatalogueMotion,
}

impl OneShotEvent {
    /// The explosion of `entry`, whose member 0 moves by `motion`.
    #[must_use]
    pub fn of_supernova(entry: &SupernovaEntry, motion: CatalogueMotion) -> Self {
        Self {
            event: entry.explosion_event(),
            host: entry.system(),
            light: OneShotLight::Supernova(entry.kind()),
            emitted: entry.explosion(),
            motion,
        }
    }

    /// The explosion of `kind` at `explosion` of member 0 `host`, which moves by `motion`: a death a
    /// feature's or the centre's list holds (P09.T36), its event the one a [`SupernovaEntry`] of
    /// it names ([`SupernovaEntry::explosion_event`]).
    #[must_use]
    pub(crate) fn of_explosion(
        host: SystemId,
        explosion: UniverseTime,
        kind: SupernovaKind,
        motion: CatalogueMotion,
    ) -> Self {
        Self {
            event: explosion_event_of(host),
            host,
            light: OneShotLight::Supernova(kind),
            emitted: explosion,
            motion,
        }
    }

    /// The binary event `event` of `host`, which moves by `motion`: a merger's luminous red nova or
    /// kilonova, at its onset.
    #[must_use]
    pub const fn of_binary(event: &BinaryEvent, host: SystemId, motion: CatalogueMotion) -> Self {
        Self {
            event: event.id(),
            host,
            light: OneShotLight::Binary(*event.curve()),
            emitted: event.onset(),
            motion,
        }
    }

    /// The event's ID: a supernova's explosion's, or a merger's binary event's.
    #[must_use]
    pub const fn event(&self) -> EventId {
        self.event
    }

    /// The host: the entry's member 0.
    #[must_use]
    pub const fn host(&self) -> SystemId {
        self.host
    }

    /// Its light: what exploded, or the merger's curve.
    #[must_use]
    pub const fn light(&self) -> &OneShotLight {
        &self.light
    }

    /// When it went off, T.
    #[must_use]
    pub const fn emitted(&self) -> UniverseTime {
        self.emitted
    }

    /// Where it went off: the host's position at T.
    #[must_use]
    pub fn site(&self) -> GalacticPosition {
        self.motion.position_at(self.emitted)
    }

    /// How the host moves, before T and after it.
    #[must_use]
    pub const fn motion(&self) -> &CatalogueMotion {
        &self.motion
    }
}

/// A one-shot class's process, as the walk reads it (P12.T5.b): the kind of event its entries
/// make, and each entry's.
///
/// Implemented for plan 09's core collapses ([`CoreCollapseProcess`]) and its Type Ia
/// ([`TypeIaProcess`], P09.T34.a), each from its entries' [`SupernovaEntry`] and member 0's
/// motion, and for plan 11's stellar mergers ([`StellarMergers`]) and neutron-star mergers
/// ([`NeutronStarMergers`]), from each entry's one binary event of its scan marks (P11.T9.b), at
/// its T and moving with the entry.
pub trait OneShotClass: ClassProcess {
    /// The kind of event the class's entries make.
    #[must_use]
    fn kind(&self) -> TransientKind;

    /// The event `entry` of `galaxy` makes, or `None` for one that makes none: a black hole's merger
    /// that ejects nothing.
    #[must_use]
    fn event(&self, galaxy: &Galaxy, entry: &Self::Entry) -> Option<OneShotEvent>;

    /// The events of the class's cell `key` of `galaxy`, in index order: by default each entry's
    /// [`event`](Self::event). A class whose scan marks give its entries' events without building
    /// the entries may read them so, and must give exactly the default's events, in its order.
    #[must_use]
    fn cell_events(&self, galaxy: &Galaxy, key: CatalogueCellKey) -> Vec<OneShotEvent>
    where
        Self: Sized,
    {
        CatalogueClassCell::generate(galaxy, self, key)
            .entries()
            .iter()
            .filter_map(|entry| self.event(galaxy, entry.value()))
            .collect()
    }

    /// The events of the class's cell `key` whose time `wanted` may accept, in index order.
    ///
    /// They are [`cell_events`](Self::cell_events)' own, less any of a candidate whose time, read
    /// first, `wanted` refuses (plan 12, P12.T8.b.2): those of all its candidates
    /// ([`cell_events_when_in`](Self::cell_events_when_in) of `0..u32::MAX`). The walks read a
    /// cell through `cell_events_when_in` alone, so a class that reads its events its own way
    /// overrides that method, never this one.
    #[must_use]
    fn cell_events_when(
        &self,
        galaxy: &Galaxy,
        key: CatalogueCellKey,
        wanted: impl Fn(UniverseTime) -> bool,
    ) -> Vec<OneShotEvent>
    where
        Self: Sized,
    {
        self.cell_events_when_in(galaxy, key, 0..u32::MAX, wanted)
    }

    /// The events of [`cell_events_when`](Self::cell_events_when) of the candidates of cell `key`
    /// whose indices lie in `indices`, held to the cell's count, in index order.
    ///
    /// They are a part of the cell's candidates, which a background walk splits a dense cell into
    /// (plan 12). By default those of the entries [`entries_when_in`] keeps; a class whose
    /// candidates read no time first gives every event of the part, and may give them its own
    /// way. Over the parts of any partition of `0..u32::MAX`, in order, they must be the whole
    /// cell's events exactly.
    #[must_use]
    fn cell_events_when_in(
        &self,
        galaxy: &Galaxy,
        key: CatalogueCellKey,
        indices: Range<u32>,
        wanted: impl Fn(UniverseTime) -> bool,
    ) -> Vec<OneShotEvent>
    where
        Self: Sized,
    {
        entries_when_in(galaxy, self, key, indices, wanted)
            .iter()
            .filter_map(|entry| self.event(galaxy, entry.value()))
            .collect()
    }
}

impl OneShotClass for CoreCollapseProcess {
    fn kind(&self) -> TransientKind {
        TransientKind::CoreCollapse
    }

    fn event(&self, _: &Galaxy, entry: &CoreCollapseEntry) -> Option<OneShotEvent> {
        Some(OneShotEvent::of_supernova(
            entry.supernova(),
            *entry.motion(),
        ))
    }
}

impl OneShotClass for TypeIaProcess {
    fn kind(&self) -> TransientKind {
        TransientKind::TypeIa
    }

    /// The explosion of the entry's [`SupernovaEntry`], member 0, the white dwarf's system, moving
    /// as it does before and after T.
    fn event(&self, _: &Galaxy, entry: &TypeIaEntry) -> Option<OneShotEvent> {
        Some(OneShotEvent::of_supernova(
            entry.supernova(),
            *entry.motion(),
        ))
    }
}

impl OneShotClass for StellarMergers {
    fn kind(&self) -> TransientKind {
        TransientKind::LuminousRedNova
    }

    fn event(&self, galaxy: &Galaxy, entry: &MergerEntry) -> Option<OneShotEvent> {
        stellar_merger_event(galaxy, entry)
    }

    /// The cell's events from its scan marks, which are its entries' exactly (the marks of all its
    /// candidates, [`scan_merger_cell`]'s), without the contact pairs and merged stars the
    /// entries build.
    ///
    /// [`scan_merger_cell`]: crate::stellar::binary::scan::scan_merger_cell
    fn cell_events(&self, galaxy: &Galaxy, key: CatalogueCellKey) -> Vec<OneShotEvent> {
        self.cell_events_when_in(galaxy, key, 0..u32::MAX, |_| true)
    }

    /// Every event of the candidates `indices` of the cell, from their scan marks (those of
    /// [`scan_merger_cell`] of the indices): a merger's candidates read no time first.
    ///
    /// [`scan_merger_cell`]: crate::stellar::binary::scan::scan_merger_cell
    fn cell_events_when_in(
        &self,
        galaxy: &Galaxy,
        key: CatalogueCellKey,
        indices: Range<u32>,
        _wanted: impl Fn(UniverseTime) -> bool,
    ) -> Vec<OneShotEvent> {
        let mut marks = Vec::new();
        scan_merger_cell_in(galaxy, self, key, indices, &mut marks);
        let mut listed = Vec::with_capacity(1);
        marks
            .iter()
            .filter_map(|scan| {
                let motion = CatalogueMotion::straight(*scan.position(), scan.velocity());
                let system = BinarySystem::stellar_merger(galaxy.seed(), scan);
                merger_event(&system, scan.merger_time(), scan.id(), motion, &mut listed)
            })
            .collect()
    }
}

impl OneShotClass for NeutronStarMergers {
    fn kind(&self) -> TransientKind {
        TransientKind::Kilonova
    }

    fn event(&self, galaxy: &Galaxy, entry: &NsmEntry) -> Option<OneShotEvent> {
        neutron_star_merger_event(galaxy, entry)
    }

    /// The cell's events from its scan marks, which are its entries' exactly (the marks of all its
    /// candidates, [`scan_nsm_cell`]'s), without the inspirals and remnants the entries build.
    ///
    /// [`scan_nsm_cell`]: crate::stellar::binary::scan::scan_nsm_cell
    fn cell_events(&self, galaxy: &Galaxy, key: CatalogueCellKey) -> Vec<OneShotEvent> {
        self.cell_events_when_in(galaxy, key, 0..u32::MAX, |_| true)
    }

    /// Every event of the candidates `indices` of the cell, from their scan marks (those of
    /// [`scan_nsm_cell`] of the indices): a merger's candidates read no time first.
    ///
    /// [`scan_nsm_cell`]: crate::stellar::binary::scan::scan_nsm_cell
    fn cell_events_when_in(
        &self,
        galaxy: &Galaxy,
        key: CatalogueCellKey,
        indices: Range<u32>,
        _wanted: impl Fn(UniverseTime) -> bool,
    ) -> Vec<OneShotEvent> {
        let mut marks = Vec::new();
        scan_nsm_cell_in(galaxy, self, key, indices, &mut marks);
        let mut listed = Vec::with_capacity(1);
        marks
            .iter()
            .filter_map(|scan| {
                let motion = CatalogueMotion::straight(*scan.position(), scan.velocity());
                let system = BinarySystem::neutron_star_merger(galaxy.seed(), scan);
                merger_event(&system, scan.merger_time(), scan.id(), motion, &mut listed)
            })
            .collect()
    }
}

/// The luminous red nova of the stellar merger `entry` of `galaxy`, at its T and moving with it, or
/// `None` where it makes none: a cell's entry's ([`StellarMergers`]' event) or a feature's or the
/// galactic centre's list's (P09.T36 in the walks).
#[must_use]
fn stellar_merger_event(galaxy: &Galaxy, entry: &MergerEntry) -> Option<OneShotEvent> {
    let scan = entry.scan();
    let system = BinarySystem::stellar_merger(galaxy.seed(), scan);
    merger_event(
        &system,
        scan.merger_time(),
        entry.record().id(),
        *entry.motion(),
        &mut Vec::with_capacity(1),
    )
}

/// The kilonova of the neutron-star merger `entry` of `galaxy`, at its T and moving with it, or
/// `None` where it makes none, as [`stellar_merger_event`] (a black hole that swallows its neutron
/// star whole).
#[must_use]
fn neutron_star_merger_event(galaxy: &Galaxy, entry: &NsmEntry) -> Option<OneShotEvent> {
    let scan = entry.scan();
    let system = BinarySystem::neutron_star_merger(galaxy.seed(), scan);
    merger_event(
        &system,
        scan.merger_time(),
        entry.record().id(),
        *entry.motion(),
        &mut Vec::with_capacity(1),
    )
}

/// The one-shot event of the merger `entry` on a feature's or the galactic centre's list
/// (P11.T8.f), as its class's walk makes a cell's entry's: a stellar merger's luminous red nova or
/// a neutron-star merger's kilonova; `None` for an entry of another class or a merger that makes
/// none.
#[must_use]
pub(super) fn listed_merger_event(galaxy: &Galaxy, entry: &ListedBinary) -> Option<OneShotEvent> {
    match entry {
        ListedBinary::StellarMerger(entry) => stellar_merger_event(galaxy, entry),
        ListedBinary::NeutronStarMerger(entry) => neutron_star_merger_event(galaxy, entry),
        ListedBinary::AccretingWd(_) | ListedBinary::XrayBinary(_) => None,
    }
}

/// The one event of the merger `system`, of `host`, at its merger `at`, moving by `motion`: plan
/// 11's listing over the nanosecond from `at`, into `events` (cleared first, a cell's walk lending
/// the same vector to each of its mergers), or `None` where it lists none.
///
/// # Panics
///
/// Never: a nanosecond after a time in the source horizon is on the clock.
#[must_use]
fn merger_event(
    system: &BinarySystem<'_>,
    at: UniverseTime,
    host: SystemId,
    motion: CatalogueMotion,
    events: &mut Vec<BinaryEvent>,
) -> Option<OneShotEvent> {
    let after = at
        .checked_add(Span::new(0, 1).expect("a nanosecond is a span"))
        .expect("a nanosecond after a merger in the source horizon is on the clock");
    let window = TimeWindow::new(at, after).expect("a nanosecond's window runs forward");
    events.clear();
    events_in(system, window, events);
    debug_assert!(events.len() <= 1, "a merger makes one event: {events:?}");
    events
        .first()
        .map(|event| OneShotEvent::of_binary(event, host, motion))
}

/// Lends the one-shot walk a class cell's events, generating them if it has not got them (P12.T5.b).
///
/// A class's cell is a pure function of the galaxy and its key, and costs milliseconds (P09.T33.a),
/// so a caller that walks the same cells again (the next window, another sensor) keeps them. An
/// implementation must lend exactly what `generate` returns. The walk names a cell by its key alone,
/// which holds its class, so a cache serves one galaxy and one process per class, as the production
/// lookup has: a caller keeps one with its galaxy (the server's caches are per universe), and a
/// galaxy's seed would not name it, since its parameters and rules shape the cells too. Nothing the
/// walk returns then depends on what the cache held.
///
/// # Examples
///
/// ```
/// use std::collections::BTreeMap;
///
/// use hyperion_sim::alerts::oneshot::{OneShotCells, OneShotEvent};
/// use hyperion_sim::galaxy::catalogue_classes::CatalogueCellKey;
///
/// /// Keeps every cell it is asked for, for one galaxy.
/// #[derive(Debug, Default)]
/// struct KeepCells(BTreeMap<CatalogueCellKey, Vec<OneShotEvent>>);
///
/// impl OneShotCells for KeepCells {
///     fn with_events(
///         &mut self,
///         key: CatalogueCellKey,
///         generate: &mut dyn FnMut() -> Vec<OneShotEvent>,
///         visit: &mut dyn FnMut(&[OneShotEvent]),
///     ) {
///         visit(self.0.entry(key).or_insert_with(generate));
///     }
/// }
/// # let _ = KeepCells::default();
/// ```
pub trait OneShotCells {
    /// Calls `visit` with the events of the cell `key`, which `generate` computes.
    fn with_events(
        &mut self,
        key: CatalogueCellKey,
        generate: &mut dyn FnMut() -> Vec<OneShotEvent>,
        visit: &mut dyn FnMut(&[OneShotEvent]),
    );
}

/// The cache that keeps nothing: every cell is generated for its visit and dropped.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NoOneShotCells;

impl OneShotCells for NoOneShotCells {
    fn with_events(
        &mut self,
        _key: CatalogueCellKey,
        generate: &mut dyn FnMut() -> Vec<OneShotEvent>,
        visit: &mut dyn FnMut(&[OneShotEvent]),
    ) {
        visit(&generate());
    }
}

/// An event the walk keeps for its line of sight: its arrival and phase, and its luminosity in
/// each band when the phase reads it.
#[derive(Debug, Clone, Copy)]
pub(super) struct Candidate {
    event: OneShotEvent,
    arrival: UniverseTime,
    phase: DetectionPhase,
    distance: Metres,
    luminosity: [Watts; BAND_COUNT],
}

/// The detections of `process`'s class that `query`'s sensor sees in its window (module
/// documentation), appended to `out` by arrival and then by event.
///
/// Nothing is appended for a class the query does not ask about, or for a sensor whose bands its
/// kind gives no light in. The detections do not depend on what the context's caches hold.
///
/// # Panics
///
/// If the reach is not finite, which [`kind_reach`] never gives; and as
/// [`CatalogueClassCell::generate`] and the class's process do.
///
/// # Examples
///
/// The core collapses a visual sensor near the Sun sees go off in a year, which is almost always
/// none: some two a century in the whole Galaxy, and the sensor's reach a few thousand light-years.
///
/// ```no_run
/// use hyperion_sim::Seed;
/// use hyperion_sim::alerts::oneshot::one_shot_detections;
/// use hyperion_sim::alerts::{AlertBand, AlertContext, AlertQuery, BandLimits, SensorHorizon};
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::events::TimeWindow;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::catalogue_classes::core_collapse::CoreCollapseProcess;
/// use hyperion_sim::galaxy::gas::ccm::Band;
/// use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
/// use hyperion_sim::galaxy::gas::noise::NoiseCache;
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::WattsPerSquareMetre;
///
/// let galaxy = Galaxy::new(Seed::new(9));
/// let w = WattsPerSquareMetre::new;
/// let v = BandLimits::new(w(1e-6), w(1e-5))?;
/// let sensor = SensorHorizon::new([(AlertBand::Photometric(Band::V), v)])?;
/// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 20.0]).ok_or("in the cube")?;
/// let year = TimeWindow::new(UniverseTime::EPOCH, UniverseTime::from_julian_years(1).ok_or("on the clock")?)?;
/// let query = AlertQuery::builder(sun, year, sensor).build()?;
/// let mut noise = NoiseCache::with_capacity(4_096);
/// let mut context = AlertContext::new(&mut noise, &NoModifiers);
/// let mut seen = Vec::new();
/// one_shot_detections(&galaxy, &mut context, &query, &CoreCollapseProcess::new(&galaxy), &mut seen);
/// for detection in &seen {
///     assert!(year.contains(detection.arrival()) || detection.arrival() < year.start());
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn one_shot_detections<P: OneShotClass>(
    galaxy: &Galaxy,
    ctx: &mut AlertContext<'_>,
    query: &AlertQuery,
    process: &P,
    out: &mut Vec<Detection>,
) {
    let class = process.class();
    let kind = process.kind();
    let Some(sphere) = walk_sphere(process, query) else {
        return;
    };
    let observer = *query.observer_position();
    let mut candidates = Vec::new();
    for key in cells_in_sphere(class, &sphere) {
        ctx.one_shot_events(
            key,
            &mut || process.cell_events(galaxy, key),
            &mut |events| {
                candidates.extend(
                    events
                        .iter()
                        .filter_map(|event| candidate(event, &observer, query)),
                );
            },
        );
    }
    let start = out.len();
    for candidate in candidates {
        if let Some(detection) = detect(galaxy, ctx, query, class, kind, &candidate) {
            out.push(detection);
        }
    }
    out[start..].sort_by(|a, b| {
        a.arrival
            .cmp(&b.arrival)
            .then_with(|| a.event.cmp(&b.event))
    });
}

/// The sphere `process`'s walk covers for `query`: its kind's reach about the observer at the
/// window's start (module documentation, step 2), or `None` if the query does not ask about the
/// class or the sensor sees nothing of its kind.
///
/// # Panics
///
/// If the reach is not finite, which [`kind_reach`] never gives.
#[must_use]
fn walk_sphere<P: OneShotClass>(process: &P, query: &AlertQuery) -> Option<QuerySphere> {
    if !query.classes().contains(process.class()) {
        return None;
    }
    let reach = kind_reach(process.kind(), query.sensor());
    (reach > LightYears::ZERO).then(|| {
        QuerySphere::new(
            *query.observer_position(),
            reach,
            query.window().start(),
            LightYears::ZERO,
        )
        .expect("a reach is a positive length within the cube's diagonal")
    })
}

/// The cells of `process`'s class that [`one_shot_detections`] walks for `query`, in key order:
/// the class's cells within its kind's reach ([`cells_in_sphere`]); none if the query does not ask
/// about the class or the sensor sees nothing of its kind (P12.T5.d).
///
/// A class that `alerts_in`'s census sends to the background (Design note 7) is walked in chunks
/// of these with [`one_shot_scan_cell`], as the recurrent kinds are with
/// [`scan_plan`](super::scan_plan).
///
/// # Panics
///
/// If the reach is not finite, which [`kind_reach`] never gives.
#[must_use]
pub fn one_shot_scan_plan<P: OneShotClass>(
    process: &P,
    query: &AlertQuery,
) -> Vec<CatalogueCellKey> {
    walk_sphere(process, query).map_or_else(Vec::new, |sphere| {
        cells_in_sphere(process.class(), &sphere).collect()
    })
}

/// Appends to `out` the detections of the one-shot events of `process`'s cell `cell` that
/// `query`'s sensor sees in its window, sorted by arrival and then by event (P12.T5.d): nothing for
/// a cell of another class, or one the query does not ask about.
///
/// A pure function of one class cell, which a background scan runs in chunks: over every cell of
/// [`one_shot_scan_plan`], merged by arrival and then by event, the detections are
/// [`one_shot_detections`]' own, each to the bit, in any order of the cells and with any cache.
/// A cell beyond the plan adds nothing, since no event beyond its kind's reach reaches the limit.
///
/// It builds only the candidates whose time, read first, can light the window from the cell
/// ([`cell_events_when`](OneShotClass::cell_events_when) with the one-shot cut, P12.T8.b.2), a
/// few in ten thousand of a supernova cell's over a year, and so never takes the cell from the
/// context's [`OneShotCells`], which hold whole cells.
///
/// # Panics
///
/// As [`one_shot_detections`].
pub fn one_shot_scan_cell<P: OneShotClass>(
    galaxy: &Galaxy,
    ctx: &mut AlertContext<'_>,
    query: &AlertQuery,
    process: &P,
    cell: CatalogueCellKey,
    out: &mut Vec<Detection>,
) {
    one_shot_scan_cell_in(galaxy, ctx, query, process, cell, 0..u32::MAX, out);
}

/// Appends to `out` [`one_shot_scan_cell`]'s detections of the candidates of `cell` whose indices
/// lie in `indices`, held to the cell's count, sorted by arrival and then by event.
///
/// It walks a part of a dense cell, which a background walk splits among its jobs (plan 12). Over
/// the parts of any partition of `0..u32::MAX` (or of `0..` the cell's count), merged by
/// arrival and then by event with a stable sort, in the parts' order, the detections are
/// `one_shot_scan_cell`'s own, each to the bit and in its order: a part's candidates are the
/// whole cell's of those indices ([`cell_events_when_in`](OneShotClass::cell_events_when_in)), and
/// each is detected or not on its own.
///
/// # Panics
///
/// As [`one_shot_detections`].
///
/// # Examples
///
/// A cell's core collapses that a visual sensor near the Sun sees over a year, walked in two parts
/// of its candidates as two jobs could take them, and merged as the whole cell's walk sorts them:
///
/// ```no_run
/// use hyperion_sim::Seed;
/// use hyperion_sim::alerts::{
///     AlertBand, AlertContext, AlertQuery, BandLimits, SensorHorizon, one_shot_scan_cell,
///     one_shot_scan_cell_in,
/// };
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::events::TimeWindow;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::catalogue_classes::core_collapse::CoreCollapseProcess;
/// use hyperion_sim::galaxy::catalogue_classes::{CatalogueCellKey, ClassId, occupied_cells};
/// use hyperion_sim::galaxy::gas::ccm::Band;
/// use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
/// use hyperion_sim::galaxy::gas::noise::NoiseCache;
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::WattsPerSquareMetre;
///
/// let galaxy = Galaxy::new(Seed::new(9));
/// let w = WattsPerSquareMetre::new;
/// let v = BandLimits::new(w(1e-6), w(1e-5))?;
/// let sensor = SensorHorizon::new([(AlertBand::Photometric(Band::V), v)])?;
/// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 20.0]).ok_or("in the cube")?;
/// let end = UniverseTime::from_julian_years(1).ok_or("on the clock")?;
/// let year = TimeWindow::new(UniverseTime::EPOCH, end)?;
/// let query = AlertQuery::builder(sun, year, sensor).build()?;
/// let process = CoreCollapseProcess::new(&galaxy);
/// let cell = CatalogueCellKey::new(ClassId::CORE_COLLAPSE, [0, 50, 0])?;
/// let count = occupied_cells(&galaxy, &process, [cell]).first().map_or(0, |&(_, n)| n);
/// let mut noise = NoiseCache::with_capacity(4_096);
/// let mut ctx = AlertContext::new(&mut noise, &NoModifiers);
/// let mut parts = Vec::new();
/// for half in [0..count / 2, count / 2..count] {
///     one_shot_scan_cell_in(&galaxy, &mut ctx, &query, &process, cell, half, &mut parts);
/// }
/// parts.sort_by(|a, b| a.arrival().cmp(&b.arrival()).then_with(|| a.event().cmp(&b.event())));
/// let mut whole = Vec::new();
/// one_shot_scan_cell(&galaxy, &mut ctx, &query, &process, cell, &mut whole);
/// assert_eq!(parts, whole);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn one_shot_scan_cell_in<P: OneShotClass>(
    galaxy: &Galaxy,
    ctx: &mut AlertContext<'_>,
    query: &AlertQuery,
    process: &P,
    cell: CatalogueCellKey,
    indices: Range<u32>,
    out: &mut Vec<Detection>,
) {
    let class = process.class();
    if cell.class() != class || !query.classes().contains(class) {
        return;
    }
    let kind = process.kind();
    let observer = *query.observer_position();
    let candidates: Vec<Candidate> = process
        .cell_events_when_in(galaxy, cell, indices, cut::one_shot(query, &cell))
        .iter()
        .filter_map(|event| candidate(event, &observer, query))
        .collect();
    let start = out.len();
    for candidate in candidates {
        if let Some(detection) = detect(galaxy, ctx, query, class, kind, &candidate) {
            out.push(detection);
        }
    }
    out[start..].sort_by(|a, b| {
        a.arrival
            .cmp(&b.arrival)
            .then_with(|| a.event.cmp(&b.event))
    });
}

/// `event` as the walk keeps it for `query`, if its light arrives in the window or arrived before
/// it, and its unextinguished flux then reaches the detection limit in one of the sensor's bands.
#[must_use]
pub(super) fn candidate(
    event: &OneShotEvent,
    observer: &GalacticPosition,
    query: &AlertQuery,
) -> Option<Candidate> {
    let window = query.window();
    if event.emitted >= window.end() {
        return None;
    }
    let site = event.site();
    let distance = site.distance_to(observer);
    let arrival = event
        .emitted
        .checked_add(light_time(distance))
        .expect("a light time across the cube keeps a time on the clock");
    let phase = if window.contains(arrival) {
        DetectionPhase::Onset
    } else if arrival < window.start() {
        DetectionPhase::InProgress
    } else {
        return None;
    };
    let age = match phase {
        DetectionPhase::Onset => None,
        DetectionPhase::InProgress => Some(
            window
                .start()
                .checked_since(arrival)
                .expect("two clock times are a span apart"),
        ),
    };
    let luminosity = match age {
        None => event.light.peaks(),
        Some(age) => event.light.luminosities(age),
    };
    let reaches = query.sensor().bands().any(|(band, limits)| {
        inverse_square_flux(luminosity[band.index()], distance)
            .is_some_and(|bare| bare >= limits.detect())
    });
    reaches.then_some(Candidate {
        event: *event,
        arrival,
        phase,
        distance,
        luminosity,
    })
}

/// The detection of `candidate`, of `class` and `kind`, by `query`'s sensor, if its flux through
/// its line of sight reaches the detection limit in one of the sensor's bands: in the band where it
/// stands highest over the limit, the first on a tie.
#[must_use]
pub(super) fn detect(
    galaxy: &Galaxy,
    ctx: &mut AlertContext<'_>,
    query: &AlertQuery,
    class: ClassId,
    kind: TransientKind,
    candidate: &Candidate,
) -> Option<Detection> {
    let observer = query.observer_position();
    let event = &candidate.event;
    let site = event.site();
    let line = ctx.sightline(galaxy, &site, observer);
    let mut best: Option<(AlertBand, WattsPerSquareMetre, f64)> = None;
    for (band, limits) in query.sensor().bands() {
        let Some(seen) = flux(
            candidate.luminosity[band.index()],
            candidate.distance,
            &line,
            band,
        ) else {
            continue;
        };
        let over = seen.value() / limits.detect().value();
        if over >= 1.0 && best.is_none_or(|(_, _, most)| over > most) {
            best = Some((band, seen, over));
        }
    }
    let (band, seen, _) = best?;
    let limits = query
        .sensor()
        .limits(band)
        .expect("the band is one of the sensor's");
    let present = event.motion.position_at(candidate.arrival);
    let retardation = Retardation::of_emission(
        observer,
        event.emitted,
        site,
        event.motion.velocity_at(event.emitted),
        &present,
    );
    debug_assert_eq!(retardation.observed(), candidate.arrival);
    let through = transmission(&line, band);
    let light = match event.light {
        OneShotLight::Supernova(kind) => {
            ArrivingLight::supernova(kind, event.emitted, band, candidate.distance, through)
        }
        OneShotLight::Binary(curve) => {
            ArrivingLight::binary(curve, event.emitted, band, candidate.distance, through)
        }
    };
    debug_assert_eq!(
        event.emitted.checked_add(light_time(candidate.distance)),
        Some(candidate.arrival)
    );
    Some(Detection::new(
        event.event,
        event.host,
        Some(class),
        kind,
        candidate.phase,
        band,
        seen,
        bearing(observer, &site)?,
        retardation,
        seen >= limits.resolve(),
        DetectionLight::Lit(light),
    ))
}
