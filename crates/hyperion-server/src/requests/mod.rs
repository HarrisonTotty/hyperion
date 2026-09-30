//! Requests: reading them from frames, the in-flight table of one connection, and the handlers
//! that answer each kind.
//!
//! The convention is plan 04's design notes 1–5 and 7. A client names each request with its own
//! [`RequestId`], and every request the server accepts ends in exactly one terminal message with
//! that ID: a `response`, or a `request_error`, `cancelled` included. [`Requests`] is the one place
//! that decides which, because the connection task that owns it is the only sender of terminal
//! messages: a request's task produces its terminal frame and hands it back through a
//! [`JoinSet`], and the connection pushes it to the writer, unless the request was cancelled
//! first, in which case the frame is dropped. A request refused on arrival is answered at once and
//! never runs.
//!
//! [`Handler`] is the seam where each kind's handler plugs in (plan 04, P04.T14). The server's is
//! [`Handlers`]; unit tests inject doubles through [`AppState`]. The handlers of the universe
//! lifecycle are in [`universe`], the galaxy's in [`galaxy`], and the system's in [`system`].

mod galaxy;
mod system;
mod universe;

use std::collections::HashMap;
use std::fmt;
use std::future::ready;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::future::BoxFuture;
use hyperion_protocol::{
    ClientMessage, ErrorCode, REQUEST_KINDS, RequestBody, RequestError, RequestId, ResponseBody,
    ServerMessage,
};
use serde_json::Value;
use tokio::task::{AbortHandle, Id as TaskId, JoinError, JoinSet};
use tracing::Instrument;

use crate::AppState;
use crate::compute::{
    CancelToken, ComputeError, JobError, Priority, SubmitJobError, panic_message,
};
use crate::limits::MAX_IN_FLIGHT_REQUESTS;
use crate::stats::Ending;

/// What a handler returns: the response's body, or why there is none.
pub(crate) type HandlerFuture = BoxFuture<'static, Result<ResponseBody, RequestError>>;

/// Answers requests.
pub(crate) trait Handler: fmt::Debug + Send + Sync {
    /// Answers `body`.
    ///
    /// The future runs on a task of its own and is dropped at an `.await` if the client cancels
    /// the request or its connection closes; `token` is cancelled first. Work handed to the CPU
    /// pool should carry `token`, so that a job still queued when the request is cancelled is
    /// skipped. A panic costs this request alone, which is answered `internal`.
    fn handle(&self, state: Arc<AppState>, body: RequestBody, token: CancelToken) -> HandlerFuture;
}

/// The server's handlers: every request kind, and the code that answers it.
///
/// Every kind of the first milestone is served (plan 04, P04.T14), plan 06's `system_summary`
/// (P06.T34), and plan 14's `system_bodies` and `body_detail` (P14.T36). A later plan's kind that
/// this server's [`REQUEST_KINDS`] does not hold is refused before it reaches here, as
/// `unsupported`. A kind the protocol already defines but whose handler has not landed is answered
/// `unsupported` here, under its own ID, as an older server would answer it (plan 04, design note
/// 15): `body_events` until plan 14's P14.T31. Each handler that names a universe starts from
/// [`universe::openable_universe`].
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Handlers;

impl Handler for Handlers {
    fn handle(&self, state: Arc<AppState>, body: RequestBody, token: CancelToken) -> HandlerFuture {
        match body {
            RequestBody::CreateUniverse(request) => Box::pin(universe::create(state, request)),
            RequestBody::ListUniverses => Box::pin(ready(Ok(universe::list(&state)))),
            RequestBody::OpenUniverse(request) => Box::pin(universe::open(state, request)),
            RequestBody::GalaxyParameters(request) => Box::pin(galaxy::parameters(state, request)),
            RequestBody::DensityMap(request) => Box::pin(galaxy::map(state, request, token)),
            RequestBody::SystemsInRange(request) => {
                Box::pin(galaxy::systems(state, request, token))
            }
            RequestBody::SystemSummary(request) => Box::pin(system::summary(state, request, token)),
            RequestBody::SystemBodies(request) => Box::pin(system::bodies(state, request, token)),
            RequestBody::BodyDetail(request) => Box::pin(system::detail(state, request, token)),
            RequestBody::BodyEvents(_) => Box::pin(ready(Err(not_served_yet("body_events")))),
        }
    }
}

/// The answer to a kind the protocol defines and this server does not serve yet: `unsupported`,
/// as an older server would answer it (plan 04, design note 15). Plan 14's `body_events` takes it
/// until P14.T31 lands.
#[must_use]
fn not_served_yet(kind: &str) -> RequestError {
    request_error(
        ErrorCode::Unsupported,
        format!("request kind `{kind}` is not served by this server yet"),
    )
}

/// A request's `kind` string, as it is on the wire and in [`REQUEST_KINDS`].
pub(crate) fn kind(body: &RequestBody) -> &'static str {
    match body {
        RequestBody::CreateUniverse(_) => "create_universe",
        RequestBody::ListUniverses => "list_universes",
        RequestBody::OpenUniverse(_) => "open_universe",
        RequestBody::GalaxyParameters(_) => "galaxy_parameters",
        RequestBody::DensityMap(_) => "density_map",
        RequestBody::SystemsInRange(_) => "systems_in_range",
        RequestBody::SystemSummary(_) => "system_summary",
        RequestBody::SystemBodies(_) => "system_bodies",
        RequestBody::BodyDetail(_) => "body_detail",
        RequestBody::BodyEvents(_) => "body_events",
    }
}

/// Whether a response can run to megabytes of JSON, and so is serialised on the CPU pool rather
/// than on the runtime (design note 22). A new kind must say which it is.
///
/// A system's bodies can: its belts' named members (plan 14, P14.T21) run to 255 a belt, each a
/// record. So can a window of body events, whose comets carry sampled tracks (P14.T31). One body's
/// record cannot.
fn is_large(body: &ResponseBody) -> bool {
    match body {
        ResponseBody::DensityMap(_)
        | ResponseBody::SystemsInRange(_)
        | ResponseBody::SystemBodies(_)
        | ResponseBody::BodyEvents(_) => true,
        ResponseBody::CreateUniverse(_)
        | ResponseBody::ListUniverses(_)
        | ResponseBody::OpenUniverse(_)
        | ResponseBody::GalaxyParameters(_)
        | ResponseBody::SystemSummary(_)
        | ResponseBody::BodyDetail(_) => false,
    }
}

/// A [`RequestError`] naming no field.
pub(crate) fn request_error(code: ErrorCode, message: impl Into<String>) -> RequestError {
    RequestError {
        code,
        message: message.into(),
        field: None,
    }
}

impl From<SubmitJobError> for RequestError {
    /// A full interactive queue is the client's to retry (`queue_full`); a pool shutting down is
    /// the server's affair (`internal`).
    fn from(error: SubmitJobError) -> Self {
        match error {
            SubmitJobError::QueueFull => request_error(
                ErrorCode::QueueFull,
                "the server is busy; try again shortly",
            ),
            SubmitJobError::ShutDown => {
                request_error(ErrorCode::Internal, "the server is shutting down")
            }
        }
    }
}

impl From<JobError> for RequestError {
    fn from(error: JobError) -> Self {
        match error {
            JobError::Cancelled => request_error(ErrorCode::Cancelled, "the request was cancelled"),
            JobError::Panicked => request_error(
                ErrorCode::Internal,
                "the server failed while answering this request",
            ),
            JobError::ShutDown => request_error(ErrorCode::Internal, "the server is shutting down"),
        }
    }
}

impl From<ComputeError> for RequestError {
    /// A cached computation fails as its pool job did, so each of those causes keeps the code it has
    /// as a job's. A galaxy whose systems cannot be numbered is the server's own affair
    /// (`internal`): the universe cannot be served at all, and the message says why, since no
    /// error code stands for "this universe is unplayable" (design note 15 keeps the codes fixed).
    fn from(error: ComputeError) -> Self {
        match error {
            ComputeError::Submit(error) => error.into(),
            ComputeError::Job(error) => error.into(),
            ComputeError::UnplayableGalaxy(cause) => request_error(
                ErrorCode::Internal,
                format!("this universe cannot be generated: {cause}"),
            ),
        }
    }
}

/// Serialises a message into the text of a frame.
pub(crate) fn to_frame(message: &ServerMessage) -> String {
    serde_json::to_string(message)
        .expect("a server message always serialises: it holds no map with non-string keys")
}

/// A text frame from a client, read.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Inbound {
    /// A well-formed message.
    Message(ClientMessage),
    /// A request that does not parse but whose ID can be read, answered with a `request_error`
    /// of its own (design note 4).
    Unparsed {
        /// The request's ID.
        id: RequestId,
        /// `unsupported` for a kind not in [`REQUEST_KINDS`], `bad_request` otherwise.
        error: RequestError,
    },
    /// A frame that is not a message and names no request, answered with the connection-level
    /// `error`.
    Malformed {
        /// What was wrong, for the client's log.
        message: String,
    },
}

impl Inbound {
    /// How the frame bears on the count of malformed frames in a row, which closes the connection
    /// at [`MAX_CONSECUTIVE_MALFORMED_FRAMES`](crate::limits::MAX_CONSECUTIVE_MALFORMED_FRAMES).
    pub(crate) fn tally(&self) -> Tally {
        match self {
            Self::Message(_) => Tally::Reset,
            // A newer client may send a kind this server does not know in good faith.
            Self::Unparsed { error, .. } if error.code == ErrorCode::Unsupported => Tally::Leave,
            Self::Unparsed { .. } | Self::Malformed { .. } => Tally::Count,
        }
    }
}

/// What a frame does to the count of malformed frames in a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Tally {
    /// A well-formed message: the count starts again from zero.
    Reset,
    /// A malformed frame: it counts.
    Count,
    /// A request of a kind this server does not know: the count stays as it is.
    Leave,
}

/// Reads a text frame. What fails to parse as a [`ClientMessage`] is read again by the lenient
/// probe of design note 4, so that a request with a readable ID is answered under that ID.
pub(crate) fn parse(text: &str) -> Inbound {
    let error = match serde_json::from_str::<ClientMessage>(text) {
        Ok(message) => return Inbound::Message(message),
        Err(error) => error,
    };
    match probe(text) {
        Some(Probe { id, kind }) => Inbound::Unparsed {
            id,
            error: match kind {
                Some(kind) if !REQUEST_KINDS.contains(&kind.as_str()) => request_error(
                    ErrorCode::Unsupported,
                    format!("unsupported request kind `{kind}`"),
                ),
                Some(_) | None => {
                    request_error(ErrorCode::BadRequest, format!("malformed request: {error}"))
                }
            },
        },
        None => Inbound::Malformed {
            message: format!("malformed message: {error}"),
        },
    }
}

/// What the lenient probe reads of a request frame: its ID, and its `body.kind` if that is a
/// string.
struct Probe {
    id: RequestId,
    kind: Option<String>,
}

/// Reads `type`, `id` and `body.kind` from any JSON object, whatever else it holds. `None` unless
/// the frame is a `request` whose `id` is a whole number that fits a [`RequestId`].
fn probe(text: &str) -> Option<Probe> {
    let value: Value = serde_json::from_str(text).ok()?;
    if value.get("type")?.as_str()? != "request" {
        return None;
    }
    let id = u32::try_from(value.get("id")?.as_u64()?).ok()?;
    let kind = value
        .get("body")
        .and_then(|body| body.get("kind"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    Some(Probe {
        id: RequestId(id),
        kind,
    })
}

/// Whether a connection's client has sent `hello`, before which requests are refused (design
/// note 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Handshake {
    /// No `hello` yet.
    Pending,
    /// `hello` received and answered.
    Done,
}

/// A request's terminal frame, as its task made it.
#[derive(Debug)]
pub(crate) struct Finished {
    frame: String,
    ending: Ending,
}

/// A request whose task has ended, with its terminal frame, not yet queued.
///
/// The request stays in flight until [`Requests::end`], so that a `cancel` arriving while the
/// connection holds the frame back still ends it (plan 04, P04.T15).
#[derive(Debug)]
pub(crate) struct Settled {
    id: RequestId,
    task: TaskId,
    frame: String,
    ending: Ending,
}

impl Settled {
    /// The payload bytes of the terminal frame.
    #[must_use]
    pub(crate) fn frame_len(&self) -> usize {
        self.frame.len()
    }
}

/// One request in flight.
#[derive(Debug)]
struct InFlight {
    kind: &'static str,
    token: CancelToken,
    task: AbortHandle,
    accepted: Instant,
}

/// One connection's requests in flight, each run by a task in a [`JoinSet`].
///
/// Owned by the connection task, so it needs no lock. Its methods return the frame to send, if
/// any, and the connection pushes it to the writer in the order it was returned, except that a
/// finished request's terminal frame may wait in [`Held`](crate::outbound::Held) for room in the
/// outbound queue, while later answers go ahead of it.
#[derive(Debug)]
pub(crate) struct Requests {
    state: Arc<AppState>,
    in_flight: HashMap<RequestId, InFlight>,
    tasks: JoinSet<Finished>,
}

impl Requests {
    pub(crate) fn new(state: Arc<AppState>) -> Self {
        Self {
            state,
            in_flight: HashMap::with_capacity(MAX_IN_FLIGHT_REQUESTS),
            tasks: JoinSet::new(),
        }
    }

    /// Accepts a request and starts its task, or refuses it and returns the answer: the
    /// connection-level `error` for an ID already in flight (design note 2), `hello_required`
    /// before `hello` (7), and `too_many_requests` beyond [`MAX_IN_FLIGHT_REQUESTS`] (24).
    pub(crate) fn submit(
        &mut self,
        id: RequestId,
        body: RequestBody,
        handshake: Handshake,
    ) -> Option<String> {
        let kind = kind(&body);
        if let Some(refusal) = self.refuse_early(id, kind, handshake) {
            return Some(refusal);
        }
        if self.in_flight.len() >= MAX_IN_FLIGHT_REQUESTS {
            return Some(self.refuse(
                id,
                kind,
                request_error(
                    ErrorCode::TooManyRequests,
                    format!("at most {MAX_IN_FLIGHT_REQUESTS} requests may be in flight at once"),
                ),
            ));
        }
        let token = CancelToken::new();
        // Counted before the task starts, so that a handler sees its request counted.
        self.state.request_stats.accepted();
        let span = tracing::debug_span!("request", id = id.0, kind);
        let task = self
            .tasks
            .spawn(run(Arc::clone(&self.state), id, body, token.clone()).instrument(span));
        self.in_flight.insert(
            id,
            InFlight {
                kind,
                token,
                task,
                accepted: Instant::now(),
            },
        );
        None
    }

    /// Answers a request that did not parse, with `error` unless it is refused as a request that
    /// parsed would be.
    pub(crate) fn reject(
        &self,
        id: RequestId,
        error: RequestError,
        handshake: Handshake,
    ) -> String {
        self.refuse_early(id, "unparsed", handshake)
            .unwrap_or_else(|| self.refuse(id, "unparsed", error))
    }

    /// Cancels a request in flight and returns its terminal `cancelled`, or `None` if it is not
    /// in flight: it has ended already, and its terminal message has been sent.
    ///
    /// The request's place is free at once. Its task stops at its next `.await`, and a pool job it
    /// had queued is skipped; one a worker holds runs to the end and its result is dropped
    /// (design note 5).
    pub(crate) fn cancel(&mut self, id: RequestId) -> Option<String> {
        let Some(entry) = self.in_flight.remove(&id) else {
            tracing::debug!(id = id.0, "ignored a cancel for a request not in flight");
            return None;
        };
        entry.token.cancel();
        entry.task.abort();
        self.state.request_stats.ended(Ending::Cancelled);
        tracing::debug!(
            id = id.0,
            kind = entry.kind,
            elapsed_ms = millis(entry.accepted.elapsed()),
            "request cancelled"
        );
        Some(to_frame(&ServerMessage::RequestError {
            id,
            error: request_error(ErrorCode::Cancelled, "cancelled by the client"),
        }))
    }

    /// Whether any request task has yet to be joined.
    pub(crate) fn has_tasks(&self) -> bool {
        !self.tasks.is_empty()
    }

    /// Waits for the next request task to end. Cancellation-safe.
    pub(crate) async fn join_next(&mut self) -> Option<Result<(TaskId, Finished), JoinError>> {
        self.tasks.join_next_with_id().await
    }

    /// Takes the terminal frame of a request whose task has ended, or `None` if the request was
    /// cancelled first and has been answered already.
    ///
    /// The request stays in flight until [`Requests::end`] queues its frame.
    ///
    /// A task that panicked, whether in its handler or in a computation it shared (a
    /// [`SingleFlight`](crate::compute::SingleFlight) passes a panic on to every waiter), is
    /// answered `internal`: it costs that request and nothing else.
    #[must_use]
    pub(crate) fn settle(&self, joined: Result<(TaskId, Finished), JoinError>) -> Option<Settled> {
        let task = match &joined {
            Ok((task, _)) => *task,
            Err(error) => error.id(),
        };
        // A cancelled request's ID may already be in use again, so the task, not the ID, says
        // which request this was.
        let (&id, entry) = self
            .in_flight
            .iter()
            .find(|(_, entry)| entry.task.id() == task)?;
        let (frame, ending) = match joined {
            Ok((_, Finished { frame, ending })) => (frame, ending),
            Err(error) => {
                let reason = match error.try_into_panic() {
                    Ok(payload) => panic_message(payload.as_ref()),
                    Err(_) => "its task was cancelled by the runtime".to_owned(),
                };
                tracing::error!(id = id.0, kind = entry.kind, %reason, "a request failed");
                let frame = to_frame(&ServerMessage::RequestError {
                    id,
                    error: request_error(
                        ErrorCode::Internal,
                        "the server failed while answering this request",
                    ),
                });
                (frame, Ending::Failed)
            }
        };
        Some(Settled {
            id,
            task,
            frame,
            ending,
        })
    }

    /// Whether a settled request is still in flight: no `cancel` has ended it since it settled.
    #[must_use]
    pub(crate) fn is_in_flight(&self, settled: &Settled) -> bool {
        self.in_flight
            .get(&settled.id)
            .is_some_and(|entry| entry.task.id() == settled.task)
    }

    /// Ends a settled request and returns its terminal frame to queue, or `None` if a `cancel`
    /// has ended it meanwhile.
    #[must_use]
    pub(crate) fn end(&mut self, settled: Settled) -> Option<String> {
        if !self.is_in_flight(&settled) {
            return None;
        }
        self.in_flight.remove(&settled.id);
        self.state.request_stats.ended(settled.ending);
        Some(settled.frame)
    }

    /// Cancels every request in flight and waits for their tasks to stop, for a connection that
    /// is closing. The requests end with no message, since there is no one left to send one to.
    pub(crate) async fn close(&mut self) {
        for (id, entry) in self.in_flight.drain() {
            entry.token.cancel();
            self.state.request_stats.ended(Ending::Abandoned);
            tracing::debug!(
                id = id.0,
                kind = entry.kind,
                "request abandoned: its connection closed"
            );
        }
        self.tasks.shutdown().await;
    }

    /// The refusals shared by requests that parsed and those that did not: an ID in flight, and
    /// a request before `hello`.
    fn refuse_early(&self, id: RequestId, kind: &str, handshake: Handshake) -> Option<String> {
        if self.in_flight.contains_key(&id) {
            // A `request_error` for this ID would end the request already using it (design note
            // 2), so the frame is dropped and answered at the connection's level.
            tracing::warn!(
                id = id.0,
                kind,
                "refused a request whose id is already in flight"
            );
            self.state.request_stats.refused();
            return Some(to_frame(&ServerMessage::Error {
                message: format!(
                    "request {} is already in flight; the new request was dropped",
                    id.0
                ),
            }));
        }
        match handshake {
            Handshake::Done => None,
            Handshake::Pending => Some(self.refuse(
                id,
                kind,
                request_error(ErrorCode::HelloRequired, "send hello before any request"),
            )),
        }
    }

    /// Refuses a request with `error`.
    fn refuse(&self, id: RequestId, kind: &str, error: RequestError) -> String {
        tracing::warn!(id = id.0, kind, code = ?error.code, message = %error.message, "refused a request");
        self.state.request_stats.refused();
        to_frame(&ServerMessage::RequestError { id, error })
    }
}

impl Drop for Requests {
    /// Cancels what is still in flight, so that queued pool jobs are skipped even if the
    /// connection task is dropped without closing (the runtime shutting down). The [`JoinSet`]
    /// aborts the tasks themselves.
    fn drop(&mut self) {
        for entry in self.in_flight.values() {
            entry.token.cancel();
        }
    }
}

/// A request's task: runs its handler and makes its terminal frame.
async fn run(
    state: Arc<AppState>,
    id: RequestId,
    body: RequestBody,
    token: CancelToken,
) -> Finished {
    let started = Instant::now();
    let handled = state
        .handler
        .handle(Arc::clone(&state), body, token.clone())
        .await;
    let answered = match handled {
        Ok(body) => respond(&state, id, body, token).await,
        Err(error) => Err(error),
    };
    let elapsed_ms = millis(started.elapsed());
    match answered {
        Ok(frame) => {
            tracing::debug!(outcome = "response", elapsed_ms, "request finished");
            Finished {
                frame,
                ending: Ending::Responded,
            }
        }
        Err(error) => {
            tracing::debug!(outcome = ?error.code, message = %error.message, elapsed_ms, "request finished");
            let ending = if error.code == ErrorCode::Cancelled {
                Ending::Cancelled
            } else {
                Ending::Failed
            };
            Finished {
                frame: to_frame(&ServerMessage::RequestError { id, error }),
                ending,
            }
        }
    }
}

/// The frame of a successful response. A large one is serialised on the CPU pool (design note
/// 22). It waits for a place in the interactive queue rather than failing with `queue_full`,
/// since the response exists already and refusing it now would waste the work.
async fn respond(
    state: &AppState,
    id: RequestId,
    body: ResponseBody,
    token: CancelToken,
) -> Result<String, RequestError> {
    let large = is_large(&body);
    let message = ServerMessage::Response { id, body };
    if !large {
        return Ok(to_frame(&message));
    }
    let receiver = state
        .pool
        .submit(Priority::Interactive, token, move |_| to_frame(&message))
        .await?;
    let frame = receiver
        .await
        .unwrap_or_else(|closed| Err(JobError::from(closed)))?;
    Ok(frame)
}

/// A duration in milliseconds, for logs.
fn millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1e3
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use hyperion_protocol::{
        BodyDetailRequest, BodyEventsRequest, BodyIdHex, CreateUniverseRequest, DensityMap,
        DensityMapRequest, DetailLevelDto, GalacticPosition, GalaxyParametersRequest,
        MapPopulation, MapView, MassLayer, OpenUniverseRequest, SystemBodiesRequest, SystemIdHex,
        SystemSummaryRequest, SystemsInRangeRequest, UniverseIdHex, UniverseTime,
    };

    use super::*;
    use crate::compute::SingleFlight;
    use crate::testing::{Call, Harness, Scripted, body, small_response};

    /// A request body of every kind.
    fn every_body() -> Vec<RequestBody> {
        let universe = UniverseIdHex::from_u64(42);
        vec![
            RequestBody::CreateUniverse(CreateUniverseRequest {
                name: "Talos".to_owned(),
                seed: None,
            }),
            RequestBody::ListUniverses,
            RequestBody::OpenUniverse(OpenUniverseRequest {
                universe: universe.clone(),
            }),
            RequestBody::GalaxyParameters(GalaxyParametersRequest {
                universe: universe.clone(),
            }),
            RequestBody::DensityMap(DensityMapRequest {
                universe: universe.clone(),
                view: MapView::FaceOn,
                population: MapPopulation::All,
                resolution: 128,
                bits: 8,
            }),
            RequestBody::SystemsInRange(SystemsInRangeRequest {
                universe: universe.clone(),
                centre: GalacticPosition::default(),
                radius_ly: 50.0,
                time: UniverseTime::default(),
                min_layer: MassLayer::A,
                limit: 5_000,
                include_stellar: false,
            }),
            RequestBody::SystemSummary(SystemSummaryRequest {
                universe: universe.clone(),
                system: SystemIdHex::from_u64(0x0200_0800_2000_0000),
                time: UniverseTime::default(),
            }),
            RequestBody::SystemBodies(SystemBodiesRequest {
                universe: universe.clone(),
                system: SystemIdHex::from_u64(0x0200_0800_2000_0000),
                time: UniverseTime::default(),
                detail: DetailLevelDto::Full,
            }),
            RequestBody::BodyDetail(BodyDetailRequest {
                universe: universe.clone(),
                body: BodyIdHex::from_parts(0x0200_0800_2000_0000, 0x0100),
                time: UniverseTime::default(),
                detail: DetailLevelDto::Full,
            }),
            RequestBody::BodyEvents(BodyEventsRequest {
                universe,
                system: SystemIdHex::from_u64(0x0200_0800_2000_0000),
                from: UniverseTime::default(),
                to: UniverseTime::default(),
            }),
        ]
    }

    fn density_map() -> ResponseBody {
        ResponseBody::DensityMap(DensityMap {
            universe: UniverseIdHex::from_u64(42),
            view: MapView::FaceOn,
            population: MapPopulation::All,
            width_px: 2,
            height_px: 2,
            centre_ly: [0.0, 0.0],
            ly_per_px: 65_536.0,
            bits: 8,
            floor_log10_per_ly2: -9.0,
            ceiling_log10_per_ly2: -4.0,
            data_base64: "AAEC/w==".to_owned(),
        })
    }

    fn unparsed(text: &str) -> (RequestId, RequestError) {
        match parse(text) {
            Inbound::Unparsed { id, error } => (id, error),
            other => panic!("expected an unparsed request from {text}, got {other:?}"),
        }
    }

    /// The code and ID of a terminal message.
    fn terminal(message: &ServerMessage) -> (u32, Option<ErrorCode>) {
        match message {
            ServerMessage::Response { id, .. } => (id.0, None),
            ServerMessage::RequestError { id, error } => (id.0, Some(error.code)),
            other => panic!("expected a terminal message, got {other:?}"),
        }
    }

    #[test]
    fn a_well_formed_frame_parses_as_its_message() {
        assert_eq!(
            parse(r#"{"type":"ping","nonce":9}"#),
            Inbound::Message(ClientMessage::Ping { nonce: 9 })
        );
        assert_eq!(
            parse(r#"{"type":"request","id":7,"body":{"kind":"list_universes"}}"#),
            Inbound::Message(ClientMessage::Request {
                id: RequestId(7),
                body: RequestBody::ListUniverses,
            })
        );
    }

    /// Ruling 64.7 of 2026-09-22: a client's float is read as the number it sent, to the bit.
    ///
    /// `558138600491200.44` m, an offset inside one light-year's cell, is the shortest text of its
    /// `f64`, seventeen digits long, and `serde_json` without its `float_roundtrip` feature reads it
    /// one ulp high, as `558138600491200.5`: a range query's centre would reach the server a
    /// sixteenth of a millimetre from where the client put it.
    #[test]
    fn a_seventeen_digit_client_float_is_read_exactly() {
        let sent = 558_138_600_491_200.44_f64;
        assert_eq!(format!("{sent:?}"), "558138600491200.44");
        let frame = format!(
            r#"{{"type":"request","id":3,"body":{{"kind":"systems_in_range","universe":"000000000000002a","centre":{{"cell_ly":[0,26000,0],"offset_m":[{sent:?},0.5,0.25]}},"radius_ly":50.0,"time":{{"seconds":0,"nanos":0}},"min_layer":"a","limit":5000,"include_stellar":false}}}}"#
        );
        let Inbound::Message(ClientMessage::Request {
            body: RequestBody::SystemsInRange(request),
            ..
        }) = parse(&frame)
        else {
            panic!("the frame parses as a range query: {frame}");
        };
        assert_eq!(request.centre.offset_m[0].to_bits(), sent.to_bits());
    }

    #[test]
    fn a_frame_with_no_usable_id_gives_the_connection_level_error() {
        for text in [
            "not json",
            "[]",
            "{}",
            r#"{"type":"warp","id":7}"#,
            r#"{"type":"request","body":{"kind":"list_universes"}}"#,
            r#"{"type":"request","id":"7","body":{"kind":"list_universes"}}"#,
            r#"{"type":"request","id":-1,"body":{"kind":"list_universes"}}"#,
            r#"{"type":"request","id":4294967296,"body":{"kind":"list_universes"}}"#,
            r#"{"type":"request","id":7.5,"body":{"kind":"list_universes"}}"#,
            r#"{"type":"cancel","id":"7"}"#,
        ] {
            match parse(text) {
                Inbound::Malformed { message } => {
                    assert!(message.starts_with("malformed message: "), "{message}");
                }
                other => panic!("expected {text} to be malformed, got {other:?}"),
            }
        }
    }

    #[test]
    fn a_malformed_body_with_a_good_id_gives_bad_request() {
        for text in [
            r#"{"type":"request","id":3,"body":{"kind":"open_universe","universe":"XYZ"}}"#,
            r#"{"type":"request","id":3,"body":{"kind":"open_universe"}}"#,
            r#"{"type":"request","id":3}"#,
            r#"{"type":"request","id":3,"body":"list_universes"}"#,
            r#"{"type":"request","id":3,"body":{"kind":7}}"#,
        ] {
            let (id, error) = unparsed(text);
            assert_eq!(id, RequestId(3), "{text}");
            assert_eq!(error.code, ErrorCode::BadRequest, "{text}");
            assert!(
                error.message.starts_with("malformed request: "),
                "{error:?}"
            );
            assert_eq!(error.field, None);
        }
    }

    #[test]
    fn an_unknown_kind_gives_unsupported() {
        let (id, error) =
            unparsed(r#"{"type":"request","id":4,"body":{"kind":"warp","factor":9}}"#);
        assert_eq!(id, RequestId(4));
        assert_eq!(
            error,
            request_error(ErrorCode::Unsupported, "unsupported request kind `warp`")
        );
    }

    #[test]
    fn only_malformed_frames_count_towards_closing_the_connection() {
        assert_eq!(parse(r#"{"type":"ping","nonce":1}"#).tally(), Tally::Reset);
        assert_eq!(parse("not json").tally(), Tally::Count);
        assert_eq!(
            parse(r#"{"type":"request","id":3,"body":{"kind":"open_universe"}}"#).tally(),
            Tally::Count
        );
        assert_eq!(
            parse(r#"{"type":"request","id":3,"body":{"kind":"warp"}}"#).tally(),
            Tally::Leave
        );
    }

    #[test]
    fn kind_names_every_body_as_the_wire_does() {
        let bodies = every_body();
        assert_eq!(bodies.len(), REQUEST_KINDS.len());
        for body in bodies {
            let wire = serde_json::to_value(&body).unwrap();
            assert_eq!(wire["kind"], kind(&body));
            assert!(REQUEST_KINDS.contains(&kind(&body)));
        }
    }

    #[tokio::test]
    async fn body_events_is_unsupported_until_its_handler_lands() {
        // The kind is the protocol's (P14.T35.c), so it parses and reaches the handlers, which
        // answer it as an older server would until P14.T31 serves it.
        let harness = Harness::start(Handlers).await;
        let events = every_body()
            .into_iter()
            .filter(|body| matches!(body, RequestBody::BodyEvents(_)))
            .collect::<Vec<_>>();
        assert_eq!(events.len(), 1);
        for body in events {
            let name = kind(&body);
            let answer = Handlers
                .handle(Arc::clone(harness.state()), body, CancelToken::new())
                .await;
            assert_eq!(answer, Err(not_served_yet(name)), "{name}");
        }
        harness.stop().await;
    }

    #[test]
    fn pool_errors_become_request_errors() {
        let code = |error: RequestError| error.code;
        assert_eq!(code(SubmitJobError::QueueFull.into()), ErrorCode::QueueFull);
        assert_eq!(code(SubmitJobError::ShutDown.into()), ErrorCode::Internal);
        assert_eq!(code(JobError::Panicked.into()), ErrorCode::Internal);
        assert_eq!(code(JobError::ShutDown.into()), ErrorCode::Internal);
        assert_eq!(code(JobError::Cancelled.into()), ErrorCode::Cancelled);
        // A cached computation's failure is its job's, with the same code.
        assert_eq!(
            code(ComputeError::from(SubmitJobError::QueueFull).into()),
            ErrorCode::QueueFull
        );
        assert_eq!(
            code(ComputeError::from(JobError::Cancelled).into()),
            ErrorCode::Cancelled
        );
        // A galaxy that cannot be generated is the server's own affair, with the reason in the
        // message so that an operator can see which layer refused it.
        let unplayable = RequestError::from(ComputeError::from(
            hyperion_sim::galaxy::placement::ExceedIndexCapacityError::LayerTooDense {
                layer: hyperion_sim::id::Layer::A,
                largest_mean: 64_000.0,
                capacity: 65_536,
            },
        ));
        assert_eq!(unplayable.code, ErrorCode::Internal);
        assert_eq!(unplayable.field, None);
        assert!(
            unplayable
                .message
                .starts_with("this universe cannot be generated: layer A's densest cell"),
            "{unplayable:?}"
        );
    }

    #[tokio::test]
    async fn a_request_is_answered_with_its_handlers_response() {
        let (handler, mut calls) = Scripted::new();
        let harness = Harness::start(handler).await;
        let mut client = harness.connect().await;
        client.hello().await;
        client.request(1, body(1)).await;
        let call = calls.next().await;
        assert_eq!((call.id(), call.token.is_cancelled()), (1, false));
        assert!(call.respond(small_response()));
        assert_eq!(
            client.next_message().await,
            ServerMessage::Response {
                id: RequestId(1),
                body: small_response(),
            }
        );
        // Fails on any frame the server should not have sent.
        client.close().await;
        harness.stop().await;
    }

    #[tokio::test]
    async fn one_terminal_message_per_request_under_interleaved_cancels() {
        let (handler, mut calls) = Scripted::new();
        let harness = Harness::start(handler).await;
        let mut client = harness.connect().await;
        client.hello().await;
        for id in 1..=6 {
            client.request(id, body(id)).await;
        }
        let mut running: BTreeMap<u32, Call> = BTreeMap::new();
        for _ in 1..=6 {
            let call = calls.next().await;
            running.insert(call.id(), call);
        }
        let mut call = |id| running.remove(&id).unwrap();

        // Cancelled while its handler runs: the handler is dropped and its token cancelled.
        client.cancel(2).await;
        let mut two = call(2);
        two.dropped().await;
        assert!(two.token.is_cancelled());
        assert!(!two.respond(small_response()), "the handler has gone");
        // Answered and cancelled at once: either may win, but only one.
        assert!(call(3).respond(small_response()));
        client.cancel(3).await;
        // Cancelled, then answered too late.
        client.cancel(4).await;
        let mut four = call(4);
        four.dropped().await;
        assert!(!four.respond(small_response()));
        // A panic and an error end their requests too.
        assert!(call(5).panic());
        assert!(call(6).fail(request_error(
            ErrorCode::UnknownUniverse,
            "no such universe"
        )));
        assert!(call(1).respond(small_response()));
        harness
            .requests_until(|counters| counters.in_flight() == 0)
            .await;
        // A cancel after the end, and one for an ID never used, are ignored.
        client.cancel(1).await;
        client.cancel(99).await;

        // The pong follows every message the server sent before it.
        let mut terminals: BTreeMap<u32, Vec<Option<ErrorCode>>> = BTreeMap::new();
        for message in client.ping(7).await {
            let (id, code) = terminal(&message);
            terminals.entry(id).or_default().push(code);
        }
        let three = terminals.remove(&3).unwrap();
        assert!(
            three == [None] || three == [Some(ErrorCode::Cancelled)],
            "request 3 ended once: {three:?}"
        );
        assert_eq!(
            terminals,
            BTreeMap::from([
                (1, vec![None]),
                (2, vec![Some(ErrorCode::Cancelled)]),
                (4, vec![Some(ErrorCode::Cancelled)]),
                (5, vec![Some(ErrorCode::Internal)]),
                (6, vec![Some(ErrorCode::UnknownUniverse)]),
            ])
        );
        let counters = harness.server().stats().requests();
        assert_eq!(
            (counters.accepted(), counters.in_flight(), counters.failed()),
            (6, 0, 2)
        );
        assert_eq!(counters.responded() + counters.cancelled(), 4);
        // Fails on any frame the server should not have sent.
        client.close().await;
        harness.stop().await;
    }

    /// How a racing request ends by itself, and the terminal code that ending gives.
    fn end_by_itself(call: Call, round: u32) -> Option<ErrorCode> {
        // Whether the handler was still waiting depends on how the race went, and does not matter.
        match round % 4 {
            0 => {
                call.respond(small_response());
                None
            }
            // Serialised on the pool, which widens the window for the cancel.
            1 => {
                call.respond(density_map());
                None
            }
            2 => {
                call.fail(request_error(
                    ErrorCode::UnknownUniverse,
                    "no such universe",
                ));
                Some(ErrorCode::UnknownUniverse)
            }
            _ => {
                call.panic();
                Some(ErrorCode::Internal)
            }
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn endings_racing_cancels_end_every_request_exactly_once_and_ids_can_be_reused() {
        const ROUNDS: u32 = 400;
        let (handler, mut calls) = Scripted::new();
        let harness = Harness::start(handler).await;
        let mut client = harness.connect().await;
        client.hello().await;
        let mut cancels_won = 0;
        for round in 0..ROUNDS {
            // Always ID 1, so that each request reuses the ID of one that has only just ended.
            client.request(1, body(1)).await;
            let first = calls.next().await;
            // The first request ends by itself as its cancel is on the way, in either order...
            let own_ending = if round % 8 < 4 {
                let own_ending = end_by_itself(first, round);
                client.cancel(1).await;
                own_ending
            } else {
                client.cancel(1).await;
                end_by_itself(first, round)
            };
            // ...and its ID is used again before the client knows how it ended.
            client.request(1, body(1)).await;
            let second = calls.next().await;
            assert!(second.respond(small_response()), "round {round}");
            // Each request ends once, the first before the second, which was read after it.
            let first_ended = terminal(&client.next_message().await);
            assert!(
                first_ended == (1, own_ending) || first_ended == (1, Some(ErrorCode::Cancelled)),
                "round {round}: the first request ended with {first_ended:?}"
            );
            if first_ended.1 == Some(ErrorCode::Cancelled) {
                cancels_won += 1;
            }
            assert_eq!(
                terminal(&client.next_message().await),
                (1, None),
                "round {round}"
            );
        }
        assert_eq!(client.ping(0).await, [], "nothing else was sent");
        let counters = harness
            .requests_until(|counters| counters.in_flight() == 0)
            .await;
        assert_eq!(
            (
                counters.accepted(),
                counters.refused(),
                counters.cancelled()
            ),
            (u64::from(2 * ROUNDS), 0, cancels_won)
        );
        client.close().await;
        harness.stop().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn requests_ending_as_their_connections_close_or_the_server_stops_leave_nothing() {
        const CLIENTS: u32 = 12;
        let limit = u32::try_from(MAX_IN_FLIGHT_REQUESTS).unwrap();
        let (handler, mut calls) = Scripted::new();
        let harness = Harness::start(handler).await;
        let mut clients = Vec::new();
        for _ in 0..CLIENTS {
            let mut client = harness.connect().await;
            client.hello().await;
            for id in 1..=limit {
                client.request(id, body(id)).await;
            }
            clients.push(client);
        }
        let mut running = Vec::new();
        for _ in 0..CLIENTS * limit {
            running.push(calls.next().await);
        }
        // A third of the requests end by themselves while their connections go: some clients
        // close properly, some drop their sockets, and the rest are still connected when the
        // server stops.
        let mut left = Vec::new();
        for (index, call) in running.into_iter().enumerate() {
            match index % 6 {
                0 => {
                    call.respond(small_response());
                }
                1 => {
                    call.panic();
                }
                _ => left.push(call),
            }
        }
        let mut closing = Vec::new();
        for (index, client) in clients.into_iter().enumerate() {
            match index % 3 {
                0 => closing.push(tokio::spawn(async move {
                    // Answers may arrive before the echoed close; they are not checked here.
                    let mut client = client;
                    client.send_close().await;
                    client.drain().await;
                })),
                1 => drop(client),
                _ => closing.push(tokio::spawn(async move {
                    let mut client = client;
                    client.drain().await;
                })),
            }
        }
        harness.stop().await;
        for task in closing {
            task.await.unwrap();
        }
        // `stop` checked the counters; every request left running was dropped, its token
        // cancelled.
        for mut call in left {
            call.dropped().await;
            assert!(call.token.is_cancelled());
        }
    }

    #[tokio::test]
    async fn a_duplicate_id_gives_error_and_the_first_request_still_completes() {
        let (handler, mut calls) = Scripted::new();
        let harness = Harness::start(handler).await;
        let mut client = harness.connect().await;
        client.hello().await;
        client.request(1, body(1)).await;
        let call = calls.next().await;
        client.request(1, body(1)).await;
        // A request under that ID that does not parse is dropped the same way.
        client
            .send_raw(r#"{"type":"request","id":1,"body":{"kind":"open_universe"}}"#)
            .await;
        for _ in 0..2 {
            match client.next_message().await {
                ServerMessage::Error { message } => {
                    assert_eq!(
                        message,
                        "request 1 is already in flight; the new request was dropped"
                    );
                }
                other => panic!("expected the connection-level error, got {other:?}"),
            }
        }
        assert!(calls.is_empty(), "the duplicates never ran");
        assert!(call.respond(small_response()));
        assert_eq!(
            client.next_message().await,
            ServerMessage::Response {
                id: RequestId(1),
                body: small_response(),
            }
        );
        // Once ended, the ID may be used again.
        client.request(1, body(1)).await;
        assert!(calls.next().await.respond(small_response()));
        assert_eq!(terminal(&client.next_message().await), (1, None));
        let counters = harness.server().stats().requests();
        assert_eq!(
            (
                counters.accepted(),
                counters.refused(),
                counters.responded()
            ),
            (2, 2, 2)
        );
        // Fails on any frame the server should not have sent.
        client.close().await;
        harness.stop().await;
    }

    #[tokio::test]
    async fn the_request_beyond_the_limit_gives_too_many_requests_and_a_cancel_frees_a_place() {
        let (handler, mut calls) = Scripted::new();
        let harness = Harness::start(handler).await;
        let mut client = harness.connect().await;
        client.hello().await;
        let limit = u32::try_from(MAX_IN_FLIGHT_REQUESTS).unwrap();
        for id in 1..=limit {
            client.request(id, body(id)).await;
        }
        let mut running = Vec::new();
        for _ in 1..=limit {
            running.push(calls.next().await);
        }
        let beyond = limit + 1;
        client.request(beyond, body(beyond)).await;
        assert_eq!(
            terminal(&client.next_message().await),
            (beyond, Some(ErrorCode::TooManyRequests))
        );
        assert!(calls.is_empty(), "the refused request never ran");
        // The `cancelled` answer frees the place at once.
        client.cancel(1).await;
        assert_eq!(
            terminal(&client.next_message().await),
            (1, Some(ErrorCode::Cancelled))
        );
        client.request(beyond, body(beyond)).await;
        let last = calls.next().await;
        assert_eq!(last.id(), beyond);
        running.push(last);
        let answered = running
            .into_iter()
            .filter(|call| call.id() != 1)
            .map(|call| call.respond(small_response()))
            .filter(|&answered| answered)
            .count();
        assert_eq!(answered, MAX_IN_FLIGHT_REQUESTS);
        for _ in 0..answered {
            assert_eq!(terminal(&client.next_message().await).1, None);
        }
        let counters = harness.server().stats().requests();
        assert_eq!(
            (
                counters.accepted(),
                counters.refused(),
                counters.responded(),
                counters.cancelled()
            ),
            (u64::from(limit) + 1, 1, u64::from(limit), 1)
        );
        // Fails on any frame the server should not have sent.
        client.close().await;
        harness.stop().await;
    }

    #[tokio::test]
    async fn a_request_before_hello_gives_hello_required() {
        let (handler, calls) = Scripted::new();
        let harness = Harness::start(handler).await;
        let mut client = harness.connect().await;
        client.request(1, body(1)).await;
        assert_eq!(
            terminal(&client.next_message().await),
            (1, Some(ErrorCode::HelloRequired))
        );
        client
            .send_raw(r#"{"type":"request","id":2,"body":{"kind":"open_universe"}}"#)
            .await;
        assert_eq!(
            terminal(&client.next_message().await),
            (2, Some(ErrorCode::HelloRequired))
        );
        // Ping is allowed at any time, and a cancel with nothing in flight is ignored.
        client.cancel(1).await;
        assert_eq!(client.ping(3).await, []);
        assert!(calls.is_empty());
        let counters = harness.server().stats().requests();
        assert_eq!((counters.accepted(), counters.refused()), (0, 2));
        // Fails on any frame the server should not have sent.
        client.close().await;
        harness.stop().await;
    }

    #[tokio::test]
    async fn a_panicking_request_answers_internal_and_the_connection_goes_on() {
        let (handler, mut calls) = Scripted::new();
        let harness = Harness::start(handler).await;
        let mut client = harness.connect().await;
        client.hello().await;
        client.request(1, body(1)).await;
        assert!(calls.next().await.panic());
        let answer = client.next_message().await;
        assert_eq!(
            answer,
            ServerMessage::RequestError {
                id: RequestId(1),
                error: request_error(
                    ErrorCode::Internal,
                    "the server failed while answering this request"
                ),
            }
        );
        assert_eq!(client.ping(2).await, []);
        client.request(2, body(2)).await;
        assert!(calls.next().await.respond(small_response()));
        assert_eq!(terminal(&client.next_message().await), (2, None));
        let counters = harness.server().stats().requests();
        assert_eq!((counters.failed(), counters.responded()), (1, 1));
        // Fails on any frame the server should not have sent.
        client.close().await;
        harness.stop().await;
    }

    /// A handler whose every request waits on one shared computation that panics.
    #[derive(Debug, Default)]
    struct SharedPanic {
        flights: SingleFlight<u32, (), RequestError>,
    }

    fn explode() -> Result<Arc<()>, RequestError> {
        panic!("a deliberate panic in a shared computation")
    }

    impl Handler for SharedPanic {
        fn handle(
            &self,
            _state: Arc<AppState>,
            _body: RequestBody,
            _token: CancelToken,
        ) -> HandlerFuture {
            let flight = self.flights.run(0, || async { explode() });
            Box::pin(async move { flight.await.map(|_: Arc<()>| small_response()) })
        }
    }

    /// A handler whose first call holds its worker, inside one poll, until the test lets it go,
    /// and then answers in that same poll: a request that is still running when it is cancelled,
    /// and so finishes although its task was aborted. Later calls go to a [`Scripted`] handler.
    #[derive(Debug)]
    struct Stubborn {
        first: std::sync::Mutex<Option<StubbornCall>>,
        later: Scripted,
    }

    #[derive(Debug)]
    struct StubbornCall {
        entered: tokio::sync::oneshot::Sender<()>,
        go: std::sync::mpsc::Receiver<()>,
    }

    impl Handler for Stubborn {
        fn handle(
            &self,
            state: Arc<AppState>,
            body: RequestBody,
            token: CancelToken,
        ) -> HandlerFuture {
            let first = self.first.lock().unwrap().take();
            match first {
                Some(StubbornCall { entered, go }) => Box::pin(async move {
                    // Hands this worker's other tasks to another thread while it blocks.
                    tokio::task::block_in_place(|| {
                        entered.send(()).unwrap();
                        go.recv().unwrap();
                    });
                    Ok(small_response())
                }),
                None => self.later.handle(state, body, token),
            }
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn a_request_that_finishes_after_its_cancel_does_not_end_the_next_one_with_its_id() {
        let (entered, has_entered) = tokio::sync::oneshot::channel();
        let (let_go, go) = std::sync::mpsc::channel();
        let (later, mut calls) = Scripted::new();
        let harness = Harness::start(Stubborn {
            first: std::sync::Mutex::new(Some(StubbornCall { entered, go })),
            later,
        })
        .await;
        let mut client = harness.connect().await;
        client.hello().await;
        client.request(1, body(1)).await;
        tokio::time::timeout(crate::testing::WAIT, has_entered)
            .await
            .expect("timed out waiting for the first request to run")
            .unwrap();
        // Cancelled while its handler is inside a poll: the abort waits for the poll to return.
        client.cancel(1).await;
        assert_eq!(
            terminal(&client.next_message().await),
            (1, Some(ErrorCode::Cancelled))
        );
        // The ID is free again at once, and taken by a new request.
        client.request(1, body(1)).await;
        let second = calls.next().await;
        // The first request's task now finishes, response and all, as though never aborted.
        let_go.send(()).unwrap();
        // A duplicate of the new request is refused only while it is still in flight: the first
        // request's late result, joined meanwhile, has not ended it.
        client.request(1, body(1)).await;
        assert_eq!(
            client.next_message().await,
            ServerMessage::Error {
                message: "request 1 is already in flight; the new request was dropped".to_owned(),
            }
        );
        assert_eq!(client.ping(2).await, [], "the late result was not sent");
        assert!(second.respond(density_map()));
        assert_eq!(
            client.next_message().await,
            ServerMessage::Response {
                id: RequestId(1),
                body: density_map(),
            }
        );
        let counters = harness.server().stats().requests();
        assert_eq!(
            (
                counters.accepted(),
                counters.cancelled(),
                counters.responded(),
                counters.refused()
            ),
            (2, 1, 1, 1)
        );
        client.close().await;
        harness.stop().await;
    }

    #[tokio::test]
    async fn a_panic_in_a_shared_computation_answers_internal_to_every_waiter() {
        let harness = Harness::start(SharedPanic::default()).await;
        let mut client = harness.connect().await;
        client.hello().await;
        client.request(1, body(1)).await;
        client.request(2, body(2)).await;
        let mut codes: Vec<_> = [client.next_message().await, client.next_message().await]
            .iter()
            .map(terminal)
            .collect();
        codes.sort_unstable_by_key(|&(id, _)| id);
        assert_eq!(
            codes,
            [
                (1, Some(ErrorCode::Internal)),
                (2, Some(ErrorCode::Internal))
            ]
        );
        assert_eq!(client.ping(3).await, []);
        // Fails on any frame the server should not have sent.
        client.close().await;
        harness.stop().await;
    }

    #[tokio::test]
    async fn a_large_response_is_serialised_on_the_pool() {
        let (handler, mut calls) = Scripted::new();
        let harness = Harness::start(handler).await;
        let mut client = harness.connect().await;
        client.hello().await;
        client.request(1, body(1)).await;
        assert!(calls.next().await.respond(small_response()));
        assert_eq!(terminal(&client.next_message().await), (1, None));
        assert_eq!(harness.server().stats().pool().completed(), 0);

        client.request(2, body(2)).await;
        assert!(calls.next().await.respond(density_map()));
        assert_eq!(
            client.next_message().await,
            ServerMessage::Response {
                id: RequestId(2),
                body: density_map(),
            }
        );
        assert_eq!(
            harness.server().stats().pool().completed(),
            1,
            "the map's frame was made by a pool job"
        );
        // Fails on any frame the server should not have sent.
        client.close().await;
        harness.stop().await;
    }
}
