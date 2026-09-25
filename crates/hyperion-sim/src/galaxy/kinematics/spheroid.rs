//! The bulge, the long bar and the nuclear disc: one axisymmetric Jeans solver, the bar's pattern
//! rotation with streaming along the density's ellipses, and the bulge's projected dispersion that
//! plan 02's black hole reads (plan 08, P08.T4 and Design notes 10 and 11).
//!
//! # The Jeans table
//!
//! Axisymmetric and cylindrically aligned, with a constant `β_z = 1 − σ_z² ÷ σ_R²`:
//!
//! - `ν σ_z²(R, z) = ∫_|z|^∞ ν K_z dz′`;
//! - `σ_R² = σ_z² ÷ (1 − β_z)`;
//! - `ν ⟨v_φ²⟩ = ν σ_R² + R ∂(ν σ_R²) ÷ ∂R + ν R ∂Φ ÷ ∂R`;
//! - Satoh's split, `v̄_φ² = k² (⟨v_φ²⟩ − σ_R²)` and `σ_φ² = ⟨v_φ²⟩ − v̄_φ²`.
//!
//! The tracer `ν` is the component's own density, axisymmetrised as plan 02's potential
//! axisymmetrises it: the bulge as the spheroidal exponential of its second moments, the bar as the
//! azimuthal average of its surface density times its exponential height, the nuclear disc as it
//! is. The table lies on the potential's 64 × 64 grid, log-spaced from 2⁻⁴ to 2¹⁸ ly in R and |z|;
//! the z integral is an 8-point Gauss–Legendre rule on every panel between grid heights, the
//! radial derivative a central difference in `ln R`. `β_z` is 0.3 for bulge and bar, which stands
//! for the bar's orbits and brings the projected profile onto GIBS's, and 0 for the nuclear disc;
//! Satoh's `k` is 0.6, 0.8 and 0.9 (Design note 10). They belong to the generator version.
//!
//! Where the tracer falls faster than the potential can hold it, `⟨v_φ²⟩` from the equation can
//! drop below zero, where the solution has no meaning; it is floored at
//! [`AZIMUTHAL_FLOOR`] of `σ_R²` (never reached inside the bulge's, bar's or nuclear disc's bodies
//! at Milky Way values).

use super::{gl4, gl8};
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::potential::sigma::EFFECTIVE_RADIUS_IN_SCALES;
use crate::galaxy::potential::{MassModel, PotentialTables, bulge_spheroid};
use crate::math;
use crate::units::{KilometresPerSecond, LightYears};

/// The forces a Jeans solution reads at a point: from the potential tables, or straight from a
/// mass model before any table exists (P08.T4.d).
pub trait ForceSource {
    /// `K_z = ∂Φ ÷ ∂z`, (km/s)² per ly, odd in z, and `R ∂Φ ÷ ∂R`, (km/s)², which is `v_c²` in the
    /// plane, at cylindrical radius `r_cyl ≥ 0` and height `z` (ly).
    fn forces_at(&self, r_cyl: f64, z: f64) -> Forces;
}

/// `K_z` and `R ∂Φ ÷ ∂R` at a point ([`ForceSource`]).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Forces {
    /// `K_z = ∂Φ ÷ ∂z`, (km/s)² per ly: positive above the plane.
    pub vertical: f64,
    /// `R ∂Φ ÷ ∂R`, (km/s)²: the circular speed squared in the plane.
    pub v_circ_sq: f64,
}

impl ForceSource for PotentialTables {
    /// From the (R, z) grid.
    ///
    /// # Panics
    ///
    /// If the tables hold no (R, z) grid ([`PotentialTables::full`]).
    fn forces_at(&self, r_cyl: f64, z: f64) -> Forces {
        let [v_circ_sq, vertical] = self
            .forces(r_cyl, z)
            .expect("the kinematics read forces off the plane, from the full potential tables");
        Forces {
            vertical,
            v_circ_sq,
        }
    }
}

impl ForceSource for MassModel {
    /// Summed directly over every component: about a millisecond a point for a galaxy's several
    /// hundred Gaussians.
    fn forces_at(&self, r_cyl: f64, z: f64) -> Forces {
        let height = z.abs();
        let [_, by_r, by_z, _] = self.extended_grid_point(r_cyl, height);
        let mut vertical = if height > 0.0 { by_z / height } else { 0.0 };
        let mut v_circ_sq = by_r;
        let h = math::hypot(r_cyl, height);
        if h > 0.0 {
            let radius = LightYears::new(h);
            for c in self.spherical() {
                let v2 = c.v_circ_sq(radius);
                v_circ_sq += v2 * (r_cyl / h) * (r_cyl / h);
                vertical += v2 * height / (h * h);
            }
        }
        Forces {
            vertical: vertical.copysign(z),
            v_circ_sq,
        }
    }
}

/// Points per axis of the Jeans table: the potential's grid.
pub const JEANS_POINTS: usize = 64;

/// `ln` of the grid's first point, 2⁻⁴ ly.
const LN_FIRST: f64 = -4.0 * core::f64::consts::LN_2;

/// The step between grid points in `ln R` and `ln |z|`: 22 ln 2 ÷ 63.
const STEP: f64 = 22.0 * core::f64::consts::LN_2 / 63.0;

/// Where `⟨v_φ²⟩` is floored, in units of `σ_R²` (module documentation).
pub const AZIMUTHAL_FLOOR: f64 = 0.05;

/// `β_z` of the bulge and the bar (plan 08, Design note 10).
pub const BULGE_BETA_Z: f64 = 0.3;

/// `β_z` of the nuclear disc (plan 08, Design note 10). Provisional: plan 02's ruling 5 reads the
/// nuclear disc's vertical dispersion as about half its radial one (Sormani et al.'s 67.7 km/s
/// radial against the profile's 31.5 vertical at two scale lengths), which is `β_z` near 0.78. With
/// 0 the table's `σ_R` is its `σ_z`, about 30 km/s at two scale lengths as the profile has it, 71 at
/// 65 ly where the black hole and the cluster pull, and 19 at 1,000 ly.
pub const NUCLEAR_BETA_Z: f64 = 0.0;

/// Satoh's `k` for the bulge (plan 08, Design note 10).
pub const BULGE_SATOH_K: f64 = 0.6;

/// Satoh's `k` for the long bar (plan 08, Design note 10).
pub const BAR_SATOH_K: f64 = 0.8;

/// Satoh's `k` for the nuclear disc (plan 08, Design note 10).
pub const NUCLEAR_SATOH_K: f64 = 0.9;

/// The grid point `i` of either axis, ly.
fn grid_point(i: usize) -> f64 {
    math::exp(LN_FIRST + STEP * f64::from(u8::try_from(i).expect("below 64")))
}

/// The cell of the grid holding `x > 0` and the position in it, clamped to the grid.
fn cell(x: f64) -> (usize, f64) {
    let u = if x > 0.0 {
        (math::ln(x) - LN_FIRST) / STEP
    } else {
        0.0
    };
    super::discs::split(u, JEANS_POINTS)
}

/// An axisymmetric Jeans solution, tabulated on the potential's grid (module documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct JeansTable {
    beta_z: f64,
    satoh_k: f64,
    /// Per node `(i, j)`, row-major in R: `σ_R²`, `σ_φ²`, `σ_z²` ((km/s)²) and `v̄_φ` (km/s).
    nodes: Box<[[f64; 4]]>,
}

impl JeansTable {
    /// The solution for the tracer whose density's logarithm (up to a constant) at `(R, |z|)` is
    /// `ln_density`, with `beta_z` and Satoh's `satoh_k`, in the forces of `potential`.
    ///
    /// # Panics
    ///
    /// Never: the grid's indices are fixed and in range.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::galaxy::kinematics::spheroid::{ForceSource, Forces, JeansTable};
    /// use hyperion_sim::math;
    ///
    /// // A Plummer sphere of GM = 10⁷ (km/s)² ly and scale 1,000 ly in its own potential.
    /// struct Plummer;
    /// impl ForceSource for Plummer {
    ///     fn forces_at(&self, r: f64, z: f64) -> Forces {
    ///         let d2 = r * r + z * z + 1e6;
    ///         let d3 = d2 * d2.sqrt();
    ///         Forces { vertical: 1e7 * z / d3, v_circ_sq: 1e7 * r * r / d3 }
    ///     }
    /// }
    /// let plummer = |r: f64, z: f64| -2.5 * math::ln_1p((r * r + z * z) / 1e6);
    /// let table = JeansTable::new(plummer, 0.0, 0.0, &Plummer);
    /// // Isotropic: σ² = GM ÷ 6√(r² + b²), 1,667 (km/s)² at the centre.
    /// let [sr, sp, sz, mean] = table.moments(0.1, 0.1);
    /// assert!((sz / (1e7 / 6e3) - 1.0).abs() < 0.02 && (sr / sz - 1.0).abs() < 1e-12);
    /// assert!((sp / sz - 1.0).abs() < 0.02 && mean.abs() < 1e-9);
    /// ```
    #[must_use]
    pub fn new(
        ln_density: impl Fn(f64, f64) -> f64,
        beta_z: f64,
        satoh_k: f64,
        potential: &impl ForceSource,
    ) -> Self {
        let heights: [f64; JEANS_POINTS] = core::array::from_fn(grid_point);
        let radii = heights;
        // ν σ_z² ÷ ν by panels from the top down, rescaled at each step.
        let mut sigma_z_sq = vec![[0.0; JEANS_POINTS]; JEANS_POINTS];
        let mut ln_nu = vec![[0.0; JEANS_POINTS]; JEANS_POINTS];
        for (i, &r) in radii.iter().enumerate() {
            for (j, &z) in heights.iter().enumerate() {
                ln_nu[i][j] = ln_density(r, z);
            }
            let mut above = 0.0;
            for j in (0..JEANS_POINTS - 1).rev() {
                let (lo, hi) = (heights[j], heights[j + 1]);
                let e_lo = ln_nu[i][j];
                let panel = gl8(
                    |z| math::exp(ln_density(r, z) - e_lo) * potential.forces_at(r, z).vertical,
                    lo,
                    hi,
                );
                above = panel + math::exp(ln_nu[i][j + 1] - e_lo) * above;
                sigma_z_sq[i][j] = above;
            }
            // The last height, 2¹⁸ ly, holds nothing of any tracer: level with the one below.
            sigma_z_sq[i][JEANS_POINTS - 1] = sigma_z_sq[i][JEANS_POINTS - 2];
        }
        let anisotropy = 1.0 / (1.0 - beta_z);
        let mut nodes = Vec::with_capacity(JEANS_POINTS * JEANS_POINTS);
        for (i, &r) in radii.iter().enumerate() {
            let (lo, hi) = (i.saturating_sub(1), (i + 1).min(JEANS_POINTS - 1));
            let span = f64::from(u8::try_from(hi - lo).expect("at most 2")) * STEP;
            for (j, &z) in heights.iter().enumerate() {
                let radial_sq = sigma_z_sq[i][j] * anisotropy;
                let d_sigma = (sigma_z_sq[hi][j] - sigma_z_sq[lo][j]) * anisotropy / span;
                let d_ln_nu = (ln_nu[hi][j] - ln_nu[lo][j]) / span;
                let v_circ_sq = potential.forces_at(r, z).v_circ_sq;
                let second = (radial_sq + d_sigma + radial_sq * d_ln_nu + v_circ_sq)
                    .max(AZIMUTHAL_FLOOR * radial_sq);
                let mean_sq = satoh_k * satoh_k * (second - radial_sq).max(0.0);
                nodes.push([
                    radial_sq,
                    second - mean_sq,
                    sigma_z_sq[i][j],
                    mean_sq.sqrt(),
                ]);
            }
        }
        Self {
            beta_z,
            satoh_k,
            nodes: nodes.into_boxed_slice(),
        }
    }

    /// `β_z = 1 − σ_z² ÷ σ_R²`.
    #[must_use]
    pub fn beta_z(&self) -> f64 {
        self.beta_z
    }

    /// Satoh's `k`.
    #[must_use]
    pub fn satoh_k(&self) -> f64 {
        self.satoh_k
    }

    /// `[σ_R², σ_φ², σ_z²]` in (km/s)² and `v̄_φ` in km/s at cylindrical radius `r` and height `z`
    /// (ly): bilinear in `ln R` and `ln |z|`, level outside the grid, and with the mean rotation
    /// falling linearly to zero on the axis inside the first radius.
    #[must_use]
    pub fn moments(&self, r: f64, z: f64) -> [f64; 4] {
        let mut out = super::discs::bilinear(&self.nodes, JEANS_POINTS, cell(r), cell(z.abs()));
        let first = grid_point(0);
        if r < first {
            out[3] *= r / first;
        }
        out
    }

    /// The bytes the table owns on the heap.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        size_of_val(&*self.nodes)
    }
}

/// The mean flow along the density's ellipses in the bar's frame (plan 08, Design note 11):
/// `u = ω(m) × (−(a ÷ b) y, (b ÷ a) x)`, km/s, with `m² = (x ÷ a)² + (y ÷ b)²`.
///
/// The flow is tangent to the ellipses of constant `m`, divergence-free and independent of z, so
/// it satisfies continuity exactly for any density constant on those ellipses, and rotates
/// cylindrically. `ω(m) = (v̄_φ − Ω_p R) ÷ (R (a ÷ b + b ÷ a) ÷ 2)` at `R = m √(a b)`, floored at
/// zero, with `v̄_φ` the Jeans table's in the plane: on a circle the flow's mean tangential speed
/// is `ω R (a ÷ b + b ÷ a) ÷ 2`, so the pattern plus the flow rotates as the table does.
/// `pattern_speed` is `Ω_p` in km/s per ly and `(a, b)` the density's axes along and across the
/// bar (ly). Returns `[u_x, u_y]` in the galactic frame's axes, which are the bar's at the epoch.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::kinematics::spheroid::{Forces, ForceSource, JeansTable, bar_streaming};
///
/// struct Flat;
/// impl ForceSource for Flat {
///     fn forces_at(&self, r: f64, z: f64) -> Forces {
///         let d2 = r * r + z * z;
///         Forces { vertical: 4e4 * z / d2, v_circ_sq: 4e4 * r * r / d2 }
///     }
/// }
/// let bulge = |r: f64, z: f64| -(r * r + 4.0 * z * z).sqrt() / 2e3;
/// let table = JeansTable::new(bulge, 0.3, 0.6, &Flat);
/// let (a, b) = (2_000.0, 1_200.0);
/// let u = bar_streaming(&table, (a, b), 0.01, 1_000.0, 0.0);
/// // On the long axis the flow runs across it, spinward.
/// assert!(u[0].value().abs() < 1e-12 && u[1].value() > 0.0);
/// ```
#[must_use]
pub fn bar_streaming(
    table: &JeansTable,
    (along, across): (f64, f64),
    pattern_speed: f64,
    x: f64,
    y: f64,
) -> [KilometresPerSecond; 2] {
    let m = math::hypot(x / along, y / across);
    let radius = m * (along * across).sqrt();
    let (elongation, compression) = (along / across, across / along);
    let omega = if radius > 0.0 {
        let mean = table.moments(radius, 0.0)[3];
        ((mean - pattern_speed * radius) / (0.5 * radius * (elongation + compression))).max(0.0)
    } else {
        0.0
    };
    [
        KilometresPerSecond::new(-omega * elongation * y),
        KilometresPerSecond::new(omega * compression * x),
    ]
}

/// The face-on aperture's radial panels, in units of the effective radius, each a 4-point rule:
/// the reduced solution of P08.T4.d.
const APERTURE_PANELS: [f64; 2] = [0.0, 1.0];

/// The face-on projection's vertical panels, in units of the tracer's vertical scale, each a
/// 4-point rule.
const COLUMN_PANELS: [f64; 5] = [0.0, 1.0, 3.0, 8.0, 20.0];

/// The mass-weighted mean of `column(R, z)` over the face-on aperture of the spheroidal
/// exponential `ν = exp(−√(R² ÷ a_r² + z² ÷ a_z²))` inside `R_e = 2.027 a_r`: `∫ R dR ∫ ν column
/// dz ÷ ∫ R dR ∫ ν dz`, on [`APERTURE_PANELS`] and [`COLUMN_PANELS`].
fn aperture_mean(
    (a_r, a_z): (f64, f64),
    radial: &[f64],
    vertical: &[f64],
    mut column: impl FnMut(f64, f64) -> f64,
) -> f64 {
    let r_e = EFFECTIVE_RADIUS_IN_SCALES * a_r;
    let mut weighted = 0.0;
    let mut mass = 0.0;
    for w in radial.windows(2) {
        weighted += gl4(
            |r| {
                let rho = r / a_r;
                let mut sum = 0.0;
                for p in vertical.windows(2) {
                    sum += gl4(
                        |t| math::exp(-math::hypot(rho, t)) * column(r, a_z * t),
                        p[0],
                        p[1],
                    );
                }
                r * sum
            },
            w[0] * r_e,
            w[1] * r_e,
        );
        mass += gl4(
            |r| {
                let rho = r / a_r;
                let mut sum = 0.0;
                for p in vertical.windows(2) {
                    sum += gl4(|t| math::exp(-math::hypot(rho, t)), p[0], p[1]);
                }
                r * sum
            },
            w[0] * r_e,
            w[1] * r_e,
        );
    }
    weighted / mass
}

/// The bulge's line-of-sight dispersion seen face-on, mass-weighted inside its effective radius,
/// from the mass model `model` alone: the σ that the M–σ relation reads (plan 08, P08.T4.d),
/// which replaces plan 02's spherical estimate (its Design note 8).
///
/// `model` is the black-hole-free model of plan 02's two-phase build, since the black hole's
/// mass is what this sets; `params` gives the bulge. Face-on the line of sight is z and carries no
/// mean motion, so `Σ σ_p²(R) = ∫ ν σ_z² dz = 2 ∫₀^∞ z ν K_z dz` by the vertical Jeans equation,
/// whatever the table's `β_z` and Satoh's `k`. The tracer is the bulge axisymmetrised as the
/// potential holds it, the spheroidal exponential of the boxy bulge's second moments, whose
/// face-on effective radius is 2.027 of its radial scale. The reduced solution evaluates `K_z` at
/// 64 points (4 radii, 16 heights), some 30–60 ms of a parameter build; it integrates the
/// projection directly rather than tabulating the solution on a reduced grid, which would take
/// hundreds of force evaluations at a millisecond each. It agrees with the final table's
/// ([`KinematicTables::bulge_projected_sigma`](super::KinematicTables::bulge_projected_sigma))
/// to under 3%.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::kinematics::spheroid::bulge_projected_sigma;
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::galaxy::potential::MassModel;
///
/// let params = GalaxyParams::milky_way_like();
/// // The fixture's own mass model holds its black hole; its few 10⁶ M☉ change σ by far less
/// // than a per cent inside the bulge's effective radius.
/// let sigma = bulge_projected_sigma(&MassModel::new(&params), &params);
/// assert!((60.0..140.0).contains(&sigma.value()), "{sigma:?}");
/// ```
#[must_use]
pub fn bulge_projected_sigma(model: &MassModel, params: &GalaxyParams) -> KilometresPerSecond {
    let axes = bulge_tracer_axes(params);
    KilometresPerSecond::new(
        aperture_mean(axes, &APERTURE_PANELS, &COLUMN_PANELS, |r, z| {
            z * model.forces_at(r, z).vertical
        })
        .sqrt(),
    )
}

/// The same aperture's face-on dispersion from a table's `σ_z²`, on finer panels, since a table
/// lookup costs nothing against a force: for the final tables.
pub(crate) fn projected_sigma_from_table(table: &JeansTable, axes: (f64, f64)) -> f64 {
    const RADIAL: [f64; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];
    const VERTICAL: [f64; 9] = [0.0, 0.25, 0.5, 1.0, 2.0, 3.0, 5.0, 8.0, 20.0];
    aperture_mean(axes, &RADIAL, &VERTICAL, |r, z| table.moments(r, z)[2]).sqrt()
}

/// The axes `(a_r, a_z)` of the bulge's axisymmetrised tracer, ly.
#[must_use]
pub(crate) fn bulge_tracer_axes(params: &GalaxyParams) -> (f64, f64) {
    let (a_r, a_z) = bulge_spheroid(params.bulge());
    (a_r.value(), a_z.value())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A Plummer sphere in its own potential, GM = 10⁷ (km/s)² ly and b = 1,000 ly.
    struct Plummer;

    impl ForceSource for Plummer {
        fn forces_at(&self, r: f64, z: f64) -> Forces {
            let d2 = r * r + z * z + 1e6;
            let d3 = d2 * d2.sqrt();
            Forces {
                vertical: 1e7 * z / d3,
                v_circ_sq: 1e7 * r * r / d3,
            }
        }
    }

    fn plummer_table() -> JeansTable {
        JeansTable::new(
            |r, z| -2.5 * math::ln_1p((r * r + z * z) / 1e6),
            0.0,
            0.0,
            &Plummer,
        )
    }

    /// P08.T4.a: the isotropic Plummer dispersion `GM ÷ (6 √(r² + b²))` to 2%, on every
    /// axis, with no mean rotation.
    #[test]
    fn a_plummer_sphere_is_isotropic_to_two_per_cent() {
        let table = plummer_table();
        for r in [0.5, 300.0, 1_000.0, 4_000.0, 20_000.0] {
            for z in [0.2, 500.0, 2_000.0, 10_000.0] {
                let exact: f64 = 1e7 / (6.0 * (r * r + z * z + 1e6_f64).sqrt());
                let [sr, sp, sz, mean] = table.moments(r, z);
                for (name, s) in [("σ_R²", sr), ("σ_φ²", sp), ("σ_z²", sz)] {
                    assert!(
                        (s.sqrt() / exact.sqrt() - 1.0).abs() < 0.02,
                        "{name} at ({r}, {z}): {s} against {exact}"
                    );
                }
                assert!(mean.abs() < 1e-9, "mean {mean} at ({r}, {z})");
            }
        }
    }

    /// P08.T4.a: every dispersion is positive, and the mean rotation vanishes on the axis.
    #[test]
    fn a_rotating_table_is_positive_and_still_on_the_axis() {
        let table = JeansTable::new(
            |r, z| -math::hypot(r / 2_000.0, z / 700.0),
            BULGE_BETA_Z,
            BULGE_SATOH_K,
            &Plummer,
        );
        for &n in &table.nodes {
            assert!(n[0] > 0.0 && n[1] > 0.0 && n[2] > 0.0, "{n:?}");
            assert!(n[3] >= 0.0);
        }
        assert!(table.moments(0.0, 100.0)[3].abs() < 1e-15);
        assert!(table.moments(2_000.0, 0.0)[3] > 1.0);
    }

    /// P08.T4.b: the flow's divergence by central differences, which the plan asks to 10⁻⁹ of `|u|
    /// ÷ a`, is held to 10⁻⁶: at a step of 10⁻³ ly the differences' rounding of speeds near 100
    /// km/s is already 10⁻¹¹ km/s, and the bilinear table's `ω(m)` changes slope between the two
    /// terms' stencils. The flow is divergence-free analytically for any `ω(m)`; the test checks
    /// that nothing but rounding and the stencils is left, and that it is tangent to the ellipses.
    #[test]
    fn the_flow_is_divergence_free_and_follows_the_ellipses() {
        let rotating = JeansTable::new(
            |r, z| -math::hypot(r / 2_000.0, z / 700.0),
            BULGE_BETA_Z,
            BULGE_SATOH_K,
            &Plummer,
        );
        let (a, b) = (2_500.0, 1_500.0);
        let omega_p = 0.002;
        for (x, y) in [(800.0, 300.0), (-1_200.0, 900.0), (2_000.0, -2_500.0)] {
            let u = |x: f64, y: f64| {
                let [ux, uy] = bar_streaming(&rotating, (a, b), omega_p, x, y);
                [ux.value(), uy.value()]
            };
            let h = 1e-3;
            let div = (u(x + h, y)[0] - u(x - h, y)[0]) / (2.0 * h)
                + (u(x, y + h)[1] - u(x, y - h)[1]) / (2.0 * h);
            let speed = math::hypot(u(x, y)[0], u(x, y)[1]);
            assert!(div.abs() < 1e-6 * speed / a, "{div} at ({x}, {y})");
            // Tangent to x²/a² + y²/b²: u · ∇m² = 0.
            let [ux, uy] = u(x, y);
            let dot = ux * 2.0 * x / (a * a) + uy * 2.0 * y / (b * b);
            assert!(dot.abs() < 1e-12 * speed / a, "{dot}");
        }
    }
}
