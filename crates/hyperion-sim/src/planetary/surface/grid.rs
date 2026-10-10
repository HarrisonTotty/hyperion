//! The coarse pass's grid: R05's cube-sphere cells at one level as a graph, in the field's
//! [`cell_index`](hyperion_surface::field::cell_index) order, with each cell's centre, solid angle
//! and neighbours (plan R09, Design notes 4–6).
//!
//! The coarse field lives on the cube-sphere quadtree at the body's one level, so that no second
//! spherical indexing exists (the brainstorm's "The grid"). The cube sphere is not equal-area, so
//! every sum over cells is weighted by the cell's true solid angle, which the quadratic warp's
//! great-circle edges keep in closed form. Every cell has four edge neighbours, a cell at a cube
//! corner included; its vertex neighbours, across its corners, number four, or three at a cube
//! corner, about which three cells of a level meet (plan R05, Design note 2). Neighbours across a
//! face edge are R05's own ([`PatchKey::edge_neighbour`], [`PatchKey::corner_neighbours`]), so
//! the graph has no special case at an edge.

use std::error::Error;
use std::fmt;

use hyperion_surface::cube::{Edge, PatchKey, st_to_uv};
use hyperion_surface::field::{CoarseLevel, cell_at_index, cell_index};

use crate::math;
use crate::units::Radians;

/// The cells of one cube-sphere level, in cell-index order, with their centres, solid angles and
/// neighbours.
///
/// Built once per pass and read by every step; immutable, so it is the same whichever step reads
/// it first. Cells are named by their [`cell_index`](hyperion_surface::field::cell_index), a `u32`.
#[derive(Debug, Clone, PartialEq)]
pub struct CellGraph {
    level: u8,
    cells: Vec<PatchKey>,
    centres: Vec<[f64; 3]>,
    solid_angles_sr: Vec<f64>,
    edges: Vec<[u32; 4]>,
    corners: Vec<[Option<u32>; 4]>,
}

/// Why [`CellGraph::new`] refused a level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BuildCellGraphError {
    /// The level asked for, above [`CoarseLevel::MAX`].
    pub level: u8,
}

impl fmt::Display for BuildCellGraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "a coarse grid has a level up to {}, not {}",
            CoarseLevel::MAX.get(),
            self.level
        )
    }
}

impl Error for BuildCellGraphError {}

/// The index into the graph's vectors of cell `cell`.
///
/// # Panics
///
/// Never on the targets the sim builds for: a `u32` fits a `usize` on 32- and 64-bit targets.
#[must_use]
fn slot(cell: u32) -> usize {
    usize::try_from(cell).expect("a cell index fits a usize on every target the sim builds for")
}

impl CellGraph {
    /// The graph of `level`'s 6 · 4^`level` cells.
    ///
    /// # Errors
    ///
    /// [`BuildCellGraphError`] for a level above [`CoarseLevel::MAX`] (8), past which a coarse
    /// grid's memory, about 80 bytes a cell, is not wanted.
    ///
    /// # Panics
    ///
    /// Never: every index below 6 · 4^`level` names a cell.
    pub fn new(level: u8) -> Result<Self, BuildCellGraphError> {
        if level > CoarseLevel::MAX.get() {
            return Err(BuildCellGraphError { level });
        }
        let count = 6_u32 << (2 * level);
        let cells: Vec<PatchKey> = (0..count)
            .map(|n| cell_at_index(level, n).expect("every index below 6 · 4^level is a cell"))
            .collect();
        // Vertex (32, 32) of a cell's 65 × 65 patch is its centre, s = (i + ½) ÷ 2ᴸ, which never
        // lies on a face edge.
        let centres = cells.iter().map(|c| c.vertex_dir(32, 32)).collect();
        let solid_angles_sr = cells.iter().map(|&c| solid_angle_sr(c)).collect();
        let edges = cells
            .iter()
            .map(|c| Edge::ALL.map(|edge| cell_index(c.edge_neighbour(edge))))
            .collect();
        let corners = cells
            .iter()
            .map(|c| c.corner_neighbours().map(|n| n.map(cell_index)))
            .collect();
        Ok(Self {
            level,
            cells,
            centres,
            solid_angles_sr,
            edges,
            corners,
        })
    }

    /// The graph of a coarse field's cells, at `level`.
    ///
    /// # Panics
    ///
    /// Never: a coarse level is at most 8.
    #[must_use]
    pub fn for_field(level: CoarseLevel) -> Self {
        Self::new(level.get()).expect("a coarse level is at most 8")
    }

    /// The graph of a coarse field's climate layer, at `level` − 1 (Design note 17).
    ///
    /// # Panics
    ///
    /// Never: a climate level is at most 7.
    #[must_use]
    pub fn for_climate(level: CoarseLevel) -> Self {
        Self::new(level.climate_level()).expect("a climate level is at most 7")
    }

    /// The quadtree level.
    #[must_use]
    pub const fn level(&self) -> u8 {
        self.level
    }

    /// The number of cells, 6 · 4ᴸ.
    ///
    /// # Panics
    ///
    /// Never: a level of at most 8 has under 2³² cells.
    #[must_use]
    pub fn len(&self) -> u32 {
        u32::try_from(self.cells.len()).expect("a level of at most 8 has under 2^32 cells")
    }

    /// Whether the graph has no cells: never, since level 0 has six.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// The cells, in cell-index order.
    #[must_use]
    pub fn cells(&self) -> &[PatchKey] {
        &self.cells
    }

    /// The cells' centres, unit vectors in the body-fixed frame, in cell-index order.
    #[must_use]
    pub fn centres(&self) -> &[[f64; 3]] {
        &self.centres
    }

    /// The cells' solid angles, steradians, in cell-index order: 4π over the sphere.
    #[must_use]
    pub fn solid_angles_sr(&self) -> &[f64] {
        &self.solid_angles_sr
    }

    /// Cell `cell`.
    ///
    /// # Panics
    ///
    /// If `cell` is not below [`len`](Self::len), as an index past a slice does.
    #[must_use]
    pub fn cell(&self, cell: u32) -> PatchKey {
        self.cells[slot(cell)]
    }

    /// The centre of cell `cell`, a unit vector in the body-fixed frame.
    ///
    /// # Panics
    ///
    /// If `cell` is not below [`len`](Self::len).
    #[must_use]
    pub fn centre(&self, cell: u32) -> [f64; 3] {
        self.centres[slot(cell)]
    }

    /// The solid angle of cell `cell`, steradians.
    ///
    /// # Panics
    ///
    /// If `cell` is not below [`len`](Self::len).
    #[must_use]
    pub fn solid_angle_sr(&self, cell: u32) -> f64 {
        self.solid_angles_sr[slot(cell)]
    }

    /// The four edge neighbours of cell `cell`, in [`Edge::ALL`]'s order: across its u = 0, u = 1,
    /// v = 0 and v = 1 edges, folded onto the next face across a face edge.
    ///
    /// # Panics
    ///
    /// If `cell` is not below [`len`](Self::len).
    #[must_use]
    pub fn edge_neighbours(&self, cell: u32) -> [u32; 4] {
        self.edges[slot(cell)]
    }

    /// The neighbour of cell `cell` across `edge`.
    ///
    /// # Panics
    ///
    /// If `cell` is not below [`len`](Self::len).
    #[must_use]
    pub fn edge_neighbour(&self, cell: u32, edge: Edge) -> u32 {
        let [u_min, u_max, v_min, v_max] = self.edge_neighbours(cell);
        match edge {
            Edge::UMin => u_min,
            Edge::UMax => u_max,
            Edge::VMin => v_min,
            Edge::VMax => v_max,
        }
    }

    /// The neighbours of cell `cell` across its four corners, in [`PatchKey::corner_neighbours`]'s
    /// order, `None` across a cube corner.
    ///
    /// # Panics
    ///
    /// If `cell` is not below [`len`](Self::len).
    #[must_use]
    pub fn corner_neighbours(&self, cell: u32) -> [Option<u32>; 4] {
        self.corners[slot(cell)]
    }

    /// The vertex neighbours of cell `cell`, across its corners: four, or three for a cell at a
    /// cube corner, in [`corner_neighbours`](Self::corner_neighbours)' order.
    ///
    /// # Panics
    ///
    /// If `cell` is not below [`len`](Self::len).
    pub fn vertex_neighbours(&self, cell: u32) -> impl Iterator<Item = u32> + '_ {
        self.corners[slot(cell)].iter().filter_map(|&n| n)
    }

    /// The great-circle angle between the centres of cells `a` and `b`, by atan2 of their cross
    /// and dot products, accurate at every angle: times the radius, the edge length of a distance
    /// transform (Design note 6).
    ///
    /// # Panics
    ///
    /// If `a` or `b` is not below [`len`](Self::len).
    #[must_use]
    pub fn arc(&self, a: u32, b: u32) -> Radians {
        arc(self.centre(a), self.centre(b))
    }
}

/// The index of the cell one level up that holds cell `cell`: the climate record over a cell of
/// the field (Design note 17), since a cell's four children have the consecutive indices 4n to
/// 4n + 3 (the field's Morton order).
#[must_use]
pub const fn parent_index(cell: u32) -> u32 {
    cell >> 2
}

/// The solid angle of `cell`, steradians, in closed form: F(u, v) = atan2(uv, √(1 + u² + v²)), the
/// solid angle the gnomonic rectangle from the face's centre to (u, v) subtends, differenced over
/// the cell's corners in face coordinates, through the pinned `atan2`.
///
/// Each corner's (u, v) is computed from its own integer s and t, so cells that share a corner
/// share its bits.
#[must_use]
fn solid_angle_sr(cell: PatchKey) -> f64 {
    let side = f64::from(1_u32 << cell.level());
    let (u0, u1) = (
        st_to_uv(f64::from(cell.i()) / side),
        st_to_uv(f64::from(cell.i() + 1) / side),
    );
    let (v0, v1) = (
        st_to_uv(f64::from(cell.j()) / side),
        st_to_uv(f64::from(cell.j() + 1) / side),
    );
    let f = |u: f64, v: f64| math::atan2(u * v, (1.0 + u * u + v * v).sqrt());
    f(u1, v1) - f(u0, v1) - f(u1, v0) + f(u0, v0)
}

/// The angle between two unit vectors, by atan2 of their cross and dot products.
#[must_use]
fn arc(a: [f64; 3], b: [f64; 3]) -> Radians {
    let cross = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    let sin = (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt();
    let cos = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    Radians::new(math::atan2(sin, cos))
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyperion_surface::cube::Face;
    use hyperion_testkit::float::assert_same_bits;

    /// The face solid angle, 4π ÷ 6.
    const FACE_SR: f64 = 2.0 * core::f64::consts::PI / 3.0;

    #[test]
    fn solid_angles_sum_to_a_sixth_of_the_sphere_a_face() {
        for level in 0..=8 {
            let graph = CellGraph::new(level).unwrap();
            let per_face = 1_usize << (2 * level);
            for (face, angles) in graph.solid_angles_sr().chunks(per_face).enumerate() {
                let total: f64 = angles.iter().sum();
                assert!(
                    (total - FACE_SR).abs() <= 1e-12,
                    "level {level} face {face}: {total} sr against {FACE_SR}"
                );
            }
        }
    }

    #[test]
    fn every_cell_has_four_distinct_edge_neighbours_and_they_are_symmetric() {
        for level in [0, 1, 3, 6] {
            let graph = CellGraph::new(level).unwrap();
            for n in 0..graph.len() {
                let neighbours = graph.edge_neighbours(n);
                for (k, &m) in neighbours.iter().enumerate() {
                    assert_ne!(m, n, "level {level}: cell {n} neighbours itself");
                    assert!(m < graph.len());
                    assert!(
                        !neighbours[..k].contains(&m),
                        "level {level}: cell {n} names {m} twice"
                    );
                    assert!(
                        graph.edge_neighbours(m).contains(&n),
                        "level {level}: {n} → {m} is not mirrored"
                    );
                }
            }
        }
    }

    #[test]
    fn a_cube_corner_cell_has_three_vertex_neighbours_and_every_other_four() {
        for level in [1, 2, 5] {
            let graph = CellGraph::new(level).unwrap();
            let last = (1_u32 << level) - 1;
            let mut corner_cells = 0;
            for n in 0..graph.len() {
                let cell = graph.cell(n);
                let at_corner = [cell.i(), cell.j()].iter().all(|&c| c == 0 || c == last);
                let count = graph.vertex_neighbours(n).count();
                if at_corner {
                    corner_cells += 1;
                    assert_eq!(count, 3, "level {level}: corner cell {n}");
                } else {
                    assert_eq!(count, 4, "level {level}: cell {n}");
                }
                for m in graph.vertex_neighbours(n) {
                    assert!(
                        graph.vertex_neighbours(m).any(|back| back == n),
                        "level {level}: vertex neighbour {n} → {m} is not mirrored"
                    );
                    assert!(!graph.edge_neighbours(n).contains(&m));
                }
            }
            // Four corners of each of six faces: the cube's eight corners, three cells each.
            assert_eq!(corner_cells, 24, "level {level}");
        }
    }

    #[test]
    fn the_graph_follows_the_field_s_cell_order() {
        let graph = CellGraph::for_field(CoarseLevel::MIN);
        assert_eq!(graph.len(), CoarseLevel::MIN.cell_count());
        for n in [0, 1, 1_023, 1_024, 6_143] {
            assert_eq!(cell_index(graph.cell(n)), n);
            let centre = graph.centre(n);
            let norm =
                (centre[0] * centre[0] + centre[1] * centre[1] + centre[2] * centre[2]).sqrt();
            assert!((norm - 1.0).abs() < 1e-15);
            assert_eq!(cell_index(PatchKey::containing(5, centre).unwrap()), n);
        }
        let climate = CellGraph::for_climate(CoarseLevel::MIN);
        assert_eq!(climate.level(), 4);
        assert_eq!(climate.len(), CoarseLevel::MIN.climate_cell_count());
        for n in [0, 5, 4_097, 6_143] {
            assert_eq!(
                Some(climate.cell(parent_index(n))),
                graph.cell(n).parent(),
                "cell {n}"
            );
        }
        assert_eq!(graph.cell(0).face(), Face::PosX);
        assert_eq!(CellGraph::new(9), Err(BuildCellGraphError { level: 9 }));
    }

    #[test]
    fn the_arc_between_neighbours_is_about_one_cell() {
        let graph = CellGraph::new(4).unwrap();
        // A level-4 cell is 2⁻⁴ of a face, whose quarter-turn spans π ÷ 2: about 0.1 rad.
        for n in [0, 100, 777, 1_535] {
            for m in graph.edge_neighbours(n) {
                let angle = graph.arc(n, m).value();
                assert!((0.06..0.15).contains(&angle), "{n} → {m}: {angle} rad");
                assert_same_bits(angle, graph.arc(m, n).value());
            }
        }
    }
}
