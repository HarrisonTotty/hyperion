//! R09.T12.e's quadrature over the reconstructed field: Gauss–Legendre nodes in each coarse cell's
//! face coordinates, each weighted by the solid angle about it, at which R09.T4's interpolant is
//! read through [`interpolate`] over the pass's own working values (plan R09, Design notes 7 and
//! 13; T4's record, "For T12.e and T14.b").
//!
//! A cell of face f spans s ∈ [i ÷ 2ᴸ, (i + 1) ÷ 2ᴸ] and t ∈ [j ÷ 2ᴸ, (j + 1) ÷ 2ᴸ]. Its n × n
//! nodes are the Gauss–Legendre nodes of each span, mapped to the face's (u, v) by S2's quadratic
//! warp ([`st_to_uv`]) and to a direction by [`face_uv_to_xyz`]. A node's raw weight is its two
//! Gauss weights times the solid angle a unit of s and t covers there, (du ÷ ds)(dv ÷ dt) ÷
//! (1 + u² + v²)^(3/2), the gnomonic face's element through the warp, whose rate du ÷ ds =
//! (8 ÷ 3) max(s, 1 − s) is smooth within every cell of a level of 1 or more; the cell's weights
//! are then scaled to sum to its own solid angle ([`CellGraph::solid_angles_sr`]), so that every
//! area sum is the grid's and a constant field's mean is the constant.
//!
//! Every sum runs in cell-index order, then t's nodes, then s's, in `f64`.

use hyperion_surface::cube::{FaceUv, PatchKey, face_uv_to_xyz, st_to_uv, unit_dir};
use hyperion_surface::field::cell_index;
use hyperion_surface::synth::interp::{CellValues, interpolate};

use crate::planetary::surface::grid::CellGraph;
use crate::planetary::surface::steps::plates::slot;

/// The nodes a side of each cell the pass's quadrature takes, n: 3 (nine a cell), the smallest
/// within 10⁻⁴ of n = 4's reconstructed variance on the reference Moon and Mars (R09.T12.e's
/// convergence check: n = 2 lies 1.4 × 10⁻⁴ short on the Moon and 1.3 × 10⁻⁵ on Mars, n = 3
/// 3.3 × 10⁻⁵ and 3.5 × 10⁻⁶ over; the plan's Risks, "Deviations in T12.e, as built";
/// `the_quadrature_has_converged_at_three_nodes_a_side`). The spline's knots lie at the cells'
/// centres, so each cell spans two of its pieces, which the rule does not split.
pub(crate) const NODES: usize = 3;

/// The Gauss–Legendre nodes and weights of `n` points on [−1, 1], ascending, for n of 2 to 4, in
/// closed form: ±1 ÷ √3 of weight 1; 0 of weight 8 ÷ 9 and ±√(3 ÷ 5) of 5 ÷ 9; ±√(3 ÷ 7 ∓ (2 ÷ 7)
/// √(6 ÷ 5)) of (18 ± √30) ÷ 36 (Abramowitz and Stegun 1964, §25.4.29 and Table 25.4, from
/// memory; the roots of the Legendre polynomials P₂, P₃ and P₄, checked by the tests to 10⁻¹⁵).
///
/// # Panics
///
/// If `n` is not 2, 3 or 4.
#[must_use]
pub(crate) fn gauss_legendre(n: usize) -> Vec<(f64, f64)> {
    match n {
        2 => {
            let x = 1.0 / 3.0_f64.sqrt();
            vec![(-x, 1.0), (x, 1.0)]
        }
        3 => {
            let x = (3.0_f64 / 5.0).sqrt();
            vec![(-x, 5.0 / 9.0), (0.0, 8.0 / 9.0), (x, 5.0 / 9.0)]
        }
        4 => {
            let root = (6.0_f64 / 5.0).sqrt();
            let (inner, outer) = (
                (3.0 / 7.0 - 2.0 / 7.0 * root).sqrt(),
                (3.0 / 7.0 + 2.0 / 7.0 * root).sqrt(),
            );
            let thirty = 30.0_f64.sqrt();
            let (near, far) = ((18.0 + thirty) / 36.0, (18.0 - thirty) / 36.0);
            vec![(-outer, far), (-inner, near), (inner, near), (outer, far)]
        }
        _ => panic!("the pass's quadrature takes 2 to 4 nodes a side, not {n}"),
    }
}

/// The rate du ÷ ds of S2's quadratic warp at the cell coordinate `s`: (8 ÷ 3) s from ½ up and
/// (8 ÷ 3)(1 − s) below ([`st_to_uv`]'s derivative).
#[must_use]
fn warp_rate(s: f64) -> f64 {
    8.0 / 3.0 * if s >= 0.5 { s } else { 1.0 - s }
}

/// Each node of the quadrature over `grid`'s cells, `nodes` a side ([`gauss_legendre`]), handed to
/// `emit` with its cell's index, its unit direction and its weight in steradians, in cell-index
/// order, then t's nodes, then s's (see the module documentation).
pub(crate) fn each_node(
    grid: &CellGraph,
    nodes: &[(f64, f64)],
    mut emit: impl FnMut(u32, [f64; 3], f64),
) {
    let side = f64::from(1_u32 << grid.level());
    let half = 0.5 / side;
    let mut at: Vec<([f64; 3], f64)> = Vec::with_capacity(nodes.len() * nodes.len());
    for (n, (key, &solid)) in (0_u32..).zip(grid.cells().iter().zip(grid.solid_angles_sr())) {
        at.clear();
        let face = key.face();
        let s_mid = (f64::from(key.i()) + 0.5) / side;
        let t_mid = (f64::from(key.j()) + 0.5) / side;
        let mut raw = 0.0;
        for &(y, wy) in nodes {
            let t = t_mid + half * y;
            let v = st_to_uv(t);
            for &(x, wx) in nodes {
                let s = s_mid + half * x;
                let u = st_to_uv(s);
                let rho2 = 1.0 + u * u + v * v;
                let weight = wx * wy * (warp_rate(s) * warp_rate(t) / (rho2 * rho2.sqrt()));
                raw += weight;
                at.push((unit_dir(face_uv_to_xyz(FaceUv { face, u, v })), weight));
            }
        }
        let scale = solid / raw;
        for &(dir, weight) in &at {
            emit(n, dir, weight * scale);
        }
    }
}

/// One value a cell of a grid's level, in cell-index order, as R09.T4's interpolant reads them.
///
/// The interpolant at a direction reads no cell more than
/// [`SYNTHESIS_MARGIN_CELLS`](hyperion_surface::field::SYNTHESIS_MARGIN_CELLS) king moves from the
/// cell that holds it (R09.T4's measure, through edge and corner neighbours, on which a partial
/// field's margin rests), and a field whose cells it reads are all one value gives that value back
/// exactly, made +0 where it is −0 (T4: a constant field interpolates to itself, bit for bit). So
/// at a node of a cell none of whose cells within the margin differs from its own, the value is
/// the cell's, and the interpolant is not read: the pass's sparse fields, a crater's lift or the
/// belts' weight, and its uniform ones cost almost nothing (`the_uniform_cells_are_read_as_the_
/// interpolant_gives_them`).
pub(crate) struct Values<'a> {
    level: u8,
    cells: &'a [f64],
    /// Whether every cell within the margin of each cell has its value.
    uniform: Vec<bool>,
}

impl<'a> Values<'a> {
    /// The cells' `values` on `grid`, one a cell in its order, each asserted finite.
    ///
    /// # Panics
    ///
    /// If a value is not finite (a bug of the step that wrote it), or there is not one a cell.
    #[must_use]
    pub(crate) fn new(grid: &CellGraph, values: &'a [f64]) -> Self {
        assert_eq!(
            values.len(),
            grid.cells().len(),
            "one value a cell of the grid"
        );
        for &v in values {
            // Asserted for its panic: a value that is not finite is a step's fault.
            let _ = hyperion_surface::num::assert_finite(v);
        }
        Self {
            level: grid.level(),
            cells: values,
            uniform: uniform_cells(grid, values),
        }
    }

    /// The interpolated value at the unit direction `dir`, a direction within cell `cell`.
    ///
    /// # Panics
    ///
    /// Never for a node of the quadrature: every cell is held.
    #[must_use]
    pub(crate) fn at(&self, cell: u32, dir: [f64; 3]) -> f64 {
        if self.uniform[slot(cell)] {
            self.cells[slot(cell)] + 0.0
        } else {
            self.interpolated(dir)
        }
    }

    /// The interpolant itself at `dir`, read whatever the cells about it hold.
    ///
    /// # Panics
    ///
    /// Never for a direction of the sphere: every cell is held.
    #[must_use]
    pub(crate) fn interpolated(&self, dir: [f64; 3]) -> f64 {
        interpolate(self, dir)
            .expect("the working state holds every cell")
            .value
    }
}

/// Whether each cell of `grid` has no cell within the synthesis margin whose value in `values`
/// differs from its own, bit for bit.
///
/// A cell within m king moves of `c` differs from it exactly where a cell within m − 1 moves of
/// `c` has a neighbour of another value (along a shortest path, the first change of value lies on
/// such a pair), so the cells that are not uniform are those within m − 1 moves of a cell with a
/// differing neighbour: a multi-source breadth-first search of m − 1 rings from them.
#[must_use]
fn uniform_cells(grid: &CellGraph, values: &[f64]) -> Vec<bool> {
    let margin = hyperion_surface::field::SYNTHESIS_MARGIN_CELLS;
    let neighbours = |c: u32| {
        grid.edge_neighbours(c)
            .into_iter()
            .chain(grid.vertex_neighbours(c))
    };
    let mut uniform = vec![true; values.len()];
    let mut ring: Vec<u32> = (0..grid.len())
        .filter(|&c| {
            let own = values[slot(c)];
            neighbours(c).any(|m| values[slot(m)].total_cmp(&own).is_ne())
        })
        .collect();
    for &c in &ring {
        uniform[slot(c)] = false;
    }
    for _ in 1..margin {
        let mut next = Vec::new();
        for &c in &ring {
            for m in neighbours(c) {
                if uniform[slot(m)] {
                    uniform[slot(m)] = false;
                    next.push(m);
                }
            }
        }
        ring = next;
    }
    uniform
}

impl CellValues for Values<'_> {
    fn level(&self) -> u8 {
        self.level
    }

    fn value(&self, cell: PatchKey) -> Option<f64> {
        self.cells.get(slot(cell_index(cell))).copied()
    }
}

/// The area-weighted means and covariances of `K` values at the nodes, about shifts close to
/// their means (each field's cell mean), so that no second moment cancels: E[(x − c)] and
/// E[(x − c)(y − d)] over the weights, then Cov(x, y) = E[(x − c)(y − d)] − E[x − c] E[y − d].
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Moments<const K: usize> {
    shift: [f64; K],
    weight: f64,
    sum: [f64; K],
    products: [[f64; K]; K],
}

impl<const K: usize> Moments<K> {
    /// No nodes yet, about `shift`.
    #[must_use]
    pub(crate) const fn new(shift: [f64; K]) -> Self {
        Self {
            shift,
            weight: 0.0,
            sum: [0.0; K],
            products: [[0.0; K]; K],
        }
    }

    /// Adds the node of weight `weight` whose values are `x`.
    pub(crate) fn add(&mut self, weight: f64, x: [f64; K]) {
        let d: [f64; K] = core::array::from_fn(|k| x[k] - self.shift[k]);
        self.weight += weight;
        for (i, &di) in d.iter().enumerate() {
            self.sum[i] += weight * di;
            for (j, &dj) in d.iter().enumerate().skip(i) {
                self.products[i][j] += weight * di * dj;
            }
        }
    }

    /// The area mean of value `i`.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn mean(&self, i: usize) -> f64 {
        self.shift[i] + self.sum[i] / self.weight
    }

    /// The area covariance of values `i` and `j`, the variance where they are one.
    #[must_use]
    pub(crate) fn covariance(&self, i: usize, j: usize) -> f64 {
        let (i, j) = if i <= j { (i, j) } else { (j, i) };
        let w = self.weight;
        self.products[i][j] / w - (self.sum[i] / w) * (self.sum[j] / w)
    }
}

/// The area-weighted mean of `values` over `grid`'s cells, by their solid angles in cell-index
/// order.
#[must_use]
pub(crate) fn cell_mean(grid: &CellGraph, values: &[f64]) -> f64 {
    let (mut total, mut sum) = (0.0, 0.0);
    for (&v, &w) in values.iter().zip(grid.solid_angles_sr()) {
        total += w;
        sum += w * v;
    }
    sum / total
}
