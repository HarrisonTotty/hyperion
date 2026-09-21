//! The old thin disc's five sub-discs by age and their heights from the vertical Jeans equation
//! (brainstorm, "Orbits and time"; plan 02, Design note 9 and P02.T7.b).
//!
//! One scale height for the whole old thin disc would contradict the observed heating of stars
//! with age, so the old thin disc is five discs by age, as in the Besançon model, each with the
//! scale height at which the Jeans equation returns its dispersion. A sub-disc's age `τ` is the
//! mean age of the formation history inside its bin, and its dispersion follows the heating law of
//! Sharma et al. (2021, MNRAS 506, 1761, eqs. 3, 4 and 7, Table 2), which the brainstorm rounds to
//! `22 km/s × (τ ÷ 10 Gyr)^0.44`:
//!
//! `σ_z(τ, z) = 21.1 km/s × ((τ ÷ Gyr + 0.1) ÷ 10.1)^0.441 × (1 + 0.20 |z| ÷ kpc)`,
//!
//! at the Sun's angular momentum and metallicity, which the reference radius below stands for.
//! Over an exponential tracer of height `h` the density-weighted mean of `(1 + γ|z|)²` is
//! `1 + 2γh + 2γ²h²`, so the target is `⟨σ_z²⟩ = σ_z(τ, 0)² (1 + 2γh + 2γ²h²)`. The height `h` is
//! the root of
//!
//! `(1 ÷ h) ∫₀^∞ z e^(−z ÷ h) K_z(R_ref, z) dz = ⟨σ_z²⟩(h)`,
//!
//! whose left side is the density-weighted vertical Jeans equation for an exponential tracer, at
//! `R_ref` three thin-disc scale lengths out, in the potential of [`MassModel`], in which the young
//! and old discs are one double exponential of the drawn mean height (plan 02, Design note 6).
//!
//! The five heights are then scaled by one factor so that the sub-discs together have the
//! mid-plane density of that one disc: their share-weighted *harmonic* mean, `1 ÷ Σ wᵢ ÷ hᵢ`, is
//! the drawn mean height. The brainstorm's figures agree only under this reading: its heights,
//! "from about 320 ly at half a gigayear to 1,700 ly at ten", taken as a power of age between
//! those two and weighted by the bins' shares at a 7 Gyr timescale, have a harmonic mean of about
//! 1,080 ly, inside the drawn 850–1,150, and an arithmetic one of about 1,260, above it; and its
//! in-plane density at 26,000 ly is worked for one disc of the mean height. The share-weighted arithmetic mean of plan 02's Design note 9
//! put 16% more systems in the mid-plane than that (plan 02, Risks, R16). Heights are constant with
//! radius.
//!
//! # Quadrature
//!
//! `K_z` costs about a millisecond to evaluate: the model holds several hundred Gaussians, each a
//! 32-node quadrature. It is therefore evaluated once, at the 16 Gauss–Legendre nodes of three
//! panels in `ln z` over 2–131,072 ly, and read between them from each panel's polynomial of
//! degree 15 ([`Gl16Panel::value`]); below 2 ly it is taken as linear in z, as an odd, smooth
//! function is, and above 131,072 ly as falling as z⁻². The Jeans integral is then `h ∫₀^∞ t e^(−t)
//! K_z(h t) dt`, taken by `gl16` on fixed panels in `t` out to 40, and the root by a fixed 48
//! bisections in 16–8,192 ly. Against direct evaluation of `K_z` at every node, the Jeans integral
//! at the solved heights agrees to better than 10⁻⁷. All of it is part of the generator version.

use crate::galaxy::ages::{AgeDistribution, SubDisc};
use crate::galaxy::consts::{LIGHT_YEARS_PER_KILOPARSEC, YEARS_PER_GIGAYEAR};
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::potential::MassModel;
use crate::galaxy::quad::{Gl16Panel, bisect};
use crate::math;
use crate::tables::gauss_legendre::{GL16_NODES, GL16_WEIGHTS};
use crate::units::{KilometresPerSecond, LightYears, Years};

/// The heating law's vertical dispersion at 10 Gyr in the mid-plane, `σ_0,z` (Sharma et al. 2021,
/// Table 2: 21.1 ± 0.2 km/s; the brainstorm rounds it to 22).
pub const SIGMA_Z_AT_TEN_GYR: KilometresPerSecond = KilometresPerSecond::new(21.1);

/// The heating law's exponent `β_z`: `σ_z ∝ (τ + 0.1 Gyr)^0.441` (Sharma et al. 2021, Table 2:
/// 0.441 ± 0.007; the brainstorm rounds it to 0.44).
pub const HEATING_EXPONENT: f64 = 0.441;

/// The heating law's age offset, Gyr: a birth dispersion for stars younger than 0.1 Gyr (Sharma et
/// al. 2021, eq. 4).
pub const HEATING_AGE_OFFSET_GYR: f64 = 0.1;

/// How the vertical dispersion grows with height, `γ_z` per kpc: `σ_z ∝ 1 + γ_z |z|` (Sharma et al.
/// 2021, eq. 7 and Table 2: 0.20 ± 0.01 per kpc).
pub const DISPERSION_HEIGHT_GRADIENT_PER_KPC: f64 = 0.20;

/// The heating law in the mid-plane, `σ_z(τ, 0) = 21.1 km/s × ((τ ÷ Gyr + 0.1) ÷ 10.1)^0.441`
/// (module documentation).
#[must_use]
pub fn heating_law(age: Years) -> KilometresPerSecond {
    let gyr = age.value() / YEARS_PER_GIGAYEAR;
    SIGMA_Z_AT_TEN_GYR
        * math::powf(
            (gyr + HEATING_AGE_OFFSET_GYR) / (10.0 + HEATING_AGE_OFFSET_GYR),
            HEATING_EXPONENT,
        )
}

/// `⟨(1 + γ|z|)²⟩ = 1 + 2γh + 2γ²h²` over an exponential tracer of height `h` (ly): how much the
/// density-weighted `σ_z²` exceeds the mid-plane one.
#[must_use]
fn height_weighting(h: f64) -> f64 {
    let g = DISPERSION_HEIGHT_GRADIENT_PER_KPC / LIGHT_YEARS_PER_KILOPARSEC * h;
    1.0 + 2.0 * g * (1.0 + g)
}

/// Where the Jeans equation is solved, in thin-disc scale lengths (plan 02, Design note 9).
pub const REFERENCE_RADIUS_LENGTHS: f64 = 3.0;

/// The bisection's bracket for an unscaled height, ly.
const HEIGHT_BRACKET: (f64, f64) = (16.0, 8_192.0);

/// Bisections of the bracket: 48 halvings narrow it to 3 × 10⁻¹¹ ly.
const BISECTIONS: u32 = 48;

/// The heights, in ly, between which `K_z` is tabulated.
const FORCE_RANGE: (f64, f64) = (2.0, 131_072.0);

/// The panels of the table in `ln z`, equally wide, by their offsets from the lowest in widths.
const FORCE_PANELS: [f64; 3] = [0.0, 1.0, 2.0];

/// The panels of the Jeans integral in `t = z ÷ h`: beyond 40 the weight `t e^(−t)` is below
/// 2 × 10⁻¹⁶.
const JEANS_EDGES: [f64; 7] = [0.0, 0.25, 1.0, 3.0, 8.0, 18.0, 40.0];

/// The number of nodes of the Jeans integral: 16 per panel.
const JEANS_NODES: usize = 16 * (JEANS_EDGES.len() - 1);

/// The old thin disc's sub-discs as the vertical Jeans equation shapes them (plan 02, Design note
/// 9): the heating law `σ_z = 21.1 km/s × ((τ ÷ Gyr + 0.1) ÷ 10.1)^0.441 × (1 + 0.20 |z| ÷ kpc)`
/// of Sharma et al. (2021) at each sub-disc's mean age, the height at which an exponential
/// tracer's density-weighted `σ_z²` is the law's, and one factor that gives the five together the
/// mid-plane density of one disc of the drawn mean height.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::fields::SubDiscHeights;
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::galaxy::potential::MassModel;
///
/// let params = GalaxyParams::milky_way_like();
/// let heights = SubDiscHeights::solve(&params, &MassModel::new(&params));
/// // Older sub-discs are hotter and thicker, and together they have the mid-plane density of one
/// // disc of the drawn mean height.
/// let h = heights.heights();
/// assert!(h.windows(2).all(|w| w[0] < w[1]));
/// assert!((heights.mean_height() / params.thin_disc().height() - 1.0).abs() < 1e-12);
/// let inverse: f64 = heights.shares().iter().zip(h).map(|(w, h)| w / h.value()).sum();
/// assert!((inverse * params.thin_disc().height().value() - 1.0).abs() < 1e-12);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SubDiscHeights {
    reference_radius: LightYears,
    mean_ages: [Years; 5],
    dispersions: [KilometresPerSecond; 5],
    shares: [f64; 5],
    unscaled: [LightYears; 5],
    scale: f64,
    heights: [LightYears; 5],
}

impl SubDiscHeights {
    /// Solves the five heights of the galaxy `params` in its mass model `model`.
    ///
    /// # Panics
    ///
    /// Never for built parameters, whose formation timescale is positive.
    #[must_use]
    pub fn solve(params: &GalaxyParams, model: &MassModel) -> Self {
        let tau = params.sfh_timescale();
        let reference_radius = REFERENCE_RADIUS_LENGTHS * params.thin_disc().length().value();
        let force = VerticalForce::new(model, reference_radius);
        let rule = JeansRule::new();
        let weights = SubDisc::ALL.map(|bin| bin.share(tau));
        let total = weights.iter().fold(0.0, |sum, w| sum + w);
        let mean_ages = SubDisc::ALL.map(|bin| {
            AgeDistribution::old_thin_disc(tau, bin)
                .expect("built parameters hold a positive timescale")
                .mean()
        });
        let dispersions = mean_ages.map(heating_law);
        let unscaled = dispersions.map(|sigma| {
            let mid_plane = sigma.value() * sigma.value();
            bisect(
                |h| rule.dispersion_sq(&force, h) - mid_plane * height_weighting(h),
                HEIGHT_BRACKET.0,
                HEIGHT_BRACKET.1,
                BISECTIONS,
            )
        });
        // The mid-plane density of the sub-discs is Σ wᵢ ÷ hᵢ, in units of the old disc's count ÷
        // (4π L²) and in order; one disc of the drawn height has 1 ÷ h̄.
        let inverse = weights
            .iter()
            .zip(&unscaled)
            .fold(0.0, |sum, (w, h)| sum + w / h);
        let scale = params.thin_disc().height().value() * inverse / total;
        Self {
            reference_radius: LightYears::new(reference_radius),
            mean_ages,
            dispersions,
            shares: weights.map(|w| w / total),
            unscaled: unscaled.map(LightYears::new),
            scale,
            heights: unscaled.map(|h| LightYears::new(scale * h)),
        }
    }

    /// `R_ref`, three thin-disc scale lengths, where the Jeans equation is solved.
    #[must_use]
    pub fn reference_radius(&self) -> LightYears {
        self.reference_radius
    }

    /// Each sub-disc's age: the mean age of the formation history inside its bin, youngest first.
    #[must_use]
    pub fn mean_ages(&self) -> [Years; 5] {
        self.mean_ages
    }

    /// Each sub-disc's vertical dispersion in the mid-plane: Sharma et al.'s heating law at its
    /// mean age (the type's documentation).
    #[must_use]
    pub fn dispersions(&self) -> [KilometresPerSecond; 5] {
        self.dispersions
    }

    /// Each sub-disc's density-weighted vertical dispersion at its unscaled height, which the Jeans
    /// equation returns there: the mid-plane one times `√(1 + 2γh + 2γ²h²)` (module
    /// documentation). Plan 08's dispersions reproduce these at the unscaled heights.
    #[must_use]
    pub fn weighted_dispersions(&self) -> [KilometresPerSecond; 5] {
        let mut out = self.dispersions;
        for (sigma, h) in out.iter_mut().zip(&self.unscaled) {
            *sigma = *sigma * height_weighting(h.value()).sqrt();
        }
        out
    }

    /// Each sub-disc's share of the old thin disc's systems; they sum to 1.
    #[must_use]
    pub fn shares(&self) -> [f64; 5] {
        self.shares
    }

    /// The heights at which the Jeans equation returns the density-weighted dispersions, before
    /// scaling.
    #[must_use]
    pub fn unscaled(&self) -> [LightYears; 5] {
        self.unscaled
    }

    /// The factor that brings the sub-discs' mid-plane density to that of one disc of the drawn mean
    /// height. Far from 1, it says the model's disc mass and the measured heating law disagree with
    /// the drawn height (plan 02, Risks, R2 and R16).
    #[must_use]
    pub fn scale(&self) -> f64 {
        self.scale
    }

    /// The sub-discs' scale heights, youngest first.
    #[must_use]
    pub fn heights(&self) -> [LightYears; 5] {
        self.heights
    }

    /// The share-weighted harmonic mean of [`heights`](Self::heights), `1 ÷ Σ wᵢ ÷ hᵢ`: the height
    /// of the one disc with the sub-discs' mid-plane density, which is the drawn mean height.
    #[must_use]
    pub fn mean_height(&self) -> LightYears {
        let inverse = self
            .shares
            .iter()
            .zip(&self.heights)
            .fold(0.0, |sum, (w, h)| sum + w / h.value());
        LightYears::new(1.0 / inverse)
    }
}

/// `K_z(R_ref, z)`, tabulated on panels in `u = ln z` (module documentation, "Quadrature").
#[derive(Debug)]
struct VerticalForce {
    panels: [Gl16Panel; 3],
    /// `ln z` at the lowest edge, and each panel's width in `ln z`.
    lo: f64,
    width: f64,
    /// `K_z(z_lo) ÷ z_lo`, (km/s)² per ly².
    linear: f64,
    /// `K_z(z_hi) z_hi²`, (km/s)² ly.
    far: f64,
}

impl VerticalForce {
    fn new(model: &MassModel, r: f64) -> Self {
        let lo = math::ln(FORCE_RANGE.0);
        let hi = math::ln(FORCE_RANGE.1);
        let width = (hi - lo) / 3.0;
        let panels = FORCE_PANELS.map(|i| {
            let (a, b) = (lo + i * width, lo + (i + 1.0) * width);
            let values = Gl16Panel::nodes(a, b)
                .map(|u| model.vertical_force(LightYears::new(r), LightYears::new(math::exp(u))));
            Gl16Panel::new(a, b, &values)
        });
        let (z_lo, z_hi) = FORCE_RANGE;
        Self {
            linear: panels[0].value(lo) / z_lo,
            far: panels[2].value(hi) * z_hi * z_hi,
            panels,
            lo,
            width,
        }
    }

    /// `K_z` at height `z > 0`, whose logarithm is `u`.
    fn at(&self, u: f64, z: f64) -> f64 {
        let offset = (u - self.lo) / self.width;
        if offset < 0.0 {
            self.linear * z
        } else if offset > 3.0 {
            self.far / (z * z)
        } else {
            let panel = if offset < 1.0 {
                0
            } else if offset < 2.0 {
                1
            } else {
                2
            };
            self.panels[panel].value(u)
        }
    }
}

/// The nodes of the Jeans integral in `t`: `t`, `ln t`, and the weight times `t e^(−t)`.
#[derive(Debug)]
struct JeansRule {
    t: [f64; JEANS_NODES],
    ln_t: [f64; JEANS_NODES],
    weight: [f64; JEANS_NODES],
}

impl JeansRule {
    fn new() -> Self {
        let mut rule = Self {
            t: [0.0; JEANS_NODES],
            ln_t: [0.0; JEANS_NODES],
            weight: [0.0; JEANS_NODES],
        };
        let mut i = 0;
        for panel in JEANS_EDGES.windows(2) {
            let half = 0.5 * (panel[1] - panel[0]);
            let mid = panel[0] + half;
            for (&x, &w) in GL16_NODES.iter().zip(&GL16_WEIGHTS) {
                let t = mid + half * x;
                rule.t[i] = t;
                rule.ln_t[i] = math::ln(t);
                rule.weight[i] = w * half * t * math::exp(-t);
                i += 1;
            }
        }
        rule
    }

    /// `⟨σ_z²⟩(h) = h ∫₀^∞ t e^(−t) K_z(h t) dt`, (km/s)², for a height `h` (ly).
    fn dispersion_sq(&self, force: &VerticalForce, h: f64) -> f64 {
        let ln_h = math::ln(h);
        let mut sum = 0.0;
        for ((&t, &ln_t), &w) in self.t.iter().zip(&self.ln_t).zip(&self.weight) {
            sum += w * force.at(ln_h + ln_t, h * t);
        }
        h * sum
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::consts::G;
    use crate::galaxy::potential::mge::double_exponential;
    use crate::galaxy::quad::gl_panels;
    use crate::units::SolarMasses;

    /// The tabulated force against direct evaluation across its range, and the Jeans integral
    /// against a direct quadrature for a few heights.
    #[test]
    fn the_table_and_the_rule_match_direct_evaluation() {
        let params = GalaxyParams::milky_way_like();
        let model = MassModel::new(&params);
        let r = 3.0 * params.thin_disc().length().value();
        let force = VerticalForce::new(&model, r);
        let direct = |z: f64| model.vertical_force(LightYears::new(r), LightYears::new(z));
        for i in 0..=24 {
            let z = 2.5 * math::exp(f64::from(i) * 0.4);
            let table = force.at(math::ln(z), z);
            assert!(
                (table / direct(z) - 1.0).abs() < 1e-4,
                "K_z({z}): {table} against {}",
                direct(z)
            );
        }
        let rule = JeansRule::new();
        for h in [200.0, 900.0] {
            let edges: Vec<f64> = [0.0, 0.25, 1.0, 3.0, 8.0, 18.0, 40.0]
                .iter()
                .map(|t| t * h)
                .collect();
            let reference = gl_panels(|z| z * math::exp(-z / h) * direct(z), &edges) / h;
            let ours = rule.dispersion_sq(&force, h);
            assert!(
                (ours / reference - 1.0).abs() < 1e-6,
                "h {h}: {ours} against {reference}"
            );
        }
    }

    /// The heights solved from the table satisfy the Jeans equation with `K_z` evaluated directly at
    /// every node of the same rule, to the 10⁻⁷ the module documentation states, and
    /// the density-weighted dispersions reported there are the Jeans equation's.
    #[test]
    fn the_solved_heights_satisfy_the_jeans_equation_with_the_direct_force() {
        let params = GalaxyParams::milky_way_like();
        let model = MassModel::new(&params);
        let heights = SubDiscHeights::solve(&params, &model);
        let r = heights.reference_radius();
        for ((h, sigma), weighted) in heights
            .unscaled()
            .iter()
            .zip(heights.dispersions())
            .zip(heights.weighted_dispersions())
        {
            let h = h.value();
            let jeans = JEANS_EDGES.windows(2).fold(0.0, |sum, panel| {
                sum + crate::galaxy::quad::gl16(
                    |t| t * math::exp(-t) * model.vertical_force(r, LightYears::new(h * t)),
                    panel[0],
                    panel[1],
                )
            }) * h;
            let target = sigma.value() * sigma.value() * height_weighting(h);
            assert!(
                (jeans / target - 1.0).abs() < 1e-7,
                "h {h}: the direct Jeans integral {jeans} against {target}"
            );
            let weighted_sq = weighted.value() * weighted.value();
            assert!((weighted_sq / target - 1.0).abs() < 1e-14, "h {h}");
        }
    }

    /// The heating law is Sharma et al.'s (2021, eq. 4 and Table 2): 21.1 km/s at 10 Gyr, a birth
    /// dispersion at zero age, the exponent 0.441 on the age plus 0.1 Gyr; and the density-weighted
    /// `⟨(1 + γ|z|)²⟩` over an exponential is its closed form.
    #[test]
    fn the_heating_law_is_sharma_s() {
        let gyr = |t: f64| Years::new(t * YEARS_PER_GIGAYEAR);
        assert!((heating_law(gyr(10.0)).value() - 21.1).abs() < 1e-12);
        let birth = 21.1 * math::powf(0.1 / 10.1, 0.441);
        assert!((heating_law(Years::ZERO).value() / birth - 1.0).abs() < 1e-14);
        let ratio = heating_law(gyr(4.0)).value() / heating_law(gyr(1.0)).value();
        assert!((ratio / math::powf(4.1 / 1.1, 0.441) - 1.0).abs() < 1e-14);
        for h in [100.0, 1_000.0, 3_000.0] {
            let gamma = 0.2 / LIGHT_YEARS_PER_KILOPARSEC;
            let edges: Vec<f64> = [0.0, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0]
                .iter()
                .map(|t| t * h)
                .collect();
            let mean = gl_panels(
                |z| (1.0 + gamma * z) * (1.0 + gamma * z) * math::exp(-z / h) / h,
                &edges,
            );
            assert!((height_weighting(h) / mean - 1.0).abs() < 1e-12, "h {h}");
        }
        assert!((height_weighting(LIGHT_YEARS_PER_KILOPARSEC) - 1.48).abs() < 1e-14);
    }

    /// Above a razor-thin sheet of surface density Σ, `K_z = 2πGΣ`, and the Jeans equation gives
    /// `h = σ² ÷ 2πGΣ` exactly, whatever the heating law.
    #[test]
    fn a_thin_sheet_gives_the_textbook_height() {
        let rule = JeansRule::new();
        let sigma = 5.0; // M☉ per ly²
        let kz = 2.0 * core::f64::consts::PI * G * sigma;
        let lo = math::ln(FORCE_RANGE.0);
        let width = (math::ln(FORCE_RANGE.1) - lo) / 3.0;
        let panel = |i: f64| {
            let (a, b) = (lo + i * width, lo + (i + 1.0) * width);
            Gl16Panel::new(a, b, &[kz; 16])
        };
        let force = VerticalForce {
            panels: [panel(0.0), panel(1.0), panel(2.0)],
            lo,
            width,
            linear: kz / FORCE_RANGE.0,
            far: kz * FORCE_RANGE.1 * FORCE_RANGE.1,
        };
        for h in [300.0, 1_000.0] {
            let ours = rule.dispersion_sq(&force, h);
            // Below 2 ly the table is linear, which a sheet is not; that sliver is (2 ÷ h)² ÷ 6.
            assert!((ours / (kz * h) - 1.0).abs() < 2e-5, "h {h}");
        }
        // The disc model's thin limit agrees: a double exponential 1 ly thick.
        let disc = double_exponential(
            SolarMasses::new(1e10),
            LightYears::new(8_000.0),
            LightYears::new(1.0),
        )
        .unwrap();
        let r = 3.0 * 8_000.0;
        let surface = 1e10 / (2.0 * core::f64::consts::PI * 8_000.0 * 8_000.0) * math::exp(-3.0);
        let far_above = disc.iter().fold(0.0, |s, g| {
            s + g.vertical_force(LightYears::new(r), LightYears::new(50.0))
        });
        assert!((far_above / (2.0 * core::f64::consts::PI * G * surface) - 1.0).abs() < 0.02);
    }
}
