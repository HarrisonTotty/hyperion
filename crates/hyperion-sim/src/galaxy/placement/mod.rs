//! Placement: every field star system of the galaxy, by exact thinning on five grids (plan 03).
//!
//! The brainstorm's "Placing star systems" and "Exact placement by thinning": five independent
//! grids, layers A to E with cells of 8 to 128 ly, each owning a band of primary initial mass
//! ([`STELLAR_LAYERS`]). A generation cell draws a Poisson number of candidates from the layer's
//! density bound over the cell (plan 02's [`bounds`](super::bounds)); each candidate draws a
//! position and one acceptance mark, which either thins it or picks the density component it
//! belongs to, and an accepted candidate becomes a system with a primary initial mass and a signed
//! age at the epoch. A system's ID is its layer, cell and candidate index, so resolving an ID
//! reruns one candidate and never generates a cell.
//!
//! Everything here is a pure function of the galaxy and the cell or ID asked about: no cell
//! depends on another, on the order cells are generated in, or on what a caller has cached.
//! Caches belong to the caller (brainstorm, "Runtime and code shape").
//!
//! The substellar layers, brown dwarfs and rogue planets, are plan 13's; their IDs are well formed
//! but not placed here.

mod cell;
mod layers;

use std::error::Error;
use std::fmt;

pub use cell::CellKey;
pub use layers::{LayerSpec, STELLAR_LAYERS, layer_for_initial_mass, layer_spec};

use crate::coords::{BuildGenCellError, GenCell};
use crate::id::Layer;

/// A [`CellKey`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildCellKeyError {
    /// The layer is not one of the five stellar layers; the substellar layers are plan 13's.
    NotStellarLayer(Layer),
    /// The cell's light-years do not fit in `i32` on some axis.
    CoordinateOutOfRange(BuildGenCellError),
    /// The cell, or the position it was asked for, lies outside the root cube.
    OutsideRootCube(GenCell),
}

impl fmt::Display for BuildCellKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotStellarLayer(layer) => {
                write!(f, "layer {} is not a stellar layer", layer.letter())
            }
            Self::CoordinateOutOfRange(e) => write!(f, "the cell is out of range: {e}"),
            Self::OutsideRootCube(cell) => {
                let [x, y, z] = cell.to_array();
                write!(
                    f,
                    "the {} ly cell ({x}, {y}, {z}) lies outside the root cube",
                    cell.size().ly()
                )
            }
        }
    }
}

impl Error for BuildCellKeyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CoordinateOutOfRange(e) => Some(e),
            Self::NotStellarLayer(_) | Self::OutsideRootCube(_) => None,
        }
    }
}

/// A well-formed system ID does not name a system this generator version places.
///
/// A system ID is only a candidate's address (brainstorm, "Identifiers": "A well-formed ID does
/// not always name a system"), so every ID read from a save or the protocol is resolved before
/// use, and each way it can fail is a variant here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResolveSystemError {
    /// The ID names no system: its index is not below the cell's candidate count, its candidate
    /// was thinned, or a catalogue class claimed it.
    NoSuchSystem,
    /// The ID is of a layer this generator version does not place: the brown dwarfs and the rogue
    /// planets, until plan 13.
    LayerNotGenerated(Layer),
    /// The ID is under the reserved layer value (a feature member, the galactic centre, a stream,
    /// a dwarf core, pinned content or a catalogue system), which this generator version does not
    /// resolve: plans 09 and 10 add those kinds.
    KindNotGenerated,
}

impl fmt::Display for ResolveSystemError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSuchSystem => f.write_str("no such system"),
            Self::LayerNotGenerated(layer) => {
                write!(f, "layer {} is not generated yet", layer.letter())
            }
            Self::KindNotGenerated => f.write_str("systems of this kind are not generated yet"),
        }
    }
}

impl Error for ResolveSystemError {}

/// A galaxy is too dense for its IDs: a cell could draw more candidates than its layer's index
/// field can number.
///
/// Plan 03, Design note 6: the largest candidate count any cell of a layer can expect, plus eight
/// standard deviations of its Poisson draw, must stay within the layer's index capacity, or the
/// galaxy is not played.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExceedIndexCapacityError {
    /// The densest possible cell of `layer` expects `largest_mean` candidates, and that plus eight
    /// standard deviations exceeds `capacity`.
    LayerTooDense {
        /// The layer.
        layer: Layer,
        /// The largest expected candidate count of any of the layer's cells: its largest density
        /// bound times the cell's volume.
        largest_mean: f64,
        /// The layer's index capacity, `2^index_bits`.
        capacity: u32,
    },
}

impl fmt::Display for ExceedIndexCapacityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LayerTooDense {
                layer,
                largest_mean,
                capacity,
            } => write!(
                f,
                "layer {}'s densest cell expects {largest_mean:.0} candidates, within eight \
                 standard deviations of its index capacity of {capacity}",
                layer.letter()
            ),
        }
    }
}

impl Error for ExceedIndexCapacityError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coords::CellSize;

    #[test]
    fn error_messages_are_lower_case_without_trailing_punctuation() {
        let cell = GenCell::new(CellSize::Ly8, [9_000, 0, 0]).unwrap();
        let gen_cell_error = GenCell::new(CellSize::Ly8, [i32::MAX, 0, 0]).unwrap_err();
        let messages = [
            BuildCellKeyError::NotStellarLayer(Layer::BrownDwarf).to_string(),
            BuildCellKeyError::CoordinateOutOfRange(gen_cell_error).to_string(),
            BuildCellKeyError::OutsideRootCube(cell).to_string(),
            ResolveSystemError::NoSuchSystem.to_string(),
            ResolveSystemError::LayerNotGenerated(Layer::RoguePlanet).to_string(),
            ResolveSystemError::KindNotGenerated.to_string(),
            ExceedIndexCapacityError::LayerTooDense {
                layer: Layer::A,
                largest_mean: 65_000.0,
                capacity: 65_536,
            }
            .to_string(),
        ];
        for message in messages {
            let first = message.chars().next().unwrap();
            assert!(!first.is_uppercase(), "{message}");
            assert!(!message.ends_with(['.', '!', '?']), "{message}");
        }
    }

    #[test]
    fn a_cell_outside_the_root_cube_is_named_in_the_message() {
        let cell = GenCell::new(CellSize::Ly8, [9_000, 0, 0]).unwrap();
        assert_eq!(
            BuildCellKeyError::OutsideRootCube(cell).to_string(),
            "the 8 ly cell (9000, 0, 0) lies outside the root cube"
        );
    }

    #[test]
    fn coordinate_out_of_range_reports_the_gen_cell_error_as_its_source() {
        let gen_cell_error = GenCell::new(CellSize::Ly8, [i32::MAX, 0, 0]).unwrap_err();
        let error = BuildCellKeyError::CoordinateOutOfRange(gen_cell_error);
        assert_eq!(
            error
                .source()
                .and_then(|source| source.downcast_ref::<BuildGenCellError>()),
            Some(&gen_cell_error)
        );
        assert!(
            BuildCellKeyError::NotStellarLayer(Layer::A)
                .source()
                .is_none()
        );
    }
}
