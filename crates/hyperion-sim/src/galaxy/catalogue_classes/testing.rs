//! Test helpers for the catalogue classes (plan 09, "Test helpers"): the catalogue's half of
//! complementarity for plan 11's binary classes (P11.T8), and a table of the X-ray binaries'
//! fitted marks that draws their high-mass kinds often enough for a sample of cells to hold them.

use crate::stellar::binary::{BinaryTimeline, CarvedClass, SegmentKind, carved_class};
use crate::tables::binary::{
    ClassSamplerTable, SamplerPick, XRB_LOW_MASS_KINDS_27, XRB_SAMPLER, XrbMark, XrbPick,
};
use crate::time::{CLOCK_WINDOW_H, LIGHT_CROSSING_L};
use crate::units::Years;

/// A table of the X-ray binaries' fitted marks (P15.T10.b's refit at 27) that draws the high-mass
/// side in half the class's hosts, Be/X and supergiant hosts half each, and the low-mass kinds in
/// their fitted weights, so that sample cells across the inner disc hold some dozens of high-mass
/// hosts. The refit gives the high-mass kinds 0.0010 of the class's hosts, about ten in the galaxy
/// and a supergiant host 0.06 of one, too few for any sample of cells to hold (the scratch
/// sampler gave them 0.16). Its hosts are the class's marks under other kinds' weights: the
/// class's own cells are not theirs, and their IDs resolve to none of them.
pub(crate) static HIGH_MASS_XRB: ClassSamplerTable<XrbMark, XrbPick> = ClassSamplerTable::new(
    XRB_SAMPLER.marks(),
    &[
        SamplerPick::new("low_mass_kind", &XRB_LOW_MASS_KINDS_27),
        SamplerPick::new("high_mass_kind", &[0.5, 0.5, 0.0]),
    ],
);

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
