//! IDs of systems placed by the grid, layers 0–6.

use super::bits::Field;
use super::layer::{GridLayout, LAYER, Layer, SPARE};
use super::system::{BuildSystemIdError, DecodeSystemIdError};
use crate::coords::GenCell;

/// Validates the bits of a grid ID under `layer`.
///
/// Every bit pattern of the cell and index fields is valid, because a stored coordinate is the
/// cell coordinate plus `2^(a − 1)` and the root cube holds exactly `2^a` cells per axis. Only the
/// spare bits can be wrong.
pub(super) const fn validate(raw: u64, layer: Layer) -> Result<(), DecodeSystemIdError> {
    if GridLayout::of(layer.cell_size()).has_spare() && !SPARE.is_zero(raw) {
        return Err(DecodeSystemIdError::SpareBitsSet);
    }
    Ok(())
}

/// The ID of a system placed by the grid: a layer, a generation cell of that layer's size, and a
/// candidate index in the cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GridId(u64);

impl GridId {
    /// The ID of candidate `index` in `cell` of `layer`.
    ///
    /// # Errors
    ///
    /// As [`SystemId::from_parts`](super::SystemId::from_parts).
    pub fn new(layer: Layer, cell: GenCell, index: u32) -> Result<Self, BuildSystemIdError> {
        if cell.size() != layer.cell_size() {
            return Err(BuildSystemIdError::CellSizeMismatch);
        }
        if !cell.in_root_cube() {
            return Err(BuildSystemIdError::CellOutsideRootCube);
        }
        let layout = GridLayout::of(layer.cell_size());
        let index = u64::from(index);
        if index > layout.index().max() {
            return Err(BuildSystemIdError::IndexTooLarge);
        }
        let mut raw = LAYER.with(0, u64::from(layer.value()));
        for (axis, coordinate) in (0..3).zip(cell.to_array()) {
            // Inside the root cube the stored coordinate is in 0..2^a, so the sum is non-negative.
            let stored = u64::from((coordinate + layout.offset()).cast_unsigned());
            raw = layout.axis(axis).with(raw, stored);
        }
        Ok(Self(layout.index().with(raw, index)))
    }

    /// Wraps a raw value that has passed [`validate`].
    #[must_use]
    pub(super) const fn from_valid_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// The raw ID.
    #[must_use]
    pub(super) const fn raw(self) -> u64 {
        self.0
    }

    /// The layer.
    ///
    /// # Panics
    ///
    /// Never: a `GridId` holds a validated layer value below 7.
    #[must_use]
    pub fn layer(self) -> Layer {
        Layer::from_value(LAYER.get_u8(self.0)).expect("a grid ID's layer value is below 7")
    }

    /// The generation cell, of the layer's size.
    ///
    /// # Panics
    ///
    /// Never: every stored coordinate names a cell inside the root cube.
    #[must_use]
    pub fn cell(self) -> GenCell {
        let layout = self.layout();
        let [x, y, z] = self.stored_cell();
        let coordinate = |stored: u32| stored.cast_signed() - layout.offset();
        GenCell::new(
            self.layer().cell_size(),
            [coordinate(x), coordinate(y), coordinate(z)],
        )
        .expect("every stored coordinate names a cell inside the root cube")
    }

    /// The candidate index in the cell.
    #[must_use]
    pub fn index(self) -> u32 {
        self.layout().index().get_u32(self.0)
    }

    /// The stored (offset, unsigned) cell coordinates, x, y, z.
    #[must_use]
    pub(super) fn stored_cell(self) -> [u32; 3] {
        let layout = self.layout();
        [0, 1, 2].map(|axis| layout.axis(axis).get_u32(self.0))
    }

    /// The index field of this ID's layout.
    #[must_use]
    pub(super) fn index_field(self) -> Field {
        self.layout().index()
    }

    /// The layout of this ID's layer.
    fn layout(self) -> GridLayout {
        GridLayout::of(self.layer().cell_size())
    }
}
