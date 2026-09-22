//! The cell walk: which cells of a layer meet a query's sphere (plan 03, P03.T9.b).
//!
//! Cells are chosen by epoch position against the padded sphere. A cell is kept when the squared
//! distance from the sphere's centre to the cell's closed box is at most the padded radius
//! squared. Each axis's distance is taken from the whole light-years between the centre's cell
//! and the box's face, an exact integer, and the centre's fraction of a light-year, in one
//! rounding, so nothing loses precision far from the origin; the three squares are added in the
//! order x, y, z. The walk is clipped to the root cube.
//!
//! Along a column of cells in z the kept cells are contiguous, because the distance to a cell
//! falls towards the centre's cell and rises beyond it and float addition is monotone. So each
//! column's ends are found by bisection on the same test, and [`count_cells_in_sphere`] counts
//! exactly the cells [`cells_in_sphere`] yields without visiting them.

use std::error::Error;
use std::fmt;

use crate::coords::{GalacticPosition, ROOT_HALF_WIDTH_LY};
use crate::galaxy::placement::{CellKey, layer_spec};
use crate::id::Layer;
use crate::time::UniverseTime;
use crate::units::LightYears;
use crate::units::consts::METRES_PER_LIGHT_YEAR;

/// The largest reach of a walk from its centre, light-years: 2³⁴, beyond the far corner of the
/// root cube from any centre whose light-year cell is an `i32`. A larger radius changes nothing.
const MAX_REACH_LY: f64 = 17_179_869_184.0;

/// A [`QuerySphere`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildQuerySphereError {
    /// The radius is not finite and positive.
    InvalidRadius,
    /// The pad is not finite and non-negative, or the padded radius is not finite.
    InvalidPad,
}

impl fmt::Display for BuildQuerySphereError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidRadius => "a query sphere's radius must be finite and positive",
            Self::InvalidPad => {
                "a query sphere's pad must be finite and non-negative, and its padded radius finite"
            }
        })
    }
}

impl Error for BuildQuerySphereError {}

/// The sphere a query walks: its centre, its radius and time, and the radius padded for motion
/// that chooses the cells.
///
/// Cells are chosen by epoch position, so the walk uses the radius plus a pad covering the
/// farthest any system can move over |t| (brainstorm, "The range query"); distances are then
/// tested at `time` against the unpadded radius.
///
/// # Examples
///
/// ```
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::query::{QuerySphere, cells_in_sphere, count_cells_in_sphere};
/// use hyperion_sim::id::Layer;
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::LightYears;
///
/// // A 50 ly sphere in the disc, at the epoch, so with no pad.
/// let centre = GalacticPosition::from_light_years([1_234.4, 26_000.6, 17.2]).expect("in range");
/// let sphere =
///     QuerySphere::new(centre, LightYears::new(50.0), UniverseTime::EPOCH, LightYears::ZERO)?;
/// // About 1,400 layer-A cells meet it, and counting them visits none.
/// let count = count_cells_in_sphere(Layer::A, &sphere);
/// assert!((1_300..1_550).contains(&count));
/// let walked = cells_in_sphere(Layer::A, &sphere).count();
/// assert_eq!(u64::try_from(walked).expect("about 1,400 cells"), count);
/// # Ok::<(), hyperion_sim::galaxy::query::BuildQuerySphereError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuerySphere {
    centre: GalacticPosition,
    radius: LightYears,
    time: UniverseTime,
    padded_radius: LightYears,
}

impl QuerySphere {
    /// The sphere of `radius` about `centre` at time `time`, whose cells are chosen within
    /// `radius + pad`.
    ///
    /// The pad must cover the farthest any system of a walked layer can move between the epoch
    /// and `time`: at least `pad_for(time, pad_speed(layer))` (P03.T9.e) for every layer walked
    /// with this sphere. A caller walking layers with different padding speeds builds a sphere for
    /// each, or pads for the fastest. A pad of zero is right only at the epoch.
    ///
    /// # Errors
    ///
    /// - [`BuildQuerySphereError::InvalidRadius`] unless `radius` is finite and positive.
    /// - [`BuildQuerySphereError::InvalidPad`] unless `pad` is finite and non-negative and the
    ///   padded radius finite.
    pub fn new(
        centre: GalacticPosition,
        radius: LightYears,
        time: UniverseTime,
        pad: LightYears,
    ) -> Result<Self, BuildQuerySphereError> {
        if !(radius.value().is_finite() && radius.value() > 0.0) {
            return Err(BuildQuerySphereError::InvalidRadius);
        }
        let padded_radius = radius + pad;
        if !(pad.value().is_finite() && pad.value() >= 0.0 && padded_radius.value().is_finite()) {
            return Err(BuildQuerySphereError::InvalidPad);
        }
        Ok(Self {
            centre,
            radius,
            time,
            padded_radius,
        })
    }

    /// The centre, in the galactic frame.
    #[must_use]
    pub const fn centre(&self) -> &GalacticPosition {
        &self.centre
    }

    /// The radius distances are tested against at [`time`](Self::time).
    #[must_use]
    pub const fn radius(&self) -> LightYears {
        self.radius
    }

    /// The time distances are tested at.
    #[must_use]
    pub const fn time(&self) -> UniverseTime {
        self.time
    }

    /// The radius cells are chosen within: [`radius`](Self::radius) plus the pad for motion.
    #[must_use]
    pub const fn padded_radius(&self) -> LightYears {
        self.padded_radius
    }
}

/// Every cell of `layer` whose closed box meets the padded sphere, inside the root cube, in
/// ascending order of x, then y, then z (the order of [`CellKey`]). Nothing for the substellar
/// layers, which plan 13 places.
pub fn cells_in_sphere(
    layer: Layer,
    sphere: &QuerySphere,
) -> impl Iterator<Item = CellKey> + use<> {
    Walk::new(layer, sphere).into_iter().flat_map(|walk| {
        walk.cells(0).flat_map(move |x| {
            let gx2_ly2 = walk.gap_squared_ly2(0, x);
            walk.cells(1)
                .filter_map(move |y| {
                    walk.column(gx2_ly2 + walk.gap_squared_ly2(1, y))
                        .map(|z| (y, z))
                })
                .flat_map(move |(y, (z_first, z_last))| {
                    (z_first..=z_last).map(move |z| walk.key([x, y, z]))
                })
        })
    })
}

/// How many cells [`cells_in_sphere`] yields, found column by column without visiting a cell.
#[must_use]
pub fn count_cells_in_sphere(layer: Layer, sphere: &QuerySphere) -> u64 {
    let Some(walk) = Walk::new(layer, sphere) else {
        return 0;
    };
    let mut count = 0_u64;
    for x in walk.cells(0) {
        let gx2_ly2 = walk.gap_squared_ly2(0, x);
        for y in walk.cells(1) {
            if let Some((first, last)) = walk.column(gx2_ly2 + walk.gap_squared_ly2(1, y)) {
                count += (last - first).unsigned_abs() + 1;
            }
        }
    }
    count
}

/// One axis of a walk: where the centre is, and the range of cells to look at.
#[derive(Debug, Clone, Copy)]
struct Axis {
    /// The centre's whole light-year on this axis.
    centre_ly: i64,
    /// The centre's distance above that light-year, light-years, in `[0, 1)`.
    fraction_ly: f64,
    /// The cell holding the centre, on the layer's grid.
    centre_cell: i64,
    /// The first and last cells of the sphere's bounding box, clipped to the root cube.
    first: i64,
    last: i64,
}

/// A walk of one stellar layer over one padded sphere.
#[derive(Debug, Clone, Copy)]
struct Walk {
    layer: Layer,
    /// The cell edge, light-years.
    cell_ly: i64,
    axes: [Axis; 3],
    /// The padded radius squared, ly².
    radius_squared_ly2: f64,
}

impl Walk {
    /// The walk of `layer` over `sphere`, or `None` for a substellar layer or a sphere that
    /// misses the root cube's cells altogether.
    #[must_use]
    fn new(layer: Layer, sphere: &QuerySphere) -> Option<Self> {
        let spec = layer_spec(layer)?;
        let cell_ly = i64::from(spec.cell_ly());
        let padded = sphere.padded_radius.value();
        // Whole light-years that cover the padded radius; the clamp keeps any finite radius in
        // range and changes no cell, since 2³⁴ ly reaches past the cube from any centre.
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a whole number of light-years in [1, 2^34], exact in i64"
        )]
        let reach = padded.ceil().clamp(1.0, MAX_REACH_LY) as i64;
        // The cube's cells on this grid run from −h to h − 1.
        let half = i64::from(ROOT_HALF_WIDTH_LY) / cell_ly;
        let cell = sphere.centre.cell().to_array();
        let offset = sphere.centre.offset_metres();
        let axes = [0, 1, 2].map(|axis| {
            let centre_ly = i64::from(cell[axis]);
            Axis {
                centre_ly,
                fraction_ly: offset[axis] / METRES_PER_LIGHT_YEAR,
                centre_cell: centre_ly.div_euclid(cell_ly),
                // [c − R, c + R] lies inside [c_ly − reach, c_ly + 1 + reach]. A cell that only
                // touches the sphere has a face on that interval's end, so the box reaches one
                // light-year beyond it on both sides: on the low side, the cell whose high face is
                // at c_ly − reach.
                first: (centre_ly - reach - 1).div_euclid(cell_ly).max(-half),
                last: (centre_ly + 1 + reach).div_euclid(cell_ly).min(half - 1),
            }
        });
        if axes.iter().any(|axis| axis.first > axis.last) {
            return None;
        }
        Some(Self {
            layer,
            cell_ly,
            axes,
            radius_squared_ly2: padded * padded,
        })
    }

    /// The cells of the bounding box on one axis, 0 for x, 1 for y.
    #[must_use]
    fn cells(self, axis: usize) -> std::ops::RangeInclusive<i64> {
        self.axes[axis].first..=self.axes[axis].last
    }

    /// The squared distance, ly², from the centre to the slab of `cell` on one axis: zero inside
    /// it, otherwise the gap to the nearer face.
    #[must_use]
    fn gap_squared_ly2(self, axis: usize, cell: i64) -> f64 {
        let axis = &self.axes[axis];
        let low = cell * self.cell_ly;
        let high = low + self.cell_ly;
        let gap_ly = if axis.centre_ly >= high {
            // At or beyond the high face: c − high = (c_ly − high) + fraction.
            exact(axis.centre_ly - high) + axis.fraction_ly
        } else if low > axis.centre_ly {
            // Below the low face: low − c = (low − c_ly) − fraction, with low ≥ c_ly + 1.
            exact(low - axis.centre_ly) - axis.fraction_ly
        } else {
            0.0
        };
        gap_ly * gap_ly
    }

    /// The first and last cells in z of the column whose squared distance in x and y is
    /// `gxy2_ly2`, or `None` if no cell of it meets the sphere.
    #[must_use]
    fn column(self, gxy2_ly2: f64) -> Option<(i64, i64)> {
        let z = self.axes[2];
        let inside =
            |cell: i64| gxy2_ly2 + self.gap_squared_ly2(2, cell) <= self.radius_squared_ly2;
        // The nearest cell of the column; the test only fails further from it on either side.
        let nearest = z.centre_cell.clamp(z.first, z.last);
        if !inside(nearest) {
            return None;
        }
        let (mut low, mut high) = (z.first, nearest);
        while low < high {
            let mid = low + (high - low) / 2;
            if inside(mid) {
                high = mid;
            } else {
                low = mid + 1;
            }
        }
        let first = low;
        let (mut low, mut high) = (nearest, z.last);
        while low < high {
            let mid = low + (high - low + 1) / 2;
            if inside(mid) {
                low = mid;
            } else {
                high = mid - 1;
            }
        }
        Some((first, low))
    }

    /// The key of a cell of the walk.
    ///
    /// # Panics
    ///
    /// Never: the walk's cells lie in the root cube, whose cell coordinates fit in `i32`.
    #[must_use]
    fn key(self, cell: [i64; 3]) -> CellKey {
        let cell = cell.map(|c| i32::try_from(c).expect("a cell of the root cube fits in i32"));
        CellKey::new(self.layer, cell)
            .expect("a walk's cells are of a stellar layer and in the cube")
    }
}

/// An integer number of light-years as an `f64`.
#[must_use]
fn exact(ly: i64) -> f64 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "a distance from an i32 light-year to a face of the root cube is below 2^33 ly, \
                  exact in f64"
    )]
    let ly = ly as f64;
    ly
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::coords::LyCell;
    use crate::galaxy::placement::{LayerSpec, STELLAR_LAYERS};

    fn stellar_layers() -> impl Iterator<Item = Layer> {
        STELLAR_LAYERS.iter().map(LayerSpec::layer)
    }

    /// A position at whole light-years `cell` plus the fractions `fraction` of a light-year.
    fn at(cell: [i32; 3], fraction: [f64; 3]) -> GalacticPosition {
        GalacticPosition::new(
            LyCell::new(cell),
            fraction.map(|f| f * METRES_PER_LIGHT_YEAR),
        )
        .unwrap()
    }

    fn sphere(centre: GalacticPosition, radius: f64) -> QuerySphere {
        QuerySphere::new(
            centre,
            LightYears::new(radius),
            UniverseTime::EPOCH,
            LightYears::ZERO,
        )
        .unwrap()
    }

    /// Every cell of the layer within two cells of the sphere's bounding box, inside the cube,
    /// kept by the closed-box distance test written out without the walk's column search.
    fn brute_force(layer: Layer, sphere: &QuerySphere) -> Vec<CellKey> {
        let size = i64::from(layer.cell_size_ly());
        let half = i64::from(ROOT_HALF_WIDTH_LY) / size;
        let radius = sphere.padded_radius().value();
        let cell = sphere.centre().cell().to_array().map(i64::from);
        let fraction = sphere
            .centre()
            .offset_metres()
            .map(|m| m / METRES_PER_LIGHT_YEAR);
        #[expect(
            clippy::cast_possible_truncation,
            reason = "test radii are small whole-number bounds"
        )]
        let reach = radius.ceil() as i64;
        let range = |axis: usize| {
            let first = (cell[axis] - reach).div_euclid(size) - 2;
            let last = (cell[axis] + reach + 1).div_euclid(size) + 2;
            first.max(-half)..=last.min(half - 1)
        };
        // max(low − c, 0, c − high) on each axis, from whole light-years and the fraction.
        let gap = |axis: usize, c: i64| {
            let low = c * size;
            let high = low + size;
            let below = exact(low - cell[axis]) - fraction[axis];
            let above = exact(cell[axis] - high) + fraction[axis];
            below.max(above).max(0.0)
        };
        let mut kept = Vec::new();
        for x in range(0) {
            for y in range(1) {
                for z in range(2) {
                    let [gx, gy, gz] = [gap(0, x), gap(1, y), gap(2, z)];
                    if gx * gx + gy * gy + gz * gz <= radius * radius {
                        let key = [x, y, z].map(|c| i32::try_from(c).unwrap());
                        kept.push(CellKey::new(layer, key).unwrap());
                    }
                }
            }
        }
        kept
    }

    fn assert_walk_is_brute_force(layer: Layer, sphere: &QuerySphere) -> usize {
        let walked: Vec<_> = cells_in_sphere(layer, sphere).collect();
        assert_eq!(walked, brute_force(layer, sphere), "{layer:?} {sphere:?}");
        assert_eq!(
            count_cells_in_sphere(layer, sphere),
            u64::try_from(walked.len()).unwrap()
        );
        // Ascending and so free of repeats.
        assert!(walked.windows(2).all(|pair| pair[0] < pair[1]));
        walked.len()
    }

    #[test]
    fn the_walk_is_the_brute_force_scan_of_the_bounding_box() {
        let centres = [
            at([1_234, 26_000, 17], [0.37, 0.61, 0.23]),
            at([-8_001, 4_095, -300], [0.999, 0.0, 0.5]),
            at([16, -16, 0], [0.0, 0.0, 0.0]),
            at([0, 26_000, 0], [0.0, 0.0, 0.0]),
        ];
        // Whole radii from whole-light-year centres put cell faces exactly on the sphere, where
        // a cell only touches it: those cells are kept, on the low side as on the high.
        for centre in centres {
            for radius in [0.3, 3.7, 8.0, 12.0, 24.0, 32.0, 50.0, 64.0, 131.5] {
                for layer in stellar_layers() {
                    assert_walk_is_brute_force(layer, &sphere(centre, radius));
                }
            }
        }
    }

    #[test]
    fn a_50_ly_sphere_meets_the_brainstorms_cell_counts() {
        // Brainstorm, "The range query": about 1,400 layer-A cells and about 300 of the other
        // layers together. Steiner's formula for a ball swept by a cube gives 1,429 and 307.
        let sphere = sphere(at([1_234, 26_000, 17], [0.37, 0.61, 0.23]), 50.0);
        let a = count_cells_in_sphere(Layer::A, &sphere);
        assert!((1_190..=1_610).contains(&a), "{a} layer-A cells");
        let coarse: u64 = [Layer::B, Layer::C, Layer::D, Layer::E]
            .into_iter()
            .map(|layer| count_cells_in_sphere(layer, &sphere))
            .sum();
        assert!(
            (240..=360).contains(&coarse),
            "{coarse} cells in layers B–E"
        );
    }

    #[test]
    fn spheres_across_the_axis_planes_are_walked_exactly() {
        let centre = at([0, -1, 0], [0.3, 0.8, 0.1]);
        for layer in stellar_layers() {
            let count = assert_walk_is_brute_force(layer, &sphere(centre, 20.0));
            assert!(count >= 8, "{layer:?}: {count}");
        }
        // Symmetric about all three planes: a sphere at the origin meets as many cells on
        // either side of each, whether or not its radius lands on cell faces.
        for radius in [30.0, 32.0, 64.0] {
            let origin = sphere(GalacticPosition::ORIGIN, radius);
            for layer in stellar_layers() {
                let keys: Vec<_> = cells_in_sphere(layer, &origin).collect();
                for axis in 0..3 {
                    let below = keys.iter().filter(|k| k.origin_ly()[axis] < 0).count();
                    assert_eq!(2 * below, keys.len(), "{layer:?} axis {axis} R {radius}");
                }
            }
        }
        // At R = 32 layer A holds 432 cells: the cells touching the sphere count on every side.
        assert_eq!(
            count_cells_in_sphere(Layer::A, &sphere(GalacticPosition::ORIGIN, 32.0)),
            432
        );
    }

    #[test]
    fn spheres_across_the_cubes_faces_are_clipped_to_it() {
        let near_corner = at([65_530, -65_530, 65_500], [0.5, 0.5, 0.5]);
        let outside = at([65_600, 0, 0], [0.0; 3]);
        for layer in stellar_layers() {
            let clipped = assert_walk_is_brute_force(layer, &sphere(near_corner, 40.0));
            let whole = count_cells_in_sphere(layer, &sphere(at([0, 0, 0], [0.5; 3]), 40.0));
            assert!(u64::try_from(clipped).unwrap() < whole, "{layer:?}");
            // A sphere centred outside the cube meets only the cells it reaches inside.
            assert_walk_is_brute_force(layer, &sphere(outside, 100.0));
            assert_eq!(count_cells_in_sphere(layer, &sphere(outside, 60.0)), 0);
        }
    }

    #[test]
    fn a_sphere_smaller_than_a_cell_meets_one_two_or_eight_cells() {
        let inside = sphere(at([3, 3, 3], [0.5; 3]), 0.5);
        assert_eq!(count_cells_in_sphere(Layer::A, &inside), 1);
        let on_a_face = sphere(at([8, 3, 3], [0.0, 0.5, 0.5]), 0.5);
        assert_eq!(count_cells_in_sphere(Layer::A, &on_a_face), 2);
        let on_a_corner = sphere(at([8, 8, -8], [0.0; 3]), 0.5);
        assert_eq!(count_cells_in_sphere(Layer::A, &on_a_corner), 8);
        let tiny = sphere(at([100, 100, 100], [0.25; 3]), 1e-9);
        for layer in stellar_layers() {
            let keys: Vec<_> = cells_in_sphere(layer, &tiny).collect();
            let expected = CellKey::containing(layer, tiny.centre()).unwrap();
            assert_eq!(keys, [expected], "{layer:?}");
        }
    }

    #[test]
    fn the_pad_widens_the_walk_and_the_radius_is_kept() {
        let centre = at([1_234, 26_000, 17], [0.37, 0.61, 0.23]);
        let padded = QuerySphere::new(
            centre,
            LightYears::new(50.0),
            UniverseTime::EPOCH,
            LightYears::new(1.0),
        )
        .unwrap();
        assert_same_bits(padded.radius().value(), 50.0);
        assert_same_bits(padded.padded_radius().value(), 51.0);
        assert_eq!(
            count_cells_in_sphere(Layer::A, &padded),
            count_cells_in_sphere(Layer::A, &sphere(centre, 51.0))
        );
        assert_walk_is_brute_force(Layer::A, &padded);
    }

    #[test]
    fn the_substellar_layers_are_not_walked() {
        let sphere = sphere(at([0, 26_000, 0], [0.5; 3]), 20.0);
        for layer in [Layer::BrownDwarf, Layer::RoguePlanet] {
            assert_eq!(cells_in_sphere(layer, &sphere).count(), 0);
            assert_eq!(count_cells_in_sphere(layer, &sphere), 0);
        }
    }

    #[test]
    fn bad_radii_and_pads_are_rejected() {
        let centre = GalacticPosition::ORIGIN;
        let build = |radius: f64, pad: f64| {
            QuerySphere::new(
                centre,
                LightYears::new(radius),
                UniverseTime::EPOCH,
                LightYears::new(pad),
            )
        };
        for radius in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(
                build(radius, 0.0),
                Err(BuildQuerySphereError::InvalidRadius),
                "{radius}"
            );
        }
        for pad in [-0.1, f64::NAN, f64::INFINITY] {
            assert_eq!(
                build(1.0, pad),
                Err(BuildQuerySphereError::InvalidPad),
                "{pad}"
            );
        }
        assert_eq!(
            build(f64::MAX, f64::MAX),
            Err(BuildQuerySphereError::InvalidPad)
        );
    }

    #[test]
    fn a_sphere_larger_than_the_cube_walks_every_cell() {
        let huge = sphere(GalacticPosition::ORIGIN, 1e12);
        assert_eq!(
            count_cells_in_sphere(Layer::E, &huge),
            1_024 * 1_024 * 1_024
        );
        let far_outside = sphere(at([2_000_000_000, 0, 0], [0.0; 3]), 1e12);
        assert_eq!(
            count_cells_in_sphere(Layer::E, &far_outside),
            1_024 * 1_024 * 1_024
        );
    }

    #[test]
    fn the_walk_is_the_same_on_repeated_calls() {
        let sphere = sphere(at([-20_000, 3_000, -40], [0.1, 0.2, 0.3]), 75.0);
        for layer in stellar_layers() {
            let first: Vec<_> = cells_in_sphere(layer, &sphere).collect();
            let second: Vec<_> = cells_in_sphere(layer, &sphere).collect();
            assert_eq!(first, second);
        }
    }
}
