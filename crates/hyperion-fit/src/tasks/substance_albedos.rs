//! `run substance_albedos`: the condensed optics of the substance registry's solids (plan 14,
//! P14.T49.c), each row's normal albedo in B, V and R at its reference grain class by Hapke's
//! model, which the sim's `substance::optics` reads.
//!
//! # The model
//!
//! For each row the sim's `substance::optics::rows` lists, in each band of
//! `substance::optics::BAND_WAVELENGTHS_NM` (440, 550 and 640 nm):
//!
//! 1. **The index**, n + ik, from the row's committed index file (rendering plan R08's, read in
//!    place through [`crate::materials`], each file's data hash in the manifest's `materials`
//!    list), found by wavelength on the file's own grid ([`index_at`]).
//! 2. **The single-scattering albedo** of a grain of the reference form's diameter D, by Hapke's
//!    equivalent slab with no internal scatterers ([`single_scattering_albedo`]): w = `S_e` +
//!    (1 − `S_e`)(1 − `S_i`) Θ ÷ (1 − `S_i` Θ), Θ = exp(−α ⟨D⟩), α = 4πk ÷ λ, ⟨D⟩ = ⅔ [n² −
//!    (n² − 1)^{3/2} ÷ n] D, and `S_e` and `S_i` Hapke 2012's fits to the Fresnel reflectances
//!    over the grain's surface, from R(0) = [(n − 1)² + k²] ÷ [(n + 1)² + k²].
//! 3. **The normal albedo**, Hapke's reflectance factor (π r ÷ μ₀, which at i = 0 is the radiance
//!    factor I/F) with isotropic multiple scattering (the IMSA, Hapke 1981 and 1986, under which
//!    both reference forms' photometry was fitted):
//!    r(μ₀, μ, g) = (w ÷ 4) ÷ (μ₀ + μ) · {p(g) [1 + B(g)] + H(μ₀) H(μ) − 1}, with the reference
//!    form's phase function p and opposition effect B
//!    ([`hyperion_sim::substance::optics::Photometry`]), no macroscopic roughness, and
//!    Chandrasekhar's exact H-function ([`h_function`]): `A_N` = r(1, 1, 0) = (w ÷ 8) {p(0) [1 +
//!    `B₀`] + H(1)² − 1} ([`Medium::normal_albedo`]).
//! 4. **The energy albedos**, by an energy-conserving solution (decision-p14-t49c-split §1.2): van
//!    de Hulst's similarity relation's isotropic medium of albedo w* = w (1 − ξ) ÷ (1 − w ξ), ξ
//!    the phase function's asymmetry ([`similar_isotropic_albedo`]), whose
//!    directional-hemispherical albedo is exactly 1 − √(1 − w*) H(μ₀; w*) at μ₀ = 1, 0.5, 0.2 and
//!    0.05, and whose spherical albedo is 2 ∫ `A_h`(μ₀) μ₀ dμ₀, by Gauss–Legendre quadrature
//!    ([`energy_albedos`]); no opposition term, since shadow hiding moves light in angle and adds
//!    none. The IMSA's own
//!    directional-hemispherical albedo ([`Medium::directional_hemispherical_albedo`]) loses up to
//!    11% of a lossless frost's light and makes more than it receives on a bright regolith, so it
//!    serves the energy test of `A_N` alone (decision-r10-limb §4).
//!
//! Every transcendental goes through `hyperion_sim::math`, and every sum runs in a fixed order, so
//! the table comes out the same to the bit on every platform.
//!
//! # Sources
//!
//! No primary of Hapke's was reachable; each formula is read from a reproduction that names its
//! place in the primary:
//!
//! - the equivalent slab, its Θ and ⟨D⟩, and Hapke 2012's surface-reflection fits: Davalos 2026
//!   (Rev. Mex. Astron. Astrofís. 62, 197), eqs. 7–14, citing Hapke 2012, §5.6, and Lapotre et al.
//!   2017 (JGR Planets 122, 983), eqs. 4 and 7–10 (its eqs. 5–6 are Lucey 1998's older surface
//!   reflections); α = 4πk ÷ λ as Khuller and Emran (arXiv 2509.06224,
//!   eq. 7) say Hapke 2012 recommends it;
//! - the IMSA's radiance factor and the shadow-hiding opposition effect: Hapke, Denevi, Sato,
//!   Braden and Robinson 2012 (JGR 117, E00H15, eqs. 2–4 and ¶21, read through the Wayback
//!   Machine's copy of the publisher's full text), Hapke's own LPSC XXVIII abstract (1997, #1214)
//!   and Protopapa et al. 2017 (arXiv 1604.08468), eqs. 1–2;
//! - the H-function: Chandrasekhar's for isotropic scattering, by its integral representation as
//!   Kawabata 2016 (arXiv 1611.10197, eqs. 2–3, p. 2) prints it, after Rutily and Bergeat 1987 (as
//!   Kawabata and Limaye 2011, arXiv 1010.4839, eq. 3, cite it), with Kawabata 2016's exact values
//!   (Tables 2, 3 and 6) and zeroth moment (eq. 12), which this task's tests reproduce;
//! - the similarity relation: van de Hulst 1974 (A&A 35, 209, read in the ADS scan), eq. 14
//!   (p. 211), which keeps his similarity parameter s = √[(1 − a) ÷ (1 − ag)] (eq. 13). His
//!   spherical albedo is a function of s alone within 0.002 for any (a, g) of a Henyey–Greenstein
//!   medium, g 0 to 0.875 (summary, p. 209; §3, p. 211), so the isotropic medium of the same s is
//!   within 0.004 of it; the regolith's backscattering Legendre function (ξ −0.11) extrapolates
//!   his test. "Such a close similarity cannot be found for the plane albedo" (summary, p. 209;
//!   §4, p. 213). `substance_albedos_the_similarity_relation_meets_van_de_hulst_s_tables`
//!   measures both against his Tables 1 and 2;
//! - the check against the Moon: Hapke, Denevi, Sato, Braden and Robinson 2012 (JGR 117, E00H15),
//!   Table 1, whose normal albedos this model reproduces from their parameters
//!   (`substance_albedos_reproduces_the_lunar_normal_albedos`).

use std::fmt::Write as _;
use std::num::NonZeroUsize;

use hyperion_sim::math;
use hyperion_sim::substance::SubstanceId;
use hyperion_sim::substance::optics::{
    self, ALBEDO_ROW_LEN, BAND_WAVELENGTHS_NM, HEMISPHERICAL_INCIDENCE_COSINES, OppositionEffect,
    PhaseFunction, Photometry, ReferenceForm,
};
use hyperion_sim::tables::gauss_legendre::{GL32_NODES, GL32_WEIGHTS, GL64_NODES, GL64_WEIGHTS};
use hyperion_sim::units::Metres;
use hyperion_surface::material::PARTICULATE_NORMAL_ALBEDO_CEILING;

use crate::emit::{RustTable, TableItem};
use crate::manifest::{Manifest, SimFingerprint};
use crate::materials::MaterialFile;
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput, check_manifest};

/// The task's revision.
pub const VERSION: u32 = 0;

/// The task's name.
const NAME: &str = "substance_albedos";

/// The table's items, one per row with condensed optics in registry order, named by its key.
pub const ITEMS: [&str; 20] = [
    "H2O_ALBEDO",
    "CH4_ALBEDO",
    "NH3_ALBEDO",
    "N2_ALBEDO",
    "O2_ALBEDO",
    "CO2_ALBEDO",
    "AR_ALBEDO",
    "CO_ALBEDO",
    "H2S_ALBEDO",
    "NH4SH_ALBEDO",
    "ZNS_ALBEDO",
    "NA2S_ALBEDO",
    "MNS_ALBEDO",
    "MGSIO3_ALBEDO",
    "MG2SIO4_ALBEDO",
    "CATIO3_ALBEDO",
    "THOLIN_ALBEDO",
    "BASALT_ALBEDO",
    "BASALT_FE_ALBEDO",
    "LAVA_GLASS_ALBEDO",
];

/// The table item's name for row `id`: its key in capitals, then `_ALBEDO`.
#[must_use]
pub fn item_name(id: SubstanceId) -> String {
    format!(
        "{}_ALBEDO",
        id.substance().key().as_str().to_ascii_uppercase()
    )
}

/// The single-scattering albedo w of a grain of diameter `diameter` whose index at
/// `wavelength_nm` is n + ik: Hapke's equivalent slab with no internal scatterers, in [0, 1].
///
/// `S_e` = 0.0587 + 0.8543 R(0) + 0.0870 R(0)² and `S_i` = 1 − [0.9413 − 0.8543 R(0) − 0.0870
/// R(0)²] ÷ n², with R(0) = [(n − 1)² + k²] ÷ [(n + 1)² + k²] (Hapke 2012's fits, Davalos 2026,
/// eqs. 9–11); Θ = exp(−α ⟨D⟩) with α = 4πk ÷ λ and ⟨D⟩ = ⅔ [n² − (n² − 1)^{3/2} ÷ n] D (eqs. 12–14
/// with s = 0); w = `S_e` + (1 − `S_e`)(1 − `S_i`) Θ ÷ (1 − `S_i` Θ) (eqs. 7–8). The slab is
/// derived for k ≪ 1 and grains larger than a few wavelengths. A grain of k = 0 has w = 1, to which
/// a sum rounded past 1 is held.
///
/// # Panics
///
/// If n is below 1, where ⟨D⟩ is not real: no index file holds one.
#[must_use]
pub fn single_scattering_albedo(n: f64, k: f64, wavelength_nm: f64, diameter: Metres) -> f64 {
    assert!(n >= 1.0, "the equivalent slab needs n ≥ 1, not {n}");
    let r0 = ((n - 1.0) * (n - 1.0) + k * k) / ((n + 1.0) * (n + 1.0) + k * k);
    let s_e = 0.0587 + 0.8543 * r0 + 0.0870 * r0 * r0;
    let s_i = 1.0 - (0.9413 - 0.8543 * r0 - 0.0870 * r0 * r0) / (n * n);
    let alpha = 4.0 * core::f64::consts::PI * k / (wavelength_nm * 1.0e-9);
    let n2 = n * n;
    let mean_path = 2.0 / 3.0 * (n2 - (n2 - 1.0) * (n2 - 1.0).sqrt() / n) * diameter.value();
    let theta = math::exp(-alpha * mean_path);
    let w = s_e + (1.0 - s_e) * (1.0 - s_i) * theta / (1.0 - s_i * theta);
    // At Θ = 1 the sum is 1 exactly, which rounding can carry past it by an ulp.
    if w > 1.0 { 1.0 } else { w }
}

/// The panels of the H-function's quadrature in s, θ = (π ÷ 2) s².
const H_PANELS: u32 = 32;

/// Below this θ, rad, 1 − w θ cot θ is taken from its series, where the difference loses digits.
const H_SERIES_BELOW_RAD: f64 = 0.05;

/// Chandrasekhar's H-function for isotropic scattering of single-scattering albedo `w` in [0, 1] at
/// `mu` in [0, 1]: ln H(μ) = −(μ ÷ π) ∫₀^{π/2} ln(1 − w θ cot θ) ÷ (cos² θ + μ² sin² θ) dθ,
/// Kawabata 2016's eqs. 2–3 (arXiv 1611.10197, p. 2), whose (1 + tan² θ) ÷ (1 + μ² tan² θ) is
/// this kernel.
///
/// The integral is taken in s, θ = (π ÷ 2) s², by [`H_PANELS`] panels of 32-point Gauss–Legendre,
/// so that the logarithm's singularity at θ = 0 when w = 1 is integrable smoothly; below
/// [`H_SERIES_BELOW_RAD`] the bracket is (1 − w) + w (θ² ÷ 3 + θ⁴ ÷ 45 + 2θ⁶ ÷ 945 + θ⁸ ÷ 4,725), θ
/// cot θ's series. The tests hold it to Kawabata 2016's exact values to 10⁻⁸ and to its zeroth
/// moment, ∫₀¹ H dμ = (2 ÷ w)(1 − √(1 − w)).
#[must_use]
pub fn h_function(w: f64, mu: f64) -> f64 {
    if mu <= 0.0 || w <= 0.0 {
        return 1.0;
    }
    let half_pi = core::f64::consts::FRAC_PI_2;
    let mut sum = 0.0;
    for panel in 0..H_PANELS {
        let lo = f64::from(panel) / f64::from(H_PANELS);
        let hi = f64::from(panel + 1) / f64::from(H_PANELS);
        let (mid, half) = (f64::midpoint(lo, hi), 0.5 * (hi - lo));
        for (x, weight) in GL32_NODES.iter().zip(GL32_WEIGHTS) {
            let s = mid + half * x;
            let theta = half_pi * s * s;
            let bracket = if theta < H_SERIES_BELOW_RAD {
                let t2 = theta * theta;
                (1.0 - w).max(0.0)
                    + w * (t2 / 3.0
                        + t2 * t2 / 45.0
                        + 2.0 * t2 * t2 * t2 / 945.0
                        + t2 * t2 * t2 * t2 / 4_725.0)
            } else {
                1.0 - w * theta / math::tan(theta)
            };
            let (sin, cos) = math::sin_cos(theta);
            let denominator = cos * cos + mu * mu * sin * sin;
            sum += half * weight * math::ln(bracket) / denominator * (core::f64::consts::PI * s);
        }
    }
    math::exp(-mu / core::f64::consts::PI * sum)
}

/// The edges of the panels in μ over which a hemisphere is integrated: each of
/// [`HEMISPHERICAL_INCIDENCE_COSINES`] is an edge, so that the opposition peak at μ = μ₀ and the
/// kernel 1 ÷ (μ₀ + μ) of a grazing incidence fall at a panel's end.
const MU_EDGES: [f64; 5] = [0.0, 0.05, 0.2, 0.5, 1.0];

/// The nodes in μ of the hemisphere's quadrature: 32-point Gauss–Legendre on each panel of
/// [`MU_EDGES`], each with its weight, in order.
#[must_use]
fn mu_nodes() -> Vec<(f64, f64)> {
    let mut nodes = Vec::with_capacity(32 * (MU_EDGES.len() - 1));
    for pair in MU_EDGES.windows(2) {
        let (mid, half) = (f64::midpoint(pair[0], pair[1]), 0.5 * (pair[1] - pair[0]));
        for (x, weight) in GL32_NODES.iter().zip(GL32_WEIGHTS) {
            nodes.push((mid + half * x, half * weight));
        }
    }
    nodes
}

/// The shadow-hiding opposition effect B(g) at phase cosine `cos_g`, amplitude `b0` and width
/// parameter `width` (h, dimensionless): `B₀` ÷ (1 + tan(g ÷ 2) ÷ h), with tan(g ÷ 2) =
/// √[(1 − cos g) ÷ (1 + cos g)]; 0 at g = π.
#[must_use]
fn shadow_hiding(cos_g: f64, b0: f64, width: f64) -> f64 {
    if b0 <= 0.0 {
        return 0.0;
    }
    let cos_g = cos_g.clamp(-1.0, 1.0);
    if cos_g <= -1.0 {
        return 0.0;
    }
    let tan_half = ((1.0 - cos_g) / (1.0 + cos_g)).sqrt();
    b0 / (1.0 + tan_half / width)
}

/// One grain population under one photometry: w, the photometry and its H-function.
#[derive(Debug, Clone, PartialEq)]
pub struct Medium {
    w: f64,
    phase: PhaseFunction,
    b0: f64,
    width: f64,
    /// Each node of [`mu_nodes`] with its weight and H there.
    nodes: Vec<(f64, f64, f64)>,
}

impl Medium {
    /// Grains of single-scattering albedo `w` under `photometry`.
    #[must_use]
    pub fn new(w: f64, photometry: &Photometry) -> Self {
        let width = match photometry.opposition() {
            OppositionEffect::None => 1.0,
            OppositionEffect::ShadowHiding { width, .. } => width,
        };
        Self::from_parts(
            w,
            photometry.phase_function(),
            photometry.opposition_amplitude(w),
            width,
        )
    }

    /// Grains of single-scattering albedo `w` with phase function `phase` and a shadow-hiding
    /// opposition effect of amplitude `b0` (`B₀`, dimensionless, at least 0; 0 for none) and width
    /// parameter `width` (h, dimensionless, positive): any Hapke solution of this form, as the
    /// tests take published ones.
    #[must_use]
    pub fn from_parts(w: f64, phase: PhaseFunction, b0: f64, width: f64) -> Self {
        Self {
            w,
            phase,
            b0,
            width,
            nodes: mu_nodes()
                .into_iter()
                .map(|(mu, weight)| (mu, weight, h_function(w, mu)))
                .collect(),
        }
    }

    /// The amplitude `B₀` of its opposition effect.
    #[must_use]
    pub const fn opposition_amplitude(&self) -> f64 {
        self.b0
    }

    /// The radiance factor at incidence cosine `mu0`, emission cosine `mu` and phase cosine
    /// `cos_g`, given H at `mu0` and `mu`.
    #[must_use]
    fn reflectance_factor(&self, mu0: f64, mu: f64, cos_g: f64, h0: f64, h: f64) -> f64 {
        let single = self.phase.at(cos_g) * (1.0 + shadow_hiding(cos_g, self.b0, self.width));
        self.w / 4.0 / (mu0 + mu) * (single + h0 * h - 1.0)
    }

    /// The normal albedo, the radiance factor at i = e = g = 0: (w ÷ 8) {p(0) [1 + `B₀`] + H(1)² −
    /// 1}.
    #[must_use]
    pub fn normal_albedo(&self) -> f64 {
        let h1 = h_function(self.w, 1.0);
        self.w / 8.0 * (self.phase.at(1.0) * (1.0 + self.b0) + h1 * h1 - 1.0)
    }

    /// The directional-hemispherical albedo at incidence cosine `mu0` in (0, 1]: (1 ÷ π) ∫ r μ dΩ
    /// over the upper hemisphere, by 32-point Gauss–Legendre in μ on each panel of [`MU_EDGES`]
    /// and 64-point in the azimuth on [0, π], the integrand being even in it, summed in order.
    #[must_use]
    pub fn directional_hemispherical_albedo(&self, mu0: f64) -> f64 {
        let h0 = h_function(self.w, mu0);
        let sin0 = (1.0 - mu0 * mu0).max(0.0).sqrt();
        let cos_phi = GL64_NODES.map(|y| math::cos(core::f64::consts::FRAC_PI_2 * (1.0 + y)));
        let mut sum = 0.0;
        for &(mu, weight, h) in &self.nodes {
            let sin = (1.0 - mu * mu).max(0.0).sqrt();
            let mut azimuthal = 0.0;
            for (cos_phi, weight_phi) in cos_phi.iter().zip(GL64_WEIGHTS) {
                let cos_g = mu0 * mu + sin0 * sin * cos_phi;
                azimuthal += core::f64::consts::FRAC_PI_2
                    * weight_phi
                    * self.reflectance_factor(mu0, mu, cos_g, h0, h);
            }
            // The azimuth's other half is the mirror image.
            sum += weight * mu * 2.0 * azimuthal;
        }
        sum / core::f64::consts::PI
    }

    /// The spherical albedo, 2 ∫₀¹ `A_h`(μ₀) μ₀ dμ₀, by 32-point Gauss–Legendre in μ₀.
    #[must_use]
    pub fn spherical_albedo(&self) -> f64 {
        let mut sum = 0.0;
        for (x, weight) in GL32_NODES.iter().zip(GL32_WEIGHTS) {
            let mu0 = 0.5 * (1.0 + x);
            sum += 0.5 * weight * mu0 * self.directional_hemispherical_albedo(mu0);
        }
        2.0 * sum
    }
}

/// The albedo of a semi-infinite layer of isotropic scatterers of single-scattering albedo `w`
/// for a beam at incidence cosine `mu0`: exactly 1 − √(1 − w) H(μ₀; w), the plane albedo that
/// Chandrasekhar's H-function gives (the isotropic case of the H-equation, which
/// `substance_albedos_the_hemisphere_s_quadrature_is_exact_for_isotropic_grains` holds the IMSA's
/// quadrature to); 1 for w = 1.
#[must_use]
pub fn isotropic_plane_albedo(w: f64, mu0: f64) -> f64 {
    1.0 - (1.0 - w).max(0.0).sqrt() * h_function(w, mu0)
}

/// The single-scattering albedo w* of the isotropic medium that the similarity relation puts in
/// place of grains of albedo `w` and phase-function asymmetry `asymmetry` (⟨cos Θ⟩): w* = w (1 − ξ)
/// ÷ (1 − w ξ), so that 1 − w* = (1 − w) ÷ (1 − w ξ), van de Hulst's a′ (1974, A&A 35, 209, eq.
/// 14, p. 211), which keeps his similarity parameter s = √[(1 − a) ÷ (1 − ag)] (eq. 13); 1 for
/// w = 1.
#[must_use]
pub fn similar_isotropic_albedo(w: f64, asymmetry: f64) -> f64 {
    let similar = w * (1.0 - asymmetry) / (1.0 - w * asymmetry);
    // At w = 1 the quotient is 1 exactly, which rounding can carry past it by an ulp.
    if similar > 1.0 { 1.0 } else { similar }
}

/// The energy albedos of grains of albedo `w` and asymmetry `asymmetry` by the similarity
/// relation: the plane albedo at each of [`HEMISPHERICAL_INCIDENCE_COSINES`], and the spherical
/// albedo 2 ∫₀¹ `A_h`(μ₀) μ₀ dμ₀ by the hemisphere's nodes ([`mu_nodes`]), summed in order and
/// held at most 1.
///
/// The spherical albedo is van de Hulst's similar quantity, within 0.004 of the anisotropic
/// medium's own (each within 0.002 of his z(s): van de Hulst 1974, summary, p. 209; §3, Fig. 1c,
/// p. 212); the plane albedo is not one (§4, p. 213).
/// `substance_albedos_the_similarity_relation_meets_van_de_hulst_s_tables` measures both.
#[must_use]
pub fn energy_albedos(w: f64, asymmetry: f64) -> ([f64; 4], f64) {
    let similar = similar_isotropic_albedo(w, asymmetry);
    let plane = HEMISPHERICAL_INCIDENCE_COSINES.map(|mu0| isotropic_plane_albedo(similar, mu0));
    let sphere = 2.0
        * mu_nodes().into_iter().fold(0.0, |sum, (mu0, weight)| {
            sum + weight * mu0 * isotropic_plane_albedo(similar, mu0)
        });
    // A lossless medium's is 1 exactly, which the nodes' rounding can carry past it by an ulp.
    (plane, sphere.min(1.0))
}

/// One row's optics, as the table holds them: w, `A_N` and the spherical albedo in B, V and R, then
/// `A_h` at each of the incidences in B, V and R.
pub type AlbedoRow = [f64; ALBEDO_ROW_LEN];

/// The optics of grains of index `index` (n, k in B, V and R) in reference form `form`: w by the
/// equivalent slab; `A_N` by the IMSA under the form's photometry ([`Medium`]); the
/// directional-hemispherical and spherical albedos by the similarity relation at the form's
/// phase-function asymmetry ([`energy_albedos`]; decision-p14-t49c-split §1.2).
///
/// # Panics
///
/// If a band's n is below 1, as [`single_scattering_albedo`]; [`row_index`] refuses such an index
/// first.
#[must_use]
pub fn row_optics(form: ReferenceForm, index: [(f64, f64); 3]) -> AlbedoRow {
    let mut row = [0.0; ALBEDO_ROW_LEN];
    let asymmetry = form.photometry().phase_function().asymmetry();
    for (band, &(n, k)) in index.iter().enumerate() {
        let w = single_scattering_albedo(n, k, BAND_WAVELENGTHS_NM[band], form.grain_diameter());
        row[band] = w;
        row[3 + band] = Medium::new(w, form.photometry()).normal_albedo();
        let (plane, sphere) = energy_albedos(w, asymmetry);
        row[6 + band] = sphere;
        for (incidence, a_h) in plane.into_iter().enumerate() {
            row[9 + 3 * incidence + band] = a_h;
        }
    }
    row
}

/// The index (n, k) of `file` at `wavelength_nm`, found by wavelength on the file's own grid: the
/// grid's value where the wavelength is a node, and between two nodes n linear and k linear in
/// ln k (linear in k where either node's k is 0), as R08's reduction put them on the grid; `None`
/// off the grid.
#[must_use]
pub fn index_at(file: &MaterialFile, wavelength_nm: f64) -> Option<(f64, f64)> {
    let (grid, n, k) = (file.wavelengths_nm(), file.n(), file.k());
    let upper = grid.iter().position(|&w| w >= wavelength_nm)?;
    if grid[upper].total_cmp(&wavelength_nm).is_eq() {
        return Some((n[upper], k[upper]));
    }
    let lower = upper.checked_sub(1)?;
    let t = (wavelength_nm - grid[lower]) / (grid[upper] - grid[lower]);
    let n_at = n[lower] + t * (n[upper] - n[lower]);
    let (k0, k1) = (k[lower], k[upper]);
    let k_at = if k0 > 0.0 && k1 > 0.0 {
        math::exp(math::ln(k0) + t * (math::ln(k1) - math::ln(k0)))
    } else {
        k0 + t * (k1 - k0)
    };
    Some((n_at, k_at))
}

/// Why a row's optics could not be computed.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum FitAlbedoError {
    /// The row has no condensed optics, so names no index file.
    #[error("`{key}` has no condensed optics")]
    NoOptics {
        /// The row.
        key: String,
    },
    /// The file given for the row is not a solid file of its key.
    #[error(
        "`{key}` names `{file}`, but the file given is `{found_key}`'s, of phase `{found_phase}`"
    )]
    NotItsFile {
        /// The row.
        key: String,
        /// The file it names.
        file: &'static str,
        /// The given file's key.
        found_key: String,
        /// The given file's phase.
        found_phase: String,
    },
    /// A band lies outside the file's grid.
    #[error("`{file}` does not reach {wavelength_nm} nm")]
    OffGrid {
        /// The file.
        file: &'static str,
        /// The band's wavelength, nm.
        wavelength_nm: f64,
    },
    /// The file's real index at a band is below 1, where the equivalent slab's mean path is not
    /// real.
    #[error("`{file}`'s n at {wavelength_nm} nm is {n}, below 1")]
    IndexBelowUnity {
        /// The file.
        file: &'static str,
        /// The band's wavelength, nm.
        wavelength_nm: f64,
        /// Its n there.
        n: f64,
    },
}

/// The index files of the rows with condensed optics, in registry order: what the task's manifest
/// lists under `materials`.
#[must_use]
pub fn row_files() -> Vec<&'static str> {
    optics::rows()
        .map(|(_, optics)| optics.index_file())
        .collect()
}

/// The index (n, k) in B, V and R of row `id` from its index file `file`.
///
/// # Errors
///
/// [`FitAlbedoError::NoOptics`] for a row without condensed optics,
/// [`FitAlbedoError::NotItsFile`] if `file` is not a solid file of the row's key,
/// [`FitAlbedoError::OffGrid`] if a band lies outside its grid, and
/// [`FitAlbedoError::IndexBelowUnity`] if its n at a band is below 1.
pub fn row_index(id: SubstanceId, file: &MaterialFile) -> Result<[(f64, f64); 3], FitAlbedoError> {
    let row = id.substance();
    let substance_key = row.key();
    let key = substance_key.as_str();
    let named = row
        .condensed_optics()
        .ok_or_else(|| FitAlbedoError::NoOptics {
            key: key.to_owned(),
        })?
        .index_file();
    if file.key() != key || file.phase() != "solid" {
        return Err(FitAlbedoError::NotItsFile {
            key: key.to_owned(),
            file: named,
            found_key: file.key().to_owned(),
            found_phase: file.phase().to_owned(),
        });
    }
    let mut index = [(0.0, 0.0); 3];
    for (band, &wavelength_nm) in BAND_WAVELENGTHS_NM.iter().enumerate() {
        let (n, k) = index_at(file, wavelength_nm).ok_or(FitAlbedoError::OffGrid {
            file: named,
            wavelength_nm,
        })?;
        if n < 1.0 {
            return Err(FitAlbedoError::IndexBelowUnity {
                file: named,
                wavelength_nm,
                n,
            });
        }
        index[band] = (n, k);
    }
    Ok(index)
}

/// Every row with condensed optics, in registry order, and its optics, its index file read from
/// `manifest`'s material directory.
///
/// # Errors
///
/// [`RunTaskError::Input`] if the manifest's `materials` are not [`row_files`], or as
/// [`row_index`]; [`RunTaskError::Dataset`] if a file cannot be read.
pub fn fit(manifest: &Manifest) -> Result<Vec<(SubstanceId, AlbedoRow)>, RunTaskError> {
    let files = row_files();
    if manifest.materials() != files.as_slice() {
        return Err(RunTaskError::Input {
            task: NAME,
            source: format!(
                "the manifest names the materials {:?}, not the optics rows' {files:?}",
                manifest.materials()
            )
            .into(),
        });
    }
    optics::rows()
        .map(|(id, optics)| {
            let file = manifest.load_material(optics.index_file()).map_err(|e| {
                RunTaskError::Dataset(crate::data::LoadDatasetError::Material(Box::new(e)))
            })?;
            let index = row_index(id, &file).map_err(|e| RunTaskError::Input {
                task: NAME,
                source: Box::new(e),
            })?;
            Ok((id, row_optics(optics.reference_form(), index)))
        })
        .collect()
}

/// The table's contents for `rows`.
///
/// # Panics
///
/// If a row has no condensed optics: [`fit`]'s rows all have them.
#[must_use]
pub fn render(rows: Vec<(SubstanceId, AlbedoRow)>) -> RustTable {
    let items = rows
        .into_iter()
        .map(|(id, values)| {
            let row = id.substance();
            let optics = row
                .condensed_optics()
                .expect("a fitted row has condensed optics");
            let form = match optics.reference_form() {
                ReferenceForm::Frost => "fresh snow or fine frost",
                ReferenceForm::Regolith => "a fresh particulate regolith",
            };
            TableItem::Array {
                name: item_name(id),
                doc: vec![
                    format!(
                        "`{}` as {form} of {} µm grains, from `{}`{}: w, `A_N` and the",
                        row.key(),
                        optics.reference_form().grain_diameter().value() * 1.0e6,
                        optics.index_file(),
                        if optics.is_stand_in() {
                            ", a stand-in"
                        } else {
                            ""
                        }
                    ),
                    "spherical albedo in B, V and R, then `A_h` at μ₀ = 1, 0.5, 0.2 and 0.05 in B,"
                        .to_owned(),
                    "V and R.".to_owned(),
                ],
                values: values.to_vec(),
            }
        })
        .collect();
    RustTable {
        summary: vec![
            "The condensed optics of the substance registry's solids (plan 14, P14.T49.c), which"
                .to_owned(),
            "`substance::optics` reads.".to_owned(),
        ],
        notes: vec![
            "Each row is the substance's grains in its reference form by Hapke's model, in B, V and"
                .to_owned(),
            "R (440, 550 and 640 nm): the equivalent slab's single-scattering albedo w from the"
                .to_owned(),
            "index file's n and k; the normal albedo by the reflectance factor with isotropic"
                .to_owned(),
            "multiple scattering, Chandrasekhar's H-function and the reference form's phase"
                .to_owned(),
            "function and opposition effect, without roughness; and the energy albedos by the"
                .to_owned(),
            "similarity relation's isotropic medium. The values are w (3), the normal albedo `A_N`"
                .to_owned(),
            "(3), the spherical albedo (3) and the directional-hemispherical albedo `A_h` at μ₀ ="
                .to_owned(),
            "1, 0.5, 0.2 and 0.05 (3 each), each triple in B, V and R.".to_owned(),
        ],
        items,
    }
}

/// The table's acceptance line for `rows`: the frosts and regoliths, the highest `A_N` and
/// `A_h` with their rows, and every row's `A_N` in V.
///
/// # Errors
///
/// [`RunTaskError::Input`] if there are no rows, or if the highest `A_N` passes R10's particulate
/// ceiling of 1.35 or the highest `A_h` passes 1, so that the line is true whenever it is written.
fn acceptance(rows: &[(SubstanceId, AlbedoRow)]) -> Result<String, RunTaskError> {
    if rows.is_empty() {
        return Err(RunTaskError::Input {
            task: NAME,
            source: "no row has condensed optics".into(),
        });
    }
    let frosts = rows
        .iter()
        .filter(|(id, _)| {
            id.substance()
                .condensed_optics()
                .is_some_and(|o| o.reference_form() == ReferenceForm::Frost)
        })
        .count();
    let (normal_max, normal_key, _) = highest(rows, 3..6);
    let (hemispherical_max, hemispherical_key, hemispherical_offset) =
        highest(rows, 9..ALBEDO_ROW_LEN);
    if normal_max > PARTICULATE_NORMAL_ALBEDO_CEILING || hemispherical_max > 1.0 {
        return Err(RunTaskError::Input {
            task: NAME,
            source: format!(
                "the highest `A_N`, {normal_max} (`{normal_key}`), or the highest `A_h`, \
                 {hemispherical_max} (`{hemispherical_key}`), passes its bound of \
                 {PARTICULATE_NORMAL_ALBEDO_CEILING} or 1"
            )
            .into(),
        });
    }
    let mut normal = String::new();
    for (id, values) in rows {
        // Writing into a `String` cannot fail.
        let _ = write!(
            normal,
            "{}`{}` {:.3}",
            if normal.is_empty() { "" } else { ", " },
            id.substance().key(),
            values[4]
        );
    }
    Ok(format!(
        "{} rows ({frosts} frosts, {} regoliths); the highest `A_N` {normal_max:.4} \
         (`{normal_key}`), the highest `A_h` {hemispherical_max:.4} (`{hemispherical_key}`, {} \
         at μ₀ = {}), both within the bounds of {PARTICULATE_NORMAL_ALBEDO_CEILING} and 1; \
         `A_N` in V: {normal}",
        rows.len(),
        rows.len() - frosts,
        ["B", "V", "R"][(hemispherical_offset - 9) % 3],
        HEMISPHERICAL_INCIDENCE_COSINES[(hemispherical_offset - 9) / 3],
    ))
}

/// The highest of `rows`' values at `offsets`, with its row's key and offset.
#[must_use]
fn highest(
    rows: &[(SubstanceId, AlbedoRow)],
    offsets: core::ops::Range<usize>,
) -> (f64, String, usize) {
    let mut best = (f64::NEG_INFINITY, String::new(), 0);
    for (id, values) in rows {
        for offset in offsets.clone() {
            if values[offset] > best.0 {
                best = (
                    values[offset],
                    id.substance().key().as_str().to_owned(),
                    offset,
                );
            }
        }
    }
    best
}

/// The task: P14.T49.c's condensed optics, fast.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct SubstanceAlbedosTask;

impl FitTask for SubstanceAlbedosTask {
    fn name(&self) -> &'static str {
        NAME
    }

    fn class(&self) -> TaskClass {
        TaskClass::Fast
    }

    fn table_path(&self) -> &'static str {
        "substance_albedo.rs"
    }

    fn revision(&self) -> u32 {
        VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &ITEMS
    }

    /// The sim values the table is made of, so that a changed constant or formula of the sim's
    /// finds the table stale (the index files' data are the inputs hash's, through the manifest's
    /// `materials`):
    ///
    /// - the bands' wavelengths and the hemispherical albedos' incidence cosines;
    /// - each reference form's grain diameter, phase-function parameters and opposition effect, its
    ///   phase function evaluated at cos g = 1, 0 and −1, its asymmetry, and its opposition
    ///   amplitude at w = 0.1 and 0.9;
    /// - each row's reference form, 0 for the frost and 1 for the regolith.
    fn fingerprint(&self) -> SimFingerprint {
        let mut probes = Vec::new();
        for (band, wavelength) in ["B", "V", "R"].into_iter().zip(BAND_WAVELENGTHS_NM) {
            probes.push((format!("{band} wavelength nm"), wavelength));
        }
        for (incidence, mu0) in HEMISPHERICAL_INCIDENCE_COSINES.into_iter().enumerate() {
            probes.push((format!("incidence {incidence} cosine"), mu0));
        }
        for form in [ReferenceForm::Frost, ReferenceForm::Regolith] {
            let label = format!("{form:?}");
            let photometry = form.photometry();
            probes.push((
                format!("{label} grain diameter m"),
                form.grain_diameter().value(),
            ));
            match photometry.phase_function() {
                PhaseFunction::HenyeyGreenstein { asymmetry } => {
                    probes.push((format!("{label} Henyey-Greenstein asymmetry"), asymmetry));
                }
                PhaseFunction::Legendre { b, c } => {
                    probes.push((format!("{label} Legendre b"), b));
                    probes.push((format!("{label} Legendre c"), c));
                }
            }
            if let OppositionEffect::ShadowHiding {
                surface_share,
                width,
            } = photometry.opposition()
            {
                probes.push((format!("{label} S(0)"), surface_share));
                probes.push((format!("{label} h"), width));
            }
            for cos_g in [1.0, 0.0, -1.0] {
                probes.push((
                    format!("{label} p at cos g {cos_g}"),
                    photometry.phase_function().at(cos_g),
                ));
            }
            probes.push((
                format!("{label} asymmetry"),
                photometry.phase_function().asymmetry(),
            ));
            for w in [0.1, 0.9] {
                probes.push((
                    format!("{label} B0 at w {w}"),
                    photometry.opposition_amplitude(w),
                ));
            }
        }
        for (id, optics) in optics::rows() {
            let form = match optics.reference_form() {
                ReferenceForm::Frost => 0.0,
                ReferenceForm::Regolith => 1.0,
            };
            probes.push((format!("{} reference form", id.substance().key()), form));
        }
        SimFingerprint::new(probes)
    }

    fn run(&self, manifest: &Manifest, _threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        check_manifest(self, manifest)?;
        let rows = fit(manifest)?;
        let names: Vec<String> = rows.iter().map(|(id, _)| item_name(*id)).collect();
        if names != ITEMS {
            return Err(RunTaskError::Input {
                task: NAME,
                source: format!(
                    "the registry's optics rows are {names:?}, not the task's {ITEMS:?}"
                )
                .into(),
            });
        }
        let acceptance = acceptance(&rows)?;
        Ok(TaskOutput {
            table: render(rows),
            source: "Hapke's equivalent slab (Hapke 2012's surface-reflection fits, as Davalos \
                     2026 reproduces them) and reflectance factor with isotropic multiple \
                     scattering (Hapke et al. 2012, eqs. 2-4) with Chandrasekhar's H-function \
                     (Kawabata 2016, eqs. 2-3), and the energy albedos by van de Hulst 1974's \
                     similarity relation (eq. 14), over rendering plan R08's committed index \
                     files; \
                     fresh snow's photometry from Domingue, Hartman and Verbiscer 1997 (Table \
                     III) and the lunar regolith's from Helfenstein and Veverka 1987 (Table 1), \
                     with Wiscombe and Warren 1980's and the Lunar Sourcebook's grain sizes, as \
                     `substance::optics` holds them"
                .to_owned(),
            acceptance,
            provisional: None,
        })
    }
}

#[cfg(test)]
mod tests;
