//! The segment walk: which cells of a layer lie within a tube about a line segment (plan 12,
//! P12.T4.a).
//!
//! A line of sight is a segment from an observer to a source, and a lens is anything close enough
//! to it. [`cells_along_segment`] finds the cells of one layer whose closed box lies within the
//! tube radius of the segment: a capsule, the segment widened by the radius in every direction.
//!
//! It is a grid traversal in the manner of Amanatides and Woo (1987, "A fast voxel traversal
//! algorithm for ray tracing", Eurographics '87), widened by the tube: it steps through the slabs
//! of cells across the segment's dominant axis, and in each takes the rectangle of cells that the
//! part of the capsule inside the slab can reach, so a wide tube costs what its cross-section holds
//! and not the cube around it. Each candidate cell is kept if the squared distance from the segment
//! to its box is at most the radius squared, found exactly from the convex, piecewise-quadratic
//! squared distance along the segment. Every coordinate is taken relative to the start's light-year
//! cell, whose difference from any other cell is an exact integer, so nothing loses precision far
//! from the origin, and a cell's faces are integer multiples of its size. The cells are returned in
//! order from the start outward, by the projection of each cell's centre on the segment, and then
//! by cell. The walk is clipped to the root cube.

use crate::coords::GalacticPosition;
use crate::galaxy::placement::CellKey;
use crate::id::Layer;
use crate::units::LightYears;
use crate::units::consts::METRES_PER_LIGHT_YEAR;

/// The cells of `layer` whose closed box lies within `tube_radius` of the segment from `from` to
/// `to`, each once, ordered from `from` outward (plan 12's Provides).
///
/// A cell is kept when the least distance between the segment and its box is at most the tube
/// radius, found exactly from the convex, piecewise-quadratic squared distance along the segment
/// (module documentation). Cells outside the root cube are left out, since they hold nothing. A
/// negative or non-finite radius is taken as zero, the segment itself.
///
/// # Examples
///
/// ```
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::query::cells_along_segment;
/// use hyperion_sim::id::Layer;
/// use hyperion_sim::units::LightYears;
///
/// let sun = GalacticPosition::from_light_years([0.5, 26_000.5, 0.5]).ok_or("in range")?;
/// let there = GalacticPosition::from_light_years([0.5, 25_000.5, 0.5]).ok_or("in range")?;
/// // A thousand light-years coreward along one column of 8 ly cells: 126 of them.
/// let cells: Vec<_> = cells_along_segment(Layer::A, &sun, &there, LightYears::ZERO).collect();
/// assert_eq!(cells.len(), 126);
/// // From the observer outward: the first holds the Sun.
/// assert_eq!(cells[0].origin_ly(), [0, 26_000, 0]);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn cells_along_segment(
    layer: Layer,
    from: &GalacticPosition,
    to: &GalacticPosition,
    tube_radius: LightYears,
) -> impl Iterator<Item = CellKey> {
    let radius = tube_radius.value();
    let radius = if radius.is_finite() && radius > 0.0 {
        radius
    } else {
        0.0
    };
    let segment = Segment::new(layer, from, to);
    let candidates = segment.candidates(radius);
    let mut kept: Vec<(f64, [i64; 3])> = candidates
        .into_iter()
        .filter(|&cell| segment.distance_squared_to_cell(cell) <= radius * radius)
        .map(|cell| (segment.projection_of_centre(cell), cell))
        .collect();
    kept.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    kept.into_iter()
        .filter_map(move |(_, cell)| segment.key(cell))
}

/// A segment in light-years relative to its start's light-year cell, and the layer's cell size.
#[derive(Debug, Clone, Copy)]
struct Segment {
    layer: Layer,
    /// The start's light-year cell, the origin of the relative coordinates.
    anchor: [i64; 3],
    /// The start, light-years from the anchor: its fraction of a light-year.
    start: [f64; 3],
    /// The end minus the start, light-years.
    direction: [f64; 3],
    /// The cell edge, light-years.
    size: f64,
}

impl Segment {
    fn new(layer: Layer, from: &GalacticPosition, to: &GalacticPosition) -> Self {
        let anchor = from.cell().to_array().map(i64::from);
        let to_cell = to.cell().to_array().map(i64::from);
        let from_offset = from.offset_metres();
        let to_offset = to.offset_metres();
        let mut start = [0.0; 3];
        let mut direction = [0.0; 3];
        for axis in 0..3 {
            start[axis] = from_offset[axis] / METRES_PER_LIGHT_YEAR;
            let end = exact(to_cell[axis] - anchor[axis]) + to_offset[axis] / METRES_PER_LIGHT_YEAR;
            direction[axis] = end - start[axis];
        }
        Self {
            layer,
            anchor,
            start,
            direction,
            size: f64::from(layer.cell_size_ly()),
        }
    }

    /// The layer cell holding the relative point `p`, per axis.
    #[cfg(test)]
    fn cell_of(&self, p: [f64; 3]) -> [i64; 3] {
        let size = i64::from(self.layer.cell_size_ly());
        let mut cell = [0_i64; 3];
        for axis in 0..3 {
            let whole = p[axis].floor();
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a whole number of light-years within the addressable range"
            )]
            let whole = whole as i64;
            cell[axis] = (self.anchor[axis] + whole).div_euclid(size);
        }
        cell
    }

    /// The low face of cell `k` on `axis`, light-years relative to the anchor: exact.
    fn face(&self, axis: usize, k: i64) -> f64 {
        exact(k * i64::from(self.layer.cell_size_ly()) - self.anchor[axis])
    }

    /// The layer cell holding the relative coordinate `x` on `axis`.
    fn cell_on(&self, axis: usize, x: f64) -> i64 {
        let size = i64::from(self.layer.cell_size_ly());
        let whole = x.floor();
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a whole number of light-years within the addressable range, clamped to it"
        )]
        let whole = whole.clamp(-f64::from(1_u32 << 31), f64::from(1_u32 << 31)) as i64;
        (self.anchor[axis] + whole).div_euclid(size)
    }

    /// Every cell the capsule of `radius` can reach, each once: slab by slab across the dominant
    /// axis, the rectangle the capsule's part inside the slab spans, widened by a hair so that no
    /// rounding of the slab's parameters drops a cell the exact test would keep.
    fn candidates(&self, radius: f64) -> Vec<[i64; 3]> {
        let hair = 1e-6 * self.size;
        let reach = radius + hair;
        let main = (0..3)
            .max_by(|&i, &j| self.direction[i].abs().total_cmp(&self.direction[j].abs()))
            .expect("three axes");
        let (second, third) = match main {
            0 => (1, 2),
            1 => (0, 2),
            _ => (0, 1),
        };
        let end = |axis: usize| self.start[axis] + self.direction[axis];
        let low = self.start[main].min(end(main)) - reach;
        let high = self.start[main].max(end(main)) + reach;
        let mut out = Vec::new();
        for slab in self.cell_on(main, low)..=self.cell_on(main, high) {
            let slab_low = self.face(main, slab) - reach;
            let slab_high = self.face(main, slab + 1) + reach;
            // The parameters at which the segment is within reach of the slab on its own axis.
            let step = self.direction[main];
            let (enter, leave) = if step.is_normal() {
                let at_low = (slab_low - self.start[main]) / step;
                let at_high = (slab_high - self.start[main]) / step;
                (at_low.min(at_high).max(0.0), at_low.max(at_high).min(1.0))
            } else {
                // A point: no step along any axis, since this is the longest.
                if self.start[main] < slab_low || self.start[main] > slab_high {
                    continue;
                }
                (0.0, 1.0)
            };
            if enter > leave {
                continue;
            }
            let span = |axis: usize| {
                let first = self.start[axis] + self.direction[axis] * enter;
                let last = self.start[axis] + self.direction[axis] * leave;
                (
                    self.cell_on(axis, first.min(last) - reach),
                    self.cell_on(axis, first.max(last) + reach),
                )
            };
            let (second_from, second_to) = span(second);
            let (third_from, third_to) = span(third);
            for across in second_from..=second_to {
                for up in third_from..=third_to {
                    let mut cell = [0_i64; 3];
                    cell[main] = slab;
                    cell[second] = across;
                    cell[third] = up;
                    out.push(cell);
                }
            }
        }
        out
    }

    /// The least squared distance from the segment to the closed box of `cell`, light-years².
    ///
    /// Along the segment the squared distance to a box is convex and quadratic between the
    /// parameters at which the segment crosses a face's plane; each piece's least value is found in
    /// closed form and the least of those is the answer.
    fn distance_squared_to_cell(&self, cell: [i64; 3]) -> f64 {
        let lo = [0, 1, 2].map(|axis| self.face(axis, cell[axis]));
        let hi = [0, 1, 2].map(|axis| self.face(axis, cell[axis] + 1));
        let mut breaks = [0.0; 8];
        let mut count = 0;
        breaks[count] = 0.0;
        count += 1;
        for axis in 0..3 {
            let d = self.direction[axis];
            if d.is_normal() {
                for plane in [lo[axis], hi[axis]] {
                    let t = (plane - self.start[axis]) / d;
                    if t > 0.0 && t < 1.0 {
                        breaks[count] = t;
                        count += 1;
                    }
                }
            }
        }
        breaks[count] = 1.0;
        count += 1;
        let breaks = &mut breaks[..count];
        breaks.sort_by(f64::total_cmp);
        let mut least = f64::INFINITY;
        for piece in breaks.windows(2) {
            let (a, b) = (piece[0], piece[1]);
            let middle = f64::midpoint(a, b);
            // f(t) = Σ (α + β t)² over the axes outside the slab at the piece's middle.
            let (mut qa, mut qb, mut qc) = (0.0, 0.0, 0.0);
            for axis in 0..3 {
                let p0 = self.start[axis];
                let d = self.direction[axis];
                let x = p0 + d * middle;
                let (alpha, beta) = if x < lo[axis] {
                    (lo[axis] - p0, -d)
                } else if x > hi[axis] {
                    (p0 - hi[axis], d)
                } else {
                    continue;
                };
                qa += beta * beta;
                qb += 2.0 * alpha * beta;
                qc += alpha * alpha;
            }
            let t = if qa > 0.0 {
                (-qb / (2.0 * qa)).clamp(a, b)
            } else {
                a
            };
            let value = (qa * t + qb) * t + qc;
            least = least.min(value.max(0.0));
        }
        least
    }

    /// The parameter along the segment of the projection of `cell`'s centre, for the order.
    fn projection_of_centre(&self, cell: [i64; 3]) -> f64 {
        let length_sq: f64 = self.direction.iter().map(|d| d * d).sum();
        if length_sq <= 0.0 {
            return 0.0;
        }
        let dot: f64 = (0..3)
            .map(|axis| {
                let centre = self.face(axis, cell[axis]) + 0.5 * self.size;
                (centre - self.start[axis]) * self.direction[axis]
            })
            .sum();
        dot / length_sq
    }

    /// The key of `cell`, or `None` outside the root cube.
    fn key(&self, cell: [i64; 3]) -> Option<CellKey> {
        let cell = [
            i32::try_from(cell[0]).ok()?,
            i32::try_from(cell[1]).ok()?,
            i32::try_from(cell[2]).ok()?,
        ];
        CellKey::new(self.layer, cell).ok()
    }
}

/// An integer number of light-years as an `f64`, exactly: every value the walk forms lies within
/// ±2³⁴, far inside the 2⁵³ an `f64` holds.
fn exact(ly: i64) -> f64 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "cell arithmetic stays within ±2^34 light-years, exact in an f64"
    )]
    let value = ly as f64;
    value
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::lcg::Lcg;

    use super::*;
    use crate::coords::ROOT_HALF_WIDTH_LY;

    fn at_ly(ly: [f64; 3]) -> GalacticPosition {
        GalacticPosition::from_light_years(ly).unwrap()
    }

    const LAYERS: [Layer; 7] = [
        Layer::A,
        Layer::B,
        Layer::C,
        Layer::D,
        Layer::E,
        Layer::BrownDwarf,
        Layer::RoguePlanet,
    ];

    /// Every cell of the bounding box of the capsule that the predicate keeps, inside the cube.
    fn brute_force(
        layer: Layer,
        from: &GalacticPosition,
        to: &GalacticPosition,
        r: f64,
    ) -> Vec<CellKey> {
        let segment = Segment::new(layer, from, to);
        let a = segment.cell_of(segment.start);
        let end = [0, 1, 2].map(|axis| segment.start[axis] + segment.direction[axis]);
        let b = segment.cell_of(end);
        #[expect(clippy::cast_possible_truncation, reason = "a few cells of padding")]
        let pad = (r / segment.size).ceil() as i64 + 1;
        let mut out = Vec::new();
        for x in a[0].min(b[0]) - pad..=a[0].max(b[0]) + pad {
            for y in a[1].min(b[1]) - pad..=a[1].max(b[1]) + pad {
                for z in a[2].min(b[2]) - pad..=a[2].max(b[2]) + pad {
                    if segment.distance_squared_to_cell([x, y, z]) <= r * r {
                        out.extend(segment.key([x, y, z]));
                    }
                }
            }
        }
        out.sort();
        out
    }

    /// P12.T4.a: the walk is the brute-force scan of the capsule's bounding box for 10³ random
    /// segments of every layer, keeps no cell twice, and runs from the start outward.
    #[test]
    fn segment_walk_is_the_brute_force_scan() {
        let mut lcg = Lcg::new(0x1204_a001);
        for i in 0..1_000 {
            let layer = LAYERS[i % LAYERS.len()];
            let size = f64::from(layer.cell_size_ly());
            let centre = [0; 3].map(|_| (lcg.next_f64() - 0.5) * 100_000.0);
            let reach = size * (0.1 + 12.0 * lcg.next_f64());
            let from = at_ly(centre);
            let to = at_ly([0, 1, 2].map(|a| centre[a] + (lcg.next_f64() - 0.5) * 2.0 * reach));
            let r = match i % 4 {
                0 => 0.0,
                1 => size * 0.01 * lcg.next_f64(),
                2 => size * lcg.next_f64(),
                _ => size * 3.0 * lcg.next_f64(),
            };
            let walked: Vec<CellKey> =
                cells_along_segment(layer, &from, &to, LightYears::new(r)).collect();
            let mut sorted = walked.clone();
            sorted.sort();
            let before = sorted.len();
            sorted.dedup();
            assert_eq!(sorted.len(), before, "a cell appears twice in walk {i}");
            assert_eq!(
                sorted,
                brute_force(layer, &from, &to, r),
                "walk {i} of layer {layer:?}"
            );
            // From the start outward.
            let segment = Segment::new(layer, &from, &to);
            let keys: Vec<f64> = walked
                .iter()
                .map(|key| {
                    let o = key.origin_ly().map(i64::from);
                    let cell =
                        [0, 1, 2].map(|axis| o[axis].div_euclid(i64::from(layer.cell_size_ly())));
                    segment.projection_of_centre(cell)
                })
                .collect();
            assert!(
                keys.windows(2).all(|p| p[0] <= p[1]),
                "walk {i} is out of order"
            );
        }
    }

    /// The predicate is the distance: every cell holding a point of the segment is kept at any
    /// radius, and a cell is kept exactly when a dense sampling of the segment comes within the
    /// radius of its box, up to the sampling's step.
    #[test]
    fn segment_walk_keeps_the_cells_within_the_radius() {
        let mut lcg = Lcg::new(0x1204_a002);
        for _ in 0..100 {
            let layer = LAYERS[usize::try_from(lcg.next_below(7)).unwrap()];
            let size = f64::from(layer.cell_size_ly());
            let from_ly = [0; 3].map(|_| (lcg.next_f64() - 0.5) * 10_000.0);
            let to_ly = [0, 1, 2].map(|a| from_ly[a] + (lcg.next_f64() - 0.5) * 8.0 * size);
            let r = size * 1.5 * lcg.next_f64();
            let from = at_ly(from_ly);
            let to = at_ly(to_ly);
            let segment = Segment::new(layer, &from, &to);
            let kept: Vec<CellKey> =
                cells_along_segment(layer, &from, &to, LightYears::new(r)).collect();
            let samples: Vec<[f64; 3]> = (0..=2_000)
                .map(|k| {
                    let t = f64::from(k) / 2_000.0;
                    [0, 1, 2].map(|a| segment.start[a] + segment.direction[a] * t)
                })
                .collect();
            let step = (0..3)
                .map(|a| segment.direction[a].abs())
                .fold(0.0, f64::max)
                / 2_000.0;
            for cell in brute_force(layer, &from, &to, r + 2.0 * size) {
                let o = cell.origin_ly().map(i64::from);
                let c = [0, 1, 2].map(|axis| o[axis].div_euclid(i64::from(layer.cell_size_ly())));
                let lo = [0, 1, 2].map(|axis| segment.face(axis, c[axis]));
                let hi = [0, 1, 2].map(|axis| segment.face(axis, c[axis] + 1));
                let nearest = samples
                    .iter()
                    .map(|p| {
                        (0..3)
                            .map(|a| {
                                let g = (lo[a] - p[a]).max(p[a] - hi[a]).max(0.0);
                                g * g
                            })
                            .sum::<f64>()
                            .sqrt()
                    })
                    .fold(f64::INFINITY, f64::min);
                let is_kept = kept.contains(&cell);
                if nearest <= r {
                    assert!(
                        is_kept,
                        "a cell {nearest} ly from the segment was dropped at {r}"
                    );
                }
                if nearest > r + 2.0 * step {
                    assert!(
                        !is_kept,
                        "a cell {nearest} ly from the segment was kept at {r}"
                    );
                }
            }
        }
    }

    #[test]
    fn segment_walk_of_a_point_takes_its_cells_and_stops_at_the_cubes_edge() {
        let p = at_ly([100.25, -26_000.5, 3.0]);
        let one: Vec<CellKey> = cells_along_segment(Layer::C, &p, &p, LightYears::ZERO).collect();
        assert_eq!(one, vec![CellKey::containing(Layer::C, &p).unwrap()]);
        // A tube about a point at a corner of eight cells takes all eight.
        let centre = at_ly([32.0, 32.0, 32.0]);
        let around: Vec<CellKey> =
            cells_along_segment(Layer::C, &centre, &centre, LightYears::new(1.0)).collect();
        assert_eq!(around.len(), 8, "a point at a corner of eight cells");
        // Cells beyond the root cube are left out.
        let half = f64::from(ROOT_HALF_WIDTH_LY);
        let edge = at_ly([half - 0.5, 0.5, 0.5]);
        let inward = at_ly([half - 300.5, 0.5, 0.5]);
        let cells: Vec<CellKey> =
            cells_along_segment(Layer::E, &edge, &inward, LightYears::new(200.0)).collect();
        assert!(!cells.is_empty());
        assert!(cells.iter().all(|c| c.origin_ly()[0] < ROOT_HALF_WIDTH_LY));
        // A bad radius is the segment itself.
        let nan: Vec<CellKey> =
            cells_along_segment(Layer::E, &edge, &inward, LightYears::new(f64::NAN)).collect();
        let zero: Vec<CellKey> =
            cells_along_segment(Layer::E, &edge, &inward, LightYears::ZERO).collect();
        assert_eq!(nan, zero);
    }
}
