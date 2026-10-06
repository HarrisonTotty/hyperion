//! The requests that stop the server, on every platform, and the rule that a second one forces the
//! exit (galaxy plan 04, P04.T17.a).
//!
//! [`StopRequests::listen`] listens, through tokio's signal module, for what each platform sends a
//! process it wants stopped:
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
//!
//! The first request starts a graceful shutdown. A listener replaces the platform's default, on
//! Unix for the life of the process and on Windows while [`StopRequests`] is held, which `main`
//! does to the end. Without more, a second request would then do nothing while a slow shutdown
//! finishes the jobs in hand. [`unless_stopped_again`] races the shutdown against the next request,
//! and `main` exits at once, with status 1, if the request comes first.
//!
//! Only the Unix listeners are tested, by `tests/stop.rs`, which sends the binary real signals.
//! The Windows ones are compiled by the Windows Clippy and are untested: no Windows runner exists,
//! and raising a console event takes a Win32 call that needs `unsafe`. Unix and Windows are the
//! only platforms tokio's signal module covers, so the server builds for no other.

#[cfg(not(any(unix, windows)))]
compile_error!(
    "hyperion-server stops on Unix's signals or Windows' console events, and tokio has neither here"
);

use std::fmt;
use std::future::{Future, poll_fn};
use std::io;
use std::task::{Context, Poll};

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
    /// Standard input reached its end, for a server run as a supervised child. Nothing requests this
    /// yet: the opt-in watch on standard input is galaxy plan 04's P04.T17.c.
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

/// Polls one listener: ready with `Some` for a request, and with `None` once it can give no more.
type PollRequest = Box<dyn FnMut(&mut Context<'_>) -> Poll<Option<()>> + Send>;

/// One kind of stop request, listened for.
struct Listener {
    reason: StopReason,
    poll: PollRequest,
}

/// The stop requests this process listens for, received one at a time by [`StopRequests::next`].
///
/// Listening spawns no task: the requests arrive only while `next` is awaited, and one that
/// arrives in between waits for the next call.
pub struct StopRequests {
    listeners: Vec<Listener>,
}

impl StopRequests {
    /// Listens for the platform's stop requests, as the [module documentation](self) lists them.
    ///
    /// A listener that fails to register is logged at `error` and left out, so the server still
    /// runs; with none, [`StopRequests::next`] never returns, and the process ends by the
    /// platform's default for each request.
    ///
    /// # Panics
    ///
    /// On Unix, panics if called outside a tokio runtime, or in one built without its I/O driver
    /// (which drives signals), as tokio's `signal` does.
    #[must_use]
    pub fn listen() -> Self {
        let mut requests = Self {
            listeners: Vec::new(),
        };
        requests.listen_on_platform();
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

    use futures_util::FutureExt;
    use tokio::sync::{mpsc, oneshot};

    use super::*;

    /// A listener the test fires with `try_send`, holding up to two requests.
    fn fake(requests: &mut StopRequests, reason: StopReason) -> mpsc::Sender<()> {
        let (fire, fired) = mpsc::channel(2);
        requests.add(reason, Ok(fired), mpsc::Receiver::poll_recv);
        fire
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
