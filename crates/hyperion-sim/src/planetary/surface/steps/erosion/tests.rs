//! R09.T14.a's tests of the drainage (Design note 9): the plan's four (drainage area at every
//! outlet, no cell draining uphill after filling, the same seed's receivers, a finite inventory
//! filling the lowest basins first and spilling) and the lane's checks (flats without ε, the flood
//! against a minimax oracle, Priority-Flood against the merge tree when every depression is full,
//! and the receivers' draw in proportion to the drop per metre). Each runs on hand-built ground
//! over a real cube-sphere level, 3 to 6, on a body of radius 1,000 km.

use hyperion_testkit::lcg::Lcg;

use super::mesh::Mesh;
use crate::math;
use crate::planetary::surface::grid::CellGraph;
use crate::planetary::surface::steps::plates::{cell_of, slot};
use crate::units::Metres;

mod depressions;
mod drainage;

/// The radius of the test bodies, 1,000 km.
const RADIUS: Metres = Metres::new(1.0e6);

/// The sea level of the test fields with a sea, metres.
const SEA_LEVEL_M: f64 = 0.0;

/// Level `level`'s cells and their mesh on the test body.
fn world(level: u8) -> (CellGraph, Mesh) {
    let graph = CellGraph::new(level).unwrap();
    let mesh = Mesh::of(&graph, RADIUS);
    assert_eq!(mesh.level(), level);
    assert_eq!(mesh.len(), graph.len());
    (graph, mesh)
}

/// `v` scaled to unit length.
fn unit(v: [f64; 3]) -> [f64; 3] {
    let norm = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    v.map(|x| x / norm)
}

/// The dot product of `a` and `b`.
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The cells of `graph` whose centres lie within `radius_rad` of the unit vector `centre`.
fn cap(graph: &CellGraph, centre: [f64; 3], radius_rad: f64) -> Vec<bool> {
    let threshold = math::cos(radius_rad);
    graph
        .centres()
        .iter()
        .map(|&d| dot(d, centre) > threshold)
        .collect()
}

/// A rough field with a sea: ground rising northwards by 2.5 km per unit of the centre's z, from
/// −1,875 m at the south pole to 3,125 m at the north, with ±300 m of independent noise a cell
/// (the testkit's LCG under `seed`), so that the land is pitted with depressions; the sea is every
/// cell below [`SEA_LEVEL_M`].
fn rough_terrain(graph: &CellGraph, seed: u64) -> (Vec<f64>, Vec<bool>) {
    let mut lcg = Lcg::new(seed);
    let z: Vec<f64> = graph
        .centres()
        .iter()
        .map(|d| 2_500.0 * (d[2] + 0.25) + 600.0 * (lcg.next_f64() - 0.5))
        .collect();
    let sea = z.iter().map(|&h| h < SEA_LEVEL_M).collect();
    (z, sea)
}

/// Rain of depth `depth_m` on every cell: its volume, m³, a cell.
fn rain(mesh: &Mesh, depth_m: f64) -> Vec<f64> {
    mesh.areas_m2().iter().map(|&a| a * depth_m).collect()
}

/// The cells `inside` marks, ascending.
fn cells_of(inside: &[bool]) -> Vec<u32> {
    inside
        .iter()
        .enumerate()
        .filter(|&(_, &x)| x)
        .map(|(c, _)| cell_of(c))
        .collect()
}

/// The cells outside `inside` that are edge neighbours of a cell inside it, ascending.
fn rim(graph: &CellGraph, inside: &[bool]) -> Vec<u32> {
    (0..graph.len())
        .filter(|&c| !inside[slot(c)] && graph.edge_neighbours(c).iter().any(|&m| inside[slot(m)]))
        .collect()
}

/// Whether the cells `cells` are connected through edge neighbours among themselves.
fn connected(graph: &CellGraph, cells: &[u32]) -> bool {
    let mut inside = vec![false; slot(graph.len())];
    for &c in cells {
        inside[slot(c)] = true;
    }
    let Some(&first) = cells.first() else {
        return true;
    };
    let mut seen = vec![false; inside.len()];
    seen[slot(first)] = true;
    let mut queue = vec![first];
    let mut count = 0;
    while let Some(c) = queue.pop() {
        count += 1;
        for m in graph.edge_neighbours(c) {
            if inside[slot(m)] && !seen[slot(m)] {
                seen[slot(m)] = true;
                queue.push(m);
            }
        }
    }
    count == cells.len()
}

/// The sum of `values` over `cells`, in their order.
fn sum_over(values: &[f64], cells: &[u32]) -> f64 {
    cells.iter().map(|&c| values[slot(c)]).sum()
}
