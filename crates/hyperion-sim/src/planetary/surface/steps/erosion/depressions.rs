//! The depressions (R09.T14.a, Design note 9): the depression hierarchy, Fill–Spill–Merge, the
//! water bodies, and the routing with water.
//!
//! - **The hierarchy** is Barnes, Callaghan and Wickert 2020's depression hierarchy (Earth Surf. Dynam. 8,
//!   431), built here as the terrain's merge tree, the lane's equivalent construction: the sea's
//!   cells are taken first, as one component, the ocean, whatever their height; then the land's,
//!   sorted by (ground by `total_cmp`, cell index), each asserted finite, with a union-find. A cell
//!   none of whose edge neighbours is taken is a pit and starts a leaf depression; one whose taken
//!   neighbours lie in one component joins it; one whose taken neighbours lie in two or more is
//!   their outlet. There the land components merge pairwise in ascending order of their top
//!   node's index, each pair under a new node, both spilling at the outlet's ground; and if the
//!   ocean is among them, the merged one is linked to the sea at the outlet, no new node, a tree
//!   of the forest (the paper's oceanlink). On a world with no sea the last component is the
//!   root, with no outlet and no bound. Each node keeps its pit (the lowest cell beneath it), the
//!   level it starts from (its pit's ground, or the outlet where its children met), its spill, and
//!   the area of its cells and Σ a (z − start) over its own, so that a node's volume to a level h
//!   is V(h) = h Σa − Σa z over the cells below h (Barnes, Callaghan and Wickert 2021, Earth Surf. Dynam. 9,
//!   105, eq. 3, with each cell's own area), heights held relative to the start for precision.
//! - **Where water goes.** Each land cell's descent pointer is its lowest taken neighbour in the
//!   walk's order: no order of the walk has flats, so a cell with none is exactly a pit, and the
//!   pointers end at a pit or the sea ("flow follows the path of steepest descent", Barnes et al.
//!   2020 §3.3; here the lowest, not the steepest by drop per metre). At each merge, each side's
//!   *geolink* is the leaf that water spilling from the other enters: the leaf the outlet's lowest
//!   neighbour on that side descends to. A tree linked to the sea spills into the leaf (or the sea)
//!   that its outlet's lowest neighbour on the ocean's side descends to, a tree linked to the sea
//!   before it: a high basin spills into the basin below it, not straight to the sea.
//! - **Fill–Spill–Merge** (Barnes et al. 2021, §3): each cell's water is passed down its pointers
//!   in the reverse of the walk's order (§3.1), which fixes the summation order, into its pit's
//!   leaf or the sea. Then (§3.2) the trees are resolved, the last linked first, each tree's nodes
//!   children first: a leaf holds what reached it up to its capacity; a node whose child overflows
//!   pours the excess into its sibling at the child's geolink, where it fills that leaf and climbs
//!   through the sibling's subtree, each full node's water passing to its sibling's geolink while
//!   the sibling has room, else to their parent, never past the merge node (the paper's
//!   `OverflowInto` with its stop node, and its jump table, so that the whole is near-linear);
//!   when both children are full, with or without excess, their parent holds both (two exactly
//!   full children are one closed lake at their sill), and the excess above its own capacity
//!   passes on. A tree's root overflows along its link to the sea, into another tree's leaf or the
//!   sea (counted as discharged); the root of a world with no sea holds all it gets. The input is
//!   the water held plus the water discharged, to rounding.
//! - **The water bodies** are the highest nodes water reached at their own level (a leaf, or a
//!   merge whose children both filled) beneath no other, each node beneath one being the body's
//!   whatever its own state (a merge of two flats at their own spill, say). A full body is
//!   at its spill level and spills (through-flow); one below it is terminal, a lake at its level,
//!   or, where it floods no cell, a dry playa at its floor's ground: a closed basin whose base
//!   level is its lake or its floor (decision-r09-t14-choices Q1 and Q3). A full body that floods
//!   no cell (a flat at its own spill) is left out. "Under water" is read on the field's
//!   millimetre codes, as the sea's is (`relief::sea`): a cell is flooded where its ground's code
//!   is below the level's, so a full body's sill and a shore a fraction of a millimetre below its
//!   level stay dry.
//! - **The amount** is a [`Measure`], which carries each cell's input:
//!   - [`Measure::Volume`], cubic metres (a paleo-inventory, R09.T14.c; decision-r09-t14-choices
//!     Q2): a depression's capacity is its volume to its spill, and a terminal lake's level solves
//!     V(h) = its water (eq. 6, h = (`V_w` + Σa z) ÷ Σa) over its subtree's cells taken in the
//!     walk's order, which is a priority flood's from its pit within the subtree (§3.3's
//!     `FillDepressions`): every cell of the subtree below the level joins the lake through cells
//!     below it, by its descent pointers. The printed pseudocode's two errata are fixed (the
//!     comparison is ≤, not <; a depth is h − z).
//!   - [`Measure::Balance`], cubic metres a year (lakes on a world wet now, R09.T14.d;
//!     decision-r09-t14-choices Q3): the inflow is the catchment's runoff with what upstream
//!     bodies spill, and a depression's capacity is its loss at its sill, Σ (E − P) a over the
//!     cells below it, which may be negative where precipitation exceeds evaporation, so that a
//!     full body spills its inflow less that loss, more than its inflow there; a terminal lake
//!     takes its subtree's cells whole in their fill order, each while half its loss fits within
//!     the inflow beside the cells taken (`climate::seas`' half-cell rule).
//! - **The routing with water** ([`route_with_water`]), on the current ground with the water
//!   bodies found on the construction's ([`water_bodies`]): the drainage from every sea cell at
//!   the sea's level and every terminal body's pit at its level, a lake's or a playa's floor (on a
//!   world with neither, the lowest cell at its ground). The flood fills a through-flow body to
//!   its spill, so that it drains towards its outlet, and a terminal lake to its level, draining to
//!   its pit, a root; a playa's catchment drains to its floor.
//!
//! Implemented from the papers alone; no code is copied or paraphrased from Barnes's `RichDEM` or
//! the papers' published sources.

use hyperion_surface::num;

use super::super::plates::{cell_of, slot};
use super::super::relief::sea::code;
use super::drainage::{self, Routing, Seed};
use super::mesh::Mesh;

/// What the water is, with each cell's input in cell-index order: what a depression's capacity
/// and a lake's level are measured in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Measure<'a> {
    /// A volume of liquid (decision-r09-t14-choices Q2): a depression's capacity is its volume to
    /// its spill, V(h) = h Σa − Σa z over the cells below h, and a terminal lake's level solves
    /// V(h) = its water (Barnes, Callaghan and Wickert 2021, eqs. 3 and 6).
    Volume {
        /// Each cell's water, cubic metres, finite and non-negative.
        water_m3: &'a [f64],
    },
    /// A water balance (decision-r09-t14-choices Q3): a depression's capacity is its loss at its
    /// sill, and a terminal lake takes whole cells in fill order by the half-cell rule.
    Balance {
        /// Each cell's own inflow, its runoff times its area, cubic metres a year, finite and
        /// non-negative.
        inflow_m3_per_year: &'a [f64],
        /// Each cell's net loss rate over open liquid, its evaporation less its precipitation,
        /// metres a year, finite and of either sign.
        net_loss_m_per_year: &'a [f64],
    },
}

impl Measure<'_> {
    /// Each cell's amount, which runs down the descent pointers to the pits: its water, or its
    /// inflow.
    ///
    /// # Panics
    ///
    /// If an input is not one finite value a cell of `n`, or an amount is negative.
    fn amounts(&self, n: usize) -> &[f64] {
        let amounts = match *self {
            Self::Volume { water_m3 } => water_m3,
            Self::Balance {
                inflow_m3_per_year,
                net_loss_m_per_year,
            } => {
                assert_eq!(net_loss_m_per_year.len(), n, "one net loss rate a cell");
                assert!(
                    net_loss_m_per_year.iter().all(|e| e.is_finite()),
                    "each cell's net loss rate is finite"
                );
                inflow_m3_per_year
            }
        };
        assert_eq!(amounts.len(), n, "one amount of water a cell");
        assert!(
            amounts.iter().all(|w| w.is_finite() && *w >= 0.0),
            "each cell's water is finite and non-negative"
        );
        amounts
    }
}

/// Where a depression spills: its outlet cell and the outlet's ground.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Spill {
    /// The outlet (saddle) cell.
    pub(crate) outlet: u32,
    /// The spill level, the outlet's ground, metres above the datum.
    pub(crate) level_m: f64,
}

/// Where a depression's excess goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Overflow {
    /// Nowhere: the root of a world with no sea (and every node until it merges).
    Nowhere,
    /// A child's: into its sibling's subtree at this leaf, its geolink.
    Sibling(u32),
    /// A tree's root, linked to the sea: into this leaf of a tree linked before it, or the sea.
    Sea(Option<u32>),
}

/// A node of the hierarchy: a leaf depression or the merge of two.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Node {
    /// The lowest cell beneath the node, in the walk's order.
    pit: u32,
    /// The level the node's own cells start from, metres: its pit's ground for a leaf, the
    /// outlet where its children met for a merge.
    start_m: f64,
    /// Where it spills, once it merges or is linked to the sea; `None` for the root of a world
    /// with no sea.
    spill: Option<Spill>,
    /// The two depressions it merges, the lower node index first; `None` for a leaf.
    children: Option<[u32; 2]>,
    /// The node it merged into; `None` for a tree's root.
    parent: Option<u32>,
    /// Where its excess goes.
    overflow: Overflow,
    /// The area of every cell beneath it, square metres.
    area_m2: f64,
    /// Σ a (z − start) over its own cells (those not beneath a child), cubic metres.
    own_m3: f64,
}

impl Node {
    /// The lowest cell beneath the node.
    #[cfg_attr(not(test), expect(dead_code, reason = "the erosion's tests read it"))]
    #[must_use]
    pub(crate) const fn pit(&self) -> u32 {
        self.pit
    }

    /// Where it spills; `None` for the root of a world with no sea.
    #[cfg_attr(not(test), expect(dead_code, reason = "the erosion's tests read it"))]
    #[must_use]
    pub(crate) const fn spill(&self) -> Option<Spill> {
        self.spill
    }

    /// The node it merged into; `None` for a tree's root.
    #[cfg_attr(not(test), expect(dead_code, reason = "the erosion's tests read it"))]
    #[must_use]
    pub(crate) const fn parent(&self) -> Option<u32> {
        self.parent
    }
}

/// A component of the walk: the ocean, or the land below a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Top {
    Ocean,
    Node(u32),
}

/// One component a cell of the walk touches: its union-find representative, its top, and the
/// cell's lowest neighbour in it.
#[derive(Debug, Clone, Copy)]
struct Side {
    rep: u32,
    top: Top,
    lowest: u32,
}

/// The depression hierarchy of one field (see the module documentation).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Hierarchy {
    ground_m: Vec<f64>,
    /// The walk's order: the sea's cells ascending, then the land's by (ground, index).
    order: Vec<u32>,
    nodes: Vec<Node>,
    /// The node each land cell is an own cell of; `None` for the ocean's component.
    holder: Vec<Option<u32>>,
    /// Each land cell's lowest neighbour before it in the walk; `None` at a pit or in the sea.
    descent: Vec<Option<u32>>,
    /// The leaf each cell's water reaches down its pointers; `None` for the sea.
    leaf_of: Vec<Option<u32>>,
    /// The trees' roots in the order they were linked to the sea; on a world with no sea, the
    /// one root.
    trees: Vec<u32>,
}

/// The walk that builds a [`Hierarchy`].
struct Walk<'a> {
    mesh: &'a Mesh,
    ground: &'a [f64],
    rank: Vec<u32>,
    uf: Vec<u32>,
    top: Vec<Top>,
    taken: Vec<bool>,
    ocean: Option<u32>,
    nodes: Vec<Node>,
    holder: Vec<Option<u32>>,
    descent: Vec<Option<u32>>,
    leaf_of: Vec<Option<u32>>,
    trees: Vec<u32>,
}

impl Walk<'_> {
    /// The representative of `x`'s component, halving the path.
    fn find(&mut self, mut x: u32) -> u32 {
        while self.uf[slot(x)] != x {
            let next = self.uf[slot(self.uf[slot(x)])];
            self.uf[slot(x)] = next;
            x = next;
        }
        x
    }

    /// Takes sea cell `c` into the ocean.
    fn take_sea(&mut self, c: u32) {
        self.taken[slot(c)] = true;
        match self.ocean {
            None => {
                self.ocean = Some(c);
                self.top[slot(c)] = Top::Ocean;
            }
            Some(rep) => self.uf[slot(c)] = rep,
        }
    }

    /// Takes land cell `c`.
    fn take_land(&mut self, c: u32) {
        let ci = slot(c);
        self.taken[ci] = true;
        let mut prior: Vec<u32> = self
            .mesh
            .neighbours(c)
            .into_iter()
            .filter(|&m| self.taken[slot(m)])
            .collect();
        prior.sort_unstable_by_key(|&m| self.rank[slot(m)]);
        let Some(&lowest) = prior.first() else {
            let k = cell_of(self.nodes.len());
            self.nodes.push(Node {
                pit: c,
                start_m: self.ground[ci],
                spill: None,
                children: None,
                parent: None,
                overflow: Overflow::Nowhere,
                area_m2: self.mesh.area_m2(c),
                own_m3: 0.0,
            });
            self.top[ci] = Top::Node(k);
            self.holder[ci] = Some(k);
            self.leaf_of[ci] = Some(k);
            return;
        };
        self.descent[ci] = Some(lowest);
        self.leaf_of[ci] = self.leaf_of[slot(lowest)];
        let mut sides: Vec<Side> = Vec::with_capacity(prior.len());
        for &m in &prior {
            let rep = self.find(m);
            if !sides.iter().any(|s| s.rep == rep) {
                sides.push(Side {
                    rep,
                    top: self.top[slot(rep)],
                    lowest: m,
                });
            }
        }
        if let [side] = sides[..] {
            self.uf[ci] = side.rep;
            match side.top {
                Top::Ocean => self.holder[ci] = None,
                Top::Node(k) => self.own(k, c),
            }
        } else {
            self.meet(c, &sides);
        }
    }

    /// Adds cell `c` to node `k`'s own cells.
    fn own(&mut self, k: u32, c: u32) {
        let a = self.mesh.area_m2(c);
        let node = &mut self.nodes[slot(k)];
        node.area_m2 += a;
        node.own_m3 += a * (self.ground[slot(c)] - node.start_m);
        self.holder[slot(c)] = Some(k);
    }

    /// Cell `c` is the outlet of the two or more components `sides`: their land merges pairwise,
    /// and joins the ocean if it is among them.
    fn meet(&mut self, c: u32, sides: &[Side]) {
        let mut land: Vec<(u32, u32)> = sides
            .iter()
            .filter_map(|s| match s.top {
                Top::Node(k) => Some((k, s.lowest)),
                Top::Ocean => None,
            })
            .collect();
        land.sort_unstable_by_key(|&(k, _)| k);
        let ocean = sides.iter().find(|s| s.top == Top::Ocean);
        let (mut merged, mut merged_lowest) = *land
            .first()
            .expect("two or more components hold at most one ocean");
        for &(next, next_lowest) in &land[1..] {
            merged = self.merge(merged, merged_lowest, next, next_lowest, c);
            if self.rank[slot(next_lowest)] < self.rank[slot(merged_lowest)] {
                merged_lowest = next_lowest;
            }
        }
        let (root, top) = if let Some(ocean) = ocean {
            let node = &mut self.nodes[slot(merged)];
            node.spill = Some(Spill {
                outlet: c,
                level_m: self.ground[slot(c)],
            });
            node.overflow = Overflow::Sea(self.leaf_of[slot(ocean.lowest)]);
            self.trees.push(merged);
            self.holder[slot(c)] = None;
            (ocean.rep, Top::Ocean)
        } else {
            self.own(merged, c);
            (sides[0].rep, Top::Node(merged))
        };
        for side in sides {
            self.uf[slot(side.rep)] = root;
        }
        self.uf[slot(c)] = root;
        self.top[slot(root)] = top;
    }

    /// Merges nodes `a` and `b`, spilling into each other at outlet `c`, whose lowest neighbours
    /// on their sides are `a_lowest` and `b_lowest`, under a new node, which it returns.
    fn merge(&mut self, a: u32, a_lowest: u32, b: u32, b_lowest: u32, c: u32) -> u32 {
        let spill = Some(Spill {
            outlet: c,
            level_m: self.ground[slot(c)],
        });
        let into_b = self.leaf_of[slot(b_lowest)].expect("a land component's cells reach a leaf");
        let into_a = self.leaf_of[slot(a_lowest)].expect("a land component's cells reach a leaf");
        let m = cell_of(self.nodes.len());
        let (pit_a, pit_b) = (self.nodes[slot(a)].pit, self.nodes[slot(b)].pit);
        let pit = if self.rank[slot(pit_a)] < self.rank[slot(pit_b)] {
            pit_a
        } else {
            pit_b
        };
        let area_m2 = self.nodes[slot(a)].area_m2 + self.nodes[slot(b)].area_m2;
        for (k, into) in [(a, into_b), (b, into_a)] {
            let node = &mut self.nodes[slot(k)];
            node.spill = spill;
            node.overflow = Overflow::Sibling(into);
            node.parent = Some(m);
        }
        self.nodes.push(Node {
            pit,
            start_m: self.ground[slot(c)],
            spill: None,
            children: Some([a, b]),
            parent: None,
            overflow: Overflow::Nowhere,
            area_m2,
            own_m3: 0.0,
        });
        m
    }
}

impl Hierarchy {
    /// The hierarchy of `mesh`'s ground `z` (metres, in cell-index order), with the cells `sea`
    /// marks as the ocean (see the module documentation).
    ///
    /// # Panics
    ///
    /// If `z` or `sea` is not one a cell, or a height is not finite.
    #[must_use]
    pub(crate) fn build(mesh: &Mesh, z: &[f64], sea: &[bool]) -> Self {
        let n = slot(mesh.len());
        assert_eq!(z.len(), n, "one height a cell");
        assert_eq!(sea.len(), n, "one sea flag a cell");
        let ground: Vec<f64> = z.iter().map(|&h| num::assert_finite(h) + 0.0).collect();
        let mut land: Vec<u32> = (0..mesh.len()).filter(|&c| !sea[slot(c)]).collect();
        land.sort_unstable_by(|&a, &b| ground[slot(a)].total_cmp(&ground[slot(b)]).then(a.cmp(&b)));
        let order: Vec<u32> = (0..mesh.len())
            .filter(|&c| sea[slot(c)])
            .chain(land)
            .collect();
        let mut rank = vec![0; n];
        for (r, &c) in order.iter().enumerate() {
            rank[slot(c)] = cell_of(r);
        }
        let mut walk = Walk {
            mesh,
            ground: &ground,
            rank,
            uf: (0..mesh.len()).collect(),
            top: vec![Top::Ocean; n],
            taken: vec![false; n],
            ocean: None,
            nodes: Vec::new(),
            holder: vec![None; n],
            descent: vec![None; n],
            leaf_of: vec![None; n],
            trees: Vec::new(),
        };
        for &c in &order {
            if sea[slot(c)] {
                walk.take_sea(c);
            } else {
                walk.take_land(c);
            }
        }
        if walk.ocean.is_none() {
            let last = *order.last().expect("a grid has cells");
            let rep = walk.find(last);
            let Top::Node(root) = walk.top[slot(rep)] else {
                unreachable!("a world with no sea has no ocean component")
            };
            walk.trees.push(root);
        }
        let Walk {
            nodes,
            holder,
            descent,
            leaf_of,
            trees,
            ..
        } = walk;
        Self {
            ground_m: ground,
            order,
            nodes,
            holder,
            descent,
            leaf_of,
            trees,
        }
    }

    /// Node `k`.
    ///
    /// # Panics
    ///
    /// If there is no node `k`.
    #[cfg_attr(not(test), expect(dead_code, reason = "the erosion's tests read it"))]
    #[must_use]
    pub(crate) fn node(&self, k: u32) -> &Node {
        &self.nodes[slot(k)]
    }

    /// The node land cell `cell` is an own cell of, or `None` for a cell of the ocean's component.
    ///
    /// # Panics
    ///
    /// If `cell` is not a cell of the field.
    #[cfg_attr(not(test), expect(dead_code, reason = "the erosion's tests read it"))]
    #[must_use]
    pub(crate) fn holder(&self, cell: u32) -> Option<u32> {
        self.holder[slot(cell)]
    }

    /// Fill–Spill–Merge of `measure`'s water over the hierarchy of `mesh`'s ground: the water
    /// bodies (see the module documentation).
    ///
    /// # Panics
    ///
    /// If `mesh` is not the hierarchy's, or `measure`'s inputs are not one a cell, finite, with
    /// no amount negative.
    #[must_use]
    pub(crate) fn fill_spill_merge(&self, mesh: &Mesh, measure: Measure<'_>) -> WaterBodies {
        let n = self.ground_m.len();
        assert_eq!(slot(mesh.len()), n, "the hierarchy's mesh");
        let amounts = measure.amounts(n);
        let mut fsm = Fsm::new(self, self.capacities(mesh, measure));
        fsm.gather(amounts);
        fsm.spread();
        self.bodies(mesh, &fsm, measure)
    }

    /// Each node's capacity to its spill in `measure`'s unit, +∞ for a root with no spill: its
    /// children's and its own cells', a merge's index being above its children's.
    fn capacities(&self, mesh: &Mesh, measure: Measure<'_>) -> Vec<f64> {
        let m = self.nodes.len();
        let mut own = vec![0.0; m];
        if let Measure::Balance {
            net_loss_m_per_year,
            ..
        } = measure
        {
            let spill_code: Vec<Option<i32>> = self
                .nodes
                .iter()
                .map(|node| node.spill.map(|s| code(s.level_m)))
                .collect();
            // Each land cell's loss counts from the lowest node below whose spill it lies, by the
            // codes (a cell at a sill's height is not under the full lake), summed in the walk's
            // order.
            for &c in &self.order {
                let Some(mut k) = self.holder[slot(c)] else {
                    continue;
                };
                let ground = code(self.ground_m[slot(c)]);
                loop {
                    if spill_code[slot(k)].is_none_or(|sill| ground < sill) {
                        own[slot(k)] += net_loss_m_per_year[slot(c)] * mesh.area_m2(c);
                        break;
                    }
                    match self.nodes[slot(k)].parent {
                        Some(p) => k = p,
                        None => break,
                    }
                }
            }
        }
        let mut capacity = vec![0.0; m];
        for (k, node) in self.nodes.iter().enumerate() {
            let children = node
                .children
                .map_or(0.0, |[a, b]| capacity[slot(a)] + capacity[slot(b)]);
            capacity[k] = match (node.spill, measure) {
                (None, _) => f64::INFINITY,
                (Some(spill), Measure::Volume { .. }) => num::max(
                    0.0,
                    children + node.area_m2 * (spill.level_m - node.start_m) - node.own_m3,
                ),
                (Some(_), Measure::Balance { .. }) => children + own[k],
            };
        }
        capacity
    }

    /// The water bodies Fill–Spill–Merge left in `fsm`.
    fn bodies(&self, mesh: &Mesh, fsm: &Fsm<'_>, measure: Measure<'_>) -> WaterBodies {
        let m = self.nodes.len();
        let mut body_of: Vec<Option<usize>> = vec![None; m];
        let mut bodies: Vec<Body> = Vec::new();
        // Parents first: a node beneath a body is the body's, whatever its own state (a merge of
        // two flats at their own spill stays inactive beneath a full or rising lake).
        for k in (0..m).rev() {
            let node = &self.nodes[k];
            let inherited = node.parent.and_then(|p| body_of[slot(p)]);
            body_of[k] = if inherited.is_some() {
                inherited
            } else if node.children.is_none() || fsm.active[k] {
                bodies.push(Body {
                    node: cell_of(k),
                    pit: node.pit,
                    level_m: 0.0,
                    full: fsm.full(cell_of(k)),
                    amount: fsm.held[k],
                    cells: Vec::new(),
                });
                Some(bodies.len() - 1)
            } else {
                None
            };
        }
        let mut subtree: Vec<Vec<u32>> = vec![Vec::new(); bodies.len()];
        for &c in &self.order {
            if let Some(b) = self.holder[slot(c)].and_then(|h| body_of[slot(h)]) {
                subtree[b].push(c);
            }
        }
        let mut water_level_m = vec![None; self.ground_m.len()];
        for (body, cells) in bodies.iter_mut().zip(&subtree) {
            let spill = self.nodes[slot(body.node)].spill;
            body.level_m = if body.full {
                spill.expect("a full body has a finite capacity").level_m
            } else {
                let level_m = match measure {
                    Measure::Volume { .. } => self.lake_level(mesh, cells, body.amount),
                    Measure::Balance {
                        net_loss_m_per_year,
                        ..
                    } => self.balance_level(mesh, cells, body.amount, net_loss_m_per_year, spill),
                };
                spill.map_or(level_m, |s| num::min(level_m, s.level_m))
            };
            let level_code = code(body.level_m);
            body.cells = cells
                .iter()
                .copied()
                .filter(|&c| code(self.ground_m[slot(c)]) < level_code)
                .collect();
            body.cells.sort_unstable();
            for &c in &body.cells {
                water_level_m[slot(c)] = Some(body.level_m);
            }
        }
        bodies.retain(|b| !(b.full && b.cells.is_empty()));
        bodies.reverse();
        WaterBodies {
            bodies,
            discharged: fsm.discharged,
            water_level_m,
        }
    }

    /// The level, metres, at which the cells `cells` of a body's subtree, in the walk's order (its
    /// pit first), hold the volume `amount`: over the cells below it, by Barnes et al. 2021's
    /// eq. 6, heights relative to the pit.
    fn lake_level(&self, mesh: &Mesh, cells: &[u32], amount: f64) -> f64 {
        let pit_m = self.ground_m[slot(cells[0])];
        let (mut area_m2, mut rel_m3) = (0.0, 0.0);
        for &c in cells {
            let rel_m = self.ground_m[slot(c)] - pit_m;
            if area_m2 > 0.0 && area_m2 * rel_m - rel_m3 >= amount {
                break;
            }
            let a = mesh.area_m2(c);
            area_m2 += a;
            rel_m3 += a * rel_m;
        }
        pit_m + (amount + rel_m3) / area_m2
    }

    /// The level, metres, of a terminal lake of inflow `amount` (m³ a⁻¹) over the cells `cells`
    /// of its subtree in their fill order, the walk's (within a subtree a cell's filled level is
    /// its ground, its descent pointers leading down to the pit): the lake takes the cells whole,
    /// each while the loss of the cells taken and half its own, `net_loss_m_per_year` times its
    /// area, stays within the inflow (`climate::seas`' half-cell rule), and its level is the
    /// filled level of the first cell it does not take, so that every cell taken lies below it
    /// and its loss is over the cells it floods; the pit's ground where it takes none, a dry
    /// playa; and its spill level, brim-full, where it takes every one (the last cell's ground
    /// for the root of a world with no sea, which has none).
    ///
    /// Lean, pending decision-r09-t14-choices Q3: the ruling's "the level is the last cell's
    /// filled level" would leave the last cell taken at the level, dry by the millimetre codes,
    /// and a lake of one cell a playa.
    fn balance_level(
        &self,
        mesh: &Mesh,
        cells: &[u32],
        amount: f64,
        net_loss_m_per_year: &[f64],
        spill: Option<Spill>,
    ) -> f64 {
        let mut loss = 0.0;
        for &c in cells {
            let cell_loss = net_loss_m_per_year[slot(c)] * mesh.area_m2(c);
            if loss + 0.5 * cell_loss > amount {
                return self.ground_m[slot(c)];
            }
            loss += cell_loss;
        }
        spill.map_or_else(
            || self.ground_m[slot(*cells.last().expect("a body holds its pit"))],
            |s| s.level_m,
        )
    }
}

/// Fill–Spill–Merge's state over a hierarchy's nodes.
struct Fsm<'a> {
    hierarchy: &'a Hierarchy,
    /// Each node's capacity to its spill in the measure's unit, +∞ with no spill; a balance's may
    /// be negative.
    capacity: Vec<f64>,
    /// The water each leaf received, before it is resolved.
    input: Vec<f64>,
    /// The water each node holds: a leaf's own; a merge's, its children's and its own, once both
    /// children are full ([`Fsm::active`]), else none.
    held: Vec<f64>,
    /// Whether a merge node holds its children's water (both are full).
    active: Vec<bool>,
    /// The water each resolved node passed on above its capacity.
    excess: Vec<f64>,
    /// Where the last pour through each full node came to rest.
    jump: Vec<Option<u32>>,
    discharged: f64,
    path: Vec<u32>,
}

impl<'a> Fsm<'a> {
    fn new(hierarchy: &'a Hierarchy, capacity: Vec<f64>) -> Self {
        let m = hierarchy.nodes.len();
        Self {
            hierarchy,
            capacity,
            input: vec![0.0; m],
            held: vec![0.0; m],
            active: vec![false; m],
            excess: vec![0.0; m],
            jump: vec![None; m],
            discharged: 0.0,
            path: Vec::new(),
        }
    }

    /// Passes each cell's water down its pointers to its pit's leaf or the sea (§3.1), in the
    /// reverse of the walk's order.
    fn gather(&mut self, water: &[f64]) {
        let h = self.hierarchy;
        let mut carried = water.to_vec();
        for &c in h.order.iter().rev() {
            let w = carried[slot(c)];
            match (h.descent[slot(c)], h.leaf_of[slot(c)]) {
                (Some(d), _) => carried[slot(d)] += w,
                (None, Some(leaf)) => self.input[slot(leaf)] += w,
                (None, None) => self.discharged += w,
            }
        }
    }

    /// Resolves the trees, the last linked to the sea first, each tree's nodes children first
    /// (§3.2), and passes each root's excess along its link.
    fn spread(&mut self) {
        let h = self.hierarchy;
        let m = h.nodes.len();
        let mut root_of = vec![0_u32; m];
        for k in (0..m).rev() {
            root_of[k] = h.nodes[k].parent.map_or(cell_of(k), |p| root_of[slot(p)]);
        }
        let mut position = vec![usize::MAX; m];
        for (t, &root) in h.trees.iter().enumerate() {
            position[slot(root)] = t;
        }
        let mut start = vec![0_usize; h.trees.len() + 1];
        for &root in &root_of {
            let t = position[slot(root)];
            assert!(t != usize::MAX, "every root is a tree of the forest");
            start[t + 1] += 1;
        }
        for t in 0..h.trees.len() {
            start[t + 1] += start[t];
        }
        let mut next = start.clone();
        let mut members = vec![0_u32; m];
        for (k, &root) in root_of.iter().enumerate() {
            let at = &mut next[position[slot(root)]];
            members[*at] = cell_of(k);
            *at += 1;
        }
        for t in (0..h.trees.len()).rev() {
            for &k in &members[start[t]..start[t + 1]] {
                self.resolve(k);
            }
            let root = h.trees[t];
            let excess = self.excess[slot(root)];
            if excess > 0.0 {
                match h.nodes[slot(root)].overflow {
                    Overflow::Sea(Some(leaf)) => {
                        debug_assert!(position[slot(root_of[slot(leaf)])] < t);
                        self.input[slot(leaf)] += excess;
                    }
                    Overflow::Sea(None) => self.discharged += excess,
                    Overflow::Nowhere | Overflow::Sibling(_) => {
                        unreachable!("a root with no spill holds all it gets, and has no sibling")
                    }
                }
            }
        }
    }

    /// Resolves node `k`, its children resolved: what it holds and what it passes on.
    fn resolve(&mut self, k: u32) {
        let ki = slot(k);
        match self.hierarchy.nodes[ki].children {
            None => {
                self.held[ki] = self.input[ki];
                self.active[ki] = true;
            }
            Some([a, b]) => {
                let (ea, eb) = (self.excess[slot(a)], self.excess[slot(b)]);
                // Both full, with or without excess (a flat's leaves hold nothing and are full):
                // the merge holds both at its own level.
                if self.full(a) && self.full(b) {
                    self.active[ki] = true;
                    self.held[ki] = self.held[slot(a)] + self.held[slot(b)] + (ea + eb);
                } else if ea > 0.0 {
                    self.pour(self.geolink(a), k, ea);
                } else if eb > 0.0 {
                    self.pour(self.geolink(b), k, eb);
                }
            }
        }
        if self.held[ki] > self.capacity[ki] {
            self.excess[ki] = self.held[ki] - self.capacity[ki];
            self.held[ki] = self.capacity[ki];
        }
    }

    /// Whether resolved node `k` is full: it holds its capacity.
    fn full(&self, k: u32) -> bool {
        self.held[slot(k)] >= self.capacity[slot(k)]
    }

    /// The leaf of its sibling's subtree that child `k`'s excess enters.
    fn geolink(&self, k: u32) -> u32 {
        match self.hierarchy.nodes[slot(k)].overflow {
            Overflow::Sibling(leaf) => leaf,
            Overflow::Nowhere | Overflow::Sea(_) => unreachable!("a merged node has a geolink"),
        }
    }

    /// Pours `extra` into leaf `start`, below `stop`, the node being resolved: it fills the
    /// leaf and climbs through the resolved subtree, each full node passing it to its sibling's
    /// geolink while the sibling has room, else to their parent, until it comes to rest or
    /// reaches `stop`, which takes what is left (Barnes et al. 2021's `OverflowInto`).
    fn pour(&mut self, start: u32, stop: u32, mut extra: f64) {
        let hierarchy: &'a Hierarchy = self.hierarchy;
        let nodes = &hierarchy.nodes;
        self.path.clear();
        let mut b = start;
        let end = loop {
            let bi = slot(b);
            if b == stop {
                self.held[bi] += extra;
                break b;
            }
            let spare = self.capacity[bi] - self.held[bi];
            if extra <= spare {
                self.held[bi] += extra;
                break b;
            }
            self.held[bi] = self.capacity[bi];
            extra -= spare;
            self.path.push(b);
            if let Some(j) = self.jump[bi] {
                b = j;
                continue;
            }
            let p = nodes[bi]
                .parent
                .expect("a node below the stop has a parent");
            let [x, y] = nodes[slot(p)].children.expect("a parent has two children");
            let sibling = if x == b { y } else { x };
            if !self.full(sibling) {
                b = self.geolink(b);
                continue;
            }
            if !self.active[slot(p)] {
                self.active[slot(p)] = true;
                self.held[slot(p)] = self.held[bi] + self.held[slot(sibling)];
            }
            b = p;
        };
        for &v in &self.path {
            self.jump[slot(v)] = Some(end);
        }
    }
}

/// A water body: a lake, full at its spill or terminal below it, or a dry playa.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Body {
    /// The hierarchy's node it fills.
    pub(crate) node: u32,
    /// Its pit, the lowest cell beneath it.
    pub(crate) pit: u32,
    /// Its surface, metres above the datum: its spill level if it is full; else its lake's level,
    /// or a dry playa's floor, its pit's ground.
    pub(crate) level_m: f64,
    /// Whether it is at its spill level and spills (through-flow); else it is terminal.
    pub(crate) full: bool,
    /// The water it holds, or for a balance the loss it takes, in its measure's unit: negative
    /// for a full body whose precipitation exceeds its evaporation.
    pub(crate) amount: f64,
    /// The cells under it, by the millimetre codes, ascending.
    pub(crate) cells: Vec<u32>,
}

impl Body {
    /// Whether it is a dry playa: terminal, and under it no cell.
    #[cfg_attr(not(test), expect(dead_code, reason = "the erosion's tests read it"))]
    #[must_use]
    pub(crate) fn is_playa(&self) -> bool {
        !self.full && self.cells.is_empty()
    }

    /// The base level of the closed basin it ends, metres (decision-r09-t14-choices Q1's `ℓ_b`):
    /// a terminal lake's level or a playa's floor; `None` for a full body, which drains on
    /// through its outlet.
    #[must_use]
    pub(crate) fn base_level_m(&self) -> Option<f64> {
        (!self.full).then_some(self.level_m)
    }
}

/// The water bodies Fill–Spill–Merge leaves, and what it discharged to the sea.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WaterBodies {
    bodies: Vec<Body>,
    discharged: f64,
    water_level_m: Vec<Option<f64>>,
}

impl WaterBodies {
    /// The bodies, in ascending order of their node.
    #[must_use]
    pub(crate) fn bodies(&self) -> &[Body] {
        &self.bodies
    }

    /// What passed to the sea, in the measure's unit.
    #[cfg_attr(not(test), expect(dead_code, reason = "the erosion's tests read it"))]
    #[must_use]
    pub(crate) const fn discharged(&self) -> f64 {
        self.discharged
    }

    /// The surface of the body over cell `cell`, metres, or `None` where it is dry.
    ///
    /// # Panics
    ///
    /// If `cell` is not a cell of the field.
    #[cfg_attr(not(test), expect(dead_code, reason = "the erosion's tests read it"))]
    #[must_use]
    pub(crate) fn water_level_m(&self, cell: u32) -> Option<f64> {
        self.water_level_m[slot(cell)]
    }
}

/// The water bodies of `measure`'s water on `mesh`'s ground `ground` (metres, in cell-index
/// order), the cells `sea` marks the ocean: the depression hierarchy and Fill–Spill–Merge (see the
/// module documentation). The solver finds them once a multigrid level, on the level's
/// construction, and holds them for its iterations there.
///
/// # Panics
///
/// As [`Hierarchy::build`] and [`Hierarchy::fill_spill_merge`].
#[must_use]
pub(crate) fn water_bodies(
    mesh: &Mesh,
    ground: &[f64],
    sea: &[bool],
    measure: Measure<'_>,
) -> WaterBodies {
    Hierarchy::build(mesh, ground, sea).fill_spill_merge(mesh, measure)
}

/// The drainage of `mesh`'s current ground `z` with the water bodies `bodies`: [`drainage::route`]
/// from every sea cell (`sea`) at `sea_level_m` and every terminal body's pit at its level, a
/// lake's or a dry playa's floor, ascending by cell, or, with neither, from the lowest cell at its
/// ground (by (ground, cell index)); each cell's receiver drawn with its uniform in `uniforms`. A
/// full body whose cells now lie above its level is simply not filled.
///
/// # Panics
///
/// As [`drainage::route`]: a sea cell's ground must be at or below the sea's level, and a
/// terminal body's pit's at or below its level, as they are on the bodies' own ground and
/// wherever the solver holds them as bounds.
#[must_use]
pub(crate) fn route_with_water(
    mesh: &Mesh,
    z: &[f64],
    sea: &[bool],
    sea_level_m: f64,
    bodies: &WaterBodies,
    uniforms: &[f64],
) -> Routing {
    route_with_water_from(mesh, z, sea, sea_level_m, bodies, uniforms, None)
}

/// [`route_with_water`] with each cell keeping its receiver of `previous` while it stays strictly
/// below it ([`drainage::route_from`], the sticky receivers).
///
/// # Panics
///
/// As [`route_with_water`] and [`drainage::route_from`].
#[must_use]
pub(crate) fn route_with_water_from(
    mesh: &Mesh,
    z: &[f64],
    sea: &[bool],
    sea_level_m: f64,
    bodies: &WaterBodies,
    uniforms: &[f64],
    previous: Option<&Routing>,
) -> Routing {
    assert_eq!(sea.len(), slot(mesh.len()), "one sea flag a cell");
    let mut seeds: Vec<Seed> = sea
        .iter()
        .zip(0..mesh.len())
        .filter(|&(&wet, _)| wet)
        .map(|(_, cell)| Seed {
            cell,
            level_m: sea_level_m,
        })
        .collect();
    seeds.extend(bodies.bodies.iter().filter(|b| !b.full).map(|b| Seed {
        cell: b.pit,
        level_m: b.level_m,
    }));
    seeds.sort_unstable_by_key(|s| s.cell);
    if seeds.is_empty() {
        let lowest = (0..mesh.len())
            .min_by(|&a, &b| {
                num::assert_finite(z[slot(a)])
                    .total_cmp(&num::assert_finite(z[slot(b)]))
                    .then(a.cmp(&b))
            })
            .expect("a grid has cells");
        seeds.push(Seed {
            cell: lowest,
            level_m: z[slot(lowest)],
        });
    }
    drainage::route_from(mesh, z, &seeds, uniforms, previous)
}
