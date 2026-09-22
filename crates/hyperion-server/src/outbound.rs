//! One connection's outbound queue, and the writer task that drains it into the socket.
//!
//! The queue is bounded twice. In frames, by a channel of [`OUTBOUND_QUEUE_FRAMES`] (plan 04,
//! P04.T13.a): when it is full the connection waits, and so stops reading. In bytes, by the
//! connection's budget, [`OUTBOUND_BYTES`] in production (P04.T15): every frame is charged its
//! payload from the moment it is queued until it has been written or dropped, and while a
//! finished request's terminal frame would take the queue past the budget, the connection keeps
//! it in [`Held`] and reads on, so that `ping` and `cancel` are still answered. A frame larger
//! than the whole budget goes once the queue is empty. Only finished requests wait for the
//! budget: the connection's other frames are small answers to frames the client sent, which the
//! frame count bounds.
//!
//! The writer gives each frame [`WRITE_TIMEOUT`] in production to be written. A frame that takes
//! longer means the client has stopped reading. The writer then tells the connection, which
//! closes with a policy violation, and writes nothing more but that close frame.
//!
//! [`OUTBOUND_QUEUE_FRAMES`]: crate::limits::OUTBOUND_QUEUE_FRAMES
//! [`OUTBOUND_BYTES`]: crate::limits::OUTBOUND_BYTES
//! [`WRITE_TIMEOUT`]: crate::limits::WRITE_TIMEOUT

use std::collections::VecDeque;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::Message;
use futures_util::{Sink, SinkExt};
use tokio::sync::mpsc::error::SendError;
use tokio::sync::{mpsc, oneshot, watch};
use tokio::time::timeout;

use crate::limits::{MAX_IN_FLIGHT_REQUESTS, OUTBOUND_QUEUE_FRAMES};
use crate::requests::Settled;
use crate::stats::OutboundStats;

/// Opens a connection's outbound queue, reporting to `stats`.
///
/// The queue has room for `budget_bytes` of payload, and gives each frame `write_timeout` to be
/// written. Returns the connection's end of it, and the writer, which the connection runs on a
/// task of its own.
#[must_use]
pub(crate) fn open(
    stats: OutboundStats,
    budget_bytes: usize,
    write_timeout: Duration,
) -> (Outbound, Writer) {
    let (frames, queue) = mpsc::channel(OUTBOUND_QUEUE_FRAMES);
    let (bytes, room) = watch::channel(0);
    let (stopped, writer_stopped) = oneshot::channel();
    let outbound = Outbound {
        frames,
        backlog: Arc::new(Backlog { bytes, stats }),
        room,
        budget_bytes,
        writer_stopped: Some(writer_stopped),
    };
    let writer = Writer {
        queue,
        write_timeout,
        stopped,
    };
    (outbound, writer)
}

/// Why the writer stopped while the connection still had its queue open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum WriterStopped {
    /// Writing to the socket failed, as it does once the client has gone.
    Failed,
    /// A frame could not be written in time.
    ///
    /// The writer goes on taking frames from the queue, but writes nothing more than the close
    /// frame.
    TimedOut,
}

/// A frame in the queue, with the charge that it releases once it has been written or dropped.
#[derive(Debug)]
pub(crate) struct Queued {
    message: Message,
    charge: Charge,
}

/// The connection's end of its outbound queue.
#[derive(Debug)]
pub(crate) struct Outbound {
    frames: mpsc::Sender<Queued>,
    backlog: Arc<Backlog>,
    /// The payload bytes queued now, for waiting on room.
    room: watch::Receiver<usize>,
    budget_bytes: usize,
    /// `None` once the writer's stop has been reported.
    writer_stopped: Option<oneshot::Receiver<WriterStopped>>,
}

impl Outbound {
    /// Queues `message`, waiting while the queue holds its full count of frames.
    ///
    /// The frame is charged to the byte budget at once, whether or not it fits. Cancellation-safe:
    /// dropped while it waits, it queues nothing and releases the charge. After a write has timed
    /// out, frames are still accepted, and the writer drops all but the close frame.
    ///
    /// # Errors
    ///
    /// Gives the frame back once the writer's task has ended: a write failed, or the task
    /// panicked or was aborted.
    pub(crate) async fn send(&self, message: Message) -> Result<(), SendError<Queued>> {
        let charge = self.backlog.charge(payload_len(&message));
        self.frames.send(Queued { message, charge }).await
    }

    /// Whether a frame of `bytes` fits the budget now.
    ///
    /// It fits when the queue is empty, or would still be within the budget with the frame added.
    #[must_use]
    pub(crate) fn has_room_for(&self, bytes: usize) -> bool {
        fits(*self.room.borrow(), bytes, self.budget_bytes)
    }

    /// Completes once a frame of `bytes` fits the budget, as [`Outbound::has_room_for`] says.
    ///
    /// The future borrows nothing, so that the connection can wait on it and on
    /// [`Outbound::writer_stopped`] at once. Cancellation-safe.
    pub(crate) fn room_for(&self, bytes: usize) -> impl Future<Output = ()> + use<> {
        let budget = self.budget_bytes;
        let mut room = self.room.clone();
        async move {
            // Ignoring the error is safe: the sender lives in the backlog, which the connection
            // keeps while it waits.
            let _ = room.wait_for(|&queued| fits(queued, bytes, budget)).await;
        }
    }

    /// Waits until the writer stops while the queue is still open, and says why.
    ///
    /// Cancellation-safe. Once it has returned it never returns again.
    pub(crate) async fn writer_stopped(&mut self) -> WriterStopped {
        let Some(stopped) = &mut self.writer_stopped else {
            return std::future::pending().await;
        };
        // The queue stays open while `self` lives, so a writer that drops its sender unsent has
        // panicked or been aborted: for the connection, it has failed.
        let why = stopped.await.unwrap_or(WriterStopped::Failed);
        self.writer_stopped = None;
        why
    }
}

/// The payload bytes queued on one connection.
///
/// Each [`Charge`] keeps it, so that a frame is counted until the charge is dropped, wherever
/// that happens.
#[derive(Debug)]
struct Backlog {
    bytes: watch::Sender<usize>,
    stats: OutboundStats,
}

impl Backlog {
    /// Charges a frame of `bytes` to the connection and to the server's totals.
    #[must_use]
    fn charge(self: &Arc<Self>, bytes: usize) -> Charge {
        let mut connection_bytes = 0;
        self.bytes.send_modify(|queued| {
            *queued = queued.saturating_add(bytes);
            connection_bytes = *queued;
        });
        self.stats.queued(bytes, connection_bytes);
        Charge {
            bytes,
            backlog: Arc::clone(self),
        }
    }
}

/// One queued frame's share of its connection's backlog, released when dropped.
///
/// It is dropped once the frame has been written, or dropped unwritten with its connection.
#[derive(Debug)]
struct Charge {
    bytes: usize,
    backlog: Arc<Backlog>,
}

impl Drop for Charge {
    fn drop(&mut self) {
        let bytes = self.bytes;
        self.backlog
            .bytes
            .send_modify(|queued| *queued = queued.saturating_sub(bytes));
        self.backlog.stats.dequeued(bytes);
    }
}

/// Whether a frame of `bytes` may join a queue holding `queued` under `budget`.
///
/// A frame larger than the whole budget fits an empty queue, or it would never be sent.
#[must_use]
fn fits(queued: usize, bytes: usize, budget: usize) -> bool {
    queued == 0 || queued.saturating_add(bytes) <= budget
}

/// The payload bytes of a frame: what the budget counts.
#[must_use]
fn payload_len(message: &Message) -> usize {
    match message {
        Message::Text(text) => text.len(),
        Message::Binary(bytes) | Message::Ping(bytes) | Message::Pong(bytes) => bytes.len(),
        // The two bytes of the close code, then the reason.
        Message::Close(frame) => frame
            .as_ref()
            .map_or(0, |frame| frame.reason.len().saturating_add(2)),
    }
}

/// Finished requests whose terminal frames wait for room in the queue, oldest first.
///
/// Each is still in flight, so that a `cancel` can end it; the connection then drops its frame
/// with [`Held::retain`]. At most [`MAX_IN_FLIGHT_REQUESTS`] are held, since each is in flight.
#[derive(Debug)]
pub(crate) struct Held {
    requests: VecDeque<Settled>,
    stats: OutboundStats,
}

impl Held {
    /// Nothing held yet, reporting to `stats`.
    #[must_use]
    pub(crate) fn new(stats: OutboundStats) -> Self {
        Self {
            requests: VecDeque::with_capacity(MAX_IN_FLIGHT_REQUESTS),
            stats,
        }
    }

    /// Holds a finished request behind those held already.
    pub(crate) fn push(&mut self, settled: Settled) {
        self.requests.push_back(settled);
        self.stats.held();
    }

    /// Whether no request is held.
    #[must_use]
    pub(crate) fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }

    /// The payload bytes of the oldest held request's frame, if any is held.
    #[must_use]
    pub(crate) fn next_len(&self) -> Option<usize> {
        self.requests.front().map(Settled::frame_len)
    }

    /// Lets go of the oldest held request.
    #[must_use]
    pub(crate) fn pop(&mut self) -> Option<Settled> {
        let settled = self.requests.pop_front()?;
        self.stats.released(1);
        Some(settled)
    }

    /// Lets go of every held request for which `keep` is false.
    pub(crate) fn retain(&mut self, keep: impl FnMut(&Settled) -> bool) {
        let before = self.requests.len();
        self.requests.retain(keep);
        self.stats.released(before - self.requests.len());
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        self.stats.released(self.requests.len());
    }
}

/// The writer's end of a connection's outbound queue.
#[derive(Debug)]
pub(crate) struct Writer {
    queue: mpsc::Receiver<Queued>,
    write_timeout: Duration,
    stopped: oneshot::Sender<WriterStopped>,
}

impl Writer {
    /// Writes queued frames to `sink` in order until the queue closes, then closes the sink.
    ///
    /// Closing a WebSocket's sink completes its close handshake. A frame that fails to write
    /// stops the writer, and one that is not written within the write timeout stops it writing
    /// anything but the close frame; either way it tells the connection, which is waiting in
    /// [`Outbound::writer_stopped`].
    pub(crate) async fn run<S>(self, mut sink: S)
    where
        S: Sink<Message> + Unpin,
        S::Error: fmt::Display,
    {
        let Self {
            mut queue,
            write_timeout,
            stopped,
        } = self;
        let stop = loop {
            let Some(Queued { message, charge }) = queue.recv().await else {
                break None;
            };
            match timeout(write_timeout, sink.send(message)).await {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    tracing::debug!(%error, "failed to write to the client");
                    break Some((WriterStopped::Failed, charge));
                }
                Err(_) => {
                    tracing::warn!(
                        bytes = charge.bytes,
                        timeout_s = write_timeout.as_secs_f64(),
                        "a frame was not written in time; closing the connection"
                    );
                    break Some((WriterStopped::TimedOut, charge));
                }
            }
        };
        if let Some((why, charge)) = stop {
            // Told before the frame's charge is released, so that a connection woken by the
            // release knows why. Ignoring the error is safe: it means the connection has ended.
            let _ = stopped.send(why);
            drop(charge);
            match why {
                WriterStopped::Failed => return,
                WriterStopped::TimedOut => {
                    if !send_close_only(&mut sink, &mut queue).await {
                        return;
                    }
                }
            }
        }
        // Answers the client's close frame, or sends ours, and flushes.
        if let Err(error) = sink.close().await {
            tracing::debug!(%error, "failed to close the socket");
        }
    }
}

/// After a frame has timed out, drops every queued frame but the connection's close frame.
///
/// The close frame follows what is left of the frame that timed out, should the client read again
/// before the connection gives up on it. Returns `false` if writing failed.
async fn send_close_only<S>(sink: &mut S, queue: &mut mpsc::Receiver<Queued>) -> bool
where
    S: Sink<Message> + Unpin,
    S::Error: fmt::Display,
{
    while let Some(Queued { message, charge }) = queue.recv().await {
        if matches!(message, Message::Close(_))
            && let Err(error) = sink.send(message).await
        {
            tracing::debug!(%error, "failed to send the close frame");
            return false;
        }
        drop(charge);
    }
    true
}

#[cfg(test)]
mod tests {
    use axum::extract::ws::{CloseFrame, Utf8Bytes, close_code};
    use hyperion_protocol::{ErrorCode, RequestId, ServerMessage};

    use std::convert::Infallible;

    use super::*;
    use crate::compute::{CancelToken, JobError, Priority};
    use crate::limits::{OUTBOUND_BYTES, WRITE_TIMEOUT};
    use crate::requests::{request_error, to_frame};
    use crate::testing::{
        CLOGGING_BYTES, Client, Harness, Scripted, WAIT, body, bulky_response, clogging_frame_len,
        response_frame_len, small_response,
    };
    use crate::ws::ConnectionLimits;

    /// A write timeout short enough for a test to wait out. A write to a client that reads
    /// completes in the first poll, so only a stuck writer reaches it.
    const SHORT_WRITE_TIMEOUT: Duration = Duration::from_millis(200);

    /// Longer than any test runs, for a limit that a test must not reach.
    const NEVER: Duration = Duration::from_secs(3600);

    /// The server's limits, with the given budget and write timeout.
    fn limits(outbound_bytes: usize, write_timeout: Duration) -> ConnectionLimits {
        ConnectionLimits {
            outbound_bytes,
            write_timeout,
            ..ConnectionLimits::default()
        }
    }

    /// A slow reader that has said hello, once its welcome has been written, so that nothing is
    /// queued for it.
    async fn greeted_slow_reader(harness: &Harness) -> Client {
        let mut slow = harness.connect_slow_reader().await;
        slow.hello().await;
        harness
            .outbound_until(|counters| counters.queued_bytes() == 0)
            .await;
        slow
    }

    /// The `cancelled` that ends request `id`.
    fn cancelled(id: u32) -> ServerMessage {
        ServerMessage::RequestError {
            id: RequestId(id),
            error: request_error(ErrorCode::Cancelled, "cancelled by the client"),
        }
    }

    fn response(id: u32, body: hyperion_protocol::ResponseBody) -> ServerMessage {
        ServerMessage::Response {
            id: RequestId(id),
            body,
        }
    }

    /// Bounds a wait on the paused clock, which runs on to the next timer whenever nothing else
    /// can happen: well beyond the write timeout, so that only a hang reaches it.
    const PAUSED_GUARD: Duration = Duration::from_secs(60);

    /// A sink that sends each frame written to it on `written`, and finishes writing the first
    /// only once `release` fires, as a socket does for a client that has stopped reading.
    fn stuck_sink(
        written: mpsc::Sender<Message>,
        release: oneshot::Receiver<()>,
    ) -> impl Sink<Message, Error = Infallible> + Unpin {
        Box::pin(futures_util::sink::unfold(
            (written, Some(release)),
            |(written, mut release), message: Message| async move {
                if let Some(release) = release.take() {
                    // Ignoring the error is safe: a dropped sender lets the frame go too.
                    let _ = release.await;
                }
                written
                    .send(message)
                    .await
                    .expect("the test reads what is written");
                Ok((written, release))
            },
        ))
    }

    #[tokio::test(start_paused = true)]
    async fn after_a_write_times_out_only_the_close_frame_follows_the_frame_that_timed_out() {
        let stats = OutboundStats::new();
        let (mut outbound, writer) = open(stats.clone(), OUTBOUND_BYTES, WRITE_TIMEOUT);
        let (release, released) = oneshot::channel();
        let (sink_writes, mut written) = mpsc::channel(8);
        let writing = tokio::spawn(writer.run(stuck_sink(sink_writes, released)));
        for text in ["stuck", "queued behind"] {
            outbound.send(Message::text(text)).await.unwrap();
        }
        // Nothing else can happen, so the paused clock runs on to the write timeout.
        let stopped = timeout(PAUSED_GUARD, outbound.writer_stopped()).await;
        assert_eq!(stopped, Ok(WriterStopped::TimedOut));
        // Frames queued since are accepted, and dropped with the one queued behind.
        outbound.send(Message::text("queued after")).await.unwrap();
        let close = Message::Close(Some(CloseFrame {
            code: close_code::POLICY,
            reason: Utf8Bytes::from_static("the client is not reading"),
        }));
        outbound.send(close.clone()).await.unwrap();
        // The client reads again: it gets the rest of the frame that timed out, then the close.
        release.send(()).unwrap();
        for expected in [Message::text("stuck"), close] {
            let next = timeout(PAUSED_GUARD, written.recv()).await;
            assert_eq!(next, Ok(Some(expected)));
        }
        drop(outbound);
        timeout(PAUSED_GUARD, writing).await.unwrap().unwrap();
        assert_eq!(written.recv().await, None, "nothing else was written");
        assert_eq!(stats.snapshot().queued_bytes(), 0);
    }

    /// A sink that takes the next of `delays` to write each frame, then sends it on `written`, as
    /// a socket does for a client that reads at its own pace.
    fn paced_sink(
        written: mpsc::Sender<Message>,
        delays: impl IntoIterator<Item = Duration>,
    ) -> impl Sink<Message, Error = Infallible> + Unpin {
        let delays: VecDeque<Duration> = delays.into_iter().collect();
        Box::pin(futures_util::sink::unfold(
            (written, delays),
            |(written, mut delays), message: Message| async move {
                let delay = delays.pop_front().expect("a delay for every frame");
                tokio::time::sleep(delay).await;
                written
                    .send(message)
                    .await
                    .expect("the test reads what is written");
                Ok((written, delays))
            },
        ))
    }

    #[tokio::test(start_paused = true)]
    async fn the_write_timeout_bounds_each_frame_and_not_the_frames_together() {
        let stats = OutboundStats::new();
        let (mut outbound, writer) = open(stats.clone(), OUTBOUND_BYTES, WRITE_TIMEOUT);
        let (sink_writes, mut written) = mpsc::channel(8);
        let just_within = WRITE_TIMEOUT / 10 * 9;
        let just_over = WRITE_TIMEOUT / 10 * 11;
        let delays = [just_within, just_within, just_within, just_over];
        let writing = tokio::spawn(writer.run(paced_sink(sink_writes, delays)));
        let started = tokio::time::Instant::now();
        let frames = ["one", "two", "three"];
        for text in frames {
            outbound.send(Message::text(text)).await.unwrap();
        }
        for text in frames {
            let next = timeout(PAUSED_GUARD, written.recv()).await;
            assert_eq!(next, Ok(Some(Message::text(text))));
        }
        assert!(
            started.elapsed() > WRITE_TIMEOUT * 2,
            "together the frames took well over the timeout, though none took it alone"
        );
        assert_eq!(
            futures_util::FutureExt::now_or_never(outbound.writer_stopped()),
            None,
            "the writer stopped for a client that reads every frame in time"
        );
        // A frame that takes longer than the timeout alone stops it.
        outbound.send(Message::text("four")).await.unwrap();
        let stopped = timeout(PAUSED_GUARD, outbound.writer_stopped()).await;
        assert_eq!(stopped, Ok(WriterStopped::TimedOut));
        drop(outbound);
        timeout(PAUSED_GUARD, writing).await.unwrap().unwrap();
        assert_eq!(stats.snapshot().queued_bytes(), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn a_failed_write_stops_the_writer_and_closes_the_queue() {
        let stats = OutboundStats::new();
        let (mut outbound, writer) = open(stats.clone(), OUTBOUND_BYTES, WRITE_TIMEOUT);
        let sink = Box::pin(futures_util::sink::unfold((), |(), _: Message| async {
            Err::<(), _>("the client has gone")
        }));
        let writing = tokio::spawn(writer.run(sink));
        outbound.send(Message::text("lost")).await.unwrap();
        let stopped = timeout(PAUSED_GUARD, outbound.writer_stopped()).await;
        assert_eq!(stopped, Ok(WriterStopped::Failed));
        timeout(PAUSED_GUARD, writing).await.unwrap().unwrap();
        match outbound.send(Message::text("too late")).await {
            Err(SendError(queued)) => assert_eq!(queued.message, Message::text("too late")),
            Ok(()) => panic!("a frame was queued for a writer that has gone"),
        }
        assert_eq!(stats.snapshot().queued_bytes(), 0);
    }

    #[test]
    fn a_frame_fits_within_the_budget_or_alone() {
        assert!(fits(0, 10, 100));
        assert!(fits(90, 10, 100));
        assert!(!fits(91, 10, 100));
        assert!(
            fits(0, 1_000, 100),
            "a frame over the budget fits an empty queue"
        );
        assert!(!fits(1, 1_000, 100));
        assert!(!fits(usize::MAX, 1, 100));
    }

    #[test]
    fn a_frame_is_charged_its_payload() {
        assert_eq!(payload_len(&Message::text("{\"type\":\"pong\"}")), 15);
        assert_eq!(payload_len(&Message::Close(None)), 0);
        let close = CloseFrame {
            code: close_code::POLICY,
            reason: Utf8Bytes::from_static("slow"),
        };
        assert_eq!(payload_len(&Message::Close(Some(close))), 6);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn a_frame_not_written_in_time_closes_the_connection_with_a_policy_violation() {
        // The close waits for the client, which reads again only after the timeout.
        let limits = ConnectionLimits {
            close_timeout: NEVER,
            ..limits(OUTBOUND_BYTES, SHORT_WRITE_TIMEOUT)
        };
        let (handler, mut calls) = Scripted::new();
        let harness = Harness::start_with_limits(handler, limits).await;
        let mut slow = greeted_slow_reader(&harness).await;
        slow.request(2, body(2)).await;
        let mut running = calls.next().await;
        harness.stick_writer(&mut calls, &mut slow, 1).await;

        harness
            .outbound_until(|counters| counters.write_timeouts() == 1)
            .await;
        // The request still running was cancelled with its connection.
        running.dropped().await;
        assert!(running.token.is_cancelled());
        assert!(!running.respond(small_response()));
        // Reading again, the client gets the rest of the frame that timed out, then the close
        // frame, and nothing else: `closed` fails on a text frame.
        assert_eq!(
            slow.next_message().await,
            response(1, bulky_response(CLOGGING_BYTES))
        );
        assert_eq!(slow.closed().await, Some(close_code::POLICY));
        harness.connections_until(0).await;

        let stats = harness.server().stats();
        let (requests, outbound) = (stats.requests(), stats.outbound());
        assert_eq!(
            (
                requests.accepted(),
                requests.responded(),
                requests.abandoned(),
                requests.in_flight()
            ),
            (2, 1, 1, 0)
        );
        assert_eq!(
            (
                outbound.queued_bytes(),
                outbound.held_requests(),
                outbound.write_timeouts()
            ),
            (0, 0, 1)
        );
        harness.stop().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn a_client_that_never_reads_again_is_dropped_and_everything_it_held_is_released() {
        let (handler, mut calls) = Scripted::new();
        let harness =
            Harness::start_with_limits(handler, limits(OUTBOUND_BYTES, SHORT_WRITE_TIMEOUT)).await;
        let pool = &harness.state().pool;
        // The pool's one worker is held, so that a job queued for the request waits.
        let (entered, has_entered) = oneshot::channel();
        let (let_go, go) = std::sync::mpsc::channel::<()>();
        let holding = pool
            .try_submit(Priority::Interactive, CancelToken::new(), move |_| {
                entered.send(()).expect("the test waits for the worker");
                go.recv().expect("the test lets the worker go");
            })
            .expect("the queue has room");
        timeout(WAIT, has_entered)
            .await
            .expect("timed out waiting for the worker")
            .expect("the job runs");

        let mut slow = greeted_slow_reader(&harness).await;
        slow.request(2, body(2)).await;
        let mut running = calls.next().await;
        // What a handler queues: a job that carries its request's token.
        let job = pool
            .try_submit(Priority::Interactive, running.token.clone(), |_| ())
            .expect("the queue has room");
        harness.stick_writer(&mut calls, &mut slow, 1).await;

        harness
            .outbound_until(|counters| counters.write_timeouts() == 1)
            .await;
        running.dropped().await;
        assert!(running.token.is_cancelled());
        // The client never answers the close, so the server drops the socket after the close
        // timeout, and the connection's place in the tracker with it.
        harness.connections_until(0).await;
        // The request's job, reached once the worker is free, is skipped.
        let_go.send(()).expect("the worker waits");
        assert_eq!(timeout(WAIT, holding).await.unwrap().unwrap(), Ok(()));
        assert_eq!(
            timeout(WAIT, job).await.unwrap().unwrap(),
            Err(JobError::Cancelled)
        );

        let stats = harness.server().stats();
        let (requests, outbound, pool) = (stats.requests(), stats.outbound(), stats.pool());
        assert_eq!(
            (
                stats.connections(),
                requests.in_flight(),
                requests.responded(),
                requests.abandoned()
            ),
            (0, 0, 1, 1)
        );
        assert_eq!(
            (
                outbound.queued_bytes(),
                outbound.held_requests(),
                outbound.write_timeouts()
            ),
            (0, 0, 1)
        );
        assert_eq!(
            (
                pool.queued_interactive(),
                pool.running(),
                pool.completed(),
                pool.cancelled()
            ),
            (0, 0, 1, 1)
        );
        drop(slow);
        harness.stop().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn the_byte_budget_holds_with_frames_of_mixed_sizes() {
        // Room for the response that sticks the writer, and 1 MiB more.
        let budget = CLOGGING_BYTES + (1 << 20);
        let (handler, mut calls) = Scripted::new();
        let harness = Harness::start_with_limits(handler, limits(budget, NEVER)).await;
        let mut slow = greeted_slow_reader(&harness).await;
        // Request ID, response size, and whether the response is queued at once. Once one is
        // held, every later one is held behind it, although some would fit.
        let answers: [(u32, usize, bool); 7] = [
            (2, 256 << 10, true),
            (3, 64 << 10, true),
            (4, 768 << 10, false),
            (5, 1 << 10, false),
            (6, 512 << 10, false),
            (7, 4 << 10, false),
            (8, 2 << 20, false),
        ];
        let mut running = std::collections::BTreeMap::new();
        for (id, ..) in answers {
            slow.request(id, body(id)).await;
            let call = calls.next().await;
            running.insert(call.id(), call);
        }
        let clogging = harness.stick_writer(&mut calls, &mut slow, 1).await;
        let mut queued = clogging;
        harness
            .outbound_until(|counters| counters.queued_bytes() == queued)
            .await;

        // Answered one at a time, each waited for, so that they finish in this order.
        let mut held = 0;
        for (id, size, at_once) in answers {
            let call = running.remove(&id).expect("every request is running");
            assert!(call.respond(bulky_response(size)));
            if at_once {
                queued += response_frame_len(id, bulky_response(size));
                harness
                    .outbound_until(|counters| counters.queued_bytes() == queued)
                    .await;
            } else {
                held += 1;
                let counters = harness
                    .outbound_until(|counters| counters.held_requests() == held)
                    .await;
                assert_eq!(counters.queued_bytes(), queued, "request {id} was queued");
            }
        }
        assert!(queued <= budget);
        assert_eq!(
            harness.server().stats().outbound().largest_queue_bytes(),
            queued
        );

        // Reading again, the client gets every response whole and in order, the held ones queued
        // as room appears.
        assert_eq!(
            slow.next_message().await,
            response(1, bulky_response(CLOGGING_BYTES))
        );
        for (id, size, _) in answers {
            assert_eq!(
                slow.next_message().await,
                response(id, bulky_response(size))
            );
        }
        let outbound = harness
            .outbound_until(|counters| counters.queued_bytes() == 0)
            .await;
        assert_eq!(outbound.held_requests(), 0);
        assert!(
            outbound.largest_queue_bytes() <= budget,
            "the queue held {} bytes, over the budget of {budget}",
            outbound.largest_queue_bytes()
        );
        assert_eq!(slow.ping(0).await, []);
        slow.close().await;
        harness.stop().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn a_frame_larger_than_the_whole_budget_is_sent_once_the_queue_is_empty() {
        // Smaller than the responses that follow.
        let budget = 64 << 10;
        let oversized = 256 << 10;
        let (handler, mut calls) = Scripted::new();
        let harness = Harness::start_with_limits(handler, limits(budget, NEVER)).await;
        let mut slow = greeted_slow_reader(&harness).await;
        slow.request(2, body(2)).await;
        let second = calls.next().await;
        slow.request(3, body(3)).await;
        let third = calls.next().await;
        // Over the budget itself, and queued at once: the queue was empty.
        let clogging = harness.stick_writer(&mut calls, &mut slow, 1).await;
        harness
            .outbound_until(|counters| counters.queued_bytes() == clogging)
            .await;
        // Over the budget, and held while anything is queued.
        assert!(second.respond(bulky_response(oversized)));
        harness
            .outbound_until(|counters| counters.held_requests() == 1)
            .await;
        // Within the budget alone, but held behind the one before it.
        assert!(third.respond(small_response()));
        let counters = harness
            .outbound_until(|counters| counters.held_requests() == 2)
            .await;
        assert_eq!(counters.queued_bytes(), clogging);

        assert_eq!(
            slow.next_message().await,
            response(1, bulky_response(CLOGGING_BYTES))
        );
        assert_eq!(
            slow.next_message().await,
            response(2, bulky_response(oversized))
        );
        assert_eq!(slow.next_message().await, response(3, small_response()));
        let outbound = harness
            .outbound_until(|counters| counters.queued_bytes() == 0)
            .await;
        // Each response went alone, into an empty queue.
        assert_eq!(
            (outbound.held_requests(), outbound.largest_queue_bytes()),
            (0, clogging)
        );
        slow.close().await;
        harness.stop().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn pings_and_cancels_are_answered_while_the_budget_is_full() {
        let (handler, mut calls) = Scripted::new();
        let harness = Harness::start_with_limits(handler, limits(64 << 10, NEVER)).await;
        let mut slow = greeted_slow_reader(&harness).await;
        slow.request(2, body(2)).await;
        let second = calls.next().await;
        slow.request(3, body(3)).await;
        let mut third = calls.next().await;
        let mut queued = harness.stick_writer(&mut calls, &mut slow, 1).await;
        harness
            .outbound_until(|counters| counters.queued_bytes() == queued)
            .await;
        // The queue is over its budget, so request 2's response is held.
        assert!(second.respond(small_response()));
        harness
            .outbound_until(|counters| counters.held_requests() == 1)
            .await;

        // A ping is answered at once, behind the response that sticks the writer.
        slow.send(&hyperion_protocol::ClientMessage::Ping { nonce: 7 })
            .await;
        queued += to_frame(&ServerMessage::Pong { nonce: 7 }).len();
        harness
            .outbound_until(|counters| counters.queued_bytes() == queued)
            .await;
        // So is the cancel of a request still running...
        slow.cancel(3).await;
        third.dropped().await;
        assert!(third.token.is_cancelled());
        queued += to_frame(&cancelled(3)).len();
        harness
            .outbound_until(|counters| counters.queued_bytes() == queued)
            .await;
        // ...and that of the held request, whose response is dropped.
        slow.cancel(2).await;
        queued += to_frame(&cancelled(2)).len();
        harness
            .outbound_until(|counters| {
                counters.queued_bytes() == queued && counters.held_requests() == 0
            })
            .await;
        // A request that finishes now is held in its turn.
        slow.request(4, body(4)).await;
        assert!(calls.next().await.respond(small_response()));
        harness
            .outbound_until(|counters| counters.held_requests() == 1)
            .await;

        // Reading again, the client gets every frame in the order it was queued, the held
        // response once the queue has room, and nothing else.
        for expected in [
            response(1, bulky_response(CLOGGING_BYTES)),
            ServerMessage::Pong { nonce: 7 },
            cancelled(3),
            cancelled(2),
            response(4, small_response()),
        ] {
            assert_eq!(slow.next_message().await, expected);
        }
        assert_eq!(slow.ping(8).await, []);
        let requests = harness.server().stats().requests();
        assert_eq!(
            (
                requests.accepted(),
                requests.responded(),
                requests.cancelled(),
                requests.refused()
            ),
            (4, 2, 2, 0)
        );
        slow.close().await;
        harness.stop().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn requests_held_when_their_client_disconnects_are_abandoned_and_released() {
        let (handler, mut calls) = Scripted::new();
        let harness = Harness::start_with_limits(handler, limits(64 << 10, NEVER)).await;
        let mut slow = greeted_slow_reader(&harness).await;
        slow.request(2, body(2)).await;
        let second = calls.next().await;
        harness.stick_writer(&mut calls, &mut slow, 1).await;
        assert!(second.respond(small_response()));
        harness
            .outbound_until(|counters| counters.held_requests() == 1)
            .await;
        // Unread data makes the client's end reset the connection, which fails the stuck write.
        drop(slow);
        harness.connections_until(0).await;

        let stats = harness.server().stats();
        let (requests, outbound) = (stats.requests(), stats.outbound());
        assert_eq!(
            (
                requests.in_flight(),
                requests.responded(),
                requests.abandoned()
            ),
            (0, 1, 1)
        );
        assert_eq!(
            (
                outbound.queued_bytes(),
                outbound.held_requests(),
                outbound.write_timeouts()
            ),
            (0, 0, 0)
        );
        harness.stop().await;
    }

    /// A write timeout long enough for a test to hold requests behind the frame that sticks the
    /// writer before it runs out: it bounds a few steps on loopback, as [`WAIT`] bounds each one.
    /// Each test that uses it checks that nothing timed out before it was ready, so that a slow
    /// run fails rather than testing something else.
    const HOLDING_WRITE_TIMEOUT: Duration = Duration::from_secs(1);

    /// Duplicates of a request in flight that a test sends behind the frame that sticks the
    /// writer: twice what the outbound queue holds. Each is refused with the connection-level
    /// `error` as the connection reads it.
    fn duplicates() -> u64 {
        u64::try_from(2 * OUTBOUND_QUEUE_FRAMES).expect("the queue is small")
    }

    /// The duplicates a connection whose writer is stuck reads before it stops: as many as its
    /// queue holds, and the one it waits to queue.
    fn read_until_full() -> u64 {
        u64::try_from(OUTBOUND_QUEUE_FRAMES + 1).expect("the queue is small")
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn requests_held_when_a_write_times_out_are_abandoned_and_never_sent() {
        // The close waits for the client, which reads again only after the timeout.
        let limits = ConnectionLimits {
            close_timeout: NEVER,
            ..limits(64 << 10, HOLDING_WRITE_TIMEOUT)
        };
        let (handler, mut calls) = Scripted::new();
        let harness = Harness::start_with_limits(handler, limits).await;
        let mut slow = greeted_slow_reader(&harness).await;
        let mut finishing = Vec::new();
        for id in [2, 3] {
            slow.request(id, body(id)).await;
            finishing.push(calls.next().await);
        }
        harness.stick_writer(&mut calls, &mut slow, 1).await;
        for call in finishing {
            assert!(call.respond(small_response()), "the write timed out first");
        }
        let counters = harness
            .outbound_until(|counters| counters.held_requests() == 2)
            .await;
        assert_eq!(counters.write_timeouts(), 0, "the write timed out first");

        // The write times out with both held: they are let go, and end with their connection.
        harness
            .outbound_until(|counters| counters.write_timeouts() == 1)
            .await;
        harness
            .requests_until(|counters| counters.in_flight() == 0)
            .await;
        // Reading again, the client gets the rest of the frame that timed out, then the close
        // frame, and neither held response: `closed` fails on a text frame.
        assert_eq!(
            slow.next_message().await,
            response(1, bulky_response(CLOGGING_BYTES))
        );
        assert_eq!(slow.closed().await, Some(close_code::POLICY));
        harness.connections_until(0).await;

        let stats = harness.server().stats();
        let (requests, outbound) = (stats.requests(), stats.outbound());
        assert_eq!(
            (
                requests.accepted(),
                requests.responded(),
                requests.abandoned(),
                requests.in_flight()
            ),
            (3, 1, 2, 0),
            "the held requests were abandoned, not answered"
        );
        assert_eq!(
            (
                outbound.queued_bytes(),
                outbound.held_requests(),
                outbound.write_timeouts()
            ),
            (0, 0, 1)
        );
        harness.stop().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn a_write_timeout_frees_a_connection_waiting_to_queue_a_frame() {
        let limits = ConnectionLimits {
            close_timeout: NEVER,
            ..limits(64 << 10, HOLDING_WRITE_TIMEOUT)
        };
        let (handler, mut calls) = Scripted::new();
        let harness = Harness::start_with_limits(handler, limits).await;
        let mut slow = greeted_slow_reader(&harness).await;
        slow.request(2, body(2)).await;
        let second = calls.next().await;
        slow.request(3, body(3)).await;
        let mut third = calls.next().await;
        harness.stick_writer(&mut calls, &mut slow, 1).await;
        assert!(
            second.respond(small_response()),
            "the write timed out first"
        );
        harness
            .outbound_until(|counters| counters.held_requests() == 1)
            .await;
        // Refusals fill the queue until the connection waits to queue one more, and so stops
        // reading, with request 2 held and request 3 running.
        for _ in 0..duplicates() {
            slow.request(3, body(3)).await;
        }
        harness
            .requests_until(|counters| counters.refused() >= read_until_full())
            .await;
        assert_eq!(
            harness.server().stats().outbound().write_timeouts(),
            0,
            "the write timed out first"
        );

        // The writer, timed out, drops the queued refusals, so the connection queues the one it
        // waited on, sees the timeout, and closes without reading further.
        harness
            .outbound_until(|counters| counters.write_timeouts() == 1)
            .await;
        third.dropped().await;
        assert!(third.token.is_cancelled());
        assert_eq!(
            slow.next_message().await,
            response(1, bulky_response(CLOGGING_BYTES))
        );
        assert_eq!(slow.closed().await, Some(close_code::POLICY));
        harness.connections_until(0).await;

        let stats = harness.server().stats();
        let (requests, outbound) = (stats.requests(), stats.outbound());
        assert_eq!(
            (
                requests.accepted(),
                requests.responded(),
                requests.abandoned(),
                requests.refused(),
                requests.in_flight()
            ),
            (3, 1, 2, read_until_full(), 0)
        );
        assert_eq!(
            (
                outbound.queued_bytes(),
                outbound.held_requests(),
                outbound.write_timeouts()
            ),
            (0, 0, 1)
        );
        harness.stop().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn requests_held_at_shutdown_are_abandoned_and_released() {
        let (handler, mut calls) = Scripted::new();
        let harness = Harness::start_with_limits(handler, limits(64 << 10, NEVER)).await;
        let mut slow = greeted_slow_reader(&harness).await;
        let mut finishing = Vec::new();
        for id in [2, 3] {
            slow.request(id, body(id)).await;
            finishing.push(calls.next().await);
        }
        slow.request(4, body(4)).await;
        let mut running = calls.next().await;
        harness.stick_writer(&mut calls, &mut slow, 1).await;
        for call in finishing {
            assert!(call.respond(small_response()));
        }
        harness
            .outbound_until(|counters| counters.held_requests() == 2)
            .await;

        // Neither the stuck frame nor the close frame behind it can be written, so the connection
        // gives up after the close timeout; `stop` checks that nothing is queued, held or in
        // flight.
        let state = Arc::clone(harness.state());
        harness.stop().await;
        running.dropped().await;
        assert!(running.token.is_cancelled());
        let requests = state.request_stats.snapshot();
        assert_eq!(
            (
                requests.accepted(),
                requests.responded(),
                requests.abandoned()
            ),
            (4, 1, 3)
        );
        assert_eq!(state.outbound_stats.snapshot().write_timeouts(), 0);
        drop(slow);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn requests_held_when_their_client_sends_its_close_are_abandoned() {
        let (handler, mut calls) = Scripted::new();
        let harness = Harness::start_with_limits(handler, limits(64 << 10, NEVER)).await;
        let mut slow = greeted_slow_reader(&harness).await;
        slow.request(2, body(2)).await;
        let second = calls.next().await;
        harness.stick_writer(&mut calls, &mut slow, 1).await;
        assert!(second.respond(small_response()));
        harness
            .outbound_until(|counters| counters.held_requests() == 1)
            .await;
        // The client closes without reading: the server reads the close, and gives up on
        // writing after the close timeout.
        slow.send_close().await;
        harness.connections_until(0).await;

        let stats = harness.server().stats();
        let (requests, outbound) = (stats.requests(), stats.outbound());
        assert_eq!(
            (
                requests.in_flight(),
                requests.responded(),
                requests.abandoned()
            ),
            (0, 1, 1)
        );
        assert_eq!((outbound.queued_bytes(), outbound.held_requests()), (0, 0));
        drop(slow);
        harness.stop().await;
    }

    /// The connection-level `error` that refuses a request whose ID is in flight.
    fn duplicate(id: u32) -> ServerMessage {
        ServerMessage::Error {
            message: format!("request {id} is already in flight; the new request was dropped"),
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn a_cancel_read_just_after_its_held_response_is_released_is_ignored() {
        // Room for the frame that sticks the writer and nothing more, so that request 2's
        // response is held until that frame has been written.
        let budget = clogging_frame_len(1);
        let (handler, mut calls) = Scripted::new();
        let harness = Harness::start_with_limits(handler, limits(budget, NEVER)).await;
        let mut slow = greeted_slow_reader(&harness).await;
        slow.request(2, body(2)).await;
        let second = calls.next().await;
        slow.request(3, body(3)).await;
        let mut third = calls.next().await;
        harness.stick_writer(&mut calls, &mut slow, 1).await;
        assert!(second.respond(small_response()));
        harness
            .outbound_until(|counters| counters.held_requests() == 1)
            .await;
        // Duplicates of request 3 fill the queue with refusals until the connection stops
        // reading, as many as the queue holds and the one it waits to queue, and the cancel of
        // request 2 waits unread behind the rest.
        for _ in 0..duplicates() {
            slow.request(3, body(3)).await;
        }
        harness
            .requests_until(|counters| counters.refused() >= read_until_full())
            .await;
        slow.cancel(2).await;

        // Reading again frees the queue, and the room and the unread frames are there at once.
        // The held response goes first, so the cancel finds request 2 answered, and is ignored.
        assert_eq!(
            slow.next_message().await,
            response(1, bulky_response(CLOGGING_BYTES))
        );
        for _ in 0..read_until_full() {
            assert_eq!(slow.next_message().await, duplicate(3));
        }
        assert_eq!(slow.next_message().await, response(2, small_response()));
        for _ in read_until_full()..duplicates() {
            assert_eq!(slow.next_message().await, duplicate(3));
        }
        assert_eq!(slow.ping(4).await, [], "request 2 had a second answer");
        slow.cancel(3).await;
        third.dropped().await;
        assert_eq!(slow.next_message().await, cancelled(3));

        let requests = harness.server().stats().requests();
        assert_eq!(
            (
                requests.accepted(),
                requests.responded(),
                requests.cancelled(),
                requests.refused()
            ),
            (3, 2, 1, duplicates())
        );
        slow.close().await;
        harness.stop().await;
    }
}
