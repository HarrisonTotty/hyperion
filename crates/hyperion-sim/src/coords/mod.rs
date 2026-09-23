//! Coordinates: the galactic, system and body frames, generation cells, the galactic axes and
//! the named directions.
//!
//! # Frames
//!
//! An `f64` in metres cannot address the galaxy: at 50,000 ly from the origin adjacent
//! representable values are about 65 km apart. Positions therefore live in nested frames:
//!
//! | Frame    | Representation                                                     | Resolution          |
//! | -------- | ------------------------------------------------------------------ | ------------------- |
//! | Galactic | [`GalacticPosition`]: a [`LyCell`] (`i32` per axis) + `f64` metres | about 2 m anywhere  |
//! | System   | [`SystemPosition`]: `f64` metres from the system barycentre        | about 1 mm at 50 au |
//! | Body     | [`BodyPosition`]: `f64` metres from the body's centre              | sub-micrometre      |
//!
//! The system and body frames are translations of the galactic frame: their axes are parallel to
//! the galactic axes and only the origin moves. No conversion between frames exists without an
//! explicit origin, so the types cannot be mixed by accident. [`Frame`] names a frame and its
//! origin's owner. Which frame a ship is in, and when it changes, is decided elsewhere, because
//! the rule needs the systems around the ship.
//!
//! # Axes
//!
//! The origin is the galactic centre. **+x** runs along the bar's long axis. **+z** is galactic
//! north, chosen so that the galaxy rotates counter-clockwise seen from the north; the spiral arms
//! trail. The named directions follow: **coreward** is towards the z axis, **spinward** is the
//! direction of rotation, **north** is +z, and rimward, antispinward and south are their
//! opposites. They are local directions ([`Directions`]): undefined on the z axis itself, and
//! turning noticeably across a chart close to the centre. (Rimward, spinward, north) is a
//! right-handed triad.
//!
//! # Cells
//!
//! The 1 ly cells of the galactic frame and the larger generation cells ([`GenCell`], 4 to 128 ly)
//! are the same grid at different scales: a generation cell's coordinate is the light-year
//! coordinate divided by the cell size, rounded down, so the planes x, y, z = 0 are cell faces at
//! every size. The root cube spans `±`[`ROOT_HALF_WIDTH_LY`] on each axis. Positions inside a
//! generation cell are drawn as integer light-years plus an offset
//! ([`GenCell::position_from_words`]), never as one `f64` across a whole cell.

mod cell;
mod directions;
mod frames;
mod galactic;
mod vec3;

pub use cell::{BuildGenCellError, CellSize, GenCell, LyCell};
pub use directions::{Cylindrical, Directions, UnitVector};
pub use frames::{BodyPosition, Frame, SystemPosition, SystemVector, SystemVelocity};
pub use galactic::{
    BuildGalacticPositionError, GalacticDisplacement, GalacticPosition, GalacticVelocity,
};

/// Half the width of the root cube in light-years: the cube runs from −65,536 to +65,536 ly on
/// each axis (2¹⁷ ly across).
pub const ROOT_HALF_WIDTH_LY: i32 = 65_536;
