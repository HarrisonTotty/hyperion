//! What a scan of one class cell needs to build (plan 12, P12.T8.b.2): the candidates whose
//! event's time T, read first ([`ClassProcess::event_time`], the contract's rule 10), can light a
//! query's window from somewhere in the cell.
//!
//! An event at T in a cell is seen from the observer by light that left the cell's box at T:
//! light that takes at least the light time from the box's nearest point, `near`, and at most the
//! light time from its farthest, `far`. So it can light a window [t₁, t₂) only if T lies in
//! [t₁ − L − far, t₂ − near), L being how long after T the event still gives light. Each bound is
//! widened by a day, for the day by which a time read first may differ from its entry's (rule 10)
//! and for the light times' rounding. A cut builds no candidate outside it, and every candidate in
//! it as the whole cell does, so a walk of the cut cell finds what a walk of the whole cell finds.
//!
//! - One-shot: L is [`SUPERNOVA_LIGHT_YEARS`], 228 years, after which no supernova template gives
//!   light. The mergers' candidates read no time first, so their walks read every candidate
//!   (`OneShotClass::cell_events_when`) and are not cut.
//! - Magnetars: L is [`MAGNETAR_ACTIVE_SPAN`] and twice the longest burst, the emission interval's
//!   reach before its root; the remnant stays within [`LIFETIME_REACH`] of its cell, so its light
//!   times are `near` and `far` widened by that, and the bounds by a day more, as the emission
//!   interval's own ([`magnetar_scan_cell`](super::magnetar_scan_cell)).
//!
//! A cut cell is not the whole cell: no walk lends it to the context's
//! [`OneShotCells`](super::OneShotCells) or [`MagnetarCells`](super::MagnetarCells).

use crate::coords::GalacticPosition;
use crate::galaxy::catalogue_classes::CatalogueCellKey;
#[cfg(doc)]
use crate::galaxy::catalogue_classes::ClassProcess;
use crate::galaxy::catalogue_classes::supernova::LIFETIME_REACH;
use crate::stellar::events::magnetar::MAX_BURST_DURATION;
use crate::time::{Span, UniverseTime};
use crate::units::consts::SECONDS_PER_JULIAN_YEAR;

use super::AlertQuery;
use super::arriving::SUPERNOVA_LIGHT_YEARS;
use super::recurrent::MAGNETAR_ACTIVE_SPAN;

/// A day, the cut's margin.
const DAY: Span = Span::from_seconds(86_400);

/// The light times from a class cell's box to an observer: from its nearest point and from its
/// farthest, each a light-year a Julian year.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BoxLight {
    near: Span,
    far: Span,
}

impl BoxLight {
    /// The light times from `key`'s closed box to `observer`.
    #[must_use]
    fn of(observer: &GalacticPosition, key: &CatalogueCellKey) -> Self {
        let centre = observer.to_light_years_f64();
        let edge = f64::from(key.edge_ly());
        let origin = key.origin_ly();
        let (mut near, mut far) = (0.0, 0.0);
        for axis in 0..3 {
            let low = f64::from(origin[axis]);
            let high = low + edge;
            let c = centre[axis];
            let gap = (low - c).max(c - high).max(0.0);
            let across = (c - low).abs().max((c - high).abs());
            near += gap * gap;
            far += across * across;
        }
        Self {
            near: years(near.sqrt()),
            far: years(far.sqrt()),
        }
    }
}

/// `years` Julian years as a span.
///
/// # Panics
///
/// For a figure beyond the clock's range, which no light time across the cube and no event's
/// light comes near.
#[must_use]
fn years(years: f64) -> Span {
    Span::from_seconds_f64(years * SECONDS_PER_JULIAN_YEAR)
        .expect("a light time or a light's span within the cube is a span")
}

/// `[from, to)` as a test on a time.
fn between(from: UniverseTime, to: UniverseTime) -> impl Fn(UniverseTime) -> bool {
    move |t| from <= t && t < to
}

/// The one-shot cut of `key` for `query` (module documentation): whether a supernova at T in the
/// cell can light the window.
///
/// # Panics
///
/// Never: the window is on the clock, and a cut's bounds lie within millennia of it.
pub(crate) fn one_shot(
    query: &AlertQuery,
    key: &CatalogueCellKey,
) -> impl Fn(UniverseTime) -> bool {
    let light = BoxLight::of(query.observer_position(), key);
    let window = query.window();
    let back = years(SUPERNOVA_LIGHT_YEARS)
        .checked_add(light.far)
        .and_then(|span| span.checked_add(DAY))
        .expect("centuries and a light time are a span");
    let from = window
        .start()
        .checked_sub(back)
        .expect("a cut's start lies within millennia of the clock");
    let to = window
        .end()
        .checked_add(DAY)
        .and_then(|t| t.checked_sub(light.near))
        .expect("a cut's end lies within millennia of the clock");
    between(from, to)
}

/// The magnetar cut of `key` for `query` (module documentation): whether a magnetar born at T in
/// the cell, its remnant within [`LIFETIME_REACH`] of it, can light the window.
///
/// # Panics
///
/// Never, as [`one_shot`].
pub(crate) fn magnetars(
    query: &AlertQuery,
    key: &CatalogueCellKey,
) -> impl Fn(UniverseTime) -> bool {
    let light = BoxLight::of(query.observer_position(), key);
    let reach = years(LIFETIME_REACH.value());
    let window = query.window();
    let back = years(MAGNETAR_ACTIVE_SPAN.value())
        .checked_add(light.far)
        .and_then(|span| span.checked_add(reach))
        .and_then(|span| span.checked_add(MAX_BURST_DURATION))
        .and_then(|span| span.checked_add(MAX_BURST_DURATION))
        .and_then(|span| span.checked_add(Span::from_seconds(3 * 86_400)))
        .expect("millennia and a light time are a span");
    let from = window
        .start()
        .checked_sub(back)
        .expect("a cut's start lies within millennia of the clock");
    let to = window
        .end()
        .checked_add(reach)
        .and_then(|t| t.checked_add(Span::from_seconds(2 * 86_400)))
        .and_then(|t| t.checked_sub(light.near))
        .expect("a cut's end lies within millennia of the clock");
    between(from, to)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::catalogue_classes::ClassId;

    /// A box's light times are its nearest and farthest points' distances, a light-year a year:
    /// zero and the half-diagonal for an observer at its centre, and from outside it the distance
    /// to its near face and to its far corner.
    #[test]
    fn a_cells_light_times_are_its_nearest_and_farthest_points() {
        let key = CatalogueCellKey::new(ClassId::CORE_COLLAPSE, [0, 50, 0]).unwrap();
        let at = |ly: [f64; 3]| GalacticPosition::from_light_years(ly).unwrap();
        let inside = BoxLight::of(&at([256.0, 25_856.0, 256.0]), &key);
        assert_eq!(inside.near, Span::ZERO);
        let half_diagonal = (3.0_f64 * 256.0 * 256.0).sqrt();
        let close = |span: Span, ly: f64| (span.as_julian_years_f64() - ly).abs() < 1e-6;
        assert!(close(inside.far, half_diagonal), "{inside:?}");
        let beside = BoxLight::of(&at([1_024.0, 25_856.0, 256.0]), &key);
        assert!(close(beside.near, 512.0), "{beside:?}");
        let corner = (1_024.0_f64 * 1_024.0 + 2.0 * 256.0 * 256.0).sqrt();
        assert!(close(beside.far, corner), "{beside:?}");
    }
}
