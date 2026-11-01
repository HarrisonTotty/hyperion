//! What one of a subscription's scans walks, on a pool worker (plan 12, P12.T8.b; Design notes 7
//! and 8).
//!
//! A [`ScanRequest`] is walked by the sim's `alerts_in_with`: each class's census and detections,
//! and the local events of the systems an observed range query finds within
//! [`LOCAL_EVENTS_RADIUS`] of the observer (P12.T5.d, P12.T5.e). [`walk_interactive`] walks what
//! the census lets it walk at once, one interactive job. A class the census sends to the background
//! is planned by [`background_plan`] and walked by [`walk_slabs`] in bulk jobs of a few x-slabs of
//! its sphere each (the service's): each job sweeps its slabs' occupied cells and builds only the
//! candidates whose event time, read first, can light the window (P12.T8.b.2), so that a sensor
//! that sees across the galaxy is walked in seconds, its class `complete` or over its host limit.
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
use std::ops::Range;
use std::sync::Arc;

use hyperion_sim::alerts::{
    AlertBand, AlertContext, AlertQuery, BuildAlertQueryError, ClassCensus, ClassSet, ClassStatus,
    Detection, LocalEventsCache, LongestRise, ProductionAlertClasses, Route, TransientKind,
    TransientRow, alerts_in_with, class_longest_rise, class_reach, kind_reach, magnetar_scan_cell,
    one_shot_scan_cell,
};
use hyperion_sim::events::TimeWindow;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::catalogue_classes::binary::{NeutronStarMergers, StellarMergers};
use hyperion_sim::galaxy::catalogue_classes::core_collapse::CoreCollapseProcess;
use hyperion_sim::galaxy::catalogue_classes::supernova::LIFETIME_REACH;
use hyperion_sim::galaxy::catalogue_classes::type_ia::TypeIaProcess;
use hyperion_sim::galaxy::catalogue_classes::{
    CatalogueCellKey, ClassId, cells_in_slab, class_slabs, occupied_cells,
};
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

/// The x-slabs of a background class's sphere each job sweeps: 4, so 64 jobs over the root cube's
/// 256 slabs of 512 ly cells, each some 0.1 s of bounds and counts at 24 ms a slab
/// (research-alerts-galaxy-scan, pod, 16 vCPU), long beside a job's round trip.
pub(crate) const SLABS_PER_JOB: usize = 4;

/// The most jobs a background walk is split into: 256 (the brief for P12.T8.b.2), more slabs to a
/// job past it.
pub(crate) const MAX_BACKGROUND_JOBS: usize = 256;

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

/// What a walk found: the tracks of what bears on its scan's window, for an interactive walk each
/// class's census, in class order, and for a background walk the candidates its cells drew.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ScanResult {
    tracks: Vec<Tracked>,
    census: Vec<ClassCensus>,
    /// The candidates a background walk's cells drew, or `None` for an interactive walk or a
    /// class no background walk reads.
    drawn: Option<u64>,
}

impl ScanResult {
    /// A background walk's, of every job's in the plan's order: no census.
    #[must_use]
    pub(crate) fn of_jobs(jobs: impl IntoIterator<Item = SlabsWalked>) -> Self {
        let mut tracks = Vec::new();
        let mut drawn: u64 = 0;
        for job in jobs {
            tracks.extend(job.tracks);
            // Each count is under 2^24 a cell, of under 2^24 cells.
            drawn += job.drawn;
        }
        Self {
            tracks,
            census: Vec::new(),
            drawn: Some(drawn),
        }
    }

    /// The background walk of a class that no background walk reads in cells: nothing.
    #[must_use]
    pub(crate) const fn unwalked() -> Self {
        Self {
            tracks: Vec::new(),
            census: Vec::new(),
            drawn: None,
        }
    }

    /// The candidates a background walk's cells drew within its class's reach, its census's
    /// count of hosts (lean, 2026-11-01): `None` for an interactive walk, or a class no
    /// background walk reads.
    #[must_use]
    pub(crate) const fn drawn(&self) -> Option<u64> {
        self.drawn
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
        drawn: None,
    }
}

/// A class's process as a background walk builds it once and lends it to every job (P12.T8.b.2):
/// the classes `ProductionAlertClasses` walks in cells, the core collapses with their magnetars.
#[derive(Debug)]
pub(crate) enum BackgroundProcess {
    CoreCollapse(CoreCollapseProcess),
    TypeIa(TypeIaProcess),
    StellarMergers(StellarMergers),
    NeutronStarMergers(NeutronStarMergers),
}

impl BackgroundProcess {
    /// `class`'s process in `galaxy`, or `None` for a class no background walk reads in cells.
    #[must_use]
    fn of(galaxy: &Galaxy, class: ClassId) -> Option<Self> {
        match class {
            ClassId::CORE_COLLAPSE => Some(Self::CoreCollapse(CoreCollapseProcess::new(galaxy))),
            // Its process costs some 37 ms (P12.T5.d), built once a walk, not once a job.
            ClassId::TYPE_IA => Some(Self::TypeIa(TypeIaProcess::new(galaxy))),
            ClassId::STELLAR_MERGER => Some(Self::StellarMergers(StellarMergers::new(galaxy))),
            ClassId::NEUTRON_STAR_MERGER => {
                Some(Self::NeutronStarMergers(NeutronStarMergers::new(galaxy)))
            }
            _ => None,
        }
    }
}

/// A background walk of one class, planned: its process, the sphere of its reach about the
/// observer, and its x-slabs in the jobs that sweep them (P12.T8.b.2). It holds no cell keys: each
/// job lists its slabs' cells as it sweeps them ([`walk_slabs`]).
#[derive(Debug, Clone)]
pub(crate) struct BackgroundPlan {
    process: Arc<BackgroundProcess>,
    /// `None` for a class the sensor cannot see, which has no job.
    sphere: Option<QuerySphere>,
    jobs: Vec<Range<i32>>,
}

impl BackgroundPlan {
    /// The slabs of each job, in x order.
    #[must_use]
    pub(crate) fn jobs(&self) -> &[Range<i32>] {
        &self.jobs
    }
}

/// The plan of `class`'s background walk for `spec` in `galaxy`, its slabs `slabs_per_job` to a
/// job, more where they would make over [`MAX_BACKGROUND_JOBS`] jobs; `None` for a class no
/// background walk reads in cells. A class the sensor cannot see has no slabs and no job.
///
/// The sphere is the class's reach ([`class_reach`]), within which every kind of the class has its
/// own; the plan sweeps it slab by slab, so no list of its cells is held, 2²⁴ of them over the cube.
#[must_use]
pub(crate) fn background_plan(
    galaxy: &Galaxy,
    spec: &ScanSpec,
    class: ClassId,
    slabs_per_job: usize,
) -> Option<BackgroundPlan> {
    let process = Arc::new(BackgroundProcess::of(galaxy, class)?);
    let query = spec.class_query(class);
    let reach = class_reach(class, query.sensor());
    let observer = *query.observer_position();
    let start = query.window().start();
    // A class the sensor cannot see has no sphere: no slab and no job, nothing drawn.
    let Ok(sphere) = QuerySphere::new(observer, reach, start, LightYears::ZERO) else {
        return Some(BackgroundPlan {
            process,
            sphere: None,
            jobs: Vec::new(),
        });
    };
    let slabs = class_slabs(class, &sphere);
    let count = slabs.len();
    // No more slabs a job than the sphere has, which are some hundreds.
    let per_job = slabs_per_job
        .max(count.div_ceil(MAX_BACKGROUND_JOBS))
        .clamp(1, count.max(1));
    let step = i32::try_from(per_job).expect("a sphere's slabs in the root cube fit in i32");
    let jobs = slabs
        .clone()
        .step_by(per_job)
        .map(|first| first..first.saturating_add(step).min(slabs.end))
        .collect();
    Some(BackgroundPlan {
        process,
        sphere: Some(sphere),
        jobs,
    })
}

/// What a background job found: the tracks of what bears on the window, and the candidates its
/// slabs' cells drew.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct SlabsWalked {
    tracks: Vec<Tracked>,
    drawn: u64,
}

/// Whether `kind`'s events of `key`'s cell can reach `query`'s sensor: its box within the kind's
/// reach and [`LIFETIME_REACH`] of the observer, and a light-year more for the rounding. A cell
/// beyond adds nothing to its kind's walk, so walking a few more changes no detection.
#[must_use]
fn reaches(query: &AlertQuery, kind: TransientKind, key: &CatalogueCellKey) -> bool {
    let reach = kind_reach(kind, query.sensor());
    if reach <= LightYears::ZERO {
        return false;
    }
    let centre = query.observer_position().to_light_years_f64();
    let edge = f64::from(key.edge_ly());
    let origin = key.origin_ly();
    let gap_squared = (0..3).fold(0.0, |sum, axis| {
        let low = f64::from(origin[axis]);
        let gap = (low - centre[axis]).max(centre[axis] - low - edge).max(0.0);
        sum + gap * gap
    });
    let padded = reach.value() + LIFETIME_REACH.value() + 1.0;
    gap_squared <= padded * padded
}

/// Walks the slabs `slabs` of `plan` for `spec` in `galaxy`, on a pool worker: every occupied cell
/// of each slab ([`occupied_cells`]), its one-shot events and, for the core collapses, its
/// magnetars, each kind where it can reach the sensor, cut to the candidates whose time read first
/// can light the window (`one_shot_scan_cell`, `magnetar_scan_cell`). `None` if `token` was
/// cancelled before the last cell.
///
/// Over every job of the plan, in any order, the tracks are the class's own in
/// [`walk_interactive`], each to the bit: each cell's walk is a pure function of the cell.
#[must_use]
pub(crate) fn walk_slabs(
    state: &AppState,
    galaxy: &Galaxy,
    spec: &ScanSpec,
    plan: &BackgroundPlan,
    slabs: Range<i32>,
    token: &CancelToken,
) -> Option<SlabsWalked> {
    let feature_cells = state.feature_cells.handle(spec.galaxy);
    let gas = WalkGas::of(state, galaxy, &feature_cells);
    let mut noise = NoiseCache::with_capacity(NOISE_ENTRIES);
    let mut ctx = AlertContext::new(&mut noise, gas.source());
    let mut detections = Vec::new();
    let mut drawn: u64 = 0;
    let process = plan.process.as_ref();
    let class = match process {
        BackgroundProcess::CoreCollapse(_) => ClassId::CORE_COLLAPSE,
        BackgroundProcess::TypeIa(_) => ClassId::TYPE_IA,
        BackgroundProcess::StellarMergers(_) => ClassId::STELLAR_MERGER,
        BackgroundProcess::NeutronStarMergers(_) => ClassId::NEUTRON_STAR_MERGER,
    };
    let query = spec.class_query(class);
    let Some(sphere) = plan.sphere.as_ref() else {
        return Some(SlabsWalked::default());
    };
    for x in slabs {
        if token.is_cancelled() {
            return None;
        }
        let cells = cells_in_slab(class, sphere, x);
        let occupied = match process {
            BackgroundProcess::CoreCollapse(p) => occupied_cells(galaxy, p, cells),
            BackgroundProcess::TypeIa(p) => occupied_cells(galaxy, p, cells),
            BackgroundProcess::StellarMergers(p) => occupied_cells(galaxy, p, cells),
            BackgroundProcess::NeutronStarMergers(p) => occupied_cells(galaxy, p, cells),
        };
        for (cell, count) in occupied {
            if token.is_cancelled() {
                return None;
            }
            drawn += u64::from(count);
            let out = &mut detections;
            match process {
                BackgroundProcess::CoreCollapse(p) => {
                    if reaches(&query, TransientKind::CoreCollapse, &cell) {
                        one_shot_scan_cell(galaxy, &mut ctx, &query, p, cell, out);
                    }
                    if reaches(&query, TransientKind::MagnetarBurst, &cell) {
                        magnetar_scan_cell(galaxy, &mut ctx, &query, p, cell, out);
                    }
                }
                BackgroundProcess::TypeIa(p) => {
                    one_shot_scan_cell(galaxy, &mut ctx, &query, p, cell, out);
                }
                BackgroundProcess::StellarMergers(p) => {
                    one_shot_scan_cell(galaxy, &mut ctx, &query, p, cell, out);
                }
                BackgroundProcess::NeutronStarMergers(p) => {
                    one_shot_scan_cell(galaxy, &mut ctx, &query, p, cell, out);
                }
            }
        }
    }
    Some(SlabsWalked {
        tracks: spec.tracked(detections),
        drawn,
    })
}

#[cfg(test)]
mod tests;
