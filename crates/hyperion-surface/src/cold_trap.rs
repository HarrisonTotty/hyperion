//! The cold-trap fraction: the share of the ground at a latitude that stays below a volatile's
//! threshold temperature all year, in the permanent shadows of topography too small for the coarse
//! field to see (plan R09, T7.d; `science-r09-drainage.md` §4 and §8).
//!
//! A cold trap is sub-cell topography, so no zonal rule makes one. Smooth ground at the lunar pole
//! reaches 156 K in equilibrium with the Sun at 1.54° (`science-r09-drainage.md` §4.1), yet ice lies
//! in its shadows (Li et al. 2018, PNAS 115, 8907). This module is the one closed form both sides
//! call. Galaxy plan 14's P14.T51.f integrates [`fraction`] over latitude for a body's cold-trapped
//! area, and R09.T13.f evaluates it per coarse cell to place that area. It is pure and in `f64`,
//! opens no stream and reads no field.
//!
//! # The model
//!
//! It is Hayne, Aharonson and Schörghofer's template (2021, Nat. Astron. 5, 169; read as the
//! preprint 2005.05369). The cold-trap area is the permanently shadowed area times the share of it
//! whose annual maximum temperature lies below the threshold (their eq. 1). Their terrain is bowl
//! craters over a share of the ground, with a log-normal depth-to-diameter ratio d/D, and rough
//! plains between them, a Gaussian surface of directional RMS slope `σ_s`. Here both are
//! populations of spherical bowls:
//!
//! - **Craters** carry the body's own d/D distribution ([`DepthRatios`]).
//! - **Plains** are a labelled stand-in for Hayne et al.'s, which they ray-traced and give no
//!   closed form for (`science-r09-drainage.md` §8.1, §8.10, §8.11 and §8.13). They are bowls whose
//!   d/D is a half-normal of scale [`PLAINS_DEPTH_SCALE`] × tan `σ_s` ÷ 2: 0.6 of half the
//!   magnitude of the surface's directional slope, a normal variable of RMS tan `σ_s`.
//!   - A bowl is a closed hollow, whose shadow stays put as the star circles the sky, while a
//!     Gaussian surface's shadows are cast by ridges and lit from other azimuths. So bowls of the
//!     surface's own mean-square slope keep several times its permanent shadow, and the scale is
//!     fitted to ray-traced surfaces built as Hayne et al.'s Methods D builds them.
//!   - Their instantaneous shadow is then 0.38–0.92 of Smith's for a Gaussian surface (1967,
//!     JGR 72, 4059): no population of bowls matches both shadows, and cold traps need the
//!     permanent one.
//!   - Their shadows also absorb [`PLAINS_SURROUNDINGS`] of S sin e from sunlit terrain beyond
//!     their hollows, which a bowl's sphere omits. At `σ_s` 5.7° this matches the ray traces'
//!     water and CO₂ cold traps; on rougher plains the form still overstates both (R09's Risks
//!     give the figures).
//!   - A log-normal of the same two moments would not do: its heavier tail leaves deep bowls in
//!     shadow equatorward of 70°.
//!   - It puts the plains' water cold-trap optimum near a `σ_s` of 16°, inside Hayne et al.'s
//!     10–20°.
//!
//! For each bowl, of depth ratio γ = d/D, at colatitude e₀ under a star whose greatest
//! declination in the sky is δ and whose angular radius is α:
//!
//! - **Permanent shadow** is ground that no part of the star's disk ever reaches. It is the exact
//!   ray geometry of a spherical bowl, tabulated offline by `hyperion-fit`'s task
//!   `bowl_permanent_shadow` ([`tables::bowl_permanent_shadow`](crate::tables::bowl_permanent_shadow);
//!   R09.T7.f, `science-r09-drainage.md` §8.12 and `decision-r09-t7f.md`). Over a year the
//!   star's disk sweeps every declination within D = δ + α of the equator, so a point is in
//!   permanent shadow when every direction toward the rim has a declination above D. The table is
//!   read trilinearly in ln γ, the colatitude over the rim's slope θ = 2 atan(2γ) and D over θ
//!   ([`bowl_permanent_shadow`]). It is zero once the highest sun clears the rim's slope,
//!   (e₀ + D) ÷ θ ≥ 1, and once D ÷ θ ≥ ½, where Hayne et al.'s polar cap (their eq. 6) closes. A
//!   bowl shallower than the table's 0.02 reads its 0.02 row at its own θ, since small slopes are
//!   similar in these axes, and one deeper than a hemisphere counts as one. The read is within about
//!   0.005 of exact geometry (the table's header gives its figures).
//!   - It replaces Hayne et al.'s eq. 26, `A_perm` ÷ `A_crater` ≈ 1 − 8βe₀ ÷ 3π − 2βδ with
//!     β = 1 ÷ 2γ − 2γ, which they fitted at d/D 0.2, 70–90° and the Moon's 1.54° (Methods A.2.5,
//!     Fig. S3). Eq. 26's declination term removes shallow bowls' shadow away from the pole (a d/D
//!     0.1 bowl at 80° keeps 0.031 by it against an exact 0.224), and it overstates deep bowls
//!     equatorward of 75° (0.47 against 0.27 at d/D 0.3 and 60°).
//! - **It is capped by the bowl's instantaneous shadow** at the upper limb's highest elevation e,
//!   since a permanent shadow lies inside every instantaneous one. Exact geometry never exceeds
//!   it, so the cap trims only the trilinear read's overshoot near the table's edges. That shadow
//!   is the exact one of a spherical bowl (`science-r09-drainage.md` §8.11): Hayne et al.'s eq. 3,
//!   exact on the transect along the sun through the bowl's centre, applied to each parallel
//!   transect at its own depth ratio and integrated across the bowl. Their eq. 5 takes the other
//!   transects as similar to the central one (their eq. 4), but a sphere's off-centre sections are
//!   shallower, so eq. 5 overstates the shadow, by up to about 17% (0.559 against 0.463 at d/D 0.1
//!   and e = 10°).
//! - **Its temperature** is Ingersoll, Svitek and Murray's for a shadowed spherical bowl (1992,
//!   Icarus 100, 40):
//!   σT⁴ = S sin e · f (1 − A) ÷ (1 − Af) · [1 + A(1 − f) ÷ ε], with view factor
//!   f = 4γ² ÷ (1 + 4γ²), flux S, Bond albedo A and emissivity ε. The paper was not reached. The
//!   formula follows from a sphere's property that every element sees every other with a view
//!   factor equal to the other's share of the sphere's area. The cavity's mean radiosity is then
//!   S sin e, and its visible part A S sin e (1 − f) ÷ (1 − Af). A shadowed element absorbs
//!   (1 − A) of the visible light and ε of the infrared it receives from the cavity.
//!   - The elevation e is the star's flux-weighted elevation at its highest, h = e₀ + δ (at most
//!     90°). That is h itself while the whole disk is up (h ≥ α). Below that it is the visible
//!     segment's first moment, to small angles:
//!     [h (α² (π/2 + asin(h/α)) + h √(α² − h²)) + (2/3)(α² − h²)^(3/2)] ÷ (π α²), which is 2α ÷ 3π
//!     at h = 0.
//!   - The internal heat flow q adds q (1 − (1 − ε) f) ÷ (ε (1 − f)) to σT⁴, exactly, by the same
//!     sum with every element given the source q. It sets the shadows' floor, which decides N₂, CO
//!     and CH₄: 24.0 K on flat lunar ground at q = 0.018 W m⁻² and ε = 0.95.
//!   - The temperature rises with γ, so the cold bowls are those below one depth ratio, and the
//!     shallowest bowl with a permanent shadow bounds the population from below.
//! - **Integration over d/D.** Each population's share is a quadrature of the bowl's shadow over
//!   its density, the trapezoid rule on 512 intervals of the population's own ±8σ: in
//!   ln γ for the craters' log-normal and in γ for the plains' half-normal. Bowls no deeper than
//!   tan(max(e₀ + D, 2D) ÷ 2) ÷ 2 keep no shadow. Ratios above 0.5, deeper than a hemisphere,
//!   count as 0.5, the limit of both the table and Ingersoll's sphere.
//!
//! **Lateral conduction** removes the cold traps smaller than a cut-off, about 1 cm near the lunar
//! pole and 10 m at 60° (Hayne et al., Methods C.1). The fraction carries no scale, so the
//! [`Terrain`] states the share of the cold-trapping area under the cut-off, which
//! [`permanent_shadow`] ignores. [`LUNAR_SHARE_UNDER_CUTOFF`] is the Moon's.
//!
//! # The threshold
//!
//! The threshold is a temperature, one per species, so nothing here is water's. P14.T51.f takes it
//! from the substance registry's saturation curve by one rule for every species: the temperature at
//! which Hertz–Knudsen sublimation reaches 144 kg m⁻² Gyr⁻¹, Schorghofer and Williams's rate for
//! water at 110 K (2020, PSJ 1, 54, Table 1). For CO₂ Clausius–Clapeyron puts it at 55.5–58.4 K,
//! and the registry's curve decides (`science-r09-drainage.md` §8.6). Hayne et al.'s figures are
//! for water's 110 K.
//!
//! # Limits
//!
//! - **Against Hayne et al.'s Table 1** (R09's Risks give the figures), at their best-fit terrain
//!   with craters of their distribution B, the permanent shadow is within 15% over 80–90°, 70–80°
//!   and the whole Moon, and the cold traps within 12% over 80–90° and the whole Moon.
//!   - Equilibrium at the highest sun stands in for the annual maximum temperature. It bounds it
//!     from above, yet its cold bowls' largest d/D at 110 K follows Hayne et al.'s no-conduction
//!     curve (their Fig. S8) closely.
//!   - Between 70° and 80° it keeps cold traps in shallow bowls' own shadow, near Diviner's mapped
//!     0.032% (Landis et al. 2022), where Table 1, which multiplies the shadow by the cold share
//!     of all bowls, has 7 × 10⁻⁴%.
//!   - A peak-temperature form also understates the most volatile species' traps: Schorghofer et
//!     al. find 3.5 times the CO₂ area by the time-averaged loss (2021, GRL 48, e2021GL095533).
//! - **Rough plains.** On plains rougher than `σ_s` ≈ 6° the cold traps are overstated, water by up
//!   to about three times at 27° and CO₂ by more, until R09.T7.g's ray-traced plains.
//! - **The flux is at periapsis**, an upper bound when periapsis and solstice differ.
//! - **The star circles the sky each day.** A body rotating synchronously with its star has no
//!   such geometry; there the night side is a zonal matter.
//! - **No atmosphere.** No downwelling radiation and no heat exchange with a gas reach the shadow,
//!   so the form holds on airless bodies and under atmospheres too thin to matter.
//! - **The present obliquity.** It is the only obliquity used, though cold traps record its
//!   history (`science-r09-drainage.md` §4.3, item 7).
//! - **The conduction share is lunar** and independent of latitude. A cut-off driven by
//!   conductivity waits for a model of the shadows' scales.

use crate::num;
use crate::tables::bowl_permanent_shadow as table;
use hyperion_base::math;
use hyperion_base::units::consts::STEFAN_BOLTZMANN;
use hyperion_base::units::{Kelvin, Radians, WattsPerSquareMetre};
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI, SQRT_2};

/// The Moon's share of the cold-trapping area under lateral conduction's cut-off, 0.05 ± 0.02,
/// labelled lunar: the reference value of [`TerrainParts::share_under_cutoff`]
/// (`science-r09-drainage.md` §8.3).
///
/// It is derived from Hayne et al. 2021 (Nat. Astron. 5, 169):
/// - permanent shadow in patches smaller than 100 m is about 10% of the whole (Conclusions);
/// - below `L_break` ≈ 100 m the shadowed area is constant per logarithmic bin of size (Methods C,
///   eqs. 30–31), down to the ~100 µm grains, about 1.7% a decade;
/// - conduction "eliminates cold traps with sizes ranging from ∼1 cm near the pole, to ∼10 m at
///   60° latitude" (Methods C.1). That removes about 3% of the area, below 1 cm, and partly warms
///   the traps up to about 1 m: those under 1 m are about 700 km², 2% of the total
///   ("Temperatures"),
///   against about 4.5% by extrapolating the 1–100 m trend.
///
/// The figure is for the Moon and for water. Another body's depends on its conductivity and on the
/// scales its shadows lie at.
pub const LUNAR_SHARE_UNDER_CUTOFF: f64 = 0.05;

/// The largest depth-to-diameter ratio a spherical bowl has: a hemisphere, where β = 0.
const HEMISPHERE: f64 = 0.5;

/// Bisection steps for the cold bowls' largest depth ratio.
///
/// Each halves an interval of the view factor that starts 0.5 wide, so 64 steps resolve it to
/// 2⁻⁶⁵, below the spacing of `f64` for any d/D above about 0.0055.
const BISECTION_STEPS: u32 = 64;

/// The colatitude plus D = δ + α, over a bowl's rim slope, at and beyond which it keeps no
/// permanent shadow: the highest sun clears the steepest part of its wall, and lights every point
/// (`decision-r09-t7f.md` §1, claim (i), proved).
const NOON_LIMIT: f64 = 1.0;

/// D = δ + α over a bowl's rim slope at and beyond which it keeps no permanent shadow: Hayne et
/// al.'s polar cap, eq. 6, closes there, and off the pole the shadow closes sooner. On the vertical
/// north–south section through any floor point, a circle of rim slope θ′ ≤ θr, the two rim points
/// are seen at elevations (θ′ ± ψ′) ÷ 2, so the lesser of their declinations is at most
/// θ′ ÷ 2 ≤ θr ÷ 2 (`decision-r09-t7f.md` §1, claim (ii), and R09.T7.f's science check).
const POLAR_LIMIT: f64 = 0.5;

/// The plains' bowls' depth-ratio scale, as a share of half the surface's directional slope,
/// tan `σ_s` ÷ 2: 0.6, fitted to the permanent shadow of ray-traced Gaussian surfaces
/// (`science-r09-drainage.md` §8.13).
///
/// A bowl is a closed hollow, whose shadow stays put as the star circles the sky, while a Gaussian
/// surface's shadows are cast by ridges and lit from other azimuths. So bowls of the surface's own
/// mean-square slope keep 3.5 times its permanent shadow over 80–90° at `σ_s` 5.7°, and 12 times
/// over 70–80°, against surfaces built as Hayne et al.'s Methods D builds them (Hurst exponent
/// 0.9, a point Sun 1.54° from the equator). At 0.6 they keep 0.87–1.09 of it over 80–90° at
/// `σ_s` 5.7–26.6°, and 1.00–1.17 with the Sun 4° from the equator.
const PLAINS_DEPTH_SCALE: f64 = 0.6;

/// The light and infrared that sunlit terrain beyond a plains hollow sends into its shadow, as a
/// share of the flux on level ground at the star's highest flux-weighted elevation, S sin e:
/// 0.0035, fitted to ray-traced Gaussian surfaces at `σ_s` 5.7° (`science-r09-drainage.md` §8.13).
///
/// A bowl's shadow sees only its own walls; a rough surface's sees sunlit slopes beyond its hollow
/// too, a median 1.3–2.7% of S sin e on the ray-traced surfaces over 80–90°. Against water's 110 K
/// that light is negligible, but CO₂'s 55–57 K allow about 0.6 W m⁻² in all, and without it the
/// plains trap 2.7–4.7 times the ray traces' CO₂. It is absorbed, so it enters σT⁴ divided by the
/// emissivity.
const PLAINS_SURROUNDINGS: f64 = 0.0035;

/// The 16-point Gauss–Legendre rule on [−1, 1]: its positive nodes and their weights.
///
/// The rule is symmetric about 0 (Abramowitz and Stegun 1964, Table 25.4; recomputed to 40 digits
/// by Newton's method on P₁₆, and rounded to the nearest `f64`).
const GAUSS_LEGENDRE_16: [(f64, f64); 8] = [
    (0.095_012_509_837_637_44, 0.189_450_610_455_068_5),
    (0.281_603_550_779_258_9, 0.182_603_415_044_923_58),
    (0.458_016_777_657_227_37, 0.169_156_519_395_002_54),
    (0.617_876_244_402_643_8, 0.149_595_988_816_576_74),
    (0.755_404_408_355_003, 0.124_628_971_255_533_88),
    (0.865_631_202_387_831_8, 0.095_158_511_682_492_79),
    (0.944_575_023_073_232_6, 0.062_253_523_938_647_894),
    (0.989_400_934_991_649_9, 0.027_152_459_411_754_096),
];

/// Intervals of the trapezoid rule over a population's window.
///
/// The nodes are the population's alone, fixed whatever the latitude, the star or the cold
/// limit, so that each function is monotone in its inputs to the last bit: every node's term is,
/// and the cold limit only decides how far the sum runs. The bowl's shadow is piecewise smooth in
/// the depth ratio, with a kink at each of the table's cells it crosses, which the rule resolves
/// as the square of its step.
const INTERVALS: u32 = 512;

/// Standard deviations either side of a population's centre over which its bowls are integrated:
/// a normal distribution's mass beyond 8σ is 1.2 × 10⁻¹⁵.
const SPREAD_WIDTHS: f64 = 8.0;

/// ∫ₐᵇ f by [`GAUSS_LEGENDRE_16`], summing the nodes in their fixed order.
#[must_use]
fn gauss_legendre(a: f64, b: f64, f: impl Fn(f64) -> f64) -> f64 {
    // `f64::midpoint` of operands this size is (a + b) ÷ 2, two of the four operators.
    let (mid, half) = (f64::midpoint(a, b), 0.5 * (b - a));
    let mut sum = 0.0;
    for (x, w) in GAUSS_LEGENDRE_16 {
        sum += w * (f(mid - half * x) + f(mid + half * x));
    }
    half * sum
}

/// β = 1 ÷ 2γ − 2γ of a spherical bowl of depth ratio γ > 0 (Hayne et al. 2021, Methods A).
#[must_use]
fn beta_of(gamma: f64) -> f64 {
    0.5 / gamma - 2.0 * gamma
}

/// A log-normal distribution of bowls' depth-to-diameter ratios d/D, by its mean and variance in
/// linear space (Hayne et al. 2021, Methods C.1, eqs. 33–35).
///
/// Hayne et al.'s "σ" is read as the variance, a labelled reading of the paper
/// (`science-r09-drainage.md` §8.1): in their eq. 35, s² = ln(1 + σ ÷ µ²) is the log-normal's
/// log-variance only if σ is the variance, and as standard deviations their values would make
/// near-delta distributions, which cannot fit Mahanti et al.'s craters. Their distribution A,
/// (µ, σ) = (0.14, 1.6 × 10⁻³), is a standard deviation of 0.04. Their eq. 33 is printed with the
/// exponent −(ln γ − m) ÷ 2s², unsquared; the standard log-normal's −(ln γ − m)² ÷ 2s² is meant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DepthRatios {
    /// The mean d/D, in (0, 0.5].
    mean: f64,
    /// The variance of d/D, finite and positive.
    variance: f64,
}

/// Why [`DepthRatios::new`] refused its values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildDepthRatiosError {
    /// The mean is not in (0, 0.5], where a spherical bowl is at most a hemisphere, or is so
    /// small that its square is not a normal `f64`.
    Mean(f64),
    /// The variance is not finite and positive, or gives the log-normal no finite spread beside the
    /// mean: so small that the spread rounds to zero, or so large that the moment of 1 ÷ γ,
    /// e^(−m + s²/2) = (1 + variance ÷ mean²) ÷ mean, is not finite.
    Variance(f64),
}

impl std::fmt::Display for BuildDepthRatiosError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Mean(m) => write!(
                f,
                "mean depth ratio {m} is not in (0, 0.5] or has no normal square"
            ),
            Self::Variance(v) => write!(
                f,
                "depth ratio variance {v} gives the log-normal no finite spread beside its mean"
            ),
        }
    }
}

impl std::error::Error for BuildDepthRatiosError {}

impl DepthRatios {
    /// Hayne et al.'s distribution A: mean 0.14, standard deviation 0.04 (variance 1.6 × 10⁻³).
    ///
    /// It is their fit to Mahanti et al.'s LROC shapes of lunar craters 10–100 m across (Hayne et
    /// al. 2021, "Temperatures").
    pub const HAYNE_A: Self = Self {
        mean: 0.14,
        variance: 1.6e-3,
    };

    /// Hayne et al.'s distribution B: mean 0.076, variance 2.3 × 10⁻⁴ (standard deviation about
    /// 0.015).
    ///
    /// It stands for larger, shallower lunar craters (Hayne et al. 2021, "Temperatures").
    pub const HAYNE_B: Self = Self {
        mean: 0.076,
        variance: 2.3e-4,
    };

    /// The distribution of mean `mean` and variance `variance`, both of d/D itself.
    ///
    /// # Errors
    ///
    /// - [`BuildDepthRatiosError::Mean`] if the mean is not in (0, 0.5], or so small that its
    ///   square is not a normal `f64`.
    /// - [`BuildDepthRatiosError::Variance`] if the variance is not finite and positive, so small
    ///   beside the mean's square that the log-normal's spread rounds to zero, or so large that
    ///   the log-normal's moment of 1 ÷ γ, e^(−m + s²/2) = (1 + variance ÷ mean²) ÷ mean, is not
    ///   finite.
    pub fn new(mean: f64, variance: f64) -> Result<Self, BuildDepthRatiosError> {
        if !(mean > 0.0 && mean <= HEMISPHERE && (mean * mean).is_normal()) {
            return Err(BuildDepthRatiosError::Mean(mean));
        }
        if !(variance.is_finite() && variance > 0.0) {
            return Err(BuildDepthRatiosError::Variance(variance));
        }
        let depths = Self { mean, variance };
        let dist = depths.log_normal();
        let inverse_moment = math::exp(-dist.m + 0.5 * dist.s * dist.s);
        if !(dist.s.is_finite() && dist.s > 0.0 && inverse_moment.is_finite()) {
            return Err(BuildDepthRatiosError::Variance(variance));
        }
        Ok(depths)
    }

    /// The mean d/D.
    #[must_use]
    pub const fn mean(self) -> f64 {
        self.mean
    }

    /// The variance of d/D.
    #[must_use]
    pub const fn variance(self) -> f64 {
        self.variance
    }

    /// The standard deviation of d/D.
    #[must_use]
    pub fn std_dev(self) -> f64 {
        self.variance.sqrt()
    }

    /// The distribution's parameters in log space (Hayne et al. 2021, eqs. 34 and 35).
    #[must_use]
    fn log_normal(self) -> LogNormal {
        let spread = 1.0 + self.variance / (self.mean * self.mean);
        let log_spread = math::ln(spread);
        LogNormal {
            m: math::ln(self.mean) - 0.5 * log_spread,
            s: log_spread.sqrt(),
        }
    }
}

/// The parts of a [`Body`], each in the unit its type states.
///
/// Plain data with public fields: [`Body::new`] validates them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BodyParts {
    /// The star's irradiance at normal incidence at the body's periapsis, W m⁻²; finite and
    /// positive. Ingersoll et al.'s S: about 1,361 W m⁻² for the Moon.
    pub flux: WattsPerSquareMetre,
    /// The star's greatest declination in the body's sky δ, radians in [0, π/2]. With the star's
    /// angular radius α it bounds the band of declinations, |dec| ≤ δ + α, that the star's disk
    /// sweeps over a year, and the star's highest elevation at colatitude e₀ is min(90°, e₀ + δ).
    ///
    /// The caller computes it. For a body of obliquity ε to its seasonal orbit it is
    /// min(ε, 180° − ε), so that a retrograde rotator such as Venus (177°) has 3°. For a moon it is
    /// the obliquity to its planet's orbit: 1.54° for the Moon.
    pub greatest_declination: Radians,
    /// The star's angular radius α seen from the body at periapsis, radians in [0, π/2): 0.27° for
    /// the Sun at 1 au, 0.87° at Mercury's perihelion (695,700 km over 46.0 × 10⁶ km). Zero treats
    /// the star as a point, as Hayne et al. did ("the Sun a point source", Methods D).
    pub star_radius: Radians,
    /// The surface's Bond albedo, in [0, 1): Hayne et al. take 0.12 for the Moon.
    pub bond_albedo: f64,
    /// The surface's thermal emissivity, in (0, 1]: Hayne et al. take 0.95.
    pub emissivity: f64,
    /// The internal heat flow through the surface, W m⁻²; finite and non-negative.
    ///
    /// It is of order 0.02 W m⁻² on the Moon, from the Apollo 15 and 17 heat-flow experiments
    /// (Langseth, Keihm and Peters 1976, Proc. Lunar Sci. Conf. 7, 3143–3171; not reached, and
    /// quoted as 21 and 14–16 mW m⁻² at the two sites).
    pub heat_flow: WattsPerSquareMetre,
}

/// The body-wide inputs of the cold-trap fraction: the star's flux, path and size in the body's
/// sky, and the surface's albedo, emissivity and heat flow.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Body {
    parts: BodyParts,
}

/// Why [`Body::new`] refused its parts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildBodyError {
    /// The flux is not finite and positive.
    Flux(f64),
    /// The greatest declination is not in [0, π/2], or is not zero and has no normal square.
    GreatestDeclination(f64),
    /// The star's angular radius is not in [0, π/2), or is not zero and has no normal square.
    StarRadius(f64),
    /// The Bond albedo is not in [0, 1).
    BondAlbedo(f64),
    /// The emissivity is not in (0, 1], or is subnormal.
    Emissivity(f64),
    /// The heat flow is not finite and non-negative.
    HeatFlow(f64),
}

impl std::fmt::Display for BuildBodyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Flux(x) => write!(f, "flux {x} W m⁻² is not finite and positive"),
            Self::GreatestDeclination(x) => write!(
                f,
                "greatest declination {x} rad is not in [0, π/2] or has no normal square"
            ),
            Self::StarRadius(x) => write!(
                f,
                "star's angular radius {x} rad is not in [0, π/2) or has no normal square"
            ),
            Self::BondAlbedo(x) => write!(f, "bond albedo {x} is not in [0, 1)"),
            Self::Emissivity(x) => write!(f, "emissivity {x} is not in (0, 1] or is subnormal"),
            Self::HeatFlow(x) => {
                write!(f, "heat flow {x} W m⁻² is not finite and non-negative")
            }
        }
    }
}

impl std::error::Error for BuildBodyError {}

impl Body {
    /// The body of `parts`.
    ///
    /// # Errors
    ///
    /// [`BuildBodyError`] naming the first part out of its range:
    /// - `Flux` if the flux is not finite and positive;
    /// - `GreatestDeclination` if the greatest declination is not in [0, π/2], or is not zero and
    ///   so small that its square is not a normal `f64`;
    /// - `StarRadius` if the star's angular radius is not in [0, π/2), or is not zero and so small
    ///   that its square is not a normal `f64`;
    /// - `BondAlbedo` if the Bond albedo is not in [0, 1);
    /// - `Emissivity` if the emissivity is not in (0, 1], or is below `f64::MIN_POSITIVE`;
    /// - `HeatFlow` if the heat flow is not finite and non-negative.
    pub fn new(parts: BodyParts) -> Result<Self, BuildBodyError> {
        let flux = parts.flux.value();
        if !(flux.is_finite() && flux > 0.0) {
            return Err(BuildBodyError::Flux(flux));
        }
        let declination = parts.greatest_declination.value();
        if !(0.0..=FRAC_PI_2).contains(&declination)
            || (declination > 0.0 && !(declination * declination).is_normal())
        {
            return Err(BuildBodyError::GreatestDeclination(declination));
        }
        let radius = parts.star_radius.value();
        if !(0.0..FRAC_PI_2).contains(&radius) || (radius > 0.0 && !(radius * radius).is_normal()) {
            return Err(BuildBodyError::StarRadius(radius));
        }
        if !(0.0..1.0).contains(&parts.bond_albedo) {
            return Err(BuildBodyError::BondAlbedo(parts.bond_albedo));
        }
        if !(parts.emissivity >= f64::MIN_POSITIVE && parts.emissivity <= 1.0) {
            return Err(BuildBodyError::Emissivity(parts.emissivity));
        }
        let heat_flow = parts.heat_flow.value();
        if !(heat_flow.is_finite() && heat_flow >= 0.0) {
            return Err(BuildBodyError::HeatFlow(heat_flow));
        }
        Ok(Self { parts })
    }

    /// The validated parts.
    #[must_use]
    pub const fn parts(&self) -> &BodyParts {
        &self.parts
    }

    /// σT⁴ of a shadowed bowl with view factor `f` under a star at an elevation of sine
    /// `sin_elevation`, W m⁻²: Ingersoll et al.'s balance with the heat flow's term.
    #[must_use]
    fn shadow_power_w_per_m2(&self, f: f64, sin_elevation: f64) -> f64 {
        let p = &self.parts;
        let a = p.bond_albedo;
        let eps = p.emissivity;
        let sunlit = p.flux.value() * sin_elevation * f * (1.0 - a) / (1.0 - a * f)
            * (1.0 + a * (1.0 - f) / eps);
        let internal = p.heat_flow.value() / eps * (1.0 - (1.0 - eps) * f) / (1.0 - f);
        sunlit + internal
    }

    /// The largest depth ratio of the bowls whose shadows stay below `threshold` with the star at
    /// an elevation of sine `sin_elevation`, when each shadow also absorbs `surroundings` × S sin e
    /// from terrain beyond its bowl: 0 when none does, and 0.5 when every bowl does.
    #[must_use]
    fn coldest_limit(&self, sin_elevation: f64, threshold: Kelvin, surroundings: f64) -> f64 {
        let t = threshold.value();
        let p = &self.parts;
        // Absorbed, so it enters σT⁴ divided by the emissivity, as the heat flow does.
        let beyond = surroundings * p.flux.value() * sin_elevation / p.emissivity;
        let target = STEFAN_BOLTZMANN * t * t * t * t - beyond;
        if self.shadow_power_w_per_m2(0.0, sin_elevation) >= target {
            return 0.0;
        }
        let hemisphere = view_factor(HEMISPHERE);
        if self.shadow_power_w_per_m2(hemisphere, sin_elevation) < target {
            return HEMISPHERE;
        }
        // The power rises with f, so bisect for where it meets the target, in a fixed number of
        // steps, so that no convergence test decides the result's bits.
        let (mut lo, mut hi) = (0.0, hemisphere);
        for _ in 0..BISECTION_STEPS {
            let mid = f64::midpoint(lo, hi);
            if self.shadow_power_w_per_m2(mid, sin_elevation) < target {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        depth_ratio_of_view_factor(lo)
    }
}

/// The parts of a [`Terrain`].
///
/// Plain data with public fields: [`Terrain::new`] validates them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerrainParts {
    /// The share of the ground in bowl craters, in [0, 1]; the rest is plains.
    ///
    /// Hayne et al.'s best fit for the Moon is 0.2, within 0.2–0.5.
    pub crater_fraction: f64,
    /// The craters' depth-to-diameter ratios.
    pub crater_depths: DepthRatios,
    /// The plains' directional RMS slope `σ_s` at the cold traps' scales, radians in [0, π/4].
    ///
    /// It is the RMS of the slope along one direction, as Smith (1967) and Hayne et al. define it
    /// ("a Gaussian surface of normal directional slope distribution"). Hayne et al.'s best fit to
    /// the Moon's LROC-NAC shadows is 5.7°, an RMS slope of 0.1, and their comparison with
    /// Diviner's ~250 m temperatures gives 5–10°.
    pub plains_slope: Radians,
    /// The share of the cold-trapping area in shadows smaller than lateral conduction's cut-off,
    /// which hold no cold trap, in [0, 1]. It scales [`fraction`] and leaves [`permanent_shadow`]
    /// alone. [`LUNAR_SHARE_UNDER_CUTOFF`] is the Moon's.
    pub share_under_cutoff: f64,
}

/// The sub-cell topography that casts the permanent shadows: bowl craters over part of the ground,
/// rough plains over the rest, and the share of their shadows too small to stay cold.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Terrain {
    parts: TerrainParts,
}

/// Why [`Terrain::new`] refused its parts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildTerrainError {
    /// The crater fraction is not in [0, 1].
    CraterFraction(f64),
    /// The plains' RMS slope is not in [0, π/4].
    PlainsSlope(f64),
    /// The share under the cut-off is not in [0, 1].
    ShareUnderCutoff(f64),
}

impl std::fmt::Display for BuildTerrainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CraterFraction(x) => write!(f, "crater fraction {x} is not in [0, 1]"),
            Self::PlainsSlope(x) => write!(f, "plains' RMS slope {x} rad is not in [0, π/4]"),
            Self::ShareUnderCutoff(x) => {
                write!(f, "share under the conduction cut-off {x} is not in [0, 1]")
            }
        }
    }
}

impl std::error::Error for BuildTerrainError {}

impl Terrain {
    /// The terrain of `parts`.
    ///
    /// # Errors
    ///
    /// [`BuildTerrainError`] naming the first part out of its range:
    /// - `CraterFraction` if the crater fraction is not in [0, 1];
    /// - `PlainsSlope` if the plains' RMS slope is not in [0, π/4];
    /// - `ShareUnderCutoff` if the share under the conduction cut-off is not in [0, 1].
    pub fn new(parts: TerrainParts) -> Result<Self, BuildTerrainError> {
        if !(0.0..=1.0).contains(&parts.crater_fraction) {
            return Err(BuildTerrainError::CraterFraction(parts.crater_fraction));
        }
        let slope = parts.plains_slope.value();
        if !(0.0..=FRAC_PI_4).contains(&slope) {
            return Err(BuildTerrainError::PlainsSlope(slope));
        }
        if !(0.0..=1.0).contains(&parts.share_under_cutoff) {
            return Err(BuildTerrainError::ShareUnderCutoff(
                parts.share_under_cutoff,
            ));
        }
        Ok(Self { parts })
    }

    /// The validated parts.
    #[must_use]
    pub const fn parts(&self) -> &TerrainParts {
        &self.parts
    }

    /// The share of the ground in permanent shadow in craters shallower than `crater_limit` and
    /// plains' bowls shallower than `plains_limit`, the craters' share first and then the plains',
    /// in that fixed order.
    #[must_use]
    fn shadowed(&self, sky: &Sky, crater_limit: f64, plains_limit: f64) -> f64 {
        let craters = self.parts.crater_fraction;
        let plains = 1.0 - craters;
        let mut share = 0.0;
        if craters > 0.0 {
            let population = Population::LogNormal(self.parts.crater_depths.log_normal());
            share += craters * sky.band.shadowed(population, crater_limit);
        }
        if plains > 0.0
            && let Some(population) = Population::of_plains(self.parts.plains_slope)
        {
            share += plains * sky.band.shadowed(population, plains_limit);
        }
        share
    }
}

/// The star's path at one latitude, as the bowls' shadows and temperatures need it.
#[derive(Debug, Clone, Copy)]
struct Sky {
    /// sin e of the star's flux-weighted elevation at its highest, for the shadows' temperature.
    sin_highest: f64,
    /// The band of declinations the star's disk sweeps, for the shadows' extent.
    band: Band,
}

impl Sky {
    /// The star's path at `latitude` above `body`.
    ///
    /// # Panics
    ///
    /// If the latitude is not in [−π/2, π/2].
    #[must_use]
    fn at(latitude: Radians, body: &Body) -> Self {
        let lat = latitude.value();
        assert!(
            (-FRAC_PI_2..=FRAC_PI_2).contains(&lat),
            "a latitude must be in [-π/2, π/2], got {lat} rad"
        );
        let colatitude = FRAC_PI_2 - lat.abs();
        let declination = body.parts.greatest_declination.value();
        let radius = body.parts.star_radius;
        let highest = Radians::new(num::min(FRAC_PI_2, colatitude + declination));
        Self {
            sin_highest: math::sin(flux_weighted_elevation(highest, radius).value()),
            band: Band::new(colatitude, declination + radius.value()),
        }
    }
}

/// The band of declinations within D = δ + α of the equator that the star's disk sweeps over a
/// year, seen at one colatitude: what a bowl's permanent shadow depends on.
#[derive(Debug, Clone, Copy)]
struct Band {
    /// The colatitude e₀, radians in [0, π/2].
    colatitude: f64,
    /// D = δ + α, radians in [0, π).
    limb: f64,
    /// cos² e of the upper limb at its highest elevation e = min(90°, e₀ + D).
    cos2: f64,
    /// sin e cos e ÷ 2 of the upper limb at its highest elevation e.
    half_sin_cos: f64,
}

impl Band {
    /// The band of D = `limb` radians at `colatitude` radians.
    #[must_use]
    fn new(colatitude: f64, limb: f64) -> Self {
        let (sin_l, cos_l) = math::sin_cos(num::min(FRAC_PI_2, colatitude + limb));
        Self {
            colatitude,
            limb,
            cos2: cos_l * cos_l,
            half_sin_cos: 0.5 * sin_l * cos_l,
        }
    }

    /// The depth ratio at and below which a bowl keeps no permanent shadow: its rim slope
    /// 2 atan(2γ) is at most the greater of e₀ + D ([`NOON_LIMIT`]) and 2D ([`POLAR_LIMIT`]). It is
    /// at least 0.5 when no bowl keeps any.
    #[must_use]
    fn shallowest(&self) -> f64 {
        let slope = num::max(
            (self.colatitude + self.limb) / NOON_LIMIT,
            self.limb / POLAR_LIMIT,
        );
        if slope >= FRAC_PI_2 {
            return HEMISPHERE;
        }
        0.5 * math::tan(0.5 * slope)
    }

    /// A spherical bowl's permanent shadow, a share of its opening: the table's trilinear read,
    /// capped by the instantaneous shadow at the upper limb's highest elevation. A ratio above 0.5
    /// counts as a hemisphere.
    #[must_use]
    fn bowl(&self, gamma: f64) -> f64 {
        let read = self.read(gamma);
        if read <= 0.0 {
            return 0.0;
        }
        num::min(read, self.instantaneous(num::min(gamma, HEMISPHERE)))
    }

    /// The table's trilinear read for a bowl of depth ratio `gamma` > 0, without the cap (the
    /// module documentation gives the rules).
    #[must_use]
    fn read(&self, gamma: f64) -> f64 {
        let ratio = num::min(gamma, HEMISPHERE);
        let slope = 2.0 * math::atan(2.0 * ratio);
        // A zero angle is zero over any slope, so that the flat limit γ → 0 at a pole the star
        // never leaves reads the first row's 1.
        let over = |angle: f64| if angle > 0.0 { angle / slope } else { 0.0 };
        let (colatitude, limb) = (over(self.colatitude), over(self.limb));
        if colatitude + limb >= NOON_LIMIT || limb >= POLAR_LIMIT {
            return 0.0;
        }
        let (depth_cell, depth_at) = cell(&table::LN_DEPTH_RATIO, math::ln(ratio));
        let (colatitude_cell, colatitude_at) = cell(&table::COLATITUDE_PER_RIM_SLOPE, colatitude);
        let (limb_cell, limb_at) = cell(&table::LIMB_PER_RIM_SLOPE, limb);
        let width = table::COLATITUDE_PER_RIM_SLOPE.len();
        let mut share = 0.0;
        for (depth_step, depth_weight) in [(0, 1.0 - depth_at), (1, depth_at)] {
            for (colatitude_step, colatitude_weight) in
                [(0, 1.0 - colatitude_at), (1, colatitude_at)]
            {
                let row = &table::SHADOW
                    [(depth_cell + depth_step) * width + colatitude_cell + colatitude_step];
                share += depth_weight
                    * colatitude_weight
                    * ((1.0 - limb_at) * row[limb_cell] + limb_at * row[limb_cell + 1]);
            }
        }
        share
    }

    /// The share of a population of bowls that lies in permanent shadow, counting only the bowls
    /// shallower than `cold_limit` (0.5 counts every bowl).
    #[must_use]
    fn shadowed(&self, dist: Population, cold_limit: f64) -> f64 {
        let shallowest = self.shallowest();
        if shallowest >= HEMISPHERE || cold_limit <= shallowest {
            return 0.0;
        }
        let mut share = dist.cumulative(num::min(cold_limit, HEMISPHERE), |gamma| self.bowl(gamma));
        // Bowls deeper than a hemisphere count as hemispheres, if they are cold.
        if cold_limit >= HEMISPHERE {
            share += self.bowl(HEMISPHERE) * dist.mass(HEMISPHERE, f64::INFINITY);
        }
        share
    }

    /// The instantaneous shadow of a spherical bowl of depth ratio `gamma` in (0, 0.5], under the
    /// star's upper limb at its highest elevation, as a share of the bowl's opening.
    ///
    /// It is `science-r09-drainage.md` §8.11's exact form. Each vertical section of the bowl along
    /// the sun is a circular arc of the sphere's centre height, so Hayne et al.'s eq. 3 holds for
    /// it at its own depth ratio `γ_y`. With the rim's radius 1, the sphere's radius is
    /// R = (1 + d²) ÷ 2d for a depth d = 2γ, and its centre h = R − d above the rim's plane. The
    /// section at offset y has radius ρ = √(R² − y²), depth ρ − h and half chord √(1 − y²), so
    /// `γ_y` = ½ √((ρ − h) ÷ (ρ + h)), which falls to 0 at the rim. The shadow is
    /// (4 ÷ π) ∫ √(1 − y²) (cos² e − `β_y` sin e cos e ÷ 2) dy over the offsets whose section is
    /// shadowed, from 0 to the y* where `γ_y` falls to the depth ratio whose β is 2 cot e. It is
    /// taken in y = sin θ, so that the integrand stays smooth to the rim, by the 16-point
    /// Gauss–Legendre rule. It matches an 801 × 801 ray grid to 10⁻⁴; the tests hold 0.005 on a
    /// coarser one.
    #[must_use]
    fn instantaneous(&self, gamma: f64) -> f64 {
        if gamma >= HEMISPHERE || self.half_sin_cos <= 0.0 {
            // A hemisphere's sections are all semicircles; with the sun on the horizon the whole
            // bowl is in shadow, cos² e = 1.
            return self.cos2;
        }
        let threshold = depth_ratio_of_beta(self.cos2 / self.half_sin_cos);
        if gamma <= threshold {
            return 0.0;
        }
        let depth = 2.0 * gamma;
        let sphere = (1.0 + depth * depth) / (2.0 * depth);
        let centre = sphere - depth;
        // R − ρ* for the section of depth ratio `threshold`, written so that it keeps its digits
        // for a shallow bowl: (4γ² − 4γ*²) ÷ (d (1 − 4γ*²)), since 2R − d = 1 ÷ d.
        let edge = 4.0 * threshold * threshold;
        let gap = 4.0 * (gamma - threshold) * (gamma + threshold) / (depth * (1.0 - edge));
        let reach = (gap * (2.0 * sphere - gap)).sqrt();
        let theta_max = math::asin(num::min(1.0, reach));
        let integral = gauss_legendre(0.0, theta_max, |theta| {
            let (offset, cos_theta) = math::sin_cos(theta);
            let rho = (sphere * sphere - offset * offset).sqrt();
            let section_depth = depth - offset * offset / (sphere + rho);
            let gamma_y = 0.5 * (section_depth / (rho + centre)).sqrt();
            cos_theta * cos_theta * (self.cos2 - beta_of(gamma_y) * self.half_sin_cos)
        });
        4.0 / PI * integral
    }
}

/// The cell of the ascending `nodes` that holds `x`, and `x`'s place in it in [0, 1], clamped to
/// the end cells: below the first node the first cell's start, above the last the last cell's end.
#[must_use]
fn cell(nodes: &[f64], x: f64) -> (usize, f64) {
    let last = nodes.len() - 1;
    if x <= nodes[0] {
        return (0, 0.0);
    }
    if x >= nodes[last] {
        return (last - 1, 1.0);
    }
    let i = nodes.partition_point(|&node| node <= x) - 1;
    (i, (x - nodes[i]) / (nodes[i + 1] - nodes[i]))
}

/// A population of bowls, by the distribution of their depth ratios.
#[derive(Debug, Clone, Copy)]
enum Population {
    /// Craters, log-normal (Hayne et al. 2021, eqs. 33–35).
    LogNormal(LogNormal),
    /// Rough plains: 0.6 of half the magnitude of a Gaussian surface's directional slope.
    HalfNormal(HalfNormal),
}

impl Population {
    /// The bowls standing for plains of directional RMS slope `slope`, or `None` for flat plains
    /// (`science-r09-drainage.md` §8.10 and §8.13).
    ///
    /// d/D = 0.6 |s| ÷ 2 for a slope s normal with RMS w = tan `σ_s`: a half-normal of scale 0.3 w,
    /// [`PLAINS_DEPTH_SCALE`]'s fit to ray-traced surfaces (§8.13). A log-normal of the same
    /// moments has a far heavier tail (P(d/D > 0.2) 50 times the half-normal's at `σ_s` = 5.7°
    /// without the scale), which would leave deep bowls in shadow equatorward of 70°. A slope under
    /// 10⁻¹⁰⁰ rad counts as flat, so that the scale's square stays a normal number.
    #[must_use]
    fn of_plains(slope: Radians) -> Option<Self> {
        let w = math::tan(slope.value());
        if w < 1e-100 {
            return None;
        }
        Some(Self::HalfNormal(HalfNormal {
            scale: PLAINS_DEPTH_SCALE * 0.5 * w,
        }))
    }

    /// The probability of a depth ratio in (a, b), 0 ≤ a, b ≤ ∞.
    #[must_use]
    fn mass(self, a: f64, b: f64) -> f64 {
        match self {
            Self::LogNormal(dist) => dist.mass(a, b),
            Self::HalfNormal(dist) => dist.mass(a, b),
        }
    }

    /// The population's window, in the variable in which it is normal: ln γ over ±8σ for the
    /// log-normal, γ from 0 to 8σ for the half-normal, either at most 0.5's.
    #[must_use]
    fn window(self) -> (f64, f64) {
        match self {
            Self::LogNormal(dist) => (
                dist.m - SPREAD_WIDTHS * dist.s,
                num::min(math::ln(HEMISPHERE), dist.m + SPREAD_WIDTHS * dist.s),
            ),
            Self::HalfNormal(dist) => (0.0, num::min(HEMISPHERE, SPREAD_WIDTHS * dist.scale)),
        }
    }

    /// The depth ratio at `x` of the window's variable.
    #[must_use]
    fn ratio(self, x: f64) -> f64 {
        match self {
            Self::LogNormal(_) => math::exp(x),
            Self::HalfNormal(_) => x,
        }
    }

    /// The window's variable at the depth ratio `gamma` > 0.
    #[must_use]
    fn variable(self, gamma: f64) -> f64 {
        match self {
            Self::LogNormal(_) => math::ln(gamma),
            Self::HalfNormal(_) => gamma,
        }
    }

    /// The population's density in the window's variable at `x`.
    #[must_use]
    fn density(self, x: f64) -> f64 {
        let root_two_pi = (2.0 * PI).sqrt();
        match self {
            Self::LogNormal(dist) => {
                let z = (x - dist.m) / dist.s;
                math::exp(-0.5 * z * z) / (dist.s * root_two_pi)
            }
            Self::HalfNormal(dist) => {
                2.0 / (dist.scale * root_two_pi) * math::exp(-dist.exponent(x))
            }
        }
    }

    /// ∫ g(γ) P(γ) dγ over the bowls of the window shallower than `top` ≤ 0.5, for a g ≥ 0.
    ///
    /// It takes the trapezoid rule on [`INTERVALS`] equal intervals of the window, summed in their
    /// fixed order, and stops at `top` under the chord of its interval's two ends. The nodes do not
    /// depend on `top` or on g, so the sum rises with `top` and with g at every node, to the last
    /// bit.
    #[must_use]
    fn cumulative(self, top: f64, g: impl Fn(f64) -> f64) -> f64 {
        let (lo, hi) = self.window();
        if top <= 0.0 || hi <= lo {
            return 0.0;
        }
        let end = self.variable(top);
        if end <= lo {
            return 0.0;
        }
        let step = (hi - lo) / f64::from(INTERVALS);
        let value = |x: f64| g(self.ratio(x)) * self.density(x);
        let mut left = value(lo);
        let mut sum = 0.0;
        for i in 0..INTERVALS {
            let x0 = lo + step * f64::from(i);
            let x1 = if i + 1 == INTERVALS {
                hi
            } else {
                lo + step * f64::from(i + 1)
            };
            let right = value(x1);
            if end < x1 {
                // The chord's area from x0 to the end: never more than the whole interval's.
                let u = (end - x0) / (x1 - x0);
                sum += 0.5 * (end - x0) * (left + (left + (right - left) * u));
                return sum;
            }
            sum += 0.5 * (x1 - x0) * (left + right);
            left = right;
        }
        sum
    }
}

/// A half-normal distribution: |X| for X normal with mean 0 and standard deviation q = `scale`.
///
/// Its density is √(2/π) ÷ q · e^(−γ² ÷ 2q²) for γ ≥ 0.
#[derive(Debug, Clone, Copy)]
struct HalfNormal {
    scale: f64,
}

impl HalfNormal {
    /// γ² ÷ 2q², the exponent of the density at γ = `gamma`.
    #[must_use]
    fn exponent(self, gamma: f64) -> f64 {
        let z = gamma / self.scale;
        0.5 * z * z
    }

    /// The probability of a ratio in (a, b): erfc(a ÷ q√2) − erfc(b ÷ q√2).
    #[must_use]
    fn mass(self, a: f64, b: f64) -> f64 {
        if b <= a {
            return 0.0;
        }
        let r = 1.0 / (self.scale * SQRT_2);
        math::erfc(a * r) - math::erfc(b * r)
    }
}

/// A log-normal distribution by its log-space mean `m` and log-space standard deviation `s`.
#[derive(Debug, Clone, Copy)]
struct LogNormal {
    m: f64,
    s: f64,
}

impl LogNormal {
    /// The standard score of `x` ≥ 0: −∞ at 0 and +∞ at +∞, as `libm`'s logarithm gives them.
    #[must_use]
    fn score(self, x: f64) -> f64 {
        (math::ln(x) - self.m) / self.s
    }

    /// The probability of a ratio in (a, b).
    #[must_use]
    fn mass(self, a: f64, b: f64) -> f64 {
        normal_between(self.score(a), self.score(b))
    }
}

/// Φ(b) − Φ(a) for the standard normal distribution function Φ, from the tail nearer each, so
/// that a small difference of two values near 0 or 1 keeps its digits.
#[must_use]
fn normal_between(a: f64, b: f64) -> f64 {
    let r = 1.0 / SQRT_2;
    if b <= a {
        0.0
    } else if a >= 0.0 {
        // Both in the upper half: 1 − Φ(z) = erfc(z/√2) ÷ 2.
        0.5 * (math::erfc(a * r) - math::erfc(b * r))
    } else if b <= 0.0 {
        // Both in the lower half: Φ(z) = erfc(−z/√2) ÷ 2.
        0.5 * (math::erfc(-b * r) - math::erfc(-a * r))
    } else {
        1.0 - f64::midpoint(math::erfc(-a * r), math::erfc(b * r))
    }
}

/// The flux-weighted elevation of a star of angular radius `radius` whose centre is at `centre` ≥ 0
/// (`science-r09-drainage.md` §8.4).
///
/// A whole disk's is its centre's. A disk the horizon cuts gives the first moment of its visible
/// segment over the whole disk's area, to small angles:
/// [h (α² (π/2 + asin(h/α)) + h √(α² − h²)) + (2/3)(α² − h²)^(3/2)] ÷ (π α²), with h the centre's
/// elevation and α the radius. It is 2α ÷ 3π with the centre on the horizon and h when the disk
/// clears it.
#[must_use]
fn flux_weighted_elevation(centre: Radians, radius: Radians) -> Radians {
    let (h, alpha) = (centre.value(), radius.value());
    if h >= alpha {
        return centre;
    }
    let a2 = alpha * alpha;
    let rest = a2 - h * h;
    let root = rest.sqrt();
    let area_above = a2 * (FRAC_PI_2 + math::asin(h / alpha)) + h * root;
    Radians::new((h * area_above + 2.0 / 3.0 * rest * root) / (PI * a2))
}

/// The view factor of a spherical bowl of depth ratio γ seen from inside it, 4γ² ÷ (1 + 4γ²): the
/// bowl's share of its sphere's area (Ingersoll et al. 1992).
#[must_use]
fn view_factor(gamma: f64) -> f64 {
    let g2 = 4.0 * gamma * gamma;
    g2 / (1.0 + g2)
}

/// The depth ratio of a spherical bowl of view factor `f`, √(f ÷ 4(1 − f)).
#[must_use]
fn depth_ratio_of_view_factor(f: f64) -> f64 {
    (f / (4.0 * (1.0 - f))).sqrt()
}

/// The depth ratio γ whose β = 1 ÷ 2γ − 2γ is `beta` ≥ 0: the root of 4γ² + 2βγ − 1 = 0, written
/// 1 ÷ (√(β² + 4) + β) so that it keeps its digits for large β and gives 0 at β = ∞.
#[must_use]
fn depth_ratio_of_beta(beta: f64) -> f64 {
    1.0 / ((beta * beta + 4.0).sqrt() + beta)
}

/// The temperature of the shadowed part of a spherical bowl of depth ratio `depth_ratio`, with the
/// star at `elevation` above `body`'s horizon (Ingersoll, Svitek and Murray 1992, Icarus 100, 40,
/// with the heat flow's term; see the module documentation).
///
/// The shadow's temperature is the same everywhere in it, a property of the sphere.
///
/// # Panics
///
/// If the depth ratio is not in (0, 0.5] or the elevation not in [0, π/2].
#[must_use]
pub fn shadow_temperature(depth_ratio: f64, elevation: Radians, body: &Body) -> Kelvin {
    assert!(
        depth_ratio > 0.0 && depth_ratio <= HEMISPHERE,
        "a bowl's depth ratio must be in (0, 0.5], got {depth_ratio}"
    );
    let e = elevation.value();
    assert!(
        (0.0..=FRAC_PI_2).contains(&e),
        "a star's elevation must be in [0, π/2], got {e} rad"
    );
    let power = body.shadow_power_w_per_m2(view_factor(depth_ratio), math::sin(e));
    Kelvin::new((power / STEFAN_BOLTZMANN).sqrt().sqrt())
}

/// The permanent shadow of one spherical bowl of depth ratio `depth_ratio`, as a share of its
/// opening in [0, 1], at `colatitude` under a star whose disk reaches `limb` = δ + α from the
/// celestial equator: its greatest declination plus its angular radius.
///
/// It is the exact ray geometry's table read trilinearly, capped by the instantaneous shadow at the
/// upper limb's highest elevation (the module documentation gives the rules). It is zero once
/// colatitude + `limb` reaches the rim's slope 2 atan(2 d/D), or `limb` half of it, does not rise
/// with `limb`, and counts a ratio above 0.5 as a hemisphere.
///
/// # Panics
///
/// If the depth ratio is not finite and positive, the colatitude not in [0, π/2], or `limb` not
/// in [0, π).
#[must_use]
pub fn bowl_permanent_shadow(depth_ratio: f64, colatitude: Radians, limb: Radians) -> f64 {
    assert!(
        depth_ratio.is_finite() && depth_ratio > 0.0,
        "a bowl's depth ratio must be finite and positive, got {depth_ratio}"
    );
    let (e0, d) = (colatitude.value(), limb.value());
    assert!(
        (0.0..=FRAC_PI_2).contains(&e0),
        "a colatitude must be in [0, π/2], got {e0} rad"
    );
    assert!(
        (0.0..PI).contains(&d),
        "a star's declination and radius must sum to [0, π), got {d} rad"
    );
    num::max(0.0, num::min(1.0, Band::new(e0, d).bowl(depth_ratio)))
}

/// The share of the ground at `latitude` in permanent shadow, in [0, 1], whatever its temperature.
///
/// It is the ground that no part of the star's disk ever reaches, Hayne et al.'s permanently
/// shadowed regions at every scale, with no conduction share. It does not rise with the greatest
/// declination or the star's angular radius.
///
/// # Panics
///
/// If the latitude is not in [−π/2, π/2].
#[must_use]
pub fn permanent_shadow(latitude: Radians, body: &Body, terrain: &Terrain) -> f64 {
    let sky = Sky::at(latitude, body);
    num::max(
        0.0,
        num::min(1.0, terrain.shadowed(&sky, HEMISPHERE, HEMISPHERE)),
    )
}

/// The cold-trap fraction: the share of the ground at `latitude`, in [0, 1], that stays below
/// `threshold` all year (the module documentation gives the model).
///
/// It is the part of [`permanent_shadow`] whose temperature at the star's highest elevation lies
/// below the threshold, times one less the share under lateral conduction's cut-off. It does not
/// rise with the greatest declination, the star's angular radius, the flux, the heat flow or the
/// conduction share, and does not fall as the threshold rises.
///
/// # Panics
///
/// If the latitude is not in [−π/2, π/2] or the threshold is not finite and positive.
#[must_use]
pub fn fraction(latitude: Radians, body: &Body, terrain: &Terrain, threshold: Kelvin) -> f64 {
    let t = threshold.value();
    assert!(
        t.is_finite() && t > 0.0,
        "a threshold must be finite and positive, got {t} K"
    );
    let sky = Sky::at(latitude, body);
    let crater_limit = body.coldest_limit(sky.sin_highest, threshold, 0.0);
    if crater_limit <= 0.0 {
        return 0.0;
    }
    // The plains' shadows also see sunlit terrain beyond their hollows, so are never colder.
    let plains_limit = body.coldest_limit(sky.sin_highest, threshold, PLAINS_SURROUNDINGS);
    let share = terrain.shadowed(&sky, crater_limit, plains_limit)
        * (1.0 - terrain.parts.share_under_cutoff);
    num::max(0.0, num::min(1.0, share))
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyperion_base::units::Degrees;
    use hyperion_testkit::float::assert_same_bits;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    const WATER: Kelvin = Kelvin::new(110.0);

    fn degrees(x: f64) -> Radians {
        Radians::from(Degrees::new(x))
    }

    /// The Moon as Hayne et al. and the ruling's recomputation take it: 1,361 W m⁻², the Sun at
    /// most 1.54° from the equator and a point (Hayne et al.'s Methods D), albedo 0.12, emissivity
    /// 0.95 and no heat flow.
    fn moon_parts() -> BodyParts {
        BodyParts {
            flux: WattsPerSquareMetre::new(1361.0),
            greatest_declination: degrees(1.54),
            star_radius: Radians::ZERO,
            bond_albedo: 0.12,
            emissivity: 0.95,
            heat_flow: WattsPerSquareMetre::ZERO,
        }
    }

    fn moon() -> Body {
        Body::new(moon_parts()).unwrap()
    }

    /// Hayne et al.'s best-fit lunar terrain: 20% craters with `depths`, plains at a directional
    /// RMS slope of 0.1 (5.7°), and no conduction share.
    fn best_fit_parts(depths: DepthRatios) -> TerrainParts {
        TerrainParts {
            crater_fraction: 0.2,
            crater_depths: depths,
            plains_slope: Radians::new(math::atan(0.1)),
            share_under_cutoff: 0.0,
        }
    }

    fn best_fit(depths: DepthRatios) -> Terrain {
        Terrain::new(best_fit_parts(depths)).unwrap()
    }

    /// The mean of `f` over latitudes `lo`..`hi` degrees, weighted by area, by the midpoint rule
    /// in 1,000 steps.
    fn band_mean(lo: f64, hi: f64, f: impl Fn(Radians) -> f64) -> f64 {
        let steps = 1000_u32;
        let width = (hi - lo) / f64::from(steps);
        let (mut sum, mut weight) = (0.0, 0.0);
        for i in 0..steps {
            let lat = degrees(lo + width * (f64::from(i) + 0.5));
            let w = math::cos(lat.value());
            sum += w * f(lat);
            weight += w;
        }
        sum / weight
    }

    /// Whether `value` is within a factor of two of `reference`.
    fn within_two(value: f64, reference: f64) -> bool {
        (reference / 2.0..=reference * 2.0).contains(&value)
    }

    #[test]
    fn ingersoll_puts_a_lunar_polar_bowl_at_97_k() {
        // The ruling's recomputation (science-r09-drainage.md §4.1): at the pole, with the Sun at
        // 1.54°, 97, 83 and 70 K at d/D 0.2, 0.14 and 0.1.
        let body = moon();
        for (gamma, kelvin) in [(0.2, 97.0), (0.14, 83.0), (0.1, 70.0)] {
            let t = shadow_temperature(gamma, degrees(1.54), &body).value();
            assert!(
                (t - kelvin).abs() <= 1.0,
                "d/D {gamma}: {t} K, not {kelvin}"
            );
        }
    }

    #[test]
    fn the_heat_flow_alone_gives_flat_lunar_ground_24_k() {
        // §8.4: q ÷ ε on flat ground, 24.0 K at 0.018 W m⁻² and ε 0.95, and 24.9 K in a d/D 0.2
        // bowl.
        let body = Body::new(BodyParts {
            heat_flow: WattsPerSquareMetre::new(0.018),
            ..moon_parts()
        })
        .unwrap();
        let flat = shadow_temperature(1e-9, Radians::ZERO, &body).value();
        assert!((flat - 24.0).abs() <= 0.1, "flat ground {flat} K");
        let bowl = shadow_temperature(0.2, Radians::ZERO, &body).value();
        assert!((bowl - 24.9).abs() <= 0.1, "d/D 0.2 {bowl} K");
        // A threshold below the floor traps nothing, even at a pole that never sees the Sun; one
        // above it traps every shadow there.
        let dark = Body::new(BodyParts {
            greatest_declination: Radians::ZERO,
            ..*body.parts()
        })
        .unwrap();
        let terrain = best_fit(DepthRatios::HAYNE_B);
        assert_same_bits(
            fraction(degrees(90.0), &dark, &terrain, Kelvin::new(20.0)),
            0.0,
        );
        let cold = fraction(degrees(90.0), &dark, &terrain, Kelvin::new(40.0));
        let shadow = permanent_shadow(degrees(90.0), &dark, &terrain);
        assert!(
            (cold - shadow).abs() < 1e-12 && shadow > 0.99,
            "{cold}, {shadow}"
        );
    }

    #[test]
    fn the_shadow_temperature_rises_with_depth_and_sun() {
        let body = moon();
        let mut last = 0.0;
        for i in 1..=50_u32 {
            let gamma = 0.01 * f64::from(i);
            let t = shadow_temperature(gamma, degrees(5.0), &body).value();
            assert!(t > last, "d/D {gamma}: {t} K after {last} K");
            last = t;
        }
        let low = shadow_temperature(0.1, degrees(2.0), &body).value();
        let high = shadow_temperature(0.1, degrees(20.0), &body).value();
        assert!(high > low);
    }

    #[test]
    fn a_disk_the_horizon_cuts_weighs_its_visible_segment() {
        let alpha = degrees(0.87).value();
        let weighted =
            |h: f64| flux_weighted_elevation(Radians::new(h), Radians::new(alpha)).value();
        // Its centre on the horizon, the upper half: 2α ÷ 3π.
        let half = weighted(0.0);
        assert!((half - 2.0 * alpha / (3.0 * PI)).abs() < 1e-15, "{half}");
        // Continuous where the disk clears the horizon: with the centre just under α, the hidden
        // sliver moves it by far less than the centre's own distance from α. Rising with the
        // centre, and never below it, since the visible segment lies above it.
        let centre = alpha * (1.0 - 1e-9);
        let just_below = weighted(centre);
        assert!(
            (just_below - centre).abs() < 1e-12 * alpha,
            "{just_below} at {centre}"
        );
        let mut last = 0.0;
        for i in 0..=100_u32 {
            let h = alpha * 0.01 * f64::from(i);
            let e = weighted(h);
            assert!(e >= last, "{i}: {e} after {last}");
            assert!(e >= h, "{i}: {e} below its centre {h}");
            last = e;
        }
    }

    /// The plains alone at a directional RMS slope of `w`: no craters and no conduction share.
    fn plains(w: f64) -> Terrain {
        Terrain::new(TerrainParts {
            crater_fraction: 0.0,
            plains_slope: Radians::new(math::atan(w)),
            ..best_fit_parts(DepthRatios::HAYNE_B)
        })
        .unwrap()
    }

    /// The Moon under the real Sun: its flux and angular radius at perihelion (0.98329 au), and
    /// the heat flow 0.018 W m⁻².
    fn real_moon() -> Body {
        Body::new(BodyParts {
            flux: WattsPerSquareMetre::new(1361.0 / (0.98329 * 0.98329)),
            star_radius: degrees(0.2711),
            heat_flow: WattsPerSquareMetre::new(0.018),
            ..moon_parts()
        })
        .unwrap()
    }

    #[test]
    fn haynes_best_fit_terrain_gives_table_1() {
        // Hayne et al. 2021, Table 1, the mean of both hemispheres, with a point Sun (Methods D)
        // and no conduction share; the bounds are science-r09-drainage.md §8.13's.
        let body = moon();
        for depths in [DepthRatios::HAYNE_B, DepthRatios::HAYNE_A] {
            let terrain = best_fit(depths);
            let cold = |lat: Radians| fraction(lat, &body, &terrain, WATER);
            let polar = band_mean(80.0, 90.0, cold);
            assert!(
                within_two(polar, 0.067),
                "{depths:?}: cold traps over 80–90° {polar}"
            );
            let whole = band_mean(0.0, 90.0, cold);
            assert!(
                within_two(whole, 0.0010),
                "{depths:?}: cold traps overall {whole}"
            );
            // From half of Landis et al. 2022's Diviner-mapped 553 km² over 60–80°, 0.032% of
            // 70–80°, to twice that taken to all scales (×2.2: Hayne et al.'s 40,000 km² against
            // Diviner's 18,300). Table 1's 7 × 10⁻⁴% is its construction's (§8.13).
            let sub = band_mean(70.0, 80.0, cold);
            assert!(
                (1.6e-4..=1.4e-3).contains(&sub),
                "{depths:?}: cold traps over 70–80° {sub}"
            );
        }
        // Permanent shadow with distribution B, whose mean d/D is nearer Table 1's d/D 0.1.
        let terrain = best_fit(DepthRatios::HAYNE_B);
        let shadow = |lat: Radians| permanent_shadow(lat, &body, &terrain);
        for (lo, hi, table) in [
            (80.0, 90.0, 0.085),
            (70.0, 80.0, 0.005),
            (0.0, 90.0, 0.0015),
        ] {
            let band = band_mean(lo, hi, shadow);
            assert!(within_two(band, table), "shadow over {lo}–{hi}° {band}");
        }
        let sixties = band_mean(60.0, 70.0, shadow);
        assert!(sixties <= 1e-4, "shadow over 60–70° {sixties}");
        let fifties = band_mean(50.0, 60.0, shadow);
        assert!(fifties <= 1e-5, "shadow over 50–60° {fifties}");
    }

    #[test]
    fn the_plains_keep_a_ray_traced_surfaces_permanent_shadow() {
        // science-r09-drainage.md §8.13: Gaussian surfaces built as Hayne et al.'s Methods D
        // builds them (Hurst exponent 0.9), ray traced under a point Sun 1.54° from the equator.
        let body = moon();
        for (w, polar, sub) in [(0.1, 0.028, 8e-4), (0.3, 0.236, 0.078), (0.5, 0.348, 0.188)] {
            let terrain = plains(w);
            let shadow = |lat: Radians| permanent_shadow(lat, &body, &terrain);
            let p = band_mean(80.0, 90.0, shadow);
            assert!(
                (p / polar - 1.0).abs() <= 0.2,
                "w {w}: 80–90° {p} against {polar}"
            );
            let s = band_mean(70.0, 80.0, shadow);
            assert!(within_two(s, sub), "w {w}: 70–80° {s} against {sub}");
        }
        // The ray traces keep no measurable shadow equatorward of 70° at σ_s 5.7°.
        let terrain = plains(0.1);
        let sixties = band_mean(60.0, 70.0, |lat| permanent_shadow(lat, &body, &terrain));
        assert!(sixties <= 1e-5, "60–70° {sixties}");
    }

    #[test]
    fn the_plains_instantaneous_shadow_falls_short_of_smiths() {
        // The recorded cost of matching the permanent shadow (science-r09-drainage.md §8.13):
        // 0.38–0.92 of Smith's at 1–12°, wherever his is at least 5%.
        for w in [0.1, 0.3, 0.5] {
            let plains = Population::of_plains(Radians::new(math::atan(w))).unwrap();
            for half_degrees in 2..=24_u32 {
                let e = degrees(0.5 * f64::from(half_degrees)).value();
                let smith = smith_shadow(w, e);
                if smith >= 0.05 {
                    let ratio = bowls_shadow(plains, e) / smith;
                    assert!(
                        (0.35..=1.0).contains(&ratio),
                        "w {w}, {half_degrees}: {ratio}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_plains_cold_traps_follow_the_ray_traces() {
        // science-r09-drainage.md §8.13: at σ_s 5.7°, under the real Sun, the ray-traced surfaces
        // put 2.16% of 80–90° below 110 K and 0.090–0.146% below 57 K.
        let body = real_moon();
        let terrain = plains(0.1);
        let water = band_mean(80.0, 90.0, |lat| fraction(lat, &body, &terrain, WATER));
        assert!((water / 0.0216 - 1.0).abs() <= 0.15, "water {water}");
        let co2 = band_mean(80.0, 90.0, |lat| {
            fraction(lat, &body, &terrain, Kelvin::new(57.0))
        });
        assert!(within_two(co2, 0.0012), "CO₂ {co2}");
    }

    #[test]
    fn ceres_holds_schorghofers_permanent_shadow_between_b_and_a() {
        // Schorghofer et al. 2024 (PSJ 5, 99): 2,282 km² at the present tilt and "essentially
        // zero" at 20°, here with Hayne et al.'s lunar terrain at Ceres's perihelion (2.5577 au,
        // the Sun 0.1042° in radius) and Ceres's mean radius of 469.7 km.
        let area_km2 = 4.0 * PI * 469.7 * 469.7;
        let ceres = |tilt: f64| {
            Body::new(BodyParts {
                flux: WattsPerSquareMetre::new(1361.0 / (2.5577 * 2.5577)),
                greatest_declination: degrees(tilt),
                star_radius: degrees(0.1042),
                ..moon_parts()
            })
            .unwrap()
        };
        let shadow_km2 = |body: &Body, depths: DepthRatios| {
            let terrain = best_fit(depths);
            area_km2 * band_mean(0.0, 90.0, |lat| permanent_shadow(lat, body, &terrain))
        };
        let (b, a) = (
            shadow_km2(&ceres(4.0), DepthRatios::HAYNE_B),
            shadow_km2(&ceres(4.0), DepthRatios::HAYNE_A),
        );
        assert!(b <= 2282.0 && 2282.0 <= a, "B {b} km², A {a} km²");
        for depths in [DepthRatios::HAYNE_B, DepthRatios::HAYNE_A] {
            let tilted = shadow_km2(&ceres(20.0), depths);
            assert!(tilted < 68.0, "{depths:?} at 20°: {tilted} km²");
        }
    }

    #[test]
    fn the_moons_co2_cold_traps_lie_within_schorghofers_bracket() {
        // science-r09-drainage.md §8.13 (e): half the Moon's area below 52.9 K and 55.6 K, the
        // temperatures of 10 kg m⁻² Gyr⁻¹ by Clausius–Clapeyron at 25.2 kJ mol⁻¹ and on Bryson et
        // al. 1974's curve, lies within 30–408 km². That is half of Schorghofer et al. 2021's
        // peak-rate 59 km² to twice their time-averaged 204 km², over 80–90°S (GRL 48,
        // e2021GL095533, Table 1).
        let moon_km2 = 4.0 * PI * 1737.4 * 1737.4;
        let body = real_moon();
        for depths in [DepthRatios::HAYNE_B, DepthRatios::HAYNE_A] {
            let terrain = Terrain::new(TerrainParts {
                share_under_cutoff: LUNAR_SHARE_UNDER_CUTOFF,
                ..best_fit_parts(depths)
            })
            .unwrap();
            for t in [52.9, 55.6] {
                let cold = |lat: Radians| fraction(lat, &body, &terrain, Kelvin::new(t));
                let south = 0.5 * moon_km2 * band_mean(0.0, 90.0, cold);
                assert!(
                    (30.0..=408.0).contains(&south),
                    "{depths:?} at {t} K: {south} km²"
                );
            }
        }
    }

    /// A terrain of one bowl of depth ratio `gamma`: every crater alike, and no plains.
    fn one_bowl(gamma: f64) -> Terrain {
        Terrain::new(TerrainParts {
            crater_fraction: 1.0,
            crater_depths: DepthRatios::new(gamma, 1e-10).unwrap(),
            ..best_fit_parts(DepthRatios::HAYNE_B)
        })
        .unwrap()
    }

    #[test]
    fn the_cap_trims_only_the_reads_overshoot() {
        // T7.d's cap stays (decision-r09-t7f.md §1): exact geometry never exceeds the
        // instantaneous shadow at the upper limb's highest elevation, so the read passes it by
        // its overshoot alone. A d/D 0.45 bowl at the equator, under a Sun that passes overhead,
        // keeps none.
        let body = moon();
        assert_same_bits(permanent_shadow(degrees(0.0), &body, &one_bowl(0.45)), 0.0);
        let mut worst = 0.0_f64;
        for lat in [10.0, 30.0, 45.0, 60.0, 70.0, 80.0, 85.0, 88.0, 89.5, 90.0] {
            let band = Sky::at(degrees(lat), &body).band;
            for i in 0..=60_u32 {
                let gamma = 0.02 * math::exp(f64::from(i) * math::ln(25.0) / 60.0);
                let over = band.read(gamma) - band.instantaneous(num::min(gamma, HEMISPHERE));
                worst = num::max(worst, over);
                assert!(
                    band.bowl(gamma) <= band.instantaneous(num::min(gamma, HEMISPHERE)),
                    "{lat}°, d/D {gamma}"
                );
            }
        }
        assert!(worst <= 0.005, "the read passes the cap by {worst}");
    }

    /// Hayne et al.'s eq. 6 as printed (Methods A.2.1): x₀″² with x₀″ = cos 2δ − (β ÷ 2) sin 2δ,
    /// held at zero once x₀″ ≤ 0. A copy independent of the fitting crate's.
    fn hayne_eq6(gamma: f64, limb: f64) -> f64 {
        let (s, c) = math::sin_cos(2.0 * limb);
        let edge = c - 0.5 * beta_of(gamma) * s;
        if edge > 0.0 { edge * edge } else { 0.0 }
    }

    #[test]
    fn the_tables_pole_row_is_eq_6() {
        // At its nodes, to 10⁻¹², and so, to first order in δ, eq. 8's 1 − 2βδ.
        let width = table::COLATITUDE_PER_RIM_SLOPE.len();
        assert_same_bits(table::COLATITUDE_PER_RIM_SLOPE[0], 0.0);
        for (i, &t) in table::LN_DEPTH_RATIO.iter().enumerate() {
            let gamma = math::exp(t);
            let slope = 2.0 * math::atan(2.0 * gamma);
            let row = &table::SHADOW[i * width];
            for (k, &b) in table::LIMB_PER_RIM_SLOPE.iter().enumerate() {
                let limb = b * slope;
                let exact = hayne_eq6(gamma, limb);
                assert!(
                    (row[k] - exact).abs() <= 1e-12,
                    "d/D {gamma}, δ {limb}: {} against eq. 6's {exact}",
                    row[k]
                );
                // Eq. 8, to first order: x₀″ = 1 − βδ − 2δ² + O(δ³), so the gap is
                // (β² − 4) δ² and higher.
                let beta = beta_of(gamma);
                if beta * limb < 0.2 && limb < 0.2 {
                    let gap = (row[k] - (1.0 - 2.0 * beta * limb)).abs();
                    let second = (beta * beta + 4.0) * limb * limb;
                    assert!(
                        gap <= 1.5 * second + 1e-15,
                        "{gamma}, {limb}: {gap} against {second}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_read_matches_exact_geometry() {
        // decision-r09-t7f.md §3 (a)'s exact values, to 0.005.
        for (gamma, lat, limb, exact) in [
            (0.1, 80.0, 1.54, 0.2240),
            (0.3, 60.0, 1.54, 0.2735),
            (0.5, 45.0, 1.54, 0.3217),
            (0.3, 75.0, 4.0, 0.5375),
            (0.5, 60.0, 20.0, 0.1987),
        ] {
            let read = bowl_permanent_shadow(gamma, degrees(90.0 - lat), degrees(limb));
            assert!(
                (read - exact).abs() <= 0.005,
                "d/D {gamma} at {lat}°, D {limb}°: {read} against {exact}"
            );
        }
        // Eq. 26, which it replaces, keeps 0.031 of the d/D 0.1 bowl at 80°.
        let e0 = degrees(10.0).value();
        let eq26 =
            1.0 - 8.0 * beta_of(0.1) * e0 / (3.0 * PI) - 2.0 * beta_of(0.1) * degrees(1.54).value();
        assert!((eq26 - 0.031).abs() < 5e-4, "eq. 26: {eq26}");
    }

    #[test]
    fn the_read_is_zero_at_both_edges() {
        // The table's own last nodes are the two limits.
        assert_same_bits(
            table::COLATITUDE_PER_RIM_SLOPE[table::COLATITUDE_PER_RIM_SLOPE.len() - 1],
            NOON_LIMIT,
        );
        assert_same_bits(
            table::LIMB_PER_RIM_SLOPE[table::LIMB_PER_RIM_SLOPE.len() - 1],
            POLAR_LIMIT,
        );
        for gamma in [0.005, 0.02, 0.1, 0.3, 0.5] {
            let slope = 2.0 * math::atan(2.0 * gamma);
            for a in [0.0, 0.1, 0.3, 0.5] {
                let edges = [(a * slope, (1.0 - a) * slope), (a * slope, 0.5 * slope)];
                for (colatitude, limb) in edges {
                    let read =
                        bowl_permanent_shadow(gamma, Radians::new(colatitude), Radians::new(limb));
                    assert!(
                        read <= 1e-12,
                        "d/D {gamma}, e₀ {colatitude}, D {limb}: {read}"
                    );
                }
            }
            // Just inside the polar edge some shadow is left.
            let inside = bowl_permanent_shadow(gamma, Radians::ZERO, Radians::new(0.45 * slope));
            assert!(inside > 0.0, "d/D {gamma}: {inside}");
        }
        // Deeper than a hemisphere reads as one; nothing past the equator or under a band of 45°.
        let deep = bowl_permanent_shadow(0.8, degrees(10.0), degrees(1.54));
        assert_same_bits(
            deep,
            bowl_permanent_shadow(0.5, degrees(10.0), degrees(1.54)),
        );
        assert_same_bits(
            bowl_permanent_shadow(0.5, degrees(90.0), Radians::ZERO),
            0.0,
        );
        assert_same_bits(
            bowl_permanent_shadow(0.5, Radians::ZERO, degrees(45.0)),
            0.0,
        );
    }

    #[test]
    fn the_trapezoid_rule_follows_each_populations_mass() {
        // The rule with a unit integrand is the population's mass up to `top`, to 5 × 10⁻⁵, and never
        // falls as `top` rises, whether `top` falls on a node or inside an interval.
        let populations = [
            Population::LogNormal(DepthRatios::HAYNE_A.log_normal()),
            Population::LogNormal(DepthRatios::HAYNE_B.log_normal()),
            Population::of_plains(Radians::new(math::atan(0.1))).unwrap(),
            Population::of_plains(Radians::new(math::atan(0.5))).unwrap(),
        ];
        for dist in populations {
            let mut last = 0.0;
            for i in 1..=997_u32 {
                let top = HEMISPHERE * f64::from(i) / 997.0;
                let sum = dist.cumulative(top, |_| 1.0);
                assert!(sum >= last, "{dist:?} at {top}: {sum} after {last}");
                last = sum;
                let mass = dist.mass(0.0, top);
                // Within the rule's h² f′ ÷ 12 where it stops.
                assert!(
                    (sum - mass).abs() <= 5e-5,
                    "{dist:?} at {top}: {sum} against {mass}"
                );
            }
        }
    }

    #[test]
    fn the_gauss_legendre_rule_integrates_degree_31_exactly() {
        let ones = gauss_legendre(0.0, 1.0, |_| 1.0);
        assert!((ones - 1.0).abs() < 1e-15, "{ones}");
        let (a, b) = (0.2, 1.7);
        let got = gauss_legendre(a, b, |x| math::powi(x, 31));
        let exact = (math::powi(b, 32) - math::powi(a, 32)) / 32.0;
        assert!((got / exact - 1.0).abs() < 1e-13, "{got} against {exact}");
    }

    #[test]
    fn a_deep_bowl_keeps_its_shadow_at_60_degrees() {
        // No latitude cut (§8.10): exact ray geometry gives a d/D 0.25 bowl at 60° (δ 1.54°) a
        // permanent shadow of 0.1738 of its area (decision-r09-t7f.md §1), and a d/D 0.1 bowl none.
        let body = moon();
        let bowl = |gamma: f64| permanent_shadow(degrees(60.0), &body, &one_bowl(gamma));
        let deep = bowl(0.25);
        assert!((0.1..=0.3).contains(&deep), "d/D 0.25: {deep}");
        assert!((deep - 0.1738).abs() <= 0.005, "d/D 0.25: {deep}");
        assert_same_bits(bowl(0.1), 0.0);
    }

    /// Smith's (1967, JGR 72, 4059) shadowed share of a Gaussian surface of directional RMS slope
    /// `w` under a sun at elevation `e`: one less (1 − erfc(v)/2) ÷ (1 + Λ), with
    /// v = tan e ÷ (√2 w) and Λ = (e^(−v²) ÷ (v√π) − erfc(v)) ÷ 2.
    fn smith_shadow(w: f64, e: f64) -> f64 {
        let v = math::tan(e) / (SQRT_2 * w);
        let lambda = 0.5 * (math::exp(-v * v) / (v * PI.sqrt()) - math::erfc(v));
        1.0 - (1.0 - 0.5 * math::erfc(v)) / (1.0 + lambda)
    }

    /// The band of a sun at elevation `e` radians, for the instantaneous shadow alone.
    fn sky_at_elevation(e: f64) -> Band {
        Band::new(0.0, e)
    }

    /// The instantaneous shadow of a population of bowls with the sun at elevation `e`: the exact
    /// transect form, with ratios above 0.5 counted as hemispheres.
    fn bowls_shadow(dist: Population, e: f64) -> f64 {
        let sky = sky_at_elevation(e);
        let shallowest = depth_ratio_of_beta(sky.cos2 / sky.half_sin_cos);
        dist.cumulative(HEMISPHERE, |gamma| {
            if gamma <= shallowest {
                0.0
            } else {
                sky.instantaneous(gamma)
            }
        }) + sky.cos2 * dist.mass(HEMISPHERE, f64::INFINITY)
    }

    /// The share of a spherical bowl's opening in shadow under a sun at `elevation` radians, by a
    /// direct ray test on a grid of `grid` × `grid` points across the opening: a point is lit when
    /// its ray toward the sun leaves through the opening before it meets the bowl's wall.
    fn ray_traced_shadow(gamma: f64, elevation: f64, grid: u32) -> f64 {
        let depth = 2.0 * gamma;
        let sphere = (1.0 + depth * depth) / (2.0 * depth);
        let centre = sphere - depth;
        let (sun_up, sun_across) = math::sin_cos(elevation);
        let (mut shadow, mut total) = (0_u32, 0_u32);
        for row in 0..grid {
            let x = -1.0 + 2.0 * (f64::from(row) + 0.5) / f64::from(grid);
            for column in 0..grid {
                let y = -1.0 + 2.0 * (f64::from(column) + 0.5) / f64::from(grid);
                if x * x + y * y >= 1.0 {
                    continue;
                }
                total += 1;
                let z = centre - (sphere * sphere - x * x - y * y).sqrt();
                let to_wall = -2.0 * (x * sun_across + (z - centre) * sun_up);
                if to_wall < -z / sun_up {
                    shadow += 1;
                }
            }
        }
        f64::from(shadow) / f64::from(total)
    }

    #[test]
    fn the_exact_bowl_matches_a_ray_test() {
        // science-r09-drainage.md §8.11: to 0.005 at d/D 0.1 (sun at 2° and 10°), 0.2 (5°) and
        // 0.3 (20°); eq. 5 gives 0.559 at d/D 0.1 and 10°, against the exact 0.463.
        for (gamma, e_deg) in [(0.1, 2.0), (0.1, 10.0), (0.2, 5.0), (0.3, 20.0)] {
            let e = degrees(e_deg).value();
            let exact = sky_at_elevation(e).instantaneous(gamma);
            let traced = ray_traced_shadow(gamma, e, 601);
            assert!(
                (exact - traced).abs() <= 0.005,
                "d/D {gamma}, e {e_deg}°: {exact} against the rays' {traced}"
            );
        }
        let ten = sky_at_elevation(degrees(10.0).value()).instantaneous(0.1);
        assert!((ten - 0.463).abs() < 0.001, "{ten}");
    }

    #[test]
    fn the_cold_traps_peak_on_plains_10_to_20_degrees_rough() {
        // Hayne et al. 2021 ("Temperatures"): the greatest cold-trapping area is at σ_s ≈ 10–20°,
        // since rougher plains cast more shadow but warm it more.
        let body = moon();
        let mut best = (0.0, 0.0);
        for deg in 1..=40_u32 {
            let slope = f64::from(deg);
            let terrain = Terrain::new(TerrainParts {
                crater_fraction: 0.0,
                plains_slope: degrees(slope),
                ..best_fit_parts(DepthRatios::HAYNE_B)
            })
            .unwrap();
            let polar = band_mean(80.0, 90.0, |lat| fraction(lat, &body, &terrain, WATER));
            if polar > best.1 {
                best = (slope, polar);
            }
        }
        assert!((10.0..=20.0).contains(&best.0), "peak at {best:?}");
    }

    /// Latitudes in degrees for the bounds tests, both hemispheres.
    const LATITUDES: [f64; 22] = [
        0.0, 30.0, 60.0, 70.0, 75.0, 80.0, 83.0, 86.0, 88.0, 89.5, 90.0, -0.0, -30.0, -60.0, -70.0,
        -75.0, -80.0, -83.0, -86.0, -88.0, -89.5, -90.0,
    ];

    /// Asserts that `f` lies in [0, 1] and does not rise over `steps` edits of the Moon's parts.
    fn assert_not_rising(
        name: &str,
        steps: u32,
        edit: impl Fn(&mut BodyParts, f64),
        f: impl Fn(Radians, &Body) -> f64,
    ) {
        for lat_deg in LATITUDES {
            let mut last = f64::INFINITY;
            for i in 0..=steps {
                let mut parts = moon_parts();
                edit(&mut parts, f64::from(i));
                let body = Body::new(parts).unwrap();
                let value = f(degrees(lat_deg), &body);
                assert!(
                    (0.0..=1.0).contains(&value),
                    "{name} {i} at {lat_deg}°: {value}"
                );
                assert!(
                    value <= last,
                    "{name} {i} at {lat_deg}°: {value} after {last}"
                );
                last = value;
            }
        }
    }

    #[test]
    fn neither_function_rises_as_the_star_climbs_widens_or_brightens() {
        let declination = |p: &mut BodyParts, i: f64| p.greatest_declination = degrees(0.5 * i);
        let radius = |p: &mut BodyParts, i: f64| p.star_radius = degrees(0.05 * i);
        let flux = |p: &mut BodyParts, i: f64| p.flux = WattsPerSquareMetre::new(100.0 * (1.0 + i));
        let heat = |p: &mut BodyParts, i: f64| p.heat_flow = WattsPerSquareMetre::new(0.01 * i);
        for depths in [DepthRatios::HAYNE_A, DepthRatios::HAYNE_B] {
            let terrain = best_fit(depths);
            let cold = |lat: Radians, body: &Body| fraction(lat, body, &terrain, WATER);
            let shadow = |lat: Radians, body: &Body| permanent_shadow(lat, body, &terrain);
            assert_not_rising("declination", 60, declination, cold);
            assert_not_rising("declination", 60, declination, shadow);
            assert_not_rising("radius", 40, radius, cold);
            assert_not_rising("radius", 40, radius, shadow);
            assert_not_rising("flux", 40, flux, cold);
            assert_not_rising("heat flow", 40, heat, cold);
        }
    }

    #[test]
    fn the_fraction_lies_within_the_shadow_and_follows_the_threshold() {
        let body = moon();
        for depths in [DepthRatios::HAYNE_A, DepthRatios::HAYNE_B] {
            for lat_deg in LATITUDES {
                let latitude = degrees(lat_deg);
                // Not falling as the threshold rises, and never beyond the permanent shadow.
                let terrain = best_fit(depths);
                let shadow = permanent_shadow(latitude, &body, &terrain);
                let mut last = 0.0;
                for t in 1..=60_u32 {
                    let f = fraction(latitude, &body, &terrain, Kelvin::new(5.0 * f64::from(t)));
                    assert!((0.0..=1.0).contains(&f), "{lat_deg}°, {} K: {f}", 5 * t);
                    assert!(f >= last, "{lat_deg}°, {} K: {f} after {last}", 5 * t);
                    assert!(f <= shadow, "{lat_deg}°, {} K: {f} beyond {shadow}", 5 * t);
                    last = f;
                }
                // Not rising with the conduction share, which leaves the shadow alone.
                let mut last = f64::INFINITY;
                for i in 0..=10_u32 {
                    let share = 0.1 * f64::from(i);
                    let terrain = Terrain::new(TerrainParts {
                        share_under_cutoff: share,
                        ..best_fit_parts(depths)
                    })
                    .unwrap();
                    let f = fraction(latitude, &body, &terrain, WATER);
                    assert!(f <= last, "{lat_deg}°, share {share}: {f} after {last}");
                    last = f;
                    assert_same_bits(permanent_shadow(latitude, &body, &terrain), shadow);
                }
            }
        }
    }

    #[test]
    fn a_moon_whose_sun_reaches_30_degrees_has_no_water_cold_traps() {
        // Ceres's permanent shadow is essentially gone at 20° obliquity (Schorghofer et al. 2024,
        // PSJ 5, 99); R09.T13.f expects no water cold trap at 25°.
        for depths in [DepthRatios::HAYNE_A, DepthRatios::HAYNE_B] {
            let terrain = best_fit(depths);
            for tilt in [25.0, 30.0] {
                let body = Body::new(BodyParts {
                    greatest_declination: degrees(tilt),
                    ..moon_parts()
                })
                .unwrap();
                for lat in 0..=90_u32 {
                    let f = fraction(degrees(f64::from(lat)), &body, &terrain, WATER);
                    assert_same_bits(f, 0.0);
                }
            }
        }
    }

    #[test]
    fn a_smooth_crater_free_body_has_no_shadow() {
        let terrain = Terrain::new(TerrainParts {
            crater_fraction: 0.0,
            plains_slope: Radians::ZERO,
            ..best_fit_parts(DepthRatios::HAYNE_A)
        })
        .unwrap();
        let body = moon();
        assert_same_bits(permanent_shadow(degrees(89.0), &body, &terrain), 0.0);
        assert_same_bits(fraction(degrees(89.0), &body, &terrain, WATER), 0.0);
    }

    #[test]
    fn haynes_sigma_is_read_as_a_variance() {
        assert!((DepthRatios::HAYNE_A.std_dev() - 0.04).abs() < 1e-15);
        assert_same_bits(DepthRatios::HAYNE_B.mean(), 0.076);
        let built = DepthRatios::new(0.14, 1.6e-3).unwrap();
        assert_eq!(built, DepthRatios::HAYNE_A);
        assert_same_bits(built.variance(), 1.6e-3);
    }

    #[test]
    fn the_inputs_refuse_values_out_of_range() {
        assert_eq!(
            DepthRatios::new(0.6, 0.01),
            Err(BuildDepthRatiosError::Mean(0.6))
        );
        assert_eq!(
            DepthRatios::new(1e-200, 0.01),
            Err(BuildDepthRatiosError::Mean(1e-200))
        );
        // A spread that rounds away: variance ÷ mean² under 2⁻⁵³.
        assert_eq!(
            DepthRatios::new(0.5, 1e-40),
            Err(BuildDepthRatiosError::Variance(1e-40))
        );
        // A spread so wide that the moment of 1 ÷ γ overflows.
        assert_eq!(
            DepthRatios::new(1e-5, 1e295),
            Err(BuildDepthRatiosError::Variance(1e295))
        );
        assert_eq!(
            BuildDepthRatiosError::Mean(0.6).to_string(),
            "mean depth ratio 0.6 is not in (0, 0.5] or has no normal square"
        );
        assert!(Population::of_plains(Radians::new(1e-160)).is_none());
        assert_eq!(
            DepthRatios::new(0.1, 0.0),
            Err(BuildDepthRatiosError::Variance(0.0))
        );
        let refuse = |edit: fn(&mut BodyParts)| {
            let mut parts = moon_parts();
            edit(&mut parts);
            Body::new(parts).unwrap_err()
        };
        assert_eq!(
            refuse(|p| p.flux = WattsPerSquareMetre::ZERO),
            BuildBodyError::Flux(0.0)
        );
        assert_eq!(
            refuse(|p| p.greatest_declination = Radians::new(-0.1)),
            BuildBodyError::GreatestDeclination(-0.1)
        );
        assert_eq!(
            refuse(|p| p.greatest_declination = Radians::new(1.6)),
            BuildBodyError::GreatestDeclination(1.6)
        );
        assert_eq!(
            refuse(|p| p.star_radius = Radians::new(2.0)),
            BuildBodyError::StarRadius(2.0)
        );
        assert_eq!(
            refuse(|p| p.star_radius = Radians::new(1e-170)),
            BuildBodyError::StarRadius(1e-170)
        );
        assert_eq!(
            refuse(|p| p.greatest_declination = Radians::new(1e-170)),
            BuildBodyError::GreatestDeclination(1e-170)
        );
        assert_eq!(
            refuse(|p| p.emissivity = 5e-324),
            BuildBodyError::Emissivity(5e-324)
        );
        assert_eq!(
            refuse(|p| p.bond_albedo = 1.0),
            BuildBodyError::BondAlbedo(1.0)
        );
        assert_eq!(
            refuse(|p| p.emissivity = 0.0),
            BuildBodyError::Emissivity(0.0)
        );
        assert_eq!(
            refuse(|p| p.heat_flow = WattsPerSquareMetre::new(-1.0)),
            BuildBodyError::HeatFlow(-1.0)
        );
        assert_eq!(moon().parts(), &moon_parts());
        let terrain = |edit: fn(&mut TerrainParts)| {
            let mut parts = best_fit_parts(DepthRatios::HAYNE_B);
            edit(&mut parts);
            Terrain::new(parts)
        };
        assert_eq!(
            terrain(|p| p.crater_fraction = 1.5),
            Err(BuildTerrainError::CraterFraction(1.5))
        );
        assert_eq!(
            terrain(|p| p.plains_slope = Radians::new(1.0)),
            Err(BuildTerrainError::PlainsSlope(1.0))
        );
        assert_eq!(
            terrain(|p| p.share_under_cutoff = -0.5),
            Err(BuildTerrainError::ShareUnderCutoff(-0.5))
        );
        assert_eq!(
            terrain(|_| {}).unwrap().parts(),
            &best_fit_parts(DepthRatios::HAYNE_B)
        );
    }
}
