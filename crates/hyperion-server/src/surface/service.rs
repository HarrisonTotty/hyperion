//! The surface service: a body's coarse field, computed once on the pool through a single flight
//! and kept in the [`FieldCache`] (the [module](super) documentation).

use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use hyperion_protocol::SeedHex;
use hyperion_sim::Seed;
use hyperion_sim::planetary::derive::figure::BodyFigure;
use hyperion_sim::planetary::hooks::{SurfaceSeed, surface_seed};
use hyperion_sim::planetary::record::{BodyRecord, RecordSection, Section};
use hyperion_sim::planetary::surface::{CoarseInputs, SurfaceInputsError, coarse_pass};
use hyperion_sim::planetary::{BodyIndex, ResolveBodyError};
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::Metres;
use hyperion_surface::field::{CoarseField, CoarseLevel, FieldView, coarse_level};
use tokio::sync::watch;
use tokio::task::JoinSet;

use super::{FieldCache, FieldKey, InputsSource};
use crate::cache::LruCounters;
use crate::compute::sky::bulk;
use crate::compute::{
    CancelOnDrop, CancelToken, ComputeError, CpuPool, Flight, GeneratedSystem, JobError, Priority,
    SingleFlight, SubmitJobError,
};
use crate::knowledge::LoadKnowledgeError;
use crate::knowledge::surveys::{RaisedRevisions, SurveyLog};
use crate::universe::{UniverseId, UniverseStore};

/// The time at which the service reads a body's record for its inputs: the epoch.
///
/// It is the same whoever asks and whenever, so that the field kept is a function of its key (the
/// [module](super) documentation). P14.T54.a, which settles the time the record's surface section
/// is read at, settles this one too.
pub const RECORD_TIME: UniverseTime = UniverseTime::EPOCH;

/// Why a body's coarse field could not be had.
///
/// The name is the plan's Provides', which R09.T19 consumes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SurfaceJobError {
    /// The key's body index is not in plan 14's layout, or its system holds no body there.
    Body(ResolveBodyError),
    /// The body's inputs cannot be had: it has no solid surface (a giant, a ring, a belt or the
    /// halo), or a section of its record that the pass reads is not modelled or is withheld.
    Inputs(SurfaceInputsError),
    /// The pass's pool job could not be queued, or did not finish: it was cancelled, panicked or
    /// was dropped with the pool, or a worker's floating-point mode flushes subnormals.
    Compute(ComputeError),
}

impl fmt::Display for SurfaceJobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Body(_) => "the body does not resolve",
            Self::Inputs(_) => "the body's coarse-pass inputs cannot be had",
            Self::Compute(_) => "the body's coarse pass did not run",
        })
    }
}

impl Error for SurfaceJobError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Body(source) => Some(source),
            Self::Inputs(source) => Some(source),
            Self::Compute(source) => Some(source),
        }
    }
}

impl From<ComputeError> for SurfaceJobError {
    fn from(error: ComputeError) -> Self {
        Self::Compute(error)
    }
}

/// The surface service's counters: the cache's size and use, and the passes that filled it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct SurfaceCounters {
    cache: LruCounters,
    computed: u64,
}

impl SurfaceCounters {
    /// The fields held, the bytes they are charged, the budget, and the hits, misses, evictions
    /// and refusals so far.
    ///
    /// A request that has to compute its field counts two misses, one before its flight and one
    /// inside it, which looks again in case another flight finished meanwhile; one that joins a
    /// flight in progress counts one.
    #[must_use]
    pub fn cache(&self) -> LruCounters {
        self.cache
    }

    /// Coarse passes run to their end so far.
    ///
    /// Callers that ask for one body's field at once count one pass between them. A refused body
    /// (no solid surface, inputs not modelled) runs none.
    #[must_use]
    pub fn computed(&self) -> u64 {
        self.computed
    }
}

/// The level of a body's coarse field, which its survey passes index, and whether the field can be
/// computed now ([`SurfaceService::field_level`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum FieldLevel {
    /// The service's source gives the body's inputs: its field is at this level, the one their
    /// radius gives, and can be computed.
    Modelled(CoarseLevel),
    /// The source answers that a section of the record the coarse pass reads is not modelled, so
    /// the body has no field yet. Its passes are still Knowledge, kept at the level of the
    /// record's figure, whose volumetric radius is the one the inputs will read once the section
    /// is modelled (R09.T19.a, ruled 2026-10-13).
    NotModelled {
        /// The level the figure's radius gives.
        level: CoarseLevel,
        /// The section the pass needs.
        section: RecordSection,
    },
}

impl FieldLevel {
    /// The level the body's cells are indexed at.
    #[must_use]
    pub(crate) const fn level(self) -> CoarseLevel {
        match self {
            Self::Modelled(level) | Self::NotModelled { level, .. } => level,
        }
    }
}

/// Every body's coarse field, computed once on the CPU pool and kept in one byte budget (the
/// [module](super) documentation).
pub struct SurfaceService {
    pool: Arc<CpuPool>,
    /// Where every field's inputs come from.
    source: Arc<dyn InputsSource>,
    held: Arc<FieldCache>,
    flights: SingleFlight<FieldKey, CoarseField, SurfaceJobError>,
    computed: Arc<AtomicU64>,
    /// The passes started by [`prefetch`](Self::prefetch), each waited on by a task of its own so
    /// that its flight lives with no request waiting; aborted by
    /// [`stop_prefetching`](Self::stop_prefetching), at the server's shutdown, or when the service
    /// is dropped.
    prefetching: Mutex<JoinSet<()>>,
    /// Each universe's survey log, opened once ([`survey_log`](Self::survey_log)).
    surveys: Arc<Mutex<HashMap<UniverseId, SurveyLog>>>,
    /// The survey logs being opened, so that two first requests of a universe open its log once.
    opening: SingleFlight<UniverseId, SurveyLog, Arc<LoadKnowledgeError>>,
    /// Each universe's raised revisions, made by the first of its scene subscriptions or its log's
    /// opening, whichever comes first, and given to the log when it opens
    /// ([`watch_surveys`](Self::watch_surveys)).
    raised: Arc<Mutex<HashMap<UniverseId, watch::Sender<RaisedRevisions>>>>,
}

impl SurfaceService {
    /// A service that computes its fields on `pool` from the inputs `source` gives, keeping at
    /// most `budget_bytes` of them.
    ///
    /// A budget of zero keeps nothing: every request computes its field, though requests made at
    /// once still share one pass.
    #[must_use]
    pub fn new(pool: Arc<CpuPool>, source: Arc<dyn InputsSource>, budget_bytes: usize) -> Self {
        Self {
            pool,
            source,
            held: Arc::new(FieldCache::new(budget_bytes)),
            flights: SingleFlight::new(),
            computed: Arc::new(AtomicU64::new(0)),
            prefetching: Mutex::new(JoinSet::new()),
            surveys: Arc::new(Mutex::new(HashMap::new())),
            opening: SingleFlight::new(),
            raised: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// The coarse field of the body `key` names, whose planetary system is `system`: the one held,
    /// or computed as one bulk job on the pool.
    ///
    /// The job reads the body's record at [`RECORD_TIME`], takes its inputs from the service's
    /// source and runs the coarse pass under the body's surface seed. Callers that ask for one
    /// field at once share the job; if every one of them gives up, a job still queued is skipped
    /// and nothing is kept.
    ///
    /// # Errors
    ///
    /// - [`SurfaceJobError::Body`] if the key's body index is not in plan 14's layout
    ///   ([`ResolveBodyError::MalformedIndex`]) or the system holds no body there
    ///   ([`ResolveBodyError::NoSuchBody`]);
    /// - [`SurfaceJobError::Inputs`] with [`SurfaceInputsError::NoSolidSurface`] for a body with
    ///   no surface seed (a ring, a belt or the halo) or none the source finds, and with the
    ///   source's [`SurfaceInputsError::NotModelled`] or [`SurfaceInputsError::NotResolved`]. The
    ///   record is read at full detail, so `NotResolved` is a bug, which a request answers
    ///   `internal` (the plan's Risks, T10's record);
    /// - [`SurfaceJobError::Compute`] if the pool is shutting down or faulted, or the job was
    ///   cancelled, panicked, was dropped with the pool or ran on a worker whose floating-point
    ///   mode flushes subnormals, which is logged.
    ///
    /// None of these is kept: the next request asks again.
    ///
    /// # Panics
    ///
    /// If `system` is not the planetary system of the key's body in the key's universe, since its
    /// field would then be kept under another body's key, or if the key's generator version is not
    /// one this build generates. The server takes both from the request's own universe and body,
    /// and the registry refuses a universe of another version, so a mismatch is a bug.
    pub async fn field(
        &self,
        key: FieldKey,
        system: &Arc<GeneratedSystem>,
    ) -> Result<Arc<CoarseField>, SurfaceJobError> {
        check_system(key, system);
        if let Some(field) = self.held.get(&key) {
            return Ok(field);
        }
        self.flight(key, system).await
    }

    /// Starts the field of the body `key` names, whose planetary system is `system`, unless it is
    /// held, and returns at once whether it started or joined a pass.
    ///
    /// The pass runs on the pool at bulk priority through the service's single flight, waited on
    /// by a task the service owns, and a request made meanwhile joins it. Session code will call it
    /// as the ship approaches a body (Design note 19), and is why it is public; `survey_pass` calls
    /// it for a body whose inputs are modelled (R09.T19.a). It looks for the field as a request
    /// does, so it counts a hit when the field is held; otherwise a miss, and one more inside the
    /// flight where it starts the pass, but none there where it joins a pass already in flight. A
    /// pass that
    /// fails is logged (a refused body at `debug`, a job that did not run at `warn`), and the next
    /// request asks again.
    ///
    /// # Panics
    ///
    /// As [`field`](Self::field), if `system` is not the key's body's; and outside a Tokio
    /// runtime, where the task that waits on the pass cannot be spawned.
    pub fn prefetch(&self, key: FieldKey, system: &Arc<GeneratedSystem>) -> bool {
        check_system(key, system);
        if self.held.get(&key).is_some() {
            return false;
        }
        let flight = self.flight(key, system);
        let mut tasks = self
            .prefetching
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        // The finished tasks are reaped as new ones start, so the set holds only waiters on passes
        // still running. A body prefetched again during its pass adds one more task awaiting the
        // same flight: the field is still computed once.
        while let Some(finished) = tasks.try_join_next() {
            if let Err(error) = finished
                && error.is_panic()
            {
                tracing::error!(%error, "a coarse pass started by a prefetch panicked");
            }
        }
        tasks.spawn(async move {
            match flight.await {
                Ok(_) => {}
                Err(error @ (SurfaceJobError::Body(_) | SurfaceJobError::Inputs(_))) => {
                    tracing::debug!(
                        body = %key.body(),
                        ?error,
                        "a prefetched body has no coarse field"
                    );
                }
                Err(error @ SurfaceJobError::Compute(_)) => {
                    tracing::warn!(
                        body = %key.body(),
                        ?error,
                        "a prefetched body's coarse field was not computed"
                    );
                }
            }
        });
        true
    }

    /// The level of the coarse field of the body `key` names, whose planetary system is `system`,
    /// and whether the field can be computed now, read as one interactive job on the pool.
    ///
    /// The job reads what [`field`](Self::field)'s pass reads before it runs: the body's surface
    /// seed, its record at [`RECORD_TIME`] and its inputs from the service's source, so that a
    /// survey pass is checked against the field's own cells and a body with no field is answered
    /// without a bulk slot. Where the source gives inputs, the level is their radius's; where it
    /// answers [`SurfaceInputsError::NotModelled`], the record's figure's
    /// ([`FieldLevel::NotModelled`]).
    ///
    /// # Errors
    ///
    /// - [`SurfaceJobError::Body`] and [`SurfaceJobError::Inputs`], as [`field`](Self::field)
    ///   gives them, except that a source's [`SurfaceInputsError::NotModelled`] is answered from
    ///   the record's figure ([`level_from`]): an error naming [`RecordSection::Figure`] only where
    ///   the figure gives no level either;
    /// - [`SurfaceJobError::Compute`] if the interactive queue is full or the pool is shutting down
    ///   or faulted, or the job was cancelled through `token`, panicked or was dropped with the
    ///   pool.
    ///
    /// # Panics
    ///
    /// As [`field`](Self::field), if `system` is not the key's body's or the key's generator
    /// version is not one this build generates.
    pub(crate) async fn field_level(
        &self,
        key: FieldKey,
        system: &Arc<GeneratedSystem>,
        token: CancelToken,
    ) -> Result<FieldLevel, SurfaceJobError> {
        check_system(key, system);
        let (source, system) = (Arc::clone(&self.source), Arc::clone(system));
        let receiver = self
            .pool
            .try_submit(Priority::Interactive, token, move |_: &CancelToken| {
                level_of(source.as_ref(), key, &system)
            })
            .map_err(|error| SurfaceJobError::Compute(error.into()))?;
        receiver
            .await
            .unwrap_or_else(|closed| Err(JobError::from(closed)))
            .map_err(|error| SurfaceJobError::Compute(error.into()))?
    }

    /// Universe `universe`'s survey log, opened from `store` on its first use and kept, so that its
    /// file has one writer (R09.T18's record).
    ///
    /// Callers that ask at once while it is being opened share one open. The service keeps the log
    /// until P12.T8's holder of a universe's Knowledge exists and takes it over.
    ///
    /// # Errors
    ///
    /// [`LoadKnowledgeError`], shared between the callers that waited on the open, as
    /// [`SurveyLog::open`] gives it; nothing is kept, and the next call opens the log again.
    pub(crate) async fn survey_log(
        &self,
        store: &UniverseStore,
        universe: UniverseId,
    ) -> Result<SurveyLog, Arc<LoadKnowledgeError>> {
        if let Some(log) = lock(&self.surveys).get(&universe) {
            return Ok(log.clone());
        }
        let (surveys, store) = (Arc::clone(&self.surveys), store.clone());
        let raised = raised_sender(&self.raised, universe);
        let opened = self
            .opening
            .run(universe, move || async move {
                // As in `flight`: a caller can arrive just after another open has finished and
                // left the registry, and the log it opened is the one to share.
                if let Some(log) = lock(&surveys).get(&universe) {
                    return Ok(Arc::new(log.clone()));
                }
                let log = SurveyLog::open_announcing(&store, universe, raised)
                    .await
                    .map_err(Arc::new)?;
                let log = lock(&surveys).entry(universe).or_insert(log).clone();
                Ok(Arc::new(log))
            })
            .await?;
        Ok(SurveyLog::clone(&opened))
    }

    /// A receiver of universe `universe`'s [`RaisedRevisions`]: woken whenever a survey pass of one
    /// of its bodies is recorded, the body's revision raised in it (R09.T19.b).
    ///
    /// It holds the revisions raised since the universe's log was opened; the scene topic reads
    /// what it holds when a subscription opens as already told, and tells its subscriber of each
    /// body raised past that. The channel is made here if the universe has none yet, and the log
    /// is given it when it opens, so a scene subscribed before the universe's first survey pass is
    /// told of it.
    #[must_use]
    pub(crate) fn watch_surveys(&self, universe: UniverseId) -> watch::Receiver<RaisedRevisions> {
        raised_sender(&self.raised, universe).subscribe()
    }

    /// Stops waiting on the passes [`prefetch`](Self::prefetch) started.
    ///
    /// Each is given up unless a request waits on it too, and its job, if still queued, is
    /// skipped. For the server's shutdown, before its pool stops. It does not wait for the waiters
    /// to end: each is aborted at its next poll.
    pub(crate) fn stop_prefetching(&self) {
        self.prefetching
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .abort_all();
    }

    /// The fields held, the bytes they are charged, their use, and the passes computed so far.
    #[must_use]
    pub fn counters(&self) -> SurfaceCounters {
        SurfaceCounters {
            cache: self.held.counters(),
            computed: self.computed.load(Ordering::Relaxed),
        }
    }

    /// The pass of `key`'s field, joined if one is in progress: it looks for the field once more
    /// inside the flight, then computes it as one bulk job and keeps it.
    fn flight(
        &self,
        key: FieldKey,
        system: &Arc<GeneratedSystem>,
    ) -> Flight<CoarseField, SurfaceJobError> {
        let (pool, source, held, computed, system) = (
            Arc::clone(&self.pool),
            Arc::clone(&self.source),
            Arc::clone(&self.held),
            Arc::clone(&self.computed),
            Arc::clone(system),
        );
        self.flights.run(key, move || async move {
            // As in `SkyTablesService::flight`: a caller can arrive just after another flight has
            // finished and left the registry, and the field it computed is worth more than a
            // second pass. The look is counted, since `SharedByteLru` has no uncounted lookup.
            if let Some(field) = held.get(&key) {
                return Ok(field);
            }
            let token = CancelToken::new();
            // Cancels the pass's queued job once every waiter on this flight has gone.
            let _cancel_on_drop = CancelOnDrop::new(token.clone());
            let asked = Instant::now();
            let (field, pass) = bulk(&pool, &token, move |_: &CancelToken| {
                let started = Instant::now();
                compute(source.as_ref(), key, &system).map(|field| (field, started.elapsed()))
            })
            .await
            .inspect_err(|error| log_refusal(key, *error))??;
            let field = Arc::new(field);
            computed.fetch_add(1, Ordering::Relaxed);
            tracing::info!(
                seed = %SeedHex::from_u64(key.galaxy().seed()),
                generator_version = %key.galaxy().generator_version(),
                body = %key.body(),
                level = field.header().level().get(),
                pass_ms = pass.as_secs_f64() * 1e3,
                waited_ms = asked.elapsed().saturating_sub(pass).as_secs_f64() * 1e3,
                heap_mib = field.heap_bytes() / (1 << 20),
                "computed a body's coarse field"
            );
            // A field larger than the whole budget is handed back and not kept; the waiters get it
            // either way.
            drop(held.insert(key, Arc::clone(&field)));
            Ok(field)
        })
    }
}

impl fmt::Debug for SurfaceService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let prefetching = self
            .prefetching
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len();
        f.debug_struct("SurfaceService")
            .field("source", &self.source)
            .field("counters", &self.counters())
            .field("computing", &self.flights.len())
            .field("prefetching", &prefetching)
            .field("survey_logs", &lock(&self.surveys).len())
            .finish_non_exhaustive()
    }
}

/// Panics unless `system` is the planetary system of `key`'s body in `key`'s universe: its system
/// ID is the body's, and where the body has a surface seed, it is the one the key's universe draws
/// for the body, so the system was generated from that universe's seed; and unless the key's
/// generator version is one this build generates, as the galaxy cache requires, so that no field
/// is kept under another version's key.
fn check_system(key: FieldKey, system: &GeneratedSystem) {
    assert!(
        key.galaxy().generator_version().is_supported(),
        "asked for a coarse field of generator version {}, which this build cannot generate",
        key.galaxy().generator_version()
    );
    let planets = &system.1;
    assert_eq!(
        planets.system(),
        key.body().system(),
        "a body's coarse field is computed from its own system, and this is another's"
    );
    if let Ok(index) = BodyIndex::try_from(key.body().body_index())
        && let Ok(Some(seed)) = planets.surface_seed(index)
    {
        assert_eq!(
            seed,
            surface_seed(Seed::new(key.galaxy().seed()), key.body()),
            "a body's coarse field is computed from its own universe's system, and this is another \
             universe's"
        );
    }
}

/// The coarse field of `key`'s body in `system`, on the pool: the body's surface seed and its
/// record at [`RECORD_TIME`], the inputs `source` reads from the record, and the pass.
///
/// # Errors
///
/// [`SurfaceJobError::Body`] and [`SurfaceJobError::Inputs`], as
/// [`SurfaceService::field`] gives them.
fn compute(
    source: &dyn InputsSource,
    key: FieldKey,
    system: &GeneratedSystem,
) -> Result<CoarseField, SurfaceJobError> {
    let (seed, record) = surface_record(key, system)?;
    let inputs = source
        .inputs(&record, &system.0)
        .map_err(SurfaceJobError::Inputs)?;
    Ok(coarse_pass(seed, &inputs))
}

/// The level of the field of `key`'s body in `system`, on the pool, as
/// [`SurfaceService::field_level`] gives it.
///
/// # Errors
///
/// As [`SurfaceService::field_level`], but for a pool job's failure.
fn level_of(
    source: &dyn InputsSource,
    key: FieldKey,
    system: &GeneratedSystem,
) -> Result<FieldLevel, SurfaceJobError> {
    let (_, record) = surface_record(key, system)?;
    level_from(source.inputs(&record, &system.0), record.figure())
}

/// The level of a body's field from what the source answered for it, `inputs`, and its record's
/// figure section, `figure`.
///
/// Inputs give the level of their radius. A section the inputs need that is not modelled gives the
/// level of the figure's volumetric radius, the radius the inputs will read once the section is
/// modelled ([`FieldLevel::NotModelled`]).
///
/// # Errors
///
/// The source's [`SurfaceInputsError::NoSolidSurface`] and [`SurfaceInputsError::NotResolved`];
/// and where the source answers [`SurfaceInputsError::NotModelled`], by the figure's state:
/// [`SurfaceInputsError::NotModelled`] naming [`RecordSection::Figure`] for a figure not
/// modelled, [`SurfaceInputsError::NotResolved`] naming it for one withheld, which full records
/// never are, and [`SurfaceInputsError::NoSolidSurface`] for one not applicable, a body absent at
/// the record's time.
fn level_from(
    inputs: Result<CoarseInputs, SurfaceInputsError>,
    figure: &Section<BodyFigure>,
) -> Result<FieldLevel, SurfaceJobError> {
    let section = match inputs {
        Ok(inputs) => return Ok(FieldLevel::Modelled(coarse_level(inputs.radius()))),
        Err(SurfaceInputsError::NotModelled(section)) => section,
        Err(error @ (SurfaceInputsError::NoSolidSurface | SurfaceInputsError::NotResolved(_))) => {
            return Err(SurfaceJobError::Inputs(error));
        }
    };
    let error = match figure {
        Section::Ok(figure) => {
            return Ok(FieldLevel::NotModelled {
                level: coarse_level(Metres::new(figure.spheroid().volumetric_radius_m())),
                section,
            });
        }
        Section::NotModelled => SurfaceInputsError::NotModelled(RecordSection::Figure),
        Section::NotResolved => SurfaceInputsError::NotResolved(RecordSection::Figure),
        Section::NotApplicable => SurfaceInputsError::NoSolidSurface,
    };
    Err(SurfaceJobError::Inputs(error))
}

/// The surface seed of `key`'s body in `system` and its record at [`RECORD_TIME`]: what the coarse
/// pass reads before its inputs.
///
/// # Errors
///
/// [`SurfaceJobError::Body`] for an index outside plan 14's layout or one at which the system
/// holds no body, and [`SurfaceJobError::Inputs`] with [`SurfaceInputsError::NoSolidSurface`] for
/// a body with no surface seed (a ring, a belt or the halo).
fn surface_record(
    key: FieldKey,
    system: &GeneratedSystem,
) -> Result<(SurfaceSeed, BodyRecord), SurfaceJobError> {
    let (ctx, planets) = system;
    let index = BodyIndex::try_from(key.body().body_index())
        .map_err(|error| SurfaceJobError::Body(ResolveBodyError::MalformedIndex(error)))?;
    let seed = planets
        .surface_seed(index)
        .map_err(SurfaceJobError::Body)?
        .ok_or(SurfaceJobError::Inputs(SurfaceInputsError::NoSolidSurface))?;
    let record = planets
        .body_at(ctx, index, RECORD_TIME)
        .map_err(SurfaceJobError::Body)?;
    Ok((seed, record))
}

/// Universe `universe`'s raised-revisions channel in `raised`, made empty if it has none yet.
///
/// A universe's channel is kept for the service's life: it is a few bytes a body surveyed, and a
/// log opened again after it was dropped (P12.T8's holder) keeps telling the same subscriptions.
fn raised_sender(
    raised: &Mutex<HashMap<UniverseId, watch::Sender<RaisedRevisions>>>,
    universe: UniverseId,
) -> watch::Sender<RaisedRevisions> {
    lock(raised).entry(universe).or_default().clone()
}

/// Takes the survey logs' lock whether or not a holder panicked: each critical section is one map
/// operation, which leaves the map whole.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Logs a pass the pool refused or did not finish where it means the server cannot generate at
/// all: a worker's floating-point mode flushes subnormals (plan R04, Design note 15), which a
/// request then answers `internal`. The other causes are the requests' own (a cancellation, a
/// shutdown) or are logged by the pool (a panic).
fn log_refusal(key: FieldKey, error: ComputeError) {
    if matches!(
        error,
        ComputeError::Job(JobError::FloatingPointMode)
            | ComputeError::Submit(SubmitJobError::Faulted)
    ) {
        tracing::error!(
            body = %key.body(),
            %error,
            "refused a body's coarse field: a cpu worker's floating-point mode flushes subnormals"
        );
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;
    use std::sync::OnceLock;
    use std::sync::atomic::AtomicUsize;
    use std::sync::mpsc;

    use futures_util::FutureExt;
    use futures_util::future::join3;
    use hyperion_protocol::{ErrorCode, RequestError};
    use hyperion_sim::galaxy::Galaxy;
    use hyperion_sim::galaxy::placement::{CellKey, generate_cell};
    use hyperion_sim::id::{BodyId, Layer, SystemId};
    use hyperion_sim::planetary::record::BodyRecord;
    use hyperion_sim::planetary::surface::{CoarseInputs, reference};
    use hyperion_sim::planetary::{DecodeBodyIndexError, SystemContext, generate};
    use hyperion_sim::{GENERATOR_VERSION, Seed};
    use hyperion_surface::field::{Cover, CoverRange, ResolutionCode};
    use hyperion_surface::wire::encode_payload;
    use tokio::time::timeout;

    use super::*;
    use crate::compute::{FlushProbe, GalaxyKey, JobReceiver, Priority};
    use crate::limits::{BULK_QUEUE_CAPACITY, INTERACTIVE_QUEUE_CAPACITY};
    use crate::testing::WAIT;

    /// The seed of the universe these tests compute fields in, the integration tests' own.
    const SEED: u64 = 0x4d2;

    fn galaxy() -> &'static Galaxy {
        static GALAXY: OnceLock<Galaxy> = OnceLock::new();
        GALAXY.get_or_init(|| Galaxy::new(Seed::new(SEED)))
    }

    /// The first system of the layer-C cell at the solar circle with three bodies or more that
    /// have a surface seed (planets and moons), with those bodies.
    fn system() -> &'static (Arc<GeneratedSystem>, Vec<BodyId>) {
        static SYSTEM: OnceLock<(Arc<GeneratedSystem>, Vec<BodyId>)> = OnceLock::new();
        SYSTEM.get_or_init(|| {
            let mut cell = Vec::new();
            generate_cell(
                galaxy(),
                CellKey::new(Layer::C, [0, 812, 0]).expect("a cell of the grid"),
                &mut cell,
            );
            cell.iter()
                .find_map(|record| {
                    let ctx = SystemContext::for_system(galaxy(), record.id()).ok()?;
                    let planets = generate(Seed::new(SEED), &ctx);
                    let bodies: Vec<BodyId> = planets
                        .bodies()
                        .iter()
                        .filter(|body| matches!(planets.surface_seed(body.index()), Ok(Some(_))))
                        .map(|body| body.index().body_id(planets.system()))
                        .collect();
                    (bodies.len() >= 3).then(|| (Arc::new((ctx, planets)), bodies))
                })
                .expect("a system of the cell has three bodies with a surface seed")
        })
    }

    fn key(body: BodyId) -> FieldKey {
        FieldKey::new(GalaxyKey::new(SEED, GENERATOR_VERSION), body)
    }

    /// The field of body `n` of [`system`].
    fn nth(n: usize) -> FieldKey {
        key(system().1[n])
    }

    /// A pool of one worker, which takes its jobs in order.
    fn one_worker() -> Arc<CpuPool> {
        Arc::new(
            CpuPool::new(
                NonZeroUsize::MIN,
                INTERACTIVE_QUEUE_CAPACITY,
                BULK_QUEUE_CAPACITY,
            )
            .unwrap(),
        )
    }

    /// A source that gives every body the reference Ceres, a level-5 field, counting its calls.
    #[derive(Debug, Default)]
    struct Counted {
        calls: AtomicUsize,
    }

    impl Counted {
        fn calls(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    impl InputsSource for Counted {
        fn inputs(
            &self,
            _: &BodyRecord,
            _: &SystemContext,
        ) -> Result<CoarseInputs, SurfaceInputsError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(reference::ceres_like())
        }
    }

    /// A service of `budget` bytes on `pool`, its source counted.
    fn service(pool: &Arc<CpuPool>, budget: usize) -> (SurfaceService, Arc<Counted>) {
        let source = Arc::new(Counted::default());
        let service = SurfaceService::new(
            Arc::clone(pool),
            Arc::clone(&source) as Arc<dyn InputsSource>,
            budget,
        );
        (service, source)
    }

    /// Holds the pool's one worker on a job until the sender returned is sent to, so that the jobs
    /// queued meanwhile wait.
    async fn hold(pool: &CpuPool) -> (mpsc::Sender<()>, JobReceiver<()>) {
        let (release, gate) = mpsc::channel::<()>();
        let held = pool
            .submit(
                Priority::Bulk,
                CancelToken::new(),
                move |_: &CancelToken| {
                    gate.recv().expect("the gate is released");
                },
            )
            .await
            .unwrap();
        (release, held)
    }

    /// Waits until the one worker has run every job queued before now.
    async fn drain(pool: &CpuPool) {
        let token = CancelToken::new();
        timeout(WAIT, bulk(pool, &token, |_: &CancelToken| ()))
            .await
            .expect("timed out draining the pool")
            .unwrap();
    }

    /// Yields until `ready` holds.
    async fn until(what: &str, ready: impl Fn() -> bool) {
        timeout(WAIT, async {
            while !ready() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("timed out waiting until {what}"));
    }

    /// The whole field as the wire carries it, every cell surveyed: the encoder writes each value's
    /// bits, so two fields that encode alike are one field, bit for bit.
    fn payload(field: &CoarseField) -> Vec<u8> {
        let cells = field.header().level().cell_count();
        let all = Cover::from_ranges([CoverRange::new(0, cells, ResolutionCode::new(1)).unwrap()])
            .unwrap();
        encode_payload(field, &all, None)
    }

    /// Asserts that `a` and `b` are one field, bit for bit, naming the first byte of their payloads
    /// that differs, since a field is too large to print.
    fn assert_same_bits(a: &CoarseField, b: &CoarseField, what: &str) {
        let (a, b) = (payload(a), payload(b));
        assert_eq!(a.len(), b.len(), "{what}: the payloads' lengths");
        assert_eq!(
            a.iter().zip(&b).position(|(x, y)| x != y),
            None,
            "{what}: the payloads' first differing byte"
        );
    }

    async fn field(
        service: &SurfaceService,
        key: FieldKey,
    ) -> Result<Arc<CoarseField>, SurfaceJobError> {
        timeout(WAIT, service.field(key, &system().0))
            .await
            .expect("timed out computing a field")
    }

    /// Two requests for one body's field at once share one pass: one call of the source, one field
    /// computed and kept, and it is the sim's own pass of the body's surface seed and its inputs.
    /// The first request looks before its flight and again inside it, the second joins the flight,
    /// so the three lookups miss; a third request is a hit and computes nothing.
    #[tokio::test]
    async fn two_requests_for_one_surface_at_once_compute_it_once() {
        let pool = one_worker();
        let (service, source) = service(&pool, 64 << 20);
        let key = nth(0);
        let (release, held) = hold(&pool).await;
        // `join3` polls in order: the first queues the pass behind the held worker, the second
        // joins its flight, and only then is the worker released.
        let (one, other, ()) = timeout(
            WAIT,
            join3(
                service.field(key, &system().0),
                service.field(key, &system().0),
                async {
                    release.send(()).unwrap();
                },
            ),
        )
        .await
        .expect("timed out computing the field");
        let (one, other) = (one.unwrap(), other.unwrap());
        assert!(Arc::ptr_eq(&one, &other), "one pass, shared");
        timeout(WAIT, held).await.unwrap().unwrap().unwrap();
        assert_eq!(source.calls(), 1);
        let counters = service.counters();
        assert_eq!(counters.computed(), 1, "{counters:?}");
        assert_eq!(
            (
                counters.cache().misses(),
                counters.cache().hits(),
                counters.cache().entries(),
                counters.cache().bytes()
            ),
            (3, 0, 1, FieldCache::charge(&one)),
            "{counters:?}"
        );
        let own = coarse_pass(
            surface_seed(Seed::new(SEED), key.body()),
            &reference::ceres_like(),
        );
        assert_same_bits(&one, &own, "the field is the sim's pass of the body's seed");

        let again = field(&service, key).await.unwrap();
        assert!(Arc::ptr_eq(&one, &again), "held");
        assert_eq!(
            (
                service.counters().computed(),
                service.counters().cache().hits(),
                source.calls()
            ),
            (1, 1, 1)
        );
        timeout(WAIT, pool.shutdown()).await.unwrap().unwrap();
    }

    /// The cache holds no more than its budget: with room for two fields, a third body's evicts
    /// the least recently used, which is computed again when it is asked for again.
    #[tokio::test]
    async fn surface_fields_past_the_budget_are_evicted() {
        let pool = one_worker();
        let (roomy, _) = service(&pool, 64 << 20);
        let charge = FieldCache::charge(&field(&roomy, nth(0)).await.unwrap());
        let budget = 2 * charge + charge / 2;
        let (service, source) = service(&pool, budget);
        let first = field(&service, nth(0)).await.unwrap();
        for n in 1..3 {
            field(&service, nth(n)).await.unwrap();
            let cache = service.counters().cache();
            assert!(
                cache.bytes() <= budget,
                "{} bytes held against {budget}",
                cache.bytes()
            );
        }
        let counters = service.counters();
        assert_eq!(
            (
                counters.computed(),
                counters.cache().entries(),
                counters.cache().evictions()
            ),
            (3, 2, 1),
            "{counters:?}"
        );
        // The first body's was the least recently used; computed again, it is the same field, bit
        // for bit.
        let again = field(&service, nth(0)).await.unwrap();
        assert_eq!(
            (service.counters().computed(), source.calls()),
            (4, 4),
            "evicted, and computed again"
        );
        assert!(!Arc::ptr_eq(&first, &again));
        assert_same_bits(&first, &again, "computed again");
        timeout(WAIT, pool.shutdown()).await.unwrap().unwrap();
    }

    /// A budget of nought keeps no field: each request computes its own.
    #[tokio::test]
    async fn a_surface_budget_of_nought_keeps_no_field() {
        let pool = one_worker();
        let (service, _) = service(&pool, 0);
        for computed in 1..=2 {
            field(&service, nth(0)).await.unwrap();
            let counters = service.counters();
            assert_eq!(
                (
                    counters.computed(),
                    counters.cache().entries(),
                    counters.cache().refused()
                ),
                (computed, 0, computed),
                "{counters:?}"
            );
        }
        timeout(WAIT, pool.shutdown()).await.unwrap().unwrap();
    }

    /// A pass every request has given up is given up too: its job, still queued, is skipped, the
    /// source is never called, and nothing is computed or kept.
    #[tokio::test]
    async fn a_cancelled_surface_pass_keeps_nothing() {
        let pool = one_worker();
        let (service, source) = service(&pool, 64 << 20);
        let (release, held) = hold(&pool).await;
        let mut asking = Box::pin(service.field(nth(0), &system().0));
        assert!(
            (&mut asking).now_or_never().is_none(),
            "the pass waits behind the held worker"
        );
        drop(asking);
        release.send(()).unwrap();
        timeout(WAIT, held).await.unwrap().unwrap().unwrap();
        drain(&pool).await;
        assert_eq!(pool.counters().cancelled(), 1, "{:?}", pool.counters());
        let counters = service.counters();
        assert_eq!(
            (
                counters.computed(),
                counters.cache().entries(),
                source.calls()
            ),
            (0, 0, 0),
            "{counters:?}"
        );
        timeout(WAIT, pool.shutdown()).await.unwrap().unwrap();
    }

    /// A source that blocks until released, so that a pass can be given up while it runs.
    #[derive(Debug)]
    struct Gated {
        entered: AtomicUsize,
        gate: Mutex<mpsc::Receiver<()>>,
    }

    impl InputsSource for Gated {
        fn inputs(
            &self,
            _: &BodyRecord,
            _: &SystemContext,
        ) -> Result<CoarseInputs, SurfaceInputsError> {
            self.entered.fetch_add(1, Ordering::SeqCst);
            self.gate
                .lock()
                .unwrap()
                .recv_timeout(WAIT)
                .expect("the test releases the pass");
            Ok(reference::ceres_like())
        }
    }

    /// A pass given up while it runs runs to its end, and its field is dropped, not kept: a
    /// cancelled request leaves nothing behind.
    #[tokio::test]
    async fn a_surface_pass_given_up_while_it_runs_keeps_nothing() {
        let pool = one_worker();
        let (release, gate) = mpsc::channel::<()>();
        let source = Arc::new(Gated {
            entered: AtomicUsize::new(0),
            gate: Mutex::new(gate),
        });
        let service = SurfaceService::new(
            Arc::clone(&pool),
            Arc::clone(&source) as Arc<dyn InputsSource>,
            64 << 20,
        );
        let mut asking = Box::pin(service.field(nth(0), &system().0));
        assert!((&mut asking).now_or_never().is_none());
        until("the pass is running", || {
            source.entered.load(Ordering::SeqCst) == 1
        })
        .await;
        drop(asking);
        release.send(()).unwrap();
        drain(&pool).await;
        let counters = service.counters();
        assert_eq!(
            (counters.computed(), counters.cache().entries()),
            (0, 0),
            "{counters:?}"
        );
        assert!(service.flights.is_empty());
        timeout(WAIT, pool.shutdown()).await.unwrap().unwrap();
    }

    /// A pass started by a prefetch, with no request waiting, is joined by a request made
    /// meanwhile: one pass between them. Once the field is held, a prefetch starts nothing and
    /// counts a hit.
    #[tokio::test]
    async fn a_surface_prefetch_is_joined_by_a_request() {
        let pool = one_worker();
        let (service, source) = service(&pool, 64 << 20);
        let key = nth(0);
        let (release, held) = hold(&pool).await;
        assert!(
            service.prefetch(key, &system().0),
            "the prefetch starts the pass"
        );
        until(
            "the prefetch's pass is queued behind the held worker",
            || {
                let counters = pool.counters();
                (counters.running(), counters.queued_bulk()) == (1, 1)
            },
        )
        .await;
        let mut asking = Box::pin(service.field(key, &system().0));
        assert!((&mut asking).now_or_never().is_none());
        release.send(()).unwrap();
        let first = timeout(WAIT, asking).await.unwrap().unwrap();
        timeout(WAIT, held).await.unwrap().unwrap().unwrap();
        let counters = service.counters();
        assert_eq!(
            (
                counters.computed(),
                counters.cache().misses(),
                source.calls()
            ),
            (1, 3, 1),
            "{counters:?}"
        );
        assert!(!service.prefetch(key, &system().0), "held: nothing starts");
        assert_eq!(service.counters().cache().hits(), 1);
        let again = field(&service, key).await.unwrap();
        assert!(Arc::ptr_eq(&first, &again));
        timeout(WAIT, pool.shutdown()).await.unwrap().unwrap();
    }

    /// A pass started by a prefetch runs to its end and is kept with no request ever made.
    #[tokio::test]
    async fn a_surface_prefetch_needs_no_request() {
        let pool = one_worker();
        let (service, _) = service(&pool, 64 << 20);
        assert!(service.prefetch(nth(1), &system().0));
        until("the field is held", || {
            service.counters().cache().entries() == 1
        })
        .await;
        assert_eq!(service.counters().computed(), 1);
        timeout(WAIT, pool.shutdown()).await.unwrap().unwrap();
    }

    /// The passes prefetches started are given up when the service stops prefetching, as at the
    /// server's shutdown: a queued one is skipped, and nothing is computed or kept.
    #[tokio::test]
    async fn surface_prefetches_are_given_up_at_shutdown() {
        let pool = one_worker();
        let (service, source) = service(&pool, 64 << 20);
        let (release, held) = hold(&pool).await;
        assert!(service.prefetch(nth(0), &system().0));
        until(
            "the prefetch's pass is queued behind the held worker",
            || {
                let counters = pool.counters();
                (counters.running(), counters.queued_bulk()) == (1, 1)
            },
        )
        .await;
        service.stop_prefetching();
        until("the pass is given up", || service.flights.is_empty()).await;
        release.send(()).unwrap();
        timeout(WAIT, held).await.unwrap().unwrap().unwrap();
        drain(&pool).await;
        assert_eq!(pool.counters().cancelled(), 1, "{:?}", pool.counters());
        let counters = service.counters();
        assert_eq!(
            (
                counters.computed(),
                counters.cache().entries(),
                source.calls()
            ),
            (0, 0, 0),
            "{counters:?}"
        );
        timeout(WAIT, pool.shutdown()).await.unwrap().unwrap();
    }

    /// Passes for its worker's start, then flushes: the probe after the first job fails.
    fn flushes_after_the_first_job() -> FlushProbe {
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        if CALLS.fetch_add(1, Ordering::SeqCst) == 0 {
            FlushProbe::KEEPS_SUBNORMALS
        } else {
            FlushProbe::FLUSHES_OUTPUTS
        }
    }

    /// A pass on a worker whose floating-point mode is found to flush subnormals is withheld, and
    /// nothing is kept; a request answers it `internal`, as every job's refusal of the kind. The
    /// next request finds the pool faulted, and is refused the same way.
    #[tokio::test]
    async fn a_surface_pass_on_a_flushing_worker_is_refused() {
        let pool = Arc::new(
            CpuPool::with_probe(
                NonZeroUsize::MIN,
                INTERACTIVE_QUEUE_CAPACITY,
                BULK_QUEUE_CAPACITY,
                flushes_after_the_first_job,
            )
            .unwrap(),
        );
        let (service, _) = service(&pool, 64 << 20);
        let withheld = ComputeError::Job(JobError::FloatingPointMode);
        assert_eq!(
            field(&service, nth(0)).await.unwrap_err(),
            SurfaceJobError::Compute(withheld)
        );
        assert_eq!(
            field(&service, nth(0)).await.unwrap_err(),
            SurfaceJobError::Compute(ComputeError::Submit(SubmitJobError::Faulted))
        );
        let counters = service.counters();
        assert_eq!(
            (counters.computed(), counters.cache().entries()),
            (0, 0),
            "{counters:?}"
        );
        let refusal = RequestError::from(withheld);
        assert_eq!(refusal.code, ErrorCode::Internal);
        assert!(
            refusal.message.contains("flushes subnormals"),
            "{}",
            refusal.message
        );
        timeout(WAIT, pool.shutdown()).await.unwrap().unwrap();
    }

    /// A key whose body index is not in plan 14's layout, or names no body of the system (the
    /// primary star's index among them), is refused as the body's, and the source is never called.
    #[tokio::test]
    async fn a_surface_of_no_body_is_refused() {
        let pool = one_worker();
        let (service, source) = service(&pool, 64 << 20);
        let planets = &system().0.1;
        let system_id = planets.system();
        assert_eq!(
            field(&service, key(BodyId::new(system_id, 0xf000)))
                .await
                .unwrap_err(),
            SurfaceJobError::Body(ResolveBodyError::MalformedIndex(
                DecodeBodyIndexError::ReservedSlot { raw: 0xf000 }
            ))
        );
        let unused = (0x0100..0x1000_u16)
            .step_by(0x100)
            .find(|&raw| BodyIndex::try_from(raw).is_ok_and(|index| planets.body(index).is_none()))
            .expect("a planet slot the system leaves empty");
        for raw in [0, unused] {
            assert_eq!(
                field(&service, key(BodyId::new(system_id, raw)))
                    .await
                    .unwrap_err(),
                SurfaceJobError::Body(ResolveBodyError::NoSuchBody),
                "{raw:#06x}"
            );
        }
        assert_eq!((service.counters().computed(), source.calls()), (0, 0));
        timeout(WAIT, pool.shutdown()).await.unwrap().unwrap();
    }

    /// Each error says what failed and keeps its cause.
    #[test]
    fn a_surface_job_error_keeps_its_cause() {
        let inputs = SurfaceJobError::Inputs(SurfaceInputsError::NoSolidSurface);
        assert_eq!(
            inputs.to_string(),
            "the body's coarse-pass inputs cannot be had"
        );
        assert_eq!(
            inputs.source().map(ToString::to_string).as_deref(),
            Some("the body has no solid surface")
        );
        let body = SurfaceJobError::Body(ResolveBodyError::NoSuchBody);
        assert_eq!(body.to_string(), "the body does not resolve");
        assert_eq!(
            body.source().map(ToString::to_string).as_deref(),
            Some("the system holds no such body")
        );
        let compute = SurfaceJobError::from(ComputeError::Job(JobError::Cancelled));
        assert_eq!(compute.to_string(), "the body's coarse pass did not run");
        assert_eq!(
            compute.source().map(ToString::to_string).as_deref(),
            Some("the computation did not finish")
        );
    }

    /// Two first asks for a universe's survey log at once share one open, so that its file has one
    /// writer: a pass recorded through either is the other's too, and a later ask is the same log.
    /// A universe with no survey file yet gets a log with no coverage, and nothing is written.
    #[tokio::test]
    async fn two_first_asks_for_a_survey_log_open_it_once() {
        use crate::knowledge::surveys::{SurveyPass, SurveyPassParts, SurveySource};
        use crate::universe::{SavedUniverse, UniverseId, UniverseStore};

        let dir = tempfile::tempdir().unwrap();
        let store = UniverseStore::new(dir.path());
        let universe = UniverseId::new(0xab);
        store
            .write(&SavedUniverse::new(
                universe,
                "Kepler Reach".parse().unwrap(),
                SEED,
                GENERATOR_VERSION,
            ))
            .unwrap();
        let pool = one_worker();
        let (service, _) = service(&pool, 0);
        let (one, other) = timeout(
            WAIT,
            futures_util::future::join(
                service.survey_log(&store, universe),
                service.survey_log(&store, universe),
            ),
        )
        .await
        .expect("timed out opening the survey log");
        let (one, other) = (one.unwrap(), other.unwrap());
        let body = system().1[0];
        assert_eq!(other.coverage(body), None);
        assert!(!store.knowledge_dir(universe).exists(), "nothing written");

        let level = hyperion_surface::field::coarse_level(reference::ceres_like().radius());
        let pass = SurveyPass::new(SurveyPassParts {
            body,
            from: UniverseTime::EPOCH,
            to: UniverseTime::EPOCH,
            source: SurveySource::Orbital,
            resolution: hyperion_sim::units::Metres::new(500.0),
            level,
            cells: std::iter::once(0..6).collect(),
        })
        .unwrap();
        assert_eq!(one.record(pass).await.unwrap().get(), 1);
        let later = service.survey_log(&store, universe).await.unwrap();
        for log in [&other, &later] {
            let coverage = log.coverage(body).expect("the pass is the shared log's");
            assert_eq!((coverage.revision().get(), coverage.surveyed()), (1, 6));
        }
        timeout(WAIT, pool.shutdown()).await.unwrap().unwrap();
    }

    /// A body's level: its inputs' where the source gives them; where it answers a section not
    /// modelled, the figure's, or a refusal by the figure's state; the source's other refusals
    /// as they are.
    #[test]
    fn a_field_s_level_comes_from_the_inputs_or_else_the_figure() {
        let ceres = coarse_level(reference::ceres_like().radius());
        let not_modelled = || Err(SurfaceInputsError::NotModelled(RecordSection::Surface));
        assert_eq!(
            level_from(Ok(reference::ceres_like()), &Section::NotModelled),
            Ok(FieldLevel::Modelled(ceres))
        );
        for (figure, refusal) in [
            (
                Section::NotModelled,
                SurfaceInputsError::NotModelled(RecordSection::Figure),
            ),
            (
                Section::NotResolved,
                SurfaceInputsError::NotResolved(RecordSection::Figure),
            ),
            (Section::NotApplicable, SurfaceInputsError::NoSolidSurface),
        ] {
            assert_eq!(
                level_from(not_modelled(), &figure),
                Err(SurfaceJobError::Inputs(refusal)),
                "{figure:?}"
            );
        }
        for refusal in [
            SurfaceInputsError::NoSolidSurface,
            SurfaceInputsError::NotResolved(RecordSection::Bulk),
        ] {
            assert_eq!(
                level_from(Err(refusal), &Section::NotModelled),
                Err(SurfaceJobError::Inputs(refusal))
            );
        }
        // A figure that is modelled: the first body of `system` with one.
        let (ctx, planets) = system().0.as_ref();
        let record = system()
            .1
            .iter()
            .find_map(|body| {
                let index = BodyIndex::try_from(body.body_index()).ok()?;
                let record = planets.body_at(ctx, index, RECORD_TIME).ok()?;
                matches!(record.figure(), Section::Ok(_)).then_some(record)
            })
            .expect("a body of the system has a figure");
        let Section::Ok(figure) = record.figure() else {
            panic!("the record found has a figure");
        };
        let level = coarse_level(Metres::new(figure.spheroid().volumetric_radius_m()));
        assert_eq!(
            level_from(not_modelled(), record.figure()),
            Ok(FieldLevel::NotModelled {
                level,
                section: RecordSection::Surface,
            })
        );
    }

    /// A field of a generator version this build does not generate is refused.
    #[tokio::test]
    #[should_panic(expected = "which this build cannot generate")]
    async fn another_versions_surface_is_refused() {
        let pool = one_worker();
        let (service, _) = service(&pool, 64 << 20);
        let later = hyperion_sim::GeneratorVersion::new(GENERATOR_VERSION.get() + 1);
        let key = FieldKey::new(GalaxyKey::new(SEED, later), system().1[0]);
        drop(service.field(key, &system().0).await);
    }

    /// A field is kept under its own body's key alone: another system's is refused.
    #[tokio::test]
    #[should_panic(expected = "computed from its own system")]
    async fn another_systems_surface_is_refused() {
        let pool = one_worker();
        let (service, _) = service(&pool, 64 << 20);
        let other = SystemId::from_raw(0x0200_0800_2000_0000).unwrap();
        assert_ne!(other, system().0.1.system());
        drop(
            service
                .field(key(BodyId::new(other, 0x0100)), &system().0)
                .await,
        );
    }
}
