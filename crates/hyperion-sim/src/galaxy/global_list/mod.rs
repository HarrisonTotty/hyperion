//! The global list: the few large features that no catalogue cell can hold (plan 10).
//!
//! The brainstorm's "Large features" keeps one list per galaxy for what is too big or too sparse
//! for the feature catalogue's cells: the galactic centre (plan 09's, entry 0), the stellar
//! streams of the halo and the cores of accreted dwarf galaxies ("Streams and accreted
//! structure"). Their members carry IDs under the `10` prefix, plan 01's [`StreamMemberId`] and
//! [`DwarfCoreMemberId`], whose number fields this module gives a meaning.
//!
//! # Numbering
//!
//! A stream's number is its place in a fixed order (plan 10, Design note 12): the living globular
//! clusters with a tube, in the order of the catalogue's walk over the globulars, then the orphan
//! streams, then the streams of the recent dwarf progenitors by accretion time. A dwarf core's
//! number is its progenitor's place among the progenitors that keep a core, in the same order.
//! Both are pure functions of the seed, so an ID read back from a save names the same stream or
//! core in every run. The list is built by P10.T4; this module so far holds the numbers, the
//! kinds, the cache trait, the orbit integrator ([`orbit`]) and the debris the list is built from
//! ([`Debris`]: which progenitors get a tube or a core, P10.T3, with [`spec`], [`orphans`] and
//! [`dwarfs`]).
//!
//! # Caches
//!
//! A stream's tube table costs about 0.2 s to build (brainstorm, "Runtime and code shape"), so
//! the caller keeps it: [`TubeLookup`] is what a caller's cache implements, and
//! [`resolve_with`](crate::galaxy::placement::resolve_with) reads a stream's or a core's member
//! through it. The sim keeps nothing.

mod debris;
pub mod dwarfs;
pub mod orbit;
pub mod orphans;
pub mod spec;
pub mod tube;

use std::error::Error;
use std::fmt;

pub use debris::Debris;
pub use spec::{ClusterStars, DwarfCoreSpec, MassLoss, SphericalOrbit, StreamSpec};
pub use tube::{TubeLookup, TubeTable};

use crate::galaxy::features::{FeatureId, FeatureKind};
use crate::id::{DwarfCoreMemberId, StreamMemberId};

/// A stream's number on the global list, below 2¹² = 4,096 (plan 01's 12-bit field of a
/// [`StreamMemberId`]).
///
/// The number is the stream's place in the order of the module's "Numbering". Numbers at or
/// above the list's stream count are reserved (plan 10, "Generator version"): they are
/// well-formed and name nothing.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::global_list::StreamNumber;
///
/// let gd1 = StreamNumber::new(17)?;
/// assert_eq!(gd1.get(), 17);
/// assert!(StreamNumber::new(4_096).is_err());
/// # Ok::<(), hyperion_sim::galaxy::global_list::BuildEntryNumberError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StreamNumber(u16);

impl StreamNumber {
    /// The exclusive bound of a stream's number: 4,096.
    pub const LIMIT: u16 = StreamMemberId::NUMBER_LIMIT;

    /// Stream `number`.
    ///
    /// # Errors
    ///
    /// [`BuildEntryNumberError::StreamNumberTooLarge`] for a number of 4,096 or more.
    pub const fn new(number: u16) -> Result<Self, BuildEntryNumberError> {
        if number < Self::LIMIT {
            Ok(Self(number))
        } else {
            Err(BuildEntryNumberError::StreamNumberTooLarge(number))
        }
    }

    /// The number, below [`LIMIT`](Self::LIMIT).
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for StreamNumber {
    type Error = BuildEntryNumberError;

    fn try_from(number: u16) -> Result<Self, Self::Error> {
        Self::new(number)
    }
}

impl From<StreamMemberId> for StreamNumber {
    /// The stream a member belongs to. A member ID's field holds 12 bits, so every one is a valid
    /// number.
    fn from(id: StreamMemberId) -> Self {
        Self(id.number())
    }
}

/// A dwarf core's number on the global list, below 4 (plan 01's 2-bit field of a
/// [`DwarfCoreMemberId`]).
///
/// A galaxy has 0–3 cores (brainstorm, "Streams and accreted structure"), numbered as the
/// module's "Numbering" says; the numbers above the list's core count are reserved.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::global_list::DwarfCoreNumber;
///
/// assert_eq!(DwarfCoreNumber::new(3)?.get(), 3);
/// assert!(DwarfCoreNumber::new(4).is_err());
/// # Ok::<(), hyperion_sim::galaxy::global_list::BuildEntryNumberError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DwarfCoreNumber(u8);

impl DwarfCoreNumber {
    /// The exclusive bound of a dwarf core's number: 4.
    pub const LIMIT: u8 = DwarfCoreMemberId::NUMBER_LIMIT;

    /// Dwarf core `number`.
    ///
    /// # Errors
    ///
    /// [`BuildEntryNumberError::DwarfCoreNumberTooLarge`] for a number of 4 or more.
    pub const fn new(number: u8) -> Result<Self, BuildEntryNumberError> {
        if number < Self::LIMIT {
            Ok(Self(number))
        } else {
            Err(BuildEntryNumberError::DwarfCoreNumberTooLarge(number))
        }
    }

    /// The number, below [`LIMIT`](Self::LIMIT).
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for DwarfCoreNumber {
    type Error = BuildEntryNumberError;

    fn try_from(number: u8) -> Result<Self, Self::Error> {
        Self::new(number)
    }
}

impl From<DwarfCoreMemberId> for DwarfCoreNumber {
    /// The core a member belongs to. A member ID's field holds 2 bits, so every one is a valid
    /// number.
    fn from(id: DwarfCoreMemberId) -> Self {
        Self(id.number())
    }
}

/// A number is beyond what its ID field can hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildEntryNumberError {
    /// A stream number of 4,096 or more.
    StreamNumberTooLarge(u16),
    /// A dwarf core number of 4 or more.
    DwarfCoreNumberTooLarge(u8),
}

impl fmt::Display for BuildEntryNumberError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StreamNumberTooLarge(n) => {
                write!(f, "stream number {n} is not below {}", StreamNumber::LIMIT)
            }
            Self::DwarfCoreNumberTooLarge(n) => write!(
                f,
                "dwarf core number {n} is not below {}",
                DwarfCoreNumber::LIMIT
            ),
        }
    }
}

impl Error for BuildEntryNumberError {}

/// What an entry of the global list is, with its number.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::features::FeatureKind;
/// use hyperion_sim::galaxy::global_list::{GlobalEntryKind, StreamNumber};
///
/// let kind = GlobalEntryKind::Stream(StreamNumber::new(0)?);
/// assert_eq!(kind.feature_kind(), FeatureKind::Stream);
/// # Ok::<(), hyperion_sim::galaxy::global_list::BuildEntryNumberError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GlobalEntryKind {
    /// The galactic centre, plan 09's model, always entry 0.
    Centre,
    /// A stellar stream: a tube of debris stripped from a globular cluster or a dwarf galaxy.
    Stream(StreamNumber),
    /// What is left, still bound, of a recently accreted dwarf galaxy.
    DwarfCore(DwarfCoreNumber),
}

impl GlobalEntryKind {
    /// The feature kind the entry is, as plan 09 names it.
    #[must_use]
    pub const fn feature_kind(self) -> FeatureKind {
        match self {
            Self::Centre => FeatureKind::GalacticCentre,
            Self::Stream(_) => FeatureKind::Stream,
            Self::DwarfCore(_) => FeatureKind::DwarfCore,
        }
    }
}

/// Where a stream's debris came from (plan 10, Design notes 10–12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StreamOrigin {
    /// A globular cluster of the catalogue that is still alive and losing stars.
    LivingGlobular(FeatureId),
    /// A globular cluster that has dissolved, leaving a stream with a gap where it was.
    Orphan,
    /// A recently accreted dwarf galaxy: recent progenitor `j` of plan 02's accretion history,
    /// [`ProgenitorKind::Recent(j)`](crate::galaxy::params::ProgenitorKind::Recent).
    Dwarf(u32),
}

/// One galaxy's global list (plan 10, P10.T4).
///
/// P10.T4 builds it with `GlobalEntry`s and bounding shells; until then a list holds only the
/// kinds of its entries, and [`GlobalList::default`] is the empty list, which is all a
/// [`TubeLookup`] can hand out before that task.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GlobalList {
    entries: Vec<GlobalEntryKind>,
}

impl GlobalList {
    /// The kinds of the entries, in list order.
    pub fn kinds(&self) -> impl Iterator<Item = GlobalEntryKind> + '_ {
        self.entries.iter().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::{Layer, MemberSlot, SystemId, SystemIdKind};

    #[test]
    fn numbers_out_of_range_are_rejected() {
        assert_eq!(StreamNumber::new(4_095).map(StreamNumber::get), Ok(4_095));
        assert_eq!(
            StreamNumber::new(4_096),
            Err(BuildEntryNumberError::StreamNumberTooLarge(4_096))
        );
        assert_eq!(
            StreamNumber::try_from(u16::MAX),
            Err(BuildEntryNumberError::StreamNumberTooLarge(u16::MAX))
        );
        assert_eq!(DwarfCoreNumber::new(3).map(DwarfCoreNumber::get), Ok(3));
        assert_eq!(
            DwarfCoreNumber::new(4),
            Err(BuildEntryNumberError::DwarfCoreNumberTooLarge(4))
        );
        assert_eq!(
            DwarfCoreNumber::try_from(u8::MAX),
            Err(BuildEntryNumberError::DwarfCoreNumberTooLarge(u8::MAX))
        );
    }

    #[test]
    fn every_member_id_names_a_valid_number() {
        for number in [0, 1, 2_047, 4_095] {
            let id = StreamMemberId::new(number, Layer::E, 16_383, [63, 0], 8_191).unwrap();
            assert_eq!(StreamNumber::from(id).get(), number);
            // Through the canonical decode, as an ID from a save arrives.
            let SystemIdKind::Stream(decoded) =
                SystemId::from_raw(SystemId::from(id).raw()).unwrap().kind()
            else {
                panic!("a stream member decodes as one");
            };
            assert_eq!(StreamNumber::from(decoded), StreamNumber::from(id));
        }
        for number in 0..DwarfCoreNumber::LIMIT {
            let id = DwarfCoreMemberId::new(number, MemberSlot::FeatureLevel { index: 5 }).unwrap();
            assert_eq!(DwarfCoreNumber::from(id).get(), number);
        }
    }

    #[test]
    fn entry_kinds_are_the_reserved_feature_kinds() {
        assert_eq!(
            GlobalEntryKind::Centre.feature_kind(),
            FeatureKind::GalacticCentre
        );
        let core = GlobalEntryKind::DwarfCore(DwarfCoreNumber::new(1).unwrap());
        assert_eq!(core.feature_kind(), FeatureKind::DwarfCore);
        assert_eq!(GlobalList::default().kinds().count(), 0);
    }

    #[test]
    fn error_messages_are_lower_case_without_trailing_punctuation() {
        for message in [
            BuildEntryNumberError::StreamNumberTooLarge(5_000).to_string(),
            BuildEntryNumberError::DwarfCoreNumberTooLarge(9).to_string(),
        ] {
            assert!(!message.starts_with(char::is_uppercase), "{message}");
            assert!(!message.ends_with(['.', '!', '?']), "{message}");
        }
        assert_eq!(
            BuildEntryNumberError::StreamNumberTooLarge(5_000).to_string(),
            "stream number 5000 is not below 4096"
        );
    }
}
