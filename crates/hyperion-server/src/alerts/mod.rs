//! The alert service: what a subscribed sensor is told of transients as their light reaches it
//! (plan 12, P12.T8; Design notes 6–9 and 11).
//!
//! The sim answers "what went off?" over a window ([`hyperion_sim::alerts`]); this module turns
//! its detections into pushes over time. A [`Subscription`] holds its observer, its sensor and an
//! [`AlertSchedule`]: the detections whose light arrives over [t, t + [`ALERT_LOOKAHEAD`]) at the
//! observer's place, each [`Tracked`] with the times its light crosses the sensor's limits.
//! [`AlertSchedule::advance`] is the pure core: from one observer to the next it returns the
//! pushes that fell due and, when the schedule needs one, the scan to run ([`Rescan`]).
//!
//! The scans run on the CPU pool through the server's `AlertScans` (P12.T8.b), which shares
//! each walk among the subscriptions that ask for it and keeps them in a byte budget. The wire's
//! `alerts` topic opens a subscription, runs its scans, makes the pushes that fall due into
//! sightings in the universe's Knowledge and pushes the contacts they change with the census, and
//! answers `alerts_observer` and `alerts_acknowledge` (P12.T9.a). Each universe's contacts are
//! opened once and shared (`UniverseContacts`). A clear, and telling the universe's other
//! subscriptions of a change, are P12.T8.c's.

mod contacts;
mod scan;
mod schedule;
mod service;
#[cfg(test)]
mod testing;
mod topic;

pub(crate) use contacts::UniverseContacts;
pub(crate) use service::AlertScans;
pub(crate) use topic::open;

pub use schedule::{
    ALERT_LOOKAHEAD, Advance, AlertSchedule, CROSSING_RESOLUTION, Crossing, GLITCH_CLEAR_AFTER,
    MergeScanError, Push, Rescan, ScanRequest, Subscription, Tracked,
};
