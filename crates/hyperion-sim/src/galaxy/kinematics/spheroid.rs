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
use crate::tables::gauss_legendre::{GL4_NODES, GL4_WEIGHTS};
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

/// The nuclear disc's central radial dispersion `σ_r,0`, km/s (Sormani et al. 2022, MNRAS 512,
/// 1857, the quasi-isothermal fit's posterior: 67.7 +4.5 −3.5; ruling 105.2).
pub const NUCLEAR_SIGMA_R0: f64 = 67.7;

/// The scale `R_σ,r` over which the nuclear disc's radial dispersion falls, `e^(−R ÷ R_σ)`: 10^3.7 pc
/// (Sormani et al. 2022, `log₁₀ R_σ,r [pc] = 3.7 +0.6 −0.4`; ruling 105.2), in light-years.
pub const NUCLEAR_SIGMA_SCALE_LY: f64 = 5_011.872_336_272_722 * 3.261_563_777_167_433_6;

/// The nuclear disc's radial law: `σ_R² = max(σ_z², σ_r,0² e^(−2R ÷ R_σ,r))` (ruling 105.2), with
/// `σ_z` the vertical Jeans integral's on its own profile. It replaces Design note 10's `β_z` of 0,
/// which gave `σ_R = σ_z` and contradicted plan 02's ruling 5 (the vertical dispersion about half
/// the radial).
pub const NUCLEAR_RADIAL_LAW: RadialLaw = RadialLaw::Floored {
    sigma0: NUCLEAR_SIGMA_R0,
    scale: NUCLEAR_SIGMA_SCALE_LY,
};

/// Satoh's `k` for the bulge (plan 08, Design note 10).
pub const BULGE_SATOH_K: f64 = 0.6;

/// Satoh's `k` for the long bar (plan 08, Design note 10).
pub const BAR_SATOH_K: f64 = 0.8;

/// Satoh's `k` for the nuclear disc (plan 08, Design note 10; re-tuned from 0.9 under ruling
/// 105.2's hotter radial law, against the 80–120 km/s rotation at 300–500 ly).
pub const NUCLEAR_SATOH_K: f64 = 0.95;

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

/// How a Jeans table's radial dispersion follows from its vertical one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RadialLaw {
    /// A constant `β_z = 1 − σ_z² ÷ σ_R²`: `σ_R² = σ_z² ÷ (1 − β_z)`.
    Anisotropic {
        /// `β_z`, below 1.
        beta_z: f64,
    },
    /// `σ_R² = max(σ_z², σ₀² e^(−2R ÷ R_σ))`, a radial dispersion of its own that the vertical
    /// one may only raise: the nuclear disc's (ruling 105.2, from Sormani et al. 2022, eq. 7).
    Floored {
        /// `σ₀`, km/s.
        sigma0: f64,
        /// `R_σ`, ly.
        scale: f64,
    },
}

impl RadialLaw {
    /// `σ_R²` at cylindrical radius `r` (ly) for the vertical dispersion squared `sigma_z_sq`,
    /// (km/s)².
    #[must_use]
    pub fn radial_sq(&self, r: f64, sigma_z_sq: f64) -> f64 {
        match *self {
            Self::Anisotropic { beta_z } => sigma_z_sq / (1.0 - beta_z),
            Self::Floored { sigma0, scale } => {
                sigma_z_sq.max(sigma0 * sigma0 * math::exp(-2.0 * r / scale))
            }
        }
    }
}

/// An axisymmetric Jeans solution, tabulated on the potential's grid (module documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct JeansTable {
    radial: RadialLaw,
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
        Self::with_radial_law(
            ln_density,
            RadialLaw::Anisotropic { beta_z },
            satoh_k,
            potential,
        )
    }

    /// The solution as [`new`](Self::new) builds it, with the radial dispersion from `radial`
    /// rather than a constant `β_z`: the nuclear disc's floored law (ruling 105.2).
    ///
    /// # Panics
    ///
    /// Never: the grid's indices are fixed and in range.
    #[must_use]
    pub fn with_radial_law(
        ln_density: impl Fn(f64, f64) -> f64,
        radial: RadialLaw,
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
        let radial_sq_at = |i: usize, j: usize| radial.radial_sq(radii[i], sigma_z_sq[i][j]);
        let mut nodes = Vec::with_capacity(JEANS_POINTS * JEANS_POINTS);
        for (i, &r) in radii.iter().enumerate() {
            let (lo, hi) = (i.saturating_sub(1), (i + 1).min(JEANS_POINTS - 1));
            let span = f64::from(u8::try_from(hi - lo).expect("at most 2")) * STEP;
            for (j, &z) in heights.iter().enumerate() {
                let radial_sq = radial_sq_at(i, j);
                let d_sigma = (radial_sq_at(hi, j) - radial_sq_at(lo, j)) / span;
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
            radial,
            satoh_k,
            nodes: nodes.into_boxed_slice(),
        }
    }

    /// How the radial dispersion follows from the vertical.
    #[must_use]
    pub fn radial_law(&self) -> RadialLaw {
        self.radial
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

/// The radii of the reduced solution's midplane profile, in units of the tracer's radial scale
/// `a_r` (P08.T4.d, ruling 105.1).
const PROFILE_RADII: [f64; 9] = [0.03, 0.1, 0.25, 0.5, 0.9, 1.5, 2.5, 4.0, 7.0];

/// The reduced solution's vertical panels, in units of the tracer's vertical scale `a_z`, each a
/// 4-point rule.
const PROFILE_COLUMN: [f64; 4] = [0.0, 1.0, 3.0, 10.0];

/// The aperture's panels along the major axis, in units of the effective radius, each a 4-point
/// rule.
const SLIT_PANELS: [f64; 4] = [0.0, 0.25, 0.5, 1.0];

/// The panels of a line of sight edge-on, in units of `a_r`, and of a column face-on, in units of
/// `a_z`, each a 4-point rule.
const SIGHT_PANELS: [f64; 6] = [0.0, 0.25, 1.0, 3.0, 8.0, 20.0];

/// The inclination average of M–σ's `σ_e²`: `⟨cos² i⟩ = 1 ÷ 3` of the face-on second moment and `2
/// ÷ 3` of the edge-on one, for a galaxy seen from a random direction (ruling 105.1).
const FACE_ON_WEIGHT: f64 = 1.0 / 3.0;

/// The luminosity-weighted line-of-sight second moment `⟨V² + σ²⟩` of the spheroidal exponential
/// `ν = exp(−√(R² ÷ a_r² + z² ÷ a_z²))` along its major axis inside `R_e = 2.027 a_r`, averaged
/// over inclination: the `σ_e` of the M–σ relation, squared (McConnell and Ma 2013, ApJ 764, 184,
/// eq. 1; Gültekin et al. 2009; the Nuker practice of `I(r) dr` weighting, Kormendy and Ho 2013),
/// as ruling 105.1 has it.
///
/// `column(R)` is `∫₀^∞ ν σ_z² dz` at radius `R` with `ν(R, 0) = e^(−R ÷ a_r)`, the face-on column
/// of the second moment, which carries no mean motion; `midplane(R)` is `(σ_R², ⟨v_φ²⟩)` in the
/// plane, whose line-of-sight mix is the edge-on moment `σ_R² sin²φ + ⟨v_φ²⟩ cos²φ` at azimuth φ
/// from the line of sight's normal; the edge-on slit is taken in the plane.
fn sigma_e_sq(
    (a_r, a_z): (f64, f64),
    mut column: impl FnMut(f64) -> f64,
    mut midplane: impl FnMut(f64) -> (f64, f64),
) -> f64 {
    let r_e = EFFECTIVE_RADIUS_IN_SCALES * a_r;
    let panels = |f: &mut dyn FnMut(f64) -> f64, edges: &[f64], scale: f64| {
        edges
            .windows(2)
            .fold(0.0, |sum, w| sum + gl4(&mut *f, w[0] * scale, w[1] * scale))
    };
    // Face-on: I(R) dR along the major axis.
    let face_moment = panels(&mut |r| column(r), &SLIT_PANELS, r_e);
    let face_light = panels(
        &mut |r| {
            let rho = r / a_r;
            a_z * panels(&mut |t| math::exp(-math::hypot(rho, t)), &SIGHT_PANELS, 1.0)
        },
        &SLIT_PANELS,
        r_e,
    );
    // Edge-on: along the major axis X in the plane, each line of sight y through it.
    let (mut edge_moment, mut edge_light) = (0.0, 0.0);
    for w in SLIT_PANELS.windows(2) {
        edge_moment += gl4(
            |x| {
                panels(
                    &mut |y| {
                        let r = math::hypot(x, y);
                        let nu = math::exp(-r / a_r);
                        let (radial_sq, azimuthal_sq) = midplane(r);
                        if r > 0.0 {
                            nu * (radial_sq * (y / r) * (y / r) + azimuthal_sq * (x / r) * (x / r))
                        } else {
                            nu * radial_sq
                        }
                    },
                    &SIGHT_PANELS,
                    a_r,
                )
            },
            w[0] * r_e,
            w[1] * r_e,
        );
        edge_light += gl4(
            |x| {
                panels(
                    &mut |y| math::exp(-math::hypot(x, y) / a_r),
                    &SIGHT_PANELS,
                    a_r,
                )
            },
            w[0] * r_e,
            w[1] * r_e,
        );
    }
    FACE_ON_WEIGHT * face_moment / face_light + (1.0 - FACE_ON_WEIGHT) * edge_moment / edge_light
}

/// The reduced solution's midplane profile: at each of [`PROFILE_RADII`], `ln ∫₀^∞ z ν K_z dz`
/// (the face-on column of `ν σ_z²`), `ln ∫₀^∞ ν K_z dz` (`ν σ_z²` in the plane) and `v_c²`, read
/// linearly in `ln R` between them and level outside.
struct ReducedProfile {
    ln_r: [f64; PROFILE_RADII.len()],
    ln_column: [f64; PROFILE_RADII.len()],
    ln_midplane: [f64; PROFILE_RADII.len()],
    v_circ_sq: [f64; PROFILE_RADII.len()],
}

impl ReducedProfile {
    fn new(model: &MassModel, (a_r, a_z): (f64, f64)) -> Self {
        let mut profile = Self {
            ln_r: [0.0; PROFILE_RADII.len()],
            ln_column: [0.0; PROFILE_RADII.len()],
            ln_midplane: [0.0; PROFILE_RADII.len()],
            v_circ_sq: [0.0; PROFILE_RADII.len()],
        };
        for (k, &rho) in PROFILE_RADII.iter().enumerate() {
            let r = rho * a_r;
            let (mut column, mut midplane) = (0.0, 0.0);
            // One force per node serves both integrals: the 4-point rule on each panel in t.
            for w in PROFILE_COLUMN.windows(2) {
                let half = 0.5 * (w[1] - w[0]);
                let mid = w[0] + half;
                for (&x, &weight) in GL4_NODES.iter().zip(&GL4_WEIGHTS) {
                    let t = mid + half * x;
                    let z = a_z * t;
                    let f = math::exp(-math::hypot(rho, t)) * model.forces_at(r, z).vertical;
                    column += weight * half * a_z * z * f;
                    midplane += weight * half * a_z * f;
                }
            }
            profile.ln_r[k] = math::ln(r);
            profile.ln_column[k] = math::ln(column);
            profile.ln_midplane[k] = math::ln(midplane);
            profile.v_circ_sq[k] = model.v_circ_sq(LightYears::new(r));
        }
        profile
    }

    /// The segment holding `ln r` and the offset in it, clamped.
    fn at(&self, r: f64) -> (usize, f64) {
        let last = self.ln_r.len() - 1;
        let u = if r > 0.0 { math::ln(r) } else { self.ln_r[0] };
        let k = self.ln_r[1..last].partition_point(|&x| x <= u);
        let t = ((u - self.ln_r[k]) / (self.ln_r[k + 1] - self.ln_r[k])).clamp(0.0, 1.0);
        (k, t)
    }

    fn lerp(values: &[f64], (k, t): (usize, f64)) -> f64 {
        values[k] * (1.0 - t) + values[k + 1] * t
    }

    /// `∫₀^∞ ν σ_z² dz` at `r`.
    fn column(&self, r: f64) -> f64 {
        math::exp(Self::lerp(&self.ln_column, self.at(r)))
    }

    /// `(σ_R², ⟨v_φ²⟩)` in the plane at `r`, for the anisotropy `beta_z`.
    fn midplane(&self, r: f64, rho: f64, beta_z: f64) -> (f64, f64) {
        let (k, t) = self.at(r);
        let ln_mid = Self::lerp(&self.ln_midplane, (k, t));
        let slope =
            (self.ln_midplane[k + 1] - self.ln_midplane[k]) / (self.ln_r[k + 1] - self.ln_r[k]);
        // ν σ_R² = ∫ ν K_z dz ÷ (1 − β_z), with ν(R, 0) = e^(−ρ).
        let radial_sq = math::exp(ln_mid + rho) / (1.0 - beta_z);
        let azimuthal_sq = radial_sq * (1.0 + slope) + Self::lerp(&self.v_circ_sq, (k, t));
        (radial_sq, azimuthal_sq.max(AZIMUTHAL_FLOOR * radial_sq))
    }
}

/// The bulge's `σ_e`, the dispersion the M–σ relation reads, from the mass model `model` alone (plan
/// 08, P08.T4.d, with ruling 105.1), which replaces plan 02's spherical estimate (its Design note
/// 8).
///
/// `σ_e` is the relation's own quantity: the line-of-sight second moment `V² + σ²`, weighted by
/// surface brightness along the major axis out to the effective radius (McConnell and Ma 2013, ApJ
/// 764, 184, eq. 1), averaged over inclination, a third face-on and two thirds edge-on (ruling
/// 105.1). Face-on the line of sight is z, `∫ ν σ_z² dz = ∫ z ν K_z dz` by the vertical Jeans
/// equation; edge-on it mixes `σ_R² = σ_z² ÷ (1 − β_z)` with `⟨v_φ²⟩ = σ_R² (1 + d ln(ν σ_R²) ÷ d ln
/// R) + v_c²`, which holds the rotation and the azimuthal dispersion together, whatever Satoh's
/// `k` splits them into. `model` is the black-hole-free model of plan 02's two-phase build, since
/// the black hole's mass is what this sets; `params` gives the bulge, axisymmetrised as the
/// potential holds it, whose effective radius is 2.027 of its radial scale. The reduced solution
/// evaluates `K_z` at 108 points (nine radii, twelve heights) and `v_c²` at nine, some 100 ms of a
/// parameter build; it agrees with the final table's
/// ([`KinematicTables::bulge_projected_sigma`](super::KinematicTables::bulge_projected_sigma)) to
/// under 3%.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::kinematics::spheroid::bulge_projected_sigma;
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::galaxy::potential::MassModel;
///
/// let params = GalaxyParams::milky_way_like();
/// let sigma = bulge_projected_sigma(&MassModel::without_centre(&params), &params);
/// // The Milky Way's `σ_e` is 103–105 ± 20 km/s (McConnell and Ma 2013; Kormendy and Ho 2013).
/// assert!((80.0..140.0).contains(&sigma.value()), "{sigma:?}");
/// ```
#[must_use]
pub fn bulge_projected_sigma(model: &MassModel, params: &GalaxyParams) -> KilometresPerSecond {
    let axes = bulge_tracer_axes(params);
    let profile = ReducedProfile::new(model, axes);
    KilometresPerSecond::new(
        sigma_e_sq(
            axes,
            |r| profile.column(r),
            |r| profile.midplane(r, r / axes.0, BULGE_BETA_Z),
        )
        .sqrt(),
    )
}

/// The same `σ_e` from a table, whose lookups cost nothing against a force: for the final tables.
pub(crate) fn projected_sigma_from_table(table: &JeansTable, (a_r, a_z): (f64, f64)) -> f64 {
    sigma_e_sq(
        (a_r, a_z),
        |r| {
            let rho = r / a_r;
            SIGHT_PANELS.windows(2).fold(0.0, |sum, w| {
                sum + gl4(
                    |t| math::exp(-math::hypot(rho, t)) * table.moments(r, a_z * t)[2] * a_z,
                    w[0],
                    w[1],
                )
            })
        },
        |r| {
            let [radial, azimuthal, _, mean] = table.moments(r, 0.0);
            (radial, azimuthal + mean * mean)
        },
    )
    .sqrt()
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
