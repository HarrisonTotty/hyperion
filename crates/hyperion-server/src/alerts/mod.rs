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
//! The wire's `alerts` topic opens a subscription and answers `alerts_observer` and
//! `alerts_acknowledge` (P12.T9.a), and each universe's contacts are opened once and shared
//! ([`UniverseContacts`]). The scans on the CPU pool and their cache are P12.T8.b's, and the pushes
//! into Knowledge and their notifications P12.T8.c's: until then a subscription's census is empty
//! and an advance pushes nothing.

mod contacts;
mod schedule;
mod topic;

pub(crate) use contacts::UniverseContacts;
pub(crate) use topic::open;

pub use schedule::{
    ALERT_LOOKAHEAD, Advance, AlertSchedule, CROSSING_RESOLUTION, Crossing, GLITCH_CLEAR_AFTER,
    MergeScanError, Push, Rescan, ScanRequest, Subscription, Tracked,
};
