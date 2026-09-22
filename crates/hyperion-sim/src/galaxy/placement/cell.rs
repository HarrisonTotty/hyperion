//! Generation cells of the stellar layers: their keys, their geometry and their candidates' IDs.

use super::layers::layer_spec;
use super::{BuildCellKeyError, ResolveSystemError};
use crate::coords::{GalacticPosition, GenCell};
use crate::galaxy::bounds::CellBox;
use crate::id::{BuildSystemIdError, Layer, SystemId, SystemIdKind};

/// A generation cell of one stellar layer: plan 01's [`GenCell`] plus the layer that owns it.
///
/// A cell size alone does not name a layer, since the brown dwarfs reuse layer B's 16 ly, so the
/// key carries both. It is valid by construction: the layer is one of the five stellar layers, the
/// cell has that layer's size ([`Layer::cell_size`]) and lies inside the root cube, so every index
/// below [`index_capacity`](Self::index_capacity) is a candidate with an ID.
///
/// Keys order by layer, then by cell.
///
/// # Examples
///
/// ```
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::placement::CellKey;
/// use hyperion_sim::id::Layer;
///
/// // The layer-A cell holding a point 26,000 ly out on the +y axis, just below the plane.
/// let position = GalacticPosition::from_light_years([0.5, 26_000.5, -0.5]).expect("in range");
/// let key = CellKey::containing(Layer::A, &position)?;
/// assert_eq!(key.origin_ly(), [0, 26_000, -8]);
/// // Its candidates' IDs name the cell, and the cell can be read back from any of them.
/// let id = key.candidate_id(17).expect("17 is below layer A's 65,536");
/// assert_eq!(CellKey::of(id), Ok(key));
/// # Ok::<(), hyperion_sim::galaxy::placement::BuildCellKeyError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CellKey {
    layer: Layer,
    cell: GenCell,
}

impl CellKey {
    /// The key of the cell with coordinates `cell` on `layer`'s grid.
    ///
    /// # Errors
    ///
    /// - [`BuildCellKeyError::NotStellarLayer`] for the brown-dwarf and rogue-planet layers.
    /// - [`BuildCellKeyError::CoordinateOutOfRange`] if the cell's light-years do not fit in `i32`
    ///   ([`GenCell::new`]).
    /// - [`BuildCellKeyError::OutsideRootCube`] unless `−65,536 ≤ origin` and
    ///   `origin + size ≤ 65,536` ly on every axis ([`GenCell::in_root_cube`]).
    pub fn new(layer: Layer, cell: [i32; 3]) -> Result<Self, BuildCellKeyError> {
        if layer_spec(layer).is_none() {
            return Err(BuildCellKeyError::NotStellarLayer(layer));
        }
        let cell = GenCell::new(layer.cell_size(), cell)
            .map_err(BuildCellKeyError::CoordinateOutOfRange)?;
        Self::inside_root_cube(layer, cell)
    }

    /// The key of `layer`'s cell that contains `position`: the position's light-year cell
    /// ([`GalacticPosition::cell`]) divided by the cell size and rounded down, so a point at −1 ly
    /// is in cell −1 of every layer.
    ///
    /// # Errors
    ///
    /// - [`BuildCellKeyError::NotStellarLayer`] for the brown-dwarf and rogue-planet layers.
    /// - [`BuildCellKeyError::OutsideRootCube`] if the position lies outside the root cube.
    pub fn containing(
        layer: Layer,
        position: &GalacticPosition,
    ) -> Result<Self, BuildCellKeyError> {
        if layer_spec(layer).is_none() {
            return Err(BuildCellKeyError::NotStellarLayer(layer));
        }
        Self::inside_root_cube(
            layer,
            GenCell::of_ly_cell(position.cell(), layer.cell_size()),
        )
    }

    /// The key of the cell a grid ID's candidate was drawn in.
    ///
    /// # Errors
    ///
    /// - [`ResolveSystemError::LayerNotGenerated`] for a brown-dwarf or rogue-planet ID.
    /// - [`ResolveSystemError::KindNotGenerated`] for any ID under the reserved layer value.
    pub fn of(id: SystemId) -> Result<Self, ResolveSystemError> {
        match id.kind() {
            SystemIdKind::Grid(grid) => {
                let layer = grid.layer();
                if layer_spec(layer).is_none() {
                    return Err(ResolveSystemError::LayerNotGenerated(layer));
                }
                // A grid ID's cell has its layer's size and lies inside the root cube.
                Ok(Self {
                    layer,
                    cell: grid.cell(),
                })
            }
            SystemIdKind::FeatureMember(_)
            | SystemIdKind::Centre(_)
            | SystemIdKind::Stream(_)
            | SystemIdKind::DwarfCore(_)
            | SystemIdKind::Pinned(_)
            | SystemIdKind::Catalogue(_) => Err(ResolveSystemError::KindNotGenerated),
        }
    }

    /// A key for a stellar layer's cell of the layer's size, once the cell is known to lie in the
    /// root cube.
    fn inside_root_cube(layer: Layer, cell: GenCell) -> Result<Self, BuildCellKeyError> {
        if cell.in_root_cube() {
            Ok(Self { layer, cell })
        } else {
            Err(BuildCellKeyError::OutsideRootCube(cell))
        }
    }

    /// The layer.
    #[must_use]
    pub const fn layer(&self) -> Layer {
        self.layer
    }

    /// The generation cell, of the layer's size.
    #[must_use]
    pub const fn gen_cell(&self) -> GenCell {
        self.cell
    }

    /// The low corner in the galactic frame, whole light-years, as `[x, y, z]`: a multiple of
    /// [`size_ly`](Self::size_ly) from −65,536 to 65,536 − `size_ly` on each axis.
    #[must_use]
    pub const fn origin_ly(&self) -> [i32; 3] {
        self.cell.origin().to_array()
    }

    /// The edge, light-years: 8 for layer A to 128 for layer E.
    #[must_use]
    pub const fn size_ly(&self) -> u32 {
        self.layer.cell_size_ly()
    }

    /// The volume, cubic light-years: 512 for layer A to 2,097,152 for layer E, exact.
    #[must_use]
    pub fn volume_ly3(&self) -> f64 {
        let edge = f64::from(self.size_ly());
        edge * edge * edge
    }

    /// The cell as plan 02's [`CellBox`], the region its density bound is taken over.
    ///
    /// # Panics
    ///
    /// Never: a generation cell's edge is a power of two, it lies inside the root cube, and the
    /// planes x, y, z = 0 are cell faces of every grid, so no plane crosses it.
    #[must_use]
    pub fn cell_box(&self) -> CellBox {
        CellBox::new(self.origin_ly(), self.size_ly()).expect(
            "a generation cell has a power-of-two edge, lies in the root cube and straddles no \
             axis plane",
        )
    }

    /// How many candidates the cell's IDs can number: `2^index_bits`, 65,536 (2¹⁶) for layer A to
    /// 268,435,456 (2²⁸) for layer E ([`Layer::index_bits`]).
    #[must_use]
    pub const fn index_capacity(&self) -> u32 {
        1 << self.layer.index_bits()
    }

    /// The ID of candidate `index`, or `None` if the index does not fit the layer's index field,
    /// that is `index ≥` [`index_capacity`](Self::index_capacity).
    ///
    /// # Panics
    ///
    /// Never: a grid ID checks only the cell's size, the root cube and the index, and the key's
    /// cell passes the first two.
    #[must_use]
    pub fn candidate_id(&self, index: u32) -> Option<SystemId> {
        match SystemId::from_parts(self.layer, self.cell, index) {
            Ok(id) => Some(id),
            Err(BuildSystemIdError::IndexTooLarge) => None,
            Err(
                e @ (BuildSystemIdError::CellOutsideRootCube
                | BuildSystemIdError::CellSizeMismatch
                | BuildSystemIdError::FieldOutOfRange { .. }
                | BuildSystemIdError::NotCanonical(_)),
            ) => panic!(
                "a grid ID checks only cell size, root cube and index, and a cell key passes the \
                 first two: {e}"
            ),
        }
    }

    /// The word that keys the cell's own streams, `ObjectKey::cell(word)`: the ID of the cell's
    /// candidate 0, which is [`SystemId::cell_word`] of any of its candidates (plan 03, Design
    /// note 1). Generating a cell and resolving one of its IDs both take the word from here.
    ///
    /// # Panics
    ///
    /// Never: index 0 fits every layer's index field.
    #[must_use]
    pub fn cell_word(&self) -> u64 {
        self.candidate_id(0)
            .expect("index 0 fits every layer's index field")
            .raw()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coords::{BuildGenCellError, CellSize, LyCell};
    use crate::galaxy::placement::{LayerSpec, STELLAR_LAYERS};
    use crate::id::CentreMemberId;
    use crate::units::consts::METRES_PER_LIGHT_YEAR;

    /// A position at whole light-years `cell` plus `fraction` of a light-year on every axis.
    fn position(cell: [i32; 3], fraction: f64) -> GalacticPosition {
        GalacticPosition::new(LyCell::new(cell), [fraction * METRES_PER_LIGHT_YEAR; 3]).unwrap()
    }

    fn stellar_layers() -> impl Iterator<Item = Layer> {
        STELLAR_LAYERS.iter().map(LayerSpec::layer)
    }

    #[test]
    fn negative_coordinates_floor_to_the_cell_below() {
        for layer in stellar_layers() {
            let key = CellKey::containing(layer, &position([-1, -1, -1], 0.5)).unwrap();
            assert_eq!(key.gen_cell().to_array(), [-1, -1, -1], "{layer:?}");
            let size = i32::try_from(key.size_ly()).unwrap();
            assert_eq!(key.origin_ly(), [-size; 3], "{layer:?}");
            // One light-year beyond the cell's low face is in the next cell down.
            let below = CellKey::containing(layer, &position([-size - 1, 0, -size], 0.0)).unwrap();
            assert_eq!(below.gen_cell().to_array(), [-2, 0, -1], "{layer:?}");
        }
    }

    #[test]
    fn the_axis_planes_are_cell_faces_in_every_layer() {
        let just_below = 1.0 - f64::EPSILON;
        for layer in stellar_layers() {
            let size = i32::try_from(layer.cell_size_ly()).unwrap();
            let at_zero = CellKey::containing(layer, &position([0, 0, 0], 0.0)).unwrap();
            assert_eq!(at_zero.origin_ly(), [0, 0, 0], "{layer:?}");
            let under = CellKey::containing(layer, &position([-1, -1, -1], just_below)).unwrap();
            assert_eq!(under.origin_ly(), [-size; 3], "{layer:?}");
            // The two cells meet at the plane on every axis: one's high face is the other's low.
            for axis in 0..3 {
                assert_eq!(under.origin_ly()[axis] + size, at_zero.origin_ly()[axis]);
            }
            // Neither box crosses a plane, so both are cell boxes.
            assert_eq!(under.cell_box().min_corner(), [-size; 3]);
            assert_eq!(at_zero.cell_box().edge(), layer.cell_size_ly());
        }
    }

    #[test]
    fn candidate_ids_round_trip_through_of() {
        for layer in stellar_layers() {
            let edge = 65_536 / i32::try_from(layer.cell_size_ly()).unwrap();
            for cell in [
                [0, 0, 0],
                [-1, 3, -7],
                [edge - 1, -edge, 0],
                [-edge, edge - 1, -edge],
            ] {
                let key = CellKey::new(layer, cell).unwrap();
                for index in [0, 1, 4_095, key.index_capacity() - 1] {
                    let id = key.candidate_id(index).unwrap();
                    assert_eq!(CellKey::of(id), Ok(key), "{layer:?} {cell:?} {index}");
                    let SystemIdKind::Grid(grid) = id.kind() else {
                        panic!("a candidate's ID is a grid ID");
                    };
                    assert_eq!(grid.index(), index);
                    // Resolving from an ID and generating from the key reach one cell word.
                    assert_eq!(CellKey::of(id).unwrap().cell_word(), id.cell_word());
                }
                assert_eq!(key.cell_word(), key.candidate_id(0).unwrap().raw());
            }
        }
    }

    #[test]
    fn containing_accepts_exactly_the_positions_in_the_root_cube() {
        for layer in stellar_layers() {
            for axis in 0..3 {
                for ly in [-65_537, -65_536, 65_535, 65_536] {
                    let mut cell = [0, 26_000, 0];
                    cell[axis] = ly;
                    let position = position(cell, 0.5);
                    assert_eq!(
                        CellKey::containing(layer, &position).is_ok(),
                        position.in_root_cube(),
                        "{layer:?} axis {axis} at {ly} ly"
                    );
                }
            }
        }
    }

    #[test]
    fn keys_order_by_layer_then_by_cell() {
        let a = CellKey::new(Layer::A, [100, 100, 100]).unwrap();
        let b = CellKey::new(Layer::B, [-100, -100, -100]).unwrap();
        assert!(a < b);
        let west = CellKey::new(Layer::C, [-5, 7, 7]).unwrap();
        let east = CellKey::new(Layer::C, [4, 7, 7]).unwrap();
        assert!(west < east);
        // The same order as the candidates' IDs.
        assert!(west.cell_word() < east.cell_word());
    }

    #[test]
    fn a_coarse_cell_contains_exactly_eight_cells_of_the_next_finer_layer() {
        let pairs = [
            (Layer::E, Layer::D),
            (Layer::D, Layer::C),
            (Layer::C, Layer::B),
            (Layer::B, Layer::A),
        ];
        for (coarse_layer, fine_layer) in pairs {
            for coarse_cell in [[0, 0, 0], [-1, -1, -1], [5, -3, 2]] {
                let coarse = CellKey::new(coarse_layer, coarse_cell).unwrap();
                let [x, y, z] = coarse_cell.map(|c| 2 * c);
                let mut inside = Vec::new();
                // Every finer cell in a block one cell wider than the coarse cell on each side.
                for fx in x - 1..=x + 2 {
                    for fy in y - 1..=y + 2 {
                        for fz in z - 1..=z + 2 {
                            let fine = CellKey::new(fine_layer, [fx, fy, fz]).unwrap();
                            let corner =
                                GalacticPosition::new(LyCell::new(fine.origin_ly()), [0.0; 3])
                                    .unwrap();
                            if CellKey::containing(coarse_layer, &corner) == Ok(coarse) {
                                // The fine cell's far corner lies in the same coarse cell.
                                let size = i32::try_from(fine.size_ly()).unwrap();
                                let far = position(fine.origin_ly().map(|c| c + size - 1), 0.999);
                                assert_eq!(CellKey::containing(coarse_layer, &far), Ok(coarse));
                                inside.push(fine.gen_cell().to_array());
                            }
                        }
                    }
                }
                let mut expected = Vec::new();
                for dx in 0..2 {
                    for dy in 0..2 {
                        for dz in 0..2 {
                            expected.push([x + dx, y + dy, z + dz]);
                        }
                    }
                }
                assert_eq!(inside, expected, "{coarse_layer:?} {coarse_cell:?}");
            }
        }
    }

    #[test]
    fn keys_outside_the_root_cube_are_rejected() {
        for layer in stellar_layers() {
            let edge = 65_536 / i32::try_from(layer.cell_size_ly()).unwrap();
            assert!(CellKey::new(layer, [edge - 1, -edge, 0]).is_ok());
            for cell in [[edge, 0, 0], [0, -edge - 1, 0], [0, 0, edge]] {
                let gen_cell = GenCell::new(layer.cell_size(), cell).unwrap();
                assert_eq!(
                    CellKey::new(layer, cell),
                    Err(BuildCellKeyError::OutsideRootCube(gen_cell)),
                    "{layer:?} {cell:?}"
                );
            }
        }
        assert_eq!(
            CellKey::new(Layer::A, [i32::MAX, 0, 0]),
            Err(BuildCellKeyError::CoordinateOutOfRange(
                BuildGenCellError::CoordinateOutOfRange {
                    axis: 0,
                    coordinate: i32::MAX
                }
            ))
        );
        let outside = position([65_536, 0, 0], 0.0);
        assert!(matches!(
            CellKey::containing(Layer::E, &outside),
            Err(BuildCellKeyError::OutsideRootCube(_))
        ));
        let at_far_face = position([65_535, -65_536, 0], 0.999);
        assert!(CellKey::containing(Layer::E, &at_far_face).is_ok());
    }

    #[test]
    fn substellar_layers_and_reserved_ids_have_no_key() {
        for layer in [Layer::BrownDwarf, Layer::RoguePlanet] {
            assert_eq!(
                CellKey::new(layer, [0, 0, 0]),
                Err(BuildCellKeyError::NotStellarLayer(layer))
            );
            assert_eq!(
                CellKey::containing(layer, &position([0, 0, 0], 0.5)),
                Err(BuildCellKeyError::NotStellarLayer(layer))
            );
            let cell = GenCell::new(layer.cell_size(), [0, 0, 0]).unwrap();
            let id = SystemId::from_parts(layer, cell, 0).unwrap();
            assert_eq!(
                CellKey::of(id),
                Err(ResolveSystemError::LayerNotGenerated(layer))
            );
        }
        let black_hole = SystemId::from(CentreMemberId::CENTRAL_BLACK_HOLE);
        assert_eq!(black_hole.raw(), 0xf000_0007_0000_0000);
        assert_eq!(
            CellKey::of(black_hole),
            Err(ResolveSystemError::KindNotGenerated)
        );
    }

    #[test]
    fn capacity_is_two_to_the_sixteen_for_a_and_two_to_the_twenty_eight_for_e() {
        let a = CellKey::new(Layer::A, [0, 0, 0]).unwrap();
        let e = CellKey::new(Layer::E, [0, 0, 0]).unwrap();
        assert_eq!(a.index_capacity(), 65_536);
        assert_eq!(e.index_capacity(), 1 << 28);
        for key in [a, e] {
            assert!(key.candidate_id(key.index_capacity() - 1).is_some());
            assert_eq!(key.candidate_id(key.index_capacity()), None);
            assert_eq!(key.candidate_id(u32::MAX), None);
        }
    }

    #[test]
    fn geometry_follows_the_layer() {
        for (layer, size, volume) in [
            (Layer::A, 8, 512.0),
            (Layer::B, 16, 4_096.0),
            (Layer::C, 32, 32_768.0),
            (Layer::D, 64, 262_144.0),
            (Layer::E, 128, 2_097_152.0),
        ] {
            let key = CellKey::new(layer, [3, -2, 1]).unwrap();
            assert_eq!(key.layer(), layer);
            assert_eq!(key.gen_cell().size(), layer.cell_size());
            assert_eq!(key.size_ly(), size);
            assert_eq!(
                key.volume_ly3().total_cmp(&volume),
                std::cmp::Ordering::Equal
            );
            let s = i32::try_from(size).unwrap();
            assert_eq!(key.origin_ly(), [3 * s, -2 * s, s]);
            assert_eq!(key.cell_box().min_corner(), key.origin_ly());
            assert_eq!(key.cell_box().edge(), size);
        }
        // A cell touching the cube's faces still has a box.
        let corner = CellKey::new(Layer::E, [-512, 511, -512]).unwrap();
        assert_eq!(corner.cell_box().min_corner(), [-65_536, 65_408, -65_536]);
        assert_eq!(CellSize::Ly128.ly(), corner.size_ly());
    }
}
