//! The offline builder of virtualized geometry's mesh pages (plan R14, T5.a).
//!
//! [`build()`] takes a triangle mesh, quantises it onto one grid, cuts it into clusters of at most
//! [`MAX_CLUSTER_TRIANGLES`] triangles, then repeatedly groups neighbouring clusters, simplifies
//! each group with its border locked and cuts the result into new clusters, until nothing
//! simplifies further. The clusters form a DAG whose errors are monotone and whose LOD spheres are
//! nested, so that the rule "own error ≤ τ < parent error", each projected at its group's sphere,
//! draws one crack-free cut of it with no traversal (Karis 2021, "A Deep Dive into Nanite
//! Virtualized Geometry", SIGGRAPH Advances in Real-Time Rendering). [`page::write_mesh`] writes
//! the DAG as pages in the client's page format, and [`cut`] is the CPU's twin of the GPU's cut.
//!
//! The clusteriser and the simplifier are this crate's own, in Rust, rather than meshoptimizer's
//! C++ (plan R14, T5.a's conditions): every step is integer arithmetic or IEEE 754 basic
//! operations and square roots, which every platform rounds alike and Rust never fuses or
//! reorders, and the few transcendental functions go through the exactly pinned `libm`. Nothing
//! iterates a hash map and every sort has a total order, so the same input gives the same pages,
//! byte for byte, on Linux, macOS and Windows.

pub mod build;
mod cluster;
pub mod cut;
mod geometry;
mod group;
pub mod mesh;
mod num;
pub mod obj;
pub mod page;
pub mod procedural;
mod simplify;
pub mod sphere;

pub use build::{BuildDagError, Cluster, Dag, Group, build};
pub use mesh::{MAX_ELEMENTS, NewSourceMeshError, QuantiseMeshError, SourceMesh};

/// The most triangles a cluster holds: Nanite's size, so that a triangle's index within its
/// cluster fits the visibility ID's 7 bits (plan R14, Design note 2).
pub const MAX_CLUSTER_TRIANGLES: usize = 128;

/// The most vertices a cluster holds, so that its local indices fit 8 bits (plan R14, Design
/// note 2).
pub const MAX_CLUSTER_VERTICES: usize = 256;
