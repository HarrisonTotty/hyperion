//! The stellar halo's velocity laws: a constant-anisotropy spherical Jeans integral per component
//! (plan 08, P08.T3 and Design note 9).
//!
//! With the anisotropy `β(r) = β∞ r² ÷ (r² + a²)`, `a` the component's core radius, the spherical
//! Jeans equation's integrating factor is `(r² + a²)^β∞`, and `σ_r²(r) = (1 ÷ (ν (r² + a²)^β∞))
//! ∫_r^∞ ν (r′² + a²)^β∞ v_c²(r′) ÷ r′ dr′`, with `ν` the component's density along its major
//! axis, without the sphere that cuts it for placement, and `v_c² = G M(<r) ÷ r` the mass model's
//! monopole (ruling 105.5: the in-plane circular speed of a disc runs about a third above the
//! monopole at 2–4 scale lengths). Tabulated on 64 radii. For a power law of slope γ in a flat curve
//! and no core this is the brainstorm's `v_c² ÷ (γ − 2β)`, and unlike that form it stays finite in
//! the core. `σ_θ² = σ_φ² = (1 − β(r)) σ_r²`. The anisotropy falls to 0 in the core, as An and
//! Evans (2006, ApJ 642, 752) require of a cored tracer (`β(0) ≤ γ(0) ÷ 2`, with `γ(0) = 0`), and
//! since every component's slope exceeds `2β∞` it meets their limit at every radius (ruling 105.5).
//!
//! The anisotropy β∞ and net rotation per component kind are Design note 9's: the dominant merger
//! 0.9 and none (Belokurov et al. 2018, MNRAS 478, 611: β ≈ 0.9 for the Gaia-Enceladus debris);
//! the in-situ component 0.3 and prograde at 0.11 `v_c`, the Splash's 25 km/s (Belokurov et al.
//! 2020, MNRAS 494, 3880, Table 1; ruling 105.5, where Design note 9 had 0.35); the globular-born
//! debris 0.5 and none; each lesser progenitor β uniform on 0.3–0.7 and a rotation uniform on ±0.25 `v_c`, drawn
//! on `halo.kinematics` keyed by its item number. The mixture is tested against an anisotropy near
//! 0.6 and a radial dispersion near 141 km/s in Bond et al.'s volume, 1 < |Z| < 5 kpc and 3 < R <
//! 13 kpc (2010, ApJ 716, 1: (141, 75, 85) ± 5 km/s, β = 0.68).
//!
//! Speeds are km/s, lengths light-years, in the local spherical axes (r outward, θ from +z, φ
//! spinward).

use super::gl8;
use crate::galaxy::consts::G;
use crate::galaxy::params::{GalaxyParams, HaloComponentKind, HaloComponentParams};
use crate::galaxy::potential::{MassModel, PotentialTables};
use crate::math;
use crate::rng::{ObjectKey, Seed, Stream, tags};
use crate::units::{KilometresPerSecond, LightYears};

/// Radii per halo table: `r_k = 16 × 2^(k ÷ 4)` ly, 16 to 880,000 ly.
pub const HALO_RADII: usize = 64;

/// The first table radius, ly.
const FIRST_RADIUS: f64 = 16.0;

/// Table radii per octave.
const PER_OCTAVE: f64 = 4.0;

/// The tail's panels beyond the last table radius, in octaves: the integrand falls at least as
/// `r^−1.4` there, and beyond 2²⁴ of the last radius it adds under 10⁻⁹ of the sum.
const TAIL_OCTAVES: u32 = 24;

/// β and net rotation (in units of `v_c`) of the dominant merger (plan 08, Design note 9).
pub const DOMINANT_MERGER: (f64, f64) = (0.9, 0.0);

/// β and net rotation of the in-situ component (plan 08, Design note 9, with ruling 105.5's
/// rotation: the Splash's 25 km/s, 0.11 of the circular speed).
pub const IN_SITU: (f64, f64) = (0.3, 0.11);

/// β and net rotation of the globular-born debris (plan 08, Design note 9).
pub const GLOBULAR_DEBRIS: (f64, f64) = (0.5, 0.0);

/// The range of a lesser progenitor's β (plan 08, Design note 9).
pub const LESSER_BETA_RANGE: (f64, f64) = (0.3, 0.7);

/// The largest net rotation of a lesser progenitor, either sense, in units of `v_c`.
pub const LESSER_ROTATION: f64 = 0.25;

/// One halo component's velocity law.
#[derive(Debug, Clone, PartialEq)]
pub struct HaloComponentKinematics {
    kind: HaloComponentKind,
    beta: f64,
    /// `a²`, ly²: where the anisotropy turns over (the component's core).
    core_sq: f64,
    rotation: f64,
    /// `σ_r²` at the table's radii, (km/s)².
    sigma_r_sq: [f64; HALO_RADII],
}

impl HaloComponentKinematics {
    /// The component this law is of.
    #[must_use]
    pub fn kind(&self) -> HaloComponentKind {
        self.kind
    }

    /// The anisotropy far out, `β∞`: `β(r) = β∞ r² ÷ (r² + a²)`.
    #[must_use]
    pub fn beta(&self) -> f64 {
        self.beta
    }

    /// The anisotropy `β = 1 − σ_t² ÷ σ_r²` at spherical radius `r` (ly).
    #[must_use]
    pub fn beta_at(&self, r: f64) -> f64 {
        let r_sq = r * r;
        if r_sq + self.core_sq > 0.0 {
            self.beta * r_sq / (r_sq + self.core_sq)
        } else {
            self.beta
        }
    }

    /// The net rotation in units of the circular speed, positive prograde.
    #[must_use]
    pub fn rotation(&self) -> f64 {
        self.rotation
    }

    /// `σ_r` at spherical radius `r` (ly): interpolated in `ln r`, level outside the table.
    #[must_use]
    pub fn sigma_r(&self, r: LightYears) -> KilometresPerSecond {
        KilometresPerSecond::new(self.sigma_r_sq_at(r.value()).sqrt())
    }

    fn sigma_r_sq_at(&self, r: f64) -> f64 {
        interpolate(&self.sigma_r_sq, r)
    }

    /// The mean `[v_r, v_θ, v_φ]` and dispersions `[σ_r, σ_θ, σ_φ]` at spherical radius `r` and
    /// `sin θ` (θ from +z), km/s, in a curve of circular speed `v_c` there. The rotation is
    /// `rotation × v_c × sin θ`, which vanishes on the axis.
    #[must_use]
    pub fn at(&self, r: f64, sin_theta: f64, v_c: f64) -> ([f64; 3], [f64; 3]) {
        let sigma_r = self.sigma_r_sq_at(r).sqrt();
        let sigma_t = sigma_r * (1.0 - self.beta_at(r)).sqrt();
        (
            [0.0, 0.0, self.rotation * v_c * sin_theta],
            [sigma_r, sigma_t, sigma_t],
        )
    }
}

/// The halo's laws, one per smooth component in the halo's order.
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::kinematics::halo::HaloKinematics;
/// use hyperion_sim::galaxy::params::{GalaxyParams, HaloComponentKind};
/// use hyperion_sim::galaxy::potential::{MassModel, PotentialTables};
/// use hyperion_sim::units::LightYears;
///
/// let params = GalaxyParams::milky_way_like();
/// let model = MassModel::new(&params);
/// let tables = PotentialTables::in_plane(&model);
/// let halo = HaloKinematics::new(Seed::new(1), &params, &model, &tables);
/// let dominant = halo
///     .components()
///     .iter()
///     .find(|c| c.kind() == HaloComponentKind::DominantMerger)
///     .expect("every halo has one");
/// // Radial orbits and no net rotation.
/// assert!((dominant.beta() - 0.9).abs() < 1e-15 && dominant.rotation().abs() < 1e-15);
/// assert!(dominant.sigma_r(LightYears::new(26_000.0)).value() > 100.0);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct HaloKinematics {
    components: Vec<HaloComponentKinematics>,
    /// The in-plane `v_c²` at the table's radii, (km/s)², which the rotation reads.
    v_circ_sq: [f64; HALO_RADII],
}

impl HaloKinematics {
    /// The laws of the halo of `params` in the monopole of `model`, whose in-plane tables are
    /// `potential` (the rotation reads their circular speed); the lesser progenitors' draws are
    /// keyed by `seed`.
    #[must_use]
    pub fn new(
        seed: Seed,
        params: &GalaxyParams,
        model: &MassModel,
        potential: &PotentialTables,
    ) -> Self {
        let v_c_sq = |r: f64| potential.v_circ_sq(LightYears::new(r));
        let monopole = Monopole::new(model);
        let monopole_sq = |r: f64| monopole.v_circ_sq(r);
        let components = params
            .halo()
            .components()
            .iter()
            .map(|c| {
                let (beta, rotation) = kinematics_of(seed, c.kind());
                let core = c.core().value();
                HaloComponentKinematics {
                    kind: c.kind(),
                    beta,
                    core_sq: core * core,
                    rotation,
                    sigma_r_sq: radial_dispersion_sq(
                        |r| ln_density(c, r),
                        (beta, core),
                        monopole_sq,
                    ),
                }
            })
            .collect();
        Self {
            components,
            v_circ_sq: core::array::from_fn(|k| v_c_sq(table_radius(k))),
        }
    }

    /// The mean `[v_r, v_θ, v_φ]` and dispersions `[σ_r, σ_θ, σ_φ]` of component `index` (in
    /// the halo's order) at spherical radius `r` (ly) and `sin θ`, km/s, with the circular speed
    /// read from the table.
    #[must_use]
    pub fn at(&self, index: usize, r: f64, sin_theta: f64) -> ([f64; 3], [f64; 3]) {
        let v_c = interpolate(&self.v_circ_sq, r).sqrt();
        self.components[index].at(r, sin_theta, v_c)
    }

    /// The components' laws, in the halo's order.
    #[must_use]
    pub fn components(&self) -> &[HaloComponentKinematics] {
        &self.components
    }

    /// The law of the component `kind`, if the halo has one.
    #[must_use]
    pub fn component(&self, kind: HaloComponentKind) -> Option<&HaloComponentKinematics> {
        self.components.iter().find(|c| c.kind == kind)
    }

    /// The bytes the laws own on the heap.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        self.components.capacity() * size_of::<HaloComponentKinematics>()
    }
}

/// β and net rotation of the component `kind`: fixed by kind, or for a lesser progenitor drawn
/// on `halo.kinematics`, words 0 and 1.
fn kinematics_of(seed: Seed, kind: HaloComponentKind) -> (f64, f64) {
    match kind {
        HaloComponentKind::DominantMerger => DOMINANT_MERGER,
        HaloComponentKind::InSitu => IN_SITU,
        HaloComponentKind::GlobularDebris => GLOBULAR_DEBRIS,
        HaloComponentKind::Lesser(_) => {
            let mut stream = Stream::open(
                seed,
                tags::HALO_KINEMATICS,
                ObjectKey::galaxy_item(kind.item()),
            );
            let beta = stream.uniform_in(LESSER_BETA_RANGE.0, LESSER_BETA_RANGE.1);
            let rotation = stream.uniform_in(-LESSER_ROTATION, LESSER_ROTATION);
            (beta, rotation)
        }
    }
}

/// `ln ν` along the component's major axis at radius `r`, without the placement's cut: `−(γ ÷ 2)
/// ln(1 + r² ÷ a²)`, less `Δ ln(r ÷ r_b)` beyond a break.
fn ln_density(c: &HaloComponentParams, r: f64) -> f64 {
    let a = c.core().value();
    let mut ln = -0.5 * c.slope() * math::ln_1p((r / a) * (r / a));
    if let Some(b) = c.outer_break()
        && r > b.radius().value()
    {
        ln -= b.steepening() * math::ln(r / b.radius().value());
    }
    ln
}

/// The mass model's monopole, `v_c² = G M(<r) ÷ r`: the Gaussians' enclosed mass tabulated at the
/// halo table's radii and read linearly in `ln r` between them (it is constant beyond the last,
/// 880,000 ly, where every Gaussian is enclosed), the spherical components' in closed form.
struct Monopole<'a> {
    model: &'a MassModel,
    extended: [f64; HALO_RADII],
}

impl<'a> Monopole<'a> {
    fn new(model: &'a MassModel) -> Self {
        Self {
            model,
            extended: core::array::from_fn(|k| {
                model
                    .expanded_enclosed_mass(LightYears::new(table_radius(k)))
                    .value()
            }),
        }
    }

    /// `G M(<r) ÷ r` at `r > 0` (ly), (km/s)².
    fn v_circ_sq(&self, r: f64) -> f64 {
        let first = table_radius(0);
        let extended = if r < first {
            // Solid body inside the first radius: the enclosed mass grows as r³.
            self.extended[0] * (r / first) * (r / first) * (r / first)
        } else {
            interpolate(&self.extended, r)
        };
        let spherical = self.model.enclosed_mass(LightYears::new(r)).value();
        G * (extended + spherical) / r
    }
}

/// `table` at radius `r` (ly): linear in `ln r` between the table's radii, level outside them.
fn interpolate(table: &[f64; HALO_RADII], r: f64) -> f64 {
    let u = if r > FIRST_RADIUS {
        math::log2(r / FIRST_RADIUS) * PER_OCTAVE
    } else {
        0.0
    };
    let (k, t) = super::discs::split(u, HALO_RADII);
    table[k] * (1.0 - t) + table[k + 1] * t
}

/// The table radius `k`, ly.
fn table_radius(k: usize) -> f64 {
    FIRST_RADIUS * math::exp2(f64::from(u8::try_from(k).expect("below 64")) / PER_OCTAVE)
}

/// `σ_r²` at the table's radii for the tracer `ln ν(r)`, anisotropy `β(r) = β∞ r² ÷ (r² + a²)`
/// from `(β∞, a)`, and curve `v_c²(r)`: the integral in `ln r` of `ν (r² + a²)^β∞ v_c²`, by an
/// 8-point rule between table radii and on doubling panels beyond, summed downwards with the
/// rescaling of the discs' integral. With `a = 0` the anisotropy is constant.
pub(crate) fn radial_dispersion_sq(
    ln_nu: impl Fn(f64) -> f64,
    (beta, core): (f64, f64),
    v_c_sq: impl Fn(f64) -> f64,
) -> [f64; HALO_RADII] {
    let core_sq = core * core;
    let weight = |r: f64| ln_nu(r) + beta * math::ln(r * r + core_sq);
    let last = table_radius(HALO_RADII - 1);
    let w_last = weight(last);
    let mut tail = 0.0;
    let mut lo = last;
    for _ in 0..TAIL_OCTAVES {
        let hi = 2.0 * lo;
        tail += gl8(
            |u| {
                let r = math::exp(u);
                math::exp(weight(r) - w_last) * v_c_sq(r)
            },
            math::ln(lo),
            math::ln(hi),
        );
        lo = hi;
    }
    let mut out = [0.0; HALO_RADII];
    out[HALO_RADII - 1] = tail;
    let mut above = tail;
    for k in (0..HALO_RADII - 1).rev() {
        let (lo, hi) = (table_radius(k), table_radius(k + 1));
        let w_lo = weight(lo);
        let panel = gl8(
            |u| {
                let r = math::exp(u);
                math::exp(weight(r) - w_lo) * v_c_sq(r)
            },
            math::ln(lo),
            math::ln(hi),
        );
        above = panel + math::exp(weight(hi) - w_lo) * above;
        out[k] = above;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// P08.T3: a pure power law of slope 3.5 in a flat curve gives `v_c ÷ √(3.5 − 2β)` to 1%
    /// between 5 and 50 core radii (the law has no core; the range is the plan's).
    #[test]
    fn a_power_law_in_a_flat_curve_gives_the_closed_form() {
        let v_c = 220.0;
        for beta in [0.0, 0.3, 0.6, 0.9] {
            let table = radial_dispersion_sq(|r| -3.5 * math::ln(r), (beta, 0.0), |_| v_c * v_c);
            let law = HaloComponentKinematics {
                kind: HaloComponentKind::InSitu,
                beta,
                core_sq: 0.0,
                rotation: 0.0,
                sigma_r_sq: table,
            };
            let exact = v_c / (3.5 - 2.0 * beta).sqrt();
            for r in [5_000.0, 12_000.0, 26_000.0, 50_000.0] {
                let s = law.sigma_r(LightYears::new(r)).value();
                assert!(
                    (s / exact - 1.0).abs() < 0.01,
                    "β {beta}, r {r}: {s} against {exact}"
                );
            }
        }
    }

    /// P08.T3: a cored law's dispersion is finite and positive at the centre.
    #[test]
    fn a_cored_law_is_finite_at_the_centre() {
        let table = radial_dispersion_sq(
            |r| -1.25 * math::ln_1p((r / 3_000.0) * (r / 3_000.0)),
            (0.5, 3_000.0),
            |r| 4e4 * r * r / (r * r + 1e6),
        );
        let law = HaloComponentKinematics {
            kind: HaloComponentKind::GlobularDebris,
            beta: 0.5,
            core_sq: 9e6,
            rotation: 0.0,
            sigma_r_sq: table,
        };
        let centre = law.sigma_r(LightYears::new(0.0)).value();
        assert!(centre.is_finite() && centre > 0.0, "{centre}");
        // The anisotropy is 0 at the centre, half its far value at the core, and tends to it.
        assert!(law.beta_at(0.0).abs() < 1e-15);
        assert!((law.beta_at(3_000.0) - 0.25).abs() < 1e-12);
        assert!((law.beta_at(3e6) - 0.5).abs() < 1e-6);
    }
}
