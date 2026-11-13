//! The erosion's mesh: one quadtree level's cells as the drainage and the solver read them, with
//! each cell's area, its four edge neighbours and the length of each link, on the body (R09.T14.a,
//! Design note 9).
//!
//! A cell's area is its solid angle times R², R the body's radius, every step's convention; a
//! link's length is the great-circle angle between the two centres ([`CellGraph::arc`]) times R.
//! The neighbours are [`CellGraph::edge_neighbours`], in [`Edge::ALL`]'s order (u = 0, u = 1,
//! v = 0, v = 1), which is the order of [`FlowDirection`]'s codes 1–4, so that link k of a cell is
//! its k-th edge. The multigrid (R09.T14.b) builds one mesh a level.
//!
//! [`Edge::ALL`]: hyperion_surface::cube::Edge::ALL
//! [`FlowDirection`]: hyperion_surface::field::FlowDirection

use super::super::plates::slot;
use crate::planetary::surface::grid::CellGraph;
use crate::units::Metres;

/// One level's cells, in cell-index order, with their areas, edge neighbours and link lengths.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Mesh {
    /// The quadtree level, which only the erosion's tests read.
    #[cfg(test)]
    level: u8,
    area_m2: Vec<f64>,
    neighbours: Vec<[u32; 4]>,
    link_m: Vec<[f64; 4]>,
}

impl Mesh {
    /// The mesh of `graph`'s cells on a body of radius `radius`.
    ///
    /// # Panics
    ///
    /// If `radius` is not positive and finite.
    #[must_use]
    pub(crate) fn of(graph: &CellGraph, radius: Metres) -> Self {
        let r = radius.value();
        assert!(
            r.is_finite() && r > 0.0,
            "a body's radius is positive and finite, not {r} m"
        );
        let r2 = r * r;
        let area_m2 = graph.solid_angles_sr().iter().map(|&w| w * r2).collect();
        let neighbours: Vec<[u32; 4]> =
            (0..graph.len()).map(|c| graph.edge_neighbours(c)).collect();
        let link_m = neighbours
            .iter()
            .zip(0..graph.len())
            .map(|(edges, c)| edges.map(|n| graph.arc(c, n).value() * r))
            .collect();
        Self {
            #[cfg(test)]
            level: graph.level(),
            area_m2,
            neighbours,
            link_m,
        }
    }

    /// The quadtree level.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn level(&self) -> u8 {
        self.level
    }

    /// The number of cells, 6 · 4ᴸ.
    ///
    /// # Panics
    ///
    /// Never: a coarse level has under 2³² cells.
    #[must_use]
    pub(crate) fn len(&self) -> u32 {
        u32::try_from(self.area_m2.len()).expect("a coarse level has under 2^32 cells")
    }

    /// The cells' areas, square metres, in cell-index order.
    #[must_use]
    pub(crate) fn areas_m2(&self) -> &[f64] {
        &self.area_m2
    }

    /// The area of cell `cell`, square metres: its solid angle times R².
    ///
    /// # Panics
    ///
    /// If `cell` is not below [`len`](Self::len).
    #[must_use]
    pub(crate) fn area_m2(&self, cell: u32) -> f64 {
        self.area_m2[slot(cell)]
    }

    /// The four edge neighbours of cell `cell`, in [`Edge::ALL`]'s order.
    ///
    /// # Panics
    ///
    /// If `cell` is not below [`len`](Self::len).
    ///
    /// [`Edge::ALL`]: hyperion_surface::cube::Edge::ALL
    #[must_use]
    pub(crate) fn neighbours(&self, cell: u32) -> [u32; 4] {
        self.neighbours[slot(cell)]
    }

    /// The length of cell `cell`'s link `k` (0–3, in [`Edge::ALL`]'s order) to its k-th edge
    /// neighbour, metres of great circle on the body.
    ///
    /// # Panics
    ///
    /// If `cell` is not below [`len`](Self::len) or `k` is above 3.
    ///
    /// [`Edge::ALL`]: hyperion_surface::cube::Edge::ALL
    #[must_use]
    pub(crate) fn link_m(&self, cell: u32, k: usize) -> f64 {
        self.link_m[slot(cell)][k]
    }
}
