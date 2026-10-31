//! Fixtures shared by the Knowledge store's tests.

use hyperion_sim::alerts::{AlertBand, Detection, DetectionPhase, TransientKind};
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::id::event_tags;
use hyperion_sim::id::{EventBin, EventId, EventSubject, EventWord, SystemId};
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::WattsPerSquareMetre;

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

/// A flare's onset of [`host`] seen in `band`, as the sightings of the tests record it: their
/// kind, phase and band.
pub(crate) fn flare_in(band: AlertBand) -> Detection {
    let at = |ly| GalacticPosition::from_light_years(ly).expect("in the cube");
    hyperion_sim::alerts::testing::detection(
        event_of(host()),
        host(),
        None,
        TransientKind::Flare,
        DetectionPhase::Onset,
        band,
        WattsPerSquareMetre::new(1e-12),
        false,
        at([-3_000.0, 21_000.0, 40.0]),
        UniverseTime::EPOCH,
        &at([0.0, 25_000.0, 0.0]),
    )
}
