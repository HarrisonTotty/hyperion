//! The WebSocket endpoint: one task per connection, speaking `hyperion-protocol`.
//!
//! The connection task owns everything about its socket (plan 04, design note 22). It reads the
//! client's frames, answers `hello` and `ping` itself, and runs each request as a task in a
//! [`JoinSet`](tokio::task::JoinSet) through [`Requests`], waiting on frames, finished requests,
//! room in its outbound queue, its writer and the server's shutdown in one `select!`. A writer
//! task, owned and joined by the connection task, sends what the connection queues for it
//! ([`outbound`](crate::outbound)). When the client reads too slowly, the queue's byte budget
//! holds finished requests back while the connection reads on, and a full count of queued frames
//! stops it reading until the queue drains; a frame not written within the write timeout closes
//! the connection with a policy violation. Closing the connection, for whatever reason, cancels
//! every request it has in flight. A request answered in parts (`sky`, rendering plan R06,
//! R06.T11.d) hands each answer before its last to the connection, which streams it as a bulk
//! answer is streamed, its chunks and then its `partial_response`, in order with the request's
//! later answers and its terminal message.

use std::collections::VecDeque;
use std::fmt;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use axum::Extension;
use axum::body::Bytes;
use axum::extract::State;
use axum::extract::connect_info::ConnectInfo;
use axum::extract::ws::{CloseFrame, Message, Utf8Bytes, WebSocket, WebSocketUpgrade, close_code};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use futures_util::StreamExt;
use futures_util::stream::SplitStream;
use hyperion_protocol::{ClientMessage, PROTOCOL_VERSION, RequestBody, RequestId, ServerMessage};
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tracing::Instrument;

use crate::AppState;
use crate::bulk::BulkPayload;
use crate::connections::{Closing, ConnectionGuard};
use crate::limits::{
    CLOSE_TIMEOUT, MAX_CONSECUTIVE_MALFORMED_FRAMES, MAX_INBOUND_FRAME_BYTES, OUTBOUND_BYTES,
    WRITE_TIMEOUT,
};
use crate::outbound::{self, Held, Outbound, WriterStopped};
use crate::requests::{
    self, Handshake, Inbound, PARTIALS_QUEUED, Partial, Requests, Settled, Tally, to_frame,
};
use crate::subscriptions::{Large, Offered, Ready, Step, Subscriptions};

/// The limits a connection enforces on writing to its client.
///
/// The server's are those of [`limits`](crate::limits), which [`ConnectionLimits::default`]
/// gives; unit tests shorten them, so that a slow reader's test need not wait out
/// [`WRITE_TIMEOUT`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ConnectionLimits {
    /// Payload bytes the outbound queue holds before finished requests are held back.
    pub(crate) outbound_bytes: usize,
    /// Longest one frame may take to be written.
    pub(crate) write_timeout: Duration,
    /// Longest a closing connection may take to send what it has queued and finish closing.
    pub(crate) close_timeout: Duration,
}

impl Default for ConnectionLimits {
    fn default() -> Self {
        Self {
            outbound_bytes: OUTBOUND_BYTES,
            write_timeout: WRITE_TIMEOUT,
            close_timeout: CLOSE_TIMEOUT,
        }
    }
}

/// Accepts a WebSocket, unless the server is shutting down. A client message or frame above
/// [`MAX_INBOUND_FRAME_BYTES`] fails the read, which ends the connection.
pub(crate) async fn upgrade(
    State(state): State<Arc<AppState>>,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    ws: WebSocketUpgrade,
) -> Response {
    // Taken before the upgrade, so that shutdown also waits for a connection still upgrading. If
    // the upgrade fails, axum drops the callback and the guard with it.
    let Some(guard) = state.connections.open() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "the server is shutting down",
        )
            .into_response();
    };
    let peer = connect_info.map(|Extension(ConnectInfo(peer))| peer);
    ws.max_message_size(MAX_INBOUND_FRAME_BYTES)
        .max_frame_size(MAX_INBOUND_FRAME_BYTES)
        .on_upgrade(move |socket| serve(socket, state, guard, peer))
}

/// The connection task.
async fn serve(
    socket: WebSocket,
    state: Arc<AppState>,
    guard: ConnectionGuard,
    peer: Option<SocketAddr>,
) {
    let span = tracing::info_span!("connection", id = guard.id(), peer = tracing::field::Empty);
    if let Some(peer) = peer {
        span.record("peer", tracing::field::display(peer));
    }
    Connection::run(socket, state, guard.closing())
        .instrument(span)
        .await;
    // Dropped only now, with the writer joined, so that shutdown waits for all of it.
    drop(guard);
}

/// The answer to `hello`.
#[must_use]
fn welcome() -> ServerMessage {
    ServerMessage::Welcome {
        server_version: env!("CARGO_PKG_VERSION").to_owned(),
        protocol_version: PROTOCOL_VERSION,
        generator_version: hyperion_sim::GENERATOR_VERSION.get(),
    }
}

/// Why a connection ended.
#[derive(Debug)]
enum End {
    /// The client sent a close frame or the stream ended.
    ClientClosed,
    /// Reading failed, as it does for a frame above [`MAX_INBOUND_FRAME_BYTES`].
    ReadFailed(axum::Error),
    /// The writer stopped, because writing to the socket failed.
    WriterGone,
    /// A frame could not be written within the write timeout: the client has stopped reading.
    WriteTimedOut,
    /// [`MAX_CONSECUTIVE_MALFORMED_FRAMES`] arrived in a row.
    TooManyMalformed,
    /// The server is shutting down.
    ShuttingDown,
}

impl End {
    /// The close frame the server sends, when it is the one closing.
    #[must_use]
    fn close_frame(&self) -> Option<CloseFrame> {
        match self {
            Self::TooManyMalformed => Some(CloseFrame {
                code: close_code::POLICY,
                reason: Utf8Bytes::from_static("too many malformed frames"),
            }),
            Self::WriteTimedOut => Some(CloseFrame {
                code: close_code::POLICY,
                reason: Utf8Bytes::from_static("the client is not reading"),
            }),
            Self::ShuttingDown => Some(CloseFrame {
                code: close_code::AWAY,
                reason: Utf8Bytes::from_static("the server is shutting down"),
            }),
            Self::ClientClosed | Self::ReadFailed(_) | Self::WriterGone => None,
        }
    }
}

impl From<WriterStopped> for End {
    fn from(stopped: WriterStopped) -> Self {
        match stopped {
            WriterStopped::Failed => Self::WriterGone,
            WriterStopped::TimedOut => Self::WriteTimedOut,
        }
    }
}

impl fmt::Display for End {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ClientClosed => f.write_str("closed by the client"),
            Self::ReadFailed(error) => write!(f, "reading failed: {error}"),
            Self::WriterGone => f.write_str("writing failed"),
            Self::WriteTimedOut => f.write_str("a frame was not written in time"),
            Self::TooManyMalformed => f.write_str("too many malformed frames"),
            Self::ShuttingDown => f.write_str("the server is shutting down"),
        }
    }
}

/// What woke the connection.
enum Event {
    /// The oldest held request's frame fits the outbound queue now.
    Room,
    /// A subscription has a push pending, or a pending push that had no room may have some now.
    Pushes,
    /// A large notification is serialised.
    Serialised(Ready),
    /// A request answered in parts has an answer before its last to stream.
    Partial(Partial),
    Finished(Result<(tokio::task::Id, requests::Finished), tokio::task::JoinError>),
    /// The streaming request's next chunk, or its terminal frame, may be queued.
    Chunk,
    Frame(Option<Result<Message, axum::Error>>),
}

/// A large notification being serialised on the CPU pool.
type Serialising = Pin<Box<dyn Future<Output = Ready> + Send>>;

/// The reading side of a connection and everything it owns.
struct Connection {
    /// The server's state, whose pool serialises a large notification.
    state: Arc<AppState>,
    requests: Requests,
    /// The answers before their last that requests answered in parts hand over (R06.T11.d).
    partials: tokio::sync::mpsc::Receiver<Partial>,
    /// The connection's subscriptions, which end with it (rendering plan R03, R03.T5.b).
    subscriptions: Subscriptions,
    /// The smallest pending push that found no room in `outbound`, in bytes, if one did.
    push_waits_for: Option<usize>,
    /// The large notification being serialised, one at a time, which the connection polls with
    /// its socket so that it reads on meanwhile.
    serialising: Option<Serialising>,
    outbound: Outbound,
    /// Finished requests waiting for room in `outbound`.
    held: Held,
    /// Answers in bulk, in the order they came, each streaming its chunks before its frame, a
    /// request's terminal frame or a `partial_response`; the first streams now (rendering plan
    /// R03, R03.T10.b; R06.T11.d).
    streams: VecDeque<Stream>,
    closing: Closing,
    handshake: Handshake,
    malformed_in_a_row: u32,
}

impl Connection {
    /// Runs a connection from its upgrade to its close.
    async fn run(socket: WebSocket, state: Arc<AppState>, closing: Closing) {
        tracing::info!("client connected");
        let limits = state.connection_limits;
        let (sink, mut stream) = socket.split();
        let (outbound, writer) = outbound::open(
            state.outbound_stats.clone(),
            limits.outbound_bytes,
            limits.write_timeout,
        );
        let mut writer = tokio::spawn(writer.run(sink).in_current_span());
        let (to_stream, partials) = tokio::sync::mpsc::channel(PARTIALS_QUEUED);
        let mut connection = Self {
            state: Arc::clone(&state),
            requests: Requests::new(Arc::clone(&state), to_stream),
            partials,
            subscriptions: Subscriptions::new(),
            push_waits_for: None,
            serialising: None,
            outbound,
            held: Held::new(state.outbound_stats.clone()),
            streams: VecDeque::new(),
            closing,
            handshake: Handshake::Pending,
            malformed_in_a_row: 0,
        };
        let end = connection.read(&mut stream).await;
        if matches!(end, End::WriteTimedOut) {
            state.outbound_stats.write_timed_out();
        }
        let Self {
            state: _,
            mut requests,
            subscriptions,
            outbound,
            held,
            streams,
            ..
        } = connection;
        // Every subscription ends with its socket: its topic's task is ended and nothing it had
        // pending is sent.
        drop(subscriptions);
        // Held and streaming requests are still in flight, and are abandoned with the rest.
        drop(held);
        drop(streams);
        requests.close().await;
        let closed = timeout(
            limits.close_timeout,
            close(end.close_frame(), outbound, &mut stream, &mut writer),
        )
        .await;
        let joined = if let Ok(joined) = closed {
            joined
        } else {
            tracing::debug!("the client did not finish closing in time; dropping the socket");
            writer.abort();
            // An aborted task ends at its next `.await`, so this returns at once.
            writer.await
        };
        if let Err(error) = joined
            && error.is_panic()
        {
            tracing::error!(%error, "the connection's writer panicked");
        }
        tracing::info!(reason = %end, "client disconnected");
    }

    /// Reads and answers frames until the connection ends, and says why it did.
    async fn read(&mut self, stream: &mut SplitStream<WebSocket>) -> End {
        loop {
            let next_held = self.held.next_len();
            let room = self.outbound.room_for(next_held.unwrap_or_default());
            let push_waits_for = self.push_waits_for;
            let push_room = self.outbound.room_for(push_waits_for.unwrap_or_default());
            let next_chunk = self.streams.front_mut().map(Stream::next_len);
            let chunk_room = self.outbound.bulk_room_for(next_chunk.unwrap_or_default());
            // The answers in parts the connection holds are few: a request with more waits.
            let partials_room = self.partials_streaming() < PARTIALS_QUEUED;
            let event = tokio::select! {
                biased;
                () = self.closing.wait() => return End::ShuttingDown,
                stopped = self.outbound.writer_stopped() => return End::from(stopped),
                // Finished requests first, so that their places are free before more are read,
                // as long as the queue has room for them. Frames are read on regardless, so that
                // `ping` and `cancel` are answered while it has none.
                () = room, if next_held.is_some() => Event::Room,
                // A request's answers in parts before its end, which was sent after them.
                Some(partial) = self.partials.recv(), if partials_room => Event::Partial(partial),
                Some(joined) = self.requests.join_next(), if self.requests.has_tasks() => {
                    Event::Finished(joined)
                }
                frame = stream.next() => Event::Frame(frame),
                ready = async {
                    match self.serialising.as_mut() {
                        Some(serialising) => serialising.await,
                        None => std::future::pending().await,
                    }
                }, if self.serialising.is_some() => Event::Serialised(ready),
                // Pushes after frames, so that a topic pushing fast cannot keep `ping`, `cancel`
                // and the close from being read.
                () = push_room, if push_waits_for.is_some() => Event::Pushes,
                () = self.subscriptions.woken() => Event::Pushes,
                // A bulk transfer last, a chunk at a time, so that everything else overtakes it
                // (Design note 11).
                () = chunk_room, if next_chunk.is_some() => Event::Chunk,
            };
            let handled = match event {
                Event::Room => match self.held.pop() {
                    Some(held) => self.end(held).await,
                    None => Ok(()),
                },
                Event::Pushes => self.flush_pushes().await,
                Event::Serialised(ready) => self.on_serialised(ready).await,
                Event::Chunk => self.stream_chunk().await,
                Event::Partial(partial) => {
                    self.stream_partial(partial);
                    Ok(())
                }
                Event::Finished(joined) => {
                    // The request's last answers in parts, sent before it ended, go first.
                    while let Ok(partial) = self.partials.try_recv() {
                        self.stream_partial(partial);
                    }
                    match self.requests.settle(joined) {
                        Some(mut settled) => match settled.take_bulk() {
                            // An answer in bulk streams its chunks first, after any streaming before
                            // it, and its terminal frame then joins the others.
                            Some(bulk) => {
                                self.streams.push_back(Stream::new(settled, Some(&bulk)));
                                Ok(())
                            }
                            // A request whose earlier answers are still streaming ends after them.
                            None if self
                                .streams
                                .iter()
                                .any(|stream| stream.id() == settled.id()) =>
                            {
                                self.streams.push_back(Stream::new(settled, None));
                                Ok(())
                            }
                            None => self.queue_terminal(settled).await,
                        },
                        None => Ok(()),
                    }
                }
                Event::Frame(None | Some(Ok(Message::Close(_)))) => Err(End::ClientClosed),
                Event::Frame(Some(Err(error))) => Err(End::ReadFailed(error)),
                Event::Frame(Some(Ok(Message::Text(text)))) => {
                    self.on_inbound(requests::parse(text.as_str())).await
                }
                // Binary frames are reserved for bulk payloads from the server (plan 04,
                // Extending the convention); a client has no use for them yet.
                Event::Frame(Some(Ok(Message::Binary(_)))) => {
                    self.on_inbound(Inbound::Malformed {
                        message: "binary frames are not used".to_owned(),
                    })
                    .await
                }
                // The WebSocket layer answers pings itself.
                Event::Frame(Some(Ok(Message::Ping(_) | Message::Pong(_)))) => Ok(()),
            };
            if let Err(end) = handled {
                return end;
            }
        }
    }

    /// Answers one frame, and closes the connection once too many malformed ones have arrived in
    /// a row.
    async fn on_inbound(&mut self, inbound: Inbound) -> Result<(), End> {
        match inbound.tally() {
            Tally::Reset => self.malformed_in_a_row = 0,
            Tally::Count => self.malformed_in_a_row = self.malformed_in_a_row.saturating_add(1),
            Tally::Leave => {}
        }
        let reply = match inbound {
            Inbound::Message(message) => self.on_message(message),
            Inbound::Unparsed { id, error } => {
                Some(self.requests.reject(id, error, self.handshake))
            }
            Inbound::Malformed { message } => {
                tracing::warn!(%message, "malformed frame");
                Some(to_frame(&ServerMessage::Error { message }))
            }
        };
        if let Some(frame) = reply {
            self.push(Message::Text(frame.into())).await?;
        }
        if self.malformed_in_a_row >= MAX_CONSECUTIVE_MALFORMED_FRAMES {
            tracing::warn!(
                count = self.malformed_in_a_row,
                "closing after too many malformed frames in a row"
            );
            return Err(End::TooManyMalformed);
        }
        Ok(())
    }

    /// Answers a well-formed message, if it has an immediate answer.
    fn on_message(&mut self, message: ClientMessage) -> Option<String> {
        match message {
            ClientMessage::Hello { client_version } => {
                tracing::debug!(%client_version, "hello");
                self.handshake = Handshake::Done;
                Some(to_frame(&welcome()))
            }
            ClientMessage::Ping { nonce } => Some(to_frame(&ServerMessage::Pong { nonce })),
            ClientMessage::Request {
                id,
                body: RequestBody::Subscribe(request),
            } => {
                self.requests
                    .submit_subscribe(id, request, self.handshake, &mut self.subscriptions)
            }
            ClientMessage::Request {
                id,
                body: RequestBody::Unsubscribe(request),
            } => Some(self.requests.unsubscribe(
                id,
                request,
                self.handshake,
                &mut self.subscriptions,
            )),
            ClientMessage::Request {
                id,
                body: RequestBody::SceneCameras(request),
            } => self
                .requests
                .scene_cameras(id, request, self.handshake, &self.subscriptions),
            ClientMessage::Request { id, body } => self.requests.submit(id, body, self.handshake),
            ClientMessage::Cancel { id } => {
                let cancelled = self.requests.cancel(id);
                if cancelled.is_some() {
                    // A subscription still opening ends with its cancelled `subscribe`.
                    self.subscriptions.failed(id);
                }
                // A held frame of the request just cancelled is dropped: `cancelled` ended it, and
                // so is a transfer's rest, of which no further chunk is queued.
                self.held
                    .retain(|settled| self.requests.is_in_flight(settled));
                self.streams
                    .retain(|stream| stream.is_current(&self.requests));
                cancelled
            }
        }
    }

    /// Ends a settled request by queuing its terminal frame, unless a `cancel` ended it first;
    /// a `subscribe` answered with `subscribed` makes its subscription live once the answer is
    /// queued, so that its notifications follow it, and one that failed ends its subscription.
    async fn end(&mut self, settled: Settled) -> Result<(), End> {
        let (id, responded) = (settled.id(), settled.responded());
        let Some(frame) = self.requests.end(settled) else {
            return Ok(());
        };
        self.push(Message::Text(frame.into())).await?;
        if responded {
            self.subscriptions.went_live(id);
        } else {
            self.subscriptions.failed(id);
        }
        Ok(())
    }

    /// Streams `partial` after the answers before it, unless its request has ended since it was
    /// sent: an answer sent before a cancel is not sent.
    fn stream_partial(&mut self, partial: Partial) {
        if self.requests.is_current(partial.id, partial.serial) {
            self.streams.push_back(Stream::partial(partial));
        }
    }

    /// The answers in parts streaming or queued to stream (R06.T11.d).
    fn partials_streaming(&self) -> usize {
        self.streams
            .iter()
            .filter(|stream| matches!(stream.ending, StreamEnd::Partial { .. }))
            .count()
    }

    /// Queues the streaming answer's next chunk, or, once all are queued, its frame: a terminal
    /// frame, which ends the request (Design note 10), or a `partial_response`, queued at once so
    /// that it precedes the request's next answer (R06.T11.d).
    async fn stream_chunk(&mut self) -> Result<(), End> {
        let Some(stream) = self.streams.front_mut() else {
            return Ok(());
        };
        if let Some(chunk) = stream.chunks.next() {
            return self.push_bulk(chunk).await;
        }
        let stream = self
            .streams
            .pop_front()
            .expect("the front stream was read just above");
        match stream.ending {
            StreamEnd::Terminal(settled) => self.queue_terminal(settled).await,
            StreamEnd::Partial { id, serial, frame } => {
                if self.requests.is_current(id, serial) {
                    self.push(Message::Text(frame.into())).await?;
                }
                Ok(())
            }
        }
    }

    /// Queues a finished request's terminal frame at once if nothing is held before it and the
    /// queue has room for it, so that terminal frames are queued in the order their requests
    /// finished; holds it otherwise.
    async fn queue_terminal(&mut self, settled: Settled) -> Result<(), End> {
        if self.held.is_empty() && self.outbound.has_room_for(settled.frame_len()) {
            self.end(settled).await
        } else {
            self.held.push(settled);
            Ok(())
        }
    }

    /// Queues a bulk frame, waiting while the queue holds its full count of frames, unless the
    /// server starts shutting down meanwhile.
    async fn push_bulk(&mut self, frame: Bytes) -> Result<(), End> {
        tokio::select! {
            biased;
            () = self.closing.wait() => Err(End::ShuttingDown),
            sent = self.outbound.send_bulk(frame) => sent.map_err(|_| End::WriterGone),
        }
    }

    /// Queues every live subscription's pending push that the outbound queue has room for, each
    /// as a `notification` numbered with its subscription's next sequence, and leaves the rest
    /// pending until room frees (rendering plan R03, Design note 5). A large one is serialised off
    /// the connection's path, so that frames are read meanwhile, and queued once it is back.
    async fn flush_pushes(&mut self) -> Result<(), End> {
        self.push_waits_for = None;
        let mut after = None;
        loop {
            let outbound = &self.outbound;
            let Some(step) = self.subscriptions.next_step(
                after,
                |bytes| outbound.has_room_for(bytes),
                if self.serialising.is_none() {
                    Large::MaySerialise
                } else {
                    Large::Hold
                },
            ) else {
                break;
            };
            after = Some(step.id());
            match step {
                Step::Send(_, frame) => self.push(Message::Text(frame.into())).await?,
                Step::Waiting(_, bytes) => self.waits_for(bytes),
                Step::Serialise(unsent) if unsent.is_large() => {
                    self.serialising =
                        Some(Box::pin(unsent.serialise_on(Arc::clone(&self.state.pool))));
                }
                Step::Serialise(unsent) => self.offer(unsent.serialise()).await?,
            }
        }
        // A subscription its topic gave up ends once its last push is queued.
        while let Some((id, frame)) = self.subscriptions.next_failed() {
            let bytes = frame.len();
            if !self.outbound.has_room_for(bytes) {
                self.waits_for(bytes);
                break;
            }
            self.subscriptions.ended_by_topic(id);
            self.push(Message::Text(frame.into())).await?;
        }
        Ok(())
    }

    /// Takes a large notification back from the pool, and queues it and whatever else is
    /// pending that has room.
    async fn on_serialised(&mut self, ready: Ready) -> Result<(), End> {
        self.serialising = None;
        self.offer(ready).await?;
        self.flush_pushes().await
    }

    /// Queues a serialised notification if the outbound queue has room for it, and leaves it
    /// waiting for room otherwise.
    async fn offer(&mut self, ready: Ready) -> Result<(), End> {
        let outbound = &self.outbound;
        match self
            .subscriptions
            .offer(ready, |bytes| outbound.has_room_for(bytes))
        {
            Offered::Send(frame) => self.push(Message::Text(frame.into())).await?,
            Offered::Waiting(bytes) => self.waits_for(bytes),
            Offered::Nothing => {}
        }
        Ok(())
    }

    /// Notes that a push waits for room for `bytes`.
    fn waits_for(&mut self, bytes: usize) {
        self.push_waits_for = Some(self.push_waits_for.map_or(bytes, |w| w.min(bytes)));
    }

    /// Queues a frame for the writer, waiting while the queue holds its full count of frames,
    /// unless the server starts shutting down meanwhile.
    async fn push(&mut self, message: Message) -> Result<(), End> {
        tokio::select! {
            biased;
            () = self.closing.wait() => Err(End::ShuttingDown),
            sent = self.outbound.send(message) => sent.map_err(|_| End::WriterGone),
        }
    }
}

/// An answer in bulk: its chunks not yet queued, then its frame.
struct Stream {
    ending: StreamEnd,
    chunks: std::iter::Peekable<Box<dyn Iterator<Item = Bytes> + Send>>,
}

/// What follows a streamed answer's chunks.
enum StreamEnd {
    /// The request's terminal frame: the request is in flight until it is queued.
    Terminal(Settled),
    /// A `partial_response` of the request `id`, the connection's `serial`th (R06.T11.d).
    Partial {
        id: RequestId,
        serial: u64,
        frame: String,
    },
}

impl Stream {
    /// The stream of `settled`'s answer, whose payload is `bulk`; no chunk for none.
    #[must_use]
    fn new(settled: Settled, bulk: Option<&BulkPayload>) -> Self {
        let chunks: Box<dyn Iterator<Item = Bytes> + Send> = match bulk {
            Some(bulk) => Box::new(bulk.frames(settled.id())),
            None => Box::new(std::iter::empty()),
        };
        Self {
            ending: StreamEnd::Terminal(settled),
            chunks: chunks.peekable(),
        }
    }

    /// The stream of an answer before its request's last: its chunks, then its frame.
    #[must_use]
    fn partial(partial: Partial) -> Self {
        let Partial {
            id,
            serial,
            frame,
            bulk,
        } = partial;
        let chunks: Box<dyn Iterator<Item = Bytes> + Send> = match bulk {
            Some(bulk) => Box::new(bulk.frames(id)),
            None => Box::new(std::iter::empty()),
        };
        Self {
            ending: StreamEnd::Partial { id, serial, frame },
            chunks: chunks.peekable(),
        }
    }

    /// The request the answer is of.
    #[must_use]
    fn id(&self) -> RequestId {
        match &self.ending {
            StreamEnd::Terminal(settled) => settled.id(),
            StreamEnd::Partial { id, .. } => *id,
        }
    }

    /// Whether its request is still in flight, so that the rest of the answer is still to send.
    #[must_use]
    fn is_current(&self, requests: &Requests) -> bool {
        match &self.ending {
            StreamEnd::Terminal(settled) => requests.is_in_flight(settled),
            StreamEnd::Partial { id, serial, .. } => requests.is_current(*id, *serial),
        }
    }

    /// The bytes of what is queued next: the next chunk, or 0 for the terminal frame, which is
    /// queued as soon as the last chunk is.
    fn next_len(&mut self) -> usize {
        self.chunks.peek().map_or(0, Bytes::len)
    }
}

/// Finishes closing a connection whose requests have been cancelled: sends the server's close
/// frame if it is the one closing, lets the writer drain, and joins it.
///
/// After its close frame the server reads on until the client's answering close. That consumes
/// whatever the client sent meanwhile, so that the socket is shut and not reset, which could
/// lose the close frame on its way to the client.
async fn close(
    frame: Option<CloseFrame>,
    outbound: Outbound,
    stream: &mut SplitStream<WebSocket>,
    writer: &mut JoinHandle<()>,
) -> Result<(), tokio::task::JoinError> {
    if let Some(frame) = frame
        && outbound.send(Message::Close(Some(frame))).await.is_ok()
    {
        while let Some(Ok(message)) = stream.next().await {
            if matches!(message, Message::Close(_)) {
                break;
            }
        }
    }
    // The writer ends once its queue is empty and closed.
    drop(outbound);
    writer.await
}

#[cfg(test)]
mod tests {
    use hyperion_protocol::{ErrorCode, RequestId};

    use super::*;
    use crate::limits::OUTBOUND_QUEUE_FRAMES;
    use crate::testing::{
        CLOGGING_BYTES, Call, Calls, Client, Harness, NEVER, Received, Scripted, body,
        bulky_response, small_response,
    };
    use crate::{Server, ServerConfig};

    #[test]
    fn hello_is_welcomed_with_both_versions() {
        assert_eq!(
            welcome(),
            ServerMessage::Welcome {
                server_version: env!("CARGO_PKG_VERSION").to_owned(),
                protocol_version: PROTOCOL_VERSION,
                generator_version: hyperion_sim::GENERATOR_VERSION.get(),
            }
        );
    }

    #[tokio::test]
    async fn the_server_holds_its_connections_to_the_limits_of_the_limits_module() {
        let data_dir = tempfile::tempdir().unwrap();
        let server = Server::start(ServerConfig::builder().data_dir(data_dir.path()).build())
            .await
            .unwrap();
        assert_eq!(
            server.state().connection_limits,
            ConnectionLimits {
                outbound_bytes: OUTBOUND_BYTES,
                write_timeout: WRITE_TIMEOUT,
                close_timeout: CLOSE_TIMEOUT,
            }
        );
        server.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn ping_is_answered_with_matching_pong_before_and_after_hello() {
        let (handler, _calls) = Scripted::new();
        let harness = Harness::start(handler).await;
        let mut client = harness.connect().await;
        client.send(&ClientMessage::Ping { nonce: u32::MAX }).await;
        assert_eq!(
            client.next_message().await,
            ServerMessage::Pong { nonce: u32::MAX }
        );
        client.hello().await;
        client.send(&ClientMessage::Ping { nonce: 0 }).await;
        assert_eq!(
            client.next_message().await,
            ServerMessage::Pong { nonce: 0 }
        );
        client.close().await;
        harness.stop().await;
    }

    #[tokio::test]
    async fn a_connection_is_refused_once_the_server_is_closing() {
        let (handler, _calls) = Scripted::new();
        let harness = Harness::start(handler).await;
        // Serving goes on, as it does between shutdown's start and the listener's close.
        tokio::time::timeout(
            crate::testing::WAIT,
            harness.state().connections.close_all(),
        )
        .await
        .expect("no connection is open");
        let refused = tokio::time::timeout(
            crate::testing::WAIT,
            tokio_tungstenite::connect_async(harness.url()),
        )
        .await
        .expect("timed out connecting");
        match refused {
            Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
                assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
            }
            other => panic!("expected 503, got {other:?}"),
        }
        assert_eq!(harness.server().stats().connections(), 0);
        harness.stop().await;
    }

    #[tokio::test]
    async fn a_client_that_disconnects_mid_request_cancels_it() {
        let (handler, mut calls) = Scripted::new();
        let harness = Harness::start(handler).await;
        let mut client = harness.connect().await;
        client.hello().await;
        client.request(1, body(1)).await;
        let mut call = calls.next().await;
        // No close frame: the TCP connection simply ends.
        drop(client);
        call.dropped().await;
        assert!(call.token.is_cancelled());
        harness.connections_until(0).await;
        let counters = harness.server().stats().requests();
        assert_eq!(
            (
                counters.accepted(),
                counters.abandoned(),
                counters.in_flight()
            ),
            (1, 1, 0)
        );
        // The server is unharmed.
        let mut other = harness.connect().await;
        other.hello().await;
        assert_eq!(other.ping(1).await, []);
        other.close().await;
        harness.stop().await;
    }

    #[tokio::test]
    async fn a_request_in_flight_when_the_client_closes_is_cancelled_and_never_answered() {
        let (handler, mut calls) = Scripted::new();
        let harness = Harness::start(handler).await;
        let mut client = harness.connect().await;
        client.hello().await;
        client.request(1, body(1)).await;
        let mut call = calls.next().await;
        // `close` fails the test on any text frame, the request's answer included.
        assert_eq!(
            client.close().await,
            None,
            "the server echoes the empty close"
        );
        call.dropped().await;
        assert!(call.token.is_cancelled());
        assert!(!call.respond(crate::testing::small_response()));
        harness.connections_until(0).await;
        assert_eq!(harness.server().stats().requests().abandoned(), 1);
        harness.stop().await;
    }

    #[tokio::test]
    async fn a_request_sent_just_before_the_close_is_never_answered() {
        let (handler, mut calls) = Scripted::new();
        let harness = Harness::start(handler).await;
        let mut client = harness.connect().await;
        client.hello().await;
        client.request(1, body(1)).await;
        client.close().await;
        harness.connections_until(0).await;
        let counters = harness.server().stats().requests();
        assert_eq!((counters.accepted(), counters.abandoned()), (1, 1));
        // The handler may or may not have been reached before the close; if it was, it has been
        // dropped.
        while !calls.is_empty() {
            let mut call = calls.next().await;
            call.dropped().await;
            assert!(call.token.is_cancelled());
        }
        harness.stop().await;
    }

    #[tokio::test]
    async fn too_many_malformed_frames_close_with_a_policy_violation_and_cancel_requests() {
        let (handler, mut calls) = Scripted::new();
        let harness = Harness::start(handler).await;
        let mut client = harness.connect().await;
        client.hello().await;
        client.request(1, body(1)).await;
        client.request(2, body(2)).await;
        let mut running = [calls.next().await, calls.next().await];
        for _ in 1..MAX_CONSECUTIVE_MALFORMED_FRAMES {
            client.send_raw("not json").await;
            assert!(matches!(
                client.next_message().await,
                ServerMessage::Error { .. }
            ));
        }
        // A kind this server does not know neither counts nor starts the count again.
        client
            .send_raw(r#"{"type":"request","id":50,"body":{"kind":"warp"}}"#)
            .await;
        assert!(matches!(
            client.next_message().await,
            ServerMessage::RequestError { id: RequestId(50), error } if error.code == ErrorCode::Unsupported
        ));
        client.send_raw("not json").await;
        assert!(matches!(
            client.next_message().await,
            ServerMessage::Error { .. }
        ));
        assert_eq!(client.closed().await, Some(close_code::POLICY));
        for call in &mut running {
            call.dropped().await;
            assert!(call.token.is_cancelled());
        }
        harness.connections_until(0).await;
        assert_eq!(harness.server().stats().requests().abandoned(), 2);
        harness.stop().await;
    }

    #[tokio::test]
    async fn an_oversized_frame_closes_the_connection_and_cancels_its_requests() {
        let (handler, mut calls) = Scripted::new();
        let harness = Harness::start(handler).await;
        let mut client = harness.connect().await;
        client.hello().await;
        client.request(1, body(1)).await;
        let mut call = calls.next().await;
        client
            .send_raw(&"x".repeat(MAX_INBOUND_FRAME_BYTES + 1))
            .await;
        // The read fails, so there is no close code to report, only the end of the connection.
        client.drain().await;
        call.dropped().await;
        assert!(call.token.is_cancelled());
        harness.connections_until(0).await;
        assert_eq!(harness.server().stats().requests().abandoned(), 1);
        harness.stop().await;
    }

    #[tokio::test]
    async fn shutdown_closes_every_connection_and_cancels_its_requests() {
        let (handler, mut calls) = Scripted::new();
        let harness = Harness::start(handler).await;
        let state = Arc::clone(harness.state());
        let mut busy = harness.connect().await;
        busy.hello().await;
        busy.request(1, body(1)).await;
        let mut call = calls.next().await;
        let mut idle = harness.connect().await;
        idle.hello().await;
        // The clients read on, as a client does, and so answer the server's close.
        let busy = tokio::spawn(async move { busy.closed().await });
        let idle = tokio::spawn(async move { idle.closed().await });

        harness.stop().await;
        assert_eq!(busy.await.unwrap(), Some(close_code::AWAY));
        assert_eq!(idle.await.unwrap(), Some(close_code::AWAY));
        call.dropped().await;
        assert!(call.token.is_cancelled());
        assert_eq!(state.connections.open_count(), 0);
        let counters = state.request_stats.snapshot();
        assert_eq!((counters.abandoned(), counters.in_flight()), (1, 0));
        assert!(
            state.connections.open().is_none(),
            "no connection opens after shutdown"
        );
    }

    /// A handler that answers every request in parts (R06.T11.d): `parts` answers before its last,
    /// each a small body after a payload of two chunks whose bytes are the answer's number, then
    /// as `ends` says.
    #[derive(Debug)]
    struct InParts {
        parts: u8,
        ends: Ending,
    }

    /// How an [`InParts`] request ends.
    #[derive(Debug, Clone, Copy)]
    enum Ending {
        /// With a last answer as the others, in a response.
        Answered,
        /// Never.
        Hangs,
        /// With an error, which no chunk precedes.
        Fails,
    }

    /// Answer `n`'s payload: just over one chunk, every byte `n`.
    fn payload_of(n: u8) -> BulkPayload {
        BulkPayload::new(Bytes::from(vec![n; crate::bulk::CHUNK_PAYLOAD_BYTES + 7]))
            .expect("a payload of two chunks")
    }

    impl crate::requests::Handler for InParts {
        fn handle(
            &self,
            state: Arc<AppState>,
            _body: RequestBody,
            token: crate::compute::CancelToken,
            replies: crate::requests::Replies,
        ) -> crate::requests::HandlerFuture {
            let Self { parts, ends } = *self;
            Box::pin(async move {
                for n in 0..parts {
                    let answer = crate::bulk::Answer {
                        body: small_response(),
                        bulk: Some(payload_of(n)),
                    };
                    replies.send(&state.pool, &token, answer).await?;
                }
                match ends {
                    Ending::Answered => Ok(crate::bulk::Answer {
                        body: small_response(),
                        bulk: Some(payload_of(parts)),
                    }),
                    Ending::Hangs => std::future::pending().await,
                    Ending::Fails => Err(crate::requests::request_error(
                        ErrorCode::Internal,
                        "a deliberate failure after the answers in parts",
                    )),
                }
            })
        }
    }

    /// The frames of one answer in parts: each chunk's request, index, count and first byte, and
    /// then the message after them.
    async fn answer_in_parts(client: &mut Client) -> (Vec<[u32; 4]>, ServerMessage) {
        let mut chunks = Vec::new();
        loop {
            match client.next_frame().await {
                Received::Binary(bytes) => {
                    let field = |at: usize| {
                        u32::from_le_bytes(bytes[at..at + 4].try_into().expect("four bytes"))
                    };
                    chunks.push([
                        field(8),
                        field(12),
                        field(16),
                        u32::from(bytes[crate::bulk::HEADER_BYTES]),
                    ]);
                }
                Received::Message(message) => return (chunks, message),
            }
        }
    }

    /// A request answered in parts sends each answer's chunks, numbered from 0, then its
    /// `partial_response`, in order, then its last answer's chunks and the terminal response
    /// (R06.T11.d).
    #[tokio::test]
    async fn answers_in_parts_stream_in_order_before_the_terminal_response() {
        let harness = Harness::start(InParts {
            parts: 2,
            ends: Ending::Answered,
        })
        .await;
        let mut client = harness.connect().await;
        client.hello().await;
        client.request(7, body(7)).await;
        for n in 0..3_u32 {
            let (chunks, message) = answer_in_parts(&mut client).await;
            assert_eq!(chunks, [[7, 0, 2, n], [7, 1, 2, n]], "answer {n}");
            match message {
                ServerMessage::PartialResponse { id, body } if n < 2 => {
                    assert_eq!((id, body), (RequestId(7), small_response()));
                }
                ServerMessage::Response { id, body } if n == 2 => {
                    assert_eq!((id, body), (RequestId(7), small_response()));
                }
                other => panic!("answer {n} ended with {other:?}"),
            }
        }
        // Nothing follows the terminal response.
        assert_eq!(client.ping(1).await, []);
        harness.stop().await;
    }

    /// A request that fails after its answers in parts ends with its error after all of them, its
    /// error waiting behind their chunks (R06.T11.d).
    #[tokio::test]
    async fn a_request_failing_after_its_answers_in_parts_ends_after_them() {
        let harness = Harness::start(InParts {
            parts: 2,
            ends: Ending::Fails,
        })
        .await;
        let mut client = harness.connect().await;
        client.hello().await;
        client.request(4, body(4)).await;
        for n in 0..2_u32 {
            let (chunks, message) = answer_in_parts(&mut client).await;
            assert_eq!(chunks, [[4, 0, 2, n], [4, 1, 2, n]], "answer {n}");
            assert!(
                matches!(
                    message,
                    ServerMessage::PartialResponse {
                        id: RequestId(4),
                        ..
                    }
                ),
                "answer {n} ended with {message:?}"
            );
        }
        match client.next_frame().await {
            Received::Message(ServerMessage::RequestError { id, error }) => {
                assert_eq!((id, error.code), (RequestId(4), ErrorCode::Internal));
            }
            other => panic!("expected the request's error, got {other:?}"),
        }
        assert_eq!(client.ping(1).await, []);
        harness.stop().await;
    }

    /// A request cancelled after an answer in parts sends nothing more for it but `cancelled`.
    #[tokio::test]
    async fn a_request_cancelled_between_its_answers_sends_nothing_more() {
        let harness = Harness::start(InParts {
            parts: 1,
            ends: Ending::Hangs,
        })
        .await;
        let mut client = harness.connect().await;
        client.hello().await;
        client.request(3, body(3)).await;
        let (chunks, message) = answer_in_parts(&mut client).await;
        assert_eq!(chunks.len(), 2);
        assert!(
            matches!(
                message,
                ServerMessage::PartialResponse {
                    id: RequestId(3),
                    ..
                }
            ),
            "{message:?}"
        );
        client.cancel(3).await;
        match client.next_message().await {
            ServerMessage::RequestError { id, error } => {
                assert_eq!((id, error.code), (RequestId(3), ErrorCode::Cancelled));
            }
            other => panic!("expected the cancel's answer, got {other:?}"),
        }
        assert_eq!(client.ping(2).await, []);
        harness.stop().await;
    }

    #[tokio::test]
    async fn shutdown_does_not_wait_for_a_client_that_never_answers_the_close() {
        let (handler, _calls) = Scripted::new();
        let harness = Harness::start(handler).await;
        let mut client = harness.connect().await;
        client.hello().await;
        // The client reads nothing until the server has shut down, which gives up on the close
        // handshake after `CLOSE_TIMEOUT`, well within the harness's own time limit.
        harness.stop().await;
        assert_eq!(client.closed().await, Some(close_code::AWAY));
    }

    /// Duplicates of an ID in flight sent behind the clogging response: twice what the outbound
    /// queue holds. Each is refused with the connection-level `error`, and counted as refused, as
    /// the connection reads it.
    const CLOGGING_DUPLICATES: usize = 2 * OUTBOUND_QUEUE_FRAMES;

    /// The frames a connection whose writer is stuck reads before it stops: as many as its queue
    /// holds, and the one it is waiting to queue.
    fn frames_read_until_full() -> u64 {
        u64::try_from(OUTBOUND_QUEUE_FRAMES + 1).expect("the queue is small")
    }

    /// A harness whose connections never time a write out. A test that sticks a writer is about
    /// the queue behind it, not the write timeout, and under load the steps it takes before the
    /// client reads again can outlast [`WRITE_TIMEOUT`]; the write timeout's own tests are in
    /// [`outbound`](crate::outbound).
    async fn harness_without_write_timeout(handler: Scripted) -> Harness {
        let limits = ConnectionLimits {
            write_timeout: NEVER,
            ..ConnectionLimits::default()
        };
        Harness::start_with_limits(handler, limits).await
    }

    /// Makes `client`'s server-side writer stick on a response the client does not read. Then
    /// sends request 2, which stays in flight, [`CLOGGING_DUPLICATES`] duplicates of it and
    /// request 3, and waits until the connection has stopped reading, part-way through the
    /// duplicates, with its queue full. Returns request 2's call.
    async fn clog(harness: &Harness, calls: &mut Calls, client: &mut Client) -> Call {
        client.hello().await;
        // Every refusal is queued behind the response, where the writer, stuck on it, takes none
        // of them.
        harness.stick_writer(calls, client, 1).await;
        client.request(2, body(2)).await;
        let second = calls.next().await;
        assert_eq!(second.id(), 2);
        for _ in 0..CLOGGING_DUPLICATES {
            client.request(2, body(2)).await;
        }
        client.request(3, body(3)).await;
        harness
            .requests_until(|counters| counters.refused() >= frames_read_until_full())
            .await;
        second
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn a_full_outbound_queue_stops_the_reader_and_loses_nothing() {
        let (handler, mut calls) = Scripted::new();
        let harness = harness_without_write_timeout(handler).await;
        let mut slow = harness.connect_slow_reader().await;
        let second = clog(&harness, &mut calls, &mut slow).await;

        // Everyone else is served meanwhile.
        let mut other = harness.connect().await;
        other.hello().await;
        assert_eq!(other.ping(0).await, []);
        other.close().await;
        // The slow client's connection has read no further: not the rest of the duplicates, and
        // not request 3.
        assert_eq!(
            harness.server().stats().requests().refused(),
            frames_read_until_full(),
            "the connection read on with its queue full"
        );
        assert!(calls.is_empty(), "request 3 was read with the queue full");

        // Reading again, the client gets every frame, in order, and the connection goes on.
        assert_eq!(
            slow.next_message().await,
            ServerMessage::Response {
                id: RequestId(1),
                body: bulky_response(CLOGGING_BYTES),
            }
        );
        for _ in 0..CLOGGING_DUPLICATES {
            assert_eq!(
                slow.next_message().await,
                ServerMessage::Error {
                    message: "request 2 is already in flight; the new request was dropped"
                        .to_owned(),
                }
            );
        }
        let third = calls.next().await;
        assert_eq!(third.id(), 3);
        for (call, id) in [(second, 2), (third, 3)] {
            assert!(call.respond(small_response()));
            assert_eq!(
                slow.next_message().await,
                ServerMessage::Response {
                    id: RequestId(id),
                    body: small_response(),
                }
            );
        }
        slow.close().await;
        harness.stop().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn shutdown_is_not_held_up_by_a_connection_whose_queue_is_full() {
        let (handler, mut calls) = Scripted::new();
        let harness = harness_without_write_timeout(handler).await;
        let mut slow = harness.connect_slow_reader().await;
        let mut second = clog(&harness, &mut calls, &mut slow).await;
        // Neither the queued frames nor the close frame behind them can be sent, and no write
        // times out; the connection gives up after `CLOSE_TIMEOUT`, and `stop` checks that
        // nothing is left.
        let state = Arc::clone(harness.state());
        harness.stop().await;
        second.dropped().await;
        assert!(second.token.is_cancelled());
        assert!(calls.is_empty(), "request 3 was never read");
        assert_eq!(state.request_stats.snapshot().abandoned(), 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn a_client_that_stops_reading_and_then_disconnects_frees_its_connection() {
        let (handler, mut calls) = Scripted::new();
        let harness = harness_without_write_timeout(handler).await;
        let mut slow = harness.connect_slow_reader().await;
        let mut second = clog(&harness, &mut calls, &mut slow).await;
        // Unread data makes the client's end reset the connection, which fails the stuck write.
        drop(slow);
        harness.connections_until(0).await;
        second.dropped().await;
        assert!(second.token.is_cancelled());
        let counters = harness.server().stats().requests();
        assert_eq!(
            (
                counters.accepted(),
                counters.responded(),
                counters.abandoned(),
                counters.in_flight()
            ),
            (2, 1, 1, 0)
        );
        assert!(calls.is_empty(), "request 3 was never read");
        harness.stop().await;
    }

    #[tokio::test]
    async fn a_connection_upgrading_as_shutdown_starts_is_closed_going_away() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let (handler, _calls) = Scripted::new();
        let harness = Harness::start(handler).await;
        let addr = harness.addr();
        let mut tcp =
            tokio::time::timeout(crate::testing::WAIT, tokio::net::TcpStream::connect(addr))
                .await
                .expect("timed out connecting")
                .unwrap();
        let upgrade = format!(
            "GET /ws HTTP/1.1\r\nHost: {addr}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
             Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n"
        );
        tokio::time::timeout(crate::testing::WAIT, tcp.write_all(upgrade.as_bytes()))
            .await
            .expect("timed out sending the upgrade")
            .unwrap();
        // The connection is counted from the moment its upgrade is accepted, before its task
        // runs, and shutdown starts at once.
        harness.connections_until(1).await;
        let stopping = tokio::spawn(harness.stop());
        // The client never answers the close, so the server drops the socket after
        // `CLOSE_TIMEOUT` and the stream ends.
        let mut received = Vec::new();
        tokio::time::timeout(crate::testing::WAIT, tcp.read_to_end(&mut received))
            .await
            .expect("timed out reading")
            .expect("the server ends the stream cleanly");
        tokio::time::timeout(crate::testing::WAIT, stopping)
            .await
            .expect("timed out stopping")
            .expect("stopping does not panic");
        let head_end = received
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("a complete response head")
            + 4;
        assert!(
            received.starts_with(b"HTTP/1.1 101 "),
            "{:?}",
            String::from_utf8_lossy(&received)
        );
        // One unmasked close frame: FIN and opcode 8, the payload's length, then the code.
        let reason = b"the server is shutting down";
        let mut close = vec![0x88, u8::try_from(2 + reason.len()).unwrap()];
        close.extend_from_slice(&close_code::AWAY.to_be_bytes());
        close.extend_from_slice(reason);
        assert_eq!(received[head_end..], close);
    }
}
