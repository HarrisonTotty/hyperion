//! The test planet's level bound `ε_n`: a hard bound on the distance between level n's mesh and the
//! finest level's (plan R05, Design note 15).
//!
//! # Derivation
//!
//! Let `F_n` be the height function at level n (its octaves, [`TestPlanet::octaves_at`]) and `S_n`
//! the mesh that interpolates it linearly on level n's triangles in the face's cell coordinates
//! (s, t), as the collision interpolant does. With f the finest level,
//! `|S_n − S_f| ≤ |S_n − F_n| + |F_n − F_f| + |F_f − S_f|`, where:
//!
//! - `|F_n − F_f|` is at most the omitted octaves' certified maxima, the sum over the octaves of f
//!   not in n of `σ_k · B ÷ σ_noise` (a ridged octave's own maximum where it is ridged);
//! - `|S_L − F_L|` is the linear-interpolation error on a triangle whose legs are one lattice step
//!   Δ = 2^−(L + 6) in (s, t). For a function g of (s, t) whose second derivative along any unit
//!   direction of the parameter plane is at most M, it is at most `½ M r²`, r the radius of the
//!   smallest disc holding the triangle (Waldron 1998, "The error in linear interpolation at the
//!   vertices of a simplex", SIAM Journal on Numerical Analysis 35, 1191), here `Δ ÷ √2`, so
//!   `M Δ² ÷ 4`. The height is F(P(s, t)) with P the spheroid point, so along a unit direction u
//!   of the plane `g'' = (J u)ᵀ H (J u) + ∇F · P_uu`, with `|J u| ≤ √2 ρ_max a` (`ρ_max` the largest
//!   arc rate per unit s, [`crate::geometry`]; a the equatorial radius, the spheroid's largest
//!   stretch) and `|P_uu| ≤ κ a` ([`PARAMETRIC_CURVATURE`]). So
//!   `|S_L − F_L| ≤ ½ ‖H‖ h_L² + ¼ |∇F| κ a Δ²`, with `h_L = ρ_max a Δ` the level's largest vertex
//!   spacing. Design note 15 wrote the first term as `h² ÷ 8` times the curvature, the
//!   one-dimensional figure; on the mesh's triangles it is `½ ‖H‖ h²`, and the second term, the
//!   warp's, is added (T6 as built).
//! - `‖H‖` and `|∇F|` are bounded octave by octave: an octave of RMS `σ_k` and lattice spacing `λ_k`
//!   adds `σ_k ÷ σ_noise × C₂ ÷ λ_k²` and `σ_k ÷ σ_noise × C₁ ÷ λ_k`, with C₁ and C₂ the noise
//!   basis's certified gradient and curvature maxima ([`NOISE_GRADIENT_BOUND`],
//!   [`NOISE_CURVATURE_BOUND`]) in lattice units. A ridged octave's term follows from the chain
//!   rule on `w · r` (see `ridged_derivative_bounds`).
//!
//! # Each octave takes the least of three bounds (2026-10-03)
//!
//! Linear interpolation is linear, so with `F_L = Σ_k g_k` over the level's octaves
//! `|S_L − F_L| = |Σ_k (I g_k − g_k)| ≤ Σ_k |I g_k − g_k|`, and each octave's error may take the
//! least of any true bounds on it (decision-r05-patch-demand.md, section 4c). For one octave g on
//! one of the mesh's triangles, with `I g(p) = Σ_i λ_i g(v_i)`, `λ_i ≥ 0`, `Σ_i λ_i = 1`:
//!
//! - **curvature**, Waldron's `½ M r²` as above: `½ ‖H_k‖ h_L² + ¼ |∇g_k| κ a Δ²`;
//! - **slope**: g as a function of (s, t) has, along any unit direction u of the parameter plane,
//!   `|∂_u g| = |∇F_k · J u| ≤ G_k √2 ρ_max a`, `G_k` the octave's gradient bound, so along the
//!   straight parameter segment from p to a vertex `|g(p) − g(v_i)| ≤ G_k √2 ρ_max a |p − v_i|`.
//!   Then `|g(p) − I g(p)| = |Σ_i λ_i (g(p) − g(v_i))| ≤ G_k √2 ρ_max a Σ_i λ_i |p − v_i|`, and for
//!   p in the triangle `Σ_i λ_i |p − v_i| ≤ (Σ_i λ_i |p − v_i|²)^½ = (R² − |p − c|²)^½ ≤ R`
//!   (Jensen's inequality, then the identity `Σ_i λ_i |p − v_i|² = R² − |p − c|²` for the triangle's
//!   circumcentre c and circumradius R, since p = `Σ_i` `λ_i` `v_i`), with `R = Δ ÷ √2` for the mesh's
//!   right isosceles triangles. So the error is at most `G_k ρ_max a Δ = G_k h_L`;
//! - **range**: `I g(p)` is a convex combination of values of g, so both it and `g(p)` lie in the
//!   interval of the octave's certified values, and the error is at most its width `W_k`:
//!   `2 B_k` for a plain octave (`B_k = σ_k B ÷ σ_noise`); for a ridged one, `w (r − r̄) ÷ r_rms`
//!   with `w ∈ [0, 1]` and `r ∈ [1 − √(B² + ε²), 1 − ε]`, whose interval
//!   `[1 − √(B² + ε²) − r̄, 1 − ε − r̄]` contains 0, so `W_k = σ_k (√(B² + ε²) − ε) ÷ r_rms`.
//!
//! The ridged octaves gain most: the curvature of a crest grows as 1 ÷ ε, while its slope and range
//! stay those of the noise.
//!
//! At the finest level ε is 0; above it, at levels selection never reaches, it is the two
//! interpolation terms alone.
//!
//! # Measured (T6, 2026-10-03; `level_bound_holds`, under `just test-slow`)
//!
//! No sample exceeded the bound at any level, ridges off or on. Per level the bound is checked at
//! 10⁴ points in 16 random patches; `σ_n` is the omitted octaves' RMS at full weight (the morph
//! is the fade); the per-patch maximum is over all 65 × 65 vertices of 32 further patches, where
//! the distance is `|F_n − F_f|`; `k_n = ε_n ÷ (S_n τ θ_px)` is the implied patch-to-distance
//! ratio at τ = 1 px, 1080p and 60° (for T13.a); `σ_n` is [`TestPlanet::omitted_sigma_m`], in
//! which a ridged octave counts at its mask's RMS. Ridges off, every patch's maximum was within
//! 1.25 × `4σ_n` at every level (R10.T4's criterion (ii), which 32 patches cannot resolve to 99%).
//!
//! Ridges Off:
//!
//! | n  | `ε_n` (m) | p99.9 (m) | p99.9 ÷ ε | `σ_n` (m) | p99.9 ÷ 4σ | patch max ÷ 4σ | `k_n` |
//! | -- | ------- | --------- | --------- | ------- | ---------- | -------------- | ----- |
//! | 0  | 7,114   | 2,081     | 0.292     | 645     | 0.806      | 0.943          | 1.27  |
//! | 1  | 4,962   | 1,392     | 0.281     | 455     | 0.765      | 1.005          | 1.77  |
//! | 2  | 3,444   | 1,014     | 0.294     | 320     | 0.791      | 1.018          | 2.46  |
//! | 3  | 2,372   | 719       | 0.303     | 225     | 0.801      | 1.027          | 3.39  |
//! | 4  | 1,615   | 470       | 0.291     | 156     | 0.754      | 1.035          | 4.62  |
//! | 5  | 1,081   | 330       | 0.305     | 106     | 0.779      | 0.995          | 6.18  |
//! | 6  | 715     | 225       | 0.314     | 73.3    | 0.767      | 0.988          | 8.18  |
//! | 7  | 460     | 151       | 0.328     | 49.4    | 0.763      | 0.940          | 10.5  |
//! | 8  | 281     | 97.5      | 0.347     | 31.3    | 0.780      | 0.986          | 12.8  |
//! | 9  | 154     | 48.4      | 0.314     | 15.6    | 0.773      | 0.973          | 14.1  |
//! | 10 | 80.4    | 23.3      | 0.290     | 7.82    | 0.745      | 0.948          | 14.7  |
//! | 11 | 41      | 12.5      | 0.305     | 3.91    | 0.797      | 0.951          | 15    |
//! | 12 | 20.6    | 6.05      | 0.293     | 1.96    | 0.773      | 0.953          | 15.1  |
//! | 13 | 10.3    | 2.95      | 0.287     | 0.977   | 0.755      | 1.066          | 15    |
//! | 14 | 5.06    | 1.41      | 0.278     | 0.488   | 0.720      | 0.968          | 14.8  |
//! | 15 | 2.44    | 0.731     | 0.299     | 0.243   | 0.753      | 0.992          | 14.3  |
//! | 16 | 1.13    | 0.354     | 0.312     | 0.118   | 0.747      | 0.929          | 13.3  |
//! | 17 | 0.476   | 0.151     | 0.316     | 0.0529  | 0.712      | 0.885          | 11.2  |
//! | 18 | 0.148   | 0.00493   | 0.033     | 0       | —          | —              | 6.95  |
//!
//! Ridges off, the curvature bound is the least for every octave, so these figures are those of the
//! curvature bound alone. The hard bound is about 3–3.5 times the 99.9th percentile, so not under a quarter (a finding)
//! except at level 18, which omits no octave and differs from the finest only by interpolation,
//! which the curvature bound overstates.
//!
//! Ridges On:
//!
//! | n  | `ε_n` (m) | p99.9 (m) | p99.9 ÷ ε | `σ_n` (m) | p99.9 ÷ 4σ | patch max ÷ 4σ | `k_n` |
//! | -- | ------- | --------- | --------- | ------- | ---------- | -------------- | ----- |
//! | 0  | 7,636   | 2,034     | 0.266     | 634     | 0.802      | 0.934          | 1.36  |
//! | 1  | 5,484   | 1,375     | 0.251     | 439     | 0.784      | 0.964          | 1.96  |
//! | 2  | 3,965   | 933       | 0.235     | 297     | 0.786      | 1.028          | 2.83  |
//! | 3  | 2,894   | 627       | 0.217     | 189     | 0.828      | 1.242          | 4.14  |
//! | 4  | 2,137   | 400       | 0.187     | 98.6    | 1.015      | 1.803          | 6.11  |
//! | 5  | 2,019   | 332       | 0.164     | 67.7    | 1.226      | 1.744          | 11.5  |
//! | 6  | 1,648   | 219       | 0.133     | 47.6    | 1.150      | 1.867          | 18.8  |
//! | 7  | 1,240   | 149       | 0.120     | 33.3    | 1.117      | 1.616          | 28.4  |
//! | 8  | 877     | 93.6      | 0.107     | 23.1    | 1.012      | 1.507          | 40.1  |
//! | 9  | 561     | 47        | 0.084     | 15.6    | 0.751      | 0.973          | 51.4  |
//! | 10 | 277     | 23.4      | 0.085     | 7.82    | 0.748      | 0.948          | 50.6  |
//! | 11 | 127     | 12.5      | 0.098     | 3.91    | 0.797      | 0.951          | 46.6  |
//! | 12 | 53.8    | 6.04      | 0.112     | 1.96    | 0.772      | 0.953          | 39.4  |
//! | 13 | 19.5    | 2.96      | 0.152     | 0.977   | 0.756      | 1.066          | 28.5  |
//! | 14 | 7.37    | 1.4       | 0.191     | 0.488   | 0.720      | 0.968          | 21.6  |
//! | 15 | 3.02    | 0.731     | 0.242     | 0.243   | 0.753      | 0.992          | 17.7  |
//! | 16 | 1.28    | 0.354     | 0.277     | 0.118   | 0.747      | 0.929          | 15    |
//! | 17 | 0.515   | 0.151     | 0.293     | 0.0529  | 0.712      | 0.885          | 12.1  |
//! | 18 | 0.16    | 0.00493   | 0.031     | 0       | —          | —              | 7.48  |
//!
//! With ridges on, each octave's interpolation error takes the least of its curvature, slope and
//! range bounds (above): the crests' curvature grows as 1 ÷ ε, so from level 5 to 12 the slope
//! bound applies to the ridged octaves, and `k_n` peaks at 51 at level 9 (230 with the curvature
//! bound alone). The 99.9th percentile is still under a quarter of the bound from level 2 to 15,
//! 8–17% at levels 5–12 (findings): selection by the hard bound over-refines the ridged planet
//! there. Decisions-r05.md item 6 keeps the hard bound in R05 and has T13.a record the demand
//! under min(hard, `4σ_n`) for the ridged planet too.
//!
//! **A finding for that count and for R10.T4.** With ridges on, the ridged term is far from
//! Gaussian: at levels 4 to 8, which omit a ridged octave, the 99.9th percentile is 1.0–1.23 ×
//! `4σ_n`, patch maxima reach 1.87 × `4σ_n`, and only 59–88% of patches lie within 1.25 ×
//! `4σ_n`. So `min(ε_n, 4σ_n)` is not a safe selection bound on the ridged planet: R10.T4's
//! criteria (i) and (ii) both fail there, and its rule would raise k.

use super::{RIDGE_EPSILON, RIDGE_MASK_RMS, RIDGE_RMS, RIDGED, Ridges, TestPlanet, octaves};
use crate::geometry::{finest_level, lattice_step, vertex_spacing};
use crate::noise::NOISE_RMS;
use crate::num;

/// C₁, the certified maximum of |∇noise| in lattice units, over all points and gradient choices.
///
/// Certified like [`NOISE_BOUND`](crate::noise::NOISE_BOUND): per axis, the largest |∂_i noise| any choice of the eight
/// corners' gradients gives at x is `U_i(x) = Σ_c max_g |∂_i (w_c (g · (x − c)))|`, so
/// `|∇noise| ≤ |U(x)|`, which is searched on a grid of step 1/256 over the 1/48 of the cell its
/// symmetry leaves and raised by a Lipschitz margin (`noise_derivative_bounds_certified`).
pub const NOISE_GRADIENT_BOUND: f64 = 6.70;

/// C₂, the certified maximum of the spectral norm of the noise's Hessian in lattice units, over
/// all points and gradient choices, bounded by its Frobenius norm and certified as
/// [`NOISE_GRADIENT_BOUND`] is.
pub const NOISE_CURVATURE_BOUND: f64 = 32.82;

/// κ, a bound on `|∂²P ÷ ∂u²|` for the unit-sphere point P(s, t) of a face along a unit direction
/// u of the (s, t) plane, over the face; found on a grid and raised by a tenth.
pub const PARAMETRIC_CURVATURE: f64 = 3.67;

/// The bounds on the second derivative's norm and the gradient's norm of one octave's
/// contribution, per metre² and per metre.
#[must_use]
fn octave_derivative_bounds(planet: &TestPlanet, k: u8) -> (f64, f64) {
    let lambda = octaves::spacing_m(k);
    let amp = planet.sigma_m(k) / NOISE_RMS;
    if planet.ridges() == Ridges::On && RIDGED.contains(&k) {
        ridged_derivative_bounds(planet, k)
    } else {
        (
            amp * NOISE_CURVATURE_BOUND / (lambda * lambda),
            amp * NOISE_GRADIENT_BOUND / lambda,
        )
    }
}

/// The bounds of a ridged octave's contribution `σ_k w r ÷ RIDGE_RMS`, with r the centred ridge
/// `1 − √(n² + ε²) − RIDGE_MEAN` and w the mask from octaves 2 and 3, by the product rule:
/// `‖H(w r)‖ ≤ ‖H_w‖ |r| + 2 |∇w| |∇r| + |w| ‖H_r‖` and `|∇(w r)| ≤ |∇w| |r| + |w| |∇r|`, with
/// `|∇r| ≤ |∇n|`, `‖H_r‖ ≤ |∇n|² ÷ ε + ‖H_n‖`, `|w| ≤ 1`, and w's derivatives those of a smoothstep
/// (slope at most 3/2, curvature at most 6) of `(n₂ + n₃) ÷ (4 σ_noise)`.
#[must_use]
fn ridged_derivative_bounds(planet: &TestPlanet, k: u8) -> (f64, f64) {
    let (c1, c2) = (NOISE_GRADIENT_BOUND, NOISE_CURVATURE_BOUND);
    let lambda = octaves::spacing_m(k);
    let (l2, l3) = (octaves::spacing_m(2), octaves::spacing_m(3));
    let s_grad = (c1 / l2 + c1 / l3) / (4.0 * NOISE_RMS);
    let s_hess = (c2 / (l2 * l2) + c2 / (l3 * l3)) / (4.0 * NOISE_RMS);
    let w_grad = 1.5 * s_grad;
    let w_hess = 1.5 * s_hess + 6.0 * s_grad * s_grad;
    let r_max = super::ridge_reach();
    let n_grad = c1 / lambda;
    let n_hess = c2 / (lambda * lambda);
    let r_grad = n_grad;
    let r_hess = n_grad * n_grad / RIDGE_EPSILON + n_hess;
    let scale = planet.sigma_m(k) / RIDGE_RMS;
    (
        scale * (w_hess * r_max + 2.0 * w_grad * r_grad + r_hess),
        scale * (w_grad * r_max + r_grad),
    )
}

impl TestPlanet {
    /// `ε_n`, the hard bound on the distance between level `level`'s mesh and the finest level's,
    /// metres (see the module documentation).
    ///
    /// # Panics
    ///
    /// If `level` is above [`crate::cube::MAX_LEVEL`].
    #[must_use]
    pub fn level_bound_m(&self, level: u8) -> f64 {
        let finest = finest_level(self.figure().equatorial_radius_m);
        if level == finest {
            return 0.0;
        }
        let own = self.octaves_at(level).count();
        let all = self.octaves_at(finest).count();
        let mut omitted = 0.0;
        for k in own..all {
            omitted += self.octave_bound_m(k);
        }
        omitted + self.interpolation_bound_m(level, own) + self.interpolation_bound_m(finest, all)
    }

    /// `σ_n`, the RMS height of the octaves level `level` omits and the finest level includes,
    /// metres: the spread of `|F_n − F_f|` that decisions-r05.md item 6 records beside `ε_n`, and
    /// that R10.T4's rule and T13.a's second count read as `min(ε_n, 4σ_n)`.
    ///
    /// The octaves are independent, so their variances add, in index order:
    /// `σ_n² = Σ_k (f_k σ_k)²` over the omitted octaves k, with `σ_k` the octave's RMS
    /// ([`TestPlanet::sigma_m`]) and `f_k` 1 for a plain octave and [`RIDGE_MASK_RMS`] for a
    /// ridged one, whose term `σ_k w (r − r̄) ÷ r_rms` has unit RMS in `(r − r̄) ÷ r_rms` times the
    /// mask's. It is taken at morph 0, the newest octave at full weight (the morph is the fade). It
    /// is 0 from the finest level on, which omits nothing; with ridges on it is smaller than with
    /// them off at the levels that omit a ridged octave (octaves 8 to 12), and the same elsewhere.
    /// A statistical figure, never a bound: selection and culling read `ε_n`.
    ///
    /// # Panics
    ///
    /// If `level` is above [`crate::cube::MAX_LEVEL`].
    #[must_use]
    pub fn omitted_sigma_m(&self, level: u8) -> f64 {
        let finest = finest_level(self.figure().equatorial_radius_m);
        let own = self.octaves_at(level).count();
        let all = self.octaves_at(finest).count();
        let mut variance = 0.0;
        for k in own..all {
            let factor = if self.ridges() == Ridges::On && RIDGED.contains(&k) {
                RIDGE_MASK_RMS
            } else {
                1.0
            };
            let sigma = factor * self.sigma_m(k);
            variance += sigma * sigma;
        }
        variance.sqrt()
    }

    /// The level table the client's selection reads at run time (plan R05, Provides): for each
    /// level n from 0 to [`crate::cube::MAX_LEVEL`], at index 4n, the level bound `ε_n`, the lowest
    /// and the highest height the level can reach, and its largest vertex spacing, metres.
    #[must_use]
    pub fn level_table(&self) -> Vec<f64> {
        let a = self.figure().equatorial_radius_m;
        let mut table = Vec::with_capacity(4 * (usize::from(crate::cube::MAX_LEVEL) + 1));
        for level in 0..=crate::cube::MAX_LEVEL {
            let (low, high) = self.height_range_m(level);
            table.extend([
                self.level_bound_m(level),
                low,
                high,
                vertex_spacing(a, level).max_m,
            ]);
        }
        table
    }

    /// The bound on the linear-interpolation error of level `level`'s mesh of octaves 0 to
    /// `count − 1`, metres.
    #[must_use]
    fn interpolation_bound_m(&self, level: u8, count: u8) -> f64 {
        let a = self.figure().equatorial_radius_m;
        let h = vertex_spacing(a, level).max_m;
        let step = lattice_step(level);
        let mut total = 0.0;
        for k in 0..count {
            let (hess, grad) = octave_derivative_bounds(self, k);
            let curvature =
                0.5 * hess * h * h + 0.25 * grad * PARAMETRIC_CURVATURE * a * step * step;
            let slope = h * grad;
            let range = self.octave_width_m(k);
            total += num::min(curvature, num::min(slope, range));
        }
        total
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cube::{Face, FaceUv, MAX_LEVEL, face_uv_to_xyz, st_to_uv, unit_dir};
    use crate::test_planet::TEST_PLANET;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    /// The twelve distinct gradients of the table.
    const EDGES: [[f64; 3]; 12] = [
        [1.0, 1.0, 0.0],
        [-1.0, 1.0, 0.0],
        [1.0, -1.0, 0.0],
        [-1.0, -1.0, 0.0],
        [1.0, 0.0, 1.0],
        [-1.0, 0.0, 1.0],
        [1.0, 0.0, -1.0],
        [-1.0, 0.0, -1.0],
        [0.0, 1.0, 1.0],
        [0.0, -1.0, 1.0],
        [0.0, 1.0, -1.0],
        [0.0, -1.0, -1.0],
    ];

    /// The fade and its first two derivatives.
    fn fade3(t: f64) -> [f64; 3] {
        [
            t * t * t * (t * (t * 6.0 - 15.0) + 10.0),
            30.0 * t * t * (t * (t - 2.0) + 1.0),
            60.0 * t * (t * (2.0 * t - 3.0) + 1.0),
        ]
    }

    /// The envelopes |U(x)| and the Frobenius norm of V(x) (see `NOISE_GRADIENT_BOUND`).
    #[expect(
        clippy::many_single_char_names,
        reason = "the derivation's names: the point, fades, weights, envelopes and gradients"
    )]
    fn envelopes(x: [f64; 3]) -> (f64, f64) {
        let f = x.map(fade3);
        let mut u = [0.0_f64; 3];
        let mut v = [[0.0_f64; 3]; 3];
        for corner in 0..8_u8 {
            let bit = [corner & 1, (corner >> 1) & 1, (corner >> 2) & 1];
            let d = [0, 1, 2].map(|a| x[a] - f64::from(bit[a]));
            // Per axis: this corner's factor, its slope and its curvature.
            let w = [0, 1, 2].map(|a| {
                if bit[a] == 1 {
                    f[a]
                } else {
                    [1.0 - f[a][0], -f[a][1], -f[a][2]]
                }
            });
            let weight = w[0][0] * w[1][0] * w[2][0];
            let first = [
                w[0][1] * w[1][0] * w[2][0],
                w[0][0] * w[1][1] * w[2][0],
                w[0][0] * w[1][0] * w[2][1],
            ];
            let second = |i: usize, j: usize| -> f64 {
                let mut p = 1.0;
                for (a, factor) in w.iter().enumerate() {
                    let order = usize::from(a == i) + usize::from(a == j);
                    p *= factor[order];
                }
                p
            };
            let mut best_u = [0.0_f64; 3];
            let mut best_v = [[0.0_f64; 3]; 3];
            for g in &EDGES {
                let value = g[0] * d[0] + g[1] * d[1] + g[2] * d[2];
                for i in 0..3 {
                    best_u[i] = best_u[i].max((first[i] * value + weight * g[i]).abs());
                    for j in 0..3 {
                        let h = second(i, j) * value + first[i] * g[j] + first[j] * g[i];
                        best_v[i][j] = best_v[i][j].max(h.abs());
                    }
                }
            }
            for i in 0..3 {
                u[i] += best_u[i];
                for j in 0..3 {
                    v[i][j] += best_v[i][j];
                }
            }
        }
        let grad = (u[0] * u[0] + u[1] * u[1] + u[2] * u[2]).sqrt();
        let hess = v.iter().flatten().map(|e| e * e).sum::<f64>().sqrt();
        (grad, hess)
    }

    #[test]
    #[ignore = "slow: certifies the noise's gradient and curvature bounds by a grid search"]
    fn noise_derivative_bounds_certified() {
        let steps = 512_u32;
        let half = steps / 2;
        let (mut grad, mut hess) = (0.0_f64, 0.0_f64);
        for i in 0..=half {
            for j in i..=half {
                for k in j..=half {
                    let x = [i, j, k].map(|n| f64::from(n) / f64::from(steps));
                    let (g, h) = envelopes(x);
                    grad = grad.max(g);
                    hess = hess.max(h);
                }
            }
        }
        // Lipschitz constants of the envelopes from the fade's derivative bounds (f' ≤ 1.875,
        // f'' ≤ 5.774, f''' ≤ 60), |g · d| ≤ √6 and |g_i| ≤ 1, summed over the corners: per
        // entry of U's gradient at most 11.55 √6 + 7.5 ≤ 35.8 on the diagonal and 14.07 √6 + 7.5
        // ≤ 41.9 off it; per entry of V's at most 120 √6 + 3 × 14.07.
        let v_max = 6.0_f64.sqrt();
        let grad_lipschitz = 3.0_f64.sqrt() * (35.8_f64 * 35.8 + 2.0 * 41.9 * 41.9).sqrt();
        let hess_lipschitz = 27.0_f64.sqrt() * (120.0 * v_max + 3.0 * 14.07);
        let half_diagonal = 3.0_f64.sqrt() / (2.0 * f64::from(steps));
        let c1 = grad + grad_lipschitz * half_diagonal;
        let c2 = hess + hess_lipschitz * half_diagonal;
        println!("gradient: grid {grad}, certified {c1}; curvature: grid {hess}, certified {c2}");
        assert!(c1 <= NOISE_GRADIENT_BOUND && NOISE_GRADIENT_BOUND - c1 < 0.01);
        assert!(c2 <= NOISE_CURVATURE_BOUND && NOISE_CURVATURE_BOUND - c2 < 0.05);
    }

    fn sphere_point(s: f64, t: f64) -> [f64; 3] {
        unit_dir(face_uv_to_xyz(FaceUv {
            face: Face::PosX,
            u: st_to_uv(s),
            v: st_to_uv(t),
        }))
    }

    #[test]
    #[ignore = "slow: measures the parametric curvature over a face"]
    fn the_parametric_curvature_is_bounded() {
        let n = 256_u32;
        let h = 1e-4;
        let mut worst = 0.0_f64;
        for i in 1..n {
            for j in 1..n {
                let (s, t) = (f64::from(i) / f64::from(n), f64::from(j) / f64::from(n));
                for a in 0..16 {
                    let angle = core::f64::consts::PI * f64::from(a) / 16.0;
                    let (sin, cos) = hyperion_base::math::sin_cos(angle);
                    let at = |k: f64| sphere_point(s + k * h * cos, t + k * h * sin);
                    let (p0, p1, p2) = (at(-1.0), at(0.0), at(1.0));
                    let acc = [0, 1, 2].map(|c| (p0[c] - 2.0 * p1[c] + p2[c]) / (h * h));
                    worst = worst.max((acc[0] * acc[0] + acc[1] * acc[1] + acc[2] * acc[2]).sqrt());
                }
            }
        }
        println!("parametric curvature: grid maximum {worst}");
        assert!(worst * 1.05 <= PARAMETRIC_CURVATURE, "{worst}");
    }

    #[test]
    fn the_bound_falls_with_level_and_is_zero_at_the_finest() {
        for planet in [TEST_PLANET, TEST_PLANET.with_ridges(Ridges::On)] {
            assert!(planet.level_bound_m(19).abs() <= 0.0);
            let mut previous = f64::INFINITY;
            for level in 0..19 {
                let bound = planet.level_bound_m(level);
                assert!(bound.is_finite() && bound > 0.0);
                // The ridges' crests are sharp (their curvature grows as 1 ÷ ε), so a ridged
                // octave's interpolation term can exceed what it adds when omitted.
                if planet.ridges() == Ridges::Off {
                    assert!(bound <= previous, "level {level}: {bound} above {previous}");
                }
                previous = bound;
            }
            for level in 20..=MAX_LEVEL {
                assert!(planet.level_bound_m(level) > 0.0);
            }
        }
    }
}
