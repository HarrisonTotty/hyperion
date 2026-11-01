//! Test helpers for the catalogue classes (plan 09, "Test helpers"): the catalogue's half of
//! complementarity for plan 11's binary classes (P11.T8).

use crate::stellar::binary::{BinaryTimeline, CarvedClass, SegmentKind, carved_class};
use crate::time::{CLOCK_WINDOW_H, LIGHT_CROSSING_L};
use crate::units::Years;

/// The evaluation ages across the source horizon [`assert_complementary`] reads, per entry, beside
/// a merger's T on either side.
const HORIZON_AGES: u32 = 16;

/// Plan 09's complementarity on the catalogue's side for plan 11's binary classes (P11.T8): each
/// entry, its class, its pair's timeline and its system's age at the epoch, passes
/// [`carved_class`] with its own class, the one test the grid applies to an evolved pair, and is
/// its class at every age of the source horizon: a merger class's pair before its time T and the
/// merged star from it, a year to either side of T among the ages; every other class's pair
/// unmerged and whole throughout, in the phase that makes it its class. Returns the entries
/// checked.
///
/// # Panics
///
/// On the first entry that fails, naming it by its place among them.
pub(crate) fn assert_complementary<'a>(
    entries: impl IntoIterator<Item = (CarvedClass, &'a BinaryTimeline, Years)>,
) -> usize {
    let (h, l) = (
        CLOCK_WINDOW_H.as_julian_years_f64(),
        LIGHT_CROSSING_L.as_julian_years_f64(),
    );
    let mut checked = 0;
    for (n, (class, timeline, age)) in entries.into_iter().enumerate() {
        assert_eq!(
            carved_class(timeline, age),
            Some(class),
            "entry {n} is not its own class"
        );
        let now = age.value();
        let (lo, hi) = ((now - (h + l)).max(0.0), now + h);
        let mut ages: Vec<f64> = (0..=HORIZON_AGES)
            .map(|k| lo + (hi - lo) * f64::from(k) / f64::from(HORIZON_AGES))
            .collect();
        let merger = timeline.merger_age();
        if let Some(t) = merger {
            ages.extend([t.value() - 1.0, t.value(), t.value() + 1.0]);
        }
        for at in ages.into_iter().filter(|a| (lo..=hi).contains(a)) {
            let kind = timeline.state_at(Years::new(at)).kind();
            match (class, merger) {
                (CarvedClass::StellarMerger | CarvedClass::NeutronStarMerger, Some(t)) => {
                    assert_eq!(
                        kind == SegmentKind::Merged,
                        at >= t.value(),
                        "entry {n} of {class:?} at age {at}, T {}: {kind:?}",
                        t.value()
                    );
                }
                (CarvedClass::StellarMerger | CarvedClass::NeutronStarMerger, None) => {
                    panic!("entry {n} of {class:?} has no merger")
                }
                (
                    CarvedClass::AccretingWdFast
                    | CarvedClass::AccretingWdSlow
                    | CarvedClass::XrayBinary,
                    _,
                ) => assert!(
                    matches!(
                        kind,
                        SegmentKind::StableTransfer { .. } | SegmentKind::Detached
                    ),
                    "entry {n} of {class:?} at age {at}: {kind:?}"
                ),
            }
        }
        checked += 1;
    }
    checked
}
