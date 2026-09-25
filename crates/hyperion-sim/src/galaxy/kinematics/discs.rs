//! The discs' velocity laws: the young disc, the old thin disc's sub-discs and the thick disc (plan
//! 08, P08.T2 and Design notes 3–5 and 8).
//!
//! - **Vertical.** On the component's own vertical profile `n(z)`, which is plan 02's cored Jeans
//!   profile, the vertical Jeans equation gives `σ_z²(R, z) = (1 ÷ n(z)) ∫_|z|^∞ n(z′) K_z(R, z′)
//!   dz′`, tabulated on 64 radii × 24 heights (heights 0 to 8 effective heights). It is the
//!   equation plan 02 solves at its reference radius to build the profile, so there the table
//!   returns the profile's own dispersion: age, height and vertical speed agree by construction.
//!   Away from it `K_z` changes and so does `σ_z`, which falls outward with the disc.
//! - **Radial.** `σ_R = σ_z ÷ r`, with `r` the component's ratio `σ_z ÷ σ_R` ([`RadialRatio`]):
//!   0.5 for the young disc and 0.54 for the thick disc (Design note 4), and for each sub-disc the
//!   ratio of Sharma et al.'s (2021) vertical and radial laws at its age and height, since their
//!   exponents do not give the plan's 0.5–0.6 (plan 08, Risks; provisional).
//! - **Azimuthal.** `σ_φ² = σ_R² κ² ÷ 4Ω²`, the epicyclic ratio.
//! - **Mean rotation.** `v̄_φ = v_c − v_a`, with the asymmetric drift in full (Design note 5,
//!   [`asymmetric_drift`]), floored at 0.2 `v_c` where the expansion fails in the inner disc.
//! - **The young disc** floors every dispersion at
//!   [`YOUNG_DISC_SIGMA_FLOOR`](super::YOUNG_DISC_SIGMA_FLOOR) and streams along its arms
//!   ([`ArmStreaming`], Design note 8).
//!
//! Speeds are km/s, lengths light-years, in the local cylindrical axes (rimward R, spinward φ,
//! north z).

use super::gl8;
use super::spheroid::ForceSource;
use crate::galaxy::PointLy;
use crate::galaxy::consts::{LIGHT_YEARS_PER_KILOPARSEC, YEARS_PER_GIGAYEAR};
use crate::galaxy::fields::arms::ArmGeometry;
use crate::galaxy::fields::disc::ExponentialDisc;
use crate::galaxy::potential::PotentialTables;
use crate::math;
use crate::units::{KilometresPerSecond, LightYears, Years};

/// Radii per disc table: `R_i = L × 2^(i ÷ 8 − 4)`, `L` the component's scale length, from
/// `L ÷ 16` to 14.7 `L`.
pub const DISC_RADII: usize = 64;

/// Heights per disc table: `z_j = j × 8h ÷ 23`, `h` the component's effective height (Design
/// note 3).
pub const DISC_HEIGHTS: usize = 24;

/// The table's top, in effective heights.
const TOP_HEIGHTS: f64 = 8.0;

/// Radial nodes per octave.
const PER_OCTAVE: f64 = 8.0;

/// The first radius, in octaves below the scale length.
const FIRST_OCTAVE: f64 = -4.0;

/// Where the vertical integral's tail is split beyond the table's top, in units of the top: the
/// profile has fallen by at least `e⁻⁸` there, and past 64 effective heights by far more than any
/// term the sum keeps.
const TAIL_EDGES: [f64; 7] = [1.0, 1.5, 2.0, 3.0, 4.0, 6.0, 8.0];

/// The mean rotation's floor in units of the circular speed: where the asymmetric drift's
/// expansion fails in the inner disc (Design note 5).
pub const MEAN_ROTATION_FLOOR: f64 = 0.2;

/// `σ_z ÷ σ_R` of the young disc (plan 08, Design note 4).
pub const YOUNG_DISC_RATIO: f64 = 0.5;

/// `σ_z ÷ σ_R` of the thick disc (plan 08, Design note 4). It is also Sharma et al.'s (2021)
/// ratio at 10 Gyr, 21.1 ÷ 39.4 = 0.536, which the thick disc's ages bracket.
pub const THICK_DISC_RATIO: f64 = 0.54;

/// Sharma et al.'s radial law at 10 Gyr in the mid-plane, `σ_0,R`, km/s (2021, MNRAS 506, 1761,
/// Table 2: 39.4 ± 0.3), at the Sun's angular momentum and solar metallicity.
pub const SIGMA_R_AT_TEN_GYR: f64 = 39.4;

/// Sharma et al.'s vertical law at 10 Gyr in the mid-plane, `σ_0,z`, km/s (2021, Table 2: 21.1 ±
/// 0.2), the law plan 02's sub-discs carry.
pub const SIGMA_Z_AT_TEN_GYR: f64 = 21.1;

/// The radial law's age exponent `β_R` (Sharma et al. 2021, Table 2: 0.251 ± 0.006), against the
/// vertical law's 0.441.
pub const RADIAL_HEATING_EXPONENT: f64 = 0.251;

/// The vertical law's age exponent `β_z` (Sharma et al. 2021, Table 2: 0.441 ± 0.007).
pub const VERTICAL_HEATING_EXPONENT: f64 = 0.441;

/// The laws' age offset, Gyr (Sharma et al. 2021, eq. 4): `((τ + 0.1) ÷ 10.1)^β`.
pub const HEATING_AGE_OFFSET_GYR: f64 = 0.1;

/// How the radial dispersion grows with height, per kpc: `σ_R ∝ 1 + 0.12 |z|` (Sharma et al. 2021,
/// eq. 7 and Table 2: 0.12 ± 0.01), capped where the vertical law's rise is capped (plan 02,
/// ruling 4 of 2026-09-22).
pub const RADIAL_HEIGHT_GRADIENT_PER_KPC: f64 = 0.12;

/// How a component's radial dispersion follows from its vertical one: `σ_R = σ_z × factor(z)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RadialRatio {
    /// A fixed ratio `σ_z ÷ σ_R` at every height.
    Constant(f64),
    /// Sharma et al.'s (2021) two laws at the sub-disc's mean age: `σ_R ÷ σ_z = (39.4 ÷ 21.1)
    /// ((τ + 0.1) ÷ 10.1)^(0.251 − 0.441) × (1 + 0.12 |z|) ÷ (1 + γ_z |z|)`, with `γ_z` and its cap
    /// the profile's own (plan 08, Risks: the plan's 0.5–0.6 fails against their exponents, which
    /// give 0.31–0.52 across the sub-discs; provisional until the owner rules).
    Sharma {
        /// The sub-disc's mean age.
        age: Years,
        /// `γ_z`, per ly, of the vertical law the profile carries.
        vertical_gradient: f64,
        /// The height above which both rises stop, ly.
        reach: f64,
    },
}

impl RadialRatio {
    /// `σ_R ÷ σ_z` at height `z` (ly).
    #[must_use]
    pub fn factor(&self, z: f64) -> f64 {
        match *self {
            Self::Constant(ratio) => 1.0 / ratio,
            Self::Sharma {
                age,
                vertical_gradient,
                reach,
            } => {
                let gyr = age.value() / YEARS_PER_GIGAYEAR;
                let f = (gyr + HEATING_AGE_OFFSET_GYR) / (10.0 + HEATING_AGE_OFFSET_GYR);
                let height = z.abs().min(reach);
                let radial_gradient = RADIAL_HEIGHT_GRADIENT_PER_KPC / LIGHT_YEARS_PER_KILOPARSEC;
                SIGMA_R_AT_TEN_GYR / SIGMA_Z_AT_TEN_GYR
                    * math::powf(f, RADIAL_HEATING_EXPONENT - VERTICAL_HEATING_EXPONENT)
                    * (1.0 + radial_gradient * height)
                    / (1.0 + vertical_gradient * height)
            }
        }
    }

    /// `σ_z ÷ σ_R` in the mid-plane, the ratio Design note 4 names.
    #[must_use]
    pub fn midplane_ratio(&self) -> f64 {
        1.0 / self.factor(0.0)
    }
}

/// The asymmetric drift `v_a = σ_R² ÷ (2 v_c) × [σ_φ² ÷ σ_R² − 1 + R ÷ R_ν + 2R ÷ R_σ]`, km/s
/// (Binney and Tremaine 2008, the asymmetric-drift equation for a cylindrically aligned
/// ellipsoid, their eq. 4.227–4.228; plan 08, Design note 5).
///
/// `density_slope` is `R ÷ R_ν = −d ln ν ÷ d ln R` and `dispersion_slope` is `2R ÷ R_σ = −d ln
/// σ_R² ÷ d ln R`. At Milky Way values it is about `σ_R² ÷ 80 km/s` (Dehnen and Binney 1998,
/// MNRAS 298, 387, eq. 17: 80 ± 5 km/s).
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::kinematics::discs::asymmetric_drift;
/// use hyperion_sim::units::KilometresPerSecond;
///
/// // Near the Sun: σ_R 35 km/s, κ² ÷ 4Ω² 0.5, three scale lengths out, and σ_R² falling with
/// // the disc's own length, in a 230 km/s curve: close to Dehnen and Binney's σ_R² ÷ 80 km/s.
/// let sigma = KilometresPerSecond::new(35.0);
/// let v_a = asymmetric_drift(sigma, 0.5, 3.0, 3.0, KilometresPerSecond::new(230.0));
/// assert!((v_a.value() - 35.0 * 35.0 / 80.0).abs() < 1.0);
/// ```
#[must_use]
pub fn asymmetric_drift(
    sigma_r: KilometresPerSecond,
    phi_to_r_sq: f64,
    density_slope: f64,
    dispersion_slope: f64,
    v_circ: KilometresPerSecond,
) -> KilometresPerSecond {
    let s2 = sigma_r.value() * sigma_r.value();
    KilometresPerSecond::new(
        s2 / (2.0 * v_circ.value()) * (phi_to_r_sq - 1.0 + density_slope + dispersion_slope),
    )
}

/// `σ²(z_j) = (1 ÷ n(z_j)) ∫_{z_j}^∞ n K_z dz` at `heights` (ly, ascending from 0), for the
/// profile `n = exp(−exponent)` and the force `k_z` (km/s)² per ly: the vertical Jeans equation's
/// dispersion on a profile it did not shape.
///
/// Each panel between heights is an 8-point Gauss–Legendre rule, and the tail above the last height
/// runs on [`TAIL_EDGES`] of it. The sum is taken downwards, each step rescaled by `n(z_{j+1}) ÷
/// n(z_j)`, so that no ratio of two underflowed densities is ever formed.
pub(crate) fn vertical_dispersion_sq(
    exponent: impl Fn(f64) -> f64,
    mut k_z: impl FnMut(f64) -> f64,
    heights: &[f64],
) -> Vec<f64> {
    let top = *heights.last().expect("at least one height");
    let e_top = exponent(top);
    let mut tail = 0.0;
    for w in TAIL_EDGES.windows(2) {
        tail += gl8(
            |z| math::exp(e_top - exponent(z)) * k_z(z),
            w[0] * top,
            w[1] * top,
        );
    }
    let mut out = vec![0.0; heights.len()];
    let last = heights.len() - 1;
    out[last] = tail;
    let mut above = tail;
    for j in (0..last).rev() {
        let (lo, hi) = (heights[j], heights[j + 1]);
        let e_lo = exponent(lo);
        let panel = gl8(|z| math::exp(e_lo - exponent(z)) * k_z(z), lo, hi);
        above = panel + math::exp(e_lo - exponent(hi)) * above;
        out[j] = above;
    }
    out
}

/// The young disc's streaming along its arms (plan 08, Design note 8): `A × (cos ψ along the arm,
/// −½ sin ψ across it, outward positive)` with `ψ` the arm phase, zero on a ridge, times the arms'
/// fade-in, and reversed outside corotation.
///
/// "Along the arm" is the unit vector `(−sin p, cos p)` in (R, φ), which runs spinward and, since
/// the arms trail, inward; "across" is `(cos p, sin p)`. `A = 5 + 10 × (A_arm − 0.7) ÷ 0.2` km/s
/// with `A_arm` the young arm fraction in its range 0.7–0.9, so 5–15 km/s. The form and its phases
/// are the plan's and provisional: a linear density-wave derivation puts the inward radial motion
/// in phase with the ridge and the azimuthal part in quadrature (lane `kin08`'s research), which is
/// for the owner to rule on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArmStreaming {
    geometry: ArmGeometry,
    amplitude: f64,
    corotation: f64,
    sin_pitch: f64,
    cos_pitch: f64,
}

/// The young arm fraction's range, which the streaming amplitude spans (plan 02, P02.T5).
const ARM_FRACTION_RANGE: (f64, f64) = (0.7, 0.9);

/// The streaming amplitude's range, km/s (brainstorm, "Orbits and time": "streaming of 5–15 km/s
/// along the arms").
pub const STREAMING_RANGE: (f64, f64) = (5.0, 15.0);

/// The arms' pattern speed over the bar's, which sets where the streaming reverses: taken as 1
/// (plan 08, Design note 8).
pub const ARM_TO_BAR_PATTERN_RATIO: f64 = 1.0;

impl ArmStreaming {
    /// The streaming of arms on `geometry` with the young arm fraction `fraction`, reversing at
    /// `corotation` (ly).
    #[must_use]
    pub fn new(geometry: ArmGeometry, fraction: f64, corotation: LightYears) -> Self {
        let (lo, hi) = ARM_FRACTION_RANGE;
        let position = ((fraction - lo) / (hi - lo)).clamp(0.0, 1.0);
        let (sin_pitch, cos_pitch) = math::sin_cos(geometry.pitch().value());
        Self {
            geometry,
            amplitude: STREAMING_RANGE.0 + (STREAMING_RANGE.1 - STREAMING_RANGE.0) * position,
            corotation: corotation.value() * ARM_TO_BAR_PATTERN_RATIO,
            sin_pitch,
            cos_pitch,
        }
    }

    /// `A`, the streaming amplitude.
    #[must_use]
    pub fn amplitude(&self) -> KilometresPerSecond {
        KilometresPerSecond::new(self.amplitude)
    }

    /// The streaming velocity `(v_R, v_φ)` at the in-plane point `(x, y)` (ly), km/s; zero at the
    /// centre.
    #[must_use]
    pub fn at(&self, x: f64, y: f64) -> [f64; 2] {
        let Some(phase) = self.geometry.phase(x, y) else {
            return [0.0, 0.0];
        };
        let r = math::hypot(x, y);
        let (sin_phase, cos_phase) = math::sin_cos(phase);
        let reversal = if r > self.corotation { -1.0 } else { 1.0 };
        let amplitude = reversal * self.amplitude * self.geometry.fade(r);
        let (along, across) = (amplitude * cos_phase, -0.5 * amplitude * sin_phase);
        [
            -along * self.sin_pitch + across * self.cos_pitch,
            along * self.cos_pitch + across * self.sin_pitch,
        ]
    }
}

/// One disc component's velocity law, tabulated once per galaxy (module documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct DiscKinematics {
    length: f64,
    height_step: f64,
    ratio: RadialRatio,
    /// Per node `(i, j)`, row-major in i: `σ_R²`, `σ_φ²`, `σ_z²` ((km/s)²) and `v̄_φ` (km/s).
    nodes: Box<[[f64; 4]]>,
    floor: Option<f64>,
    streaming: Option<ArmStreaming>,
}

/// What a disc's velocity law is: its ratio, and the young disc's floor and streaming.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiscLaw {
    /// How `σ_R` follows from `σ_z`.
    pub ratio: RadialRatio,
    /// The floor on every dispersion, km/s, for the young disc.
    pub floor: Option<KilometresPerSecond>,
    /// The young disc's arm streaming.
    pub streaming: Option<ArmStreaming>,
}

impl DiscKinematics {
    /// The law of `disc` in the potential `potential`, which must hold the (R, z) grid.
    ///
    /// # Panics
    ///
    /// If `potential` has no (R, z) grid ([`PotentialTables::full`]).
    #[must_use]
    pub fn new(disc: &ExponentialDisc, law: DiscLaw, potential: &PotentialTables) -> Self {
        let length = disc.length().value();
        let profile = disc.profile();
        let height_step =
            TOP_HEIGHTS * profile.effective_height().value() / f64::from(disc_heights_minus_one());
        let heights: Vec<f64> = (0..DISC_HEIGHTS)
            .map(|j| height_step * index_f64(j))
            .collect();
        let floor_sq = law.floor.map(|f| f.value() * f.value());
        let mut sigma_z_sq = vec![[0.0; DISC_HEIGHTS]; DISC_RADII];
        for (i, row) in sigma_z_sq.iter_mut().enumerate() {
            let r = radius(length, i);
            let column = vertical_dispersion_sq(
                |z| profile.exponent(z),
                |z| potential.forces_at(r, z).vertical,
                &heights,
            );
            row.copy_from_slice(&column);
        }
        let inv_length = 1.0 / length;
        let hole = disc.hole_ly();
        let mut nodes = Vec::with_capacity(DISC_RADII * DISC_HEIGHTS);
        for i in 0..DISC_RADII {
            let r = radius(length, i);
            let rl = LightYears::new(r);
            let v_c = potential.v_circ(rl).value();
            let epicyclic = potential.kappa(rl).value() / potential.omega(rl).value();
            let phi_to_r = 0.25 * epicyclic * epicyclic;
            // −d ln ν ÷ d ln R of exp(−R ÷ L − R_h ÷ R).
            let density_slope = r * inv_length - if hole > 0.0 { hole / r } else { 0.0 };
            for (j, &z) in heights.iter().enumerate() {
                let factor = law.ratio.factor(z);
                let sigma_r_sq_at = |k: usize| {
                    let s = sigma_z_sq[k][j] * factor * factor;
                    floor_sq.map_or(s, |f| s.max(f))
                };
                let radial_sq = sigma_r_sq_at(i);
                let (lo, hi) = (i.saturating_sub(1), (i + 1).min(DISC_RADII - 1));
                let span = f64::from(u8::try_from(hi - lo).expect("at most 2"))
                    * core::f64::consts::LN_2
                    / PER_OCTAVE;
                let dispersion_slope =
                    -(math::ln(sigma_r_sq_at(hi)) - math::ln(sigma_r_sq_at(lo))) / span;
                let mut sigma_phi_sq = radial_sq * phi_to_r;
                let mut sigma_z = sigma_z_sq[i][j];
                if let Some(f) = floor_sq {
                    sigma_phi_sq = sigma_phi_sq.max(f);
                    sigma_z = sigma_z.max(f);
                }
                let drift = asymmetric_drift(
                    KilometresPerSecond::new(radial_sq.sqrt()),
                    sigma_phi_sq / radial_sq,
                    density_slope,
                    dispersion_slope,
                    KilometresPerSecond::new(v_c),
                )
                .value();
                let mean = (v_c - drift).max(MEAN_ROTATION_FLOOR * v_c);
                nodes.push([radial_sq, sigma_phi_sq, sigma_z, mean]);
            }
        }
        Self {
            length,
            height_step,
            ratio: law.ratio,
            nodes: nodes.into_boxed_slice(),
            floor: law.floor.map(KilometresPerSecond::value),
            streaming: law.streaming,
        }
    }

    /// How `σ_R` follows from `σ_z` for this component.
    #[must_use]
    pub fn ratio(&self) -> RadialRatio {
        self.ratio
    }

    /// The young disc's arm streaming, if this is the young disc.
    #[must_use]
    pub fn streaming(&self) -> Option<&ArmStreaming> {
        self.streaming.as_ref()
    }

    /// The floor on every dispersion, if any.
    #[must_use]
    pub fn floor(&self) -> Option<KilometresPerSecond> {
        self.floor.map(KilometresPerSecond::new)
    }

    /// `[σ_R², σ_φ², σ_z², v̄_φ]` at cylindrical radius `r` and height `z` (ly), without the arm
    /// streaming: bilinear in `ln R` and `z` between the table's nodes, level beyond its radii and
    /// above its top, and with the mean rotation falling linearly to zero inside its first radius.
    #[must_use]
    pub(crate) fn moments(&self, r: f64, z: f64) -> [f64; 4] {
        let first = radius(self.length, 0);
        let position = if r > first {
            (math::log2(r / self.length) - FIRST_OCTAVE) * PER_OCTAVE
        } else {
            0.0
        };
        let radial = split(position, DISC_RADII);
        let vertical = split(z.abs() / self.height_step, DISC_HEIGHTS);
        let mut out = bilinear(&self.nodes, DISC_HEIGHTS, radial, vertical);
        if r < first {
            out[3] *= r / first;
        }
        out
    }

    /// The mean velocity `[v_R, v_φ, v_z]` and the dispersions `[σ_R, σ_φ, σ_z]` at `p`, km/s,
    /// with the young disc's streaming.
    #[must_use]
    pub fn at(&self, p: &PointLy) -> ([f64; 3], [f64; 3]) {
        let r = math::hypot(p.x, p.y);
        let [sr, sp, sz, mean] = self.moments(r, p.z);
        let [stream_r, stream_phi] = self.streaming.map_or([0.0, 0.0], |s| s.at(p.x, p.y));
        // The nodes are floored already; flooring again after the interpolation keeps the last
        // bit of a floored value from rounding under it.
        let floor = self.floor.unwrap_or(0.0);
        (
            [stream_r, mean + stream_phi, 0.0],
            [sr, sp, sz].map(|s2| s2.sqrt().max(floor)),
        )
    }

    /// The bytes the table owns on the heap.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        size_of_val(&*self.nodes)
    }
}

/// 23, the last height index, as a float's source.
fn disc_heights_minus_one() -> u8 {
    u8::try_from(DISC_HEIGHTS - 1).expect("24 heights")
}

/// A small index as a float.
fn index_f64(i: usize) -> f64 {
    f64::from(u16::try_from(i).expect("a table index"))
}

/// The table radius `i`, ly.
fn radius(length: f64, i: usize) -> f64 {
    length * math::exp2(index_f64(i) / PER_OCTAVE + FIRST_OCTAVE)
}

/// `nodes` (row-major, `columns` per row) at the cells and offsets `(row, across)` and `(column,
/// up)`, bilinear.
pub(crate) fn bilinear(
    nodes: &[[f64; 4]],
    columns: usize,
    (row, across): (usize, f64),
    (column, up): (usize, f64),
) -> [f64; 4] {
    let at = |a: usize, b: usize| nodes[a * columns + b];
    let mut out = [0.0; 4];
    for (k, value) in out.iter_mut().enumerate() {
        let low = at(row, column)[k] * (1.0 - up) + at(row, column + 1)[k] * up;
        let high = at(row + 1, column)[k] * (1.0 - up) + at(row + 1, column + 1)[k] * up;
        *value = low * (1.0 - across) + high * across;
    }
    out
}

/// The cell of a table of `n` nodes holding the position `u` (in nodes), and the offset in it,
/// clamped to the table.
pub(crate) fn split(u: f64, n: usize) -> (usize, f64) {
    let last = index_f64(n - 1);
    let u = u.clamp(0.0, last);
    let floor = u.floor().min(last - 1.0);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a whole number clamped to 0 – n − 2"
    )]
    let i = floor as usize;
    (i, u - floor)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An isothermal sheet `n ∝ sech²(z ÷ z₀)` in its own force `K_z = A tanh(z ÷ z₀)` has
    /// `σ² = A z₀ ÷ 2` at every height (Spitzer 1942).
    #[test]
    fn the_vertical_integral_returns_an_isothermal_sheet_s_dispersion() {
        let (z0, a) = (300.0, 0.05);
        let exponent = |z: f64| 2.0 * math::ln(math::cosh(z / z0));
        let heights: Vec<f64> = (0..24).map(|j| f64::from(j) * 8.0 * z0 / 23.0).collect();
        let sigma_sq = vertical_dispersion_sq(exponent, |z| a * math::tanh(z / z0), &heights);
        let exact = a * z0 / 2.0;
        for (z, s) in heights.iter().zip(&sigma_sq) {
            assert!(
                (s.sqrt() / exact.sqrt() - 1.0).abs() < 5e-3,
                "{z} ly: {s} against {exact}"
            );
        }
    }

    #[test]
    fn sharma_s_ratio_rises_with_age_and_falls_with_height() {
        let ratio = |gyr: f64| RadialRatio::Sharma {
            age: Years::new(gyr * 1e9),
            vertical_gradient: 0.2 / LIGHT_YEARS_PER_KILOPARSEC,
            reach: 2.0 * LIGHT_YEARS_PER_KILOPARSEC,
        };
        // The research's figures: 0.313 at 0.5 Gyr, 0.519 at 8.5 Gyr, 0.536 at 10.
        assert!((ratio(0.5).midplane_ratio() - 0.313).abs() < 2e-3);
        assert!((ratio(8.5).midplane_ratio() - 0.519).abs() < 2e-3);
        assert!((ratio(10.0).midplane_ratio() - 21.1 / 39.4).abs() < 1e-12);
        let r = ratio(5.0);
        assert!(r.factor(3_000.0) < r.factor(0.0));
        assert!((r.factor(20_000.0) - r.factor(2.0 * LIGHT_YEARS_PER_KILOPARSEC)).abs() < 1e-15);
        assert!((RadialRatio::Constant(0.5).factor(123.0) - 2.0).abs() < 1e-15);
    }

    #[test]
    fn the_split_clamps_to_the_table() {
        assert_eq!(split(-3.0, 64), (0, 0.0));
        assert_eq!(split(63.0, 64), (62, 1.0));
        let (i, t) = split(10.25, 64);
        assert_eq!(i, 10);
        assert!((t - 0.25).abs() < 1e-15);
    }
}
