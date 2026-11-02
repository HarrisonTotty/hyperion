//! Spheres as the pages store them, in `f32`, built so that what they must contain they contain
//! as stored.
//!
//! A cluster's own sphere bounds its vertices, and a group's LOD sphere bounds its clusters' own
//! spheres, so the DAG's spheres nest (plan R14, Design note 3). Each centre is rounded to `f32`
//! first and each radius then measured in `f64` from that centre and rounded up, with a relative
//! margin of 2⁻²⁰ against the square root's rounding, so that containment holds of the `f32`
//! values themselves, not only of the `f64` ones they came from.

use crate::geometry::{F3, length, sub};
use crate::num::{f32_nearest, f32_up, max, min};

/// The radius's relative margin over the largest distance measured.
const MARGIN: f64 = 1.0 + 1.0 / 1_048_576.0;

/// A sphere about the page's origin, m.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Sphere {
    centre_m: [f32; 3],
    radius_m: f32,
}

impl Sphere {
    /// A sphere from its centre and radius, m.
    #[must_use]
    pub const fn new(centre_m: [f32; 3], radius_m: f32) -> Self {
        Self { centre_m, radius_m }
    }

    /// Its centre about the page's origin, m.
    #[must_use]
    pub const fn centre_m(&self) -> [f32; 3] {
        self.centre_m
    }

    /// Its radius, m.
    #[must_use]
    pub const fn radius_m(&self) -> f32 {
        self.radius_m
    }

    /// Its centre about the page's origin, m, in `f64` (exactly).
    #[must_use]
    pub fn centre_f64(&self) -> F3 {
        self.centre_m.map(f64::from)
    }

    /// Whether it holds `inner`, to `tolerance_m`.
    #[must_use]
    pub fn contains(&self, inner: &Self, tolerance_m: f64) -> bool {
        length(sub(inner.centre_f64(), self.centre_f64())) + f64::from(inner.radius_m)
            <= f64::from(self.radius_m) + tolerance_m
    }

    /// Whether it holds the point `p`, to `tolerance_m`.
    #[must_use]
    pub fn contains_point(&self, p: F3, tolerance_m: f64) -> bool {
        length(sub(p, self.centre_f64())) <= f64::from(self.radius_m) + tolerance_m
    }

    /// Whether the two hold the same bits.
    #[must_use]
    pub fn same_bits(&self, other: &Self) -> bool {
        let bits =
            |s: &Self| [s.centre_m[0], s.centre_m[1], s.centre_m[2], s.radius_m].map(f32::to_bits);
        bits(self) == bits(other)
    }
}

/// The sphere about the centre of the points' box that holds them all.
#[must_use]
pub(crate) fn enclosing_points(points: &[F3]) -> Sphere {
    let (lo, hi) = bounds(points.iter().map(|&p| (p, p)));
    let centre = rounded_centre(lo, hi);
    let c = centre.map(f64::from);
    let radius = points.iter().map(|&p| length(sub(p, c))).fold(0.0, max);
    Sphere::new(centre, f32_up(radius * MARGIN))
}

/// The sphere about the centre of the spheres' box that holds them all.
#[must_use]
pub(crate) fn enclosing_spheres(spheres: &[Sphere]) -> Sphere {
    let (lo, hi) = bounds(spheres.iter().map(|s| {
        let c = s.centre_f64();
        let r = f64::from(s.radius_m);
        (c.map(|x| x - r), c.map(|x| x + r))
    }));
    let centre = rounded_centre(lo, hi);
    let c = centre.map(f64::from);
    let radius = spheres
        .iter()
        .map(|s| length(sub(s.centre_f64(), c)) + f64::from(s.radius_m))
        .fold(0.0, max);
    Sphere::new(centre, f32_up(radius * MARGIN))
}

fn bounds(boxes: impl Iterator<Item = (F3, F3)>) -> (F3, F3) {
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for (l, h) in boxes {
        for axis in 0..3 {
            lo[axis] = min(lo[axis], l[axis]);
            hi[axis] = max(hi[axis], h[axis]);
        }
    }
    (lo, hi)
}

fn rounded_centre(lo: F3, hi: F3) -> [f32; 3] {
    std::array::from_fn(|axis| f32_nearest(f64::midpoint(lo[axis], hi[axis])))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_points_sphere_holds_them_as_stored() {
        let points = [[0.1, 0.2, 0.3], [10.7, -3.3, 2.9], [1.0 / 3.0, 7.0, -1.0]];
        let s = enclosing_points(&points);
        assert!(points.iter().all(|&p| s.contains_point(p, 0.0)), "{s:?}");
    }

    #[test]
    fn a_spheres_sphere_holds_each_as_stored() {
        let inner = [
            Sphere::new([0.0, 0.0, 0.0], 1.0),
            Sphere::new([3.1, 0.7, -2.2], 0.3),
        ];
        let outer = enclosing_spheres(&inner);
        assert!(inner.iter().all(|s| outer.contains(s, 0.0)), "{outer:?}");
    }
}
