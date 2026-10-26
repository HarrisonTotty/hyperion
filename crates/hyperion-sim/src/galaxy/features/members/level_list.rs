//! A feature's feature-level member list (plan 09, P09.T22; Design note 16; brainstorm, "Dense
//! features").
//!
//! Members under plan 01's [`MemberSlot::FeatureLevel`] (band value 7) are drawn on the feature's
//! own streams, not in a cell. Index 0 is member zero where the kind has one (the galactic centre's
//! black hole, P09.T27; no catalogue feature has one). From 1 upward come the list's classes, in
//! [`FEATURE_LEVEL_LIST_ORDER`], each with a Poisson count of its own, and each member's position
//! is drawn by inverse transform of its class's radial cumulative distribution and an isotropic
//! direction, which is exact.
//!
//! # Streams
//!
//! Both draws are keyed by `ObjectKey::feature(word)`, the feature's object word, and read at word
//! offsets fixed by the class, so that registering a class moves no other class's count or
//! positions, only, when it sits earlier in the list order, the indices after it:
//!
//! - `member.list`: class `c`'s count at word `c × 2³²` (its [`ClassId`] value).
//! - `member.list_position`: member `k` of class `c` at word `c × 2³² + 3k`, three uniforms, the
//!   radius's quantile, then the direction's `cos θ` and azimuth.
//!
//! # Capacity
//!
//! The index is 13 bits on a catalogue feature's or a dwarf core's list and 16 bits on the galactic
//! centre's ([`ListIndex`]; plan 01, Design note 17, as ruled 2026-10-26 and built by P11.T8.e.b).
//! The list's summed expected count plus eight standard deviations must stay under its index's
//! limit, 8,192 or 65,536 ([`FeatureLevelList::with_classes`] asserts it), and the count is clamped
//! there, the later classes losing members first.
//!
//! The bound, `expected + 8√expected + 1 < limit` with index 0 spared whether or not it is a
//! member, is one function, which plan 11's
//! [`check_feature_list_headroom`](super::check_feature_list_headroom) shares (P11.T8.e) at the
//! centre's limit, so that a centre the check passes cannot fail the assertion when P11.T8.f builds
//! its list. It holds up to `(√(limit + 15) − 4)²` expected members: 7,498.3 at 8,192 and 63,518.8
//! at 65,536.
//!
//! This task registers no class: [`FeatureLevelList::of`] lists none, and the classes' tasks
//! (P09.T30.b, T36 and T43) register theirs in [`registered_classes`].

use crate::coords::{GalacticDisplacement, GalacticPosition};
use crate::galaxy::Galaxy;
use crate::galaxy::catalogue_classes::{ClassId, FEATURE_LEVEL_LIST_ORDER};
use crate::id::{CentreMemberId, MemberSlot};
use crate::math;
use crate::rng::{ObjectKey, Seed, Stream, tags};
use crate::units::consts::METRES_PER_LIGHT_YEAR;

use super::super::catalogue::FeatureRecord;
use super::super::interior::ClassProfile;
use super::MemberRecord;

/// The words each class owns on the list's streams: 2³², far beyond any count's draws.
const CLASS_WORDS: u64 = 1 << 32;

/// Standard deviations of a list's Poisson count that its index must spare: eight, plan 03's rule
/// for its cells (its Design note 6), which P09.T22 takes for the list.
pub(crate) const LIST_HEADROOM_SIGMAS: f64 = 8.0;

/// Whether a list expecting `expected` members in all keeps [`LIST_HEADROOM_SIGMAS`] standard
/// deviations and index 0 under its index's exclusive bound `limit` ([`ListIndex::limit`]):
/// `expected + 8√expected + 1 < limit`, the bound [`FeatureLevelList::with_classes`] asserts
/// (module documentation).
///
/// A count that is NaN fits nothing.
#[must_use]
pub(crate) fn fits_list_index(expected: f64, limit: u32) -> bool {
    expected + LIST_HEADROOM_SIGMAS * expected.sqrt() + 1.0 < f64::from(limit)
}

/// The indices a list expecting `expected` members in all leaves free under its index's exclusive
/// bound `limit` once `sigmas` standard deviations and index 0 are spared: `limit − (expected +
/// sigmas √expected + 1)`, positive where it fits.
///
/// At [`LIST_HEADROOM_SIGMAS`] it is positive exactly where [`fits_list_index`] holds, for a
/// finite count: the difference of two finite doubles is zero only where they are equal.
#[must_use]
pub(crate) fn list_index_margin(expected: f64, sigmas: f64, limit: u32) -> f64 {
    f64::from(limit) - (expected + sigmas * expected.sqrt() + 1.0)
}

/// The index a feature-level list numbers its entries with (plan 01, Design note 17).
///
/// The galactic centre's list is the longest, holding plan 11's carved binary classes, and its
/// index alone was widened from 13 bits to 16 (ruled 2026-10-26, P11.T8.e.b): every earlier ID
/// keeps its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ListIndex {
    /// A catalogue feature's or a dwarf core's: 13 bits, 8,192 entries
    /// ([`MemberSlot::INDEX_LIMIT`]).
    Feature,
    /// The galactic centre's: 16 bits, 65,536 entries ([`CentreMemberId::LIST_INDEX_LIMIT`]).
    Centre,
}

impl ListIndex {
    /// The exclusive bound of the index: 8,192, or 65,536 at the centre.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::galaxy::features::members::ListIndex;
    ///
    /// // A list's expected members plus eight standard deviations and index 0 must stay under it.
    /// assert_eq!(ListIndex::Feature.limit(), 8_192);
    /// assert_eq!(ListIndex::Centre.limit(), 65_536);
    /// ```
    #[must_use]
    pub fn limit(self) -> u32 {
        match self {
            Self::Feature => u32::from(MemberSlot::INDEX_LIMIT),
            Self::Centre => CentreMemberId::LIST_INDEX_LIMIT,
        }
    }
}

/// A class on a feature's list: its expected members and its profile about the feature's centre.
#[derive(Debug, Clone, PartialEq)]
pub struct ListClass {
    class: ClassId,
    expected: f64,
    profile: ClassProfile,
}

impl ListClass {
    /// Class `class` expecting `expected` members on the list, placed by `profile`.
    ///
    /// # Panics
    ///
    /// If `expected` is negative or not finite.
    #[must_use]
    pub fn new(class: ClassId, expected: f64, profile: ClassProfile) -> Self {
        assert!(
            expected >= 0.0 && expected.is_finite(),
            "a list class expecting {expected} members"
        );
        Self {
            class,
            expected,
            profile,
        }
    }

    /// Its class.
    #[must_use]
    pub const fn class(&self) -> ClassId {
        self.class
    }

    /// Its expected members.
    #[must_use]
    pub const fn expected(&self) -> f64 {
        self.expected
    }
}

/// One entry of a feature-level list: its index, its class and number within the class, and its
/// position at the epoch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ListEntry {
    index: u16,
    class: ClassId,
    number: u32,
    position: GalacticPosition,
}

impl ListEntry {
    /// Its index on the list, 1 upward.
    #[must_use]
    pub const fn index(&self) -> u16 {
        self.index
    }

    /// Its class.
    #[must_use]
    pub const fn class(&self) -> ClassId {
        self.class
    }

    /// Its number within its class, 0 upward.
    #[must_use]
    pub const fn number(&self) -> u32 {
        self.number
    }

    /// Its position at the epoch.
    #[must_use]
    pub const fn position(&self) -> &GalacticPosition {
        &self.position
    }
}

/// Whether a list's index 0 is a member.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MemberZero {
    /// Index 0 is unused: every catalogue feature's list.
    Absent,
    /// Index 0 is a member, such as the galactic centre's black hole (P09.T27).
    Present,
}

/// The classes registered on `feature`'s list, in list order, with their counts and profiles:
/// none until P09.T30.b, T36 and T43 register theirs.
#[must_use]
pub fn registered_classes(_galaxy: &Galaxy, _feature: &FeatureRecord) -> Vec<ListClass> {
    Vec::new()
}

/// A feature's feature-level list (module documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct FeatureLevelList {
    seed: Seed,
    word: u64,
    centre: GalacticPosition,
    member_zero: MemberZero,
    classes: Vec<ListClass>,
    /// Each class's count after the clamp, in list order.
    counts: Vec<u32>,
}

impl FeatureLevelList {
    /// The list of `feature` with the registered classes: no member zero, since no catalogue
    /// feature has one, and a catalogue feature's 13-bit index.
    #[must_use]
    pub fn of(galaxy: &Galaxy, feature: &FeatureRecord) -> Self {
        Self::with_classes(
            galaxy,
            feature.id().object_word(),
            feature.position(),
            MemberZero::Absent,
            ListIndex::Feature,
            registered_classes(galaxy, feature),
        )
    }

    /// The list keyed by the feature word `word` about `centre`, with or without member zero,
    /// numbered by `index`, and `classes` (module documentation).
    ///
    /// # Panics
    ///
    /// - If `classes` are not in [`FEATURE_LEVEL_LIST_ORDER`] or one appears twice.
    /// - If the summed expected count plus eight standard deviations and index 0 reaches the
    ///   index's limit, [`ListIndex::limit`]: 8,192, or 65,536 at the centre.
    #[must_use]
    pub fn with_classes(
        galaxy: &Galaxy,
        word: u64,
        centre: &GalacticPosition,
        member_zero: MemberZero,
        index: ListIndex,
        classes: Vec<ListClass>,
    ) -> Self {
        let place = |c: ClassId| {
            FEATURE_LEVEL_LIST_ORDER
                .iter()
                .position(|&o| o == c)
                .expect("a list class is in the list order")
        };
        assert!(
            classes
                .windows(2)
                .all(|w| place(w[0].class) < place(w[1].class)),
            "a list's classes must be distinct and in the list order"
        );
        let expected: f64 = classes.iter().map(|c| c.expected).sum();
        let limit = index.limit();
        assert!(
            fits_list_index(expected, limit),
            "a feature-level list expecting {expected} members cannot keep eight standard \
             deviations under its {limit} index"
        );
        let mut room = limit - 1;
        let counts = classes
            .iter()
            .map(|c| {
                let mut stream =
                    Stream::open(galaxy.seed(), tags::MEMBER_LIST, ObjectKey::feature(word));
                stream.seek(CLASS_WORDS * u64::from(c.class.value()));
                let drawn = u32::try_from(stream.poisson(c.expected)).unwrap_or(u32::MAX);
                let kept = drawn.min(room);
                room -= kept;
                kept
            })
            .collect();
        Self {
            seed: galaxy.seed(),
            word,
            centre: *centre,
            member_zero,
            classes,
            counts,
        }
    }

    /// Whether index 0 is a member.
    #[must_use]
    pub const fn member_zero(&self) -> MemberZero {
        self.member_zero
    }

    /// The number of indices in use: 1 (index 0, a member or not) plus every class's count.
    #[must_use]
    pub fn len(&self) -> u32 {
        1 + self.counts.iter().sum::<u32>()
    }

    /// Whether the list holds no member at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.member_zero == MemberZero::Absent && self.counts.iter().all(|&n| n == 0)
    }

    /// The count of `class` on the list, 0 if it is not registered.
    #[must_use]
    pub fn count(&self, class: ClassId) -> u32 {
        self.classes
            .iter()
            .zip(&self.counts)
            .find(|(c, _)| c.class == class)
            .map_or(0, |(_, &n)| n)
    }

    /// The entry at `index` of the classes' part, 1 upward, or `None` for index 0 or an index past
    /// the list.
    #[must_use]
    pub fn entry(&self, index: u16) -> Option<ListEntry> {
        let mut first = 1_u32;
        let wanted = u32::from(index);
        for (class, &n) in self.classes.iter().zip(&self.counts) {
            if (first..first + n).contains(&wanted) {
                let number = wanted - first;
                return Some(ListEntry {
                    index,
                    class: class.class,
                    number,
                    position: self.position_of(class, number),
                });
            }
            first += n;
        }
        None
    }

    /// Every entry of the classes' part, in index order.
    ///
    /// # Panics
    ///
    /// Never: a list's length is clamped at its index's limit, at most 65,536 at the centre, so
    /// each index below it, at most 65,535, fits in `u16`.
    pub fn entries(&self) -> impl Iterator<Item = ListEntry> + '_ {
        (1..self.len()).filter_map(|i| {
            self.entry(
                u16::try_from(i).expect("a list's indices are below its length, 65,536 at most"),
            )
        })
    }

    /// Member `number` of `class`'s position: the radius's quantile of its profile, an isotropic
    /// direction.
    #[must_use]
    fn position_of(&self, class: &ListClass, number: u32) -> GalacticPosition {
        let mut stream = Stream::open(
            self.seed,
            tags::MEMBER_LIST_POSITION,
            ObjectKey::feature(self.word),
        );
        stream.seek(CLASS_WORDS * u64::from(class.class.value()) + 3 * u64::from(number));
        let r = class.profile.radius_quantile(stream.uniform());
        let cos_theta = 2.0 * stream.uniform() - 1.0;
        let phi = 2.0 * core::f64::consts::PI * stream.uniform();
        let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
        let (s, c) = math::sin_cos(phi);
        let local = [r * sin_theta * c, r * sin_theta * s, r * cos_theta];
        self.centre
            .translated(GalacticDisplacement::new(
                local.map(|x| x * METRES_PER_LIGHT_YEAR),
            ))
            .expect("a list member lies within its feature's reach of a centre in the root cube")
    }

    /// The member at `index`, or `None`: no member zero is generated for a catalogue feature, and
    /// the classes' members are drawn by the classes' own tasks, which register them.
    #[must_use]
    pub fn member(&self, index: u16) -> Option<MemberRecord> {
        self.entry(index).and(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::features::catalogue::FeatureCatalogue;
    use crate::galaxy::features::interior::ProfileShape;
    use crate::galaxy::features::members::resolve_member;
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::placement::ResolveSystemError;
    use crate::id::{FeatureCell, FeatureMemberId, SystemId, SystemIdKind};
    use hyperion_testkit::float::assert_same_bits;

    fn galaxy() -> Galaxy {
        Galaxy::from_params(Seed::new(0x0922_0000), GalaxyParams::milky_way_like()).unwrap()
    }

    fn feature(galaxy: &Galaxy) -> FeatureRecord {
        FeatureCatalogue::cell(galaxy, FeatureCell::new([0, 6, 0]).unwrap()).features()[0]
    }

    fn class(class: ClassId, expected: f64) -> ListClass {
        let shape = ProfileShape::Plummer { scale: 3.0 };
        ListClass::new(class, expected, ClassProfile::new(shape, 60.0, 1.0))
    }

    fn list(galaxy: &Galaxy, f: &FeatureRecord, classes: Vec<ListClass>) -> FeatureLevelList {
        FeatureLevelList::with_classes(
            galaxy,
            f.id().object_word(),
            f.position(),
            MemberZero::Absent,
            ListIndex::Feature,
            classes,
        )
    }

    /// A galactic centre's list, member zero present, whose one class expects `expected` members.
    fn centre_list(galaxy: &Galaxy, expected: f64) -> FeatureLevelList {
        let word = SystemId::from(CentreMemberId::CENTRAL_BLACK_HOLE).raw();
        FeatureLevelList::with_classes(
            galaxy,
            word,
            &GalacticPosition::ORIGIN,
            MemberZero::Present,
            ListIndex::Centre,
            vec![class(ClassId::ACCRETING_WHITE_DWARF, expected)],
        )
    }

    /// The largest expectation the bound keeps under `limit`: the root of `c + 8√c + 1 = limit`,
    /// `(√(limit + 15) − 4)²`.
    fn largest_fitting(limit: u32) -> f64 {
        let root = (f64::from(limit) + 15.0).sqrt() - 4.0;
        root * root
    }

    #[test]
    fn with_no_class_registered_a_catalogue_feature_s_list_is_empty() {
        let galaxy = galaxy();
        let f = feature(&galaxy);
        let list = FeatureLevelList::of(&galaxy, &f);
        assert!(list.is_empty());
        assert_eq!(list.len(), 1);
        assert_eq!(list.entry(0), None);
        assert_eq!(list.entry(1), None);
        for index in [0, 1, 8_191] {
            let id = FeatureMemberId::new(f.id().feature_ref(), MemberSlot::FeatureLevel { index })
                .unwrap();
            assert_eq!(
                resolve_member(
                    &galaxy,
                    &crate::galaxy::features::members::NoInteriorCache,
                    id
                ),
                Err(ResolveSystemError::NoSuchSystem)
            );
        }
    }

    #[test]
    fn a_list_s_entries_resolve_and_their_ids_are_canonical() {
        let galaxy = galaxy();
        let f = feature(&galaxy);
        let list = list(
            &galaxy,
            &f,
            vec![
                class(ClassId::CORE_COLLAPSE, 40.0),
                class(ClassId::TYPE_IA, 12.0),
            ],
        );
        let n = list.count(ClassId::CORE_COLLAPSE);
        assert!(n > 10, "{n} core collapses");
        let entries: Vec<ListEntry> = list.entries().collect();
        assert_eq!(u32::try_from(entries.len()).unwrap(), list.len() - 1);
        for (i, e) in entries.iter().enumerate() {
            assert_eq!(usize::from(e.index()), i + 1);
            assert_eq!(list.entry(e.index()), Some(*e));
            let class = if u32::from(e.index()) <= n {
                ClassId::CORE_COLLAPSE
            } else {
                ClassId::TYPE_IA
            };
            assert_eq!(e.class(), class);
            // Inside the profile's tidal radius about the feature.
            let d = f.position().distance_to(e.position()).value() / METRES_PER_LIGHT_YEAR;
            assert!(d <= 60.0 * (1.0 + 1e-9), "{d} ly");
            // The ID: band 7, level and cell bits zero, and it decodes back to the slot.
            let id = FeatureMemberId::new(
                f.id().feature_ref(),
                MemberSlot::FeatureLevel { index: e.index() },
            )
            .unwrap();
            let raw = SystemId::from(id).raw();
            assert_eq!(raw & 0x7fff_e000, 7 << 28, "{raw:#x}");
            let SystemIdKind::FeatureMember(back) = SystemId::from_raw(raw).unwrap().kind() else {
                panic!("a feature member's ID decodes as one")
            };
            assert_eq!(back.slot(), MemberSlot::FeatureLevel { index: e.index() });
        }
        assert_eq!(list.entry(u16::try_from(list.len()).unwrap()), None);
    }

    #[test]
    fn registering_a_later_class_leaves_the_first_s_members_where_they_were() {
        let galaxy = galaxy();
        let f = feature(&galaxy);
        let alone = list(&galaxy, &f, vec![class(ClassId::CORE_COLLAPSE, 30.0)]);
        let both = list(
            &galaxy,
            &f,
            vec![
                class(ClassId::CORE_COLLAPSE, 30.0),
                class(ClassId::LUMINOUS_BLUE_VARIABLE, 5.0),
            ],
        );
        let first: Vec<ListEntry> = alone.entries().collect();
        let prefix: Vec<ListEntry> = both.entries().take(first.len()).collect();
        assert_eq!(first, prefix);
        // A class registered in front moves only the indices after it, not the members.
        let ia = list(&galaxy, &f, vec![class(ClassId::TYPE_IA, 9.0)]);
        let front = list(
            &galaxy,
            &f,
            vec![
                class(ClassId::CORE_COLLAPSE, 30.0),
                class(ClassId::TYPE_IA, 9.0),
            ],
        );
        assert_eq!(ia.count(ClassId::TYPE_IA), front.count(ClassId::TYPE_IA));
        let moved: Vec<(u32, GalacticPosition)> = front
            .entries()
            .filter(|e| e.class() == ClassId::TYPE_IA)
            .map(|e| (e.number(), *e.position()))
            .collect();
        let kept: Vec<(u32, GalacticPosition)> =
            ia.entries().map(|e| (e.number(), *e.position())).collect();
        assert_eq!(moved, kept);
    }

    #[test]
    #[should_panic(expected = "distinct and in the list order")]
    fn classes_out_of_list_order_are_refused() {
        let galaxy = galaxy();
        let f = feature(&galaxy);
        let _ = list(
            &galaxy,
            &f,
            vec![
                class(ClassId::TYPE_IA, 1.0),
                class(ClassId::CORE_COLLAPSE, 1.0),
            ],
        );
    }

    #[test]
    #[should_panic(expected = "eight standard deviations")]
    fn a_list_that_could_overflow_its_index_is_refused() {
        let galaxy = galaxy();
        let f = feature(&galaxy);
        let _ = list(&galaxy, &f, vec![class(ClassId::CORE_COLLAPSE, 7_500.0)]);
    }

    /// The bound's root at each limit: (√8,207 − 4)² = 7,498.26 and (√65,551 − 4)² = 63,518.77.
    /// Just under it the bound holds and a list builds, just over it the bound fails.
    #[test]
    fn each_index_keeps_eight_standard_deviations_up_to_its_root() {
        let galaxy = galaxy();
        let f = feature(&galaxy);
        for (index, want) in [
            (ListIndex::Feature, 7_498.26),
            (ListIndex::Centre, 63_518.77),
        ] {
            let limit = index.limit();
            let largest = largest_fitting(limit);
            assert!((largest - want).abs() < 0.005, "{index:?}: {largest}");
            let (under, over) = (largest * (1.0 - 1e-9), largest * (1.0 + 1e-9));
            assert!(fits_list_index(under, limit), "{index:?}");
            assert!(!fits_list_index(over, limit), "{index:?}");
            assert!(list_index_margin(under, 8.0, limit) > 0.0, "{index:?}");
            assert!(list_index_margin(over, 8.0, limit) < 0.0, "{index:?}");
            let built = match index {
                ListIndex::Feature => list(&galaxy, &f, vec![class(ClassId::CORE_COLLAPSE, under)]),
                ListIndex::Centre => centre_list(&galaxy, under),
            };
            assert!(
                f64::from(built.len()) > 0.95 * under,
                "{index:?}: {}",
                built.len()
            );
            assert!(built.len() < limit, "{index:?}: {}", built.len());
        }
        assert_eq!(ListIndex::Feature.limit(), 8_192);
        assert_eq!(ListIndex::Centre.limit(), 65_536);
    }

    #[test]
    #[should_panic(expected = "eight standard deviations under its 8192 index")]
    fn a_catalogue_feature_s_list_just_over_its_bound_is_refused() {
        let galaxy = galaxy();
        let f = feature(&galaxy);
        let over = largest_fitting(8_192) * (1.0 + 1e-9);
        let _ = list(&galaxy, &f, vec![class(ClassId::CORE_COLLAPSE, over)]);
    }

    #[test]
    #[should_panic(expected = "eight standard deviations under its 65536 index")]
    fn the_centre_s_list_just_over_its_bound_is_refused() {
        let _ = centre_list(&galaxy(), largest_fitting(65_536) * (1.0 + 1e-9));
    }

    /// What a catalogue feature's list refuses, 7,500 members, the centre's builds, and its
    /// entries number past 8,191 with canonical IDs that share the list's word.
    #[test]
    fn the_centre_s_list_numbers_its_entries_past_8191() {
        let galaxy = galaxy();
        let list = centre_list(&galaxy, 9_000.0);
        assert!(list.len() > 8_193, "{}", list.len());
        let mut n = 0_u32;
        for e in list.entries() {
            n += 1;
            assert_eq!(u32::from(e.index()), n);
            let id = SystemId::from(
                CentreMemberId::new(MemberSlot::FeatureLevel { index: e.index() }).unwrap(),
            );
            assert_eq!(
                id.cell_word(),
                SystemId::from(CentreMemberId::CENTRAL_BLACK_HOLE).raw()
            );
        }
        assert_eq!(n, list.len() - 1);
        assert!(f64::from(centre_list(&galaxy, 7_500.0).len()) > 7_000.0);
    }

    /// The index's width sets only the bound and the clamp's room: below the clamp the same word
    /// and classes draw the same counts and the same entries, bit for bit, under either width.
    #[test]
    fn the_index_s_width_moves_no_draw() {
        let galaxy = galaxy();
        let f = feature(&galaxy);
        let build = |index| {
            FeatureLevelList::with_classes(
                &galaxy,
                f.id().object_word(),
                f.position(),
                MemberZero::Absent,
                index,
                vec![
                    class(ClassId::CORE_COLLAPSE, 300.0),
                    class(ClassId::TYPE_IA, 40.0),
                ],
            )
        };
        let (narrow, wide) = (build(ListIndex::Feature), build(ListIndex::Centre));
        assert_eq!(narrow.counts, wide.counts);
        assert!(narrow.len() > 300, "{}", narrow.len());
        for (a, b) in narrow.entries().zip(wide.entries()) {
            assert_eq!(
                (a.index(), a.class(), a.number()),
                (b.index(), b.class(), b.number())
            );
            assert_eq!(a.position().cell(), b.position().cell());
            let offsets = [a, b].map(|e| e.position().offset_metres());
            for (x, y) in offsets[0].into_iter().zip(offsets[1]) {
                assert_same_bits(x, y);
            }
        }
    }

    /// A centre's list at its limit, 65,535 entries after index 0, has 65,536 indices, which no
    /// `u16` holds: its entries still run to 65,535.
    #[test]
    fn a_full_centre_list_s_entries_run_to_65535() {
        let galaxy = galaxy();
        let mut full = centre_list(&galaxy, 1.0);
        full.counts = vec![u32::from(u16::MAX)];
        assert_eq!(full.len(), 65_536);
        let (mut n, mut last) = (0_u32, 0_u16);
        for e in full.entries() {
            n += 1;
            last = e.index();
        }
        assert_eq!((n, last), (65_535, u16::MAX));
    }
}
