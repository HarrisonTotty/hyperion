//! What one of a subscription's scans walks, on a pool worker (plan 12, P12.T8.b; Design notes 7
//! and 8).
//!
//! A [`ScanRequest`] is walked by the sim's `alerts_in_with`: each class's census and detections,
//! and the local events of the systems an observed range query finds within
//! [`LOCAL_EVENTS_RADIUS`] of the observer (P12.T5.d, P12.T5.e). [`walk_interactive`] walks what
//! the census lets it walk at once, one interactive job. A class the census sends to the background
//! is walked by [`walk_cells`] over the cells [`background_cells`] plans, in bulk jobs of
//! [`BACKGROUND_CELLS_PER_JOB`] cells each (the service's).
//!
//! The first scan after a schedule starts or restarts walks each class back from its window's start
//! by the class's longest rise to peak ([`class_longest_rise`]; research's lean of 2026-10-30), and
//! the local events by the local kinds' longest, 25 years for an FU Orionis outburst: a riser whose
//! onset arrived before the window can cross a limit in it. Of what a walk finds, only what is lit
//! at the window's start or crosses a limit from then on is kept ([`ScanResult`]).
//!
//! Each detection is tracked on its own light ([`Tracked::of`], P12.T8.b.1), through the gas of the
//! features at the epoch while they are served: so a host's light is the same whichever scan lists
//! it, as the schedule asks (lean, 2026-11-01). The centre is not walked: the server holds no
//! universe's global list, so its census says so (`Unwalked::NoGlobalList`; lean, 2026-11-01).

use std::mem::size_of;

use hyperion_sim::alerts::{
    AlertBand, AlertContext, AlertQuery, BuildAlertQueryError, ClassCensus, ClassSet, ClassStatus,
    Detection, LocalEventsCache, LongestRise, ProductionAlertClasses, Route, TransientKind,
    TransientRow, alerts_in_with, class_longest_rise, class_reach, magnetar_scan_cell,
    magnetar_scan_plan, one_shot_scan_cell, one_shot_scan_plan,
};
use hyperion_sim::events::TimeWindow;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::catalogue_classes::core_collapse::CoreCollapseProcess;
use hyperion_sim::galaxy::catalogue_classes::type_ia::TypeIaProcess;
use hyperion_sim::galaxy::catalogue_classes::{CatalogueCellKey, ClassId, cells_in_sphere};
use hyperion_sim::galaxy::features::catalogue::FeatureCellCache;
use hyperion_sim::galaxy::features::gas_overlay::FeatureGas;
use hyperion_sim::galaxy::gas::modifiers::{GasModifierSource, NoModifiers};
use hyperion_sim::galaxy::gas::noise::NoiseCache;
use hyperion_sim::galaxy::query::{QueryMode, QuerySphere, RangeQuery, range_query_observed};
use hyperion_sim::time::{ClockWindow, Span, UniverseTime};
use hyperion_sim::units::{LightYears, Years};

use super::{Crossing, ScanRequest, Subscription, Tracked};
use crate::cache::HeapBytes;
use crate::compute::{CancelToken, GalaxyKey};
use crate::{AppState, FeatureMembers};

/// How far about the observer a scan lists the local events of the systems a range query finds:
/// 10 ly, some fifteen systems at the Sun's density (lean, 2026-11-01). A flare of a star further
/// out reaches few sensors' limits, and a wider query reads every one of its stars at each scan.
pub(crate) const LOCAL_EVENTS_RADIUS: LightYears = LightYears::new(10.0);

/// Catalogue cells per background job: 256, the few hundred Design note 7 has, some 0.6 s of
/// core-collapse cells at their 2.3 ms each by the solar circle (P12.T5.b's record).
pub(crate) const BACKGROUND_CELLS_PER_JOB: usize = 256;

/// The most cells a class's background walk reads: 2²⁰, ten times the census's interactive bound
/// (`BACKGROUND_THRESHOLD_CELLS`), a 512 ly class's cells within some 32,000 ly, some 40 minutes of
/// a worker's time at 2.3 ms a cell (lean, 2026-11-01). The sim's plans list every cell of the
/// reach's sphere, a sensor that reaches across the root cube some 1.3 × 10⁸ of them, 2 GB of keys
/// and days of walking, so a class whose sphere holds more is not walked and is over the limit.
pub(crate) const MAX_BACKGROUND_CELLS: usize = 1 << 20;

/// The stars a subscription's local events cache keeps: every star of a 10 ly query's few dozen,
/// many times over, at a few dozen bytes each.
pub(crate) const LOCAL_EVENTS_STARS: usize = 1_024;

/// The entries of each walk's noise cache: the sim's own examples' size for a walk's lines.
const NOISE_ENTRIES: usize = 4_096;

/// One scan of a subscription: its request, the sim's query for it, and the galaxy it walks.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ScanSpec {
    galaxy: GalaxyKey,
    request: ScanRequest,
    query: AlertQuery,
}

impl ScanSpec {
    /// The scan `request` of `subscription` asks for, in the galaxy of `galaxy`.
    ///
    /// # Errors
    ///
    /// As [`Subscription::query`], which a request its schedule made never trips.
    pub(crate) fn new(
        galaxy: GalaxyKey,
        subscription: &Subscription,
        request: &ScanRequest,
    ) -> Result<Self, BuildAlertQueryError> {
        Ok(Self {
            galaxy,
            request: *request,
            query: subscription.query(request)?,
        })
    }

    /// The request the scan answers.
    #[must_use]
    pub(crate) const fn request(&self) -> &ScanRequest {
        &self.request
    }

    /// The scan's key in the service's cache.
    #[must_use]
    pub(crate) fn key(&self) -> ScanKey {
        let position = self.query.observer_position();
        let window = self.query.window();
        ScanKey {
            galaxy: self.galaxy,
            classes: self.query.classes(),
            host_limit: self.query.host_limit().get(),
            sensor: self
                .query
                .sensor()
                .bands()
                .map(|(band, limits)| {
                    let [detect, resolve] =
                        [limits.detect(), limits.resolve()].map(|flux| flux.value().to_bits());
                    (band, detect, resolve)
                })
                .collect(),
            position: (
                position.cell().to_array(),
                position.offset_metres().map(f64::to_bits),
            ),
            window: (window.start(), window.end()),
            first: self.request.starts_schedule(),
        }
    }

    /// The query of `class` alone, its window reaching back by the class's longest rise for a
    /// first scan.
    #[must_use]
    pub(crate) fn class_query(&self, class: ClassId) -> AlertQuery {
        self.walked(ClassSet::of(class), class_longest_rise(class))
    }

    /// The query of no class, for the local events, its window reaching back by the local kinds'
    /// longest rise for a first scan.
    #[must_use]
    fn local_query(&self) -> AlertQuery {
        let longest = TransientKind::ALL
            .into_iter()
            .map(TransientKind::row)
            .filter(|row| row.route() == Route::Local)
            .filter_map(TransientRow::longest_rise)
            .reduce(|a, b| match (a, b) {
                (LongestRise::AtMost(a), LongestRise::AtMost(b)) => LongestRise::AtMost(a.max(b)),
                (LongestRise::PerEvent, _) | (_, LongestRise::PerEvent) => LongestRise::PerEvent,
            });
        self.walked(ClassSet::default(), longest)
    }

    /// The query of `classes`, over the request's window, reaching back for a first scan by
    /// `rise`'s bound, held inside the clock window. A kind whose rise is each event's own walks
    /// its own look-back, and so back by nothing more.
    #[must_use]
    fn walked(&self, classes: ClassSet, rise: Option<LongestRise>) -> AlertQuery {
        let window = self.query.window();
        let back = match rise {
            Some(LongestRise::AtMost(span)) if self.request.starts_schedule() => span,
            Some(LongestRise::AtMost(_) | LongestRise::PerEvent) | None => Span::ZERO,
        };
        let start = window
            .start()
            .checked_sub(back)
            .map_or(ClockWindow::START, |start| start.max(ClockWindow::START));
        let walked = TimeWindow::new(start, window.end())
            .expect("a window reaching back from its start still ends after it");
        AlertQuery::builder(
            *self.query.observer_position(),
            walked,
            self.query.sensor().clone(),
        )
        .classes(classes)
        .host_limit(self.query.host_limit())
        .build()
        .expect("the request's own place, and a window held inside the clock window")
    }

    /// What of `detections` bears on the request's window, tracked on their lights.
    #[must_use]
    fn tracked(&self, detections: Vec<Detection>) -> Vec<Tracked> {
        let start = self.query.window().start();
        let sensor = self.query.sensor();
        detections
            .into_iter()
            .filter_map(|detection| Tracked::of(detection, sensor))
            .filter(|track| bears_on(track, start))
            .collect()
    }
}

/// A scan's key in the service's cache (plan 12's Provides): the galaxy, the classes and their
/// host limit, the sensor's limits, the observer's place, the window, and whether the scan starts
/// its schedule. The limits and the place are held by their bits, as the sightline cache holds its
/// ends: a request's own floats, never compared as numbers. The galaxy stands for Design note 8's
/// universe: a walk reads nothing of a universe but its galaxy, so two universes of one seed share
/// their walks (lean, 2026-11-01).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct ScanKey {
    galaxy: GalaxyKey,
    classes: ClassSet,
    host_limit: u32,
    sensor: Vec<(AlertBand, u64, u64)>,
    position: ([i32; 3], [u64; 3]),
    window: (UniverseTime, UniverseTime),
    first: bool,
}

/// What a walk found: the tracks of what bears on its scan's window, and for an interactive walk
/// each class's census, in class order.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ScanResult {
    tracks: Vec<Tracked>,
    census: Vec<ClassCensus>,
    /// Whether a background walk was refused for its cells, more than [`MAX_BACKGROUND_CELLS`].
    oversized: bool,
}

impl ScanResult {
    /// A background walk's, of every chunk's tracks in the plan's order: no census.
    #[must_use]
    pub(crate) fn of_chunks(chunks: impl IntoIterator<Item = Vec<Tracked>>) -> Self {
        Self {
            tracks: chunks.into_iter().flatten().collect(),
            census: Vec::new(),
            oversized: false,
        }
    }

    /// A background walk refused for its cells: nothing.
    #[must_use]
    pub(crate) const fn oversized() -> Self {
        Self {
            tracks: Vec::new(),
            census: Vec::new(),
            oversized: true,
        }
    }

    /// Whether the walk was a background one refused for its cells, more than
    /// [`MAX_BACKGROUND_CELLS`].
    #[must_use]
    pub(crate) const fn is_oversized(&self) -> bool {
        self.oversized
    }

    /// The tracks: an interactive walk's each class's in class order and then the local events',
    /// and a background walk's in its plan's order.
    #[must_use]
    pub(crate) fn tracks(&self) -> &[Tracked] {
        &self.tracks
    }

    /// Each class's census, in class order.
    #[must_use]
    pub(crate) fn census(&self) -> &[ClassCensus] {
        &self.census
    }

    /// The classes the census sends to the background, in class order.
    pub(crate) fn background(&self) -> impl Iterator<Item = ClassId> + '_ {
        self.census
            .iter()
            .filter(|census| census.status() == ClassStatus::NeedsBackgroundScan)
            .map(ClassCensus::class)
    }
}

impl HeapBytes for ScanResult {
    fn heap_bytes(&self) -> usize {
        tracks_bytes(&self.tracks) + self.census.capacity() * size_of::<ClassCensus>()
    }
}

/// The bytes a list of tracks owns: each track, and its list of crossings.
#[must_use]
fn tracks_bytes(tracks: &[Tracked]) -> usize {
    tracks
        .iter()
        .map(|track| size_of::<Tracked>() + size_of_val(track.crossings()))
        .sum()
}

/// Whether `track` bears on a window from `start`: it is lit then, or crosses a limit from then
/// on. A walk back finds others, which the schedule would not push.
#[must_use]
fn bears_on(track: &Tracked, start: UniverseTime) -> bool {
    let mut lit = false;
    for &(at, crossing, _) in track.crossings() {
        if at >= start {
            return true;
        }
        lit = crossing != Crossing::Clear;
    }
    lit
}

/// The gas a walk's lines of sight are marched through: plan 09's features at the epoch once the
/// feature members are served, or the smooth field alone while they are held, as the range query's
/// rows are.
enum WalkGas<'a> {
    Smooth(NoModifiers),
    Features(FeatureGas<'a>),
}

impl<'a> WalkGas<'a> {
    /// The gas `state` serves in `galaxy`, its features looked up through `cells`.
    #[must_use]
    fn of(state: &AppState, galaxy: &'a Galaxy, cells: &'a dyn FeatureCellCache) -> Self {
        match state.feature_members {
            FeatureMembers::Held => Self::Smooth(NoModifiers),
            FeatureMembers::Served => Self::Features(FeatureGas::new(galaxy, cells, Years::ZERO)),
        }
    }

    /// The source a walk's context takes.
    #[must_use]
    fn source(&self) -> &dyn GasModifierSource {
        match self {
            Self::Smooth(smooth) => smooth,
            Self::Features(features) => features,
        }
    }
}

/// Walks `spec`'s classes and local events in `galaxy` as far as each class's census lets it, on
/// a pool worker, through `state`'s caches and the subscription's `local` cache (module
/// documentation).
///
/// The local events are those of the systems within [`LOCAL_EVENTS_RADIUS`] of the observer at
/// the window's start. A range query that finds a member of the galactic centre is refused until
/// plan 09's P09.T28 builds its orbit, and the scan then lists no local event.
#[must_use]
pub(crate) fn walk_interactive(
    state: &AppState,
    galaxy: &Galaxy,
    spec: &ScanSpec,
    local: &mut LocalEventsCache,
) -> ScanResult {
    let key = spec.galaxy;
    let mut cells = state.cells.handle(key);
    let mut stars = state.systems.handle(key);
    let feature_cells = state.feature_cells.handle(key);
    let gas = WalkGas::of(state, galaxy, &feature_cells);
    let mut noise = NoiseCache::with_capacity(NOISE_ENTRIES);
    let mut ctx = AlertContext::new(&mut noise, gas.source());
    let mut detections = Vec::new();
    let mut census = Vec::new();
    for class in spec.query.classes().iter() {
        let query = spec.class_query(class);
        let result = alerts_in_with(
            galaxy,
            &mut ctx,
            &query,
            &ProductionAlertClasses,
            &[],
            &mut stars,
            local,
        );
        census.extend_from_slice(result.census());
        detections.extend_from_slice(result.detections());
    }
    let position = *spec.query.observer_position();
    let around = RangeQuery::builder(position, LOCAL_EVENTS_RADIUS)
        .time(spec.query.window().start())
        .mode(QueryMode::ObservedFrom(position))
        .build()
        .expect("an alert query's observer is in the root cube and its window on the clock");
    // The centre's members are refused whole (above): no local event is listed then.
    if let Ok(found) = range_query_observed(galaxy, &mut cells, &mut stars, &[], &around) {
        let result = alerts_in_with(
            galaxy,
            &mut ctx,
            &spec.local_query(),
            &ProductionAlertClasses,
            found.observed(),
            &mut stars,
            local,
        );
        detections.extend_from_slice(result.detections());
    }
    census.sort_by_key(ClassCensus::class);
    ScanResult {
        tracks: spec.tracked(detections),
        census,
        oversized: false,
    }
}

/// One catalogue cell of a background walk, and the walk that reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum BackgroundCell {
    /// A cell of the class's one-shot events, its supernovae (`one_shot_scan_cell`).
    OneShot(CatalogueCellKey),
    /// A cell of the class's magnetars (`magnetar_scan_cell`).
    Magnetars(CatalogueCellKey),
}

/// The cells of `class` that a background walk of `spec` reads, in the plan's order: the class's
/// supernovae's cells, then its magnetars'; `None` if the class's cells within its reach are more
/// than `most`, which are not planned. The core collapses and the Type Ia are the classes walked
/// in cells (P12.T5.d's `ProductionAlertClasses`); the others have none.
///
/// Each plan's sphere is one of the class's kinds' reach, within the class's: so each holds at
/// most the class sphere's cells, which are counted first, up to `most` and one.
#[must_use]
pub(crate) fn background_cells(
    galaxy: &Galaxy,
    spec: &ScanSpec,
    class: ClassId,
    most: usize,
) -> Option<Vec<BackgroundCell>> {
    let query = spec.class_query(class);
    let reach = class_reach(class, query.sensor());
    if reach > LightYears::ZERO {
        let sphere = QuerySphere::new(
            *query.observer_position(),
            reach,
            query.window().start(),
            LightYears::ZERO,
        )
        .expect("a reach is a positive length within the cube's diagonal");
        if cells_in_sphere(class, &sphere).take(most + 1).count() > most {
            return None;
        }
    }
    let one_shot = match class {
        ClassId::CORE_COLLAPSE => one_shot_scan_plan(&CoreCollapseProcess::new(galaxy), &query),
        ClassId::TYPE_IA => one_shot_scan_plan(&TypeIaProcess::new(galaxy), &query),
        _ => Vec::new(),
    };
    let magnetars = if class == ClassId::CORE_COLLAPSE {
        magnetar_scan_plan(class, &query)
    } else {
        Vec::new()
    };
    Some(
        one_shot
            .into_iter()
            .map(BackgroundCell::OneShot)
            .chain(magnetars.into_iter().map(BackgroundCell::Magnetars))
            .collect(),
    )
}

/// Walks `cells` of `class` for `spec` in `galaxy`, on a pool worker: the tracks of what bears on
/// the window, in the cells' order, or `None` if `token` was cancelled before the last cell.
///
/// Over every cell of [`background_cells`], in chunks of any size, the tracks are the class's own
/// in [`walk_interactive`], each to the bit: each cell's walk is a pure function of the cell
/// (P12.T5.d), and each event is one cell's.
#[must_use]
pub(crate) fn walk_cells(
    state: &AppState,
    galaxy: &Galaxy,
    spec: &ScanSpec,
    class: ClassId,
    cells: &[BackgroundCell],
    token: &CancelToken,
) -> Option<Vec<Tracked>> {
    let feature_cells = state.feature_cells.handle(spec.galaxy);
    let gas = WalkGas::of(state, galaxy, &feature_cells);
    let mut noise = NoiseCache::with_capacity(NOISE_ENTRIES);
    let mut ctx = AlertContext::new(&mut noise, gas.source());
    let query = spec.class_query(class);
    let core_collapses =
        (class == ClassId::CORE_COLLAPSE).then(|| CoreCollapseProcess::new(galaxy));
    let type_ia = (class == ClassId::TYPE_IA).then(|| TypeIaProcess::new(galaxy));
    let mut detections = Vec::new();
    for &cell in cells {
        if token.is_cancelled() {
            return None;
        }
        match (cell, &core_collapses, &type_ia) {
            (BackgroundCell::OneShot(cell), Some(process), _) => {
                one_shot_scan_cell(galaxy, &mut ctx, &query, process, cell, &mut detections);
            }
            (BackgroundCell::OneShot(cell), None, Some(process)) => {
                one_shot_scan_cell(galaxy, &mut ctx, &query, process, cell, &mut detections);
            }
            (BackgroundCell::Magnetars(cell), Some(process), _) => {
                magnetar_scan_cell(galaxy, &mut ctx, &query, process, cell, &mut detections);
            }
            // A cell no plan of the class holds: `background_cells` gives none.
            (BackgroundCell::OneShot(_), None, None) | (BackgroundCell::Magnetars(_), None, _) => {}
        }
    }
    Some(spec.tracked(detections))
}

/// Splits `cells` into the chunks of a background walk's jobs, `per_job` cells each and at least
/// one, the last the rest.
#[must_use]
pub(crate) fn chunks(cells: &[BackgroundCell], per_job: usize) -> Vec<Vec<BackgroundCell>> {
    cells.chunks(per_job.max(1)).map(<[_]>::to_vec).collect()
}

#[cfg(test)]
mod tests;
