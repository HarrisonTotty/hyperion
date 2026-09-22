//! Test helpers for event kinds, compiled for this crate's tests and behind the `testing` feature.
//!
//! The brainstorm's time test: "Both event constructions return the same events in any order of
//! asking" ("Testing"). [`assert_partition_independent`] checks one half of it, that a listing over
//! a window is the union of the listings over the pieces of any partition of it. The other half,
//! the order of asking, is checked by handing the pieces from [`partition`] to
//! `hyperion_testkit::order::assert_order_independent`, which the sim cannot depend on outside its
//! tests.

use std::fmt::Debug;

use super::{EventsPerSecond, RateModel, TimeWindow};
use crate::time::UniverseTime;

/// The pieces of `whole` cut at `cuts`, in order: `[start, c₀), [c₀, c₁), …, [cₙ, end)`.
///
/// # Panics
///
/// Unless the cuts are non-decreasing and lie inside `[start, end]`.
#[must_use]
pub fn partition(whole: TimeWindow, cuts: &[UniverseTime]) -> Vec<TimeWindow> {
    let mut pieces = Vec::with_capacity(cuts.len() + 1);
    let mut start = whole.start();
    for &cut in cuts.iter().chain(std::iter::once(&whole.end())) {
        assert!(
            whole.start() <= cut && cut <= whole.end(),
            "cut {cut} lies outside the window [{}, {})",
            whole.start(),
            whole.end()
        );
        pieces.push(TimeWindow::new(start, cut).expect("cuts are non-decreasing"));
        start = cut;
    }
    pieces
}

/// Asserts that listing `whole` gives the listings of the pieces of `whole` cut at `cuts`, one
/// after another.
///
/// `list` returns the events of one window in time order, as both constructions do, so the pieces'
/// listings in order must equal the whole's exactly: no event lost, doubled or changed at a cut.
///
/// # Panics
///
/// If the two differ, naming the first event where they part, or as [`partition`].
#[track_caller]
pub fn assert_partition_independent<E: PartialEq + Debug>(
    whole: TimeWindow,
    cuts: &[UniverseTime],
    list: impl Fn(TimeWindow) -> Vec<E>,
) {
    let expected = list(whole);
    let pieces: Vec<E> = partition(whole, cuts).into_iter().flat_map(&list).collect();
    if let Some(i) =
        (0..expected.len().max(pieces.len())).find(|&i| expected.get(i) != pieces.get(i))
    {
        panic!(
            "the listing over [{}, {}) cut at {} points differs from the listing over the whole \
             at event {i}: pieces gave {:?}, the whole gave {:?} ({} events against {})",
            whole.start(),
            whole.end(),
            cuts.len(),
            pieces.get(i),
            expected.get(i),
            pieces.len(),
            expected.len(),
        );
    }
}

/// A constant rate, which is also its own bound.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct ConstantRate {
    rate: EventsPerSecond,
}

impl ConstantRate {
    /// A constant `rate`.
    ///
    /// # Panics
    ///
    /// Unless the rate is finite and not negative.
    #[must_use]
    pub fn new(rate: EventsPerSecond) -> Self {
        assert!(
            rate.value().is_finite() && rate.value() >= 0.0,
            "a rate must be finite and not negative, got {rate:?}"
        );
        Self { rate }
    }

    /// The rate.
    #[must_use]
    pub const fn get(self) -> EventsPerSecond {
        self.rate
    }
}

impl RateModel for ConstantRate {
    fn bound(&self, _bin: TimeWindow) -> EventsPerSecond {
        self.rate
    }

    fn rate(&self, _t: UniverseTime) -> EventsPerSecond {
        self.rate
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;

    fn at(seconds: i64) -> UniverseTime {
        UniverseTime::new(seconds, 0).unwrap()
    }

    fn window(start: i64, end: i64) -> TimeWindow {
        TimeWindow::new(at(start), at(end)).unwrap()
    }

    /// A toy listing: one "event" per whole second divisible by 3.
    fn thirds(w: TimeWindow) -> Vec<i64> {
        (w.start().seconds()..w.end().seconds())
            .filter(|s| s.rem_euclid(3) == 0)
            .collect()
    }

    #[test]
    fn partition_cuts_in_order_and_keeps_empty_pieces() {
        let pieces = partition(window(0, 10), &[at(2), at(2), at(7)]);
        assert_eq!(
            pieces,
            vec![window(0, 2), window(2, 2), window(2, 7), window(7, 10)]
        );
        assert_eq!(partition(window(0, 10), &[]), vec![window(0, 10)]);
    }

    #[test]
    fn a_consistent_listing_passes() {
        assert_partition_independent(window(-20, 20), &[at(-3), at(0), at(0), at(17)], thirds);
    }

    #[test]
    #[should_panic(expected = "differs from the listing over the whole at event")]
    fn a_listing_that_doubles_at_a_cut_fails() {
        assert_partition_independent(window(0, 10), &[at(3)], |w| {
            let mut events = thirds(w);
            if w.start() == at(3) {
                events.insert(0, 3);
            }
            events
        });
    }

    #[test]
    #[should_panic(expected = "lies outside the window")]
    fn a_cut_outside_the_window_is_rejected() {
        let _ = partition(window(0, 10), &[at(11)]);
    }

    #[test]
    fn a_constant_rate_is_its_own_bound() {
        let rate = ConstantRate::new(EventsPerSecond::new(0.25));
        assert_same_bits(rate.get().value(), 0.25);
        assert_same_bits(rate.bound(window(0, 1)).value(), rate.rate(at(5)).value());
    }
}
