//! The server's record of its open WebSocket connections, which is how shutdown finds and waits
//! for them.
//!
//! axum runs each upgraded socket on a task of its own that graceful shutdown neither signals nor
//! joins. Each connection therefore holds a [`ConnectionGuard`] from before its upgrade until its
//! task has finished, and watches for the server closing. [`Connections::close_all`] refuses new
//! connections, tells the open ones to close and waits for every guard to be dropped. Each
//! connection closes within [`CLOSE_TIMEOUT`](crate::limits::CLOSE_TIMEOUT) of being told, so the
//! wait is bounded.

use tokio::sync::watch;

/// The connections open now, and whether the server is closing them.
#[derive(Debug)]
pub(crate) struct Connections {
    state: watch::Sender<Tracked>,
    /// Raised once by [`Connections::close_all`]. A channel of its own, so that connections
    /// opening and closing do not wake every connection waiting on it.
    signal: watch::Sender<bool>,
}

#[derive(Debug, Clone, Copy, Default)]
struct Tracked {
    /// Guards alive now.
    open: usize,
    /// The ID the next connection gets.
    next_id: u64,
    /// Set once by [`Connections::close_all`]; no connection opens after it.
    closing: bool,
}

impl Connections {
    pub(crate) fn new() -> Self {
        Self {
            state: watch::Sender::new(Tracked::default()),
            signal: watch::Sender::new(false),
        }
    }

    /// Records a new connection, or `None` if the server is closing.
    pub(crate) fn open(&self) -> Option<ConnectionGuard> {
        let mut id = None;
        // One critical section checks the flag and counts the connection, so that `close_all`
        // either sees the connection or the connection sees the flag.
        self.state.send_if_modified(|tracked| {
            if tracked.closing {
                return false;
            }
            tracked.open += 1;
            id = Some(tracked.next_id);
            tracked.next_id = tracked.next_id.wrapping_add(1);
            true
        });
        id.map(|id| ConnectionGuard {
            id,
            state: self.state.clone(),
            signal: self.signal.subscribe(),
        })
    }

    /// Connections open now.
    pub(crate) fn open_count(&self) -> usize {
        self.state.borrow().open
    }

    /// Waits until exactly `count` connections are open.
    #[cfg(test)]
    pub(crate) async fn wait_until_open(&self, count: usize) {
        // The sender lives in `self`, so the channel cannot close while this waits.
        let _ = self
            .state
            .subscribe()
            .wait_for(|tracked| tracked.open == count)
            .await;
    }

    /// Refuses new connections, tells every open one to close, and waits until all have.
    ///
    /// Cancellation-safe, and calling it again waits again.
    pub(crate) async fn close_all(&self) {
        // The flag first: a connection that opens after it sees it, and one that opened before
        // it is signalled below, since it subscribed to the signal when it opened.
        self.state.send_modify(|tracked| tracked.closing = true);
        self.signal.send_replace(true);
        // The sender lives in `self`, so the channel cannot close while this waits.
        let _ = self
            .state
            .subscribe()
            .wait_for(|tracked| tracked.open == 0)
            .await;
    }
}

/// One open connection's place in [`Connections`]. Dropping it closes the place, so shutdown
/// stops waiting for the connection.
#[derive(Debug)]
#[must_use = "a connection is tracked only while its guard lives"]
pub(crate) struct ConnectionGuard {
    id: u64,
    state: watch::Sender<Tracked>,
    signal: watch::Receiver<bool>,
}

impl ConnectionGuard {
    /// The connection's number, counted from 0 in the order connections opened, for the logs.
    pub(crate) fn id(&self) -> u64 {
        self.id
    }

    /// A signal that completes once the server starts closing its connections.
    pub(crate) fn closing(&self) -> Closing {
        Closing(self.signal.clone())
    }
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.state
            .send_modify(|tracked| tracked.open = tracked.open.saturating_sub(1));
    }
}

/// Completes once the server starts closing its connections. Clones watch the same signal.
#[derive(Debug, Clone)]
pub(crate) struct Closing(watch::Receiver<bool>);

impl Closing {
    /// Waits for the signal; returns at once if it has been given. Cancellation-safe.
    pub(crate) async fn wait(&mut self) {
        // The sender lives in the server's state, which every connection holds, so the channel
        // cannot close while a connection waits; were it to close, the server would be gone and
        // the connection should close too.
        let _ = self.0.wait_for(|closing| *closing).await;
    }
}

#[cfg(test)]
mod tests {
    use std::pin::pin;
    use std::time::Duration;

    use futures_util::poll;
    use tokio::time::timeout;

    use super::*;

    const WAIT: Duration = Duration::from_secs(5);

    #[tokio::test]
    async fn close_all_signals_open_connections_and_waits_for_them() {
        let connections = Connections::new();
        let first = connections.open().unwrap();
        let second = connections.open().unwrap();
        assert_eq!((first.id(), second.id()), (0, 1));
        assert_eq!(connections.open_count(), 2);
        let mut signal = first.closing();
        assert!(poll!(pin!(signal.wait())).is_pending(), "not closing yet");

        let mut closing = pin!(connections.close_all());
        assert!(poll!(&mut closing).is_pending(), "two connections are open");
        timeout(WAIT, signal.wait())
            .await
            .expect("the signal fired");
        assert!(connections.open().is_none(), "no connection opens now");
        drop(first);
        assert!(poll!(&mut closing).is_pending(), "one connection is open");
        drop(second);
        timeout(WAIT, closing)
            .await
            .expect("every connection closed");
        assert_eq!(connections.open_count(), 0);
    }

    #[tokio::test]
    async fn close_all_with_nothing_open_returns_at_once() {
        let connections = Connections::new();
        drop(connections.open().unwrap());
        timeout(WAIT, connections.close_all()).await.unwrap();
    }
}
