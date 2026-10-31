//! The one-shot classes' walk (plan 12, P12.T5.b): the supernovae a sensor sees go off in its
//! window, each the explosion of one catalogue entry.
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
//!    smallest `f64`), so an entry the sensor can see has its light arriving after −H − 228 years,
//!    and with at most the cube's diagonal to cross it exploded after −(H + L), when the rule holds
//!    it. Design note 5's widening by `UNBOUND_PAD_SPEED` ÷ c is for hosts walked on their present
//!    positions; the walk reads each entry's site at T.
//! 3. Each entry's arrival. One that arrives in the window is an [`Onset`], seen at its peak in
//!    each band ([`supernova_peak`]); one that arrived before `t₁` is [`InProgress`] if its light
//!    at `t₁`, `t₁` − `t_a` after its onset reached the observer
//!    ([`supernova_luminosity`](super::supernova_luminosity)), is still above the limit. One that
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
//! interactively at all is the census's (P12.T5.d, Design note 7). A class's cells cost
//! milliseconds each (P09.T33.a), so the caller lends the walk a cache of them
//! ([`OneShotCells`], [`AlertContext::with_one_shot_cells`]).
//!
//! # What a detection reports
//!
//! The entry's explosion event ([`SupernovaEntry::explosion_event`], the same from any distance),
//! its member 0 as the host, its class and kind, T and `t_a`; the band and flux above; the bearing
//! of its site from the observer; the onset's light ([`Retardation`]: observed at `t_a`, emitted
//! at T from the site, the host then moving at its velocity from T, its remnant's); whether the
//! flux reaches the band's resolve limit; and the template's light in the band at every time
//! ([`DetectionLight`], P12.T8.b.1), through the same line of sight from the site.
//!
//! [`Onset`]: DetectionPhase::Onset
//! [`InProgress`]: DetectionPhase::InProgress
//! [`Retardation`]: crate::observe::Retardation

use crate::coords::GalacticPosition;
use crate::galaxy::Galaxy;
use crate::galaxy::catalogue_classes::core_collapse::{CoreCollapseEntry, CoreCollapseProcess};
use crate::galaxy::catalogue_classes::supernova::{SupernovaEntry, SupernovaKind};
use crate::galaxy::catalogue_classes::{
    CatalogueCellKey, CatalogueClassCell, CatalogueMotion, ClassProcess, cells_in_sphere,
};
use crate::galaxy::query::QuerySphere;
use crate::id::{EventId, SystemId};
use crate::observe::{Retardation, bearing, light_time};
use crate::time::UniverseTime;
use crate::units::{LightYears, Metres, Watts, WattsPerSquareMetre, Years};

use super::arriving::{ArrivingLight, DetectionLight};
use super::light::{supernova_luminosities, supernova_peak};
use super::reach::{flux, inverse_square_flux, kind_reach, transmission};
use super::registry::TransientKind;
use super::{AlertBand, AlertContext, AlertQuery, BAND_COUNT, Detection, DetectionPhase};

/// One entry's one-shot event, as the walk reads it: the event, its host, what exploded, when, and
/// how the host moves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OneShotEvent {
    event: EventId,
    host: SystemId,
    supernova: SupernovaKind,
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
            supernova: entry.kind(),
            emitted: entry.explosion(),
            motion,
        }
    }

    /// The event: the explosion's ID.
    #[must_use]
    pub const fn event(&self) -> EventId {
        self.event
    }

    /// The host: the entry's member 0.
    #[must_use]
    pub const fn host(&self) -> SystemId {
        self.host
    }

    /// What exploded.
    #[must_use]
    pub const fn supernova(&self) -> SupernovaKind {
        self.supernova
    }

    /// When it exploded, T.
    #[must_use]
    pub const fn emitted(&self) -> UniverseTime {
        self.emitted
    }

    /// Where it exploded: the host's position at T.
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
/// Implemented for plan 09's core collapses ([`CoreCollapseProcess`]); P09.T34.a's Type Ia process
/// implements it the same way, from its entries' [`SupernovaEntry`] and member 0's motion, and
/// plan 11's two merger classes from their own events (P11.T9).
pub trait OneShotClass: ClassProcess {
    /// The kind of event the class's entries make.
    #[must_use]
    fn kind(&self) -> TransientKind;

    /// The event `entry` makes.
    #[must_use]
    fn event(&self, entry: &Self::Entry) -> OneShotEvent;
}

impl OneShotClass for CoreCollapseProcess {
    fn kind(&self) -> TransientKind {
        TransientKind::CoreCollapse
    }

    fn event(&self, entry: &CoreCollapseEntry) -> OneShotEvent {
        OneShotEvent::of_supernova(entry.supernova(), *entry.motion())
    }
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

/// The events of `process`'s cell `key` in `galaxy`, in index order.
#[must_use]
fn events_of<P: OneShotClass>(
    galaxy: &Galaxy,
    process: &P,
    key: CatalogueCellKey,
) -> Vec<OneShotEvent> {
    CatalogueClassCell::generate(galaxy, process, key)
        .entries()
        .iter()
        .map(|entry| process.event(entry.value()))
        .collect()
}

/// An event the walk keeps for its line of sight: its arrival and phase, and its luminosity in
/// each band when the phase reads it.
#[derive(Debug, Clone, Copy)]
struct Candidate {
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
    if !query.classes().contains(class) {
        return;
    }
    let kind = process.kind();
    let reach = kind_reach(kind, query.sensor());
    if reach <= LightYears::ZERO {
        return;
    }
    let observer = *query.observer_position();
    let window = query.window();
    let sphere = QuerySphere::new(observer, reach, window.start(), LightYears::ZERO)
        .expect("a reach is a positive length within the cube's diagonal");
    let mut candidates = Vec::new();
    for key in cells_in_sphere(class, &sphere) {
        ctx.one_shot_events(
            key,
            &mut || events_of(galaxy, process, key),
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
        if let Some(detection) = detect(galaxy, ctx, query, process, kind, &candidate) {
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
fn candidate(
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
        DetectionPhase::InProgress => Some(Years::new(
            window
                .start()
                .checked_since(arrival)
                .expect("two clock times are a span apart")
                .as_julian_years_f64(),
        )),
    };
    let luminosity = match age {
        None => AlertBand::ALL.map(|band| supernova_peak(event.supernova, band)),
        Some(age) => supernova_luminosities(event.supernova, age),
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

/// The detection of `candidate` by `query`'s sensor, if its flux through its line of sight reaches
/// the detection limit in one of the sensor's bands: in the band where it stands highest over the
/// limit, the first on a tie.
#[must_use]
fn detect<P: OneShotClass>(
    galaxy: &Galaxy,
    ctx: &mut AlertContext<'_>,
    query: &AlertQuery,
    process: &P,
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
    let light = ArrivingLight::supernova(
        event.supernova,
        event.emitted,
        band,
        candidate.distance,
        transmission(&line, band),
    );
    debug_assert_eq!(
        event.emitted.checked_add(light_time(candidate.distance)),
        Some(candidate.arrival)
    );
    Some(Detection::new(
        event.event,
        event.host,
        Some(process.class()),
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
