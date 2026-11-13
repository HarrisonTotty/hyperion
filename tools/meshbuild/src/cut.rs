//! The DAG cut: the CPU's twin of the GPU's choice of clusters (plan R14, Design note 3).
//!
//! A cluster is drawn where its own error ≤ τ < its parent error, each projected at its sphere by
//! the function selection uses, ε W ÷ (2 d tan(θ ÷ 2)) px (`screenSpaceErrorPx`, R05), with W the
//! view's width, θ its horizontal field of view and d the distance from the eye to the sphere's
//! surface. An eye within a sphere projects its error
//! without bound, so that the finest level is drawn; an error of 0 projects to 0 wherever the eye
//! is. With monotone errors and nested spheres, whether a group's clusters are replaced by those
//! made from it is the same decision in every record that holds the group, and the drawn set is
//! one crack-free cut.

use crate::geometry::{length, sub};
use crate::page::{MeshFile, Page, PageCluster};
use crate::sphere::Sphere;

/// A view the cut is made for: the eye about the page's origin and the projection's scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CutView {
    eye_m: [f64; 3],
    focal_px: f64,
}

impl CutView {
    /// A view from `eye_m` (about the page's origin, m) of `width_px` across a horizontal field
    /// of view of `fov_h_rad`, in (0, π).
    #[must_use]
    pub fn new(eye_m: [f64; 3], width_px: f64, fov_h_rad: f64) -> Self {
        Self {
            eye_m,
            focal_px: width_px / (2.0 * libm::tan(0.5 * fov_h_rad)),
        }
    }

    /// Its focal length, px: W ÷ (2 tan(θ ÷ 2)).
    #[must_use]
    pub const fn focal_px(&self) -> f64 {
        self.focal_px
    }

    /// `error_m` projected at `sphere`, px (see the module's documentation).
    #[must_use]
    pub fn projected_px(&self, error_m: f32, sphere: &Sphere) -> f64 {
        let error = f64::from(error_m);
        if error <= 0.0 {
            return 0.0;
        }
        let d = length(sub(sphere.centre_f64(), self.eye_m)) - f64::from(sphere.radius_m());
        if d <= 0.0 || error.is_infinite() {
            f64::INFINITY
        } else {
            error * self.focal_px / d
        }
    }

    /// Whether `cluster` is drawn at the tolerance `tau_px`.
    #[must_use]
    pub fn draws(&self, cluster: &PageCluster, tau_px: f64) -> bool {
        self.projected_px(cluster.own_error_m(), cluster.own_sphere()) <= tau_px
            && tau_px < self.projected_px(cluster.parent_error_m(), cluster.parent_sphere())
    }
}

/// Whether `cluster` is in the uniform cut at the object-space error `error_m`: own ≤ error <
/// parent.
#[must_use]
pub fn in_uniform_cut(cluster: &PageCluster, error_m: f64) -> bool {
    f64::from(cluster.own_error_m()) <= error_m && error_m < f64::from(cluster.parent_error_m())
}

/// The clusters of `mesh` that `keep` draws.
pub fn cut(mesh: &MeshFile, keep: impl Fn(&PageCluster) -> bool) -> Vec<&PageCluster> {
    mesh.pages()
        .iter()
        .flat_map(Page::clusters)
        .filter(|c| keep(c))
        .collect()
}
