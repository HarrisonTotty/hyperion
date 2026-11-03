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
//! This task registers no class: the classes' tasks register theirs in [`registered_classes`],
//! P09.T36 the recently dead and the luminous blue variables of a catalogue feature
//! ([`recent`](super::recent)), held until generator version 27. The first class of the order, the
//! tidal disruptions' victims (P09.T30.b), is the galactic centre's alone and is listed by the
//! centre itself, at indices 1 upward after its black hole
//! ([`events::victims`](crate::galaxy::features::centre::events::victims)), so a class listed on
//! the centre after them starts past their count.

use std::num::NonZeroU16;
use std::sync::{Arc, OnceLock};

use crate::coords::{GalacticDisplacement, GalacticPosition, GalacticVelocity};
use crate::galaxy::Galaxy;
use crate::galaxy::catalogue_classes::binary::feature::{
    FeatureBinaries, ListedBinary, is_listed_binary,
};
use crate::galaxy::catalogue_classes::source::member_layers;
use crate::galaxy::catalogue_classes::{CatalogueMotion, ClassId, FEATURE_LEVEL_LIST_ORDER};
use crate::galaxy::placement::SystemRecord;
use crate::galaxy::query::{LayerCounts, LayerSet};
use crate::id::{CentreMemberId, FeatureMemberId, MemberSlot, SystemId, SystemIdKind};
use crate::math;
use crate::rng::{ObjectKey, Seed, Stream, tags};
use crate::stellar::system::SystemStars;
use crate::time::{CLOCK_WINDOW_H, UniverseTime};
use crate::units::LightYears;
use crate::units::consts::METRES_PER_LIGHT_YEAR;

use super::super::catalogue::{FeatureCatalogue, FeatureRecord};
use super::super::interior::ClassProfile;
use super::MemberRecord;
use super::recent::{FeatureStars, RecentClass, RecentEntry};

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
/// from generator version 26 its recently dead and its luminous blue variables (P09.T36, P09.T43;
/// `members::recent`), and from version 27 plan 11's binary classes after them (P11.T8.f;
/// `catalogue_classes::binary::feature`), none before (the tidal disruptions' victims, P09.T30.b,
/// are on the galactic centre's list alone, which the centre lists itself).
#[must_use]
pub fn registered_classes(galaxy: &Galaxy, feature: &FeatureRecord) -> Vec<ListClass> {
    let stars = FeatureStars::listing(galaxy, feature);
    let mut classes = stars
        .as_ref()
        .map_or_else(Vec::new, |stars| stars.list_classes(galaxy));
    if let Some(binaries) = FeatureBinaries::of_feature(galaxy, feature, stars.as_ref()) {
        classes.extend(binaries.list_classes());
    }
    classes
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
    /// The index of the classes' first entry: 1, or past the galactic centre's victims.
    first: u32,
    /// The feature's stars, which draw its recent classes' entries, where it registers any.
    stars: Option<Box<FeatureStars>>,
    /// The feature's hosts of plan 11's binary classes, which draw their entries, where it
    /// registers any (P11.T8.f).
    binaries: Option<Box<FeatureBinaries>>,
    /// Its core collapses that their test keeps, found once ([`kept_deaths`](Self::kept_deaths)).
    deaths: DeathsFound,
}

/// A feature's list's core collapses that their test keeps, each on its broken line
/// (fix-p09-recent-kicks), and the farthest any of their remnants lies from its candidate's line
/// by the clock window's end: what a range query reads of them (P09.T37), found once a list
/// ([`FeatureLevelList::kept_deaths`]). A cache may keep it apart from its list, which is far
/// larger, so that a query reads a list only where its entries can reach its sphere, and of a list
/// whose dead alone can, these.
#[derive(Debug, Clone, PartialEq)]
pub struct KeptDeaths {
    velocity: Option<GalacticVelocity>,
    reach: LightYears,
    deaths: Vec<KeptDeath>,
}

impl KeptDeaths {
    /// The velocity its list's entries move at before any death, its feature's bulk motion:
    /// `None` for a list that has neither stars nor binary hosts to draw them.
    #[must_use]
    pub const fn velocity(&self) -> Option<GalacticVelocity> {
        self.velocity
    }

    /// How far from its candidate's line any of them lies at any time of the clock window, ly,
    /// with margins over the rounding: zero for a list that keeps none.
    #[must_use]
    pub const fn reach(&self) -> LightYears {
        self.reach
    }

    /// Each, in index order.
    #[must_use]
    pub fn deaths(&self) -> &[KeptDeath] {
        &self.deaths
    }

    /// The bytes it owns on the heap, its deaths' buffer by its capacity: what a caller's
    /// byte-bounded cache charges it.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        self.deaths.capacity() * size_of::<KeptDeath>()
    }
}

/// One of a list's core collapses that its test keeps: its candidate's index and place, its
/// member's record and its broken line ([`RecentEntry::motion`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeptDeath {
    index: u16,
    candidate: GalacticPosition,
    record: SystemRecord,
    motion: CatalogueMotion,
}

impl KeptDeath {
    /// Its index on the list.
    #[must_use]
    pub const fn index(&self) -> u16 {
        self.index
    }

    /// Its candidate's place at the epoch, on whose line its progenitor moves.
    #[must_use]
    pub const fn candidate(&self) -> &GalacticPosition {
        &self.candidate
    }

    /// Its member's record.
    #[must_use]
    pub const fn record(&self) -> &SystemRecord {
        &self.record
    }

    /// Its broken line.
    #[must_use]
    pub const fn motion(&self) -> &CatalogueMotion {
        &self.motion
    }
}

/// A list's [`KeptDeaths`], found when first asked for: a function of the list, so no part of its
/// value, and two lists are equal whatever either has found.
#[derive(Debug, Clone, Default)]
struct DeathsFound(OnceLock<Arc<KeptDeaths>>);

impl PartialEq for DeathsFound {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl FeatureLevelList {
    /// The list of `feature` with the registered classes: no member zero, since no catalogue
    /// feature has one, and a catalogue feature's 13-bit index.
    #[must_use]
    pub fn of(galaxy: &Galaxy, feature: &FeatureRecord) -> Self {
        let stars = FeatureStars::listing(galaxy, feature);
        let mut classes = stars
            .as_ref()
            .map_or_else(Vec::new, |stars| stars.list_classes(galaxy));
        let recent = !classes.is_empty();
        let binaries = FeatureBinaries::of_feature(galaxy, feature, stars.as_ref())
            .filter(|binaries| !binaries.is_empty());
        if let Some(binaries) = &binaries {
            classes.extend(binaries.list_classes());
        }
        Self {
            stars: stars.filter(|_| recent).map(Box::new),
            binaries: binaries.map(Box::new),
            ..Self::with_classes(
                galaxy,
                feature.id().object_word(),
                feature.position(),
                MemberZero::Absent,
                ListIndex::Feature,
                classes,
            )
        }
    }

    /// The list of `feature` ([`of`](Self::of)) if it registers any class, plan 09's recent
    /// classes (P09.T36) or plan 11's binary classes (P11.T8.f): what the range query reads
    /// (P09.T37); `None` for a feature that registers none.
    #[must_use]
    pub fn listed_of(galaxy: &Galaxy, feature: &FeatureRecord) -> Option<Self> {
        let list = Self::of(galaxy, feature);
        if list.classes.is_empty() {
            return None;
        }
        // A query reads every list's dead (fix-p09-recent-kicks), so they are found with it, and a
        // cache that keeps the list charges their bytes; the list keeps them, so the reference is
        // not needed here.
        let _ = list.kept_deaths(galaxy);
        Some(list)
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
        Self::with_classes_from(
            galaxy,
            word,
            centre,
            member_zero,
            index,
            NonZeroU16::MIN,
            classes,
        )
    }

    /// [`with_classes`](Self::with_classes) with the classes' first entry at index `first`: the
    /// galactic centre's list, whose classes after its victims start past their count (P09.T36.b;
    /// the victims' record). Indices 1 to `first − 1` are the caller's, and count against the
    /// index's limit as members.
    ///
    /// # Panics
    ///
    /// As [`with_classes`](Self::with_classes), the indices below `first` counted with the
    /// expected members.
    #[must_use]
    pub(crate) fn with_classes_from(
        galaxy: &Galaxy,
        word: u64,
        centre: &GalacticPosition,
        member_zero: MemberZero,
        index: ListIndex,
        first: NonZeroU16,
        classes: Vec<ListClass>,
    ) -> Self {
        let first = u32::from(first.get());
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
        let expected: f64 = classes.iter().map(|c| c.expected).sum::<f64>() + f64::from(first - 1);
        let limit = index.limit();
        assert!(
            fits_list_index(expected, limit),
            "a feature-level list expecting {expected} members cannot keep eight standard \
             deviations under its {limit} index"
        );
        let mut room = limit - first;
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
            first,
            stars: None,
            binaries: None,
            deaths: DeathsFound::default(),
        }
    }

    /// The bytes it owns on the heap: its classes' and counts' buffers, by their capacities, each
    /// class's profile ([`ClassProfile::heap_bytes`]), and the feature's stars and binary hosts it
    /// keeps where it lists their classes, boxed. What a caller's byte-bounded cache charges a
    /// list (P09.T37).
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        self.classes.capacity() * size_of::<ListClass>()
            + self
                .classes
                .iter()
                .map(|class| class.profile.heap_bytes())
                .sum::<usize>()
            + self.counts.capacity() * size_of::<u32>()
            + self
                .stars
                .as_ref()
                .map_or(0, |stars| size_of::<FeatureStars>() + stars.heap_bytes())
            + self.binaries.as_ref().map_or(0, |binaries| {
                size_of::<FeatureBinaries>() + binaries.heap_bytes()
            })
            + self
                .deaths
                .0
                .get()
                .map_or(0, |found| size_of::<KeptDeaths>() + found.heap_bytes())
    }

    /// Its core collapses that their test keeps, found when first asked for and kept with the
    /// list (P09.T37; fix-p09-recent-kicks): each with its candidate's place, its member's record
    /// and its broken line, and how far from its candidate's line any lies by the clock window's
    /// end, +H. Its remnant leaves the line at its death, so the farthest is at +H, its kick's
    /// travel since; a margin of 10⁻⁹ of it and 10⁻⁶ ly covers the rounding of both lines.
    /// Empty for a list without its stars.
    ///
    /// # Panics
    ///
    /// Never: +H is on the clock, and no line slower than light leaves the addressable range by
    /// then.
    #[must_use]
    pub fn kept_deaths(&self, galaxy: &Galaxy) -> &Arc<KeptDeaths> {
        self.deaths.0.get_or_init(|| {
            let end = UniverseTime::EPOCH
                .checked_add(CLOCK_WINDOW_H)
                .expect("+H is on the clock");
            let mut deaths = Vec::new();
            let mut farthest: f64 = 0.0;
            let velocity = self.stars.as_deref().map(FeatureStars::velocity);
            for entry in self.entries() {
                if entry.class() != ClassId::CORE_COLLAPSE {
                    continue;
                }
                let Some((death, velocity)) =
                    self.recent_entry(galaxy, entry.index()).zip(velocity)
                else {
                    continue;
                };
                let motion = *death.motion();
                let line = CatalogueMotion::straight(*entry.position(), velocity).position_at(end);
                let off = LightYears::from(motion.position_at(end).distance_to(&line)).value();
                farthest = farthest.max(off);
                deaths.push(KeptDeath {
                    index: entry.index(),
                    candidate: *entry.position(),
                    record: *death.member().record(),
                    motion,
                });
            }
            Arc::new(KeptDeaths {
                velocity: self.entries_velocity(),
                reach: LightYears::new(if deaths.is_empty() {
                    0.0
                } else {
                    farthest * (1.0 + 1e-9) + 1e-6
                }),
                deaths,
            })
        })
    }

    /// Whether index 0 is a member.
    #[must_use]
    pub const fn member_zero(&self) -> MemberZero {
        self.member_zero
    }

    /// The number of indices in use: the classes' first index (index 0, a member or not, and any
    /// before the classes) plus every class's count.
    #[must_use]
    pub fn len(&self) -> u32 {
        self.first + self.counts.iter().sum::<u32>()
    }

    /// Whether the list holds no member at all: no member zero, no index before the classes' and
    /// no class's entry.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.member_zero == MemberZero::Absent
            && self.first == 1
            && self.counts.iter().all(|&n| n == 0)
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

    /// The entry at `index` of the classes' part, from its first index upward, or `None` for an
    /// index before it or past the list.
    #[must_use]
    pub fn entry(&self, index: u16) -> Option<ListEntry> {
        let mut first = self.first;
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
        (self.first..self.len()).filter_map(|i| {
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
    /// an entry of a recent class or of one of plan 11's binary classes is its member where the
    /// class's test keeps it ([`recent_entry`](Self::recent_entry),
    /// [`binary_entry`](Self::binary_entry)).
    #[must_use]
    pub fn member(&self, galaxy: &Galaxy, index: u16) -> Option<MemberRecord> {
        if let Some(entry) = self.recent_entry(galaxy, index) {
            return Some(*entry.member());
        }
        let binaries = self.binaries.as_ref()?;
        let entry = self.binary_entry(galaxy, index)?;
        Some(MemberRecord::of_listed_binary(
            *entry.record(),
            entry.member_class(),
            *entry.composition(),
            binaries.host().velocity(),
            entry.class(),
        ))
    }

    /// The entry at `index` of one of plan 11's binary classes (P11.T8.f), its class's entry drawn
    /// at its list host, or `None` for an index of no such class, past the list, or a candidate
    /// its class's draws or test refuse.
    #[must_use]
    pub fn binary_entry(&self, galaxy: &Galaxy, index: u16) -> Option<ListedBinary> {
        let binaries = self.binaries.as_ref()?;
        let entry = self.entry(index)?;
        if !is_listed_binary(entry.class()) {
            return None;
        }
        binaries.entry(galaxy, entry.class(), index, entry.position())
    }

    /// Every entry of plan 11's binary classes that their draws and tests keep, in index order.
    pub fn binary_entries<'a>(
        &'a self,
        galaxy: &'a Galaxy,
    ) -> impl Iterator<Item = ListedBinary> + 'a {
        self.entries()
            .filter(|entry| is_listed_binary(entry.class()))
            .filter_map(move |entry| self.binary_entry(galaxy, entry.index()))
    }

    /// The velocity a catalogue feature's entries move at, its bulk motion, which its recent
    /// classes' and plan 11's binary classes' members share (P09.T37): `None` for a list of
    /// neither, the galactic centre's among them.
    #[must_use]
    pub(crate) fn entries_velocity(&self) -> Option<GalacticVelocity> {
        self.stars
            .as_deref()
            .map(FeatureStars::velocity)
            .or_else(|| self.binaries.as_deref().map(|b| b.host().velocity()))
    }

    /// The layers the census counts the entries of `class`, one of the list's, in (P09.T37): a
    /// recent class's its primaries' ([`RecentClass::layer`]), one of plan 11's its catalogue
    /// class's (the class source's `member_layers`), none for another class.
    #[must_use]
    pub(crate) fn class_layers(galaxy: &Galaxy, class: ClassId) -> LayerSet {
        match RecentClass::of(class) {
            Some(recent) => LayerSet::EMPTY.with(recent.layer()),
            None => member_layers(class, galaxy.rules().binary_tables)
                .and_then(|sets| sets.first().copied())
                .unwrap_or(LayerSet::EMPTY),
        }
    }

    /// The census's share of a catalogue feature's listed classes in the ball of `radius` about
    /// `centre` at the epoch, added to `counts` in each layer [`class_layers`](Self::class_layers)
    /// gives its class (P09.T37): each class's expected candidates times the share of its profile
    /// between `d − radius` and `d + radius` from the feature's centre, `d` the ball's distance from
    /// it, a shell that holds the ball, so the count errs high, as
    /// [`SystemSource`](crate::galaxy::query::SystemSource)'s contract allows. Nothing for the
    /// galactic centre's list, whose entries draw their own places. Classes in list order.
    pub(crate) fn add_expected_within(
        &self,
        galaxy: &Galaxy,
        (centre, radius): (&GalacticPosition, LightYears),
        counts: &mut LayerCounts,
    ) {
        if self.entries_velocity().is_none() {
            return;
        }
        let distance = LightYears::from(self.centre.distance_to(centre)).value();
        let radius = radius.value();
        for class in &self.classes {
            let layers = Self::class_layers(galaxy, class.class);
            if layers.is_empty() {
                continue;
            }
            let share = class.profile.cumulative(distance + radius)
                - class.profile.cumulative(distance - radius);
            for layer in layers.iter() {
                counts.set(layer, counts.get(layer) + class.expected * share.max(0.0));
            }
        }
    }

    /// The entry at `index` of a recent class (P09.T36), its member, site and shell, or `None` for
    /// an index of no such class, past the list, or a candidate the class's test rejects.
    #[must_use]
    pub fn recent_entry(&self, galaxy: &Galaxy, index: u16) -> Option<RecentEntry> {
        let stars = self.stars.as_ref()?;
        let entry = self.entry(index)?;
        let class = RecentClass::of(entry.class())?;
        stars.entry(galaxy, class, index, entry.position())
    }

    /// Every entry of the recent classes that their tests keep, in index order.
    pub fn recent_entries<'a>(
        &'a self,
        galaxy: &'a Galaxy,
    ) -> impl Iterator<Item = RecentEntry> + 'a {
        self.entries()
            .filter_map(move |entry| self.recent_entry(galaxy, entry.index()))
    }
}

/// How the feature-level member `id` moves if it is a listed core collapse (plan 09, P09.T36):
/// its progenitor's line broken at its death, where its remnant takes its natal kick
/// (fix-p09-recent-kicks; [`RecentEntry::motion`]); `None` for any other member, a cell's, a list
/// entry of another class, or one its class's test refuses.
#[must_use]
pub fn listed_death_motion(galaxy: &Galaxy, id: FeatureMemberId) -> Option<CatalogueMotion> {
    let MemberSlot::FeatureLevel { index } = id.slot() else {
        return None;
    };
    let feature = FeatureCatalogue::resolve(galaxy, id.feature().into())?;
    let list = FeatureLevelList::of(galaxy, &feature);
    if list.entry(index)?.class() != ClassId::CORE_COLLAPSE {
        return None;
    }
    list.recent_entry(galaxy, index)
        .map(|entry| *entry.motion())
}

/// The stars of the listed binary member `system` (P11.T8.f): its feature's list's entry's, its
/// class's marked pair, or `None` if `system` names no entry of plan 11's binary classes on a
/// catalogue feature's list.
#[must_use]
pub(crate) fn listed_binary_stars(galaxy: &Galaxy, system: SystemId) -> Option<SystemStars> {
    let SystemIdKind::FeatureMember(member) = system.kind() else {
        return None;
    };
    let MemberSlot::FeatureLevel { index } = member.slot() else {
        return None;
    };
    let feature = FeatureCatalogue::resolve(galaxy, member.feature().into())?;
    FeatureLevelList::of(galaxy, &feature)
        .binary_entry(galaxy, index)
        .map(|entry| entry.stars(galaxy))
}

#[cfg(test)]
mod tests {
    use super::*;
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

    /// A list built from `first` (P09.T36.b), member zero absent.
    fn list_from(
        galaxy: &Galaxy,
        f: &FeatureRecord,
        first: u16,
        classes: Vec<ListClass>,
    ) -> FeatureLevelList {
        FeatureLevelList::with_classes_from(
            galaxy,
            f.id().object_word(),
            f.position(),
            MemberZero::Absent,
            ListIndex::Feature,
            NonZeroU16::new(first).unwrap(),
            classes,
        )
    }

    /// The classes' first index (P09.T36.b): a list from index 30 draws the counts that a list
    /// from 1 draws, each class's on its own words, and numbers its entries 29 later; its length
    /// counts the 29 indices before them, no index below 30 is an entry, and a list from 30 with no
    /// class is not empty.
    #[test]
    fn a_list_from_a_later_first_index_numbers_its_classes_from_there() {
        let galaxy = galaxy();
        let f = feature(&galaxy);
        let classes = || {
            vec![
                class(ClassId::CORE_COLLAPSE, 40.0),
                class(ClassId::TYPE_IA, 5.0),
            ]
        };
        let (from_one, later) = (
            list(&galaxy, &f, classes()),
            list_from(&galaxy, &f, 30, classes()),
        );
        for c in [ClassId::CORE_COLLAPSE, ClassId::TYPE_IA] {
            assert_eq!(later.count(c), from_one.count(c), "{c:?}");
        }
        assert_eq!(later.len(), from_one.len() + 29);
        for index in 0..30 {
            assert_eq!(later.entry(index), None, "{index}");
        }
        let shifted: Vec<_> = from_one
            .entries()
            .map(|e| (e.index() + 29, e.class(), e.number()))
            .collect();
        let entries: Vec<_> = later
            .entries()
            .map(|e| (e.index(), e.class(), e.number()))
            .collect();
        assert_eq!(entries, shifted);
        assert!(!later.is_empty());
        let none = list_from(&galaxy, &f, 30, Vec::new());
        assert_eq!(none.len(), 30);
        assert!(!none.is_empty());
        assert!(list(&galaxy, &f, Vec::new()).is_empty());
    }

    /// The indices before the first count against the index's limit: 7,000 expected members fit a
    /// catalogue feature's 8,192 from index 1, and not from index 2,000.
    #[test]
    #[should_panic(expected = "eight standard deviations under its 8192 index")]
    fn the_indices_before_the_first_count_against_the_limit() {
        let galaxy = galaxy();
        let f = feature(&galaxy);
        let classes = || vec![class(ClassId::CORE_COLLAPSE, 7_000.0)];
        let _ = list(&galaxy, &f, classes());
        let _ = list_from(&galaxy, &f, 2_000, classes());
    }
}
