//! The drainage (R09.T14.a, Design note 9): the priority flood with pop-order flats, the random
//! receivers with their slope correction, the depth-first order and the accumulation.
//!
//! - **The flood** is Barnes, Lehman and Mulla 2014's improved Priority-Flood (Computers &
//!   Geosciences 62, 117; arXiv:1511.04463; their Algorithm 2), from the seeds the caller gives
//!   (the sea's cells at its level, the pits of terminal lakes at theirs): a priority queue of the
//!   cells opened above the level that reached them, keyed by (level by `total_cmp`, cell index),
//!   lowest first, and a plain FIFO queue of the cells raised to it, always drained first. Each
//!   cell is closed once, by the cell being popped when it is first reached, its *parent*. The
//!   filled surface W is at or above the ground, non-increasing along the parent links, and the
//!   minimax ("lowest saddle") level from the seeds.
//! - **The receivers** are one of the four edge neighbours (Design note 9; Tzathas et al. 2024,
//!   CGF 43(2), doi:10.1111/cgf.15033, §4.3). A cell with neighbours strictly below it on W draws
//!   one of them with probability proportional to the drop per metre of great-circle distance,
//!   from its own uniform on `surface.coarse.erosion` (instance [`RECEIVER_INSTANCE`]); one with
//!   none, a flat or a filled cell, drains to its parent, so that a flat drains along the flood's
//!   breadth-first order towards its spill with no ε-raising (Design note 9's "flat directions
//!   taken from the pop order"; the lane's extension of Algorithm 2, whose Algorithm 4 assigns
//!   directions the same way without filling). A seed has none. A strict drop lowers W and a
//!   parent was popped before its child at the same or a lower W, so the receivers form a forest
//!   rooted at the seeds, and no cell drains uphill after filling.
//! - **The slope correction** is Tzathas et al.'s (§4.3, their page 7): a(x) = k A^m δx ‖∇z‖ ÷
//!   (z(x) − z(`x_r`)), ‖∇z‖ evaluated downstream on each of the face's two axes from the lower of
//!   its two neighbours there. [`Routing::correction`] is `S_r` ÷ ‖∇W‖, so that the solver's 1/a is
//!   it ÷ (k A^m).
//! - **The order** is depth-first from the roots in ascending cell order, each cell's donors in
//!   ascending order, with each cell's depth, so that the solver (R09.T14.b) can walk it with a
//!   path array, `path[depth[c]] = c`, and read a cell's ancestry from `path[..=depth[c]]`. Each
//!   cell's basin is the root its receivers lead to, keyed by the root's cell index, and the
//!   basin's base level is the root's seed level (decision-r09-t14-choices Q1's b and `ℓ_b`).
//! - **The accumulation** ([`accumulate`]) sums weights down the receivers in reverse preorder:
//!   the geometric drainage area (Hack 1957's planimetric area, by solid angle) of the cells'
//!   areas, and the equivalent area (Hergarten 2021, Earth Surf. Dynam. 9, 937, eqs. 16 and 20) of their runoff
//!   times their area.
//!
//! Implemented from the papers alone; no code is copied or paraphrased from Barnes's `RichDEM`,
//! Tzathas et al.'s repository (Inria research-only licence) or Dendry (GPL-3.0).

use std::cmp::Ordering;
use std::collections::{BinaryHeap, VecDeque};

use hyperion_surface::cube::Edge;
use hyperion_surface::field::FlowDirection;
use hyperion_surface::num;
use hyperion_surface::tags::SURFACE_COARSE_EROSION;

use super::super::plates::{cell_of, slot};
use super::mesh::Mesh;
use crate::math;
use crate::planetary::hooks::SurfaceSeed;
use crate::planetary::surface::grid::CellGraph;
use crate::rng::ObjectKey;

/// The instance of each cell's receiver draw on `surface.coarse.erosion`: one uniform a cell a
/// level, keyed by the cell's own face, level, i and j.
pub(crate) const RECEIVER_INSTANCE: u16 = 0;

/// A cell the flood starts from, with its filled level: a root of the receivers' forest.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Seed {
    /// The cell.
    pub(crate) cell: u32,
    /// Its filled level, metres above the datum, at or above its ground: the sea's water surface
    /// for a sea cell, a terminal lake's level for its pit.
    pub(crate) level_m: f64,
}

/// A cell's link to its receiver.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Link {
    /// The receiver.
    pub(crate) cell: u32,
    /// The edge the water leaves by, on the cell's own face's axes.
    pub(crate) edge: Edge,
    /// The link's length, metres of great circle on the body.
    pub(crate) length_m: f64,
}

/// The drainage of one field: its filled surface, receivers, slope corrections, depth-first order
/// and donors.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Routing {
    filled_m: Vec<f64>,
    receivers: Vec<Option<Link>>,
    correction: Vec<f64>,
    roots: Vec<u32>,
    root_of: Vec<u32>,
    preorder: Vec<u32>,
    depth: Vec<u32>,
    donor_start: Vec<u32>,
    donors: Vec<u32>,
}

impl Routing {
    /// The number of cells.
    ///
    /// # Panics
    ///
    /// Never: a coarse level has under 2³² cells.
    #[must_use]
    pub(crate) fn len(&self) -> u32 {
        cell_of(self.filled_m.len())
    }

    /// The filled surface W, metres above the datum, in cell-index order: at or above the ground.
    #[must_use]
    pub(crate) fn filled_m(&self) -> &[f64] {
        &self.filled_m
    }

    /// Cell `cell`'s link to its receiver, or `None` for a root (a seed).
    ///
    /// # Panics
    ///
    /// If `cell` is not below [`len`](Self::len).
    #[must_use]
    pub(crate) fn receiver(&self, cell: u32) -> Option<Link> {
        self.receivers[slot(cell)]
    }

    /// Where cell `cell`'s water goes, as the field codes it.
    ///
    /// # Panics
    ///
    /// If `cell` is not below [`len`](Self::len).
    #[must_use]
    pub(crate) fn flow(&self, cell: u32) -> FlowDirection {
        self.receivers[slot(cell)].map_or(FlowDirection::Terminal, |link| link.edge.into())
    }

    /// Cell `cell`'s slope correction `S_r` ÷ ‖∇W‖, 0 to 1: its drop per metre to its receiver over
    /// the norm of the downstream gradient on its face's two axes (Tzathas et al. 2024, §4.3), so
    /// that the stream-power law's 1/a is it ÷ (k A^m). It is 1 where ‖∇W‖ is 0 (a flat, a filled
    /// cell or a root), and held to 1 where the cube sphere's unequal links would put a receiver's
    /// drop per metre a little above its axis's.
    ///
    /// # Panics
    ///
    /// If `cell` is not below [`len`](Self::len).
    #[must_use]
    pub(crate) fn correction(&self, cell: u32) -> f64 {
        self.correction[slot(cell)]
    }

    /// The roots, the seeds' cells, ascending.
    #[must_use]
    pub(crate) fn roots(&self) -> &[u32] {
        &self.roots
    }

    /// Cell `cell`'s basin: the root its chain of receivers ends at, keyed by that root's cell
    /// index (decision-r09-t14-choices Q1's b).
    ///
    /// # Panics
    ///
    /// If `cell` is not below [`len`](Self::len).
    #[must_use]
    pub(crate) fn root_of(&self, cell: u32) -> u32 {
        self.root_of[slot(cell)]
    }

    /// The base level of cell `cell`'s basin, metres above the datum: its root's seed level, the
    /// sea's level for a basin draining to the sea, a terminal lake's level, or a dry playa's
    /// floor (decision-r09-t14-choices Q1's `ℓ_b`).
    ///
    /// # Panics
    ///
    /// If `cell` is not below [`len`](Self::len).
    #[must_use]
    pub(crate) fn base_level_m(&self, cell: u32) -> f64 {
        self.filled_m[slot(self.root_of[slot(cell)])]
    }

    /// Every cell, depth first from the roots in ascending order, each cell's donors in ascending
    /// order: each cell comes after its receiver.
    #[must_use]
    pub(crate) fn preorder(&self) -> &[u32] {
        &self.preorder
    }

    /// Cell `cell`'s depth in its tree: 0 at a root, its receiver's plus 1 elsewhere.
    ///
    /// # Panics
    ///
    /// If `cell` is not below [`len`](Self::len).
    #[must_use]
    pub(crate) fn depth(&self, cell: u32) -> u32 {
        self.depth[slot(cell)]
    }

    /// The cells whose receiver is cell `cell`, ascending.
    ///
    /// # Panics
    ///
    /// If `cell` is not below [`len`](Self::len).
    #[must_use]
    pub(crate) fn donors(&self, cell: u32) -> &[u32] {
        let c = slot(cell);
        &self.donors[slot(self.donor_start[c])..slot(self.donor_start[c + 1])]
    }
}

/// An opened cell of the flood: its filled level and its index, ordered so that the heap pops the
/// lowest level first, then the least index (the sea's own order, `relief::sea`'s `Entry`).
#[derive(Debug, Clone, Copy)]
struct Entry {
    /// The filled level, metres.
    level_m: f64,
    cell: u32,
}

impl Ord for Entry {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .level_m
            .total_cmp(&self.level_m)
            .then_with(|| other.cell.cmp(&self.cell))
    }
}

impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

impl Eq for Entry {}

/// `x` once asserted finite, with a negative zero made positive, so that `total_cmp` orders the
/// field as `<` does and every strict drop is above zero.
#[must_use]
fn canonical(x: f64) -> f64 {
    num::assert_finite(x) + 0.0
}

/// One uniform in [0, 1) per cell of `graph`, in cell-index order, each from its own stream on
/// `surface.coarse.erosion`, keyed by its `surface_cell` (face, level, i, j) with instance
/// [`RECEIVER_INSTANCE`]: the same seed gives the same receivers.
///
/// # Panics
///
/// Never: every cell of a coarse level is a surface cell.
#[must_use]
pub(crate) fn receiver_uniforms(seed: SurfaceSeed, graph: &CellGraph) -> Vec<f64> {
    graph
        .cells()
        .iter()
        .map(|cell| {
            seed.stream(
                SURFACE_COARSE_EROSION,
                ObjectKey::surface_cell(
                    cell.face().index(),
                    cell.level(),
                    cell.i(),
                    cell.j(),
                    RECEIVER_INSTANCE,
                )
                .expect("a coarse cell is a surface cell"),
            )
            .uniform()
        })
        .collect()
}

/// The drainage of `mesh`'s ground `z` (metres, in cell-index order) from `seeds`, each cell's
/// receiver drawn with its uniform in `uniforms` (see the module documentation).
///
/// # Panics
///
/// If `z` or `uniforms` is not one a cell, a height or a seed's level is not finite, a seed's
/// level is below its ground, or the seeds are empty or do not ascend strictly by cell.
#[must_use]
pub(crate) fn route(mesh: &Mesh, z: &[f64], seeds: &[Seed], uniforms: &[f64]) -> Routing {
    let n = slot(mesh.len());
    assert_eq!(z.len(), n, "one height a cell");
    assert_eq!(uniforms.len(), n, "one uniform a cell");
    assert!(!seeds.is_empty(), "a flood starts from at least one seed");
    assert!(
        seeds.windows(2).all(|w| w[0].cell < w[1].cell),
        "the seeds ascend strictly by cell"
    );
    let ground: Vec<f64> = z.iter().map(|&h| canonical(h)).collect();
    let (filled, parent, is_seed) = flood(mesh, &ground, seeds);
    let mut receivers = Vec::with_capacity(n);
    let mut correction = vec![1.0; n];
    for (c, &seed) in is_seed.iter().enumerate() {
        if seed {
            receivers.push(None);
            continue;
        }
        let cell = cell_of(c);
        let k = receiver_edge(mesh, &filled, cell, uniforms[c], parent[c]);
        let neighbours = mesh.neighbours(cell);
        let length_m = mesh.link_m(cell, k);
        let gradient = math::hypot(
            axis_slope(mesh, &filled, cell, [0, 1]),
            axis_slope(mesh, &filled, cell, [2, 3]),
        );
        if gradient > 0.0 {
            let drop = (filled[c] - filled[slot(neighbours[k])]) / length_m;
            correction[c] = num::min(1.0, drop / gradient);
        }
        receivers.push(Some(Link {
            cell: neighbours[k],
            edge: Edge::ALL[k],
            length_m,
        }));
    }
    let (donor_start, donors) = donor_lists(&receivers);
    let roots: Vec<u32> = seeds.iter().map(|s| s.cell).collect();
    let (preorder, depth) = depth_first(&roots, &donor_start, &donors, n);
    let mut root_of = vec![0; n];
    for &c in &preorder {
        root_of[slot(c)] = receivers[slot(c)].map_or(c, |link: Link| root_of[slot(link.cell)]);
    }
    Routing {
        filled_m: filled,
        receivers,
        correction,
        roots,
        root_of,
        preorder,
        depth,
        donor_start,
        donors,
    }
}

/// Barnes, Lehman and Mulla 2014's Algorithm 2 on `ground` from `seeds`: each cell's filled level,
/// the cell that closed it (`None` for a seed), and whether it is a seed.
///
/// # Panics
///
/// If a seed's level is not finite or is below its ground, or a cell is never reached, which a
/// connected graph rules out.
fn flood(mesh: &Mesh, ground: &[f64], seeds: &[Seed]) -> (Vec<f64>, Vec<Option<u32>>, Vec<bool>) {
    let n = ground.len();
    let mut filled = vec![0.0; n];
    let mut closed = vec![false; n];
    let mut parent = vec![None; n];
    let mut open = BinaryHeap::with_capacity(seeds.len());
    let mut pit = VecDeque::new();
    for seed in seeds {
        let c = slot(seed.cell);
        let level_m = canonical(seed.level_m);
        assert!(
            level_m >= ground[c],
            "seed {}'s level {level_m} m is below its ground {} m",
            seed.cell,
            ground[c]
        );
        closed[c] = true;
        filled[c] = level_m;
        open.push(Entry {
            level_m,
            cell: seed.cell,
        });
    }
    let is_seed = closed.clone();
    let mut reached = seeds.len();
    while let Some(c) = pit.pop_front().or_else(|| open.pop().map(|e| e.cell)) {
        let level_m = filled[slot(c)];
        for m in mesh.neighbours(c) {
            let mi = slot(m);
            if closed[mi] {
                continue;
            }
            closed[mi] = true;
            parent[mi] = Some(c);
            reached += 1;
            if ground[mi].total_cmp(&level_m).is_le() {
                filled[mi] = level_m;
                pit.push_back(m);
            } else {
                filled[mi] = ground[mi];
                open.push(Entry {
                    level_m: ground[mi],
                    cell: m,
                });
            }
        }
    }
    assert_eq!(
        reached, n,
        "the flood reaches every cell of a connected graph"
    );
    (filled, parent, is_seed)
}

/// The edge (0–3) of the receiver of `cell`, not a seed, on the filled surface `filled`, with its
/// uniform `u`: a neighbour strictly below it drawn with probability proportional to the drop per
/// metre, in edge order, the first whose running sum of weights exceeds u times their total (the
/// last with a weight above zero, should rounding leave none); or, with no weight above zero (a
/// flat or a filled cell), its parent's.
///
/// # Panics
///
/// If `cell` has no weight above zero and its parent is not one of its edge neighbours, which the
/// flood rules out.
fn receiver_edge(mesh: &Mesh, filled: &[f64], cell: u32, u: f64, parent: Option<u32>) -> usize {
    let level_m = filled[slot(cell)];
    let neighbours = mesh.neighbours(cell);
    let mut weights = [0.0; 4];
    let mut total = 0.0;
    for (k, (&m, weight)) in neighbours.iter().zip(&mut weights).enumerate() {
        let below = filled[slot(m)];
        if below.total_cmp(&level_m).is_lt() {
            *weight = (level_m - below) / mesh.link_m(cell, k);
            total += *weight;
        }
    }
    if total > 0.0 {
        let target = u * total;
        let (mut running, mut last) = (0.0, None);
        for (k, &weight) in weights.iter().enumerate() {
            if weight > 0.0 {
                running += weight;
                last = Some(k);
                if running > target {
                    return k;
                }
            }
        }
        return last.expect("a total above zero has a weight above zero");
    }
    let parent = parent.expect("a cell that is not a seed was closed by a neighbour");
    neighbours
        .iter()
        .position(|&m| m == parent)
        .expect("a cell's parent is one of its edge neighbours")
}

/// The downstream slope of `cell` on `filled` along one of its face's axes, the axis's two edges
/// `axis` (0 and 1 for u, 2 and 3 for v): its drop to the lower of the two (by `total_cmp`, ties to
/// the lower edge) per metre of that link, or 0 if neither is below it.
fn axis_slope(mesh: &Mesh, filled: &[f64], cell: u32, axis: [usize; 2]) -> f64 {
    let neighbours = mesh.neighbours(cell);
    let [low, high] = axis;
    let k = if filled[slot(neighbours[high])]
        .total_cmp(&filled[slot(neighbours[low])])
        .is_lt()
    {
        high
    } else {
        low
    };
    num::max(0.0, filled[slot(cell)] - filled[slot(neighbours[k])]) / mesh.link_m(cell, k)
}

/// Each cell's donors as compressed rows: `donors[start[c]..start[c + 1]]` are the cells whose
/// receiver is c, ascending, by a counting sort over `receivers` in cell-index order.
fn donor_lists(receivers: &[Option<Link>]) -> (Vec<u32>, Vec<u32>) {
    let mut start = vec![0_u32; receivers.len() + 1];
    for link in receivers.iter().flatten() {
        start[slot(link.cell) + 1] += 1;
    }
    for c in 0..receivers.len() {
        start[c + 1] += start[c];
    }
    let mut next = start.clone();
    let mut donors = vec![0; slot(start[receivers.len()])];
    for (c, link) in receivers.iter().enumerate() {
        if let Some(link) = link {
            let at = &mut next[slot(link.cell)];
            donors[slot(*at)] = cell_of(c);
            *at += 1;
        }
    }
    (start, donors)
}

/// The depth-first preorder from `roots`, ascending, each cell's donors visited in ascending
/// order, by an explicit stack, and each cell's depth.
///
/// # Panics
///
/// If the order misses a cell: the receivers are not a forest over the roots.
fn depth_first(roots: &[u32], start: &[u32], donors: &[u32], n: usize) -> (Vec<u32>, Vec<u32>) {
    let mut preorder = Vec::with_capacity(n);
    let mut depth = vec![0_u32; n];
    let mut stack = Vec::new();
    for &root in roots {
        stack.push(root);
        while let Some(c) = stack.pop() {
            preorder.push(c);
            let ci = slot(c);
            for &d in donors[slot(start[ci])..slot(start[ci + 1])].iter().rev() {
                depth[slot(d)] = depth[ci] + 1;
                stack.push(d);
            }
        }
    }
    assert_eq!(
        preorder.len(),
        n,
        "the receivers are a forest over the roots"
    );
    (preorder, depth)
}

/// `weights` summed down the receivers of `routing`: each cell's own plus every cell upstream of
/// it, in reverse preorder, which is the summation order. The geometric drainage area is that of
/// the cells' areas; the equivalent area (Hergarten 2021, eq. 20) that of runoff times area,
/// divided by the reference runoff, 1 m a⁻¹.
///
/// # Panics
///
/// If `weights` is not one a cell.
#[must_use]
pub(crate) fn accumulate(routing: &Routing, weights: &[f64]) -> Vec<f64> {
    assert_eq!(weights.len(), routing.receivers.len(), "one weight a cell");
    let mut acc = weights.to_vec();
    for &c in routing.preorder.iter().rev() {
        if let Some(link) = routing.receivers[slot(c)] {
            acc[slot(link.cell)] += acc[slot(c)];
        }
    }
    acc
}
