//! The layers of the placement grid and the grid layout, which follows a layer's cell size.

use super::bits::{Field, assert_tiles_64};
use crate::coords::CellSize;

/// The layer field, bits `[63:61]` of every system ID.
pub(super) const LAYER: Field = Field::new(63, 61);

/// The spare bits of the grid layouts at 8 ly and coarser, `[60:58]`: always zero.
pub(super) const SPARE: Field = Field::new(60, 58);

/// The layer value under which IDs are not placed by the grid: members of large features, pinned
/// content and catalogue systems.
pub const RESERVED_LAYER_VALUE: u8 = 7;

/// A layer of the placement grid: a stack of independent grids, one per band of primary mass.
///
/// The five stellar layers own bands of primary *initial* mass (brainstorm, "Sizing the layers"):
///
/// | Layer | Value | Cell   | Primary initial mass |
/// | ----- | ----- | ------ | -------------------- |
/// | A     | 0     | 8 ly   | 0.08–0.5 M☉          |
/// | B     | 1     | 16 ly  | 0.5–0.75 M☉          |
/// | C     | 2     | 32 ly  | 0.75–2.5 M☉          |
/// | D     | 3     | 64 ly  | 2.5–8 M☉             |
/// | E     | 4     | 128 ly | 8–150 M☉             |
///
/// The two substellar layers of "Between the stars" follow: free-floating brown dwarfs (value 5,
/// 16 ly cells) and rogue planets (value 6, 4 ly cells). Value 7 is
/// [`RESERVED_LAYER_VALUE`], which is not a layer. The same numbers, 0–6, are the mass bands of
/// feature members, where 7 marks a feature-level member.
///
/// # Examples
///
/// ```
/// use hyperion_sim::id::Layer;
///
/// // Capacity per cubic light-year: the finest stellar layer is the tightest.
/// let per_ly3 = |layer: Layer| (1_u64 << layer.index_bits()) / u64::from(layer.cell_size_ly()).pow(3);
/// assert_eq!(per_ly3(Layer::A), 128);
/// assert_eq!(per_ly3(Layer::RoguePlanet), 1_024);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Layer {
    /// Layer A: 8 ly cells, primaries of 0.08–0.5 M☉.
    A,
    /// Layer B: 16 ly cells, primaries of 0.5–0.75 M☉.
    B,
    /// Layer C: 32 ly cells, primaries of 0.75–2.5 M☉.
    C,
    /// Layer D: 64 ly cells, primaries of 2.5–8 M☉.
    D,
    /// Layer E: 128 ly cells, primaries of 8–150 M☉.
    E,
    /// Free-floating brown dwarfs: 16 ly cells.
    BrownDwarf,
    /// Rogue planets: 4 ly cells.
    RoguePlanet,
}

impl Layer {
    /// Every layer, in value order.
    pub const ALL: [Self; 7] = [
        Self::A,
        Self::B,
        Self::C,
        Self::D,
        Self::E,
        Self::BrownDwarf,
        Self::RoguePlanet,
    ];

    /// The layer's value in the ID's layer field, 0–6.
    #[must_use]
    pub const fn value(self) -> u8 {
        match self {
            Self::A => 0,
            Self::B => 1,
            Self::C => 2,
            Self::D => 3,
            Self::E => 4,
            Self::BrownDwarf => 5,
            Self::RoguePlanet => 6,
        }
    }

    /// The layer with this value, or `None` for 7 ([`RESERVED_LAYER_VALUE`]) and above.
    #[must_use]
    pub const fn from_value(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::A),
            1 => Some(Self::B),
            2 => Some(Self::C),
            3 => Some(Self::D),
            4 => Some(Self::E),
            5 => Some(Self::BrownDwarf),
            6 => Some(Self::RoguePlanet),
            _ => None,
        }
    }

    /// The layer's generation cell size.
    #[must_use]
    pub const fn cell_size(self) -> CellSize {
        match self {
            Self::A => CellSize::Ly8,
            Self::B | Self::BrownDwarf => CellSize::Ly16,
            Self::C => CellSize::Ly32,
            Self::D => CellSize::Ly64,
            Self::E => CellSize::Ly128,
            Self::RoguePlanet => CellSize::Ly4,
        }
    }

    /// The layer's cell edge in light-years.
    #[must_use]
    pub const fn cell_size_ly(self) -> u32 {
        self.cell_size().ly()
    }

    /// Bits per axis of the cell field: `14 − k` at cell size `8 × 2ᵏ` ly.
    #[must_use]
    pub const fn cell_bits_per_axis(self) -> u32 {
        GridLayout::of(self.cell_size()).axis_bits()
    }

    /// Bits of the index field: `16 + 3k` for the stellar sizes, and 16 for the rogue planets,
    /// which take the spare bits.
    #[must_use]
    pub const fn index_bits(self) -> u32 {
        GridLayout::of(self.cell_size()).index().width()
    }

    /// The layer's letter in designations: `A`–`E`, `F` for brown dwarfs, `G` for rogue planets.
    #[must_use]
    pub const fn letter(self) -> char {
        match self {
            Self::A => 'A',
            Self::B => 'B',
            Self::C => 'C',
            Self::D => 'D',
            Self::E => 'E',
            Self::BrownDwarf => 'F',
            Self::RoguePlanet => 'G',
        }
    }

    /// The layer with this designation letter.
    #[must_use]
    pub const fn from_letter(letter: char) -> Option<Self> {
        match letter {
            'A' => Some(Self::A),
            'B' => Some(Self::B),
            'C' => Some(Self::C),
            'D' => Some(Self::D),
            'E' => Some(Self::E),
            'F' => Some(Self::BrownDwarf),
            'G' => Some(Self::RoguePlanet),
            _ => None,
        }
    }
}

/// The grid layout of one cell size.
///
/// With `a` bits per axis and `n` index bits, the index is `[n − 1:0]`, z starts at bit `n`, y at
/// `n + a` and x at `n + 2a`. Sizes of 8 ly and up have `a = 14 − k`, `n = 16 + 3k` and the spare
/// bits `[60:58]`; the 4 ly size (`k = −1`) has `a = 15` and takes the spare bits to keep `n = 16`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct GridLayout {
    axis_bits: u32,
    index_bits: u32,
    has_spare: bool,
}

impl GridLayout {
    /// The layout of a cell size.
    #[must_use]
    pub(super) const fn of(size: CellSize) -> Self {
        match size {
            // k = −1: 15 bits per axis for 32,768 cells, and the three spare bits keep the index
            // at 16 bits (brainstorm, "Identifiers").
            CellSize::Ly4 => Self {
                axis_bits: 15,
                index_bits: 16,
                has_spare: false,
            },
            CellSize::Ly8 | CellSize::Ly16 | CellSize::Ly32 | CellSize::Ly64 | CellSize::Ly128 => {
                let k = size.log2_ly() - 3;
                Self {
                    axis_bits: 14 - k,
                    index_bits: 16 + 3 * k,
                    has_spare: true,
                }
            }
        }
    }

    /// Bits per axis.
    #[must_use]
    pub(super) const fn axis_bits(self) -> u32 {
        self.axis_bits
    }

    /// The index field.
    #[must_use]
    pub(super) const fn index(self) -> Field {
        Field::new(self.index_bits - 1, 0)
    }

    /// The cell field of an axis, 0 for x, 1 for y, 2 for z.
    #[must_use]
    pub(super) const fn axis(self, axis: u32) -> Field {
        Field::above(
            self.index_bits + (2 - axis) * self.axis_bits,
            self.axis_bits,
        )
    }

    /// Whether the layout has the spare bits [`SPARE`].
    #[must_use]
    pub(super) const fn has_spare(self) -> bool {
        self.has_spare
    }

    /// The offset added to a cell coordinate to store it: `2^(a − 1)`.
    #[must_use]
    pub(super) const fn offset(self) -> i32 {
        1 << (self.axis_bits - 1)
    }
}

/// Fails compilation unless a size's layout covers 64 bits exactly.
const fn assert_grid_budget(size: CellSize) {
    let layout = GridLayout::of(size);
    if layout.has_spare {
        assert_tiles_64(&[
            LAYER,
            SPARE,
            layout.axis(0),
            layout.axis(1),
            layout.axis(2),
            layout.index(),
        ]);
    } else {
        assert_tiles_64(&[
            LAYER,
            layout.axis(0),
            layout.axis(1),
            layout.axis(2),
            layout.index(),
        ]);
    }
}

const _: () = {
    let mut i = 0;
    while i < CellSize::ALL.len() {
        assert_grid_budget(CellSize::ALL[i]);
        i += 1;
    }
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layers_match_the_layout_tables() {
        // (layer, value, cell ly, bits per axis, index bits, x field, index capacity)
        let table = [
            (Layer::A, 0, 8, 14, 16, (57, 44), 65_536),
            (Layer::B, 1, 16, 13, 19, (57, 45), 524_288),
            (Layer::C, 2, 32, 12, 22, (57, 46), 4_194_304),
            (Layer::D, 3, 64, 11, 25, (57, 47), 33_554_432),
            (Layer::E, 4, 128, 10, 28, (57, 48), 268_435_456),
            (Layer::BrownDwarf, 5, 16, 13, 19, (57, 45), 524_288),
            (Layer::RoguePlanet, 6, 4, 15, 16, (60, 46), 65_536),
        ];
        for (layer, value, ly, axis_bits, index_bits, (x_hi, x_lo), capacity) in table {
            assert_eq!(layer.value(), value);
            assert_eq!(Layer::from_value(value), Some(layer));
            assert_eq!(layer.cell_size_ly(), ly);
            assert_eq!(layer.cell_bits_per_axis(), axis_bits);
            assert_eq!(layer.index_bits(), index_bits);
            assert_eq!(1_u64 << layer.index_bits(), capacity);
            let x = GridLayout::of(layer.cell_size()).axis(0);
            assert_eq!((x.hi(), x.lo()), (x_hi, x_lo), "{layer:?}");
            assert_eq!(Layer::from_letter(layer.letter()), Some(layer));
        }
        assert_eq!(Layer::from_value(RESERVED_LAYER_VALUE), None);
        assert_eq!(Layer::ALL.map(Layer::value), [0, 1, 2, 3, 4, 5, 6]);
        assert_eq!(
            Layer::ALL.map(Layer::letter),
            ['A', 'B', 'C', 'D', 'E', 'F', 'G']
        );
    }

    #[test]
    fn grid_fields_sit_where_the_tables_put_them() {
        // Layer A: x [57:44], y [43:30], z [29:16], index [15:0].
        let a = GridLayout::of(CellSize::Ly8);
        assert_eq!((a.axis(1).hi(), a.axis(1).lo()), (43, 30));
        assert_eq!((a.axis(2).hi(), a.axis(2).lo()), (29, 16));
        assert_eq!((a.index().hi(), a.index().lo()), (15, 0));
        // Layer E: z [37:28], index [27:0].
        let e = GridLayout::of(CellSize::Ly128);
        assert_eq!((e.axis(2).hi(), e.axis(2).lo()), (37, 28));
        assert_eq!(e.index().hi(), 27);
        // Rogue planets: y [45:31], z [30:16].
        let r = GridLayout::of(CellSize::Ly4);
        assert_eq!((r.axis(1).hi(), r.axis(1).lo()), (45, 31));
        assert_eq!((r.axis(2).hi(), r.axis(2).lo()), (30, 16));
        assert!(!r.has_spare());
        assert_eq!(r.offset(), 16_384);
        assert_eq!(a.offset(), 8_192);
    }

    #[test]
    fn capacity_per_cubic_light_year_is_128_for_a_and_1024_for_rogue_planets() {
        let per_ly3 =
            |layer: Layer| (1_u64 << layer.index_bits()) / u64::from(layer.cell_size_ly()).pow(3);
        assert_eq!(per_ly3(Layer::A), 128);
        assert_eq!(per_ly3(Layer::RoguePlanet), 1_024);
        // The sliding split gives every layer of 8 ly and up the same room per cubic light-year,
        // so the finest stellar layer, with the largest share of systems, is the tightest: at 76%
        // of systems (Kroupa) it caps the total near 170 per cubic light-year (brainstorm,
        // "Identifiers"). The brown dwarfs reuse layer B's layout and so its room.
        for layer in [Layer::B, Layer::C, Layer::D, Layer::E, Layer::BrownDwarf] {
            assert_eq!(per_ly3(layer), 128, "{layer:?}");
        }
        // The rogue planets' densest centre, about 700 per cubic light-year for the densest seed
        // (brainstorm, "Between the stars"), fits under their limit.
        assert!(per_ly3(Layer::RoguePlanet) > 700);
    }
}
