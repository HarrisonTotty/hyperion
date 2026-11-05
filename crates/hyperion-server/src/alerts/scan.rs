//! What one of a subscription's scans walks, on a pool worker (plan 12, P12.T8.b; Design notes 7
//! and 8).
//!
//! A [`ScanRequest`] is walked by the sim's `alerts_in_with`: each class's census and detections,
//! and the local events of the systems an observed range query finds within
//! [`LOCAL_EVENTS_RADIUS`] of the observer (P12.T5.d, P12.T5.e). [`walk_interactive`] walks what
//! the census lets it walk at once, one interactive job. A class the census sends to the background
//! is planned by [`background_plan`], counted by [`census_slabs`] in bulk jobs of a few x-slabs of
//! its sphere each, and walked by [`walk_pieces`] in bulk jobs that take its pieces in turn (the
//! service's; plan 12, the mergers' balance): each piece one occupied cell, or a part of a merger
//! class's dense cell by candidate index, or one slab's lists. Each builds only the candidates
//! whose event time, read first, can light the window (P12.T8.b.2), so that a sensor that sees
//! across the galaxy is walked in seconds, its class `complete` or over its host limit.
//!
//! The first scan after a schedule starts or restarts walks each class back from its window's start
//! by the class's longest rise to peak ([`class_longest_rise`]; research's lean of 2026-10-30), and
//! the local events by the local kinds' longest, 25 years for an FU Orionis outburst: a riser whose
//! onset arrived before the window can cross a limit in it. Of what a walk finds, only what is lit
//! at the window's start or crosses a limit from then on is kept ([`ScanResult`]).
//!
//! Each detection is tracked on its own light ([`Tracked::of`], P12.T8.b.1), through the gas of the
//! features at the epoch while they are served: so a host's light is the same whichever scan lists
//! it, as the schedule asks (lean, 2026-11-01). The centre's black hole is not walked: the server
//! holds no universe's global list, so its census says so (`Unwalked::NoGlobalList`; lean,
//! 2026-11-01). From generator version 29 every class's walk reads the hosts of the features'
//! lists and the centre's beside its cells (the sim's `listed`), through the server's feature
//! cells and its listed hosts (`SharedListedHostsCache`), interactive and in the background jobs'
//! slabs alike.

use std::mem::size_of;
use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use hyperion_sim::alerts::{
    AlertBand, AlertContext, AlertQuery, BuildAlertQueryError, ClassCensus, ClassSet, ClassStatus,
    Detection, LocalEventsCache, LongestRise, ProductionAlertClasses, RecurrentProcess, Route,
    TransientKind, TransientRow, alerts_in_with, class_longest_rise, class_reach, kind_reach,
    listed_scan_slab, magnetar_scan_cell, one_shot_scan_cell, one_shot_scan_cell_in,
    recurrent_reach,
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

/// The x-slabs of a background class's sphere each census job counts: 1, so 256 jobs over the root
/// cube's 256 slabs of 512 ly cells, each some 24 ms of bounds and counts
/// (research-alerts-galaxy-scan), long beside a job's round trip. Four a job left the dense slabs
/// about the centre to a few workers: on 47 workers a whole-cube scan took 12.65 s of the fast
/// white dwarfs, 0.69 s of the magnetars and 0.25 s of the supernovae, one a job 7.78 s, 0.38 s
/// and 0.20 s (`awd_scan_cold` and `oneshot_scan_galaxy`, pod, 48 vCPU, 2026-11-01).
pub(crate) const SLABS_PER_JOB: usize = 1;

/// The most census jobs a background walk is split into: 256 (the brief for P12.T8.b.2), more
/// slabs to a job past it; and the most of its walk's jobs queued at once.
pub(crate) const MAX_BACKGROUND_JOBS: usize = 256;

/// How long a background walk's job takes pieces for before it hands its worker back, to the
/// pool's interactive jobs first: 1 s. A job takes one piece more whatever it has run, so a piece
/// is the least it walks. Whole x-slabs a job, the stellar mergers' two slabs about the centre
/// held a 4,096 ly whole-cube walk at 119.7 s on 16 workers, their neighbours' 36–39 s against a
/// median of 2.3 s (V 10⁻¹² W m⁻², 1,000 yr, the Milky Way-like seed `0x0926_0000`; pod, 48 vCPU,
/// release, 2026-11-05).
pub(crate) const JOB_BUDGET: Duration = Duration::from_secs(1);

/// How a background walk splits each occupied cell of a merger class among its jobs, by candidate
/// index (plan 12, the mergers' balance): into parts of at least `fewest` candidates, and into at
/// most `most`. A merger's candidates read no time first, so its walk builds every candidate's scan
/// marks: the eight 4,096 ly cells that meet at the galactic centre each draw some 12.15 million
/// candidates on the nuclear components' bound, at some 2 µs each, 23–25 s a cell; and the inner
/// disc's cells some 10,000, nearly all kept, at about 0.45 ms each, up to 5.7 s a cell (the
/// Milky Way-like seed `0x0926_0000`; pod, 48 vCPU, release, 2026-11-05).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CellParts {
    fewest: u32,
    most: u32,
}

impl CellParts {
    /// The service's: parts of at least 512 candidates, at most 64 a cell. An inner disc cell's
    /// 10,000 candidates are then 19 parts of some 0.3 s, a cell of 1,000 to 2,000 two or three of
    /// under a second, and a central cell's 64 of some 190,000, some 0.4 s each. At 1,024 the
    /// cells of 1,600 to 2,047 stayed whole, the walk's longest pieces at up to 1.9 s.
    pub(crate) const SERVICE: Self = Self {
        fewest: 512,
        most: 64,
    };

    /// Parts of at least `fewest` candidates and at most `most` a cell, each at least 1.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn new(fewest: u32, most: u32) -> Self {
        Self {
            fewest: if fewest == 0 { 1 } else { fewest },
            most: if most == 0 { 1 } else { most },
        }
    }

    /// The parts of a cell of `count` candidates: consecutive ranges of their indices, in order,
    /// that cover `0..count`, each of at least `fewest` candidates but in a cell of fewer, which is
    /// one part, and of at least one.
    fn of(self, count: u32) -> impl ExactSizeIterator<Item = Range<u32>> {
        let parts = (count / self.fewest).clamp(1, self.most);
        let at = move |part: u32| {
            // A count under 2²⁴ times at most `parts` fits in u64, and the quotient in the count.
            u32::try_from(u64::from(count) * u64::from(part) / u64::from(parts))
                .expect("a part's first index lies within the count")
        };
        (0..parts).map(move |part| at(part)..at(part + 1))
    }
}

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
    /// The candidates a background walk's cells drew, with its lists' from 29, or `None` for an
    /// interactive walk or a class no background walk reads.
    drawn: Option<u64>,
}

impl ScanResult {
    /// A background walk's, of every piece of `work` that `jobs` walked, in the walk's order: no
    /// census. The tracks of the parts of a cell split among its jobs are merged by arrival and
    /// then by event, with a stable sort, as the whole cell's walk sorts its detections
    /// ([`one_shot_scan_cell_in`]), so that they are the whole cell's in its order.
    ///
    /// # Panics
    ///
    /// If the jobs did not take every piece of `work`, or one twice: the jobs of a finished walk
    /// take each once.
    #[must_use]
    pub(crate) fn of_pieces(
        work: &BackgroundWork,
        jobs: impl IntoIterator<Item = PiecesWalked>,
    ) -> Self {
        let (mut taken, mut found) = (0, Vec::new());
        for job in jobs {
            taken += job.taken;
            found.extend(job.found);
        }
        assert_eq!(
            taken,
            work.pieces().len(),
            "every piece of a finished walk walked"
        );
        found.sort_unstable_by_key(|&(index, _)| index);
        assert!(
            found.windows(2).all(|pair| pair[0].0 < pair[1].0),
            "a piece walked twice"
        );
        let mut drawn = work.drawn;
        let walked = found.into_iter().map(|(index, walked)| {
            // Each count is under 2^24 a slab's lists, of under 2^24 slabs.
            drawn += walked.drawn;
            (&work.pieces()[index], walked.tracks)
        });
        let tracks = in_walk_order(walked, |track| {
            let detection = track.detection();
            (detection.arrival(), detection.event())
        });
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

    /// The candidates a background walk's cells drew within its class's reach, and from 29 its
    /// features' lists, its census's count of hosts (lean, 2026-11-01): `None` for an interactive
    /// walk, or a class no background walk reads.
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

/// The items of each of the pieces `walked`, given in the walk's order (any that found nothing may
/// be left out): each piece's in turn, and the parts of a cell split among a walk's jobs merged by
/// `key` with a stable sort, in the parts' order, as the whole cell's walk sorts them; a cell
/// walked whole keeps its own order. A cell's parts are consecutive pieces, so the parts of one
/// cell met in turn are all of it that found anything.
#[must_use]
fn in_walk_order<'a, T, K: Ord>(
    walked: impl IntoIterator<Item = (&'a Piece, Vec<T>)>,
    key: impl Fn(&T) -> K,
) -> Vec<T> {
    let mut items = Vec::new();
    // The cell whose parts' items end the list: its key, where they begin, and its parts.
    let mut cell: Option<(CatalogueCellKey, usize, usize)> = None;
    let merge = |items: &mut [T], cell: Option<(CatalogueCellKey, usize, usize)>| {
        if let Some((_, start, parts)) = cell
            && parts > 1
        {
            items[start..].sort_by_key(&key);
        }
    };
    for (piece, walked) in walked {
        match (piece, &mut cell) {
            (Piece::Part { cell: key, .. }, Some((last, _, parts))) if key == last => {
                *parts += 1;
            }
            (Piece::Part { cell: key, .. }, _) => {
                let start = items.len();
                merge(&mut items, cell.replace((*key, start, 1)));
            }
            (Piece::Cell(_) | Piece::Listed(_), _) => merge(&mut items, cell.take()),
        }
        items.extend(walked);
    }
    merge(&mut items, cell);
    items
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
    let listed = state.listed_hosts.handle(key);
    let gas = WalkGas::of(state, galaxy, &feature_cells);
    let mut noise = NoiseCache::with_capacity(NOISE_ENTRIES);
    let mut ctx = AlertContext::new(&mut noise, gas.source())
        .with_feature_cells(&feature_cells)
        .with_listed_hosts(&listed);
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
/// every class `ProductionAlertClasses` walks in cells, so every class its census can send to the
/// background. The one-shot classes, the core collapses with their magnetars; and the classes
/// walked through their recurrent events alone, the accreting white dwarfs, the X-ray binaries and
/// the luminous blue variables (P12.T5.c).
#[derive(Debug)]
#[expect(
    clippy::large_enum_variant,
    reason = "one is built a walk, behind an `Arc` its jobs share, never held in bulk"
)]
pub(crate) enum BackgroundProcess {
    CoreCollapse(CoreCollapseProcess),
    TypeIa(TypeIaProcess),
    StellarMergers(StellarMergers),
    NeutronStarMergers(NeutronStarMergers),
    Recurrent(RecurrentProcess),
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
            _ => RecurrentProcess::of(galaxy, class).map(Self::Recurrent),
        }
    }

    /// The class.
    #[must_use]
    fn class(&self) -> ClassId {
        match self {
            Self::CoreCollapse(_) => ClassId::CORE_COLLAPSE,
            Self::TypeIa(_) => ClassId::TYPE_IA,
            Self::StellarMergers(_) => ClassId::STELLAR_MERGER,
            Self::NeutronStarMergers(_) => ClassId::NEUTRON_STAR_MERGER,
            Self::Recurrent(process) => process.class(),
        }
    }
}

/// A background walk of one class, planned: its process, the sphere of its reach about the
/// observer, and its x-slabs in the census jobs that count them (P12.T8.b.2). It holds no cell
/// keys: each census job lists its slabs' occupied cells as it sweeps them ([`census_slabs`]).
#[derive(Debug, Clone)]
pub(crate) struct BackgroundPlan {
    process: Arc<BackgroundProcess>,
    /// `None` for a class the sensor cannot see, which has no job.
    sphere: Option<QuerySphere>,
    jobs: Vec<Range<i32>>,
}

impl BackgroundPlan {
    /// The slabs of each census job, in x order.
    #[must_use]
    pub(crate) fn jobs(&self) -> &[Range<i32>] {
        &self.jobs
    }
}

/// The plan of `class`'s background walk for `spec` in `galaxy`, its slabs `slabs_per_job` to a
/// census job, more where they would make over [`MAX_BACKGROUND_JOBS`] jobs; `None` for a class no
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

/// One piece of a background walk, which one of its jobs walks whole (plan 12, the mergers'
/// balance).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Piece {
    /// Every candidate of an occupied cell.
    Cell(CatalogueCellKey),
    /// The candidates `indices` of an occupied cell of a merger class, a part of a dense cell
    /// ([`CellParts`]): one of two or more, consecutive in the walk.
    Part {
        cell: CatalogueCellKey,
        indices: Range<u32>,
    },
    /// The class's hosts on the lists of the features whose centres lie in the x-slab, from
    /// generator version 29 (the sim's `listed`).
    Listed(i32),
}

/// What a census job found in its slabs: the pieces of their occupied cells and lists, in the
/// walk's order, and the candidates their cells drew.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SlabsCensus {
    pieces: Vec<Piece>,
    drawn: u64,
}

/// Counts the slabs `slabs` of `plan` in `galaxy`, on a pool worker: in x order, each slab's
/// occupied cells ([`occupied_cells`]) in its order, each one piece, or for a merger class the
/// parts `parts` makes of it, in index order, where it makes more than one; and then the slab's
/// lists. `None` if `token` was cancelled before the last slab.
#[must_use]
pub(crate) fn census_slabs(
    galaxy: &Galaxy,
    plan: &BackgroundPlan,
    slabs: Range<i32>,
    parts: CellParts,
    token: &CancelToken,
) -> Option<SlabsCensus> {
    let mut census = SlabsCensus::default();
    let Some(sphere) = plan.sphere.as_ref() else {
        return Some(census);
    };
    let process = plan.process.as_ref();
    let class = process.class();
    for x in slabs {
        if token.is_cancelled() {
            return None;
        }
        let cells = cells_in_slab(class, sphere, x);
        let (occupied, split) = match process {
            BackgroundProcess::CoreCollapse(p) => (occupied_cells(galaxy, p, cells), false),
            BackgroundProcess::TypeIa(p) => (occupied_cells(galaxy, p, cells), false),
            BackgroundProcess::StellarMergers(p) => (occupied_cells(galaxy, p, cells), true),
            BackgroundProcess::NeutronStarMergers(p) => (occupied_cells(galaxy, p, cells), true),
            BackgroundProcess::Recurrent(p) => (p.occupied_cells(galaxy, cells), false),
        };
        for (cell, count) in occupied {
            // Each count is under 2^24 a cell, of under 2^24 cells.
            census.drawn += u64::from(count);
            let of = parts.of(count);
            if split && of.len() > 1 {
                census
                    .pieces
                    .extend(of.map(|indices| Piece::Part { cell, indices }));
            } else {
                census.pieces.push(Piece::Cell(cell));
            }
        }
        census.pieces.push(Piece::Listed(x));
    }
    Some(census)
}

/// A background walk's pieces, in the walk's order, which its jobs take in turn: each piece is
/// walked by whichever job takes it next ([`walk_pieces`]), so that no job waits on a dense one
/// that another has (plan 12, the mergers' balance).
#[derive(Debug, Default)]
pub(crate) struct BackgroundWork {
    pieces: Vec<Piece>,
    drawn: u64,
    next: AtomicUsize,
}

impl BackgroundWork {
    /// The pieces of every census job's slabs, in the plan's order.
    #[must_use]
    pub(crate) fn new(census: Vec<SlabsCensus>) -> Self {
        let mut work = Self::default();
        // Some 549,000 pieces for the slow accreting white dwarfs' whole cube.
        work.pieces
            .reserve_exact(census.iter().map(|job| job.pieces.len()).sum());
        for job in census {
            work.pieces.extend(job.pieces);
            work.drawn += job.drawn;
        }
        work
    }

    /// The pieces, in the walk's order.
    #[must_use]
    pub(crate) fn pieces(&self) -> &[Piece] {
        &self.pieces
    }

    /// Whether every piece has been taken by a job.
    #[must_use]
    pub(crate) fn is_taken(&self) -> bool {
        self.next.load(Ordering::Relaxed) >= self.pieces.len()
    }

    /// The next piece no job has taken, with its place in the walk.
    fn take(&self) -> Option<(usize, &Piece)> {
        // Each place is handed out once; what a job walks returns with the job.
        let index = self.next.fetch_add(1, Ordering::Relaxed);
        self.pieces.get(index).map(|piece| (index, piece))
    }
}

/// What one piece of a background walk found: the tracks of what bears on the window, and the
/// candidates a slab's lists drew.
#[derive(Debug, Clone, Default, PartialEq)]
struct PieceWalked {
    tracks: Vec<Tracked>,
    drawn: u64,
}

/// What a background job walked: how many pieces it took, and each of them that found anything,
/// with its place in the walk. A whole-cube walk of the slow accreting white dwarfs takes some
/// 549,000 pieces, nearly all of which find nothing.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct PiecesWalked {
    taken: usize,
    found: Vec<(usize, PieceWalked)>,
}

/// Whether events of `key`'s cell seen within `reach` can reach `query`'s sensor: its box within
/// the reach and [`LIFETIME_REACH`] of the observer, and a light-year more for the rounding. A cell
/// beyond adds nothing to its kind's walk, so walking a few more changes no detection.
#[must_use]
fn reaches(query: &AlertQuery, reach: LightYears, key: &CatalogueCellKey) -> bool {
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

/// Walks the pieces of `work` that no other job has taken, for `spec` in `galaxy` by `plan`, on a
/// pool worker, until none is left or `budget` has run, and at least one if any is left: each
/// cell's one-shot events and, for the core collapses, its magnetars, each kind where it can reach
/// the sensor, cut to the candidates whose time read first can light the window
/// ([`one_shot_scan_cell`], [`magnetar_scan_cell`]), or of a merger class's part its candidates
/// ([`one_shot_scan_cell_in`]); or, for a class walked through its recurrent events alone, those
/// events where they can reach the sensor (`RecurrentProcess::scan_cell`, every candidate read).
/// And from generator version 29 each slab's features' lists of the class's hosts
/// (`listed_scan_slab`, through the server's listed hosts), their candidates counted with the
/// cells'. `None` if `token` was cancelled before the last piece.
///
/// Over every piece of the walk, however its jobs took them, the tracks merged in the walk's order
/// ([`ScanResult::of_pieces`]) are the class's own in [`walk_interactive`], each to the bit: each
/// piece's walk is a pure function of the piece.
#[must_use]
pub(crate) fn walk_pieces(
    state: &AppState,
    galaxy: &Galaxy,
    spec: &ScanSpec,
    plan: &BackgroundPlan,
    work: &BackgroundWork,
    budget: Duration,
    token: &CancelToken,
) -> Option<PiecesWalked> {
    let started = Instant::now();
    let mut walked = PiecesWalked::default();
    let Some(sphere) = plan.sphere.as_ref() else {
        return Some(walked);
    };
    let feature_cells = state.feature_cells.handle(spec.galaxy);
    let listed = state.listed_hosts.handle(spec.galaxy);
    let gas = WalkGas::of(state, galaxy, &feature_cells);
    let mut noise = NoiseCache::with_capacity(NOISE_ENTRIES);
    let mut ctx = AlertContext::new(&mut noise, gas.source())
        .with_feature_cells(&feature_cells)
        .with_listed_hosts(&listed);
    let process = plan.process.as_ref();
    let class = process.class();
    let query = spec.class_query(class);
    let sensor = query.sensor();
    let (supernovae, magnetars, recurrent) = (
        kind_reach(TransientKind::CoreCollapse, sensor),
        kind_reach(TransientKind::MagnetarBurst, sensor),
        recurrent_reach(class, sensor),
    );
    while walked.taken == 0 || started.elapsed() < budget {
        if token.is_cancelled() {
            return None;
        }
        let Some((index, piece)) = work.take() else {
            break;
        };
        let mut detections = Vec::new();
        let out = &mut detections;
        let mut drawn = 0;
        match (piece, process) {
            (&Piece::Cell(cell), BackgroundProcess::CoreCollapse(p)) => {
                if reaches(&query, supernovae, &cell) {
                    one_shot_scan_cell(galaxy, &mut ctx, &query, p, cell, out);
                }
                if reaches(&query, magnetars, &cell) {
                    magnetar_scan_cell(galaxy, &mut ctx, &query, p, cell, out);
                }
            }
            (&Piece::Cell(cell), BackgroundProcess::TypeIa(p)) => {
                one_shot_scan_cell(galaxy, &mut ctx, &query, p, cell, out);
            }
            (&Piece::Cell(cell), BackgroundProcess::StellarMergers(p)) => {
                one_shot_scan_cell(galaxy, &mut ctx, &query, p, cell, out);
            }
            (&Piece::Cell(cell), BackgroundProcess::NeutronStarMergers(p)) => {
                one_shot_scan_cell(galaxy, &mut ctx, &query, p, cell, out);
            }
            (&Piece::Cell(cell), BackgroundProcess::Recurrent(p)) => {
                if reaches(&query, recurrent, &cell) {
                    p.scan_cell(galaxy, &mut ctx, cell, &query, out);
                }
            }
            (Piece::Part { cell, indices }, BackgroundProcess::StellarMergers(p)) => {
                let part = indices.clone();
                one_shot_scan_cell_in(galaxy, &mut ctx, &query, p, *cell, part, out);
            }
            (Piece::Part { cell, indices }, BackgroundProcess::NeutronStarMergers(p)) => {
                let part = indices.clone();
                one_shot_scan_cell_in(galaxy, &mut ctx, &query, p, *cell, part, out);
            }
            (
                Piece::Part { .. },
                BackgroundProcess::CoreCollapse(_)
                | BackgroundProcess::TypeIa(_)
                | BackgroundProcess::Recurrent(_),
            ) => unreachable!("the census splits the merger classes' cells alone"),
            // Nothing for a class no list holds, or below 29.
            (&Piece::Listed(x), _) => {
                drawn = listed_scan_slab(galaxy, &mut ctx, &query, (class, sphere), x, out);
            }
        }
        walked.taken += 1;
        let tracks = spec.tracked(detections);
        if !tracks.is_empty() || drawn > 0 {
            walked.found.push((index, PieceWalked { tracks, drawn }));
        }
    }
    Some(walked)
}

#[cfg(test)]
mod tests;
