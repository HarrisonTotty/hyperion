//! The boxy bulge's two-dimensional expansion `MGE_BOXY` (plan 15, P15.T3.b), run from
//! `manifests/mge.refit.toml` with the one-dimensional refit ([`super::refit`]).
//!
//! P15.T3.c puts into plan 02's potential the bulge's own body made axisymmetric in the plane,
//! `exp(−((R ÷ s)^c + (|z| ÷ h)^c)^(1 ÷ c))`, with `c` its boxiness, `h` its vertical scale and
//! `s = √((a² + b²) ÷ 2)` of its in-plane scales (ruled 2026-10-23, decision-p15-revalidation Q8).
//! It is the sim's boxy bulge (`galaxy::fields::bulge::BoxyBulge`), whose density falls
//! exponentially along each axis (Wegg and Gerhard 2013, MNRAS 435, 1874) and is boxy for `c` of
//! 3–4. In units of `s` and `h` it is the unit body [`density`], `exp(−(R^c + |z|^c)^(1 ÷ c))`.
//! This module fits it for each `c` of the manifest as `Σ w exp(−(R² + z² ÷ q²) ÷ 2σ²)`, with 16
//! terms, each with its own weight `w ≥ 0`, width `σ` and axis ratio `q` (`q > 1` is prolate).
//!
//! # What such a sum can follow
//!
//! Write `u = R²` and `v = z²`. Each term `w exp(−αu − βv)` is log-affine in `(u, v)`, so a sum of
//! them with non-negative weights is log-convex there, and on each circle `u + v = r²` its `ln ρ̂`
//! at the diagonal is at most the mean of its values on the two axes. The unit body's `ln ρ =
//! −(u^(c÷2) + v^(c÷2))^(1 ÷ c)` is higher on the diagonal than that mean by `r (1 − 2^(1 ÷ c −
//! 1 ÷ 2))` for every `c > 2` (and concave along those circles for `2 < c ≤ 4`): the body is
//! denser on the diagonal than on the axes. So the expansion cannot follow the body's boxiness.
//! At best it is flat along those circles, and its error in `ln ρ` is then at least `½ r (1 −
//! 2^(1 ÷ c − 1 ÷ 2))` at radius `r`. Over a dictionary of 1,250 terms the best mass-weighted rms
//! relative density error is 12.8% at `c` = 3 and 19.2% at `c` = 4, against plan 15's 3%. Main
//! ruled that miss structural on 2026-10-23 (issue #94): the weights stay non-negative, the fit is
//! weighted towards the mass inside spheres and the midplane force, which the potential reads, and
//! the header stays provisional by P15.T3.b with the density figure in its acceptance line.
//!
//! # The fit
//!
//! The grid is polar: `radii` log-spaced spherical radii on `range` by `angles` latitudes at the
//! Gauss–Legendre nodes of `[0, π ÷ 2]`, which make the latitude averages below accurate. Its
//! residuals are, in order:
//!
//! - one per grid point, `(ρ̂ ÷ ρ − 1) √m`, where `m` is the point's share of the grid's mass: the
//!   mass-weighted relative density error. Out where the body is faint, `√m ÷ ρ` is large, so an
//!   expansion that is far denser than the body there is held back;
//! - one per radius out to `monopole_reach`, the latitude average of `ρ̂` over that of `ρ`, less 1,
//!   times `monopole_weight`: the monopole, which holds the mass inside every sphere;
//! - one per radius of [`FORCE_RADII`], `force_weight (F̂ ÷ F − 1)` of the inward pull in the
//!   plane, `F` by [`multipole_force`] and `F̂` by the sim's [`Gaussian`]s;
//! - the central density, `centre_weight (Σ w − 1)`, and the whole mass, `mass_weight (M̂ ÷ M −
//!   1)` with `M` by [`total_mass`]: without them a term could hide a spike inside the grid's
//!   first radius or a vast, faint halo outside its last.
//!
//! The monopole, force and mass rows are main's ruling of 2026-10-23: the fit is weighted
//! towards the mass inside spheres and the force at six radii.
//!
//! The seed is the one-dimensional `MGE_EXP` of the refit, which is the `c = 2` body, the
//! spherical exponential, when every `q` is 1. Its 14 terms keep their widths with `q = 1`, and
//! their weights are solved by [`nnls`] on the residuals above. Then the two that carry the most
//! mass, `w σ³`, are each split into two of half the weight with `q` = `split` and 1 ÷ `split`.
//! That makes 16, every one with a weight, and breaks the symmetry between oblate and prolate;
//! solving the weights after the split would leave one of each pair at zero, since the two are
//! nearly the same column, and a term of zero weight gives Levenberg–Marquardt no gradient in its
//! width or `q`. Levenberg–Marquardt ([`levenberg_marquardt`]) moves the weights, the ln widths
//! and the ln axis ratios together, with an analytic Jacobian for the density and forward
//! differences for the forces. The weights are held non-negative by its projected step and the
//! widths and axis ratios within the manifest's ranges, so that a term whose weight has gone to 0
//! cannot wander off. At the end the terms are sorted by width, ascending, the first among equals
//! first.
//!
//! # Figures
//!
//! [`figures`] measures each row against the profile itself: the mass-weighted rms relative
//! density error over the whole body, the mass inside spheres against [`enclosed_mass`], the
//! whole mass against [`total_mass`], and the inward force in the plane against
//! [`multipole_force`], which the tests hold to [`midplane_force`]'s brute-force integral. The
//! expansion's mass and force are those of the sim's [`Gaussian`]s, which T3.c builds from the
//! table.
//!
//! Every boxiness `c` here is the dimensionless exponent of the body, positive, 2 for the sphere;
//! every length is in the body's scale lengths.

use core::f64::consts::PI;
use std::fmt::Write as _;

use hyperion_sim::galaxy::consts::G;
use hyperion_sim::galaxy::potential::mge::Gaussian;
use hyperion_sim::galaxy::quad::{gl16, gl32};
use hyperion_sim::math;
use hyperion_sim::tables::gauss_legendre::{GL32_NODES, GL32_WEIGHTS};
use hyperion_sim::units::{LightYears, SolarMasses};

use super::refit::{count, iterations, largest, non_negative, positive, positives, range};
use super::{WIDTHS, index};
use crate::emit::literal;
use crate::manifest::{Manifest, ManifestParamError};
use crate::optimise::{levenberg_marquardt, nnls};

/// The number of terms in each row of `MGE_BOXY`, which its format fixes.
pub const TERMS: usize = 16;

/// The number of boxinesses `MGE_BOXY` holds, which its format fixes.
pub const ROWS: usize = 3;

/// The radii of the enclosed-mass figure, in scale lengths (plan 15's P15.T3.b).
pub const MASS_RADII: [f64; 4] = [0.5, 1.0, 2.0, 4.0];

/// The radii of the midplane-force figure and the fit's force rows, in scale lengths: six, from
/// inside the body's core to well outside it.
pub const FORCE_RADII: [f64; 6] = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0];

/// The largest radius, width or axis ratio a manifest may give, in scale lengths: past 100 the
/// body's density underflows near its axes, and the mass of a term stays finite below it.
const LARGEST: f64 = 100.0;

/// One term of the expansion: `weight × exp(−(R² + z² ÷ q²) ÷ 2 width²)`.
///
/// Lengths are in the body's scale lengths. The emitted table names the sim's `BoxyTerm`
/// (P15.T3.c), which has these fields.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Term {
    /// The weight: the term's density at the centre, in units of the body's; non-negative.
    pub weight: f64,
    /// The width along R, in scale lengths, within the manifest's `boxy_widths`.
    pub width: f64,
    /// The axis ratio, the width along z over the width along R, within the manifest's
    /// `boxy_axis_ratios`.
    pub q: f64,
}

impl Term {
    /// The term's mass, `w (2π)^(3÷2) σ³ q`, in units of the body's central density times a cubic
    /// scale length.
    #[must_use]
    pub fn mass(&self) -> f64 {
        self.weight * unit_mass(self.width, self.q)
    }
}

/// The mass of a term of unit weight, `(2π)^(3÷2) σ³ q`.
#[must_use]
fn unit_mass(width: f64, q: f64) -> f64 {
    let two_pi = 2.0 * PI;
    two_pi * two_pi.sqrt() * width * width * width * q
}

/// The boxy fit's parameters, from the refit's manifest by [`BoxyParams::from_manifest`], which
/// checks them.
#[derive(Debug, Clone, PartialEq)]
pub struct BoxyParams {
    /// The boxinesses `c`, ascending, one row of `MGE_BOXY` each.
    boxiness: [f64; ROWS],
    /// The grid's spherical radii are log-spaced on this range, in scale lengths, at most 100.
    range: (f64, f64),
    /// The number of the grid's radii, at least [`TERMS`].
    radii: usize,
    /// The number of the grid's latitudes, at least 3, so that the density rows alone outnumber
    /// the fit's 48 parameters.
    angles: usize,
    /// The weight of each monopole residual, at least 0.
    monopole_weight: f64,
    /// The weight of each force residual, at least 0.
    force_weight: f64,
    /// The monopole rows stop at this radius, in scale lengths: beyond it the body's exponential
    /// tail holds no mass that matters, and a Gaussian sum, which falls faster, would only be
    /// pulled wide by it.
    monopole_reach: f64,
    /// The weight of the central-density residual, `Σ w − 1`.
    centre_weight: f64,
    /// The weight of the total-mass residual, `Σ w (2π)^(3÷2) σ³ q ÷ M − 1`.
    mass_weight: f64,
    /// The range a term's width may take, in scale lengths, at most 100.
    widths: (f64, f64),
    /// The range a term's axis ratio may take, at most 100, holding `1 ÷ split` and `split`.
    axis_ratios: (f64, f64),
    /// The seed's split of `q`, above 1.
    split: f64,
    /// Levenberg–Marquardt's iterations for each row.
    iterations: u32,
}

impl BoxyParams {
    /// The manifest's keys that the boxy fit reads.
    pub(super) const KEYS: [&'static str; 14] = [
        "boxy_terms",
        "boxy_boxiness",
        "boxy_range",
        "boxy_radii",
        "boxy_angles",
        "boxy_monopole_weight",
        "boxy_force_weight",
        "boxy_monopole_reach",
        "boxy_centre_weight",
        "boxy_mass_weight",
        "boxy_widths",
        "boxy_axis_ratios",
        "boxy_split",
        "boxy_iterations",
    ];

    /// The boxy fit's parameters of the refit's manifest.
    ///
    /// # Errors
    ///
    /// [`ManifestParamError`] naming a `boxy_*` key that is missing, of the wrong kind or out of
    /// its range, `boxy_terms` other than [`TERMS`], or a split of `q` outside the axis ratios'
    /// range.
    pub fn from_manifest(manifest: &Manifest) -> Result<Self, ManifestParamError> {
        if count(manifest, "boxy_terms", 0)? != TERMS {
            return Err(ManifestParamError::new(
                "boxy_terms",
                &format!("{TERMS}, the terms of each row of `MGE_BOXY`"),
            ));
        }
        let boxiness = match positives(manifest, "boxy_boxiness", ROWS).as_deref() {
            Some(&[a, b, c]) if a < b && b < c => [a, b, c],
            _ => {
                return Err(ManifestParamError::new(
                    "boxy_boxiness",
                    &format!("an array of {ROWS} positive numbers, ascending"),
                ));
            }
        };
        let capped = |key: &str| {
            let (lo, hi) = range(manifest, key)?;
            if hi <= LARGEST {
                Ok((lo, hi))
            } else {
                Err(ManifestParamError::new(
                    key,
                    &format!("a range that ends at {LARGEST} or below"),
                ))
            }
        };
        let monopole_reach = positive(manifest, "boxy_monopole_reach")?;
        if monopole_reach > LARGEST {
            return Err(ManifestParamError::new(
                "boxy_monopole_reach",
                &format!("a radius of {LARGEST} or below"),
            ));
        }
        let axis_ratios = capped("boxy_axis_ratios")?;
        let split = positive(manifest, "boxy_split")?;
        if !(split > 1.0 && axis_ratios.0 <= 1.0 / split && split <= axis_ratios.1) {
            return Err(ManifestParamError::new(
                "boxy_split",
                "a number above 1 whose inverse and itself lie within `boxy_axis_ratios`",
            ));
        }
        Ok(Self {
            boxiness,
            range: capped("boxy_range")?,
            radii: count(manifest, "boxy_radii", TERMS)?,
            angles: count(manifest, "boxy_angles", 3)?,
            monopole_weight: non_negative(manifest, "boxy_monopole_weight")?,
            force_weight: non_negative(manifest, "boxy_force_weight")?,
            monopole_reach,
            centre_weight: non_negative(manifest, "boxy_centre_weight")?,
            mass_weight: non_negative(manifest, "boxy_mass_weight")?,
            widths: capped("boxy_widths")?,
            axis_ratios,
            split,
            iterations: iterations(manifest, "boxy_iterations", 0)?,
        })
    }

    /// The boxinesses, ascending.
    #[must_use]
    pub fn boxiness(&self) -> [f64; ROWS] {
        self.boxiness
    }
}

/// The unit body `exp(−(R^c + |z|^c)^(1 ÷ c))` at `(R, z)` in scale lengths, for boxiness `c > 0`:
/// 1 at the centre.
#[must_use]
pub fn density(c: f64, r_cyl: f64, z: f64) -> f64 {
    let sum = math::powf(r_cyl.abs(), c) + math::powf(z.abs(), c);
    if sum > 0.0 {
        math::exp(-math::powf(sum, 1.0 / c))
    } else {
        1.0
    }
}

/// `N(β) = (cos^c β + sin^c β)^(1 ÷ c)` at latitude `beta` in `[0, π ÷ 2]`: the body's density
/// along that direction is `exp(−N r)`.
#[must_use]
fn direction(c: f64, beta: f64) -> f64 {
    let (sin, cos) = math::sin_cos(beta);
    math::powf(math::powf(cos.abs(), c) + math::powf(sin.abs(), c), 1.0 / c)
}

/// `γ(3, x) = ∫₀ˣ t² e^(−t) dt`: its series below 1, where the closed form `2 − e^(−x)(x² + 2x + 2)`
/// cancels, and the closed form from there.
#[must_use]
fn gamma_three(x: f64) -> f64 {
    if x < 1.0 {
        // Σₖ (−1)ᵏ x^(k+3) ÷ (k! (k + 3)): each power −x ÷ (k + 1) of the last; 25 terms reach
        // 10⁻²⁵ of the first at x = 1.
        let mut power = x * x * x;
        let mut sum = 0.0;
        for k in 0..25_u32 {
            let kf = f64::from(k);
            sum += power / (kf + 3.0);
            power *= -x / (kf + 1.0);
        }
        sum
    } else {
        2.0 - math::exp(-x) * (x * x + 2.0 * x + 2.0)
    }
}

/// The latitude panels for the body's integrals: the integrand changes fastest near the axes.
const LATITUDE_EDGES: [f64; 5] = [0.0, PI / 8.0, PI / 4.0, 3.0 * PI / 8.0, PI / 2.0];

/// The body's mass inside the sphere of radius `r ≥ 0` (scale lengths), for boxiness `c > 0`, by a
/// direct integral of [`density`].
///
/// Along each direction the radial integral is `∫₀ʳ ρ² e^(−Nρ) dρ = γ(3, N r) ÷ N³`, and the
/// latitude integral `4π ∫₀^(π÷2) cos β (…) dβ` is taken by [`gl32`] on four panels.
#[must_use]
pub fn enclosed_mass(c: f64, r: f64) -> f64 {
    LATITUDE_EDGES.windows(2).fold(0.0, |sum, panel| {
        sum + gl32(
            |beta| {
                let n = direction(c, beta);
                math::cos(beta) * gamma_three(n * r) / (n * n * n)
            },
            panel[0],
            panel[1],
        )
    }) * 4.0
        * PI
}

/// The body's whole mass for boxiness `c > 0`, in units of its central density times a cubic
/// scale length.
///
/// It is `8π ∫₀^(π÷2) cos β ÷ N³ dβ`, by [`gl32`] on four panels: `8π` at `c` = 2, and `3! V(c)`
/// with `V` the unit body's volume (`BoxyBulge::unit_volume` in the sim), which the tests check.
#[must_use]
pub fn total_mass(c: f64) -> f64 {
    LATITUDE_EDGES.windows(2).fold(0.0, |sum, panel| {
        sum + gl32(
            |beta| {
                let n = direction(c, beta);
                math::cos(beta) / (n * n * n)
            },
            panel[0],
            panel[1],
        )
    }) * 8.0
        * PI
}

/// The inward pull in the plane at radius `r_cyl > 0` (scale lengths) of the body of boxiness `c >
/// 0`, `G` = 1: the brute-force integral of `ρ(x') (x₀ − x') · R̂ ÷ |x₀ − x'|³` over all space.
///
/// Around the field point `x₀ = (R, 0, 0)` the volume element is `s² ds dΩ`, which cancels the
/// kernel's `1 ÷ s²`: the force is `∫ dΩ n ∫₀^∞ ρ(x₀ + s n) ds`, with no singular point. With `n =
/// (cos θ, sin θ cos φ, sin θ sin φ)` and the body's symmetry in y and z, the inward pull is
/// `−4 ∫₀^(π÷2) dφ ∫₀^π sin θ cos θ dθ ∫₀^∞ ρ ds`. Each integral is taken by [`gl16`] on fixed
/// panels: φ on two, θ on six that narrow towards the ray through the centre, and s on panels at
/// fractions of `R` and then at `R` plus 1 to 64 scale lengths, beyond which at most about
/// `e^(−54)` of the density is left (the diagonal's, at `c` = 4).
/// About 0.2 s a point; the tests read it, and the fit and the figures read [`multipole_force`].
#[must_use]
pub fn midplane_force(c: f64, r_cyl: f64) -> f64 {
    let theta_edges = [0.0, 0.5, 0.75, 0.875, 0.9375, 0.968_75, 1.0].map(|f| f * PI);
    let mut s_edges: Vec<f64> = [0.0, 0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 3.0]
        .iter()
        .map(|f| f * r_cyl)
        .chain([1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0].map(|d| r_cyl + d))
        .collect();
    s_edges.sort_by(f64::total_cmp);
    s_edges.dedup_by(|a, b| a.total_cmp(b).is_eq());
    let ray = |theta: f64, phi: f64| {
        let (sin_t, cos_t) = math::sin_cos(theta);
        let (sin_p, cos_p) = math::sin_cos(phi);
        s_edges.windows(2).fold(0.0, |sum, panel| {
            sum + gl16(
                |s| {
                    let x = r_cyl + s * cos_t;
                    let y = s * sin_t * cos_p;
                    let z = s * sin_t * sin_p;
                    density(c, (x * x + y * y).sqrt(), z)
                },
                panel[0],
                panel[1],
            )
        })
    };
    let polar = |phi: f64| {
        theta_edges.windows(2).fold(0.0, |sum, panel| {
            sum + gl16(
                |theta| {
                    let (sin, cos) = math::sin_cos(theta);
                    sin * cos * ray(theta, phi)
                },
                panel[0],
                panel[1],
            )
        })
    };
    -4.0 * (gl16(polar, 0.0, PI / 4.0) + gl16(polar, PI / 4.0, PI / 2.0))
}

/// The highest Legendre order of [`multipole_force`], 64: its even orders 0 to 64 enter.
const MULTIPOLE_ORDER: usize = 64;

/// The panels in `μ = z ÷ r` on `[0, 1]` for [`multipole_force`]: the body's density has `|μ|^c`
/// at the plane and `(1 − μ²)^(c÷2)` at the axis.
const MU_EDGES: [f64; 4] = [0.0, 0.5, 0.9, 1.0];

/// The sim's 32-point Gauss–Legendre rule mapped onto each panel between `edges`: `(node,
/// weight)`, panel by panel.
#[must_use]
fn panels(edges: &[f64]) -> Vec<(f64, f64)> {
    edges
        .windows(2)
        .flat_map(|panel| {
            let (mid, half) = (
                f64::midpoint(panel[0], panel[1]),
                0.5 * (panel[1] - panel[0]),
            );
            GL32_NODES
                .iter()
                .zip(GL32_WEIGHTS)
                .map(move |(&x, w)| (mid + half * x, half * w))
        })
        .collect()
}

/// The inward pull in the plane at radius `r_cyl > 0` (scale lengths) of the body of boxiness `c >
/// 0`, `G` = 1, by the multipole expansion of its potential (Binney and Tremaine 2008, *Galactic
/// Dynamics*, 2nd ed., §2.4) to Legendre order 64; 0 at the centre, by symmetry.
///
/// It is fast and, to its order, exact; the tests hold it to [`midplane_force`]'s brute-force
/// integral. With `μ = z ÷ r` and the moments `mₗ(r) = ∫₀¹ ρ(r, μ) Pₗ(μ) dμ`, of even `l` only
/// since the body is even in z, the potential is
///
/// ```text
/// Φ = −4π Σₗ Pₗ(μ) [r^(−(l+1)) ∫₀ʳ mₗ r'^(l+2) dr' + rˡ ∫ᵣ^∞ mₗ r'^(1−l) dr'],
/// ```
///
/// and its radial derivative in the plane, the inward pull, is
///
/// ```text
/// 4π Σₗ Pₗ(0) [(l + 1) ∫₀ʳ mₗ (r' ÷ r)^(l+2) dr' − l ∫ᵣ^∞ mₗ (r ÷ r')^(l−1) dr'],
/// ```
///
/// the boundary terms of the derivative cancelling. The `μ` integrals are by the 32-point rule on
/// three panels, the radial ones by the same rule on panels to `r + 40`.
#[must_use]
pub fn multipole_force(c: f64, r_cyl: f64) -> f64 {
    if r_cyl <= 0.0 {
        return 0.0;
    }
    let nodes = panels(&MU_EDGES);
    let orders = MULTIPOLE_ORDER / 2 + 1;
    // Pₗ(μ) at each node for even l, by the recurrence (k + 1) Pₖ₊₁ = (2k + 1) μ Pₖ − k Pₖ₋₁.
    let legendre: Vec<Vec<f64>> = nodes
        .iter()
        .map(|&(mu, _)| {
            let mut even = Vec::with_capacity(orders);
            let (mut p0, mut p1) = (1.0, mu);
            even.push(1.0);
            for k in 1..MULTIPOLE_ORDER {
                let kf = index(k);
                let p2 = ((2.0 * kf + 1.0) * mu * p1 - kf * p0) / (kf + 1.0);
                p0 = p1;
                p1 = p2;
                if (k + 1) % 2 == 0 {
                    even.push(p1);
                }
            }
            even
        })
        .collect();
    let moments = |r: f64| -> Vec<f64> {
        let mut m = vec![0.0; orders];
        for (&(mu, w), p) in nodes.iter().zip(&legendre) {
            let rho = w * density(c, r * (1.0 - mu * mu).sqrt(), r * mu);
            for (ml, &pl) in m.iter_mut().zip(p) {
                *ml += rho * pl;
            }
        }
        m
    };
    // Every order's inner integral, `∫₀ʳ mₗ (r' ÷ r)^(l+2) dr'`, and outer one, `∫ᵣ^∞ mₗ (r ÷
    // r')^(l−1) dr'`, the powers stepped by two orders at a time.
    let mut inner = vec![0.0; orders];
    for (rp, w) in panels(&[0.0, r_cyl / 8.0, r_cyl / 4.0, r_cyl / 2.0, r_cyl]) {
        let ratio = rp / r_cyl;
        let mut power = ratio * ratio;
        for (sum, ml) in inner.iter_mut().zip(moments(rp)) {
            *sum += w * ml * power;
            power *= ratio * ratio;
        }
    }
    let mut outer = vec![0.0; orders];
    for (rp, w) in panels(&[0.0, 0.5, 2.0, 6.0, 15.0, 40.0].map(|d| r_cyl + d)) {
        let ratio = r_cyl / rp;
        let mut power = 1.0 / ratio;
        for (sum, ml) in outer.iter_mut().zip(moments(rp)) {
            *sum += w * ml * power;
            power *= ratio * ratio;
        }
    }
    let mut pull = 0.0;
    let mut legendre_at_zero = 1.0;
    for (j, (a, b)) in inner.iter().zip(&outer).enumerate() {
        let l = index(2 * j);
        pull += legendre_at_zero * ((l + 1.0) * a - l * b);
        legendre_at_zero *= -(l + 1.0) / (l + 2.0);
    }
    4.0 * PI * pull
}

/// The `n`-point Gauss–Legendre rule on `[−1, 1]`: `(node, weight)` from the largest node down.
///
/// The nodes are by Newton's method on `Pₙ` from `cos(π (i + ¾) ÷ (n + ½))`, the leading term of
/// Abramowitz and Stegun's (1964) 22.16.6 for its zeros, twelve steps, which converge to
/// rounding; the weights are `2 ÷ ((1 − x²) Pₙ'(x)²)`, their 25.4.29, which the 1964 printing
/// misprints as `2 ÷ (1 − x)²…` (DLMF 3.5.21 has it right). The fit's grid needs 12 latitudes,
/// for which the sim has no table.
#[must_use]
fn gauss_legendre(n: usize) -> Vec<(f64, f64)> {
    let legendre = |x: f64| {
        let (mut p0, mut p1) = (1.0, x);
        for k in 1..n {
            let kf = index(k);
            let p2 = ((2.0 * kf + 1.0) * x * p1 - kf * p0) / (kf + 1.0);
            p0 = p1;
            p1 = p2;
        }
        // Pₙ and Pₙ' = n (x Pₙ − Pₙ₋₁) ÷ (x² − 1).
        (p1, index(n) * (x * p1 - p0) / (x * x - 1.0))
    };
    (0..n)
        .map(|i| {
            let mut x = math::cos(PI * (index(i) + 0.75) / (index(n) + 0.5));
            for _ in 0..12 {
                let (p, d) = legendre(x);
                x -= p / d;
            }
            let (_, d) = legendre(x);
            (x, 2.0 / ((1.0 - x * x) * d * d))
        })
        .collect()
}

/// One point of the fit's grid.
#[derive(Debug, Clone, Copy)]
struct Point {
    r2: f64,
    z2: f64,
    /// `√m ÷ ρ`, the density residual's factor on `ρ̂`.
    scale: f64,
    /// `√m`, the density residual's target.
    root_mass: f64,
}

/// The fit's grid for one boxiness: its points, radius by radius with the latitudes within, the
/// latitudes' averaging weights, the monopole factor `monopole_weight ÷ ρ̄` of each radius within
/// the reach, each force radius with its factor `force_weight ÷ F` ([`multipole_force`]), and the
/// central density's and the total mass's weights.
struct Grid {
    points: Vec<Point>,
    average: Vec<f64>,
    monopole: Vec<f64>,
    monopole_weight: f64,
    forces: Vec<(f64, f64)>,
    force_weight: f64,
    centre_weight: f64,
    mass_weight: f64,
    /// The body's whole mass, [`total_mass`].
    mass: f64,
}

/// The forward-difference step of the force rows' derivatives, in ln width and ln q.
const FORCE_STEP: f64 = 1e-6;

/// The inward pull in the plane, `G` = 1, of a term of unit weight, width `width` and axis ratio
/// `q`, at each of `radii`: the sim's [`Gaussian`] of mass `(2π)^(3÷2) σ³ q`.
///
/// # Panics
///
/// If the width or q is not positive and finite, which the fit's bounds rule out.
#[must_use]
fn unit_pulls(width: f64, q: f64, radii: &[(f64, f64)]) -> Vec<f64> {
    let gaussian = Gaussian::new(
        SolarMasses::new(unit_mass(width, q)),
        LightYears::new(width),
        q,
    )
    .expect("a width and q within the manifest's checked bounds give a finite, positive mass");
    radii
        .iter()
        .map(|&(r, _)| gaussian.v_circ_sq(LightYears::new(r)) / (G * r))
        .collect()
}

impl Grid {
    #[must_use]
    fn new(c: f64, params: &BoxyParams) -> Self {
        let latitudes: Vec<(f64, f64)> = gauss_legendre(params.angles)
            .iter()
            .map(|&(x, w)| (0.25 * PI * (x + 1.0), 0.25 * PI * w))
            .collect();
        let total = latitudes
            .iter()
            .fold(0.0, |sum, &(beta, w)| sum + w * math::cos(beta));
        let average: Vec<f64> = latitudes
            .iter()
            .map(|&(beta, w)| w * math::cos(beta) / total)
            .collect();
        let (lo, hi) = (math::ln(params.range.0), math::ln(params.range.1));
        let step = (hi - lo) / index(params.radii - 1);
        let mut points = Vec::with_capacity(params.radii * params.angles);
        let mut masses = Vec::with_capacity(points.capacity());
        let mut monopole = Vec::with_capacity(params.radii);
        for i in 0..params.radii {
            let r = math::exp(lo + step * index(i));
            let mut mean = 0.0;
            for (&(beta, _), &a) in latitudes.iter().zip(&average) {
                let (sin, cos) = math::sin_cos(beta);
                let (r_cyl, z) = (r * cos, r * sin);
                let rho = density(c, r_cyl, z);
                mean += a * rho;
                // The point's mass, ρ r³ d(ln r) × its latitude's weight, before normalising.
                masses.push(rho * r * r * r * a);
                points.push(Point {
                    r2: r_cyl * r_cyl,
                    z2: z * z,
                    scale: rho,
                    root_mass: 0.0,
                });
            }
            if r <= params.monopole_reach {
                monopole.push(params.monopole_weight / mean);
            }
        }
        let mass = masses.iter().fold(0.0, |sum, &m| sum + m);
        for (point, &m) in points.iter_mut().zip(&masses) {
            point.root_mass = (m / mass).sqrt();
            point.scale = point.root_mass / point.scale;
        }
        let forces = FORCE_RADII
            .iter()
            .map(|&r| (r, params.force_weight / multipole_force(c, r)))
            .collect();
        Self {
            points,
            average,
            monopole,
            monopole_weight: params.monopole_weight,
            forces,
            force_weight: params.force_weight,
            centre_weight: params.centre_weight,
            mass_weight: params.mass_weight,
            mass: total_mass(c),
        }
    }

    /// The number of residuals: the density's, the monopole's, the forces', the central density's
    /// and the total mass's.
    #[must_use]
    fn len(&self) -> usize {
        self.points.len() + self.monopole.len() + self.forces.len() + 2
    }

    /// The residuals of the expansion whose density at each point is `fitted`, into `out`.
    fn residuals(&self, fitted: &[f64], out: &mut [f64]) {
        let n = self.points.len();
        let (density, monopole) = out.split_at_mut(n);
        for ((o, point), &f) in density.iter_mut().zip(&self.points).zip(fitted) {
            *o = point.scale * f - point.root_mass;
        }
        for ((o, &g), shell) in monopole
            .iter_mut()
            .zip(&self.monopole)
            .zip(fitted.chunks_exact(self.average.len()))
        {
            let mean = self
                .average
                .iter()
                .zip(shell)
                .fold(0.0, |sum, (&a, &f)| sum + a * f);
            *o = g * mean - self.monopole_weight;
        }
    }

    /// The residuals of `terms` into `out` and, when `jacobian` is given, their derivatives by
    /// each term's weight, then each one's ln width, then each one's ln q, column by column, for
    /// any number of terms. Every entry of the Jacobian is written, those that are zero included.
    fn evaluate(&self, terms: &[Term], out: &mut [f64], mut jacobian: Option<&mut [f64]>) {
        let (rows, points, count) = (self.len(), self.points.len(), terms.len());
        if let Some(j) = jacobian.as_deref_mut() {
            j.fill(0.0);
        }
        let mut fitted = vec![0.0; points];
        for (k, term) in terms.iter().enumerate() {
            let inverse = 0.5 / (term.width * term.width);
            let inv_q2 = 1.0 / (term.q * term.q);
            for (i, point) in self.points.iter().enumerate() {
                let vertical = point.z2 * inv_q2 * inverse;
                let exponent = point.r2 * inverse + vertical;
                let e = math::exp(-exponent);
                fitted[i] += term.weight * e;
                if let Some(j) = jacobian.as_deref_mut() {
                    j[k * rows + i] = e;
                    j[(count + k) * rows + i] = term.weight * e * 2.0 * exponent;
                    j[(2 * count + k) * rows + i] = term.weight * e * 2.0 * vertical;
                }
            }
        }
        self.residuals(&fitted, out);
        // The force rows, `force_weight (F̂ ÷ F − 1)`: each term's pull is linear in its weight,
        // and its derivatives in ln width and ln q are forward differences.
        let first = points + self.monopole.len();
        let mut pulls = vec![0.0; self.forces.len()];
        let step = math::exp(FORCE_STEP);
        for (k, term) in terms.iter().enumerate() {
            let unit = unit_pulls(term.width, term.q, &self.forces);
            for (sum, &f) in pulls.iter_mut().zip(&unit) {
                *sum += term.weight * f;
            }
            if let Some(j) = jacobian.as_deref_mut() {
                let wider = unit_pulls(term.width * step, term.q, &self.forces);
                let taller = unit_pulls(term.width, term.q * step, &self.forces);
                for (i, &(_, scale)) in self.forces.iter().enumerate() {
                    j[k * rows + first + i] = scale * unit[i];
                    j[(count + k) * rows + first + i] =
                        scale * term.weight * (wider[i] - unit[i]) / FORCE_STEP;
                    j[(2 * count + k) * rows + first + i] =
                        scale * term.weight * (taller[i] - unit[i]) / FORCE_STEP;
                }
            }
        }
        for ((o, &(_, scale)), &pull) in out[first..].iter_mut().zip(&self.forces).zip(&pulls) {
            *o = scale * pull - self.force_weight;
        }
        // The central density `Σ w` and the total mass `Σ w (2π)^(3÷2) σ³ q`, against the body's
        // 1 and `M`: they keep a term from hiding a spike inside the grid's first radius or a
        // vast, faint halo outside its last.
        let centre = first + self.forces.len();
        let (mut central, mut mass) = (0.0, 0.0);
        for (k, term) in terms.iter().enumerate() {
            let unit = unit_mass(term.width, term.q);
            central += term.weight;
            mass += term.weight * unit;
            if let Some(j) = jacobian.as_deref_mut() {
                let scale = self.mass_weight / self.mass;
                j[k * rows + centre] = self.centre_weight;
                j[k * rows + centre + 1] = scale * unit;
                j[(count + k) * rows + centre + 1] = scale * term.weight * 3.0 * unit;
                j[(2 * count + k) * rows + centre + 1] = scale * term.weight * unit;
            }
        }
        out[centre] = self.centre_weight * (central - 1.0);
        out[centre + 1] = self.mass_weight * (mass / self.mass - 1.0);
        if let Some(j) = jacobian {
            // Each column holds ∂ρ̂ at the points; its monopole rows are their latitude averages,
            // and its density rows are scaled last.
            for column in j.chunks_exact_mut(rows) {
                let (density, monopole) = column.split_at_mut(points);
                for ((o, &g), shell) in monopole
                    .iter_mut()
                    .zip(&self.monopole)
                    .zip(density.chunks_exact(self.average.len()))
                {
                    *o = g * self
                        .average
                        .iter()
                        .zip(shell)
                        .fold(0.0, |sum, (&a, &d)| sum + a * d);
                }
                for (d, point) in density.iter_mut().zip(&self.points) {
                    *d *= point.scale;
                }
            }
        }
    }
}

/// The terms that the parameters `p` (weights, ln widths, ln axis ratios) stand for.
#[must_use]
fn terms_of(p: &[f64]) -> [Term; TERMS] {
    core::array::from_fn(|k| Term {
        weight: p[k],
        width: math::exp(p[TERMS + k]),
        q: math::exp(p[2 * TERMS + k]),
    })
}

/// The weights of `terms` solved by [`nnls`] on `grid`'s residuals, whose design is the
/// Jacobian's weight columns and whose right-hand side is the residuals at zero weight.
#[must_use]
fn solve_weights(grid: &Grid, terms: &[Term]) -> Vec<f64> {
    let m = grid.len();
    let n = terms.len();
    let mut out = vec![0.0; m];
    let mut jacobian = vec![0.0; m * 3 * n];
    grid.evaluate(terms, &mut out, Some(&mut jacobian));
    let zero: Vec<Term> = terms.iter().map(|t| Term { weight: 0.0, ..*t }).collect();
    grid.evaluate(&zero, &mut out, None);
    let rows: Vec<Vec<f64>> = (0..m)
        .map(|i| (0..n).map(|k| jacobian[k * m + i]).collect())
        .collect();
    let rhs: Vec<f64> = out.iter().map(|&r| -r).collect();
    nnls(&rows, &rhs)
}

/// The seed from the one-dimensional `exp` (module documentation): its 14 terms at `q = 1` with
/// their weights solved on `grid`, then the two carrying the most mass each split into two of
/// half the weight at `q` = `split` and 1 ÷ `split`, the first among equals first.
#[must_use]
fn seed(grid: &Grid, exp: &[(f64, f64); WIDTHS], split: f64) -> [Term; TERMS] {
    let spherical: Vec<Term> = exp
        .iter()
        .map(|&(w, s)| Term {
            weight: w,
            width: s,
            q: 1.0,
        })
        .collect();
    let weights = solve_weights(grid, &spherical);
    let solved: Vec<Term> = spherical
        .iter()
        .zip(weights)
        .map(|(t, weight)| Term { weight, ..*t })
        .collect();
    let mut heaviest: Vec<usize> = (0..WIDTHS).collect();
    heaviest.sort_by(|&i, &j| {
        solved[j]
            .mass()
            .total_cmp(&solved[i].mass())
            .then(i.cmp(&j))
    });
    let split_at = [heaviest[0], heaviest[1]];
    let mut terms = Vec::with_capacity(TERMS);
    for (i, term) in solved.iter().enumerate() {
        if split_at.contains(&i) {
            for q in [split, 1.0 / split] {
                terms.push(Term {
                    weight: 0.5 * term.weight,
                    q,
                    ..*term
                });
            }
        } else {
            terms.push(*term);
        }
    }
    core::array::from_fn(|k| terms[k])
}

/// One fitted row of `MGE_BOXY`: its boxiness, its terms, widths ascending, and the
/// Levenberg–Marquardt steps it took. Only [`fit_row`] makes one.
#[derive(Debug, Clone, PartialEq)]
pub struct BoxyRow {
    c: f64,
    terms: [Term; TERMS],
    steps: u32,
}

impl BoxyRow {
    /// The boxiness.
    #[must_use]
    pub fn c(&self) -> f64 {
        self.c
    }

    /// The terms, widths strictly ascending in a fitted row.
    #[must_use]
    pub fn terms(&self) -> &[Term; TERMS] {
        &self.terms
    }

    /// The Levenberg–Marquardt steps taken.
    #[must_use]
    pub fn steps(&self) -> u32 {
        self.steps
    }
}

/// Fits the body of boxiness `c` from the one-dimensional `exp` (module documentation).
#[must_use]
pub fn fit_row(c: f64, exp: &[(f64, f64); WIDTHS], params: &BoxyParams) -> BoxyRow {
    let grid = Grid::new(c, params);
    let start = seed(&grid, exp, params.split);
    let p: Vec<f64> = start
        .iter()
        .map(|t| t.weight)
        .chain(start.iter().map(|t| math::ln(t.width)))
        .chain(start.iter().map(|t| math::ln(t.q)))
        .collect();
    // The weights from 0 up; the widths and axis ratios within the manifest's ranges, so that a
    // term whose weight has gone to 0 cannot wander off to a width or q no table should hold.
    let bounds = |k: usize| match k / TERMS {
        0 => (0.0, f64::INFINITY),
        1 => (math::ln(params.widths.0), math::ln(params.widths.1)),
        _ => (
            math::ln(params.axis_ratios.0),
            math::ln(params.axis_ratios.1),
        ),
    };
    let lower: Vec<f64> = (0..3 * TERMS).map(|k| bounds(k).0).collect();
    let upper: Vec<f64> = (0..3 * TERMS).map(|k| bounds(k).1).collect();
    let fit = levenberg_marquardt(
        |p, r, jacobian| grid.evaluate(&terms_of(p), r, jacobian),
        &p,
        &lower,
        &upper,
        grid.len(),
        params.iterations,
    );
    let mut terms = terms_of(&fit.params);
    terms.sort_by(|a, b| a.width.total_cmp(&b.width));
    BoxyRow {
        c,
        terms,
        steps: fit.steps,
    }
}

/// Fits every row of `MGE_BOXY` from the one-dimensional `exp`.
#[must_use]
pub fn fit(exp: &[(f64, f64); WIDTHS], params: &BoxyParams) -> [BoxyRow; ROWS] {
    params.boxiness.map(|c| fit_row(c, exp, params))
}

/// How closely one row follows its body (P15.T3.b's acceptance).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoxyFigures {
    /// The boxiness.
    pub c: f64,
    /// The mass-weighted rms of `ρ̂ ÷ ρ − 1` over the whole body.
    pub rms_density: f64,
    /// `M̂(< r) ÷ M(< r) − 1` at each of [`MASS_RADII`], `M` by [`enclosed_mass`].
    pub enclosed: [f64; 4],
    /// `F̂(R) ÷ F(R) − 1` of the inward pull in the plane at each of [`FORCE_RADII`], `F` by
    /// [`multipole_force`], which the tests hold to [`midplane_force`]'s brute force.
    pub force: [f64; 6],
    /// `M̂ ÷ M − 1` of the whole mass, `M` by [`total_mass`]: what the potential's normalisation
    /// to the bulge's mass reads (no criterion of plan 15's).
    pub total: f64,
}

impl BoxyFigures {
    /// The largest `|M̂ ÷ M − 1|`, a NaN counting as larger than any number.
    #[must_use]
    pub fn worst_enclosed(&self) -> f64 {
        largest(&self.enclosed.map(f64::abs))
    }

    /// The largest `|F̂ ÷ F − 1|`, a NaN counting as larger than any number.
    #[must_use]
    pub fn worst_force(&self) -> f64 {
        largest(&self.force.map(f64::abs))
    }

    /// Whether every figure meets P15.T3.b's criterion: the density within 3%, the masses within
    /// 1% and the forces within 2%.
    #[must_use]
    pub fn passes(&self) -> bool {
        self.rms_density < 0.03 && self.worst_enclosed() < 0.01 && self.worst_force() < 0.02
    }
}

/// The sim's Gaussians for `terms`, each of mass `w (2π)^(3÷2) σ³ q` in units of the body's
/// central density times a cubic scale length, lengths in scale lengths taken as light-years.
///
/// # Panics
///
/// If a term's weight is negative or its width or q not positive, or its mass not finite, which
/// a fitted row's bounds rule out.
#[must_use]
fn gaussians(terms: &[Term]) -> Vec<Gaussian> {
    terms
        .iter()
        .map(|t| {
            Gaussian::new(SolarMasses::new(t.mass()), LightYears::new(t.width), t.q).expect(
                "a fitted term has a non-negative weight, a width and q within the manifest's \
                 checked bounds and so a finite mass",
            )
        })
        .collect()
}

/// The radial panels for the density figure, in scale lengths: narrow at the centre, where the
/// body has its cusp, and out to where `e^(−0.8 r) r²` is below 10⁻¹².
const RADIAL_EDGES: [f64; 13] = [
    0.0, 0.05, 0.2, 0.5, 1.0, 2.0, 4.0, 7.0, 11.0, 16.0, 24.0, 36.0, 50.0,
];

/// The figures of `row` (module documentation).
#[must_use]
pub fn figures(row: &BoxyRow) -> BoxyFigures {
    let c = row.c;
    // Mass-weighted rms over the body: ∫ ρ ε² dV ÷ ∫ ρ dV with dV ∝ r² cos β dr dβ.
    let (mut weighted, mut mass) = (0.0, 0.0);
    for radial in RADIAL_EDGES.windows(2) {
        for latitude in LATITUDE_EDGES.windows(2) {
            let cell = gl16(
                |r| {
                    gl16(
                        |beta| {
                            let (sin, cos) = math::sin_cos(beta);
                            let (r_cyl, z) = (r * cos, r * sin);
                            let rho = density(c, r_cyl, z);
                            let fitted = row.terms.iter().fold(0.0, |sum, t| {
                                let q = t.q;
                                sum + t.weight
                                    * math::exp(
                                        -(r_cyl * r_cyl + z * z / (q * q))
                                            / (2.0 * t.width * t.width),
                                    )
                            });
                            let error = fitted / rho - 1.0;
                            r * r * cos * rho * error * error
                        },
                        latitude[0],
                        latitude[1],
                    )
                },
                radial[0],
                radial[1],
            );
            weighted += cell;
        }
        mass += gl16(
            |r| r * r * enclosed_density_shell(c, r),
            radial[0],
            radial[1],
        );
    }
    let gaussians = gaussians(&row.terms);
    let enclosed = MASS_RADII.map(|r| {
        let fitted = gaussians.iter().fold(0.0, |sum, g| {
            sum + g.enclosed_mass(LightYears::new(r)).value()
        });
        fitted / enclosed_mass(c, r) - 1.0
    });
    let force = FORCE_RADII.map(|r| {
        let fitted = gaussians
            .iter()
            .fold(0.0, |sum, g| sum + g.v_circ_sq(LightYears::new(r)))
            / (G * r);
        fitted / multipole_force(c, r) - 1.0
    });
    let whole = row.terms.iter().fold(0.0, |sum, t| sum + t.mass());
    BoxyFigures {
        c,
        rms_density: (weighted / mass).sqrt(),
        enclosed,
        force,
        total: whole / total_mass(c) - 1.0,
    }
}

/// `∫₀^(π÷2) ρ(r, β) cos β dβ`, the latitude integral of the body at radius `r`.
#[must_use]
fn enclosed_density_shell(c: f64, r: f64) -> f64 {
    LATITUDE_EDGES.windows(2).fold(0.0, |sum, panel| {
        sum + gl16(
            |beta| {
                let (sin, cos) = math::sin_cos(beta);
                cos * density(c, r * cos, r * sin)
            },
            panel[0],
            panel[1],
        )
    })
}

/// `values` as a list, `a, b and c`.
#[must_use]
fn listed(values: impl Iterator<Item = String>) -> String {
    let all: Vec<String> = values.collect();
    match all.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
        Some((last, _)) => last.clone(),
        None => String::new(),
    }
}

/// The figures as the header's acceptance line states them.
#[must_use]
pub(crate) fn acceptance(figures: &[BoxyFigures; ROWS]) -> String {
    let each = |f: fn(&BoxyFigures) -> f64, digits: usize| {
        listed(
            figures
                .iter()
                .map(|g| format!("{:.digits$}%", 100.0 * f(g))),
        )
    };
    let span = |radii: &[f64]| {
        format!(
            "{}–{}",
            radii.first().copied().unwrap_or_default(),
            radii.last().copied().unwrap_or_default()
        )
    };
    format!(
        "`MGE_BOXY` at c = {}: mass-weighted rms relative density error {} (plan 15's 3% is out \
         of reach of non-negative, axis-aligned centred Gaussians, which are log-convex in (R², \
         z²): a structural miss, ruled 2026-10-23), mass inside spheres of {} scale lengths \
         within {}, whole mass within {} and midplane force at {} scale lengths within {}",
        listed(figures.iter().map(|g| format!("{}", g.c))),
        each(|g| g.rms_density, 1),
        span(&MASS_RADII),
        each(BoxyFigures::worst_enclosed, 3),
        each(|g| g.total.abs(), 3),
        span(&FORCE_RADII),
        each(BoxyFigures::worst_force, 2),
    )
}

/// The notes of `MGE_BOXY` in the refit's table.
#[must_use]
pub(crate) fn notes(params: &BoxyParams) -> String {
    format!(
        "\
- [`MGE_BOXY`]: the boxy bulge's unit body `exp(−(R^c + |z|^c)^(1 ÷ c))`, `R` and `z` in its
  scale lengths, for c = {boxiness}: {TERMS} terms each, every term with its own weight, width
  and axis ratio q. On a polar grid of {radii} log-spaced radii from {} to {} by {angles}
  Gauss–Legendre latitudes, in the density's relative error weighted by each point's share of
  the mass, with a row per radius to {reach} for the latitude average weighted {monopole}, one
  per radius {force_radii} for the midplane force weighted {force}, and one each for the central
  density and the whole mass weighted {centre} and {mass}. Seeded from [`MGE_EXP`] at q = 1, its
  weights solved by non-negative least squares and its two heaviest terms then split into q =
  {split} and 1 ÷ {split}; then Levenberg–Marquardt on the weights, ln widths and ln q together,
  at most {iterations} iterations, the weights held non-negative, the widths within {} to {} and
  q within {} to {}. Such a sum is log-convex in (R², z²) and cannot follow the body's boxiness,
  so the fit is weighted towards the mass and the force, which the potential reads.",
        params.range.0,
        params.range.1,
        params.widths.0,
        params.widths.1,
        params.axis_ratios.0,
        params.axis_ratios.1,
        boxiness = listed(params.boxiness.iter().map(|c| format!("{c}"))),
        force_radii = listed(FORCE_RADII.iter().map(|r| format!("{r}"))),
        radii = params.radii,
        angles = params.angles,
        reach = params.monopole_reach,
        monopole = literal(params.monopole_weight),
        force = literal(params.force_weight),
        centre = literal(params.centre_weight),
        mass = literal(params.mass_weight),
        split = literal(params.split),
        iterations = params.iterations,
    )
}

/// The `use` line of the refit's table: `MGE_BOXY`'s row type is its consumer's (P15.T3.c).
pub(crate) const USE_LINE: &str = "use crate::galaxy::potential::mge::BoxyTerm;\n";

/// `MGE_BOXY` as Rust source, ending with a newline.
#[must_use]
pub(crate) fn item(rows: &[BoxyRow; ROWS]) -> String {
    let mut out = String::from(
        "/// `Σ weight × exp(−(R² + z² ÷ q²) ÷ 2 width²) ≈ exp(−(R^c + |z|^c)^(1 ÷ c))`, the boxy\n\
         /// bulge's unit body, `R` and `z` in its scale lengths (`s` in the plane and `h` along z):\n\
         /// rows `(c, terms)`, `c` ascending, each row's widths ascending and weights non-negative.\n\
         #[rustfmt::skip]\n",
    );
    writeln!(
        out,
        "pub const MGE_BOXY: [(f64, [BoxyTerm; {TERMS}]); {ROWS}] = ["
    )
    .expect("writing to a String");
    for row in rows {
        writeln!(out, "    ({}, [", literal(row.c)).expect("writing to a String");
        for t in &row.terms {
            writeln!(
                out,
                "        BoxyTerm {{ weight: {}, width: {}, q: {} }},",
                literal(t.weight),
                literal(t.width),
                literal(t.q)
            )
            .expect("writing to a String");
        }
        out.push_str("    ]),\n");
    }
    out.push_str("];\n");
    out
}
