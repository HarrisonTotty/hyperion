//! Building a mesh's cluster DAG (plan R14, T5.a; Karis 2021).
//!
//! The mesh's triangles are cut into the leaf clusters. Then, level by level, the clusters not yet
//! simplified are grouped, each group is simplified to half its triangles with its border locked,
//! and the result is cut into new clusters, which the next level takes with the clusters left over.
//! A group that cannot get below 85% of its triangles (meshoptimizer's demonstration's bound) is
//! left as it is for the next level, whose groups differ; the build stops when one cluster is left
//! or no group of a level simplifies, and the clusters left are the DAG's roots.
//!
//! - **Errors**: a leaf's own error is 0. A group's error is its measured deviation (the larger of
//!   the farthest input vertex from the simplified surface and the farthest simplified sample from
//!   the input, each against the triangles about where it collapsed to) plus the largest own error
//!   of its clusters, so that by the triangle
//!   inequality it bounds the new clusters' distance from the leaves; rounded up into `f32`, it is
//!   the parent error of the group's clusters and the own error of the clusters made from it, the
//!   same bits in both. So errors are monotone along every path of the DAG.
//! - **Spheres**: a leaf's own sphere bounds its vertices; a group's LOD sphere bounds its
//!   clusters' own spheres, and is the parent sphere of the group's clusters and the own sphere of
//!   those made from it. The new clusters' vertices are the group's (half-edge collapses add none),
//!   so each cluster lies within its own sphere, and the spheres nest.
//!
//! A cluster is then drawn where its own error ≤ τ < its parent's, each projected at its sphere,
//! and that set is one watertight cut of the DAG for every τ and view ([`crate::cut`]).

use thiserror::Error;

use crate::cluster;
use crate::group;
use crate::mesh::{QuantiseMeshError, QuantisedMesh, SourceMesh};
use crate::num::{f32_up, ix, u32_of};
use crate::simplify::{self, Simplified};
use crate::sphere::{self, Sphere};

/// The most levels the build makes: far more than a mesh of 2³² triangles needs at a halving a
/// level, so reached only if something stops simplifying without saying so.
const MAX_LEVELS: u32 = 64;

/// Why a DAG could not be built.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum BuildDagError {
    /// The mesh could not be quantised.
    #[error("the mesh could not be quantised")]
    Quantise(#[from] QuantiseMeshError),
}

/// A cluster of the DAG.
#[derive(Debug, Clone, PartialEq)]
pub struct Cluster {
    triangles: Vec<[u32; 3]>,
    level: u32,
    own_error_m: f32,
    own_sphere: Sphere,
    parent_error_m: f32,
    parent_sphere: Sphere,
    generating_group: Option<u32>,
    consuming_group: Option<u32>,
}

impl Cluster {
    /// How many triangles it holds, at most [`crate::MAX_CLUSTER_TRIANGLES`].
    #[must_use]
    pub fn triangle_count(&self) -> usize {
        self.triangles.len()
    }

    /// The level it was made at (0 for a leaf).
    #[must_use]
    pub const fn level(&self) -> u32 {
        self.level
    }

    /// Its own error, m: 0 for a leaf.
    #[must_use]
    pub const fn own_error_m(&self) -> f32 {
        self.own_error_m
    }

    /// Its own sphere: the LOD sphere of the group it was made from, or a leaf's bounds.
    #[must_use]
    pub const fn own_sphere(&self) -> Sphere {
        self.own_sphere
    }

    /// Its parent error, m: its group's error, or +∞ for a root.
    #[must_use]
    pub const fn parent_error_m(&self) -> f32 {
        self.parent_error_m
    }

    /// Its parent sphere: its group's LOD sphere, or a root's own sphere.
    #[must_use]
    pub const fn parent_sphere(&self) -> Sphere {
        self.parent_sphere
    }

    /// The group it was made from, `None` for a leaf.
    #[must_use]
    pub const fn generating_group(&self) -> Option<u32> {
        self.generating_group
    }

    /// The group it was simplified in, `None` for a root.
    #[must_use]
    pub const fn consuming_group(&self) -> Option<u32> {
        self.consuming_group
    }

    #[must_use]
    pub(crate) fn triangles(&self) -> &[[u32; 3]] {
        &self.triangles
    }
}

/// A group of the DAG: clusters simplified together into new ones.
#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    inputs: Vec<u32>,
    outputs: Vec<u32>,
    error_m: f32,
    sphere: Sphere,
    level: u32,
}

impl Group {
    /// The clusters it simplified.
    #[must_use]
    pub fn inputs(&self) -> &[u32] {
        &self.inputs
    }

    /// The clusters it made.
    #[must_use]
    pub fn outputs(&self) -> &[u32] {
        &self.outputs
    }

    /// Its error, m.
    #[must_use]
    pub const fn error_m(&self) -> f32 {
        self.error_m
    }

    /// Its LOD sphere.
    #[must_use]
    pub const fn sphere(&self) -> Sphere {
        self.sphere
    }

    /// The level it simplified (its outputs are at the next).
    #[must_use]
    pub const fn level(&self) -> u32 {
        self.level
    }
}

/// A mesh's cluster DAG.
#[derive(Debug, Clone, PartialEq)]
pub struct Dag {
    mesh: QuantisedMesh,
    clusters: Vec<Cluster>,
    groups: Vec<Group>,
}

impl Dag {
    /// Its clusters, leaves first.
    #[must_use]
    pub fn clusters(&self) -> &[Cluster] {
        &self.clusters
    }

    /// Its groups, in the order they were simplified.
    #[must_use]
    pub fn groups(&self) -> &[Group] {
        &self.groups
    }

    /// Its levels: one more than its highest cluster's.
    #[must_use]
    pub fn levels(&self) -> u32 {
        self.clusters
            .iter()
            .map(Cluster::level)
            .max()
            .map_or(0, |l| l + 1)
    }

    /// Its triangles after quantisation: the leaves'.
    #[must_use]
    pub fn leaf_triangles(&self) -> usize {
        self.mesh.triangles.len()
    }

    pub(crate) const fn mesh(&self) -> &QuantisedMesh {
        &self.mesh
    }
}

/// Builds `source`'s DAG (see the module's documentation).
///
/// # Errors
///
/// [`BuildDagError::Quantise`] if the mesh cannot be quantised: its positions span nothing, or
/// every triangle is degenerate on the grid.
pub fn build(source: &SourceMesh) -> Result<Dag, BuildDagError> {
    let threads = std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
    build_on(source, threads)
}

/// [`build`] on `threads` threads, which change nothing but the time it takes.
fn build_on(source: &SourceMesh, threads: usize) -> Result<Dag, BuildDagError> {
    let mesh = QuantisedMesh::new(source)?;
    let mut clusters: Vec<Cluster> = cluster::split(&mesh, &mesh.triangles)
        .into_iter()
        .map(|triangles| {
            let points: Vec<_> = triangles
                .iter()
                .flatten()
                .map(|&v| mesh.local_m(mesh.position_of(v)))
                .collect();
            let bounds = sphere::enclosing_points(&points);
            Cluster {
                triangles,
                level: 0,
                own_error_m: 0.0,
                own_sphere: bounds,
                parent_error_m: f32::INFINITY,
                parent_sphere: bounds,
                generating_group: None,
                consuming_group: None,
            }
        })
        .collect();
    let mut groups: Vec<Group> = Vec::new();
    let mut pending: Vec<u32> = (0..u32_of(clusters.len())).collect();
    let mut level = 0;
    while pending.len() > 1 && level < MAX_LEVELS {
        let Some(next) =
            simplify_level(&mesh, &mut clusters, &mut groups, &pending, level, threads)
        else {
            break;
        };
        pending = next;
        level += 1;
    }
    for &root in &pending {
        let c = &mut clusters[ix(root)];
        c.parent_error_m = f32::INFINITY;
        c.parent_sphere = c.own_sphere;
        c.consuming_group = None;
    }
    Ok(Dag {
        mesh,
        clusters,
        groups,
    })
}

/// One level: the clusters the next level takes, or `None` if no group simplified.
fn simplify_level(
    mesh: &QuantisedMesh,
    clusters: &mut Vec<Cluster>,
    groups: &mut Vec<Group>,
    pending: &[u32],
    level: u32,
    threads: usize,
) -> Option<Vec<u32>> {
    let lists: Vec<&[[u32; 3]]> = pending
        .iter()
        .map(|&c| clusters[ix(c)].triangles())
        .collect();
    let members = group::group(&lists, |v| mesh.position_of(v));
    let member_ids: Vec<Vec<u32>> = members
        .iter()
        .map(|m| m.iter().map(|&i| pending[i]).collect())
        .collect();
    let group_triangles: Vec<Vec<[u32; 3]>> = member_ids
        .iter()
        .map(|ids| {
            ids.iter()
                .flat_map(|&c| clusters[ix(c)].triangles.iter().copied())
                .collect()
        })
        .collect();
    let topology = Topology::new(mesh, &group_triangles);
    let results: Vec<Simplified> = in_parallel(&group_triangles, threads, |g, triangles| {
        let g = u32_of(g);
        let input = simplify::Group {
            mesh,
            triangles,
            locked: |p: u32| topology.locked[ix(p)],
            outside_neighbours: |p: u32, out: &mut Vec<u32>| topology.outside(g, p, out),
        };
        simplify::simplify(&input, triangles.len() / 2)
    });
    let mut next = Vec::with_capacity(pending.len());
    let mut simplified_any = false;
    for ((ids, triangles), result) in member_ids.into_iter().zip(&group_triangles).zip(results) {
        if 20 * result.triangles.len() > 17 * triangles.len() {
            next.extend(ids);
            continue;
        }
        simplified_any = true;
        let id = u32_of(groups.len());
        let deepest = ids
            .iter()
            .map(|&c| f64::from(clusters[ix(c)].own_error_m))
            .fold(0.0, crate::num::max);
        let error_m = f32_up(deepest + result.error_steps * mesh.step_m);
        let spheres: Vec<Sphere> = ids.iter().map(|&c| clusters[ix(c)].own_sphere).collect();
        let lod = sphere::enclosing_spheres(&spheres);
        for &c in &ids {
            let c = &mut clusters[ix(c)];
            c.parent_error_m = error_m;
            c.parent_sphere = lod;
            c.consuming_group = Some(id);
        }
        let mut outputs = Vec::new();
        for part in cluster::split(mesh, &result.triangles) {
            outputs.push(u32_of(clusters.len()));
            clusters.push(Cluster {
                triangles: part,
                level: level + 1,
                own_error_m: error_m,
                own_sphere: lod,
                parent_error_m: f32::INFINITY,
                parent_sphere: lod,
                generating_group: Some(id),
                consuming_group: None,
            });
        }
        next.extend_from_slice(&outputs);
        groups.push(Group {
            inputs: ids,
            outputs,
            error_m,
            sphere: lod,
            level,
        });
    }
    simplified_any.then_some(next)
}

/// `f` of each item and its index, in order, on `threads` threads in contiguous shares.
///
/// Each group's simplification reads only its own triangles and the level's topology, so the
/// results are the same whatever the threads. A thread's panic is carried on to the caller.
fn in_parallel<T: Sync, R: Send>(
    items: &[T],
    threads: usize,
    f: impl Fn(usize, &T) -> R + Sync,
) -> Vec<R> {
    let share = items.len().div_ceil(threads.max(1)).max(1);
    let f = &f;
    std::thread::scope(|scope| {
        let parts: Vec<_> = items
            .chunks(share)
            .enumerate()
            .map(|(k, part)| {
                scope.spawn(move || {
                    part.iter()
                        .enumerate()
                        .map(|(i, x)| f(k * share + i, x))
                        .collect::<Vec<R>>()
                })
            })
            .collect();
        parts
            .into_iter()
            .flat_map(|part| {
                part.join()
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
            })
            .collect()
    })
}

/// A level's triangles by position: which positions more than one group uses, and each
/// position's triangles, by their group.
struct Topology {
    /// Offsets into `entries` by position (one more than the grid's positions).
    start: Vec<usize>,
    /// The level's triangles about each position.
    entries: Vec<u32>,
    positions: Vec<[u32; 3]>,
    group_of: Vec<u32>,
    locked: Vec<bool>,
}

impl Topology {
    fn new(mesh: &QuantisedMesh, groups: &[Vec<[u32; 3]>]) -> Self {
        let count = mesh.grid.len();
        let mut positions = Vec::new();
        let mut group_of = Vec::new();
        let mut locked = vec![false; count];
        let mut last = vec![u32::MAX; count];
        for (g, triangles) in groups.iter().enumerate() {
            let g = u32_of(g);
            for t in triangles {
                let p = t.map(|v| mesh.position_of(v));
                for &x in &p {
                    let seen = last[ix(x)];
                    if seen != u32::MAX && seen != g {
                        locked[ix(x)] = true;
                    }
                    last[ix(x)] = g;
                }
                positions.push(p);
                group_of.push(g);
            }
        }
        let mut start = vec![0_usize; count + 1];
        for p in positions.iter().flatten() {
            start[ix(*p) + 1] += 1;
        }
        for i in 0..count {
            start[i + 1] += start[i];
        }
        let mut fill = start.clone();
        let mut entries = vec![0_u32; 3 * positions.len()];
        for (t, p) in positions.iter().enumerate() {
            for &x in p {
                entries[fill[ix(x)]] = u32_of(t);
                fill[ix(x)] += 1;
            }
        }
        Self {
            start,
            entries,
            positions,
            group_of,
            locked,
        }
    }

    /// Appends `p`'s neighbours through triangles of groups other than `g`.
    fn outside(&self, g: u32, p: u32, out: &mut Vec<u32>) {
        let range = self.start[ix(p)]..self.start[ix(p) + 1];
        for &t in &self.entries[range] {
            if self.group_of[ix(t)] != g {
                out.extend(self.positions[ix(t)].iter().filter(|&&x| x != p));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::procedural;

    #[test]
    fn the_thread_count_changes_nothing() {
        let source = procedural::torus_knot(2, 3, 128, 16, 1.0, 0.25);
        let one = build_on(&source, 1).unwrap();
        for threads in [2, 7] {
            assert_eq!(
                build_on(&source, threads).unwrap(),
                one,
                "{threads} threads"
            );
        }
    }

    #[test]
    fn one_cluster_is_its_own_root() {
        let dag = build(&procedural::terrain(4, 4, 1.0, 0.5, 1)).unwrap();
        assert_eq!(dag.clusters().len(), 1);
        assert_eq!(
            dag.groups().len(),
            0,
            "one cluster has nothing to simplify with"
        );
        let root = &dag.clusters()[0];
        assert_eq!((root.own_error_m(), root.consuming_group()), (0.0, None));
        assert!(root.parent_error_m().is_infinite());
    }
}
