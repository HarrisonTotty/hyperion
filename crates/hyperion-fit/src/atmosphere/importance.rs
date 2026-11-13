//! The deep estimator's importance (R08.T12.h): Ψ(h, μ₀), the mean intensity of the forward solar
//! field at height h under a sun at local cosine μ₀, per unit irradiance, from a δ-Eddington
//! diffusion solve of the case's own column.
//!
//! A backward path's expected score from a vertex is, to the diffusion approximation, the forward
//! field's mean intensity there, so Ψ(x) ÷ Ψ(x₀) is the importance the weight windows of the
//! `transport` module split and roulette by (the deterministic adjoint as Monte Carlo's importance
//! map, as CADIS takes it: Wagner and Haghighat 1998, Nucl. Sci. Eng. 128, 186). It only steers:
//! splitting and roulette by any positive importance are unbiased (Spanier and Gelbard 1969,
//! _Monte Carlo Principles and Neutron Transport Problems_), so its error costs efficiency, never
//! bias, and it never enters an estimate.
//!
//! - **The Eddington equations** (Shettle and Weinman 1970, J. Atmos. Sci. 27, 1048): with τ down
//!   from the top and I(τ, μ) = I₀ + μ I₁, μ the cosine from the upward vertical,
//!   `(1 ÷ 3) dI₁/dτ = (1 − ω) I₀ − ω B` and `dI₀/dτ = (1 − ω g) I₁ + 3 ω g μ₀ B`, B = T ÷ 4π the
//!   direct beam's mean intensity per unit irradiance, T its transmittance from the top.
//! - **δ-scaled** (Joseph, Wiscombe and Weinman 1976, J. Atmos. Sci. 33, 2452): a fraction f = g²
//!   of each layer's scattering is taken as forward, τ′ = (1 − ω f) τ, ω′ = (1 − f) ω ÷ (1 − ω f),
//!   g′ = (g − f) ÷ (1 − f), and the beam is attenuated by the scaled extinction.
//! - **The beam pseudo-spherical** (Dahlback and Stamnes 1991, Planet. Space Sci. 39, 671): T at each
//!   node is the slant transmittance along the true ray towards the sun through the case's own
//!   spherical (or spheroidal) shells, which the caller marches, so that a sun below the local
//!   horizon still lights the column's upper part.
//! - **The boundaries:** Marshak's at the top (no diffuse light coming down, I₁ = (3 ÷ 2) I₀;
//!   Marshak 1947, Phys. Rev. 71, 443), and a Lambertian ground of albedo A, whose upward flux
//!   π I₀ + (2π ÷ 3) I₁ is A times the downward diffuse and direct flux.
//! - **The discretisation:** finite volumes about each node of the zeroth-moment equation, I₁
//!   between two nodes from the first-moment equation by central differences, the beam's value
//!   between them their geometric mean; a tridiagonal system solved by the Thomas algorithm. Its
//!   matrix is diagonally dominant with a negative diagonal and positive neighbours, so I₀ is not
//!   negative wherever its right side is not positive: under a sun that is up, where
//!   3 ω′ g′ μ₀ B ÷ (1 − ω′ g′) does not grow with depth. Under a set sun, or at a cloud's top,
//!   that is not guaranteed, and ln Ψ is read from Ψ floored at the least positive `f64`.
//!
//! Ψ = I₀ + B is tabulated at the column's nodes and at [`MU_NODES`] values of μ₀ from −0.5 to 1
//! in steps of 0.05, and read with ln Ψ bilinear in h and μ₀, both clamped to the table.

use core::f64::consts::PI;

use hyperion_sim::math;

/// The values of μ₀ the table holds, from −0.5 to 1 in steps of 0.05: below −0.5 a sun is 30°
/// under the horizon, which lights nothing of a column whose top is below R (sec 30° − 1) =
/// 0.155 R (the Titan-class case's top is 390 km of R's 2,575 km); a lower sun's Ψ is the −0.5
/// row's, which only steers.
pub(crate) const MU_NODES: usize = 31;

/// The least δ-scaled optical depth an interval is solved with, so that its coupling stays finite
/// in a vacuum near the top: two nodes this close are held equal.
const LEAST_SCALED_DEPTH: f64 = 1e-9;

/// The table's μ₀ at node `k` of [`MU_NODES`]: (k − 10) ÷ 20.
#[must_use]
pub(crate) fn mu_node(k: usize) -> f64 {
    let k = u32::try_from(k).expect("the table holds 31 nodes of μ₀");
    (f64::from(k) - 10.0) / 20.0
}

/// One interval of the column between two neighbouring nodes, as the tracer reads it.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct Interval {
    /// Its scattering optical depth, vertical.
    pub(crate) scattering_depth: f64,
    /// Its absorption optical depth, vertical.
    pub(crate) absorption_depth: f64,
    /// Its scattering-weighted mean cosine, Σ τₛ g ÷ τₛ over its shells' parts.
    pub(crate) asymmetry: f64,
    /// Its δ-scaling's forward fraction, Σ τₛ g² ÷ τₛ: f = g² shell by shell.
    pub(crate) forward_fraction: f64,
}

/// An interval's δ-scaled optical depth, single-scattering albedo and asymmetry.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Scaled {
    depth: f64,
    albedo: f64,
    asymmetry: f64,
}

impl Interval {
    /// Its δ-scaled optics (module documentation).
    #[must_use]
    fn scaled(&self) -> Scaled {
        let tau = self.scattering_depth + self.absorption_depth;
        if self.scattering_depth <= 0.0 || tau <= 0.0 {
            return Scaled {
                depth: tau.max(LEAST_SCALED_DEPTH),
                albedo: 0.0,
                asymmetry: 0.0,
            };
        }
        let omega = self.scattering_depth / tau;
        let f = self.forward_fraction;
        let g = self.asymmetry;
        Scaled {
            depth: (tau * (1.0 - omega * f)).max(LEAST_SCALED_DEPTH),
            albedo: omega * (1.0 - f) / (1.0 - omega * f),
            asymmetry: if f < 1.0 { (g - f) / (1.0 - f) } else { 0.0 },
        }
    }
}

/// The δ-Eddington solution of a column under one sun, per unit irradiance, its nodes from the
/// top down.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ColumnSolution {
    /// The diffuse mean intensity I₀ at each node, sr⁻¹.
    diffuse: Vec<f64>,
    /// The direct beam's mean intensity B = T ÷ 4π at each node, sr⁻¹.
    beam: Vec<f64>,
    /// The diffuse first moment I₁ at the ground, sr⁻¹, upward positive.
    ground_first_moment: f64,
}

impl ColumnSolution {
    /// The downward flux at the ground per unit irradiance under a sun at local cosine `mu0`: the
    /// diffuse π I₀ − (2π ÷ 3) I₁, and the δ-scaled beam's μ₀ T where the sun is up.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn ground_downward_flux(&self, mu0: f64) -> f64 {
        let ground = self.diffuse.len() - 1;
        let diffuse = PI * self.diffuse[ground] - 2.0 * PI / 3.0 * self.ground_first_moment;
        diffuse + mu0.max(0.0) * 4.0 * PI * self.beam[ground]
    }
}

/// The δ-Eddington solve of `intervals`, from the top down, over a Lambertian ground of albedo
/// `albedo`, under a sun at local cosine `mu0` whose beam's δ-scaled transmittance at each node,
/// from the top down, is `transmittance` (module documentation).
///
/// # Panics
///
/// If `transmittance` has a length other than one more than `intervals`', or `intervals` is
/// empty: a caller's bug.
#[must_use]
pub(crate) fn solve(
    intervals: &[Interval],
    transmittance: &[f64],
    albedo: f64,
    mu0: f64,
) -> ColumnSolution {
    let ground = intervals.len();
    assert!(ground > 0, "a column has at least one interval");
    assert_eq!(
        transmittance.len(),
        ground + 1,
        "a transmittance at every node"
    );
    let beam: Vec<f64> = transmittance.iter().map(|t| t / (4.0 * PI)).collect();
    let scaled: Vec<Scaled> = intervals.iter().map(Interval::scaled).collect();
    // Each interval's 3 ω′ g′ μ₀ B at its middle, and 1 − ω′ g′.
    let beam_term = |k: usize| {
        let layer = scaled[k];
        3.0 * layer.albedo * layer.asymmetry * mu0 * (beam[k] * beam[k + 1]).sqrt()
    };
    let damping = |k: usize| 1.0 - scaled[k].albedo * scaled[k].asymmetry;
    // The ground's I₁ = p I₀ + q.
    let ground_slope = 3.0 * (albedo - 1.0) / (2.0 * (1.0 + albedo));
    let ground_source = 6.0 * albedo * mu0.max(0.0) * beam[ground] / (1.0 + albedo);
    // below I₀[j − 1] + diagonal I₀[j] + above I₀[j + 1] = right at each node j, 0 the top.
    let nodes = ground + 1;
    let mut below = vec![0.0; nodes];
    let mut diagonal = vec![0.0; nodes];
    let mut above = vec![0.0; nodes];
    let mut right = vec![0.0; nodes];
    for j in 0..nodes {
        let (mut absorbed, mut scattered) = (0.0, 0.0);
        if j > 0 {
            // −(1 ÷ 3) I₁ half an interval above, I₁ = ((I₀[j] − I₀[j − 1]) ÷ τ′ − beam) ÷ e.
            let layer = scaled[j - 1];
            let coupling = 1.0 / (3.0 * layer.depth * damping(j - 1));
            below[j] = coupling;
            diagonal[j] -= coupling;
            right[j] -= beam_term(j - 1) / (3.0 * damping(j - 1));
            absorbed += 0.5 * (1.0 - layer.albedo) * layer.depth;
            scattered += 0.5 * layer.albedo * layer.depth;
        }
        if j < ground {
            // +(1 ÷ 3) I₁ half an interval below.
            let layer = scaled[j];
            let coupling = 1.0 / (3.0 * layer.depth * damping(j));
            above[j] = coupling;
            diagonal[j] -= coupling;
            right[j] += beam_term(j) / (3.0 * damping(j));
            absorbed += 0.5 * (1.0 - layer.albedo) * layer.depth;
            scattered += 0.5 * layer.albedo * layer.depth;
        }
        diagonal[j] -= absorbed;
        right[j] -= scattered * beam[j];
        if j == 0 {
            // Marshak's top: −(1 ÷ 3) I₁ there is −I₀ ÷ 2.
            diagonal[j] -= 0.5;
        }
        if j == ground {
            // The ground's I₁, +(1 ÷ 3)(p I₀ + q).
            diagonal[j] += ground_slope / 3.0;
            right[j] -= ground_source / 3.0;
        }
    }
    // The Thomas algorithm.
    for j in 1..nodes {
        let factor = below[j] / diagonal[j - 1];
        diagonal[j] -= factor * above[j - 1];
        right[j] -= factor * right[j - 1];
    }
    let mut diffuse = vec![0.0; nodes];
    diffuse[ground] = right[ground] / diagonal[ground];
    for j in (0..ground).rev() {
        diffuse[j] = (right[j] - above[j] * diffuse[j + 1]) / diagonal[j];
    }
    let ground_first_moment = ground_slope * diffuse[ground] + ground_source;
    ColumnSolution {
        diffuse,
        beam,
        ground_first_moment,
    }
}

/// Ψ(h, μ₀) at the column's nodes and the table's μ₀ (module documentation).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Importance {
    /// The nodes' heights, ascending, m: gravity-scaled on a spheroid, as the profiles read them.
    heights_m: Vec<f64>,
    /// The table's μ₀, [`mu_node`]'s.
    mus: [f64; MU_NODES],
    /// ln Ψ by μ₀ node, then by height node, ascending.
    ln_psi: Vec<f64>,
}

impl Importance {
    /// The table of the column of `intervals`, from the top down, its nodes' heights `heights_m`
    /// from the top down, over a Lambertian ground of albedo `albedo`, the beam's δ-scaled
    /// transmittance at node j under a sun at local cosine μ₀ `transmittance(j, μ₀)`.
    ///
    /// # Panics
    ///
    /// As [`solve`], if `heights_m` is not one longer than `intervals`.
    #[must_use]
    pub(crate) fn new(
        intervals: &[Interval],
        heights_m: &[f64],
        albedo: f64,
        mut transmittance: impl FnMut(usize, f64) -> f64,
    ) -> Self {
        assert_eq!(
            heights_m.len(),
            intervals.len() + 1,
            "a height at every node"
        );
        let n = heights_m.len();
        let mut ln_psi = Vec::with_capacity(MU_NODES * n);
        for k in 0..MU_NODES {
            let mu0 = mu_node(k);
            let beam: Vec<f64> = (0..n).map(|j| transmittance(j, mu0)).collect();
            let solution = solve(intervals, &beam, albedo, mu0);
            // Ascending in height: the solution's nodes run from the top down.
            ln_psi.extend(
                solution
                    .diffuse
                    .iter()
                    .zip(&solution.beam)
                    .rev()
                    .map(|(&i0, &b)| math::ln((i0 + b).max(f64::MIN_POSITIVE))),
            );
        }
        Self {
            heights_m: heights_m.iter().rev().copied().collect(),
            mus: core::array::from_fn(mu_node),
            ln_psi,
        }
    }

    /// Ψ at height `height_m` under a sun at local cosine `mu0`, per unit irradiance, sr⁻¹: ln Ψ
    /// bilinear between the table's nodes, both clamped to the table.
    #[must_use]
    pub(crate) fn at(&self, height_m: f64, mu0: f64) -> f64 {
        let (node, up) = between(&self.heights_m, height_m);
        let (row, along) = between(&self.mus, mu0);
        let nodes = self.heights_m.len();
        let read = |row: usize| {
            let at = &self.ln_psi[row * nodes..(row + 1) * nodes];
            at[node] + (at[node + 1] - at[node]) * up
        };
        let (low, high) = (read(row), read(row + 1));
        math::exp(low + (high - low) * along)
    }
}

/// The node of ascending `nodes` below `x` and the fraction of the way to the next, clamped to
/// the nodes.
#[must_use]
fn between(nodes: &[f64], x: f64) -> (usize, f64) {
    let last = nodes.len() - 1;
    let i = nodes.partition_point(|&v| v <= x).clamp(1, last) - 1;
    let fraction = ((x - nodes[i]) / (nodes[i + 1] - nodes[i])).clamp(0.0, 1.0);
    (i, fraction)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A uniform column of `tau` in `m` intervals, single-scattering albedo `omega` and asymmetry
    /// `g`, from the top down, with the plane-parallel beam under μ₀ at each node.
    fn uniform(tau: f64, omega: f64, g: f64, m: u32, mu0: f64) -> (Vec<Interval>, Vec<f64>) {
        let part = tau / f64::from(m);
        let intervals = vec![
            Interval {
                scattering_depth: omega * part,
                absorption_depth: (1.0 - omega) * part,
                asymmetry: g,
                forward_fraction: g * g,
            };
            usize::try_from(m).unwrap()
        ];
        let f = g * g;
        let scaled = tau * (1.0 - omega * f) / f64::from(m);
        let beam = (0..=m)
            .map(|j| math::exp(-scaled * f64::from(j) / mu0))
            .collect();
        (intervals, beam)
    }

    #[test]
    fn atmosphere_eddington_conserves_energy_in_a_conservative_slab() {
        // ω = 1 over a black ground: what is not reflected at the top is transmitted, diffusely or
        // in the beam, to the trapezoid rule's error in the beam's own attenuation, under 10⁻⁵ of
        // the incident flux over these intervals of τ 0.01 (τ′ 0.01 to 0.0028), and some
        // 2 × 10⁻⁴ over the tracer's own of τ 0.05.
        for (g, mu0) in [(0.0, 1.0), (0.7, 0.5), (0.85, 0.866)] {
            let (intervals, beam) = uniform(6.0, 1.0, g, 600, mu0);
            let solution = solve(&intervals, &beam, 0.0, mu0);
            let up = PI * solution.diffuse[0] + 2.0 * PI / 3.0 * 1.5 * solution.diffuse[0];
            let down = solution.ground_downward_flux(mu0);
            assert!(
                (up + down - mu0).abs() < 1e-3 * mu0,
                "g {g}, μ₀ {mu0}: {up} + {down} against {mu0}"
            );
            assert!(solution.diffuse.iter().all(|&x| x > 0.0 && x.is_finite()));
        }
    }

    #[test]
    fn atmosphere_eddington_reflects_a_thick_conservative_layer_and_a_white_ground_whole() {
        // A conservative semi-infinite layer reflects everything, so one of τ 2,000 over a black
        // ground, whose diffuse transmission is some 4 ÷ 3τ, reflects within 1% of it (in
        // intervals of τ 0.05, where the trapezoid rule's error in the beam is 6 × 10⁻⁴); and a
        // white ground under a thin layer gives back what reaches it.
        let mu0 = 0.6;
        let (intervals, beam) = uniform(2000.0, 1.0, 0.0, 40_000, mu0);
        let solution = solve(&intervals, &beam, 0.0, mu0);
        let up = PI * solution.diffuse[0] * 2.0;
        assert!((up / mu0 - 1.0).abs() < 0.01, "{up} against {mu0}");
        let (intervals, beam) = uniform(1e-6, 1.0, 0.0, 1, mu0);
        let solution = solve(&intervals, &beam, 1.0, mu0);
        let up = PI * solution.diffuse[0] * 2.0;
        assert!((up / mu0 - 1.0).abs() < 1e-4, "{up} against {mu0}");
    }

    #[test]
    fn atmosphere_eddington_reflects_as_meador_and_weavers_closed_form() {
        // A conservative layer's plane albedo under the Eddington approximation (no δ-scaling),
        // Meador and Weaver 1980 (J. Atmos. Sci. 37, 630), Table 1's Eddington γ₁ = 3(1 − g) ÷ 4
        // and γ₃ = (2 − 3 g μ₀) ÷ 4 at ω = 1, and their conservative solution over a black ground,
        // R = (γ₁ τ + (γ₃ − γ₁ μ₀)(1 − e^(−τ/μ₀))) ÷ (1 + γ₁ τ). The beam's anisotropic source
        // sets γ₃'s sign: the other sign reads 0.64 here, not 0.27.
        for (tau, g, mu0) in [(1.0, 0.7, 0.5), (6.0, 0.85, 0.8), (0.3, 0.0, 0.3)] {
            let m = 4000;
            let part = tau / f64::from(m);
            let intervals = vec![
                Interval {
                    scattering_depth: part,
                    absorption_depth: 0.0,
                    asymmetry: g,
                    forward_fraction: 0.0,
                };
                usize::try_from(m).unwrap()
            ];
            let beam: Vec<f64> = (0..=m)
                .map(|j| math::exp(-part * f64::from(j) / mu0))
                .collect();
            let solution = solve(&intervals, &beam, 0.0, mu0);
            let reflected = 2.0 * PI * solution.diffuse[0] / mu0;
            let (gamma1, gamma3) = (0.75 * (1.0 - g), (2.0 - 3.0 * g * mu0) / 4.0);
            let expected = (gamma1 * tau + (gamma3 - gamma1 * mu0) * -math::exp_m1(-tau / mu0))
                / (1.0 + gamma1 * tau);
            assert!(
                (reflected - expected).abs() < 1e-3,
                "τ {tau}, g {g}, μ₀ {mu0}: {reflected} against {expected}"
            );
        }
    }

    #[test]
    fn atmosphere_importance_reads_its_nodes_and_clamps() {
        // At a node of both height and μ₀ the table gives the solve's Ψ, between them ln Ψ is
        // bilinear, and outside them it is the edge's.
        let (intervals, _) = uniform(8.0, 0.95, 0.6, 160, 1.0);
        let heights: Vec<f64> = (0..=160).rev().map(|j| f64::from(j) * 100.0).collect();
        let f = 0.36;
        let scaled = 8.0 * (1.0 - 0.95 * f) / 160.0;
        let transmittance = |j: usize, mu0: f64| {
            if mu0 <= 0.0 {
                0.0
            } else {
                math::exp(-scaled * f64::from(u32::try_from(j).unwrap()) / mu0)
            }
        };
        let importance = Importance::new(&intervals, &heights, 0.3, transmittance);
        let mu0 = mu_node(26);
        let beam: Vec<f64> = (0..=160).map(|j| transmittance(j, mu0)).collect();
        let solution = solve(&intervals, &beam, 0.3, mu0);
        // Node j from the top is height (160 − j) × 100 m.
        for j in [0, 40, 160] {
            let psi = solution.diffuse[j] + solution.beam[j];
            let h = heights[j];
            assert!(
                (importance.at(h, mu0) / psi - 1.0).abs() < 1e-12,
                "node {j}"
            );
        }
        let (a, b) = (
            importance.at(500.0, mu_node(20)),
            importance.at(500.0, mu_node(21)),
        );
        let between = importance.at(500.0, f64::midpoint(mu_node(20), mu_node(21)));
        assert!((between / (a * b).sqrt() - 1.0).abs() < 1e-12);
        assert!((importance.at(-50.0, 2.0) / importance.at(0.0, 1.0) - 1.0).abs() < 1e-15);
        assert!((importance.at(1e9, -1.0) / importance.at(16_000.0, -0.5) - 1.0).abs() < 1e-15);
        // Under a set sun only the diffuse field from what the beam lit is left: none here, so
        // Ψ is the floor's, positive and finite.
        assert!(importance.at(0.0, -0.5) > 0.0);
    }
}
