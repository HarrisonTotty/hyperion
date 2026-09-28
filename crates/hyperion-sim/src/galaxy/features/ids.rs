//! Feature identifiers, processes and kinds (plan 09, P09.T1).
//!
//! Plan 01 builds the bit layouts: a feature is a [`FeatureRef`], a 4,096 ly [`FeatureCell`] and a
//! 14-bit candidate index in it. This module gives them meaning. A [`FeatureId`] wraps the
//! reference and names it; a [`FeatureProcess`] is one of the catalogue's Poisson processes, whose
//! candidates share a cell's index space in a fixed order (Design note 6); a [`FeatureKind`] is
//! what a feature is at an evaluation time, since a nursery is a star-forming region while
//! embedded and then a bound cluster or an association (Design note 1).

use std::fmt;
use std::str::FromStr;

use crate::galaxy::ages::SubDisc;
use crate::id::{
    Designation, FeatureCell, FeatureMemberId, FeatureRef, MemberSlot, ParseDesignationError,
    SystemId, SystemIdKind,
};

/// A feature of the catalogue: a thin wrapper of plan 01's [`FeatureRef`] (P09.T1).
///
/// A well-formed ID does not always name a feature: the index is a candidate number, and resolving
/// it reruns the candidate ([`FeatureCatalogue::resolve`](super::catalogue::FeatureCatalogue::resolve)).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FeatureId(FeatureRef);

impl FeatureId {
    /// The feature with plan 01's reference `feature`.
    #[must_use]
    pub const fn new(feature: FeatureRef) -> Self {
        Self(feature)
    }

    /// Candidate `index` of `cell`, or `None` for an index of 16,384 or more.
    #[must_use]
    pub fn of(cell: FeatureCell, index: u16) -> Option<Self> {
        FeatureRef::new(cell, index).ok().map(Self)
    }

    /// Plan 01's reference.
    #[must_use]
    pub const fn feature_ref(self) -> FeatureRef {
        self.0
    }

    /// The feature cell.
    #[must_use]
    pub const fn cell(self) -> FeatureCell {
        self.0.cell()
    }

    /// The candidate index in the cell, 0–16,383.
    #[must_use]
    pub const fn index(self) -> u16 {
        self.0.index()
    }

    /// The word that keys the feature's streams, `ObjectKey::feature(word)` (Design note 23): the
    /// ID of the feature's member with the slot zeroed.
    #[must_use]
    pub fn object_word(self) -> u64 {
        self.0.object_word()
    }

    /// The system ID of the feature's member zero, the first entry of its feature-level list.
    ///
    /// # Panics
    ///
    /// Never: slot 0 of a feature-level list is always well-formed.
    #[must_use]
    pub fn member_zero(self) -> SystemId {
        let member = FeatureMemberId::new(self.0, MemberSlot::FeatureLevel { index: 0 })
            .expect("member zero of a feature-level list is always well-formed");
        SystemId::from(member)
    }

    /// The feature's designation: `<sector> F<index>`, as plan 01 names its members, such as
    /// `H7K F212`.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::galaxy::features::FeatureId;
    /// use hyperion_sim::id::FeatureCell;
    ///
    /// let id = FeatureId::of(FeatureCell::new([1, -9, 3])?, 212).unwrap();
    /// let name = id.designation().to_string();
    /// assert_eq!(name, "H7K F212");
    /// assert_eq!(name.parse::<hyperion_sim::galaxy::features::FeatureDesignation>()?.feature(), id);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub const fn designation(self) -> FeatureDesignation {
        FeatureDesignation(self)
    }
}

impl From<FeatureRef> for FeatureId {
    fn from(feature: FeatureRef) -> Self {
        Self(feature)
    }
}

/// A feature packed into 29 bits, as a record's [`SystemOrigin::FeatureMember`] carries it: the
/// cell's three stored 5-bit coordinates and the 14-bit index (ruling 20 of 2026-09-22: a member's
/// payload packs into a `u32`).
///
/// [`SystemOrigin::FeatureMember`]: crate::galaxy::placement::SystemOrigin::FeatureMember
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackedFeature(u32);

impl PackedFeature {
    /// The feature it packs.
    ///
    /// # Panics
    ///
    /// Never: it is only ever built from a feature.
    #[must_use]
    pub fn feature(self) -> FeatureId {
        let c = |shift: u32| i32::try_from((self.0 >> shift) & 31).expect("five bits") - 16;
        let cell = FeatureCell::new([c(24), c(19), c(14)])
            .expect("a packed cell's coordinates are a feature cell's");
        let index = u16::try_from(self.0 & 0x3FFF).expect("fourteen bits");
        FeatureId::of(cell, index).expect("a packed index is below 2^14")
    }

    /// The packed word.
    #[must_use]
    pub const fn word(self) -> u32 {
        self.0
    }
}

impl From<FeatureId> for PackedFeature {
    fn from(feature: FeatureId) -> Self {
        let stored = feature
            .cell()
            .to_array()
            .map(|c| u32::try_from(c + 16).expect("a feature cell coordinate is in -16..=15"));
        Self((stored[0] << 24) | (stored[1] << 19) | (stored[2] << 14) | u32::from(feature.index()))
    }
}

impl From<FeatureRef> for PackedFeature {
    fn from(feature: FeatureRef) -> Self {
        Self::from(FeatureId::new(feature))
    }
}

/// A feature's designation, `<sector> F<index>`: plan 01's feature-member designation without its
/// slot, so the two can never disagree. Like plan 01's, the format is not part of the generator
/// version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FeatureDesignation(FeatureId);

/// The slot plan 01's designation gives member zero, which [`FeatureDesignation`] leaves off.
const MEMBER_ZERO_SUFFIX: &str = " M0";

impl FeatureDesignation {
    /// The feature it names.
    #[must_use]
    pub const fn feature(self) -> FeatureId {
        self.0
    }
}

impl fmt::Display for FeatureDesignation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let member = self.0.member_zero().designation().to_string();
        let name = member
            .strip_suffix(MEMBER_ZERO_SUFFIX)
            .expect("plan 01 names member zero of a feature-level list `… M0`");
        f.write_str(name)
    }
}

/// A feature designation could not be parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseFeatureDesignationError {
    /// Plan 01's parser refused the text as a feature's.
    Designation(ParseDesignationError),
    /// The text names something other than a feature.
    NotAFeature,
}

impl fmt::Display for ParseFeatureDesignationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Designation(error) => write!(f, "not a feature designation: {error}"),
            Self::NotAFeature => f.write_str("the designation does not name a feature"),
        }
    }
}

impl std::error::Error for ParseFeatureDesignationError {}

impl FromStr for FeatureDesignation {
    type Err = ParseFeatureDesignationError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let member: Designation = format!("{text}{MEMBER_ZERO_SUFFIX}")
            .parse()
            .map_err(ParseFeatureDesignationError::Designation)?;
        match member.system().kind() {
            SystemIdKind::FeatureMember(member) => Ok(Self(FeatureId(member.feature()))),
            _ => Err(ParseFeatureDesignationError::NotAFeature),
        }
    }
}

/// One of the feature catalogue's Poisson processes (Design notes 1, 3 and 6).
///
/// Each process draws its own candidate count in a feature cell, and a candidate's index is its
/// number plus the counts of the processes before it, in the order of [`FeatureProcess::ALL`]:
/// globulars first, so that plan 10 can walk them alone, then the old open clusters, one process
/// per sub-disc of the old thin disc, youngest first, then the nurseries, then the clouds. The
/// order is part of the generator version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FeatureProcess {
    /// Globular clusters, drawn as they are today (phase 3 supplies their density).
    Globular,
    /// Bound clusters older than the young disc, following a sub-disc of the old thin disc.
    OldOpenCluster(SubDisc),
    /// The young disc's nurseries: star-forming regions, then young bound clusters or associations.
    Nursery,
    /// Molecular clouds and dark nebulae, following the smooth neutral and molecular gas.
    Cloud,
}

impl FeatureProcess {
    /// Every process in index order (Design note 6).
    pub const ALL: [Self; 8] = [
        Self::Globular,
        Self::OldOpenCluster(SubDisc::First),
        Self::OldOpenCluster(SubDisc::Second),
        Self::OldOpenCluster(SubDisc::Third),
        Self::OldOpenCluster(SubDisc::Fourth),
        Self::OldOpenCluster(SubDisc::Fifth),
        Self::Nursery,
        Self::Cloud,
    ];

    /// The process's place in [`ALL`](Self::ALL), 0–7.
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Globular => 0,
            Self::OldOpenCluster(sub_disc) => 1 + sub_disc.index(),
            Self::Nursery => 6,
            Self::Cloud => 7,
        }
    }

    /// Whether the process's candidates are proposed in height (Design note 21): every disc
    /// process, but not the globulars.
    #[must_use]
    pub const fn proposed_in_height(self) -> bool {
        !matches!(self, Self::Globular)
    }
}

impl fmt::Display for FeatureProcess {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Globular => f.write_str("globular"),
            Self::OldOpenCluster(sub_disc) => write!(f, "open cluster {}", sub_disc.index() + 1),
            Self::Nursery => f.write_str("nursery"),
            Self::Cloud => f.write_str("cloud"),
        }
    }
}

/// What a feature is (brainstorm, "Large features"): its kind at an evaluation time.
///
/// The catalogue's processes make the first five; the galactic centre, streams and dwarf cores are
/// on plan 10's global list, and the last two are reserved for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FeatureKind {
    /// A globular cluster.
    Globular,
    /// A bound open cluster, young or old.
    OpenCluster,
    /// An unbound OB association, expanding until it dissolves.
    Association,
    /// A star-forming region: an embedded cluster still forming, with its gas.
    StarFormingRegion,
    /// A molecular cloud or dark nebula: gas and dust, no members.
    MolecularCloud,
    /// The galactic centre, the first entry of plan 10's global list.
    GalacticCentre,
    /// A stellar stream, reserved for plan 10.
    Stream,
    /// The core of an accreted dwarf galaxy, reserved for plan 10.
    DwarfCore,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn process_indices_follow_the_fixed_order() {
        for (i, process) in FeatureProcess::ALL.iter().enumerate() {
            assert_eq!(process.index(), i, "{process}");
        }
        assert!(!FeatureProcess::Globular.proposed_in_height());
        assert!(FeatureProcess::Cloud.proposed_in_height());
    }

    #[test]
    fn designations_are_unique_over_a_sampled_feature_cell_and_parse_back() {
        let mut names = BTreeSet::new();
        for cell in [[-16, -16, -16], [0, 6, 0], [15, 15, 15], [-1, 0, 3]] {
            let cell = FeatureCell::new(cell).unwrap();
            for index in (0..FeatureRef::INDEX_LIMIT).step_by(37) {
                let id = FeatureId::of(cell, index).unwrap();
                let name = id.designation().to_string();
                let back: FeatureDesignation = name.parse().unwrap();
                assert_eq!(back.feature(), id, "{name}");
                assert!(names.insert(name.clone()), "{name} twice");
            }
            let last = FeatureId::of(cell, FeatureRef::INDEX_LIMIT - 1).unwrap();
            assert!(names.insert(last.designation().to_string()));
        }
    }

    #[test]
    fn a_designation_is_the_member_designation_without_its_slot() {
        let id = FeatureId::of(FeatureCell::new([1, -9, 3]).unwrap(), 212).unwrap();
        let member = id.member_zero().designation().to_string();
        assert_eq!(member, format!("{} M0", id.designation()));
    }

    #[test]
    fn other_designations_are_not_features() {
        assert!("H7K 4C0RFZ A-7".parse::<FeatureDesignation>().is_err());
        assert!("nonsense".parse::<FeatureDesignation>().is_err());
        assert_eq!(
            FeatureId::of(FeatureCell::new([0, 0, 0]).unwrap(), 16_384),
            None
        );
    }

    #[test]
    fn a_packed_feature_unpacks_to_itself() {
        for cell in [[-16, -16, -16], [15, 15, 15], [0, 6, -1]] {
            for index in [0, 1, 8_191, 16_383] {
                let id = FeatureId::of(FeatureCell::new(cell).unwrap(), index).unwrap();
                let packed = PackedFeature::from(id);
                assert_eq!(packed.feature(), id);
                assert!(packed.word() < 1 << 29);
            }
        }
    }

    #[test]
    fn object_words_are_distinct_per_feature() {
        let cell = FeatureCell::new([2, -3, 0]).unwrap();
        let a = FeatureId::of(cell, 0).unwrap();
        let b = FeatureId::of(cell, 1).unwrap();
        assert_ne!(a.object_word(), b.object_word());
        assert_eq!(a.object_word(), a.feature_ref().object_word());
    }
}
