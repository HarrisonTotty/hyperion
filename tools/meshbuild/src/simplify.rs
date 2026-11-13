//! Simplifying one group of clusters with its border locked.
//!
//! Half-edge collapses ordered by quadric error (Garland and Heckbert 1997, "Surface Simplification
//! Using Quadric Error Metrics", SIGGRAPH), in passes as meshoptimizer makes them (Kapoulkine,
//! `simplifier.cpp`, MIT): each pass sorts every allowed collapse by its error and makes the
//! cheapest ones that touch no vertex already moved in the pass, until the group is down to its
//! target. A half-edge collapse moves a vertex onto a neighbour's grid point, so the simplified
//! surface's vertices are the input's: it stays on the grid, and within its inputs' spheres.
//!
//! - **Locked**: a vertex any other group also uses (the group's border), or one whose fan is not
//!   a disc or half-disc, never moves; so the group's border is the same edges before and after,
//!   and every cut of the DAG is watertight.
//! - **Border**: a vertex on the mesh's open boundary moves only along it, and the boundary's
//!   edges carry a quadric of the plane through them normal to their face, so that it keeps its
//!   shape.
//! - **Valid**: a collapse must keep the surface manifold (the link condition, which counts the
//!   neighbours the locked vertices have outside the group), flip no triangle and make none
//!   degenerate (the flip turning a normal by more than about 75°; the sign exact, in integers),
//!   leave no triangle with every corner locked (which on a group's border could only be a sliver
//!   along it, which a neighbouring group could make as well), and keep its attribute seams: each of the
//!   moving vertex's attribute vertices must map to one of the target's through the triangles that
//!   collapse, so that a UV or normal seam moves only along itself.
//!
//! The group's error is then measured, not taken from the quadrics: the larger of the farthest
//! input vertex from the simplified surface and the farthest simplified sample (each triangle's
//! centroid and edge midpoints) from the input surface, each against the triangles about where it
//! collapsed to, which bounds the distance to the whole surface from above.

use crate::geometry::{
    F3, I3, cross, cross_i64, dot, dot_i128, length, point_triangle_distance, scale, sub, sub_i64,
    to_f64,
};
use crate::mesh::QuantisedMesh;
use crate::num::{ix, max, min, u32_of};

/// The weight of an open boundary edge's quadric, against its faces' area: meshoptimizer's
/// `kBoundaryWeight`.
const BORDER_WEIGHT: f64 = 10.0;

/// The cosine of the most a collapse may turn a triangle's normal: about 75°, meshoptimizer's
/// bound (`hasTriangleFlip`), since a bound at 90° lets a run of collapses fold a surface.
const FLIP_COSINE: f64 = 0.25;

/// Whether a triangle's normal turning from `before` to `after` stays within [`FLIP_COSINE`]:
/// its sign exact in integers, its size in `f64`.
fn within_flip_bound(before: I3, after: I3) -> bool {
    let d = dot_i128(before, after);
    if d <= 0 {
        return false;
    }
    let (b, a) = (to_f64(before), to_f64(after));
    crate::num::f64_of_i128(d) > FLIP_COSINE * (dot(b, b) * dot(a, a)).sqrt()
}

/// A simplified group.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Simplified {
    /// Its triangles, as indices into the mesh's vertices.
    pub(crate) triangles: Vec<[u32; 3]>,
    /// Its measured deviation from the input, in grid steps.
    pub(crate) error_steps: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Interior,
    Border,
    Locked,
}

/// A plane quadric's sum: Q(v) = vᵀAv + 2bᵀv + c, and the area `w` it was weighted by.
#[derive(Debug, Clone, Copy, Default)]
struct Quadric {
    a: [f64; 6],
    b: F3,
    c: f64,
    w: f64,
}

impl Quadric {
    /// `weight` × the squared distance to the plane n·v + d = 0, n unit length.
    fn plane(normal: F3, offset: f64, weight: f64) -> Self {
        let [nx, ny, nz] = normal;
        Self {
            a: [nx * nx, nx * ny, nx * nz, ny * ny, ny * nz, nz * nz].map(|v| v * weight),
            b: scale(normal, offset * weight),
            c: offset * offset * weight,
            w: weight,
        }
    }

    fn add(&mut self, o: &Self) {
        for i in 0..6 {
            self.a[i] += o.a[i];
        }
        for i in 0..3 {
            self.b[i] += o.b[i];
        }
        self.c += o.c;
        self.w += o.w;
    }

    /// The area-weighted mean squared distance from `v` to the planes.
    fn error(&self, point: F3) -> f64 {
        let [a00, a01, a02, a11, a12, a22] = self.a;
        let [px, py, pz] = point;
        let q = px * (a00 * px + a01 * py + a02 * pz)
            + py * (a01 * px + a11 * py + a12 * pz)
            + pz * (a02 * px + a12 * py + a22 * pz)
            + 2.0 * dot(self.b, point)
            + self.c;
        let q = max(q, 0.0);
        if self.w > 0.0 { q / self.w } else { q }
    }
}

/// The group's triangles, and its positions' context at the level.
pub(crate) struct Group<'a, L, O>
where
    L: Fn(u32) -> bool,
    O: Fn(u32, &mut Vec<u32>),
{
    pub(crate) mesh: &'a QuantisedMesh,
    /// Its triangles, as indices into the mesh's vertices.
    pub(crate) triangles: &'a [[u32; 3]],
    /// Whether a (welded) position is used by another group.
    pub(crate) locked: L,
    /// Appends a position's neighbours through triangles outside the group.
    pub(crate) outside_neighbours: O,
}

/// Simplifies `group` towards `target` triangles (see the module's documentation).
#[must_use]
pub(crate) fn simplify<L, O>(group: &Group<'_, L, O>, target: usize) -> Simplified
where
    L: Fn(u32) -> bool,
    O: Fn(u32, &mut Vec<u32>),
{
    let mut state = State::new(group);
    let input_positions = state.tpos.clone();
    let input_incident = state.incident.clone();
    while state.live > target && state.pass(target) {}
    let error_steps = state.measure(&input_positions, &input_incident);
    let triangles = (0..state.corners.len())
        .filter(|&t| state.alive[t])
        .map(|t| state.corners[t])
        .collect();
    Simplified {
        triangles,
        error_steps,
    }
}

struct State {
    /// Each local position's welded position.
    global: Vec<u32>,
    /// Its grid coordinates less the group's least corner.
    pos: Vec<I3>,
    kind: Vec<Kind>,
    /// A locked position's neighbours outside the group, welded, sorted.
    outside: Vec<Vec<u32>>,
    quadric: Vec<Quadric>,
    /// The position each collapsed into (itself while it stands).
    into: Vec<u32>,
    /// Each triangle's vertices (indices into the mesh's).
    corners: Vec<[u32; 3]>,
    /// Each triangle's local positions.
    tpos: Vec<[u32; 3]>,
    alive: Vec<bool>,
    /// Each local position's live triangles.
    incident: Vec<Vec<u32>>,
    live: usize,
}

impl State {
    fn new<L, O>(group: &Group<'_, L, O>) -> Self
    where
        L: Fn(u32) -> bool,
        O: Fn(u32, &mut Vec<u32>),
    {
        let mesh = group.mesh;
        let mut global: Vec<u32> = group
            .triangles
            .iter()
            .flatten()
            .map(|&v| mesh.position_of(v))
            .collect();
        global.sort_unstable();
        global.dedup();
        let local = |p: u32| u32_of(global.binary_search(&p).expect("a position of the group"));
        let tpos: Vec<[u32; 3]> = group
            .triangles
            .iter()
            .map(|t| t.map(|v| local(mesh.position_of(v))))
            .collect();
        let grid: Vec<I3> = global.iter().map(|&p| mesh.grid_i64(p)).collect();
        let mut lo = [i64::MAX; 3];
        for g in &grid {
            for axis in 0..3 {
                lo[axis] = lo[axis].min(g[axis]);
            }
        }
        let pos: Vec<I3> = grid.iter().map(|&g| sub_i64(g, lo)).collect();
        let mut incident = vec![Vec::new(); global.len()];
        for (t, corners) in tpos.iter().enumerate() {
            for &p in corners {
                incident[ix(p)].push(u32_of(t));
            }
        }
        let mut state = Self {
            kind: vec![Kind::Interior; global.len()],
            outside: vec![Vec::new(); global.len()],
            quadric: vec![Quadric::default(); global.len()],
            into: (0..u32_of(global.len())).collect(),
            corners: group.triangles.to_vec(),
            alive: vec![true; tpos.len()],
            live: tpos.len(),
            global,
            pos,
            tpos,
            incident,
        };
        state.classify(&group.locked, &group.outside_neighbours);
        state.add_quadrics();
        state
    }

    fn classify(&mut self, locked: &impl Fn(u32) -> bool, outside: &impl Fn(u32, &mut Vec<u32>)) {
        for p in 0..self.global.len() {
            self.kind[p] = if locked(self.global[p]) {
                let mut list = Vec::new();
                outside(self.global[p], &mut list);
                list.sort_unstable();
                list.dedup();
                self.outside[p] = list;
                Kind::Locked
            } else {
                self.fan_kind(u32_of(p))
            };
        }
    }

    /// Interior if `p`'s fan is a disc, Border if a half-disc, else Locked.
    fn fan_kind(&self, p: u32) -> Kind {
        let mut link: Vec<(u32, u32)> = self.incident[ix(p)]
            .iter()
            .map(|&t| {
                let c = self.tpos[ix(t)];
                let k = c
                    .iter()
                    .position(|&x| x == p)
                    .expect("a corner of its triangle");
                (c[(k + 1) % 3], c[(k + 2) % 3])
            })
            .collect();
        link.sort_unstable();
        let mut tails: Vec<u32> = link.iter().map(|e| e.1).collect();
        tails.sort_unstable();
        if link.windows(2).any(|w| w[0].0 == w[1].0) || tails.windows(2).any(|w| w[0] == w[1]) {
            return Kind::Locked;
        }
        let starts: Vec<u32> = link
            .iter()
            .map(|e| e.0)
            .filter(|x| tails.binary_search(x).is_err())
            .collect();
        let next = |x: u32| {
            link.binary_search_by_key(&x, |e| e.0)
                .ok()
                .map(|i| link[i].1)
        };
        let (first, cycle) = match starts.as_slice() {
            [] => (link[0].0, true),
            [start] => (*start, false),
            _ => return Kind::Locked,
        };
        let (mut x, mut steps) = (first, 0);
        while let Some(y) = next(x) {
            steps += 1;
            x = y;
            if x == first || steps > link.len() {
                break;
            }
        }
        match (cycle, steps == link.len() && (x == first) == cycle) {
            (true, true) => Kind::Interior,
            (false, true) => Kind::Border,
            (_, false) => Kind::Locked,
        }
    }

    fn add_quadrics(&mut self) {
        let mut directed: Vec<(u32, u32)> = self
            .tpos
            .iter()
            .flat_map(|c| (0..3).map(move |k| (c[k], c[(k + 1) % 3])))
            .collect();
        directed.sort_unstable();
        for triangle in 0..self.tpos.len() {
            let corners = self.tpos[triangle];
            let [first, second, third] = corners.map(|p| to_f64(self.pos[ix(p)]));
            let normal = cross(sub(second, first), sub(third, first));
            let twice_area = length(normal);
            if twice_area <= 0.0 {
                continue;
            }
            let unit = scale(normal, 1.0 / twice_area);
            let face = Quadric::plane(unit, -dot(unit, first), 0.5 * twice_area);
            for p in corners {
                self.quadric[ix(p)].add(&face);
            }
            for k in 0..3 {
                let (from, to) = (corners[k], corners[(k + 1) % 3]);
                let open = directed.binary_search(&(to, from)).is_err();
                let locked = |p: u32| self.kind[ix(p)] == Kind::Locked;
                if open && !(locked(from) && locked(to)) {
                    self.add_border_quadric(from, to, unit);
                }
            }
        }
    }

    /// The quadric of the plane through an open edge normal to its face, on both its ends.
    fn add_border_quadric(&mut self, from: u32, to: u32, face_normal: F3) {
        let start = to_f64(self.pos[ix(from)]);
        let edge = sub(to_f64(self.pos[ix(to)]), start);
        let across = cross(edge, face_normal);
        let len = length(across);
        if len <= 0.0 {
            return;
        }
        let across = scale(across, 1.0 / len);
        let side = Quadric::plane(across, -dot(across, start), dot(edge, edge) * BORDER_WEIGHT);
        self.quadric[ix(from)].add(&side);
        self.quadric[ix(to)].add(&side);
    }

    /// The live neighbours of `p`, local, sorted.
    fn neighbours(&self, p: u32) -> Vec<u32> {
        let mut n: Vec<u32> = self.incident[ix(p)]
            .iter()
            .flat_map(|&t| self.tpos[ix(t)])
            .filter(|&x| x != p)
            .collect();
        n.sort_unstable();
        n.dedup();
        n
    }

    /// How many live triangles hold both `p` and `q`.
    fn shared(&self, p: u32, q: u32) -> usize {
        self.incident[ix(p)]
            .iter()
            .filter(|&&t| self.tpos[ix(t)].contains(&q))
            .count()
    }

    /// One pass of collapses; whether it made any.
    fn pass(&mut self, target: usize) -> bool {
        let mut candidates: Vec<(f64, u32, u32)> = Vec::new();
        for p in 0..u32_of(self.global.len()) {
            if self.kind[ix(p)] == Kind::Locked || self.incident[ix(p)].is_empty() {
                continue;
            }
            for q in self.neighbours(p) {
                if self.kind[ix(p)] == Kind::Border && self.shared(p, q) != 1 {
                    continue;
                }
                let cost = self.quadric[ix(p)].error(to_f64(self.pos[ix(q)]));
                debug_assert!(!cost.is_nan(), "a collapse's cost is a number");
                candidates.push((cost, p, q));
            }
        }
        candidates.sort_unstable_by(|x, y| x.0.total_cmp(&y.0).then((x.1, x.2).cmp(&(y.1, y.2))));
        let needed = (self.live - target).div_ceil(2);
        let mut touched = vec![false; self.global.len()];
        let mut made = 0;
        for (_, p, q) in candidates {
            if made >= needed || self.live <= target {
                break;
            }
            if touched[ix(p)] || touched[ix(q)] {
                continue;
            }
            if self.kind[ix(p)] == Kind::Border && self.shared(p, q) != 1 {
                continue;
            }
            if let Some(wedges) = self.valid(p, q) {
                self.collapse(p, q, &wedges);
                touched[ix(p)] = true;
                touched[ix(q)] = true;
                made += 1;
            }
        }
        made > 0
    }

    /// The attribute map of the collapse p → q if it is valid (see the module's documentation).
    fn valid(&self, p: u32, q: u32) -> Option<Vec<(u32, u32)>> {
        let to_global = |n: Vec<u32>| {
            n.into_iter()
                .map(|x| self.global[ix(x)])
                .collect::<Vec<_>>()
        };
        let around_p = to_global(self.neighbours(p));
        let mut around_q = to_global(self.neighbours(q));
        around_q.extend_from_slice(&self.outside[ix(q)]);
        around_q.sort_unstable();
        around_q.dedup();
        let common: Vec<u32> = around_p
            .iter()
            .copied()
            .filter(|x| around_q.binary_search(x).is_ok())
            .collect();
        let mut opposite: Vec<u32> = Vec::new();
        let mut wedges: Vec<(u32, u32)> = Vec::new();
        for &t in &self.incident[ix(p)] {
            let c = self.tpos[ix(t)];
            let kp = c
                .iter()
                .position(|&x| x == p)
                .expect("a corner of its triangle");
            if let Some(kq) = c.iter().position(|&x| x == q) {
                opposite.push(self.global[ix(c[3 - kp - kq])]);
                let pair = (self.corners[ix(t)][kp], self.corners[ix(t)][kq]);
                match wedges.iter().find(|w| w.0 == pair.0) {
                    Some(w) if w.1 != pair.1 => return None,
                    Some(_) => {}
                    None => wedges.push(pair),
                }
            }
        }
        opposite.sort_unstable();
        if common != opposite {
            return None;
        }
        for &t in &self.incident[ix(p)] {
            let c = self.tpos[ix(t)];
            if c.contains(&q) {
                continue;
            }
            let kp = c
                .iter()
                .position(|&x| x == p)
                .expect("a corner of its triangle");
            if !wedges.iter().any(|w| w.0 == self.corners[ix(t)][kp]) {
                return None;
            }
            let corner = |k: usize, moved: bool| {
                if moved && k == kp {
                    self.pos[ix(q)]
                } else {
                    self.pos[ix(c[k])]
                }
            };
            let normal = |moved: bool| {
                cross_i64(
                    sub_i64(corner(1, moved), corner(0, moved)),
                    sub_i64(corner(2, moved), corner(0, moved)),
                )
            };
            let (before, after) = (normal(false), normal(true));
            if after == [0; 3] || !within_flip_bound(before, after) {
                return None;
            }
            if (0..3).all(|k| k == kp || self.kind[ix(c[k])] == Kind::Locked)
                && self.kind[ix(q)] == Kind::Locked
            {
                return None;
            }
        }
        Some(wedges)
    }

    fn collapse(&mut self, p: u32, q: u32, wedges: &[(u32, u32)]) {
        for t in std::mem::take(&mut self.incident[ix(p)]) {
            let c = self.tpos[ix(t)];
            if c.contains(&q) {
                self.alive[ix(t)] = false;
                self.live -= 1;
                for x in c {
                    if x != p {
                        self.incident[ix(x)].retain(|&u| u != t);
                    }
                }
            } else {
                let kp = c
                    .iter()
                    .position(|&x| x == p)
                    .expect("a corner of its triangle");
                self.tpos[ix(t)][kp] = q;
                let from = self.corners[ix(t)][kp];
                self.corners[ix(t)][kp] = wedges
                    .iter()
                    .find(|w| w.0 == from)
                    .expect("checked by valid")
                    .1;
                self.incident[ix(q)].push(t);
            }
        }
        self.into[ix(p)] = q;
        let moved = self.quadric[ix(p)];
        self.quadric[ix(q)].add(&moved);
    }

    fn standing(&self, mut p: u32) -> u32 {
        while self.into[ix(p)] != p {
            p = self.into[ix(p)];
        }
        p
    }

    /// The measured error, in grid steps (see the module's documentation).
    fn measure(&self, input_positions: &[[u32; 3]], input_incident: &[Vec<u32>]) -> f64 {
        let point = |p: u32| to_f64(self.pos[ix(p)]);
        let standing: Vec<u32> = (0..u32_of(self.global.len()))
            .map(|p| self.standing(p))
            .collect();
        let mut members: Vec<Vec<u32>> = vec![Vec::new(); self.global.len()];
        for (p, &s) in standing.iter().enumerate() {
            members[ix(s)].push(u32_of(p));
        }
        let live: Vec<u32> = (0..u32_of(self.tpos.len()))
            .filter(|&t| self.alive[ix(t)])
            .collect();
        let distance = |x: F3, triangles: &[u32], positions: &dyn Fn(u32) -> [u32; 3]| {
            triangles.iter().fold(f64::INFINITY, |d, &t| {
                let [a, b, c] = positions(t).map(point);
                min(d, point_triangle_distance(x, a, b, c))
            })
        };
        let now = |t: u32| self.tpos[ix(t)];
        let before = |t: u32| input_positions[ix(t)];
        let mut error: f64 = 0.0;
        let mut near: Vec<u32> = Vec::new();
        for p in 0..u32_of(self.global.len()) {
            if standing[ix(p)] == p {
                continue;
            }
            near.clear();
            for &t in &input_incident[ix(p)] {
                for x in input_positions[ix(t)] {
                    near.extend_from_slice(&self.incident[ix(standing[ix(x)])]);
                }
            }
            near.sort_unstable();
            near.dedup();
            let d = if near.is_empty() {
                distance(point(p), &live, &now)
            } else {
                distance(point(p), &near, &now)
            };
            error = max(error, d);
        }
        for &t in &live {
            let c = self.tpos[ix(t)];
            near.clear();
            for x in c {
                for &m in &members[ix(x)] {
                    near.extend_from_slice(&input_incident[ix(m)]);
                }
            }
            near.sort_unstable();
            near.dedup();
            let [a, b, d] = c.map(point);
            let samples = [
                scale(
                    [a[0] + b[0] + d[0], a[1] + b[1] + d[1], a[2] + b[2] + d[2]],
                    1.0 / 3.0,
                ),
                scale(crate::geometry::add(a, b), 0.5),
                scale(crate::geometry::add(b, d), 0.5),
                scale(crate::geometry::add(d, a), 0.5),
            ];
            for s in samples {
                error = max(error, distance(s, &near, &before));
            }
        }
        error
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::SourceMesh;

    /// An n × n grid of unit quads at z = 0, quantised.
    fn flat_grid(n: u32) -> QuantisedMesh {
        let mut positions = Vec::new();
        for j in 0..=n {
            for i in 0..=n {
                positions.push([f64::from(i), f64::from(j), 0.0]);
            }
        }
        let mut triangles = Vec::new();
        for j in 0..n {
            for i in 0..n {
                let v = |di: u32, dj: u32| (j + dj) * (n + 1) + i + di;
                triangles.push([v(0, 0), v(1, 0), v(1, 1)]);
                triangles.push([v(0, 0), v(1, 1), v(0, 1)]);
            }
        }
        QuantisedMesh::new(&SourceMesh::new(positions, triangles).unwrap()).unwrap()
    }

    fn boundary(mesh: &QuantisedMesh, triangles: &[[u32; 3]]) -> Vec<(u32, u32)> {
        let mut edges: Vec<(u32, u32)> = triangles
            .iter()
            .map(|t| t.map(|v| mesh.position_of(v)))
            .flat_map(|p| [(p[0], p[1]), (p[1], p[2]), (p[2], p[0])])
            .collect();
        edges.sort_unstable();
        let reversed = edges.clone();
        edges.retain(|&(a, b)| reversed.binary_search(&(b, a)).is_err());
        edges
    }

    #[test]
    fn a_flat_grid_with_its_border_locked_halves_with_no_error_and_keeps_its_border() {
        let mesh = flat_grid(12);
        let far = mesh.grid.iter().map(|g| g[0]).max().unwrap();
        let on_border = |p: u32| {
            let q = mesh.grid[ix(p)];
            q[0] == 0 || q[1] == 0 || q[0] == far || q[1] == far
        };
        let group = Group {
            mesh: &mesh,
            triangles: &mesh.triangles,
            locked: on_border,
            outside_neighbours: |_: u32, _: &mut Vec<u32>| {},
        };
        let result = simplify(&group, mesh.triangles.len() / 2);
        assert!(
            result.triangles.len() <= mesh.triangles.len() / 2,
            "{}",
            result.triangles.len()
        );
        assert!(
            result.error_steps < 1e-9,
            "a plane simplifies exactly: {}",
            result.error_steps
        );
        assert_eq!(
            boundary(&mesh, &result.triangles),
            boundary(&mesh, &mesh.triangles)
        );
    }

    #[test]
    fn an_open_grid_simplifies_along_its_boundary() {
        let mesh = flat_grid(12);
        let group = Group {
            mesh: &mesh,
            triangles: &mesh.triangles,
            locked: |_: u32| false,
            outside_neighbours: |_: u32, _: &mut Vec<u32>| {},
        };
        let result = simplify(&group, 8);
        assert!(result.triangles.len() < 40, "{}", result.triangles.len());
        assert!(result.error_steps < 1e-9, "{}", result.error_steps);
    }

    #[test]
    fn fans_are_told_apart() {
        let mesh = flat_grid(2);
        let group = Group {
            mesh: &mesh,
            triangles: &mesh.triangles,
            locked: |_: u32| false,
            outside_neighbours: |_: u32, _: &mut Vec<u32>| {},
        };
        let state = State::new(&group);
        let kind_at = |x: u32, y: u32| {
            let p = mesh
                .grid
                .iter()
                .position(|g| g[0] == x && g[1] == y)
                .unwrap();
            let local = state.global.binary_search(&u32_of(p)).unwrap();
            state.kind[local]
        };
        let far = mesh.grid.iter().map(|g| g[0]).max().unwrap();
        assert_eq!(kind_at(far / 2, far / 2), Kind::Interior);
        assert_eq!(kind_at(far / 2, 0), Kind::Border);
        assert_eq!(kind_at(0, 0), Kind::Border);
    }
}
