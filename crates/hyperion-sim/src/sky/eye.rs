//! The naked eye's threshold for a star against a background, and the veiling glare of bright
//! stars (rendering plan R06, Design notes 2–4).
//!
//! The threshold is Crumey's (2014, "Human contrast threshold and astronomical visibility", MNRAS
//! 442, 2600; arXiv:1405.4209) point-source model with Blackwell's (1946) coefficients, his eq. 34
//! over the whole range of backgrounds rather than his scotopic eq. 53 joined to it: eq. 34 is
//! within 0.03 mag of eq. 53 above μ<sub>V</sub> 20 (0.026 there, 0.02 from μ<sub>V</sub> 20.6),
//! and eq. 53 alone gives fainter limits under brighter skies below μ<sub>V</sub> 16.7, where its
//! bracket peaks. Eq. 34 has a shallow dip of 0.029 mag around Crumey's transition at 7.08 × 10⁻²
//! cd m⁻² (§2.2), an artefact of the fit, removed by the running minimum from the dark side (see
//! [`threshold_illuminance`]). It has no absolute threshold, so the colour-corrected background is
//! clamped at Crumey's 10⁻⁵ cd m⁻² ([`DARKEST_BACKGROUND`]), below which he holds the threshold
//! constant.
//!
//! Colour enters twice, through the scotopic-to-photopic ("S/P") ratio ρ of Crumey's eq. 5:
//!
//! - The background is taken to Blackwell's 2,850 K light ([`BLACKWELL_SP_RATIO`], ρ = 1.408,
//!   Crumey §1.3): a background of photopic luminance B and ratio ρ₀ looks to the rods like
//!   (ρ₀ ÷ 1.408) B of Blackwell's light (Crumey eq. 6).
//! - A star's threshold moves by 2.5 log₁₀(ρ★ ÷ 2.297) ([`star_colour_offset`]), the reference
//!   being a star of B − V = 0.7 ([`REFERENCE_SP_RATIO`]). Taking eq. 34's thresholds as those of
//!   that star, the typical naked-eye star Crumey names "if this is considered the standard"
//!   (§3.1), is the plan's convention (Design note 3), under which the empirical field factor F
//!   keeps its calibration; read literally, his eqs. 6 and 16 put them at 2,850 K.
//!
//! Both corrections are scotopic. In mesopic backgrounds (a starlit μ<sub>V</sub> brighter than
//! about 19.2, where its scotopic luminance passes 0.005 cd m⁻²) they fade with the photopic weight
//! m of CIE 191:2010's MES2 system ([`mesopic_weight`]): each light is weighed by its MES2 mesopic
//! luminance, so the background's factor is (m + (1 − m) ρ₀ C) ÷ (m + (1 − m) 1.408 C), with C =
//! V′(555 nm) = 683 ÷ 1699 ([`blackwell_equivalent_factor`]). This is the plan's construction, the
//! equal-mesopic-luminance analogue of Crumey's eq. 6, not Crumey's, who leaves mesopic photometry
//! open (§1.3); it moves the limit by under 0.03 mag between μ<sub>V</sub> 16 and 19.
//!
//! The glare of a resolved star is CIE 146:2002's general disability glare equation (Vos 2003,
//! "Reflections on glare", Lighting Res. Technol. 35, 163), the "standard way" Crumey points to
//! (Adrian 1989), of the glare's illuminance in the plane of the eye; see [`veiling_luminance`].

use std::error::Error;
use std::fmt;

use crate::math;
use crate::units::{CandelasPerSquareMetre, Degrees, Lux, Magnitudes, MagnitudesPerArcsec2};

/// The deepest cut a sky may be asked to, V = 11.0: a camera's limit is clamped there, because a
/// narrow zoom deeper than it would ask for some 10⁶ stars; a deeper exposure asks for a cone
/// (Design note 5). The protocol re-states it (R06.T10).
pub const MAX_CUT_V: f64 = 11.0;

/// The S/P ratio of a star of B − V = 0.7, Cinzano's typical naked-eye star: 2.297, from Crumey's
/// eq. 13, log₁₀ ρ = −0.1094 (B − V) + 0.4378. A star of this ratio has no colour offset.
pub const REFERENCE_SP_RATIO: f64 = 2.297;

/// The S/P ratio of Blackwell's (1946) 2,850 K incandescent sources, 1.408 (Crumey 2014, §1.3, by
/// Planck's law against the CIE 1924 V(λ) and 1951 V′(λ)): every threshold of eq. 34 is measured
/// in that light.
pub const BLACKWELL_SP_RATIO: f64 = 1.408;

/// The darkest background the threshold reads, 10⁻⁵ cd m⁻² of Blackwell's light (after the colour
/// correction): eq. 34 tends to zero threshold as B → 0 (V 15.3 at μ 40), and Crumey holds the
/// threshold constant for B ≤ 10⁻⁵ cd m⁻², where "the background becomes effectively zero" (2014,
/// §2.3, eqs. 47–52; §3.2, eq. 71, ζ = 1.150 × 10⁻⁹ lx). At F = 1.4 that is a limit of 7.99, which
/// a starlit background (ρ₀ = 2.26) reaches at μ<sub>V</sub> 25.6 (decided 2026-10-02 in place of
/// the plan's μ 27, which gave 8.64).
pub const DARKEST_BACKGROUND: CandelasPerSquareMetre = CandelasPerSquareMetre::new(1e-5);

/// The zero point between surface brightness and luminance, mag arcsec⁻²: μ<sub>V</sub> = 12.58 −
/// 2.5 log₁₀(B ÷ 1 cd m⁻²) (Crumey 2014, §1.2).
const SURFACE_BRIGHTNESS_ZERO_POINT: f64 = 12.58;

/// The zero point between illuminance and magnitude, mag: m = −2.5 log₁₀(I ÷ 1 lx) − 13.99, which
/// is V = 0 at 2.54 µlx (Crumey 2014, §1.2, after Allen's value in Cox 2000, §15).
const ILLUMINANCE_ZERO_POINT: f64 = 13.99;

/// Blackwell's point-source coefficients of Crumey's eq. 34 (his eq. 28), for B in cd m⁻² and ΔI
/// in lux: a₁ to a₅.
const A1: f64 = 5.949e-8;
const A2: f64 = -2.389e-7;
const A3: f64 = 2.459e-7;
const A4: f64 = 4.120e-4;
const A5: f64 = -4.225e-4;

/// The background, after the colour correction, at which eq. 34's limit has its local minimum on
/// the dark side of its bump, cd m⁻²: dm/dB = 0 at B = 0.021 567 3 (found by bisection on eq. 34;
/// the limit there is 5.2446 at F = 1.4). The limit then rises with brightness to a local maximum
/// of 5.2732 at 0.0471 and falls back to this value at 0.0650; the running minimum from the dark
/// side holds it at this value across that span (Design note 2).
const DIP_ONSET: f64 = 0.021_567_318_658;

/// CIE 191:2010's MES2 constants: the photopic and scotopic adaptation bounds in cd m⁻², the
/// scotopic luminous efficiency at 555 nm, V′(λ₀) = 683 ÷ 1699, and the regression m = a + b
/// log₁₀ L<sub>mes</sub>.
const MES2_SCOTOPIC_BOUND: f64 = 0.005;
const MES2_PHOTOPIC_BOUND: f64 = 5.0;
const MES2_V_PRIME_555: f64 = 683.0 / 1699.0;
const MES2_A: f64 = 0.7670;
const MES2_B: f64 = 0.3334;
/// MES2's iteration converges to 10⁻¹² well within this many steps for any background; the count
/// bounds the loop, so the weight is a fixed function of its inputs.
const MES2_MAX_ITERATIONS: u32 = 64;

/// The field factors an [`EyeObserver`] accepts.
const FIELD_FACTOR_RANGE: (f64, f64) = (0.1, 100.0);

/// The S/P ratios an [`SpRatio`] accepts.
const SP_RATIO_RANGE: (f64, f64) = (0.01, 100.0);

/// The brightest background a [`SkyBackground`] accepts, cd m⁻².
const MAX_LUMINANCE: f64 = 1e12;

/// CIE 146:2002's validity bounds of the glare angle, degrees: smaller angles are read as 0.1°, and
/// a source beyond 100° adds no glare. (A source at or beyond 90° gives no illuminance in the plane
/// of the eye, so the limit map never reaches the upper bound; see [`veiling_luminance`].)
const GLARE_MIN_ANGLE_DEG: f64 = 0.1;
const GLARE_MAX_ANGLE_DEG: f64 = 100.0;

/// An [`EyeObserver`], an [`SpRatio`], a [`SkyBackground`] or a [`PhotopicWeight`] could not be
/// built from the values given.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildEyeError {
    /// The field factor F is not within 0.1–100.
    FieldFactor,
    /// The age is not finite and non-negative.
    AgeYears,
    /// The pigmentation factor p is not finite and within CIE 146's 0–1.2.
    Pigmentation,
    /// An S/P ratio is not within 0.01–100.
    SpRatio,
    /// A background luminance is not within 0–10¹² cd m⁻².
    Luminance,
    /// A photopic weight is not within 0–1.
    PhotopicWeight,
}

impl fmt::Display for BuildEyeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::FieldFactor => "the field factor is not within 0.1 to 100",
            Self::AgeYears => "the age is not finite and non-negative",
            Self::Pigmentation => "the pigmentation is not finite and within 0 to 1.2",
            Self::SpRatio => "the scotopic-to-photopic ratio is not within 0.01 to 100",
            Self::Luminance => "the background luminance is not within 0 to 1e12 cd/m2",
            Self::PhotopicWeight => "the photopic weight is not within 0 to 1",
        })
    }
}

impl Error for BuildEyeError {}

/// The observer whose eye a limit belongs to: Crumey's field factor F and CIE 146's age and eye
/// pigmentation, which set the glare.
///
/// The defaults are F = 1.4, the keen end of Crumey's 1.4–2.4 for real observing (1.378 fitted
/// for M33; §3.1 and Design note 2), an age of 25 years and a pigmentation of 0.5 (brown eyes; CIE
/// 146:2002 takes 0 for black eyes, 1 for light ones and 1.2 for very light blue-green).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EyeObserver {
    field_factor: f64,
    age_years: f64,
    pigmentation: f64,
}

impl EyeObserver {
    /// The default field factor, 1.4.
    pub const DEFAULT_FIELD_FACTOR: f64 = 1.4;
    /// The default age, 25 years.
    pub const DEFAULT_AGE_YEARS: f64 = 25.0;
    /// The default pigmentation factor, 0.5.
    pub const DEFAULT_PIGMENTATION: f64 = 0.5;

    /// An observer of field factor `field_factor`, age `age_years` and pigmentation factor
    /// `pigmentation`.
    ///
    /// # Errors
    ///
    /// [`BuildEyeError::FieldFactor`] unless the field factor is within 0.1–100 (Crumey's
    /// real-world range is 1.4–2.4, §3.1; the bounds keep every threshold finite),
    /// [`BuildEyeError::AgeYears`] unless the age is finite and non-negative, and
    /// [`BuildEyeError::Pigmentation`] unless the pigmentation is finite and within 0–1.2.
    pub fn new(
        field_factor: f64,
        age_years: f64,
        pigmentation: f64,
    ) -> Result<Self, BuildEyeError> {
        if !(FIELD_FACTOR_RANGE.0..=FIELD_FACTOR_RANGE.1).contains(&field_factor) {
            return Err(BuildEyeError::FieldFactor);
        }
        if !(age_years.is_finite() && age_years >= 0.0) {
            return Err(BuildEyeError::AgeYears);
        }
        if !(pigmentation.is_finite() && (0.0..=1.2).contains(&pigmentation)) {
            return Err(BuildEyeError::Pigmentation);
        }
        Ok(Self {
            field_factor,
            age_years,
            pigmentation,
        })
    }

    /// Crumey's field factor F, which multiplies every threshold: a factor F moves every limit by
    /// −2.5 log₁₀ F.
    #[must_use]
    pub const fn field_factor(&self) -> f64 {
        self.field_factor
    }

    /// The observer's age in years, CIE 146's A.
    #[must_use]
    pub const fn age_years(&self) -> f64 {
        self.age_years
    }

    /// The eye's pigmentation factor, CIE 146's p.
    #[must_use]
    pub const fn pigmentation(&self) -> f64 {
        self.pigmentation
    }
}

impl Default for EyeObserver {
    fn default() -> Self {
        Self {
            field_factor: Self::DEFAULT_FIELD_FACTOR,
            age_years: Self::DEFAULT_AGE_YEARS,
            pigmentation: Self::DEFAULT_PIGMENTATION,
        }
    }
}

/// A scotopic-to-photopic ratio ρ, within 0.01–100 (Crumey's eq. 5; CIE 191:2010's "S/P
/// ratio"): the light's scotopic luminance, weighted by V′(λ) at 1,700 lm W⁻¹, over its photopic
/// luminance, weighted by V(λ) at 683 lm W⁻¹.
///
/// Starlight with no airglow is about 2.26 (Crumey §1.3, from Leinert et al. 1998), an M dwarf
/// lower and an O star higher.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct SpRatio(f64);

impl SpRatio {
    /// Blackwell's 2,850 K light ([`BLACKWELL_SP_RATIO`]).
    pub const BLACKWELL: Self = Self(BLACKWELL_SP_RATIO);
    /// The reference star of B − V = 0.7 ([`REFERENCE_SP_RATIO`]).
    pub const REFERENCE: Self = Self(REFERENCE_SP_RATIO);
    /// The least ratio accepted, 0.01.
    pub const MIN: Self = Self(SP_RATIO_RANGE.0);
    /// The greatest ratio accepted, 100.
    pub const MAX: Self = Self(SP_RATIO_RANGE.1);

    /// The ratio `value`.
    ///
    /// # Errors
    ///
    /// [`BuildEyeError::SpRatio`] unless `value` is within 0.01–100: a light's ratio runs from
    /// about 0.2 for a deep red source to under 4 for the hottest stars, and the bounds keep every
    /// threshold finite.
    pub fn new(value: f64) -> Result<Self, BuildEyeError> {
        if (SP_RATIO_RANGE.0..=SP_RATIO_RANGE.1).contains(&value) {
            Ok(Self(value))
        } else {
            Err(BuildEyeError::SpRatio)
        }
    }

    /// The ratio.
    #[must_use]
    pub const fn value(self) -> f64 {
        self.0
    }
}

/// The background a star is seen against: its photopic luminance, 0–10¹² cd m⁻² (the Sun's disc
/// is 1.6 × 10⁹), and its S/P ratio.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkyBackground {
    luminance: CandelasPerSquareMetre,
    sp_ratio: SpRatio,
}

impl SkyBackground {
    /// The brightest luminance accepted, 10¹² cd m⁻².
    pub const MAX_LUMINANCE: CandelasPerSquareMetre = CandelasPerSquareMetre::new(MAX_LUMINANCE);

    /// A background of photopic luminance `luminance` and S/P ratio `sp_ratio`.
    ///
    /// # Errors
    ///
    /// [`BuildEyeError::Luminance`] unless the luminance is within 0–10¹² cd m⁻².
    pub fn new(
        luminance: CandelasPerSquareMetre,
        sp_ratio: SpRatio,
    ) -> Result<Self, BuildEyeError> {
        let b = luminance.value();
        if (0.0..=MAX_LUMINANCE).contains(&b) {
            Ok(Self {
                luminance,
                sp_ratio,
            })
        } else {
            Err(BuildEyeError::Luminance)
        }
    }

    /// The photopic luminance.
    #[must_use]
    pub const fn luminance(&self) -> CandelasPerSquareMetre {
        self.luminance
    }

    /// The S/P ratio.
    #[must_use]
    pub const fn sp_ratio(&self) -> SpRatio {
        self.sp_ratio
    }
}

/// The luminance of a surface brightness μ<sub>V</sub>: B = 10^((12.58 − μ) ÷ 2.5) cd m⁻² (Crumey
/// 2014, §1.2).
#[must_use]
pub fn luminance(surface_brightness: MagnitudesPerArcsec2) -> CandelasPerSquareMetre {
    CandelasPerSquareMetre::new(math::exp10(
        (SURFACE_BRIGHTNESS_ZERO_POINT - surface_brightness.value()) / 2.5,
    ))
}

/// The surface brightness of a luminance: μ<sub>V</sub> = 12.58 − 2.5 log₁₀(B ÷ 1 cd m⁻²) (Crumey
/// 2014, §1.2).
///
/// `None` for a luminance that is negative or not finite; zero luminance is +∞.
#[must_use]
pub fn surface_brightness(luminance: CandelasPerSquareMetre) -> Option<MagnitudesPerArcsec2> {
    let b = luminance.value();
    (b.is_finite() && b >= 0.0)
        .then(|| MagnitudesPerArcsec2::new(SURFACE_BRIGHTNESS_ZERO_POINT - 2.5 * math::log10(b)))
}

/// MES2's photopic weight m, 0 (fully scotopic) to 1 (fully photopic): the share of a light's
/// effect carried by the cones (CIE 191:2010).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct PhotopicWeight(f64);

impl PhotopicWeight {
    /// A fully scotopic eye, m = 0.
    pub const SCOTOPIC: Self = Self(0.0);
    /// A fully photopic eye, m = 1.
    pub const PHOTOPIC: Self = Self(1.0);

    /// The weight `value`.
    ///
    /// # Errors
    ///
    /// [`BuildEyeError::PhotopicWeight`] unless `value` is within 0–1.
    pub fn new(value: f64) -> Result<Self, BuildEyeError> {
        if (0.0..=1.0).contains(&value) {
            Ok(Self(value))
        } else {
            Err(BuildEyeError::PhotopicWeight)
        }
    }

    /// The weight.
    #[must_use]
    pub const fn value(self) -> f64 {
        self.0
    }
}

/// The photopic weight m of CIE 191:2010's MES2 mesopic system for a background, 0 (fully
/// scotopic) to 1 (fully photopic).
///
/// With L<sub>s</sub> = ρ L<sub>p</sub> the scotopic luminance, MES2 takes m = 0 where
/// L<sub>s</sub> ≤ 0.005 cd m⁻² and m = 1 where L<sub>p</sub> ≥ 5 cd m⁻² (CIE 191:2010; Gao et al.
/// 2017, Optics Express 25, 18365, eqs. 8–10), and otherwise iterates from m₀ = 0.5 the mesopic
/// luminance L<sub>mes</sub> = (m L<sub>p</sub> + (1 − m) L<sub>s</sub> V′(λ₀)) ÷ (m + (1 − m)
/// V′(λ₀)) and the weight m = 0.7670 + 0.3334 log₁₀ L<sub>mes</sub>, clamped to 0–1 (their
/// eqs. 5–7 and 11–13). A starlit background (ρ = 2.26) darker than about μ<sub>V</sub> 19.2 is
/// scotopic.
#[must_use]
pub fn mesopic_weight(background: &SkyBackground) -> PhotopicWeight {
    let photopic = background.luminance.value();
    let scotopic = background.sp_ratio.value() * photopic;
    if scotopic <= MES2_SCOTOPIC_BOUND {
        return PhotopicWeight::SCOTOPIC;
    }
    if photopic >= MES2_PHOTOPIC_BOUND {
        return PhotopicWeight::PHOTOPIC;
    }
    let mut m = 0.5;
    for _ in 0..MES2_MAX_ITERATIONS {
        let mesopic = (m * photopic + (1.0 - m) * scotopic * MES2_V_PRIME_555)
            / (m + (1.0 - m) * MES2_V_PRIME_555);
        let next = if mesopic <= MES2_SCOTOPIC_BOUND {
            0.0
        } else if mesopic >= MES2_PHOTOPIC_BOUND {
            1.0
        } else {
            (MES2_A + MES2_B * math::log10(mesopic)).clamp(0.0, 1.0)
        };
        let converged = (next - m).abs() <= 1e-12;
        m = next;
        if converged {
            break;
        }
    }
    PhotopicWeight(m)
}

/// How much more a light of ratio `sp_ratio` excites the eye than the same photopic luminance of
/// Blackwell's light, at photopic weight `photopic_weight`: the ratio of the two lights' MES2
/// mesopic luminances, (m + (1 − m) ρ V′(λ₀)) ÷ (m + (1 − m) 1.408 V′(λ₀)) (see the
/// [module](self) documentation). It is ρ ÷ 1.408, Crumey's eq. 6, when scotopic and 1 when
/// photopic. A background of several lights, as the limit map's band and the veils of its stars
/// are (R06.T9.c), is their photopic luminances' sum at the S/P ratio of their scotopic light over
/// it: this weighting of each light at the whole background's m, since MES2 is linear in the light
/// at a fixed m.
#[must_use]
pub fn blackwell_equivalent_factor(sp_ratio: SpRatio, photopic_weight: PhotopicWeight) -> f64 {
    let m = photopic_weight.0;
    let rods = (1.0 - m) * MES2_V_PRIME_555;
    (m + rods * sp_ratio.value()) / (m + rods * BLACKWELL_SP_RATIO)
}

/// Eq. 34's threshold at F = 1, lux, for a background already in Blackwell's light.
fn eq34(b: f64) -> f64 {
    let root = math::powf_positive(b, 0.25);
    let half = root * root;
    let three_quarters = half * root;
    let bracket = (A1 * half + A2 * three_quarters + A3 * b).sqrt() + A4 * root + A5 * half;
    bracket * bracket
}

/// The threshold illuminance at the eye of the reference star ([`REFERENCE_SP_RATIO`]) against
/// `background`: Crumey's eq. 34, ΔI = F (√(a₁B^½ + a₂B^¾ + a₃B) + a₄B^¼ + a₅B^½)² lux, at the
/// background's colour-corrected luminance.
///
/// The background is taken to Blackwell's light (the [module](self) documentation) and read no
/// darker than [`DARKEST_BACKGROUND`]; then, past the dip's onset at 0.0216 cd m⁻², the threshold
/// is held at no less than its value there: the running minimum of the limit from the dark side,
/// which removes eq. 34's dip and is exact, since the colour-corrected luminance rises with the
/// luminance.
#[must_use]
pub fn threshold_illuminance(eye: &EyeObserver, background: &SkyBackground) -> Lux {
    let m = mesopic_weight(background);
    let equivalent = (background.luminance.value()
        * blackwell_equivalent_factor(background.sp_ratio, m))
    .max(DARKEST_BACKGROUND.value());
    let threshold = if equivalent > DIP_ONSET {
        eq34(equivalent).max(eq34(DIP_ONSET))
    } else {
        eq34(equivalent)
    };
    Lux::new(eye.field_factor * threshold)
}

/// The V magnitude of a star of illuminance `illuminance` at the eye: −2.5 log₁₀(I ÷ 1 lx) −
/// 13.99 (Crumey 2014, §1.2).
///
/// `None` for an illuminance that is not finite and positive.
#[must_use]
pub fn magnitude_of_illuminance(illuminance: Lux) -> Option<Magnitudes> {
    let i = illuminance.value();
    (i.is_finite() && i > 0.0)
        .then(|| Magnitudes::new(-2.5 * math::log10(i) - ILLUMINANCE_ZERO_POINT))
}

/// The illuminance at the eye of a star of V magnitude `v`: 10^(−0.4 (V + 13.99)) lux, so V = 0
/// gives 2.54 µlx.
#[must_use]
pub fn illuminance_of_magnitude(v: Magnitudes) -> Lux {
    Lux::new(math::exp10(-0.4 * (v.value() + ILLUMINANCE_ZERO_POINT)))
}

/// The faintest V magnitude of the reference star the observer sees against `background`:
/// −2.5 log₁₀ ΔI − 13.99, with ΔI from [`threshold_illuminance`]. A star of another colour is
/// seen to this plus its [`star_colour_offset`].
///
/// With F = 1.4 and a starlit background (ρ₀ = 2.26) it is 6.60 at μ<sub>V</sub> 22.4, 7.41 at 24.3
/// and 7.99 from about 25.6 on.
///
/// # Panics
///
/// Never: the threshold is finite and positive for every background, since the colour-corrected
/// luminance lies between [`DARKEST_BACKGROUND`] and 10¹⁴ cd m⁻² and the field factor within
/// 0.1–100, by construction of [`SkyBackground`], [`SpRatio`] and [`EyeObserver`].
///
/// # Examples
///
/// ```
/// use hyperion_sim::sky::eye::{EyeObserver, SkyBackground, SpRatio, luminance, naked_eye_limit};
/// use hyperion_sim::units::MagnitudesPerArcsec2;
///
/// // The band near the Sun, μ_V 22.4 of starlight with no airglow.
/// let band = SkyBackground::new(
///     luminance(MagnitudesPerArcsec2::new(22.4)),
///     SpRatio::new(2.26)?,
/// )?;
/// let limit = naked_eye_limit(&EyeObserver::default(), &band);
/// assert!((limit.value() - 6.60).abs() < 0.03);
/// # Ok::<(), hyperion_sim::sky::eye::BuildEyeError>(())
/// ```
#[must_use]
pub fn naked_eye_limit(eye: &EyeObserver, background: &SkyBackground) -> Magnitudes {
    magnitude_of_illuminance(threshold_illuminance(eye, background))
        .expect("eq. 34's threshold is finite and positive for every clamped background")
}

/// How much fainter a star of ratio `star` is seen than the reference star against `background`:
/// 2.5 log₁₀(ρ★ ÷ 2.297) in a scotopic background, fading to zero with the photopic weight in a
/// mesopic one (Crumey eq. 15 relative to [`REFERENCE_SP_RATIO`]; the [module](self)
/// documentation). Positive for a star hotter than the reference.
#[must_use]
pub fn star_colour_offset(star: SpRatio, background: &SkyBackground) -> Magnitudes {
    let m = mesopic_weight(background);
    Magnitudes::new(
        2.5 * math::log10(
            blackwell_equivalent_factor(star, m)
                / blackwell_equivalent_factor(SpRatio::REFERENCE, m),
        ),
    )
}

/// The veiling luminance an illuminance `illuminance` at the eye from a source at angle `angle`
/// from the line of sight adds to the background: CIE 146:2002's general disability glare
/// equation (Vos 2003),
///
/// L<sub>veil</sub> = E [10 ÷ θ³ + (5 ÷ θ² + 0.1 p ÷ θ)(1 + (A ÷ 62.5)⁴) + 0.0025 p],
///
/// θ in degrees, A the observer's age and p their pigmentation. Below 0.1°, the equation's lower
/// bound, θ is read as 0.1°; beyond 100°, its upper bound, the source adds nothing.
///
/// E is the glare illuminance in the plane of the observer's eye, the pupil's plane, which is
/// perpendicular to the line of sight. CIE 146:2002's abstract gives the veil of "a point glare
/// source at an angle Theta (in degrees) to the line of sight that gives rise to an illuminance E
/// glare (in lx) in the plane of the observer's eye". IJspeert, de Waard, van den Berg and de Jong
/// (1990, Vision Res. 30, 699), whose data set CIE's age term, take "the illuminance caused by the
/// point source at the pupil plane", and Stiles and Crawford (1937, Proc. R. Soc. B 122, 255) take
/// the illumination on the pupillary plane. A source of illuminance E normal to its own direction, at
/// θ from the line of sight, therefore gives E cos θ, which the caller passes. A source at or
/// beyond 90° lies behind the eye's plane and gives none there, so the limit map takes none
/// (R06.T9.h, decided 2026-10-06, `decision-r06-t9c-glare.md`), although CIE's range runs to 100°
/// for light that reaches the retina past the cornea and through the eye wall.
///
/// `None` for an illuminance or an angle that is negative or not finite.
///
/// # Examples
///
/// The glare of a bright star 1° from a faint one is added to the band behind the faint one, in
/// the star's own light (its scotopic veil the photopic one times its S/P ratio), before its
/// limit is taken, as the limit map adds it (R06.T9.c); a red star is then seen to a brighter
/// magnitude than the reference star:
///
/// ```
/// use hyperion_sim::math;
/// use hyperion_sim::sky::eye::{
///     EyeObserver, SkyBackground, SpRatio, illuminance_of_magnitude, luminance, naked_eye_limit,
///     star_colour_offset, veiling_luminance,
/// };
/// use hyperion_sim::units::consts::RADIANS_PER_DEGREE;
/// use hyperion_sim::units::{Degrees, Magnitudes, MagnitudesPerArcsec2};
///
/// let eye = EyeObserver::default();
/// let starlight = 2.26;
/// let band = SkyBackground::new(
///     luminance(MagnitudesPerArcsec2::new(22.4)),
///     SpRatio::new(starlight)?,
/// )?;
/// // A V = −1.5 star of ratio 2.6, 1° away: its illuminance in the plane of the eye, E cos 1°,
/// // then the band's and the veil's light together.
/// let glare = illuminance_of_magnitude(Magnitudes::new(-1.5)) * math::cos(RADIANS_PER_DEGREE);
/// let veil = veiling_luminance(&eye, glare, Degrees::new(1.0)).ok_or("a valid glare")?;
/// let total = band.luminance() + veil;
/// let ratio = (band.luminance().value() * starlight + veil.value() * 2.6) / total.value();
/// let glared = SkyBackground::new(total, SpRatio::new(ratio)?)?;
/// let limit = naked_eye_limit(&eye, &glared);
/// assert!(limit < naked_eye_limit(&eye, &band));
/// let red = limit + star_colour_offset(SpRatio::new(1.2)?, &glared);
/// assert!(red < limit);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn veiling_luminance(
    eye: &EyeObserver,
    illuminance: Lux,
    angle: Degrees,
) -> Option<CandelasPerSquareMetre> {
    let e = illuminance.value();
    let theta = angle.value();
    if !(e.is_finite() && e >= 0.0 && theta.is_finite() && theta >= 0.0) {
        return None;
    }
    if theta > GLARE_MAX_ANGLE_DEG {
        return Some(CandelasPerSquareMetre::ZERO);
    }
    let theta = theta.max(GLARE_MIN_ANGLE_DEG);
    let p = eye.pigmentation;
    let age = eye.age_years / 62.5;
    let age_term = 1.0 + age * age * age * age;
    let per_lux = 10.0 / (theta * theta * theta)
        + (5.0 / (theta * theta) + 0.1 * p / theta) * age_term
        + 0.0025 * p;
    Some(CandelasPerSquareMetre::new(e * per_lux))
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;

    fn starlit(mu: f64) -> SkyBackground {
        SkyBackground::new(
            luminance(MagnitudesPerArcsec2::new(mu)),
            SpRatio::new(2.26).unwrap(),
        )
        .unwrap()
    }

    fn limit_at(mu: f64) -> f64 {
        naked_eye_limit(&EyeObserver::default(), &starlit(mu)).value()
    }

    /// Crumey's scotopic eq. 53, ΔI = F (6.505 × 10⁻⁴ B^¼ − 8.461 × 10⁻⁴ B^½)², with no colour
    /// correction, as a limit.
    fn eq53_limit(b: f64, f: f64) -> f64 {
        let root = math::powf_positive(b, 0.25);
        let bracket = 6.505e-4 * root - 8.461e-4 * root * root;
        -2.5 * math::log10(f * bracket * bracket) - ILLUMINANCE_ZERO_POINT
    }

    fn eq34_limit(b: f64, f: f64) -> f64 {
        -2.5 * math::log10(f * eq34(b)) - ILLUMINANCE_ZERO_POINT
    }

    #[test]
    fn eq53_gives_crumeys_dark_sky_figure_and_eq34_agrees_above_mu_20() {
        let b = luminance(MagnitudesPerArcsec2::new(21.83)).value();
        assert!((b - 2e-4).abs() < 1e-6, "{b}");
        // Crumey §3.1: "Eq. 53 gives a magnitude limit m₀ = 6.93 − 2.5 log F".
        assert!((eq53_limit(b, 1.0) - 6.93).abs() < 0.005);
        // The plan's "within 0.02 mag for μ ≥ 20" holds from μ 20.6; at μ 20 the two differ by
        // 0.026 (a correction recorded in the plan's Risks).
        for step in 0..=80 {
            let mu = 20.0 + 0.1 * f64::from(step);
            let b = luminance(MagnitudesPerArcsec2::new(mu)).value();
            let gap = (eq34_limit(b, 1.0) - eq53_limit(b, 1.0)).abs();
            let bound = if mu < 20.55 { 0.03 } else { 0.02 };
            assert!(gap < bound, "μ {mu}: eq. 34 and eq. 53 differ by {gap}");
        }
    }

    #[test]
    fn starlit_limits_follow_the_plan_figures() {
        let cases = [
            (22.4, 6.60, 0.03),
            (24.3, 7.41, 0.01),
            (25.0, 7.72, 0.01),
            (26.0, 7.99, 0.01),
            (27.0, 7.99, 0.01),
            (21.0, 6.07, 0.01),
            (19.7, 5.65, 0.01),
            (18.8, 5.42, 0.01),
            (17.5, 5.25, 0.01),
            (16.5, 5.24, 0.01),
        ];
        for (mu, expected, tolerance) in cases {
            let limit = limit_at(mu);
            assert!(
                (limit - expected).abs() <= tolerance,
                "μ {mu}: limit {limit}, expected {expected} ± {tolerance}"
            );
        }
    }

    #[test]
    fn the_limit_is_clamped_at_crumeys_zero_background() {
        // Crumey's ζ = 1.150 × 10⁻⁹ lx at F = 1.4 (§3.2, eq. 71); eq. 34 at 10⁻⁵ gives 7.987.
        let at_clamp = limit_at(26.0);
        let zeta = -2.5 * math::log10(1.4 * 1.150e-9) - ILLUMINANCE_ZERO_POINT;
        assert!((at_clamp - zeta).abs() < 0.01, "{at_clamp} against {zeta}");
        assert!(limit_at(25.5) < at_clamp);
        for mu in [27.0, 30.0, 40.0] {
            assert_same_bits(limit_at(mu), at_clamp);
        }
        let black =
            SkyBackground::new(CandelasPerSquareMetre::ZERO, SpRatio::new(2.26).unwrap()).unwrap();
        assert_same_bits(
            naked_eye_limit(&EyeObserver::default(), &black).value(),
            at_clamp,
        );
    }

    #[test]
    fn the_dip_is_held_at_its_onset() {
        // The plan's closed form: 5.2446 − 2.5 log₁₀(F ÷ 1.4) across the dip.
        let plateau = eq34_limit(DIP_ONSET, 1.4);
        assert!((plateau - 5.2446).abs() < 1e-4, "{plateau}");
        // dm/dB = 0 at the onset, to the bisection's precision.
        let below = eq34_limit(DIP_ONSET * (1.0 - 1e-6), 1.0);
        let above = eq34_limit(DIP_ONSET * (1.0 + 1e-6), 1.0);
        assert!((below - above).abs() < 1e-10);
        // The dip's far end, 0.0650 cd m⁻², is where eq. 34 falls back to the plateau.
        assert!(eq34_limit(0.064, 1.4) > plateau);
        assert!(eq34_limit(0.066, 1.4) < plateau);
    }

    #[test]
    fn a_field_factor_of_two_costs_0_387_mag() {
        let two = EyeObserver::new(2.0, 25.0, 0.5).unwrap();
        for mu in [17.0, 21.83, 24.3, 30.0] {
            let cost = limit_at(mu) - naked_eye_limit(&two, &starlit(mu)).value();
            assert!((cost - 0.387).abs() < 5e-4, "μ {mu}: {cost}");
        }
    }

    #[test]
    fn the_limit_is_monotone_from_mu_15_to_27() {
        let mut previous = limit_at(15.0);
        for step in 1..=1200 {
            let mu = 15.0 + 0.01 * f64::from(step);
            let limit = limit_at(mu);
            assert!(
                limit >= previous,
                "μ {mu}: {limit} shallower than {previous}"
            );
            previous = limit;
        }
    }

    #[test]
    fn the_colour_offset_is_zero_at_the_reference_and_positive_for_a_hotter_star() {
        for mu in [16.0, 18.0, 22.4, 25.0] {
            let sky = starlit(mu);
            let reference = star_colour_offset(SpRatio::REFERENCE, &sky).value();
            assert!(reference.abs() < 1e-15, "μ {mu}: {reference}");
            assert!(star_colour_offset(SpRatio::new(3.0).unwrap(), &sky).value() > 0.0);
            assert!(star_colour_offset(SpRatio::new(1.5).unwrap(), &sky).value() < 0.0);
        }
        // Scotopic: exactly 2.5 log₁₀(ρ★ ÷ 2.297).
        let dark = starlit(24.0);
        let offset = star_colour_offset(SpRatio::new(3.2).unwrap(), &dark).value();
        assert!((offset - 2.5 * math::log10(3.2 / 2.297)).abs() < 1e-12);
    }

    #[test]
    fn the_mesopic_weight_runs_from_scotopic_to_photopic() {
        assert_eq!(mesopic_weight(&starlit(24.0)), PhotopicWeight::SCOTOPIC);
        assert_eq!(mesopic_weight(&starlit(19.3)), PhotopicWeight::SCOTOPIC);
        assert!(mesopic_weight(&starlit(19.0)).value() > 0.0);
        let mesopic = mesopic_weight(&starlit(16.0)).value();
        assert!(mesopic > 0.0 && mesopic < 1.0, "{mesopic}");
        let day = SkyBackground::new(
            CandelasPerSquareMetre::new(100.0),
            SpRatio::new(2.26).unwrap(),
        )
        .unwrap();
        assert_eq!(mesopic_weight(&day), PhotopicWeight::PHOTOPIC);
        // The factor is Crumey's eq. 6 when scotopic and 1 when photopic.
        let star = SpRatio::new(0.8).unwrap();
        let scotopic = blackwell_equivalent_factor(star, PhotopicWeight::SCOTOPIC);
        assert!((scotopic - 0.8 / 1.408).abs() < 1e-15);
        assert!((blackwell_equivalent_factor(star, PhotopicWeight::PHOTOPIC) - 1.0).abs() < 1e-15);
        assert_eq!(
            PhotopicWeight::new(1.01),
            Err(BuildEyeError::PhotopicWeight)
        );
        assert_eq!(
            PhotopicWeight::new(f64::NAN),
            Err(BuildEyeError::PhotopicWeight)
        );
    }

    /// Design note 4's formula written out again, independently of the code.
    fn cie_glare(e: f64, theta: f64, age: f64, p: f64) -> f64 {
        e * (10.0 / math::powi(theta, 3)
            + (5.0 / math::powi(theta, 2) + 0.1 * p / theta) * (1.0 + math::powi(age / 62.5, 4))
            + 0.0025 * p)
    }

    #[test]
    fn the_veiling_luminance_follows_cie_146() {
        let eye = EyeObserver::default();
        let e = illuminance_of_magnitude(Magnitudes::ZERO);
        assert!((e.value() - 2.54e-6).abs() < 0.005e-6);
        for theta in [0.1, 1.0, 10.0, 100.0] {
            let veil = veiling_luminance(&eye, e, Degrees::new(theta))
                .unwrap()
                .value();
            let expected = cie_glare(e.value(), theta, 25.0, 0.5);
            assert!(
                ((veil - expected) / expected).abs() < 1e-9,
                "θ {theta}: {veil} against {expected}"
            );
        }
        let near = veiling_luminance(&eye, e, Degrees::new(0.1)).unwrap();
        let far = veiling_luminance(&eye, e, Degrees::new(1.0)).unwrap();
        assert!(far < near);
        assert_eq!(
            veiling_luminance(&eye, e, Degrees::new(0.01)),
            Some(near),
            "below 0.1° is read as 0.1°"
        );
        assert_eq!(
            veiling_luminance(&eye, e, Degrees::new(120.0)),
            Some(CandelasPerSquareMetre::ZERO)
        );
    }

    #[test]
    fn invalid_inputs_are_refused() {
        let eye = EyeObserver::default();
        assert_eq!(
            veiling_luminance(&eye, Lux::new(-1.0), Degrees::new(1.0)),
            None
        );
        assert_eq!(
            veiling_luminance(&eye, Lux::new(f64::NAN), Degrees::new(1.0)),
            None
        );
        assert_eq!(
            veiling_luminance(&eye, Lux::new(1.0), Degrees::new(f64::NAN)),
            None
        );
        assert_eq!(
            veiling_luminance(&eye, Lux::new(1.0), Degrees::new(-1.0)),
            None
        );
        assert_eq!(
            EyeObserver::new(5e-324, 25.0, 0.5),
            Err(BuildEyeError::FieldFactor)
        );
        assert_eq!(
            EyeObserver::new(1e307, 25.0, 0.5),
            Err(BuildEyeError::FieldFactor)
        );
        assert_eq!(SpRatio::new(1e300), Err(BuildEyeError::SpRatio));
        assert_eq!(
            SkyBackground::new(CandelasPerSquareMetre::new(1e300), SpRatio::REFERENCE),
            Err(BuildEyeError::Luminance)
        );
        assert_eq!(
            surface_brightness(CandelasPerSquareMetre::new(f64::INFINITY)),
            None
        );
        assert_eq!(
            EyeObserver::new(0.0, 25.0, 0.5),
            Err(BuildEyeError::FieldFactor)
        );
        assert_eq!(
            EyeObserver::new(f64::NAN, 25.0, 0.5),
            Err(BuildEyeError::FieldFactor)
        );
        assert_eq!(
            EyeObserver::new(1.4, -1.0, 0.5),
            Err(BuildEyeError::AgeYears)
        );
        assert_eq!(
            EyeObserver::new(1.4, 25.0, 1.3),
            Err(BuildEyeError::Pigmentation)
        );
        assert_eq!(SpRatio::new(0.0), Err(BuildEyeError::SpRatio));
        assert_eq!(SpRatio::new(f64::INFINITY), Err(BuildEyeError::SpRatio));
        assert_eq!(
            SkyBackground::new(CandelasPerSquareMetre::new(-1e-6), SpRatio::REFERENCE),
            Err(BuildEyeError::Luminance)
        );
        assert_eq!(
            SkyBackground::new(CandelasPerSquareMetre::new(f64::NAN), SpRatio::REFERENCE),
            Err(BuildEyeError::Luminance)
        );
        assert_eq!(surface_brightness(CandelasPerSquareMetre::new(-1.0)), None);
        assert_eq!(
            surface_brightness(CandelasPerSquareMetre::new(f64::NAN)),
            None
        );
        assert_eq!(magnitude_of_illuminance(Lux::ZERO), None);
    }

    #[test]
    fn the_accepted_ranges_are_their_bounds() {
        assert_eq!(SpRatio::new(SpRatio::MIN.value()), Ok(SpRatio::MIN));
        assert_eq!(SpRatio::new(SpRatio::MAX.value()), Ok(SpRatio::MAX));
        assert_eq!(
            SpRatio::new(0.999 * SpRatio::MIN.value()),
            Err(BuildEyeError::SpRatio)
        );
        assert_eq!(
            SpRatio::new(1.001 * SpRatio::MAX.value()),
            Err(BuildEyeError::SpRatio)
        );
        let brightest = SkyBackground::MAX_LUMINANCE;
        assert!(SkyBackground::new(brightest, SpRatio::REFERENCE).is_ok());
        assert_eq!(
            SkyBackground::new(brightest * 1.001, SpRatio::REFERENCE),
            Err(BuildEyeError::Luminance)
        );
    }

    #[test]
    fn surface_brightness_and_luminance_invert() {
        for mu in [15.0, 21.83, 24.3, 27.0] {
            let back = surface_brightness(luminance(MagnitudesPerArcsec2::new(mu)))
                .unwrap()
                .value();
            assert!((back - mu).abs() < 1e-12);
        }
    }

    #[test]
    fn the_default_observer_is_f_1_4_aged_25_with_brown_eyes() {
        assert_eq!(
            EyeObserver::default(),
            EyeObserver::new(1.4, 25.0, 0.5).unwrap()
        );
    }

    #[test]
    fn the_limit_is_finite_at_the_extremes_of_every_input() {
        let extremes = [
            (0.1, 0.0, 0.01),
            (0.1, MAX_LUMINANCE, 100.0),
            (100.0, 0.0, 100.0),
            (100.0, MAX_LUMINANCE, 0.01),
        ];
        for (f, b, rho) in extremes {
            let eye = EyeObserver::new(f, 25.0, 0.5).unwrap();
            let sky =
                SkyBackground::new(CandelasPerSquareMetre::new(b), SpRatio::new(rho).unwrap())
                    .unwrap();
            assert!(naked_eye_limit(&eye, &sky).value().is_finite());
        }
    }

    /// The default eye is the cross-language fixture's, which the client's `DEFAULT_EYE_OBSERVER`
    /// reads too (rendering plan R06, T13.e).
    #[test]
    fn the_default_eye_is_the_fixtures() {
        let fixture = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/protocol/fixtures/eye_observer.json"
        ));
        let field = |name: &str| -> f64 {
            fixture
                .split(&format!("\"{name}\":"))
                .nth(1)
                .and_then(|rest| rest.split(['\n', '}']).next())
                .and_then(|v| v.trim().trim_end_matches(',').parse().ok())
                .unwrap_or_else(|| panic!("the fixture holds {name}"))
        };
        let eye = EyeObserver::default();
        assert_same_bits(eye.field_factor(), field("field_factor"));
        assert_same_bits(eye.age_years(), field("age_years"));
        assert_same_bits(eye.pigmentation(), field("pigmentation"));
    }
}
