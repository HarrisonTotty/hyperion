//! Cutting a set of triangles into clusters of at most [`MAX_CLUSTER_TRIANGLES`] triangles and
//! [`MAX_CLUSTER_VERTICES`] vertices.
//!
//! By recursive bisection at the median of the triangles' centroids along the axis of their
//! largest spread, into ⌈n ÷ 128⌉ parts as even as the count allows, so that clusters are compact
//! and nearly full. Centroids are compared as integer sums of grid coordinates, with the
//! triangle's place in the input breaking ties, so the cut is a function of the input alone, and
//! each cluster keeps its triangles in their input order.

use crate::mesh::QuantisedMesh;
use crate::{MAX_CLUSTER_TRIANGLES, MAX_CLUSTER_VERTICES};

#[derive(Debug, Clone, Copy)]
struct Item {
    corners: [u32; 3],
    /// Three times the centroid, in grid units.
    key: [i64; 3],
    order: usize,
}

/// `triangles` (indices into the mesh's vertices) cut into clusters.
#[must_use]
pub(crate) fn split(mesh: &QuantisedMesh, triangles: &[[u32; 3]]) -> Vec<Vec<[u32; 3]>> {
    let mut items: Vec<Item> = triangles
        .iter()
        .enumerate()
        .map(|(order, &corners)| {
            let p = corners.map(|v| mesh.grid_i64(mesh.position_of(v)));
            let key = std::array::from_fn(|axis| p[0][axis] + p[1][axis] + p[2][axis]);
            Item {
                corners,
                key,
                order,
            }
        })
        .collect();
    let mut clusters = Vec::with_capacity(triangles.len().div_ceil(MAX_CLUSTER_TRIANGLES));
    let mut scratch = Vec::with_capacity(3 * MAX_CLUSTER_TRIANGLES);
    bisect(&mut items, &mut scratch, &mut clusters);
    clusters
}

fn bisect(items: &mut [Item], scratch: &mut Vec<u32>, clusters: &mut Vec<Vec<[u32; 3]>>) {
    let n = items.len();
    if n <= MAX_CLUSTER_TRIANGLES && distinct_vertices(items, scratch) <= MAX_CLUSTER_VERTICES {
        items.sort_unstable_by_key(|item| item.order);
        clusters.push(items.iter().map(|item| item.corners).collect());
        return;
    }
    let parts = n.div_ceil(MAX_CLUSTER_TRIANGLES).max(2);
    let left = n * (parts / 2) / parts;
    let axis = widest_axis(items);
    items.select_nth_unstable_by(left, |a, b| {
        a.key[axis].cmp(&b.key[axis]).then(a.order.cmp(&b.order))
    });
    let (lower, upper) = items.split_at_mut(left);
    bisect(lower, scratch, clusters);
    bisect(upper, scratch, clusters);
}

fn distinct_vertices(items: &[Item], scratch: &mut Vec<u32>) -> usize {
    scratch.clear();
    scratch.extend(items.iter().flat_map(|item| item.corners));
    scratch.sort_unstable();
    scratch.dedup();
    scratch.len()
}

fn widest_axis(items: &[Item]) -> usize {
    let mut lo = [i64::MAX; 3];
    let mut hi = [i64::MIN; 3];
    for item in items {
        for axis in 0..3 {
            lo[axis] = lo[axis].min(item.key[axis]);
            hi[axis] = hi[axis].max(item.key[axis]);
        }
    }
    (0..3)
        .max_by_key(|&axis| (hi[axis] - lo[axis], 2 - axis))
        .expect("three axes")
}

/// The distinct vertices of a cluster's triangles, in the order they are first used.
#[must_use]
pub(crate) fn local_vertices(triangles: &[[u32; 3]]) -> Vec<u32> {
    let mut seen: Vec<u32> = Vec::with_capacity(MAX_CLUSTER_VERTICES);
    for &v in triangles.iter().flatten() {
        if !seen.contains(&v) {
            seen.push(v);
        }
    }
    seen
}

/// A cluster's triangles as indices into its [`local_vertices`].
#[must_use]
pub(crate) fn local_triangles(triangles: &[[u32; 3]], vertices: &[u32]) -> Vec<[u8; 3]> {
    triangles
        .iter()
        .map(|t| {
            t.map(|v| {
                let i = vertices
                    .iter()
                    .position(|&w| w == v)
                    .expect("a vertex of the cluster");
                u8::try_from(i).expect("a cluster holds at most 256 vertices")
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::num::ix;
    use crate::procedural;

    fn uses_every_position(mesh: &QuantisedMesh, triangles: &[[u32; 3]]) -> bool {
        let mut used = vec![false; mesh.grid.len()];
        for &v in triangles.iter().flatten() {
            used[ix(mesh.position_of(v))] = true;
        }
        used.iter().all(|&u| u)
    }

    #[test]
    fn clusters_are_full_and_cover_each_triangle_once() {
        let source = procedural::terrain(40, 30, 1.0, 3.0, 7);
        let mesh = QuantisedMesh::new(&source).unwrap();
        let clusters = split(&mesh, &mesh.triangles);
        assert_eq!(
            clusters.len(),
            mesh.triangles.len().div_ceil(MAX_CLUSTER_TRIANGLES)
        );
        let mut all: Vec<[u32; 3]> = clusters.iter().flatten().copied().collect();
        assert!(clusters.iter().all(|c| c.len() <= MAX_CLUSTER_TRIANGLES));
        assert!(
            clusters
                .iter()
                .all(|c| local_vertices(c).len() <= MAX_CLUSTER_VERTICES)
        );
        all.sort_unstable();
        let mut expected = mesh.triangles.clone();
        expected.sort_unstable();
        assert_eq!(all, expected);
        assert!(uses_every_position(&mesh, &all));
    }

    #[test]
    fn local_indices_name_the_first_use_order() {
        let triangles = [[7, 3, 9], [3, 9, 4]];
        let vertices = local_vertices(&triangles);
        assert_eq!(vertices, vec![7, 3, 9, 4]);
        assert_eq!(
            local_triangles(&triangles, &vertices),
            vec![[0, 1, 2], [1, 2, 3]]
        );
    }
}
