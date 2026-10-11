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
//!   closed form for (`science-r09-drainage.md` §8.1, §8.10 and §8.11). They are bowls whose d/D
//!   is half the magnitude of the surface's directional slope, a normal variable of RMS tan `σ_s`:
//!   a half-normal of scale tan `σ_s` ÷ 2. A paraboloid bowl's RMS directional slope is 2 d/D, and
//!   a spherical bowl's is within 6% of that below d/D 0.2, so the population keeps the plains'
//!   mean-square slope. A log-normal of the same two moments would not do: its heavier tail leaves
//!   deep bowls in shadow equatorward of 70°.
//!   - Its instantaneous shadow lies within 15% of Smith's shadowing function for a Gaussian surface
//!     (1967, JGR 72, 4059) with the sun up to 4° high, and Hayne et al. found Smith's function
//!     within 5% of their ray tracing.
//!   - With the sun higher it overstates Smith's: by up to 1.8 times to 12°, which covers the
//!     Moon's cold traps, and by up to 2.4 times above that, near the edge of Smith's shadow. A
//!     bowl casts shadow once its rim, 4γ ÷ (1 − 4γ²) steep, passes the sun, and a Gaussian surface
//!     of the same mean-square slope has fewer steep facets.
//!   - It puts the plains' cold-trap optimum at a `σ_s` of about 12°, inside Hayne et al.'s 10–20°.
//!   - The module's tests pin all three below 12°.
//!
//! For each bowl, of depth ratio γ = d/D, at colatitude e₀ under a star whose greatest
//! declination in the sky is δ and whose angular radius is α:
//!
//! - **Permanent shadow** is ground that no part of the star's disk ever reaches. It is Hayne et
//!   al.'s eq. 26, `A_perm` ÷ `A_crater` ≈ 1 − 8βe₀ ÷ 3π − 2βδ with β = 1 ÷ 2γ − 2γ, reckoned from
//!   the upper limb, so that δ becomes δ + α (Hayne et al.'s Sun is a point, α = 0). They fitted it
//!   at d/D 0.2, 70–90° and the Moon's 1.54° (Methods A.2.5, Fig. S3), and it is used beyond that.
//! - **It is capped by the bowl's instantaneous shadow** at the upper limb's highest elevation e,
//!   since a permanent shadow lies inside every instantaneous one. That shadow is the exact one of
//!   a spherical bowl (`science-r09-drainage.md` §8.11): Hayne et al.'s eq. 3, exact on the transect
//!   along the sun through the bowl's centre, applied to each parallel transect at its own depth
//!   ratio and integrated across the bowl. Their eq. 5 takes the other transects as similar to the
//!   central one (their eq. 4), but a sphere's off-centre sections are shallower, so eq. 5
//!   overstates the shadow, by up to about 17% (0.559 against 0.463 at d/D 0.1 and e = 10°). The
//!   cap keeps a deep bowl near the equator out of permanent shadow.
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
//! - **Integration over d/D.** Eq. 26 is linear in β, so its part is closed-form: the partial
//!   moments of γ⁰, γ¹ and γ⁻¹ are differences of the normal distribution function for the
//!   craters' log-normal, and of the error function, the exponential and the exponential integral
//!   E₁ for the plains' half-normal. The capped part, the bowls deeper than where eq. 26 meets the
//!   instantaneous shadow, takes four panels of a 16-point Gauss–Legendre rule over the
//!   population's own ±8σ. Ratios above 0.5, deeper than a hemisphere, count as 0.5, the limit of
//!   both eq. 26 and Ingersoll's sphere.
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
//! - **The fraction errs low**, at Hayne et al.'s best-fit terrain about 0.7 of their Table 1
//!   poleward of 80°, and none between 70° and 80°, where they find 7 × 10⁻⁴%.
//!   - Equilibrium at the highest sun stands in for the annual maximum temperature. It bounds it
//!     from above, yet its cold bowls' largest d/D at 110 K follows Hayne et al.'s
//!     no-conduction curve (their Fig. S8) closely, so it is not the main cause.
//!   - Eq. 26 errs both ways against a spherical bowl's exact ray geometry
//!     (`science-r09-drainage.md` §8.10). Its declination term removes shallow bowls' shadow away
//!     from the pole (d/D 0.1 at 80°: 3% against an exact 22%), the cold bowls among them. It
//!     overstates deep bowls equatorward of 75° (up to 1.7 times at d/D 0.3 and 60°). That puts
//!     the Moon's 70–80° permanent shadow at about 0.3 of Hayne et al.'s.
//!   - A peak-temperature form also understates the most volatile species' traps: Schorghofer et
//!     al. find 3.6 times the CO₂ area by the time-averaged loss (LPSC 2021, abstract 1389).
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

/// Bisection steps for the depth ratio where eq. 26 meets the instantaneous shadow.
///
/// Each halves an interval at most 0.5 wide, so 40 steps resolve it to 5 × 10⁻¹³. The two shares
/// are equal there, so an error in it moves the integral only to second order.
const CROSSING_STEPS: u32 = 40;

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

/// Panels of the composite rule for the deep bowls' part of a population.
const CAP_PANELS: u32 = 4;

/// Standard deviations either side of a population's centre over which its deep bowls' part is
/// integrated: a normal distribution's mass beyond 8σ is 1.2 × 10⁻¹⁵. Four panels of the 16-point
/// rule integrate a normal density over ±8σ to 10⁻¹⁶.
const SPREAD_WIDTHS: f64 = 8.0;

/// ∫ₐᵇ f by [`CAP_PANELS`] equal panels of [`GAUSS_LEGENDRE_16`], in their fixed order; zero when
/// b ≤ a.
#[must_use]
fn composite(a: f64, b: f64, f: impl Fn(f64) -> f64) -> f64 {
    if b <= a {
        return 0.0;
    }
    let width = (b - a) / f64::from(CAP_PANELS);
    let mut sum = 0.0;
    for panel in 0..CAP_PANELS {
        let lo = a + width * f64::from(panel);
        sum += gauss_legendre(lo, lo + width, &f);
    }
    sum
}

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
    /// The star's greatest declination in the body's sky δ, radians in [0, π/2]: eq. 26's δ, from
    /// which the star's highest elevation at colatitude e₀, min(90°, e₀ + δ), follows.
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
    /// an elevation of sine `sin_elevation`: 0 when none does, and 0.5 when every bowl does.
    #[must_use]
    fn coldest_limit(&self, sin_elevation: f64, threshold: Kelvin) -> f64 {
        let t = threshold.value();
        let target = STEFAN_BOLTZMANN * t * t * t * t;
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

    /// The share of the ground in permanent shadow in bowls shallower than `cold_limit`, the
    /// craters' share first and then the plains', in that fixed order.
    #[must_use]
    fn shadowed(&self, sky: &Sky, cold_limit: f64) -> f64 {
        let craters = self.parts.crater_fraction;
        let plains = 1.0 - craters;
        let mut share = 0.0;
        if craters > 0.0 {
            let population = Population::LogNormal(self.parts.crater_depths.log_normal());
            share += craters * sky.shadowed(population, cold_limit);
        }
        if plains > 0.0
            && let Some(population) = Population::of_plains(self.parts.plains_slope)
        {
            share += plains * sky.shadowed(population, cold_limit);
        }
        share
    }
}

/// The star's path at one latitude, as the bowls' shadows and temperatures need it.
#[derive(Debug, Clone, Copy)]
struct Sky {
    /// sin e of the star's flux-weighted elevation at its highest, for the shadows' temperature.
    sin_highest: f64,
    /// Eq. 26's coefficient of β, 8e₀ ÷ 3π + 2(δ + α), from the star's upper limb.
    c: f64,
    /// cos² e of the upper limb at its highest elevation e.
    cos2: f64,
    /// sin e cos e ÷ 2 of the upper limb at its highest elevation e.
    half_sin_cos: f64,
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
        let limb = declination + radius.value();
        let highest = Radians::new(num::min(FRAC_PI_2, colatitude + declination));
        let limb_highest = num::min(FRAC_PI_2, colatitude + limb);
        let (sin_l, cos_l) = math::sin_cos(limb_highest);
        Self {
            sin_highest: math::sin(flux_weighted_elevation(highest, radius).value()),
            c: 8.0 * colatitude / (3.0 * PI) + 2.0 * limb,
            cos2: cos_l * cos_l,
            half_sin_cos: 0.5 * sin_l * cos_l,
        }
    }

    /// The share of a population of bowls that lies in permanent shadow, counting only the bowls
    /// shallower than `cold_limit` (0.5 counts every bowl).
    ///
    /// A bowl's share is max(0, min(eq. 26, its instantaneous shadow)). Both rise with the bowl's
    /// depth ratio. Eq. 26 is the smaller for the shallow bowls, from where it leaves zero up to
    /// where it meets the instantaneous shadow, and the instantaneous shadow caps the deeper ones.
    #[must_use]
    fn shadowed(&self, dist: Population, cold_limit: f64) -> f64 {
        // β where eq. 26 reaches zero, 1 ÷ c, and where the central transect's shadow does,
        // 2 cot e: the shallowest bowl with a shadow.
        let beta_zero = num::min(1.0 / self.c, self.cos2 / self.half_sin_cos);
        let shallowest = depth_ratio_of_beta(beta_zero);
        let crossing = self.cap_crossing(shallowest);
        let top = num::min(cold_limit, HEMISPHERE);

        let mut share = 0.0;
        // Shallow bowls, eq. 26: 1 − cβ, in closed form.
        share += dist.linear_in_beta(shallowest, num::min(crossing, top), 1.0, self.c);
        // Deep bowls, capped by their instantaneous shadow.
        let deep = num::max(shallowest, crossing);
        if top > deep {
            share += dist.integral(deep, top, |gamma| self.instantaneous(gamma));
        }
        // Bowls deeper than a hemisphere count as hemispheres, if they are cold: eq. 26 gives
        // them 1, and their instantaneous shadow cos² e.
        if cold_limit >= HEMISPHERE && shallowest < HEMISPHERE {
            share += self.cos2 * dist.mass(HEMISPHERE, f64::INFINITY);
        }
        share
    }

    /// The depth ratio from which a bowl's instantaneous shadow caps eq. 26.
    ///
    /// It is found by bisection, in a fixed number of steps, between `shallowest` and a
    /// hemisphere. It is a hemisphere when eq. 26 never exceeds the instantaneous shadow, and
    /// `shallowest` when it exceeds it from the start.
    #[must_use]
    fn cap_crossing(&self, shallowest: f64) -> f64 {
        // A hemisphere's β is 0, so eq. 26 gives it 1.
        if shallowest >= HEMISPHERE || 1.0 - self.instantaneous(HEMISPHERE) <= 0.0 {
            return HEMISPHERE;
        }
        let excess = |gamma: f64| 1.0 - self.c * beta_of(gamma) - self.instantaneous(gamma);
        if excess(shallowest) > 0.0 {
            return shallowest;
        }
        let (mut lo, mut hi) = (shallowest, HEMISPHERE);
        for _ in 0..CROSSING_STEPS {
            let mid = f64::midpoint(lo, hi);
            if excess(mid) > 0.0 {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        hi
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

/// A population of bowls, by the distribution of their depth ratios.
#[derive(Debug, Clone, Copy)]
enum Population {
    /// Craters, log-normal (Hayne et al. 2021, eqs. 33–35).
    LogNormal(LogNormal),
    /// Rough plains: half the magnitude of a Gaussian surface's directional slope.
    HalfNormal(HalfNormal),
}

impl Population {
    /// The bowls standing for plains of directional RMS slope `slope`, or `None` for flat plains
    /// (`science-r09-drainage.md` §8.10).
    ///
    /// A bowl's directional RMS slope is 2 d/D, so d/D = |s| ÷ 2 for a slope s normal with RMS
    /// w = tan `σ_s`: a half-normal of scale w ÷ 2, whose mean-square slope is the surface's. A
    /// log-normal of the same two moments has a far heavier tail (P(d/D > 0.2) 50 times the
    /// half-normal's at `σ_s` = 5.7°), which would leave deep bowls in shadow equatorward of 70°. A
    /// slope under 10⁻¹⁰⁰ rad counts as flat, so that the scale's square stays a normal number.
    #[must_use]
    fn of_plains(slope: Radians) -> Option<Self> {
        let w = math::tan(slope.value());
        if w < 1e-100 {
            return None;
        }
        Some(Self::HalfNormal(HalfNormal { scale: 0.5 * w }))
    }

    /// The probability of a depth ratio in (a, b), 0 ≤ a, b ≤ ∞.
    #[must_use]
    fn mass(self, a: f64, b: f64) -> f64 {
        match self {
            Self::LogNormal(dist) => dist.mass(a, b),
            Self::HalfNormal(dist) => dist.mass(a, b),
        }
    }

    /// ∫ₐᵇ (k₀ − k₁ β(γ)) P(γ) dγ over the depth ratios γ in (a, b), with β = 1 ÷ 2γ − 2γ,
    /// 0 ≤ a and b ≤ 0.5.
    #[must_use]
    fn linear_in_beta(self, a: f64, b: f64, k0: f64, k1: f64) -> f64 {
        match self {
            Self::LogNormal(dist) => dist.linear_in_beta(a, b, k0, k1),
            Self::HalfNormal(dist) => dist.linear_in_beta(a, b, k0, k1),
        }
    }

    /// ∫ₐᵇ g(γ) P(γ) dγ for a smooth g, 0 ≤ a.
    ///
    /// It takes [`CAP_PANELS`] panels of [`GAUSS_LEGENDRE_16`] over the part of (a, b) within
    /// [`SPREAD_WIDTHS`] standard deviations of the distribution's centre, in the variable in which
    /// it is normal: ln γ for the log-normal, γ for the half-normal. So a narrow population is
    /// resolved wherever it lies in (a, b).
    #[must_use]
    fn integral(self, a: f64, b: f64, g: impl Fn(f64) -> f64) -> f64 {
        let root_two_pi = (2.0 * PI).sqrt();
        match self {
            Self::LogNormal(dist) => {
                let lo = num::max(math::ln(a), dist.m - SPREAD_WIDTHS * dist.s);
                let hi = num::min(math::ln(b), dist.m + SPREAD_WIDTHS * dist.s);
                composite(lo, hi, |t| {
                    let z = (t - dist.m) / dist.s;
                    g(math::exp(t)) * math::exp(-0.5 * z * z) / (dist.s * root_two_pi)
                })
            }
            Self::HalfNormal(dist) => {
                let hi = num::min(b, SPREAD_WIDTHS * dist.scale);
                composite(a, hi, |gamma| {
                    g(gamma) * 2.0 / (dist.scale * root_two_pi) * math::exp(-dist.exponent(gamma))
                })
            }
        }
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

    /// ∫ₐᵇ (k₀ − k₁ β(γ)) P(γ) dγ, as [`Population::linear_in_beta`].
    ///
    /// It takes the partial moments E[γ; a, b] = q √(2/π) (e^(−u(a)) − e^(−u(b))) and
    /// E[γ⁻¹; a, b] = (E₁(u(a)) − E₁(u(b))) ÷ q√(2π), with u(γ) = γ² ÷ 2q² and E₁ the exponential
    /// integral. E[γ⁻¹] diverges at a = 0, which only a pole with no greatest declination and a
    /// point star reaches, where k₁ is zero; the β term is left out when k₁ is. A declination
    /// whose square is not a normal `f64` would reach a = 0 with k₁ above zero, so [`Body::new`]
    /// refuses it.
    #[must_use]
    fn linear_in_beta(self, a: f64, b: f64, k0: f64, k1: f64) -> f64 {
        if b <= a {
            return 0.0;
        }
        let mut share = k0 * self.mass(a, b);
        if k1 > 0.0 {
            let (ua, ub) = (self.exponent(a), self.exponent(b));
            let m1 = self.scale * (2.0 / PI).sqrt() * (math::exp(-ua) - math::exp(-ub));
            let m_inv = (exponential_integral(ua) - exponential_integral(ub))
                / (self.scale * (2.0 * PI).sqrt());
            share -= k1 * (0.5 * m_inv - 2.0 * m1);
        }
        share
    }
}

/// The Euler–Mascheroni constant γ, 0.577 215 664 901 532 860 6… (OEIS A001620).
const EULER_GAMMA: f64 = 0.577_215_664_901_532_9;

/// Terms of the exponential integral's power series, used for x ≤ 1.5: the 30th is at most
/// 1.5³⁰ ÷ (30 · 30!) ≈ 2.4 × 10⁻²⁹, and the first one left out 1.1 × 10⁻³⁰.
const SERIES_TERMS: u32 = 30;

/// Depth of the exponential integral's continued fraction, used for x > 1.5, evaluated from its
/// tail: 64 levels give it to within 5 × 10⁻¹⁶ of its value at x = 1.5 and closer above.
const FRACTION_DEPTH: u32 = 64;

/// The exponential integral E₁(x) = ∫ₓ^∞ e^(−t) ÷ t dt for x ≥ 0: +∞ at 0 and 0 at +∞.
///
/// Below 1.5 by its power series, E₁(x) = −γ − ln x − Σₖ (−x)ᵏ ÷ (k · k!) (Abramowitz and Stegun
/// 1964, 5.1.11); above, by its even continued fraction, E₁(x) = e^(−x) ÷ (x + 1 − 1² ÷ (x + 3 −
/// 2² ÷ (x + 5 − …))) (A&S 5.1.22, contracted as in Press et al., Numerical Recipes, 3rd ed.,
/// §6.3), each with a fixed number of terms. Its relative error is under 6 × 10⁻¹⁵ against a
/// 60-digit series from 10⁻³⁰⁰ to 30.
#[must_use]
fn exponential_integral(x: f64) -> f64 {
    if x <= 1.5 {
        let mut term = 1.0;
        let mut sum = 0.0;
        for k in 1..=SERIES_TERMS {
            let k = f64::from(k);
            term *= -x / k;
            sum -= term / k;
        }
        -EULER_GAMMA - math::ln(x) + sum
    } else {
        let mut tail = 0.0;
        for k in (1..=FRACTION_DEPTH).rev() {
            let k = f64::from(k);
            tail = k * k / (x + 2.0 * k + 1.0 - tail);
        }
        math::exp(-x) / (x + 1.0 - tail)
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

    /// ∫ₐᵇ (k₀ − k₁ β(γ)) P(γ) dγ over the ratios γ in (a, b), with β = 1 ÷ 2γ − 2γ.
    ///
    /// It takes the partial moments of γ⁰, γ¹ and γ⁻¹. A log-normal's are E[γ; a, b] = e^(m + s²/2) [Φ(z(b) − s) −
    /// Φ(z(a) − s)] and E[γ⁻¹; a, b] = e^(−m + s²/2) [Φ(z(b) + s) − Φ(z(a) + s)].
    #[must_use]
    fn linear_in_beta(self, a: f64, b: f64, k0: f64, k1: f64) -> f64 {
        if b <= a {
            return 0.0;
        }
        let (za, zb) = (self.score(a), self.score(b));
        let half_s2 = 0.5 * self.s * self.s;
        let m0 = normal_between(za, zb);
        let m1 = math::exp(self.m + half_s2) * normal_between(za - self.s, zb - self.s);
        let m_inv = math::exp(-self.m + half_s2) * normal_between(za + self.s, zb + self.s);
        let beta = 0.5 * m_inv - 2.0 * m1;
        k0 * m0 - k1 * beta
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
    num::max(0.0, num::min(1.0, terrain.shadowed(&sky, HEMISPHERE)))
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
    let cold_limit = body.coldest_limit(sky.sin_highest, threshold);
    if cold_limit <= 0.0 {
        return 0.0;
    }
    let share = terrain.shadowed(&sky, cold_limit) * (1.0 - terrain.parts.share_under_cutoff);
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

    #[test]
    fn haynes_best_fit_terrain_gives_table_1_within_a_factor_of_two() {
        // Hayne et al. 2021, Table 1, the mean of both hemispheres: permanent shadow is 8.5% of
        // 80–90°, 0.5% of 70–80° and 0.15% of the whole Moon; cold traps (Tmax < 110 K) are 6.7%,
        // 7.0 × 10⁻⁴% and 0.10%. A point Sun, as theirs (Methods D), and no conduction share.
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
            // Equilibrium at the highest sun bounds Hayne et al.'s transient peak from above, so
            // the 70–80° band's cold traps are held from above only (science-r09-drainage.md §8.2).
            let sub = band_mean(70.0, 80.0, cold);
            assert!(sub <= 1.4e-5, "{depths:?}: cold traps over 70–80° {sub}");
        }
        // Permanent shadow with distribution B (§8.1): A's deep bowls keep about 2.4 times Hayne
        // et al.'s whole-Moon shadow.
        let terrain = best_fit(DepthRatios::HAYNE_B);
        let shadow = |lat: Radians| permanent_shadow(lat, &body, &terrain);
        let polar = band_mean(80.0, 90.0, shadow);
        assert!(within_two(polar, 0.085), "shadow over 80–90° {polar}");
        let whole = band_mean(0.0, 90.0, shadow);
        assert!(within_two(whole, 0.0015), "shadow overall {whole}");
        // Over 70–80° within a factor of four, since eq. 26 understates shallow bowls' shadow away
        // from the pole (§8.10), and next to none further out.
        let sub = band_mean(70.0, 80.0, shadow);
        assert!(
            (0.005 / 4.0..=0.005 * 4.0).contains(&sub),
            "shadow over 70–80° {sub}"
        );
        let sixties = band_mean(60.0, 70.0, shadow);
        assert!(sixties <= 1e-4, "shadow over 60–70° {sixties}");
        let fifties = band_mean(50.0, 60.0, shadow);
        assert!(fifties <= 1e-5, "shadow over 50–60° {fifties}");
    }

    #[test]
    fn the_instantaneous_shadow_caps_a_deep_bowl() {
        // A d/D 0.45 bowl: uncapped, eq. 26 leaves 0.71 of it in shadow at the equator, under a Sun
        // that passes overhead, and 0.80 at 30°.
        let body = moon();
        let terrain = Terrain::new(TerrainParts {
            crater_fraction: 1.0,
            crater_depths: DepthRatios::new(0.45, 1e-10).unwrap(),
            ..best_fit_parts(DepthRatios::HAYNE_B)
        })
        .unwrap();
        let equator = permanent_shadow(degrees(0.0), &body, &terrain);
        assert!(equator < 1e-9, "the equator: {equator}");
        let sky = Sky::at(degrees(30.0), &body);
        let cap = sky.instantaneous(0.45);
        let eq26 = 1.0 - sky.c * beta_of(0.45);
        assert!(cap < 0.5 * eq26, "30°: cap {cap}, eq. 26 {eq26}");
        let thirty = permanent_shadow(degrees(30.0), &body, &terrain);
        assert!(
            (thirty - cap).abs() < 1e-4,
            "30°: {thirty} against the cap {cap}"
        );
        // At 80° the two meet strictly between the shallowest shadowed bowl and a hemisphere.
        let sky = Sky::at(degrees(80.0), &body);
        let shallowest = depth_ratio_of_beta(num::min(1.0 / sky.c, sky.cos2 / sky.half_sin_cos));
        let crossing = sky.cap_crossing(shallowest);
        assert!(
            shallowest < crossing && crossing < HEMISPHERE,
            "{shallowest} < {crossing} < 0.5"
        );
        let gap = 1.0 - sky.c * beta_of(crossing) - sky.instantaneous(crossing);
        assert!(
            gap.abs() < 1e-9,
            "eq. 26 and the cap differ by {gap} where they meet"
        );
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
        // permanent shadow of 0.17 of its area, and a d/D 0.1 bowl none.
        let body = moon();
        let bowl = |gamma: f64| {
            let terrain = Terrain::new(TerrainParts {
                crater_fraction: 1.0,
                crater_depths: DepthRatios::new(gamma, 1e-10).unwrap(),
                ..best_fit_parts(DepthRatios::HAYNE_B)
            })
            .unwrap();
            permanent_shadow(degrees(60.0), &body, &terrain)
        };
        let deep = bowl(0.25);
        assert!((0.1..=0.3).contains(&deep), "d/D 0.25: {deep}");
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

    /// The sky of a sun at elevation `e` radians, for the instantaneous shadow alone.
    fn sky_at_elevation(e: f64) -> Sky {
        let (sin_e, cos_e) = math::sin_cos(e);
        Sky {
            sin_highest: sin_e,
            c: f64::INFINITY,
            cos2: cos_e * cos_e,
            half_sin_cos: 0.5 * sin_e * cos_e,
        }
    }

    /// The instantaneous shadow of a population of bowls with the sun at elevation `e`: the exact
    /// transect form, with ratios above 0.5 counted as hemispheres.
    fn bowls_shadow(dist: Population, e: f64) -> f64 {
        let sky = sky_at_elevation(e);
        let shallowest = depth_ratio_of_beta(sky.cos2 / sky.half_sin_cos);
        dist.integral(shallowest, HEMISPHERE, |gamma| sky.instantaneous(gamma))
            + sky.cos2 * dist.mass(HEMISPHERE, f64::INFINITY)
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
    fn the_plains_bowls_cast_smiths_shadow_where_the_cold_traps_lie() {
        // Hayne et al. found Smith's function within 5% of their ray-traced plains, at RMS slopes
        // of 0.1, 0.3 and 0.5 (5.7°, 16.7° and 26.6°). science-r09-drainage.md §8.11: within 15%
        // with the sun up to 4° high, and at most 1.8 times Smith's shadow from there to 12°,
        // wherever it is at least 5%. Above 12° the overstatement, up to 2.4 times, is recorded
        // in R09's Risks and not asserted.
        for w in [0.1, 0.3, 0.5] {
            let plains = Population::of_plains(Radians::new(math::atan(w))).unwrap();
            for half_degrees in 1..=24_u32 {
                let e_deg = 0.5 * f64::from(half_degrees);
                let e = degrees(e_deg).value();
                let smith = smith_shadow(w, e);
                let ratio = bowls_shadow(plains, e) / smith;
                if e_deg <= 4.0 {
                    assert!(
                        (ratio - 1.0).abs() <= 0.15,
                        "w {w}, e {e_deg}°: {ratio} of Smith's"
                    );
                } else if smith >= 0.05 {
                    assert!(ratio <= 1.8, "w {w}, e {e_deg}°: {ratio} of Smith's");
                }
            }
        }
    }

    #[test]
    fn the_exponential_integral_matches_its_series() {
        // A 60-digit evaluation of A&S 5.1.11.
        let reference = [
            (1e-12, 27.053_805_451_028_015),
            (0.01, 4.037_929_576_538_114),
            (0.5, 0.559_773_594_776_160_8),
            (1.0, 0.219_383_934_395_520_29),
            (1.5, 0.100_019_582_406_632_65),
            (2.0, 0.048_900_510_708_061_12),
            (5.0, 0.001_148_295_591_275_325_7),
            (10.0, 4.156_968_929_685_325e-6),
            (20.0, 9.835_525_290_649_882e-11),
        ];
        for (x, e1) in reference {
            let got = exponential_integral(x);
            assert!((got / e1 - 1.0).abs() < 1e-14, "E1({x}) = {got}, not {e1}");
        }
        let at_zero = exponential_integral(0.0);
        assert!(at_zero.is_infinite() && at_zero > 0.0, "E1(0) = {at_zero}");
        assert_same_bits(exponential_integral(f64::INFINITY), 0.0);
    }

    #[test]
    fn the_half_normal_moments_match_a_quadrature() {
        // ∫ (k₀ − k₁ β(γ)) P(γ) dγ by the midpoint rule in 200,000 steps.
        for scale in [0.005, 0.05, 0.2] {
            let dist = HalfNormal { scale };
            for (a, b) in [(0.01, 0.1), (0.05, 0.5), (0.2, 0.5)] {
                let (k0, k1) = (0.7, 0.3);
                let steps = 200_000_u32;
                let width = (b - a) / f64::from(steps);
                let mut sum = 0.0;
                for step in 0..steps {
                    let gamma = a + width * (f64::from(step) + 0.5);
                    let density = (2.0 / PI).sqrt() / scale * math::exp(-dist.exponent(gamma));
                    sum += (k0 - k1 * beta_of(gamma)) * density * width;
                }
                let closed = dist.linear_in_beta(a, b, k0, k1);
                assert!(
                    (closed - sum).abs() <= 1e-12 + 1e-8 * sum.abs(),
                    "scale {scale}, ({a}, {b}): {closed} against {sum}"
                );
            }
        }
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
