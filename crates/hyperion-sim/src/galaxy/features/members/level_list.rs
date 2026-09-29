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
//! The index is 13 bits. The list's summed expected count plus eight standard deviations must stay
//! under 8,192 ([`FeatureLevelList::with_classes`] asserts it), and the count is clamped there, the
//! later classes losing members first.
//!
//! This task registers no class: [`FeatureLevelList::of`] lists none, and the classes' tasks
//! (P09.T30.b, T36 and T43) register theirs in [`registered_classes`].

use crate::coords::{GalacticDisplacement, GalacticPosition};
use crate::galaxy::Galaxy;
use crate::galaxy::catalogue_classes::{ClassId, FEATURE_LEVEL_LIST_ORDER};
use crate::id::MemberSlot;
use crate::math;
use crate::rng::{ObjectKey, Seed, Stream, tags};
use crate::units::consts::METRES_PER_LIGHT_YEAR;

use super::super::catalogue::FeatureRecord;
use super::super::interior::ClassProfile;
use super::MemberRecord;

/// The words each class owns on the list's streams: 2³², far beyond any count's draws.
const CLASS_WORDS: u64 = 1 << 32;

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
    /// feature has one.
    #[must_use]
    pub fn of(galaxy: &Galaxy, feature: &FeatureRecord) -> Self {
        Self::with_classes(
            galaxy,
            feature.id().object_word(),
            feature.position(),
            MemberZero::Absent,
            registered_classes(galaxy, feature),
        )
    }

    /// The list keyed by the feature word `word` about `centre`, with or without member zero, and
    /// `classes` (module documentation).
    ///
    /// # Panics
    ///
    /// - If `classes` are not in [`FEATURE_LEVEL_LIST_ORDER`] or one appears twice.
    /// - If the summed expected count plus eight standard deviations reaches the 8,192 index.
    #[must_use]
    pub fn with_classes(
        galaxy: &Galaxy,
        word: u64,
        centre: &GalacticPosition,
        member_zero: MemberZero,
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
        let limit = f64::from(MemberSlot::INDEX_LIMIT);
        assert!(
            expected + 8.0 * expected.sqrt() + 1.0 < limit,
            "a feature-level list expecting {expected} members cannot keep eight standard \
             deviations under its 8,192 index"
        );
        let mut room = u32::from(MemberSlot::INDEX_LIMIT) - 1;
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
    /// Never: a list's length is clamped at 8,192.
    pub fn entries(&self) -> impl Iterator<Item = ListEntry> + '_ {
        let last = u16::try_from(self.len()).expect("a list's length is at most 8,192");
        (1..last).filter_map(|i| self.entry(i))
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
            classes,
        )
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
}
