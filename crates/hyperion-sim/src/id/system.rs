//! The 64-bit system ID, its kinds, and the errors of decoding and building one.

use std::error::Error;
use std::fmt;

use super::catalogue::{CatalogueSystemId, PinnedId};
use super::designation::Designation;
use super::global::{CentreMemberId, DwarfCoreMemberId, StreamMemberId};
use super::grid::{self, GridId};
use super::layer::{LAYER, Layer};
use super::nested::MEMBER_INDEX;
use super::reserved::{self, FeatureMemberId};
use crate::coords::GenCell;

/// A system ID's bits break a rule of its layout.
///
/// Every variant is a well-defined rejection: a bit pattern that no system has, so that one
/// system has one ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DecodeSystemIdError {
    /// A spare bit of a stellar or brown-dwarf layout, `[60:58]`, is set.
    SpareBitsSet,
    /// A padding bit of a global-list layout (prefix `10`) is set.
    PaddingBitsSet,
    /// The global-list sub-kind is `11`, which names nothing.
    UnknownSubKind,
    /// A feature-level member (mass band 7) has a non-zero level or cell.
    FeatureLevelFieldsSet,
    /// A nested cell of level 1 or more lies wholly in the inner half of its level, whose volume
    /// the level below owns.
    InnerCellOwnedByLowerLevel,
    /// A galactic-centre level is 12 or more; the centre has twelve levels.
    LevelOutOfRange,
    /// A stream member's mass band is 7: streams have no feature-level members.
    BandOutOfRange,
}

impl fmt::Display for DecodeSystemIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::SpareBitsSet => "spare bits 60 to 58 of a grid system id are set",
            Self::PaddingBitsSet => "padding bits of a global-list system id are set",
            Self::UnknownSubKind => "global-list sub-kind 11 names nothing",
            Self::FeatureLevelFieldsSet => "a feature-level member has a non-zero level or cell",
            Self::InnerCellOwnedByLowerLevel => {
                "an inner cell of a nested level belongs to the level below"
            }
            Self::LevelOutOfRange => "the galactic centre has levels 0 to 11 only",
            Self::BandOutOfRange => "mass band 7 names no stream member",
        })
    }
}

impl Error for DecodeSystemIdError {}

/// A system ID could not be built from its parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildSystemIdError {
    /// The cell lies outside the root cube, or its coordinates do not fit the field.
    CellOutsideRootCube,
    /// The generation cell's size is not the layer's.
    CellSizeMismatch,
    /// The index does not fit the layout's index field.
    IndexTooLarge,
    /// A field other than a cell or an index does not fit its range.
    FieldOutOfRange {
        /// The field's name.
        field: &'static str,
    },
    /// The parts fit their fields but break a rule of the layout.
    NotCanonical(DecodeSystemIdError),
}

impl fmt::Display for BuildSystemIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CellOutsideRootCube => f.write_str("the cell lies outside the root cube"),
            Self::CellSizeMismatch => f.write_str("the cell's size is not the layer's"),
            Self::IndexTooLarge => f.write_str("the index does not fit the layout"),
            Self::FieldOutOfRange { field } => write!(f, "the {field} is out of range"),
            Self::NotCanonical(e) => write!(f, "the parts are not canonical: {e}"),
        }
    }
}

impl Error for BuildSystemIdError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::NotCanonical(e) => Some(e),
            Self::CellOutsideRootCube
            | Self::CellSizeMismatch
            | Self::IndexTooLarge
            | Self::FieldOutOfRange { .. } => None,
        }
    }
}

/// A validated 64-bit system ID.
///
/// The encoding is canonical: [`from_raw`](Self::from_raw) rejects every bit pattern that breaks a
/// rule of its layout, so one system has one ID, and a `SystemId` always holds a well-formed one.
/// Whether a well-formed ID names a system (its index below the cell's candidate count, the
/// candidate accepted) is for the galaxy's `resolve`, not this type. The layouts are tabulated in
/// the [module documentation](crate::id).
///
/// IDs order by their raw value, which for grid IDs is layer, then x, y, z, then index.
///
/// # Examples
///
/// ```
/// use hyperion_sim::coords::{CellSize, GenCell};
/// use hyperion_sim::id::{Layer, SystemId, SystemIdKind};
///
/// let cell = GenCell::new(CellSize::Ly8, [0, 0, 0])?;
/// let id = SystemId::from_parts(Layer::A, cell, 0)?;
/// assert_eq!(id.raw(), 0x0200_0800_2000_0000);
/// assert_eq!(SystemId::from_raw(id.raw()), Ok(id));
/// let SystemIdKind::Grid(grid) = id.kind() else { unreachable!() };
/// assert_eq!((grid.layer(), grid.cell(), grid.index()), (Layer::A, cell, 0));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SystemId(u64);

impl SystemId {
    /// Decodes and validates a raw ID.
    ///
    /// # Errors
    ///
    /// The [`DecodeSystemIdError`] of the first rule the bits break.
    pub fn from_raw(raw: u64) -> Result<Self, DecodeSystemIdError> {
        match Layer::from_value(LAYER.get_u8(raw)) {
            Some(layer) => grid::validate(raw, layer)?,
            None => reserved::validate(raw)?,
        }
        Ok(Self(raw))
    }

    /// The ID of candidate `index` in a generation cell of a layer.
    ///
    /// # Errors
    ///
    /// - [`BuildSystemIdError::CellSizeMismatch`] if the cell's size is not the layer's.
    /// - [`BuildSystemIdError::CellOutsideRootCube`] if the cell is not inside the root cube.
    /// - [`BuildSystemIdError::IndexTooLarge`] if `index` does not fit in
    ///   [`Layer::index_bits`].
    pub fn from_parts(layer: Layer, cell: GenCell, index: u32) -> Result<Self, BuildSystemIdError> {
        GridId::new(layer, cell, index).map(Self::from)
    }

    /// Wraps a raw value that the caller has already validated.
    #[must_use]
    pub(super) const fn from_valid_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// The raw 64-bit value, as stored in saves and sent (as text) over the protocol.
    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }

    /// What the ID names and where its fields are.
    #[must_use]
    pub fn kind(self) -> SystemIdKind {
        match Layer::from_value(LAYER.get_u8(self.0)) {
            Some(_) => SystemIdKind::Grid(GridId::from_valid_raw(self.0)),
            None => reserved::kind(self.0),
        }
    }

    /// The grid layer, or `None` under the reserved layer value.
    #[must_use]
    pub fn layer(self) -> Option<Layer> {
        Layer::from_value(LAYER.get_u8(self.0))
    }

    /// The ID with its index zeroed: the `ObjectKey` word of the cell the system was drawn in.
    ///
    /// For a grid ID it is the ID of the generation cell's candidate 0. For a member of a feature,
    /// the centre, a stream or a dwarf core the 13-bit index is zeroed, which leaves the nested cell
    /// and band (or, in band 7, the feature-level list). For a catalogue system the index and
    /// member are zeroed, leaving class and catalogue cell. A pinned ID has no cell and no index,
    /// and its word is the ID itself.
    #[must_use]
    pub fn cell_word(self) -> u64 {
        let index = match self.kind() {
            SystemIdKind::Grid(id) => id.index_field(),
            SystemIdKind::FeatureMember(_)
            | SystemIdKind::Centre(_)
            | SystemIdKind::Stream(_)
            | SystemIdKind::DwarfCore(_) => MEMBER_INDEX,
            SystemIdKind::Pinned(_) => return self.0,
            SystemIdKind::Catalogue(_) => CatalogueSystemId::INDEX_AND_MEMBER,
        };
        index.with(self.0, 0)
    }

    /// The ID's human-readable designation.
    #[must_use]
    pub const fn designation(self) -> Designation {
        Designation::of_system(self)
    }
}

impl fmt::Debug for SystemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SystemId({:#018x})", self.0)
    }
}

/// What a [`SystemId`] names, with typed access to the fields of its layout.
///
/// Each variant's value converts infallibly into a [`SystemId`] with `From`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SystemIdKind {
    /// A system placed by the grid, in layers 0–6.
    Grid(GridId),
    /// A member of a catalogue feature: reserved layer, prefix `0`.
    FeatureMember(FeatureMemberId),
    /// A member of the galactic centre: prefix `10`, sub-kind `00`.
    Centre(CentreMemberId),
    /// A member of a stream: prefix `10`, sub-kind `01`.
    Stream(StreamMemberId),
    /// A member of a dwarf-galaxy core: prefix `10`, sub-kind `10`.
    DwarfCore(DwarfCoreMemberId),
    /// Pinned content: prefix `110`.
    Pinned(PinnedId),
    /// A catalogue system: prefix `111`.
    Catalogue(CatalogueSystemId),
}

impl From<SystemIdKind> for SystemId {
    fn from(kind: SystemIdKind) -> Self {
        match kind {
            SystemIdKind::Grid(id) => id.into(),
            SystemIdKind::FeatureMember(id) => id.into(),
            SystemIdKind::Centre(id) => id.into(),
            SystemIdKind::Stream(id) => id.into(),
            SystemIdKind::DwarfCore(id) => id.into(),
            SystemIdKind::Pinned(id) => id.into(),
            SystemIdKind::Catalogue(id) => id.into(),
        }
    }
}

/// Implements `From<$kind> for SystemId` for a kind that holds a validated raw ID.
macro_rules! into_system_id {
    ($($kind:ty),* $(,)?) => {
        $(
            impl From<$kind> for SystemId {
                fn from(id: $kind) -> Self {
                    Self::from_valid_raw(id.raw())
                }
            }
        )*
    };
}

into_system_id!(
    GridId,
    FeatureMemberId,
    CentreMemberId,
    StreamMemberId,
    DwarfCoreMemberId,
    PinnedId,
    CatalogueSystemId,
);

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use hyperion_testkit::lcg::Lcg;

    use super::*;
    use crate::coords::CellSize;
    use crate::id::testing::random_valid_id;
    use crate::id::{FeatureCell, FeatureRef};

    fn cell(size: CellSize, c: [i32; 3]) -> GenCell {
        GenCell::new(size, c).unwrap()
    }

    #[test]
    fn hand_computed_grid_ids() {
        // Layer A, cell (0, 0, 0): each stored coordinate is 2¹³, at bits 57, 43 and 29.
        let a = SystemId::from_parts(Layer::A, cell(CellSize::Ly8, [0, 0, 0]), 0).unwrap();
        assert_eq!(a.raw(), (1 << 57) | (1 << 43) | (1 << 29));
        assert_eq!(a.raw(), 0x0200_0800_2000_0000);
        // Layer E, the low corner: every stored coordinate 0, layer 4 in [63:61].
        let e_low =
            SystemId::from_parts(Layer::E, cell(CellSize::Ly128, [-512, -512, -512]), 0).unwrap();
        assert_eq!(e_low.raw(), 4 << 61);
        assert_eq!(e_low.raw(), 0x8000_0000_0000_0000);
        // Layer E, the high corner and the last index: bits 57..0 all ones.
        let e_high = SystemId::from_parts(
            Layer::E,
            cell(CellSize::Ly128, [511, 511, 511]),
            (1 << 28) - 1,
        )
        .unwrap();
        assert_eq!(e_high.raw(), (4 << 61) | ((1 << 58) - 1));
        assert_eq!(e_high.raw(), 0x83FF_FFFF_FFFF_FFFF);
        // Rogue planets, the high corner and the last index: bits 60..0 all ones.
        let r = SystemId::from_parts(
            Layer::RoguePlanet,
            cell(CellSize::Ly4, [16_383, 16_383, 16_383]),
            u32::from(u16::MAX),
        )
        .unwrap();
        assert_eq!(r.raw(), 0xDFFF_FFFF_FFFF_FFFF);
    }

    #[test]
    fn every_layer_round_trips_over_corner_cells_and_extreme_indices() {
        for layer in Layer::ALL {
            let size = layer.cell_size();
            let half = 65_536 / i32::try_from(size.ly()).unwrap();
            let max_index = (1_u32 << layer.index_bits()) - 1;
            for x in [-half, 0, half - 1] {
                for y in [-half, -1, half - 1] {
                    for z in [-half, 1, half - 1] {
                        for index in [0, 1, max_index] {
                            let c = cell(size, [x, y, z]);
                            let id = SystemId::from_parts(layer, c, index).unwrap();
                            assert_eq!(SystemId::from_raw(id.raw()), Ok(id));
                            assert_eq!(id.layer(), Some(layer));
                            let SystemIdKind::Grid(grid) = id.kind() else {
                                panic!("{id:?} is a grid ID");
                            };
                            assert_eq!(
                                (grid.layer(), grid.cell(), grid.index()),
                                (layer, c, index)
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn any_spare_bit_is_rejected_below_the_rogue_planets() {
        for layer in &Layer::ALL[..6] {
            let base = SystemId::from_parts(*layer, cell(layer.cell_size(), [3, -7, 11]), 5)
                .unwrap()
                .raw();
            for bit in 58..=60 {
                assert_eq!(
                    SystemId::from_raw(base | (1 << bit)),
                    Err(DecodeSystemIdError::SpareBitsSet),
                    "{layer:?} bit {bit}"
                );
            }
        }
    }

    #[test]
    fn build_errors_are_specific() {
        let c8 = cell(CellSize::Ly8, [0, 0, 0]);
        for layer in Layer::ALL {
            let c = cell(layer.cell_size(), [0; 3]);
            let n = layer.index_bits();
            assert_eq!(
                SystemId::from_parts(layer, c, 1 << n),
                Err(BuildSystemIdError::IndexTooLarge),
                "{layer:?}"
            );
            assert!(SystemId::from_parts(layer, c, (1 << n) - 1).is_ok());
        }
        assert_eq!(
            SystemId::from_parts(Layer::A, c8, 1 << 16),
            Err(BuildSystemIdError::IndexTooLarge)
        );
        assert_eq!(
            SystemId::from_parts(Layer::B, c8, 0),
            Err(BuildSystemIdError::CellSizeMismatch)
        );
        assert_eq!(
            SystemId::from_parts(Layer::A, cell(CellSize::Ly8, [8_192, 0, 0]), 0),
            Err(BuildSystemIdError::CellOutsideRootCube)
        );
        assert_eq!(
            SystemId::from_parts(Layer::A, cell(CellSize::Ly8, [0, -8_193, 0]), 0),
            Err(BuildSystemIdError::CellOutsideRootCube)
        );
    }

    #[test]
    fn ids_sort_by_layer_then_x_y_z_then_index() {
        let mut lcg = Lcg::new(0x5eed);
        let mut parts = Vec::new();
        for _ in 0..2_000 {
            let layer = Layer::ALL[usize::try_from(lcg.next_below(7)).unwrap()];
            let half = 65_536 / u64::from(layer.cell_size_ly());
            let mut coordinate =
                || i32::try_from(lcg.next_below(2 * half)).unwrap() - i32::try_from(half).unwrap();
            let c = [coordinate(), coordinate(), coordinate()];
            let index = u32::try_from(lcg.next_below(1 << layer.index_bits())).unwrap();
            parts.push((layer, c, index));
        }
        let mut ids: Vec<SystemId> = parts
            .iter()
            .map(|&(layer, c, index)| {
                SystemId::from_parts(layer, cell(layer.cell_size(), c), index).unwrap()
            })
            .collect();
        parts.sort_unstable();
        ids.sort_unstable();
        let decoded: Vec<_> = ids
            .iter()
            .map(|id| {
                let SystemIdKind::Grid(g) = id.kind() else {
                    unreachable!()
                };
                (g.layer(), g.cell().to_array(), g.index())
            })
            .collect();
        assert_eq!(decoded, parts);
    }

    #[test]
    fn rogue_planet_ids_accept_every_bit_pattern() {
        let mut lcg = Lcg::new(6);
        for _ in 0..100_000 {
            let raw = (lcg.next_u64() & !(0b111 << 61)) | (6 << 61);
            let id = SystemId::from_raw(raw).expect("every rogue-planet pattern is well-formed");
            let SystemIdKind::Grid(g) = id.kind() else {
                unreachable!()
            };
            assert_eq!(g.layer(), Layer::RoguePlanet);
            assert_eq!(
                SystemId::from_parts(g.layer(), g.cell(), g.index())
                    .unwrap()
                    .raw(),
                raw
            );
        }
    }

    #[test]
    fn brown_dwarf_layout_is_layer_b_with_another_layer_value() {
        let mut lcg = Lcg::new(5);
        for _ in 0..10_000 {
            let mut coordinate = || i32::try_from(lcg.next_below(8_192)).unwrap() - 4_096;
            let c = cell(CellSize::Ly16, [coordinate(), coordinate(), coordinate()]);
            let index = u32::try_from(lcg.next_below(1 << 19)).unwrap();
            let b = SystemId::from_parts(Layer::B, c, index).unwrap().raw();
            let bd = SystemId::from_parts(Layer::BrownDwarf, c, index)
                .unwrap()
                .raw();
            assert_eq!(LAYER.with(b, 5), bd);
        }
    }

    #[test]
    fn cell_word_zeroes_the_index_and_equals_candidate_zero() {
        for layer in Layer::ALL {
            let c = cell(layer.cell_size(), [-3, 2, 9]);
            let max = (1_u32 << layer.index_bits()) - 1;
            let id = SystemId::from_parts(layer, c, max).unwrap();
            let zero = SystemId::from_parts(layer, c, 0).unwrap();
            assert_eq!(id.cell_word(), zero.raw());
            assert_eq!(zero.cell_word(), zero.raw());
        }
        let mut lcg = Lcg::new(77);
        for _ in 0..10_000 {
            let id = random_valid_id(&mut lcg);
            let word = id.cell_word();
            assert_eq!(SystemId::from_raw(word).map(SystemId::cell_word), Ok(word));
        }
    }

    /// What the head bits alone say a raw ID is, read with plain shifts and independently of the
    /// decoder: the layer `[63:61]`, then under layer 7 the prefix and sub-kind bits `[60:57]`.
    fn kind_from_head(raw: u64) -> &'static str {
        if raw >> 61 != 7 {
            return "grid";
        }
        match (raw >> 57) & 0b1111 {
            0b0000..=0b0111 => "feature member",
            0b1000 => "centre",
            0b1001 => "stream",
            0b1010 => "dwarf core",
            0b1011 => "unknown sub-kind",
            0b1100 | 0b1101 => "pinned",
            _ => "catalogue",
        }
    }

    fn kind_name(kind: SystemIdKind) -> &'static str {
        match kind {
            SystemIdKind::Grid(_) => "grid",
            SystemIdKind::FeatureMember(_) => "feature member",
            SystemIdKind::Centre(_) => "centre",
            SystemIdKind::Stream(_) => "stream",
            SystemIdKind::DwarfCore(_) => "dwarf core",
            SystemIdKind::Pinned(_) => "pinned",
            SystemIdKind::Catalogue(_) => "catalogue",
        }
    }

    /// The ID rebuilt from its decoded fields through the public constructors.
    fn rebuilt(id: SystemId) -> SystemId {
        match id.kind() {
            SystemIdKind::Grid(g) => SystemId::from_parts(g.layer(), g.cell(), g.index()).unwrap(),
            SystemIdKind::FeatureMember(m) => {
                let cell = FeatureCell::new(m.feature().cell().to_array()).unwrap();
                let feature = FeatureRef::new(cell, m.feature().index()).unwrap();
                FeatureMemberId::new(feature, m.slot()).unwrap().into()
            }
            SystemIdKind::Centre(m) => CentreMemberId::new(m.slot()).unwrap().into(),
            SystemIdKind::Stream(s) => {
                StreamMemberId::new(s.number(), s.band(), s.along(), s.across(), s.index())
                    .unwrap()
                    .into()
            }
            SystemIdKind::DwarfCore(d) => {
                DwarfCoreMemberId::new(d.number(), d.slot()).unwrap().into()
            }
            SystemIdKind::Pinned(p) => PinnedId::new(p.number()).unwrap().into(),
            SystemIdKind::Catalogue(c) => {
                CatalogueSystemId::new(c.class(), c.cell(), c.index(), c.member())
                    .unwrap()
                    .into()
            }
        }
    }

    #[test]
    fn every_raw_is_rejected_or_decodes_to_the_one_kind_that_rebuilds_it() {
        use DecodeSystemIdError as E;
        // Zero regions of the layouts: spare bits, the three paddings, and the level and cell of
        // the feature and centre slots. Clearing a random choice of them, and sometimes writing
        // band 7, makes every kind well-formed often and every rule fail often.
        const ZERO_REGIONS: [u64; 6] = [
            0b111 << 58,
            ((1 << 22) - 1) << 35,
            0b111 << 54,
            ((1 << 24) - 1) << 33,
            ((1 << 15) - 1) << 13,
            ((1 << 19) - 1) << 13,
        ];
        let mut lcg = Lcg::new(0xb175);
        let mut accepted: BTreeMap<&str, u32> = BTreeMap::new();
        let mut rejected: BTreeMap<String, u32> = BTreeMap::new();
        for i in 0..200_000 {
            let mut raw = lcg.next_u64();
            if i % 2 == 1 {
                for region in ZERO_REGIONS {
                    if lcg.next_below(2) == 0 {
                        raw &= !region;
                    }
                }
                if lcg.next_below(4) == 0 {
                    raw |= 7 << [28, 32, 39][usize::try_from(lcg.next_below(3)).unwrap()];
                }
                // A grid layer, or layer 7 with one of the sixteen patterns of [60:57].
                let head = lcg.next_below(23);
                raw = if head < 7 {
                    (raw & !(0b111 << 61)) | (head << 61)
                } else {
                    (raw & !(0b111_1111 << 57)) | (0b111 << 61) | ((head - 7) << 57)
                };
            }
            let head = kind_from_head(raw);
            match SystemId::from_raw(raw) {
                Ok(id) => {
                    assert_eq!(kind_name(id.kind()), head, "{raw:#018x}");
                    assert_eq!(rebuilt(id), id);
                    let word = id.cell_word();
                    let cell = SystemId::from_raw(word).expect("a cell word is well-formed");
                    assert_eq!(kind_name(cell.kind()), head, "cell word of {raw:#018x}");
                    assert_eq!(cell.cell_word(), word);
                    *accepted.entry(head).or_default() += 1;
                }
                Err(e) => {
                    let possible: &[E] = match head {
                        "grid" if raw >> 61 == 6 => &[],
                        "grid" => &[E::SpareBitsSet],
                        "feature member" => {
                            &[E::FeatureLevelFieldsSet, E::InnerCellOwnedByLowerLevel]
                        }
                        "centre" => &[
                            E::PaddingBitsSet,
                            E::FeatureLevelFieldsSet,
                            E::LevelOutOfRange,
                            E::InnerCellOwnedByLowerLevel,
                        ],
                        "stream" => &[E::PaddingBitsSet, E::BandOutOfRange],
                        "dwarf core" => &[
                            E::PaddingBitsSet,
                            E::FeatureLevelFieldsSet,
                            E::InnerCellOwnedByLowerLevel,
                        ],
                        "unknown sub-kind" => &[E::UnknownSubKind],
                        _ => &[],
                    };
                    assert!(possible.contains(&e), "{raw:#018x} ({head}) gave {e:?}");
                    *rejected.entry(format!("{e:?}")).or_default() += 1;
                }
            }
        }
        // Not vacuous: every kind was accepted and every rule fired, many times over.
        assert_eq!(accepted.len(), 7, "{accepted:?}");
        assert!(accepted.values().all(|&n| n >= 1_000), "{accepted:?}");
        assert_eq!(rejected.len(), 7, "{rejected:?}");
        assert!(rejected.values().all(|&n| n >= 100), "{rejected:?}");
    }

    #[test]
    fn debug_prints_hex() {
        assert_eq!(
            format!("{:?}", SystemId::from_raw(0x8000_0000_0000_0000).unwrap()),
            "SystemId(0x8000000000000000)"
        );
    }
}
