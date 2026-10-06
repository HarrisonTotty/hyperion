//! The requests that stop the server, on every platform, and the rule that a second one forces the
//! exit (galaxy plan 04, P04.T17.a and P04.T17.c).
//!
//! [`StopRequests::listen`] listens, through tokio's signal module, for what each platform sends a
//! process it wants stopped, and, when asked, for the end of standard input:
//!
//! - **Unix:** `SIGINT` and `SIGTERM`. `SIGHUP` keeps its default, which ends the process: there are
//!   no reload semantics to claim it for, nothing is lost (both stores are write-through), and a
//!   handler once installed can never give the default back.
//! - **Windows:** Ctrl-C, Ctrl-Break, the console closing and the system shutting down. Logoff is
//!   not listened for: it "does not indicate which user is logging off", so a service must not stop
//!   on it (Microsoft Learn, *`HandlerRoutine` callback function*). For a closed console and a
//!   shutdown, tokio parks the thread Windows calls its handler on, so the process runs its
//!   shutdown until `main` returns or until Windows' timeout ends it, 5 s for a closed console and
//!   20 s for a service at shutdown.
//! - **Every platform, under [`StdinStop::Watch`]:** the end of standard input, for a server run as
//!   a supervised child. Its parent stops it by closing the child's stdin. On Windows that is the
//!   only graceful stop a parent has: Node's `subprocess.kill()` there is `TerminateProcess`, and a
//!   GUI parent cannot send its child console events. If the parent dies, the pipe closes on every
//!   OS, so the server does not outlive it. It is opt-in, since an interactive server's stdin is
//!   the terminal, and a server run with stdin closed (systemd's default, or `< /dev/null`) would
//!   stop at once.
//!
//! The first request starts a graceful shutdown. A listener replaces the platform's default, on
//! Unix for the life of the process and on Windows while [`StopRequests`] is held, which `main`
//! does to the end. Without more, a second request would then do nothing while a slow shutdown
//! finishes the jobs in hand. [`unless_stopped_again`] races the shutdown against the next request,
//! and `main` exits at once, with status 1, if the request comes first.
//!
//! # The thread that watches standard input
//!
//! Standard input is read and discarded to its end on a thread of its own, named `stdin-watch`.
//! The thread is detached, an exception by design to the rule that everything spawned has an owner
//! and a shutdown path. A blocking read cannot be cancelled, and the read is not left to tokio,
//! since a read on its blocking pool would hold up the runtime's drop at exit until the read ended
//! (tokio's `Stdin` documentation). The process's exit ends the thread.
//!
//! The Unix signals and the watch on standard input are tested by `tests/stop.rs`, which runs the
//! binary. The Windows events are compiled by the Windows Clippy and are untested: no Windows
//! runner exists, and raising a console event takes a Win32 call that needs `unsafe`. Unix and
//! Windows are the only platforms tokio's signal module covers, so the server builds for no other.

#[cfg(not(any(unix, windows)))]
compile_error!(
    "hyperion-server stops on Unix's signals or Windows' console events, and tokio has neither here"
);

use std::fmt;
use std::future::{Future, poll_fn};
use std::io::{self, Read};
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use tokio::sync::oneshot;

/// The name of the thread that watches standard input under [`StdinStop::Watch`].
const STDIN_WATCH_THREAD: &str = "stdin-watch";

/// Why the server was asked to stop.
///
/// Every variant exists on every platform, so that a match on a reason is exhaustive everywhere.
/// The platforms that never send one are named in its documentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StopReason {
    /// `SIGINT` on Unix, from Ctrl-C at the terminal or `kill -INT`; `CTRL_C_EVENT` on Windows, from
    /// Ctrl-C at the console or `GenerateConsoleCtrlEvent`.
    Interrupt,
    /// `SIGTERM`, from systemd, `docker stop`, `kill` or Node's `subprocess.kill()`. Unix only:
    /// Windows has no such signal, and its `TerminateProcess` cannot be caught.
    Terminate,
    /// `CTRL_BREAK_EVENT`, from Ctrl-Break at the console or `GenerateConsoleCtrlEvent`. Windows
    /// only.
    Break,
    /// `CTRL_CLOSE_EVENT`: the console window was closed, or End Task was chosen in Task Manager.
    /// Windows only, and Windows ends the process 5 s later.
    ConsoleClosed,
    /// `CTRL_SHUTDOWN_EVENT`: the system is shutting down. Windows only, and only a service receives
    /// it; Windows ends a service 20 s later.
    SystemShutdown,
    /// Standard input reached its end, or could not be read, under [`StdinStop::Watch`]: on every
    /// platform, from a parent that closed its child's stdin or that died.
    StdinClosed,
}

impl fmt::Display for StopReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Interrupt => "interrupt signal",
            Self::Terminate => "terminate signal",
            Self::Break => "break signal",
            Self::ConsoleClosed => "console window closed",
            Self::SystemShutdown => "system shutting down",
            Self::StdinClosed => "standard input closed",
        })
    }
}

/// Whether the end of standard input asks the server to stop, for a server run as a supervised
/// child (P04.T17.c).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum StdinStop {
    /// Standard input is read and discarded, and its end, or an error reading it, is a stop
    /// request, [`StopReason::StdinClosed`]. A missing standard input reads as an empty one, so it
    /// stops the server at once.
    Watch,
    /// Standard input is left alone. The default, since an interactive server's stdin is the
    /// terminal, and one run under systemd, or with `< /dev/null`, has none.
    #[default]
    Ignore,
}

/// Polls one listener: ready with `Some` for a request, and with `None` once it can give no more.
type PollRequest = Box<dyn FnMut(&mut Context<'_>) -> Poll<Option<()>> + Send>;

/// One kind of stop request, listened for.
struct Listener {
    reason: StopReason,
    poll: PollRequest,
}

/// The stop requests this process listens for, received one at a time by [`StopRequests::next`].
///
/// Listening spawns no tokio task: the requests arrive only while `next` is awaited, and one that
/// arrives in between waits for the next call. Under [`StdinStop::Watch`] it starts one detached
/// thread, as the [module documentation](self#the-thread-that-watches-standard-input) explains.
pub struct StopRequests {
    listeners: Vec<Listener>,
}

impl StopRequests {
    /// Listens for the platform's stop requests, and for the end of standard input under
    /// [`StdinStop::Watch`], as the [module documentation](self) lists them.
    ///
    /// A listener that fails to register, or a watch on standard input whose thread cannot start,
    /// is logged at `error` and left out, so the server still runs; with none,
    /// [`StopRequests::next`] never returns, and the process ends by the platform's default for
    /// each request.
    ///
    /// # Panics
    ///
    /// On Unix, panics if called outside a tokio runtime, or in one built without its I/O driver
    /// (which drives signals), as tokio's `signal` does.
    #[must_use]
    pub fn listen(stdin: StdinStop) -> Self {
        let mut requests = Self {
            listeners: Vec::new(),
        };
        requests.listen_on_platform();
        match stdin {
            StdinStop::Watch => requests.watch_to_the_end(io::stdin()),
            StdinStop::Ignore => {}
        }
        requests
    }

    /// Unix's requests: `SIGINT` and `SIGTERM`, and not `SIGHUP`.
    #[cfg(unix)]
    fn listen_on_platform(&mut self) {
        use tokio::signal::unix::{Signal, SignalKind, signal};

        self.add(
            StopReason::Interrupt,
            signal(SignalKind::interrupt()),
            Signal::poll_recv,
        );
        self.add(
            StopReason::Terminate,
            signal(SignalKind::terminate()),
            Signal::poll_recv,
        );
    }

    /// Windows' requests: Ctrl-C, Ctrl-Break, Close and Shutdown, and not Logoff.
    #[cfg(windows)]
    fn listen_on_platform(&mut self) {
        use tokio::signal::windows::{
            CtrlBreak, CtrlC, CtrlClose, CtrlShutdown, ctrl_break, ctrl_c, ctrl_close,
            ctrl_shutdown,
        };

        self.add(StopReason::Interrupt, ctrl_c(), CtrlC::poll_recv);
        self.add(StopReason::Break, ctrl_break(), CtrlBreak::poll_recv);
        self.add(
            StopReason::ConsoleClosed,
            ctrl_close(),
            CtrlClose::poll_recv,
        );
        self.add(
            StopReason::SystemShutdown,
            ctrl_shutdown(),
            CtrlShutdown::poll_recv,
        );
    }

    /// Adds a listener that requests a stop, as [`StopReason::StdinClosed`], once `reader` has been
    /// read to its end or has failed. `reader` is read and discarded on a detached thread, as the
    /// [module documentation](self#the-thread-that-watches-standard-input) explains.
    fn watch_to_the_end(&mut self, mut reader: impl Read + Send + 'static) {
        let (end, ended) = oneshot::channel();
        let watching = std::thread::Builder::new()
            .name(STDIN_WATCH_THREAD.to_owned())
            .spawn(move || {
                if let Err(error) = io::copy(&mut reader, &mut io::sink()) {
                    tracing::warn!(%error, "cannot read standard input, so it counts as closed");
                }
                // An error means the requests were dropped, so nothing is left to stop.
                end.send(()).ok();
            });
        // Dropping the handle detaches the thread.
        let ended = watching.map(|_detached| Some(ended));
        self.add(StopReason::StdinClosed, ended, poll_ended);
    }

    /// Adds a listener for `reason` once it has registered, or logs why it could not.
    fn add<L>(
        &mut self,
        reason: StopReason,
        registered: io::Result<L>,
        poll: fn(&mut L, &mut Context<'_>) -> Poll<Option<()>>,
    ) where
        L: Send + 'static,
    {
        match registered {
            Ok(mut listener) => self.listeners.push(Listener {
                reason,
                poll: Box::new(move |cx| poll(&mut listener, cx)),
            }),
            Err(error) => {
                tracing::error!(%reason, %error, "cannot listen for this stop request");
            }
        }
    }

    /// Waits for the next stop request, and says what it was.
    ///
    /// When several are waiting, the first in the order the [module documentation](self) lists
    /// them is taken, and the rest wait for later calls.
    ///
    /// # Cancel safety
    ///
    /// Cancel safe: a request that arrives while the future is being dropped is kept for the next
    /// call.
    pub async fn next(&mut self) -> StopReason {
        poll_fn(|cx| self.poll_next(cx)).await
    }

    fn poll_next(&mut self, cx: &mut Context<'_>) -> Poll<StopReason> {
        let mut index = 0;
        while let Some(listener) = self.listeners.get_mut(index) {
            match (listener.poll)(cx) {
                Poll::Ready(Some(())) => return Poll::Ready(listener.reason),
                // tokio documents that its listeners never end, but one that did would be ready
                // for ever, and every request after the first would look like a second one.
                Poll::Ready(None) => {
                    self.listeners.remove(index);
                }
                Poll::Pending => index += 1,
            }
        }
        Poll::Pending
    }
}

impl fmt::Debug for StopRequests {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StopRequests")
            .field(
                "listening_for",
                &self
                    .listeners
                    .iter()
                    .map(|listener| listener.reason)
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

/// Polls the watch on a reader: one request when the reader ends, and none after it.
fn poll_ended(ended: &mut Option<oneshot::Receiver<()>>, cx: &mut Context<'_>) -> Poll<Option<()>> {
    // A oneshot that has given its value panics if it is polled again, so it is let go.
    let Some(receiver) = ended else {
        return Poll::Ready(None);
    };
    let sent = ready!(Pin::new(receiver).poll(cx));
    *ended = None;
    if sent.is_err() {
        // The thread ended without sending, which only a panic in it does. The watch is over all
        // the same, and a server that can no longer see its parent go stops rather than outlive it.
        tracing::error!("the watch on standard input ended unexpectedly, so it counts as closed");
    }
    Poll::Ready(Some(()))
}

/// How a shutdown raced against another stop request ended, from [`unless_stopped_again`].
#[must_use = "a forced shutdown must end the process"]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Shutdown<T> {
    /// The shutdown finished first, with its output.
    Finished(T),
    /// Another stop request came first, for this reason, and the shutdown was dropped unfinished.
    Forced(StopReason),
}

/// Runs `shutdown` unless `again`, the next stop request, comes first.
///
/// A shutdown that is ready when a request is gives [`Shutdown::Finished`]: nothing is gained by
/// forcing an exit that is already clean.
pub async fn unless_stopped_again<T>(
    shutdown: impl Future<Output = T>,
    again: impl Future<Output = StopReason>,
) -> Shutdown<T> {
    tokio::select! {
        biased;
        output = shutdown => Shutdown::Finished(output),
        reason = again => Shutdown::Forced(reason),
    }
}

/// For the tests, which replace the platform's listeners with their own.
#[cfg(test)]
impl StopRequests {
    fn none() -> Self {
        Self {
            listeners: Vec::new(),
        }
    }

    fn reasons(&self) -> Vec<StopReason> {
        self.listeners
            .iter()
            .map(|listener| listener.reason)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::pin::pin;
    use std::sync::mpsc::{Receiver, sync_channel};
    use std::time::Duration;

    use futures_util::FutureExt;
    use tokio::sync::mpsc;

    use super::*;

    /// Upper bound on a watch's noticing that its reader has ended.
    const WATCH_TIMEOUT: Duration = Duration::from_secs(10);

    /// A listener the test fires with `try_send`, holding up to two requests.
    fn fake(requests: &mut StopRequests, reason: StopReason) -> mpsc::Sender<()> {
        let (fire, fired) = mpsc::channel(2);
        requests.add(reason, Ok(fired), mpsc::Receiver::poll_recv);
        fire
    }

    /// Waits up to [`WATCH_TIMEOUT`] for the next request, or `None`.
    fn wait_for(requests: &mut StopRequests) -> Option<StopReason> {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(async { tokio::time::timeout(WATCH_TIMEOUT, requests.next()).await })
            .ok()
    }

    /// A reader that fails every read.
    struct Failing;

    impl Read for Failing {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("refused"))
        }
    }

    /// A reader that panics on its first read.
    struct Panicking;

    impl Read for Panicking {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            panic!("the test's reader panics")
        }
    }

    /// A reader that gives one chunk the test sends per read, and ends once the test drops its
    /// sender.
    struct Fed(Receiver<Vec<u8>>);

    impl Read for Fed {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            match self.0.recv() {
                Ok(chunk) => chunk.as_slice().read(buf),
                Err(_) => Ok(0),
            }
        }
    }

    #[test]
    fn a_closed_reader_requests_a_stop() {
        let mut requests = StopRequests::none();
        requests.watch_to_the_end(io::empty());
        assert_eq!(requests.reasons(), [StopReason::StdinClosed]);

        assert_eq!(wait_for(&mut requests), Some(StopReason::StdinClosed));
        assert_eq!(requests.next().now_or_never(), None, "a reader ends once");
        assert_eq!(requests.reasons(), [], "and its watch is let go");
    }

    #[test]
    fn a_reader_that_fails_requests_a_stop() {
        let mut requests = StopRequests::none();
        requests.watch_to_the_end(Failing);
        assert_eq!(wait_for(&mut requests), Some(StopReason::StdinClosed));
    }

    #[test]
    fn a_watch_that_panics_requests_a_stop() {
        let mut requests = StopRequests::none();
        requests.watch_to_the_end(Panicking);
        assert_eq!(wait_for(&mut requests), Some(StopReason::StdinClosed));
    }

    #[test]
    fn a_reader_requests_a_stop_only_at_its_end() {
        // A rendezvous: each send waits for the reader to take the chunk.
        let (input, chunks) = sync_channel(0);
        let mut requests = StopRequests::none();
        requests.watch_to_the_end(Fed(chunks));

        input.send(b"input".to_vec()).unwrap();
        // Taken only once the reader has come back for more, after discarding the first.
        input.send(b"more input".to_vec()).unwrap();
        assert_eq!(
            requests.next().now_or_never(),
            None,
            "input read and discarded is not a request"
        );

        drop(input);
        assert_eq!(wait_for(&mut requests), Some(StopReason::StdinClosed));
    }

    #[test]
    fn a_second_request_during_the_shutdown_forces_the_exit() {
        let (_finish, finished) = oneshot::channel::<()>();
        let (request, requested) = oneshot::channel();
        let mut race = pin!(unless_stopped_again(finished, async {
            requested.await.expect("the test sends a request")
        }));
        assert_eq!(
            race.as_mut().now_or_never(),
            None,
            "nothing has happened yet"
        );

        request.send(StopReason::Terminate).unwrap();
        assert_eq!(
            race.now_or_never(),
            Some(Shutdown::Forced(StopReason::Terminate))
        );
    }

    #[test]
    fn a_shutdown_that_finishes_first_is_not_forced() {
        let (finish, finished) = oneshot::channel();
        let (_request, requested) = oneshot::channel::<StopReason>();
        let mut race = pin!(unless_stopped_again(finished, async {
            requested.await.expect("the test keeps the sender")
        }));
        assert_eq!(
            race.as_mut().now_or_never(),
            None,
            "nothing has happened yet"
        );

        finish.send(7).unwrap();
        assert_eq!(race.now_or_never(), Some(Shutdown::Finished(Ok(7))));

        // Ready together, the shutdown wins.
        let both = unless_stopped_again(async { 7 }, async { StopReason::Interrupt });
        assert_eq!(both.now_or_never(), Some(Shutdown::Finished(7)));
    }

    #[test]
    fn a_request_names_the_listener_it_came_from() {
        let mut requests = StopRequests::none();
        let interrupt = fake(&mut requests, StopReason::Interrupt);
        let terminate = fake(&mut requests, StopReason::Terminate);
        assert_eq!(
            requests.next().now_or_never(),
            None,
            "nothing has been sent"
        );

        terminate.try_send(()).unwrap();
        assert_eq!(requests.next().now_or_never(), Some(StopReason::Terminate));
        assert_eq!(requests.next().now_or_never(), None, "one request was sent");

        interrupt.try_send(()).unwrap();
        terminate.try_send(()).unwrap();
        assert_eq!(
            requests.next().now_or_never(),
            Some(StopReason::Interrupt),
            "the first listener is taken first"
        );
        assert_eq!(
            requests.next().now_or_never(),
            Some(StopReason::Terminate),
            "the other request waited"
        );
    }

    #[test]
    fn a_listener_that_fails_to_register_is_left_out() {
        let mut requests = StopRequests::none();
        requests.add(
            StopReason::Terminate,
            Err::<mpsc::Receiver<()>, _>(io::Error::other("refused")),
            mpsc::Receiver::poll_recv,
        );
        let interrupt = fake(&mut requests, StopReason::Interrupt);
        assert_eq!(requests.reasons(), [StopReason::Interrupt]);
        assert_eq!(
            format!("{requests:?}"),
            "StopRequests { listening_for: [Interrupt] }"
        );

        interrupt.try_send(()).unwrap();
        assert_eq!(requests.next().now_or_never(), Some(StopReason::Interrupt));
    }

    #[test]
    fn a_listener_that_ends_is_left_out() {
        let mut requests = StopRequests::none();
        let interrupt = fake(&mut requests, StopReason::Interrupt);
        let terminate = fake(&mut requests, StopReason::Terminate);

        drop(interrupt);
        assert_eq!(
            requests.next().now_or_never(),
            None,
            "an ended listener is not a request"
        );
        assert_eq!(requests.reasons(), [StopReason::Terminate]);

        terminate.try_send(()).unwrap();
        assert_eq!(requests.next().now_or_never(), Some(StopReason::Terminate));

        drop(terminate);
        assert_eq!(
            requests.next().now_or_never(),
            None,
            "with no listener left, no request ever comes"
        );
        assert_eq!(requests.reasons(), []);
    }

    #[test]
    fn every_reason_reads_in_lower_case() {
        let reasons = [
            StopReason::Interrupt,
            StopReason::Terminate,
            StopReason::Break,
            StopReason::ConsoleClosed,
            StopReason::SystemShutdown,
            StopReason::StdinClosed,
        ];
        for reason in reasons {
            let text = reason.to_string();
            assert_eq!(text, text.to_lowercase(), "{reason:?}");
        }
        assert_eq!(StopReason::Terminate.to_string(), "terminate signal");
        assert_eq!(
            StopReason::ConsoleClosed.to_string(),
            "console window closed"
        );
    }
}
