//! A member class's radial profile in its cluster (plan 09, P09.T8.a; Design note 9).
//!
//! A class's density is a shape times the common taper `(1 − r² ÷ r_t²)²` inside the tidal radius
//! `r_t`, and zero outside:
//!
//! - **Core:** `(1 + r² ÷ r_c²)^(−3q′ ÷ 2)`, with `q′ = q^η` for `q = m ÷ m_TO` below 1 and `q`
//!   above (η = [`EQUIPARTITION_EXPONENT`]; Heinke et al. 2005, ApJ 625, 796's segregation by mass).
//! - **Cusp:** `(r² + ε²)^(−γ ÷ 2)` for a core-collapsed cluster, γ its drawn slope and ε = 10⁻³
//!   ly, so that the cusp stays integrable and bounded.
//! - **Plummer:** `(1 + r² ÷ a²)^(−5/2)`, the black holes' compact subsystem (P09.T9.c).
//!
//! Every shape and the taper never rise with `r`, so the profile never rises with `|x|`, `|y|` or
//! `|z|` and a cell's nearest corner bounds it. A profile may be flattened along the grid's z by an
//! axis ratio `p ≤ 1`: it is then read at `m = √(x² + y² + (z ÷ p)²)` and divided by `p`, which
//! keeps it normalised. No class of phase 2 is flattened; the galactic centre's will be.
//!
//! The profile is normalised to integrate to 1 by a fixed 64-node Gauss–Legendre quadrature (two
//! panels of 32) in `t = ln(1 + r ÷ s)`, `s` the shape's scale, in which every shape is smooth. Its
//! radial cumulative distribution is tabulated at [`CDF_KNOTS`] knots even in `t` for the inverse
//! transform, which reads it linearly in `t` between knots.

use crate::galaxy::PointLy;
use crate::galaxy::quad::{gl_panels, gl16};
use crate::math;
use crate::tables::cluster_dynamics::EQUIPARTITION_EXPONENT;

/// A cusp's softening length, ly (Design note 9).
pub const CUSP_SOFTENING: f64 = 1e-3;

/// The knots of the tabulated radial cumulative distribution.
pub const CDF_KNOTS: usize = 256;

/// The shape of a class's profile, before the taper.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProfileShape {
    /// `(1 + r² ÷ r_c²)^(−exponent)`: a core of radius `core` (ly), exponent `3q′ ÷ 2`.
    Core {
        /// The core radius, ly.
        core: f64,
        /// The exponent, `3q′ ÷ 2`.
        exponent: f64,
    },
    /// `(r² + ε²)^(−slope ÷ 2)`: a core-collapsed cluster's cusp.
    Cusp {
        /// The slope γ, 1.6–2.
        slope: f64,
    },
    /// `(1 + r² ÷ a²)^(−5/2)`: a Plummer sphere of scale `scale` (ly).
    Plummer {
        /// The Plummer scale, ly.
        scale: f64,
    },
}

impl ProfileShape {
    /// The cored shape of a class of mass ratio `q = m ÷ m_TO` (module documentation).
    #[must_use]
    pub fn cored(core: f64, q: f64) -> Self {
        let q_prime = if q < 1.0 {
            math::powf(q, EQUIPARTITION_EXPONENT)
        } else {
            q
        };
        Self::Core {
            core,
            exponent: 1.5 * q_prime,
        }
    }

    /// The shape at radius `r` (ly), without the taper.
    #[must_use]
    fn at(&self, r: f64) -> f64 {
        match *self {
            Self::Core { core, exponent } => {
                let x = r / core;
                math::powf(1.0 + x * x, -exponent)
            }
            Self::Cusp { slope } => {
                math::powf(r * r + CUSP_SOFTENING * CUSP_SOFTENING, -0.5 * slope)
            }
            Self::Plummer { scale } => {
                let x = r / scale;
                math::powf(1.0 + x * x, -2.5)
            }
        }
    }

    /// The scale of the quadrature's variable `t = ln(1 + r ÷ s)`, ly.
    #[must_use]
    fn scale(&self) -> f64 {
        match *self {
            Self::Core { core, .. } => core,
            Self::Cusp { .. } => CUSP_SOFTENING,
            Self::Plummer { scale } => scale,
        }
    }
}

/// A class's normalised radial profile, per cubic light-year (module documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct ClassProfile {
    shape: ProfileShape,
    tidal: f64,
    flattening: f64,
    /// The inverse of `4π ∫ shape × taper r² dr`; the density divides by `p` besides.
    norm: f64,
    /// `ln(1 + r_t ÷ s)`.
    t_max: f64,
    /// The cumulative mass fraction at `CDF_KNOTS + 1` knots even in `t`, from 0 to 1.
    cdf: Box<[f64]>,
}

impl ClassProfile {
    /// The profile of `shape` inside the tidal radius `tidal` (ly), flattened along z by
    /// `flattening` (1 for a sphere).
    ///
    /// # Panics
    ///
    /// If `tidal` or the shape's scale is not positive and finite, or `flattening` is not in
    /// (0, 1].
    #[must_use]
    pub fn new(shape: ProfileShape, tidal: f64, flattening: f64) -> Self {
        let s = shape.scale();
        assert!(
            tidal > 0.0 && tidal.is_finite() && s > 0.0 && s.is_finite(),
            "a profile of scale {s} ly inside {tidal} ly"
        );
        assert!(
            flattening > 0.0 && flattening <= 1.0,
            "a flattening of {flattening}"
        );
        let t_max = math::ln_1p(tidal / s);
        let shell = |t: f64| {
            let r = s * math::exp_m1(t);
            let taper = 1.0 - (r / tidal) * (r / tidal);
            4.0 * core::f64::consts::PI * r * r * shape.at(r) * taper * taper * (r + s)
        };
        let total = gl_panels(shell, &[0.0, 0.5 * t_max, t_max]);
        let mut cdf = Vec::with_capacity(CDF_KNOTS + 1);
        cdf.push(0.0);
        let mut running = 0.0;
        #[expect(
            clippy::cast_precision_loss,
            reason = "the knot count is small and exact"
        )]
        let dt = t_max / CDF_KNOTS as f64;
        for k in 0..CDF_KNOTS {
            #[expect(
                clippy::cast_precision_loss,
                reason = "a knot index is small and exact"
            )]
            let a = dt * k as f64;
            running += gl16(shell, a, a + dt);
            cdf.push(running);
        }
        let last = running;
        for c in &mut cdf {
            *c /= last;
        }
        Self {
            shape,
            tidal,
            flattening,
            norm: 1.0 / total,
            t_max,
            cdf: cdf.into_boxed_slice(),
        }
    }

    /// The shape.
    #[must_use]
    pub const fn shape(&self) -> ProfileShape {
        self.shape
    }

    /// The tidal radius, ly.
    #[must_use]
    pub const fn tidal_radius(&self) -> f64 {
        self.tidal
    }

    /// The normalised density at radius `r` (ly) of an unflattened profile, per cubic light-year;
    /// for a flattened one, at ellipsoidal radius `r`.
    #[must_use]
    pub fn density_at_radius(&self, r: f64) -> f64 {
        let r = r.abs();
        if r >= self.tidal {
            return 0.0;
        }
        let taper = 1.0 - (r / self.tidal) * (r / self.tidal);
        self.norm * self.shape.at(r) * taper * taper / self.flattening
    }

    /// The normalised density at the local position `p` (ly from the cluster's centre).
    #[must_use]
    pub fn density(&self, p: &PointLy) -> f64 {
        let z = p.z / self.flattening;
        self.density_at_radius((p.x * p.x + p.y * p.y + z * z).sqrt())
    }

    /// The share of the class inside radius `r` (ly; ellipsoidal radius if flattened), by the
    /// tabulated distribution read linearly in `t`.
    #[must_use]
    pub fn cumulative(&self, r: f64) -> f64 {
        if r <= 0.0 {
            return 0.0;
        }
        if r >= self.tidal {
            return 1.0;
        }
        let var = math::ln_1p(r / self.shape.scale());
        #[expect(
            clippy::cast_precision_loss,
            reason = "the knot count is small and exact"
        )]
        let place = var / self.t_max * CDF_KNOTS as f64;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "place is in [0, CDF_KNOTS) here"
        )]
        let knot = (place.floor() as usize).min(CDF_KNOTS - 1);
        #[expect(
            clippy::cast_precision_loss,
            reason = "a knot index is small and exact"
        )]
        let frac = place - knot as f64;
        self.cdf[knot] + frac * (self.cdf[knot + 1] - self.cdf[knot])
    }

    /// The radius (ly; ellipsoidal if flattened) below which a share `u` of the class lies: the
    /// inverse transform, for `u` in [0, 1].
    #[must_use]
    pub fn radius_quantile(&self, u: f64) -> f64 {
        let u = u.clamp(0.0, 1.0);
        let k = self.cdf.partition_point(|&c| c <= u).clamp(1, CDF_KNOTS) - 1;
        let (lo, hi) = (self.cdf[k], self.cdf[k + 1]);
        let f = if hi > lo { (u - lo) / (hi - lo) } else { 0.0 };
        #[expect(
            clippy::cast_precision_loss,
            reason = "a knot index is small and exact"
        )]
        let t = (k as f64 + f) / CDF_KNOTS as f64 * self.t_max;
        (self.shape.scale() * math::exp_m1(t)).min(self.tidal)
    }

    /// The bytes it owns on the heap.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        size_of_val(&*self.cdf)
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::stats::{ALPHA, assert_p_value, ks_one_sample};

    use super::*;
    use crate::galaxy::quad::gl32;
    use crate::rng::{ObjectKey, Seed, Stream, tags};

    fn shapes() -> [ProfileShape; 5] {
        [
            ProfileShape::cored(2.0, 1.0),
            ProfileShape::cored(2.0, 0.2),
            ProfileShape::cored(0.3, 3.0),
            ProfileShape::Cusp { slope: 1.8 },
            ProfileShape::Plummer { scale: 1.5 },
        ]
    }

    /// A fine independent quadrature: 64 panels of 32 nodes in t.
    fn fine_total(profile: &ClassProfile) -> f64 {
        let scale = profile.shape.scale();
        let t_max = profile.t_max;
        let panels = 64;
        (0..panels)
            .map(|k| {
                let from = t_max * f64::from(k) / f64::from(panels);
                let to = t_max * f64::from(k + 1) / f64::from(panels);
                gl32(
                    |t| {
                        let r = scale * math::exp_m1(t);
                        4.0 * core::f64::consts::PI
                            * r
                            * r
                            * profile.density_at_radius(r)
                            * (r + scale)
                            * profile.flattening
                    },
                    from,
                    to,
                )
            })
            .sum()
    }

    #[test]
    fn every_profile_integrates_to_one_and_never_rises() {
        for shape in shapes() {
            for flattening in [1.0, 0.5] {
                let profile = ClassProfile::new(shape, 60.0, flattening);
                let total = fine_total(&profile);
                assert!((total - 1.0).abs() < 1e-9, "{shape:?}: {total}");
                let mut last = f64::INFINITY;
                for k in 0..=6_000 {
                    let r = f64::from(k) * 0.01;
                    let d = profile.density_at_radius(r);
                    assert!(d <= last, "{shape:?} rises at {r}");
                    last = d;
                }
                assert!(profile.density_at_radius(60.0).abs() < 1e-300);
            }
        }
    }

    #[test]
    fn the_inverse_transform_reproduces_the_profile() {
        let mut stream = Stream::open(
            Seed::new(0x0908),
            tags::SELFTEST_STREAM,
            ObjectKey::galaxy(),
        );
        for shape in shapes() {
            let profile = ClassProfile::new(shape, 60.0, 1.0);
            let mut radii: Vec<f64> = (0..20_000)
                .map(|_| profile.radius_quantile(stream.uniform()))
                .collect();
            // Against the exact distribution by the fine quadrature, not the table.
            let s = shape.scale();
            let exact = |r: f64| {
                if r <= 0.0 {
                    return 0.0;
                }
                let top = math::ln_1p(r.min(60.0) / s);
                gl_panels(
                    |t| {
                        let x = s * math::exp_m1(t);
                        4.0 * core::f64::consts::PI * x * x * profile.density_at_radius(x) * (x + s)
                    },
                    &[0.0, 0.25 * top, 0.5 * top, 0.75 * top, top],
                )
            };
            let ks = ks_one_sample(&mut radii, exact);
            assert_p_value(&format!("{shape:?}"), ks.p_value, ALPHA);
        }
    }

    #[test]
    fn a_flattened_profile_is_thinner_in_z() {
        let profile = ClassProfile::new(ProfileShape::cored(2.0, 1.0), 60.0, 0.5);
        let along = profile.density(&PointLy::new(4.0, 0.0, 0.0));
        let up = profile.density(&PointLy::new(0.0, 0.0, 2.0));
        assert!((along / up - 1.0).abs() < 1e-12);
    }
}
