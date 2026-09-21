//! Identifiers: the 64-bit system ID, body IDs, event words and designations.
//!
//! Every generated object has a stable ID that also says where to regenerate it from. Generated
//! data is never stored, because it can be rebuilt from the ID; IDs are what go into saves, the
//! protocol and player logs. This module decodes and validates the *form* of an ID. Whether a
//! well-formed ID names a system (the cell's candidate count, the index check, the acceptance
//! test) is the galaxy's `resolve`.
//!
//! The encoding is canonical, so that one system has one ID: cell coordinates are stored with an
//! offset so that they are unsigned, every field not listed is zero, and anything else is rejected
//! by [`SystemId::from_raw`] with a specific [`DecodeSystemIdError`]. Bit 63 is the most
//! significant, and a field `[hi:lo]` holds bits `hi` down to `lo`.
//!
//! # Grid layouts, layers 0–6
//!
//! The layer is bits `[63:61]` in every layout. With `a = 14 − k` bits per axis and `n = 16 + 3k`
//! index bits at cell size `8 × 2ᵏ` ly, z starts at bit `n`, y at `n + a` and x at `n + 2a`. A
//! stored coordinate is the generation cell's coordinate plus `2^(a − 1)`, so every bit pattern
//! of the cell field is a cell inside the root cube. IDs sort by layer, x, y, z, index.
//!
//! | Layer        | Value | Cell (ly) | k   | Spare     | Cell x    | Cell y    | Cell z    | Index    | Capacity    |
//! | ------------ | ----- | --------- | --- | --------- | --------- | --------- | --------- | -------- | ----------- |
//! | A            | 0     | 8         | 0   | `[60:58]` | `[57:44]` | `[43:30]` | `[29:16]` | `[15:0]` | 65,536      |
//! | B            | 1     | 16        | 1   | `[60:58]` | `[57:45]` | `[44:32]` | `[31:19]` | `[18:0]` | 524,288     |
//! | C            | 2     | 32        | 2   | `[60:58]` | `[57:46]` | `[45:34]` | `[33:22]` | `[21:0]` | 4,194,304   |
//! | D            | 3     | 64        | 3   | `[60:58]` | `[57:47]` | `[46:36]` | `[35:25]` | `[24:0]` | 33,554,432  |
//! | E            | 4     | 128       | 4   | `[60:58]` | `[57:48]` | `[47:38]` | `[37:28]` | `[27:0]` | 268,435,456 |
//! | Brown dwarfs | 5     | 16        | 1   | `[60:58]` | `[57:45]` | `[44:32]` | `[31:19]` | `[18:0]` | 524,288     |
//! | Rogue planet | 6     | 4         | −1  | none      | `[60:46]` | `[45:31]` | `[30:16]` | `[15:0]` | 65,536      |
//!
//! The spare bits are zero. The layout follows the cell size, not the layer value: the brown
//! dwarfs reuse layer B's, and the rogue planets' 4 ly cells take the spare bits to keep a 16-bit
//! index, 1,024 per cubic light-year.
//!
//! # The reserved layer value, 7
//!
//! Objects not placed by the grid: members of large features, pinned content and catalogue
//! systems. Bit 60 starts a prefix.
//!
//! Prefix `0`, a member of a catalogue feature ([`FeatureMemberId`]):
//!
//! | Field           | Bits      | Notes                                                    |
//! | --------------- | --------- | -------------------------------------------------------- |
//! | Prefix          | `[60]`    | `0`                                                      |
//! | Feature cell    | `[59:45]` | x, y, z of 5 bits: 4,096 ly cells, coordinate + 16       |
//! | Feature index   | `[44:31]` | Candidate number in the feature cell                     |
//! | Mass band       | `[30:28]` | 0–6 as [`Layer`]; 7 marks a feature-level member         |
//! | Level           | `[27:25]` | 0–7                                                      |
//! | Cell in level   | `[24:13]` | x, y, z of 4 bits, 0–15; the centre is the corner of 7/8 |
//! | Index           | `[12:0]`  | Candidate number in the cell and band, or member number  |
//!
//! Band 7 requires level and cell to be zero, and its index counts feature-level members from 0
//! ("member zero"). At level 1 and above a cell whose three coordinates all lie in 4–11 is
//! rejected: the level below owns that volume.
//!
//! Prefix `10`, a member of a feature on the global list; the sub-kind is `[58:57]` and `11` is
//! rejected. Padding bits are zero.
//!
//! | Sub-kind              | Padding   | Fields, high to low                                                                                             |
//! | --------------------- | --------- | --------------------------------------------------------------------------------------------------------------- |
//! | `00` centre           | `[56:35]` | band `[34:32]`, level `[31:28]` (0–11), cell x `[27:23]`, y `[22:18]`, z `[17:13]` (0–31), index `[12:0]`       |
//! | `01` stream           | `[56:54]` | number `[53:42]`, band `[41:39]` (0–6), along `[38:25]`, across a `[24:19]`, across b `[18:13]`, index `[12:0]` |
//! | `10` dwarf core       | `[56:33]` | number `[32:31]`, then band, level, cell and index exactly as a catalogue feature's, `[30:0]`                   |
//!
//! The centre's band 7 is its feature-level list, whose member 0 is the central black hole,
//! `0xF000_0007_0000_0000` ([`CentreMemberId::CENTRAL_BLACK_HOLE`]); its inner-cell rule uses
//! 8–23. A stream has no band 7.
//!
//! Prefix `110`, pinned content ([`PinnedId`]): `[57:0]` is an opaque 58-bit number.
//!
//! Prefix `111`, a catalogue system ([`CatalogueSystemId`]):
//!
//! | Field  | Bits      | Notes                                                          |
//! | ------ | --------- | -------------------------------------------------------------- |
//! | Class  | `[57:52]` | 0–63; the registry belongs to the catalogue                    |
//! | Cell   | `[51:28]` | x, y, z of 8 bits: 512 ly cells, coordinate + 128              |
//! | Index  | `[27:4]`  | Candidate number                                               |
//! | Member | `[3:0]`   | 0 is the entry itself; others are further members of the entry |
//!
//! # Bodies and events
//!
//! A [`BodyId`] is a [`SystemId`] and a 16-bit body index; what the indices mean belongs to the
//! stellar and planetary stages. An [`EventId`] is a system or body plus an [`EventWord`]:
//!
//! | Field | Bits      | Width | Notes                                                                 |
//! | ----- | --------- | ----- | --------------------------------------------------------------------- |
//! | Tag   | `[63:48]` | 16    | An [`EventTag`] from the registry in [`event_tags`]; 0 is never valid |
//! | k     | `[47:8]`  | 40    | [`EventBin`]: bin or cycle number, two's complement, −2³⁹ to 2³⁹ − 1  |
//! | j     | `[7:0]`   | 8     | Number within the bin                                                 |
//!
//! # Text
//!
//! IDs travel as fixed-width lower-case hexadecimal, one string per ID, because a JSON number
//! loses integers above 2⁵³:
//!
//! | Type          | Form                  | Example                                  |
//! | ------------- | --------------------- | ---------------------------------------- |
//! | [`SystemId`]  | 16 digits             | `0200080020000000`                       |
//! | [`BodyId`]    | system, `.`, 4 digits | `0200080020000000.0003`                  |
//! | [`EventWord`] | 16 digits             | `0001ffffffffff00`                       |
//! | [`EventId`]   | subject, `:`, word    | `0200080020000000.0003:0001ffffffffff00` |
//!
//! People read [`Designation`]s, such as `H7K 4C0RFZ A-7`. Neither form is part of the generator
//! version.

mod bits;
mod body;
mod catalogue;
mod designation;
mod event;
pub mod event_tags;
mod global;
mod grid;
mod layer;
mod nested;
mod reserved;
mod system;
mod text;

pub use body::BodyId;
pub use catalogue::{CatalogueSystemId, PinnedId};
pub use designation::{ConvertDesignationError, Designation, ParseDesignationError};
pub use event::{
    BuildEventBinError, DecodeEventWordError, EventBin, EventId, EventSubject, EventTag, EventWord,
};
pub use global::{CentreMemberId, DwarfCoreMemberId, StreamMemberId};
pub use grid::GridId;
pub use layer::{Layer, RESERVED_LAYER_VALUE};
pub use nested::MemberSlot;
pub use reserved::{FeatureCell, FeatureMemberId, FeatureRef};
pub use system::{BuildSystemIdError, DecodeSystemIdError, SystemId, SystemIdKind};
pub(crate) use text::{HexFault, parse_lower_hex};
pub use text::{ParseBodyIdError, ParseEventIdError, ParseEventWordError, ParseSystemIdError};

use crate::rng::{ObjectKey, TagScope};

impl From<SystemId> for ObjectKey {
    /// A system's key: its raw ID as the counter word, `sub` 0, scope [`TagScope::System`].
    fn from(id: SystemId) -> Self {
        Self::new(id.raw(), 0, TagScope::System)
    }
}

impl From<BodyId> for ObjectKey {
    /// A body's key: its system's raw ID as the counter word and its body index as `sub`, which
    /// shares counter word 1 with the draw number; scope [`TagScope::Body`].
    fn from(id: BodyId) -> Self {
        Self::new(id.system().raw(), id.body_index(), TagScope::Body)
    }
}

/// Test helpers shared by the unit tests of this module.
#[cfg(test)]
pub(crate) mod testing {
    use hyperion_testkit::lcg::Lcg;

    use super::SystemId;

    /// The top seven bits: layer, prefix and sub-kind.
    const HEAD_7: u64 = 0b111_1111 << 57;

    /// Recipes for random IDs of each kind: `(fixed bits, mask of the fixed bits)`. The rest of
    /// the word is random, and a draw that still breaks a rule is redrawn.
    const KINDS: [(u64, u64); 15] = [
        // Grid layers 0–5: layer and spare bits fixed.
        (0, 0b11_1111 << 58),
        (1 << 61, 0b11_1111 << 58),
        (2 << 61, 0b11_1111 << 58),
        (3 << 61, 0b11_1111 << 58),
        (4 << 61, 0b11_1111 << 58),
        (5 << 61, 0b11_1111 << 58),
        // Rogue planets: every pattern.
        (6 << 61, 0b111 << 61),
        // Prefix 0: a feature member in a cell, and a feature-level member (band 7, level and
        // cell zero).
        (0b1110 << 60, 0b1111 << 60),
        (
            (0b1110 << 60) | (7 << 28),
            (0b1111 << 60) | (((1 << 18) - 1) << 13),
        ),
        // Prefix 10: the centre, in a cell and at feature level; a stream; a dwarf core, in a
        // cell and at feature level. Padding fixed at zero.
        (0b111_1000 << 57, HEAD_7 | (((1 << 22) - 1) << 35)),
        (
            (0b111_1000 << 57) | (7 << 32),
            HEAD_7 | (((1 << 44) - 1) << 13),
        ),
        (0b111_1001 << 57, HEAD_7 | (0b111 << 54)),
        (0b111_1010 << 57, HEAD_7 | (((1 << 24) - 1) << 33)),
        (
            (0b111_1010 << 57) | (7 << 28),
            HEAD_7 | (((1 << 24) - 1) << 33) | (((1 << 18) - 1) << 13),
        ),
        // Prefixes 110 and 111, which alternate on the random bit 58.
        (0b1_1111 << 59, 0b1_1111 << 59),
    ];

    /// A well-formed ID of a kind chosen by `lcg`, each recipe about equally often.
    pub(crate) fn random_valid_id(lcg: &mut Lcg) -> SystemId {
        let n = u64::try_from(KINDS.len()).expect("fifteen recipes");
        let (fixed, mask) = KINDS[usize::try_from(lcg.next_below(n)).expect("below fifteen")];
        loop {
            if let Ok(id) = SystemId::from_raw(fixed | (lcg.next_u64() & !mask)) {
                return id;
            }
        }
    }
}
