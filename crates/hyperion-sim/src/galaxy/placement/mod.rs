//! Placement: every field star system of the galaxy, by exact thinning on five grids (plan 03), and
//! the free-floating brown dwarfs and rogue planets on two more (plan 13).
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
//! The substellar layers, brown dwarfs in 16 ly cells and rogue planets in 4 ly cells
//! ([`SUBSTELLAR_LAYERS`]), are placed by the same code (plan 13, Design note 1): the same
//! candidate count, position, thinning, population pick and age, with the mass drawn from the
//! layer's own law on a stream of its own. Their records are [`SystemRecord`]s whose
//! [`kind`](SystemRecord::kind) is not [`SystemKind::Stellar`]. The catalogue-class hook is not
//! called for them.

mod cache;
mod candidate;
mod cell;
mod generate;
mod headroom;
mod layers;
mod record;
mod resolve;

use std::error::Error;
use std::fmt;

pub use cache::{CellCache, NoCache};
pub use candidate::{CandidateOutcome, evaluate_candidate};
pub use cell::{CellKey, candidate_count};
pub use generate::{cell_heap_bytes, generate_cell};
#[cfg(test)]
pub(crate) use headroom::largest_headroom_mean;
pub use headroom::{check_index_headroom, rogue_planet_saturation_density};
pub(crate) use headroom::{root_octant, saturated};
pub use layers::{
    LayerSpec, STELLAR_LAYERS, SUBSTELLAR_LAYERS, layer_for_initial_mass, layer_spec,
};
pub use record::{Existence, SystemKind, SystemOrigin, SystemRecord};
pub use resolve::{resolve, resolve_with};

use candidate::evaluate_candidate_from_bound;
use cell::layer_bound;

use crate::coords::{BuildGenCellError, GenCell};
use crate::id::Layer;

/// A [`CellKey`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildCellKeyError {
    /// The cell's light-years do not fit in `i32` on some axis.
    CoordinateOutOfRange(BuildGenCellError),
    /// The cell, or the position it was asked for, lies outside the root cube.
    OutsideRootCube(GenCell),
}

impl fmt::Display for BuildCellKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
            Self::OutsideRootCube(_) => None,
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
    /// The ID is of a layer whose objects the stage asked has no model for yet. [`resolve`] no
    /// longer returns it, since plan 13 places the brown dwarfs and the rogue planets (P13.T3.c);
    /// the stellar stage's callers return it for a substellar record until P13.T5.a routes brown
    /// dwarfs through that stage and plan 14 takes the rogue planets.
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
                write!(
                    f,
                    "objects of layer {} are not modelled yet",
                    layer.letter()
                )
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
        let cell = GenCell::new(CellSize::Ly8, [9_000, 0, 0]).unwrap();
        assert!(BuildCellKeyError::OutsideRootCube(cell).source().is_none());
    }
}
