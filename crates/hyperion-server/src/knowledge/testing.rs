//! Fixtures shared by the Knowledge store's tests.

use hyperion_sim::id::event_tags;
use hyperion_sim::id::{EventBin, EventId, EventSubject, EventWord, SystemId};

/// A grid system, as the host of the tests' events.
pub(crate) fn host() -> SystemId {
    "0200080020000000".parse().expect("a grid system's ID")
}

/// Another grid system.
pub(crate) fn other_host() -> SystemId {
    "0200080020000001".parse().expect("a grid system's ID")
}

/// A third grid system.
pub(crate) fn third_host() -> SystemId {
    "0200080020000002".parse().expect("a grid system's ID")
}

/// An event of `system`: the first registered tag, bin 0, number 0.
pub(crate) fn event_of(system: SystemId) -> EventId {
    let bin = EventBin::new(0).expect("0 is in range");
    EventId::new(
        EventSubject::System(system),
        EventWord::new(event_tags::ALL[0], bin, 0),
    )
}
