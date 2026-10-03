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
//! At the finest level ε is 0; above it, at levels selection never reaches, it is the two
//! interpolation terms alone.
//!
//! # Measured (T6, 2026-10-02; `level_bound_holds`, under `just test-slow`)
//!
//! No sample exceeded the bound at any level, ridges off or on. Per level, 10⁴ samples in 16
//! patches; `σ_n` is the omitted octaves' RMS, `k_n` the implied patch-to-distance ratio at τ = 1 px,
//! 1080p and 60° (for T13.a). Ridges off:
//!
//! | n  | `ε_n` (m) | p99.9 ÷ ε | `σ_n` (m) | p99.9 ÷ 4σ | patch max ÷ 4σ | `k_n`   |
//! | -- | ------- | --------- | ------- | ---------- | -------------- | ----- |
//! | 0  | 7,114   | 0.29      | 645     | 0.81       | 0.93           | 1.27  |
//! | 2  | 3,444   | 0.29      | 320     | 0.78       | 0.90           | 2.46  |
//! | 4  | 1,615   | 0.30      | 156     | 0.77       | 0.85           | 4.62  |
//! | 6  | 715     | 0.32      | 73.3    | 0.79       | 0.84           | 8.18  |
//! | 8  | 281     | 0.33      | 31.3    | 0.74       | 0.89           | 12.8  |
//! | 10 | 80.4    | 0.30      | 7.82    | 0.77       | 0.87           | 14.7  |
//! | 12 | 20.6    | 0.29      | 1.96    | 0.75       | 0.86           | 15.1  |
//! | 14 | 5.06    | 0.28      | 0.488   | 0.73       | 0.93           | 14.8  |
//! | 16 | 1.13    | 0.31      | 0.118   | 0.74       | 0.89           | 13.3  |
//! | 17 | 0.476   | 0.31      | 0.0529  | 0.70       | 0.79           | 11.2  |
//! | 18 | 0.148   | 0.037     | 0       | —          | —              | 6.95  |
//!
//! The hard bound is about 3.5 times the 99.9th percentile, so it is not under a quarter (a
//! finding) except at level 18, which omits no octave and differs from the finest only by
//! interpolation, which the curvature bound overstates. Every patch's maximum is within
//! 1.25 × `4σ_n`, as R10.T4's criterion (ii) asks. With ridges on, the ridged octaves' sharp crests
//! (curvature growing as 1 ÷ ε) make the interpolation term dominate from level 5 to 12, where the
//! 99.9th percentile is 2–10% of the bound (findings) and `k_n` reaches 230 at level 9: selection by
//! the hard bound over-refines the ridged planet heavily there, which the spike's ridged runs will
//! show as demand. Decisions-r05.md item 6 keeps the hard bound in R05 regardless.

use super::{RIDGE_EPSILON, RIDGE_RMS, RIDGED, Ridges, TestPlanet, octaves};
use crate::geometry::{finest_level, lattice_step, vertex_spacing};
use crate::noise::NOISE_RMS;

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
        let (mut hess, mut grad) = (0.0, 0.0);
        for k in 0..count {
            let (kh, kg) = octave_derivative_bounds(self, k);
            hess += kh;
            grad += kg;
        }
        0.5 * hess * h * h + 0.25 * grad * PARAMETRIC_CURVATURE * a * step * step
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
