//! Every metallicity-dependent coefficient of the formulae, evaluated once per Z (plan 06,
//! P06.T4.b).
//!
//! [`ZCoeffs::new`] evaluates the Appendix polynomials of Hurley, Pols and Tout (2000, MNRAS 315,
//! 543; "HPT") in ζ = log₁₀(Z ÷ 0.02), applies the special cases and derived forms the Appendix
//! lists after each table, evaluates Tout et al.'s (1996) zero-age main-sequence coefficients, and
//! the critical masses of HPT section 5 (equations 1–3). Everything else in [`sse`](super) reads
//! the coefficients from here and has no Z dependence of its own.
//!
//! The arithmetic form is output: each polynomial is evaluated by Horner's rule from its highest
//! power, every power through [`math`], and every clamp in the order the Appendix prints it.

use crate::math;
use crate::stellar::composition::{Z_FIT_MAX, Z_FIT_MIN, Z_SOLAR};
use crate::units::{MetalFraction, SolarMasses};

use super::coeffs_data::{A, B, ZAMS_L, ZAMS_R};

/// A polynomial α + βζ + γζ² + ηζ³ + µζ⁴ by Horner's rule.
#[must_use]
fn poly(c: &[f64; 5], zeta: f64) -> f64 {
    c[0] + zeta * (c[1] + zeta * (c[2] + zeta * (c[3] + zeta * c[4])))
}

/// Every metallicity-dependent coefficient of the analytic evolution formulae at one Z.
///
/// Built once per metallicity and read by every phase of [`sse`](super). The coefficients aₙ and
/// bₙ are those of HPT's Appendix after its special cases; row 0 and the numbers the paper never
/// uses (b8, b35) are zero, and b50, which depends on mass, is evaluated where it is used.
///
/// # Examples
///
/// The critical masses of HPT section 5 at solar metallicity: a hook appears on the main sequence
/// above about 1.02 M☉, helium ignites in a flash below about 2.0 M☉, and on the giant branch
/// below about 13 M☉.
///
/// ```
/// use hyperion_sim::stellar::sse::ZCoeffs;
/// use hyperion_sim::units::MetalFraction;
///
/// let solar = ZCoeffs::new(MetalFraction::new(0.02));
/// assert!((solar.m_hook().value() - 1.0185).abs() < 1e-12);
/// assert!((solar.m_hef().value() - 1.995).abs() < 1e-12);
/// assert!((solar.m_fgb().value() - 13.032).abs() < 1e-3);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ZCoeffs {
    z: MetalFraction,
    zeta: f64,
    a: [f64; 82],
    b: [f64; 58],
    zams_l: [f64; 7],
    zams_r: [f64; 9],
    m_hook: SolarMasses,
    m_hef: SolarMasses,
    m_fgb: SolarMasses,
    /// A = min(b4 M^−b5, b6 M^−b7) of the giant's radius (HPT equation 46).
    giant_radius_scale: LesserPowerLaw,
    /// A = min(b51 M^−b52, b53 M^−b54) of the asymptotic giant's radius from `M_HeF` up (HPT
    /// equation 74).
    agb_radius_scale: LesserPowerLaw,
    /// min(a34 ÷ M^a35, a36 ÷ M^a37) of the luminosity hook's amplitude (HPT equation 16).
    hook_luminosity_scale: LesserPowerLaw,
    /// 0.0258 (1 + X)^(5/3) of the degenerate radius floor (HPT equation 24), R☉ M☉^⅓.
    degenerate_radius: f64,
}

impl ZCoeffs {
    /// Evaluates every coefficient at the metal fraction `z_fit`, clamped to the formulae's range
    /// 0.0001–0.03 as [`Composition::z_fit`](crate::stellar::Composition::z_fit) clamps it, so
    /// that an unclamped Z cannot extrapolate the fits.
    ///
    /// # Panics
    ///
    /// If `z_fit` is NaN, whose clamp would carry NaN into every coefficient.
    #[must_use]
    pub fn new(z_fit: MetalFraction) -> Self {
        assert!(!z_fit.value().is_nan(), "a metal fraction is a number");
        let z = z_fit.value().clamp(Z_FIT_MIN.value(), Z_FIT_MAX.value());
        let z_fit = MetalFraction::new(z);
        let zeta = math::log10(z / Z_SOLAR.value());
        let a = a_coefficients(z, zeta);
        let b = b_coefficients(z, zeta);
        let zams_l = core::array::from_fn(|i| poly(&ZAMS_L[i], zeta));
        let zams_r = core::array::from_fn(|i| poly(&ZAMS_R[i], zeta));
        // HPT equations 1–3; see `m_fgb` for the form of equation 3.
        let m_hook = 1.0185 + zeta * (0.16015 + zeta * 0.0892);
        let m_hef = 1.995 + zeta * (0.25 + zeta * 0.087);
        let m_fgb = 13.048 * math::powf_positive(z / 0.02, 0.06)
            / (1.0 + 0.0012 * math::powf_positive(0.02 / z, 1.27));
        let mut coeffs = Self {
            z: z_fit,
            zeta,
            a,
            b,
            zams_l,
            zams_r,
            m_hook: SolarMasses::new(m_hook),
            m_hef: SolarMasses::new(m_hef),
            m_fgb: SolarMasses::new(m_fgb),
            giant_radius_scale: LesserPowerLaw::new(PowerForm::Product, [b[4], b[5]], [b[6], b[7]]),
            agb_radius_scale: LesserPowerLaw::new(
                PowerForm::Product,
                [b[51], b[52]],
                [b[53], b[54]],
            ),
            hook_luminosity_scale: LesserPowerLaw::new(
                PowerForm::Quotient,
                [a[34], a[35]],
                [a[36], a[37]],
            ),
            // `powf`: `stellar::substellar` pins its cooling fits to this floor at 0.1 M☉ and
            // computes it again, outside `sse`, with `powf` (P06.T13); once per Z, it costs nothing.
            degenerate_radius: 0.0258 * math::powf(1.0 + super::ms::hydrogen(z), 5.0 / 3.0),
        };
        // b46 = −b46 log₁₀(`M_HeF` ÷ `M_FGB`) needs the critical masses (Appendix, after b49).
        coeffs.b[46] = -coeffs.b[46] * math::log10(m_hef / m_fgb);
        coeffs
    }

    /// The metal fraction the coefficients were evaluated at.
    #[must_use]
    pub const fn z(&self) -> MetalFraction {
        self.z
    }

    /// ζ = log₁₀(Z ÷ 0.02).
    #[must_use]
    pub const fn zeta(&self) -> f64 {
        self.zeta
    }

    /// The initial mass above which the main sequence has a hook, `M_hook` (HPT equation 1):
    /// 1.0185 + 0.16015ζ + 0.0892ζ², about 1.02 M☉ at Z = 0.02.
    #[must_use]
    pub const fn m_hook(&self) -> SolarMasses {
        self.m_hook
    }

    /// The largest initial mass that ignites helium in a degenerate flash, `M_HeF` (HPT equation 2):
    /// 1.995 + 0.25ζ + 0.087ζ², about 2.0 M☉ at Z = 0.02.
    #[must_use]
    pub const fn m_hef(&self) -> SolarMasses {
        self.m_hef
    }

    /// The largest initial mass that ignites helium on the first giant branch, `M_FGB` (HPT
    /// equation 3), about 13.0 M☉ at Z = 0.02.
    ///
    /// Evaluated as the paper prints it, 13.048 (Z ÷ 0.02)^0.06 ÷ (1 + 0.0012 (0.02 ÷ Z)^1.27).
    /// The published SSE code writes 16.5 Z^0.06 ÷ (1 + (10⁻⁴ ÷ Z)^1.27), of which the paper
    /// rounds the constants (16.5 × 0.02^0.06 = 13.0479, 0.005^1.27 = 0.001196); the two differ by
    /// at most 0.18%, at Z = 10⁻⁴. Against the code that moves the radius of a gap star between
    /// `M_FGB` and 12 M☉ by up to 1.3 × 10⁻³ dex (5 M☉ at Z = 10⁻⁴, through equation 50's µ) and
    /// `L_min,He` by up to 8 × 10⁻⁵ dex, well inside P06.T12.b's 0.02 dex.
    #[must_use]
    pub const fn m_fgb(&self) -> SolarMasses {
        self.m_fgb
    }

    /// Coefficient aₙ of HPT's Appendix, 1 ≤ n ≤ 81, after its special cases.
    #[must_use]
    pub(crate) const fn a(&self, n: usize) -> f64 {
        self.a[n]
    }

    /// Coefficient bₙ of HPT's Appendix, 1 ≤ n ≤ 57, after its special cases.
    #[must_use]
    pub(crate) const fn b(&self, n: usize) -> f64 {
        self.b[n]
    }

    /// The power law of HPT equation 21a for the radius α coefficient at mass `m` (M☉), the one copy
    /// that both a64's special case and the main sequence's αR use.
    #[must_use]
    pub(crate) fn alpha_r_power_law(&self, m: f64) -> f64 {
        alpha_r_power_law(&self.a, m)
    }

    /// The giant's radius scale A = min(b4 M^−b5, b6 M^−b7) of HPT equation 46.
    #[must_use]
    pub(crate) const fn giant_radius_scale(&self) -> &LesserPowerLaw {
        &self.giant_radius_scale
    }

    /// The asymptotic giant's radius scale A = min(b51 M^−b52, b53 M^−b54) of HPT equation 74 from
    /// `M_HeF` up.
    #[must_use]
    pub(crate) const fn agb_radius_scale(&self) -> &LesserPowerLaw {
        &self.agb_radius_scale
    }

    /// The luminosity hook's amplitude scale min(a34 ÷ M^a35, a36 ÷ M^a37) of HPT equation 16.
    #[must_use]
    pub(crate) const fn hook_luminosity_scale(&self) -> &LesserPowerLaw {
        &self.hook_luminosity_scale
    }

    /// 0.0258 (1 + X)^(5/3), R☉ M☉^⅓, the degenerate radius floor of HPT equation 24 times M^⅓.
    #[must_use]
    pub(crate) const fn degenerate_radius(&self) -> f64 {
        self.degenerate_radius
    }

    /// Tout et al.'s (1996) zero-age main-sequence luminosity coefficients α, β, γ, δ, ε, ζ, η.
    #[must_use]
    pub(crate) const fn zams_l(&self) -> &[f64; 7] {
        &self.zams_l
    }

    /// Tout et al.'s (1996) zero-age main-sequence radius coefficients θ, ι, κ, λ, µ, ν, ξ, ο, π.
    #[must_use]
    pub(crate) const fn zams_r(&self) -> &[f64; 9] {
        &self.zams_r
    }
}

/// The relative distance from a crossing of two power laws within which [`LesserPowerLaw::at`] and
/// [`lesser_of_crossing_laws`] evaluate both laws. Two laws whose exponents differ by δe stand in
/// the ratio (x ÷ x*)^δe near their crossing x*, which at this distance is 1 ± δe × 10⁻⁹ (at least
/// 10⁻¹² above [`MIN_EXPONENT_GAP`]), while [`math::powf_positive`] errs by 2.2 × 10⁻¹⁶ ×
/// (1 + 1.5 |e ln x|), under 2 × 10⁻¹⁴ for these laws, and the crossing itself by a few ulps: the
/// lesser law is certain outside it, so the laws are only both evaluated where it is not.
const CROSSING_MARGIN: f64 = 1e-9;

/// The smallest difference of exponents for which [`LesserPowerLaw`] uses its crossing: below it the
/// ratio near the crossing is too close to 1 for [`CROSSING_MARGIN`] to decide, and both laws are
/// evaluated everywhere.
const MIN_EXPONENT_GAP: f64 = 1e-3;

/// min(k₁ x^−e₁, k₂ x^−e₂), a coefficient of HPT's formulae, which evaluates only the lesser law
/// where it is certain which that is (plan 06's integrator speed: each `pow` costs about seven calls
/// of `math::exp`, and a track evaluates these at every step of its phases).
///
/// The result is the printed `min` of the two laws bit for bit, in the arithmetic form the formula
/// is written in ([`PowerForm`]): away from the crossing x* the lesser law's own value is what `min`
/// would return, and within [`CROSSING_MARGIN`] of it both are evaluated and `min` decides.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LesserPowerLaw {
    form: PowerForm,
    first: [f64; 2],
    second: [f64; 2],
    /// Where the laws cross, and whether the first is the lesser below it; `None` where the
    /// crossing does not decide (see [`LesserPowerLaw::new`]).
    crossing: Option<(f64, bool)>,
}

impl LesserPowerLaw {
    /// The lesser of k₁ x^−e₁ and k₂ x^−e₂, from `first` = [k₁, e₁] and `second` = [k₂, e₂],
    /// evaluated in `form`.
    ///
    /// The laws cross at x* = (k₂ ÷ k₁)^(1 ÷ (e₂ − e₁)), below which the first is the lesser if
    /// e₂ > e₁. Without two positive finite coefficients, or with exponents closer than
    /// [`MIN_EXPONENT_GAP`], there is no crossing to use, and both laws are always evaluated.
    #[must_use]
    pub(crate) fn new(form: PowerForm, first: [f64; 2], second: [f64; 2]) -> Self {
        let [k1, e1] = first;
        let [k2, e2] = second;
        let gap = e2 - e1;
        let usable = k1 > 0.0
            && k2 > 0.0
            && k1.is_finite()
            && k2.is_finite()
            && gap.abs() > MIN_EXPONENT_GAP;
        let crossing = usable
            .then(|| (math::powf_positive(k2 / k1, 1.0 / gap), gap > 0.0))
            .filter(|(x, _)| x.is_finite() && *x > 0.0);
        Self {
            form,
            first,
            second,
            crossing,
        }
    }

    /// min(k₁ x^−e₁, k₂ x^−e₂) at `x`.
    #[must_use]
    pub(crate) fn at(&self, x: f64) -> f64 {
        let law = |[k, e]: [f64; 2]| match self.form {
            PowerForm::Product => k * math::powf_positive(x, -e),
            PowerForm::Quotient => k / math::powf_positive(x, e),
        };
        let first = || law(self.first);
        let second = || law(self.second);
        match self
            .crossing
            .map(|(crossing, first_below)| lesser_side(x, crossing, first_below))
        {
            Some(Some(Side::First)) => first(),
            Some(Some(Side::Second)) => second(),
            Some(None) | None => first().min(second()),
        }
    }
}

/// How a law k x^−e is written in the formula it comes from, which its arithmetic follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PowerForm {
    /// k × x^(−e).
    Product,
    /// k ÷ x^e.
    Quotient,
}

/// Which of two crossing laws is the lesser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Side {
    First,
    Second,
}

/// Which of two laws that cross at `crossing` is the lesser at `x`, if it is certain: `First`
/// below the crossing when `first_below`, and the other side above it; `None` within
/// [`CROSSING_MARGIN`] of it, or for an `x` that is not positive (or NaN).
#[must_use]
pub(crate) fn lesser_side(x: f64, crossing: f64, first_below: bool) -> Option<Side> {
    let (below, above) = if first_below {
        (Side::First, Side::Second)
    } else {
        (Side::Second, Side::First)
    };
    if x > 0.0 && x < crossing * (1.0 - CROSSING_MARGIN) {
        Some(below)
    } else if x > crossing * (1.0 + CROSSING_MARGIN) {
        Some(above)
    } else {
        None
    }
}

/// The radius α coefficient of HPT equation 21a at mass `m` (M☉) from the raw a58–a61:
/// a58 M^a60 ÷ (a59 + M^a61).
///
/// The journal prints this form; the arXiv preprint (astro-ph/0001295, page 8) misprints the
/// denominator as a59 M^a61.
#[must_use]
fn alpha_r_power_law(a: &[f64; 82], m: f64) -> f64 {
    a[58] * math::powf_positive(m, a[60]) / (a[59] + math::powf_positive(m, a[61]))
}

/// The aₙ at Z, with the Appendix's special cases in its order.
#[must_use]
fn a_coefficients(z: f64, zeta: f64) -> [f64; 82] {
    let sigma = math::log10(z);
    let mut a: [f64; 82] = core::array::from_fn(|n| poly(&A[n], zeta));
    // a11 = a′11 a14, a12 = a′12 a14.
    a[11] *= a[14];
    a[12] *= a[14];
    // log a17 = max(0.097 − 0.1072 (σ + 3), max(0.097, min(0.1461, 0.1461 + 0.1237 (σ + 2)))).
    let log_a17 = (0.097 - 0.1072 * (sigma + 3.0))
        .max(0.097_f64.max(0.1461_f64.min(0.1461 + 0.1237 * (sigma + 2.0))));
    a[17] = math::exp10(log_a17);
    // a18 = a′18 a20, a19 = a′19 a20.
    a[18] *= a[20];
    a[19] *= a[20];
    // a29 = a′29^a32.
    a[29] = math::powf_positive(a[29], a[32]);
    // a33 = min(1.4, 1.5135 + 0.3769ζ); a33 = max(0.6355 − 0.4192ζ, max(1.25, a33)).
    a[33] = 1.4_f64.min(1.5135 + 0.3769 * zeta);
    a[33] = (0.6355 - 0.4192 * zeta).max(1.25_f64.max(a[33]));
    // a42 = min(1.25, max(1.1, a42)); a44 = min(1.3, max(0.45, a44)).
    a[42] = 1.25_f64.min(1.1_f64.max(a[42]));
    a[44] = 1.3_f64.min(0.45_f64.max(a[44]));
    // a49 to a53.
    a[49] = a[49].max(0.145);
    a[50] = a[50].min(0.306 + 0.053 * zeta);
    a[51] = a[51].min(0.3625 + 0.062 * zeta);
    a[52] = a[52].max(0.9);
    if z > 0.01 {
        a[52] = a[52].min(1.0);
    }
    a[53] = a[53].max(1.0);
    if z > 0.01 {
        a[53] = a[53].min(1.1);
    }
    // a57 = min(1.4, a57); a57 = max(0.6355 − 0.4192ζ, max(1.25, a57)).
    a[57] = 1.4_f64.min(a[57]);
    a[57] = (0.6355 - 0.4192 * zeta).max(1.25_f64.max(a[57]));
    // a62 to a68.
    a[62] = 0.065_f64.max(a[62]);
    if z < 0.004 {
        a[63] = 0.055_f64.min(a[63]);
    }
    a[64] = 0.091_f64.max(0.121_f64.min(a[64]));
    a[66] = a[66].max(1.6_f64.min(-0.308 - 1.046 * zeta));
    a[66] = 0.8_f64.max((0.8 - 2.0 * zeta).min(a[66]));
    a[68] = 0.9_f64.max(a[68].min(1.0));
    // a64 = B = αR(M = a66) (equation 21a) for a68 > a66; a68 = min(a68, a66).
    if a[68] > a[66] {
        a[64] = alpha_r_power_law(&a, a[66]);
    }
    a[68] = a[68].min(a[66]);
    // a72 = max(a72, 0.95) for Z > 0.01; a74 = max(1.4, min(a74, 1.6)).
    if z > 0.01 {
        a[72] = a[72].max(0.95);
    }
    a[74] = 1.4_f64.max(a[74].min(1.6));
    // a75 to a81.
    a[75] = 1.0_f64.max(a[75].min(1.27));
    a[75] = a[75].max(0.6355 - 0.4192 * zeta);
    a[76] = a[76].max(-0.101_556_4 + zeta * (-0.216_126_4 - zeta * 0.051_825_16));
    a[77] = (-0.386_877_6 + zeta * (-0.545_707_8 - zeta * 0.146_347_2)).max(0.0_f64.min(a[77]));
    a[78] = 0.0_f64.max(a[78].min(7.454 + 9.046 * zeta));
    a[79] = a[79].min(2.0_f64.max(-13.3 - 18.6 * zeta));
    a[80] = 0.058_554_2_f64.max(a[80]);
    a[81] = 1.5_f64.min(0.4_f64.max(a[81]));
    a
}

/// The bₙ at Z, with the Appendix's special cases in its order, except b46, which needs the
/// critical masses and is finished by [`ZCoeffs::new`].
#[must_use]
fn b_coefficients(z: f64, zeta: f64) -> [f64; 58] {
    let sigma = math::log10(z);
    let rho = zeta + 1.0;
    let zeta5 = math::powi(zeta, 5);
    let mut b: [f64; 58] = core::array::from_fn(|n| poly(&B[n], zeta));
    // b1 = min(0.54, b1).
    b[1] = 0.54_f64.min(b[1]);
    // b2 = 10^(−4.6739 − 0.9394σ); b2 = min(max(b2, −0.04167 + 55.67Z), 0.4771 − 9329.21 Z^2.94).
    b[2] = math::exp10(-4.6739 - 0.9394 * sigma);
    b[2] = b[2]
        .max(-0.04167 + 55.67 * z)
        .min(0.4771 - 9329.21 * math::powf_positive(z, 2.94));
    // b′3 = max(−0.1451, −2.2794 − 1.5175σ − 0.254σ²); b3 = 10^b′3;
    // b3 = max(b3, 0.7307 + 14265.1 Z^3.395) for Z > 0.004.
    let b3_log = (-0.1451_f64).max(-2.2794 + sigma * (-1.5175 - sigma * 0.254));
    b[3] = math::exp10(b3_log);
    if z > 0.004 {
        b[3] = b[3].max(0.7307 + 14_265.1 * math::powf_positive(z, 3.395));
    }
    // b4 = b4 + 0.1231572ζ⁵; b6 = b6 + 0.01640687ζ⁵.
    b[4] += 0.123_157_2 * zeta5;
    b[6] += 0.016_406_87 * zeta5;
    // b11 = b′11², b13 = b′13².
    b[11] *= b[11];
    b[13] *= b[13];
    // b14 = b′14^b15, b16 = b′16^b15.
    b[14] = math::powf_positive(b[14], b[15]);
    b[16] = math::powf_positive(b[16], b[15]);
    // b17 = 1.0, or 1.0 − 0.3880523 (ζ + 1.0)^0.6371760 for ζ > −1.0. Both versions of the paper
    // print the exponent as 2.862149, which is b′16's second coefficient: a misprint that the SSE
    // code (`zdata.h`) settles.
    b[17] = if zeta > -1.0 {
        1.0 - 0.388_052_3 * math::powf_positive(zeta + 1.0, 0.637_176)
    } else {
        1.0
    };
    // b24 = b′24^b28; b26 = 5.0 − 0.09138012 Z^−0.3671407; b27 = b′27^(2 b28).
    b[24] = math::powf_positive(b[24], b[28]);
    b[26] = 5.0 - 0.091_380_12 * math::powf_positive(z, -0.367_140_7);
    b[27] = math::powf_positive(b[27], 2.0 * b[28]);
    // b31 = b′31^b33, b34 = b′34^b33.
    b[31] = math::powf_positive(b[31], b[33]);
    b[34] = math::powf_positive(b[34], b[33]);
    // b36 = b′36⁴, b37 = 4.0 b′37, b38 = b′38⁴.
    b[36] = math::powi(b[36], 4);
    b[37] *= 4.0;
    b[38] = math::powi(b[38], 4);
    // b40 = max(b40, 1.0); b41 = b′41^b42; b44 = b′44⁵.
    b[40] = b[40].max(1.0);
    b[41] = math::powf_positive(b[41], b[42]);
    b[44] = math::powi(b[44], 5);
    // b45 = 1.0 − (2.47162ρ − 5.401682ρ² + 3.247361ρ³), or 1.0 for ρ ≤ 0.0 (the text layer of the
    // paper drops the parentheses; the printed page and the SSE code have them);
    // b47 = 1.127733ρ + 0.2344416ρ² − 0.3793726ρ³.
    b[45] = if rho <= 0.0 {
        1.0
    } else {
        1.0 - rho * (2.471_62 + rho * (-5.401_682 + rho * 3.247_361))
    };
    b[47] = rho * (1.127_733 + rho * (0.234_441_6 - rho * 0.379_372_6));
    // b51 = b′51 − 0.1343798ζ⁵; b53 = b′53 + 0.4426929ζ⁵;
    // b55 = min(0.99164 − 743.123 Z^2.83, b55);
    // b56 = b′56 + 0.1140142ζ⁵; b57 = b′57 − 0.01308728ζ⁵.
    b[51] -= 0.134_379_8 * zeta5;
    b[53] += 0.442_692_9 * zeta5;
    b[55] = (0.991_64 - 743.123 * math::powf_positive(z, 2.83)).min(b[55]);
    b[56] += 0.114_014_2 * zeta5;
    b[57] -= 0.013_087_28 * zeta5;
    b
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::bits;

    use super::*;

    /// The five metallicities of the published SSE comparison (P06.T12).
    const REFERENCE_Z: [f64; 5] = [1e-4, 1e-3, 4e-3, 0.02, 0.03];

    /// log₁₀ Z at 200 points from 0.0001 to 0.03.
    fn z_sweep() -> impl Iterator<Item = f64> {
        let (lo, hi) = (math::log10(1e-4), math::log10(0.03));
        (0..200).map(move |i| math::exp10(lo + (hi - lo) * f64::from(i) / 199.0).clamp(1e-4, 0.03))
    }

    /// A law as the formula prints it.
    type Printed<'a> = Box<dyn Fn(f64) -> f64 + 'a>;

    /// Masses from 0.05 to 200 M☉, denser within 10⁻⁶ of `crossing` on either side, and the
    /// crossing's neighbours to the ulp.
    fn masses_around(crossing: Option<f64>) -> Vec<f64> {
        let mut xs: Vec<f64> = (0..400)
            .map(|i| math::exp10(-1.3 + 3.6 * f64::from(i) / 399.0))
            .collect();
        if let Some(x) = crossing {
            xs.extend((-50..=50).map(|k| x * (1.0 + 1e-8 * f64::from(k))));
            xs.extend([x, x.next_up(), x.next_down(), x.next_up().next_up()]);
        }
        xs
    }

    /// The lesser of two power laws is `min` of both, bit for bit, everywhere, including at and
    /// around their crossing, for every radius and hook coefficient of the sweep of Z.
    #[test]
    fn the_lesser_power_law_is_the_printed_min_bit_for_bit() {
        let mut crossings = 0;
        for z in z_sweep() {
            let c = ZCoeffs::new(MetalFraction::new(z));
            let b = |n| c.b(n);
            let a = |n| c.a(n);
            let laws: [(&LesserPowerLaw, Printed); 3] = [
                (
                    c.giant_radius_scale(),
                    Box::new(move |m| {
                        (b(4) * math::powf_positive(m, -b(5)))
                            .min(b(6) * math::powf_positive(m, -b(7)))
                    }),
                ),
                (
                    c.agb_radius_scale(),
                    Box::new(move |m| {
                        (b(51) * math::powf_positive(m, -b(52)))
                            .min(b(53) * math::powf_positive(m, -b(54)))
                    }),
                ),
                (
                    c.hook_luminosity_scale(),
                    Box::new(move |m| {
                        (a(34) / math::powf_positive(m, a(35)))
                            .min(a(36) / math::powf_positive(m, a(37)))
                    }),
                ),
            ];
            for (law, printed) in &laws {
                let crossing = law.crossing.map(|(x, _)| x);
                crossings += usize::from(crossing.is_some());
                for m in masses_around(crossing) {
                    assert_eq!(
                        bits(law.at(m)),
                        bits(printed(m)),
                        "Z = {z}, M = {m}: {law:?}"
                    );
                }
            }
        }
        assert!(crossings > 0, "some law has a crossing to test");
    }

    /// A pair of laws with no usable crossing evaluates both everywhere.
    #[test]
    fn a_lesser_power_law_without_a_crossing_evaluates_both() {
        let law = LesserPowerLaw::new(PowerForm::Product, [1.0, 0.3], [2.0, 0.3]);
        assert_eq!(law.crossing, None);
        assert_eq!(bits(law.at(3.0)), bits(math::powf_positive(3.0, -0.3)));
        let negative = LesserPowerLaw::new(PowerForm::Product, [-1.0, 0.1], [2.0, 0.3]);
        assert_eq!(negative.crossing, None);
        assert_eq!(
            bits(negative.at(3.0)),
            bits(-math::powf_positive(3.0, -0.1))
        );
        assert_eq!(lesser_side(-1.0, 2.0, true), None);
        assert_eq!(lesser_side(f64::NAN, 2.0, true), None);
    }

    #[test]
    fn critical_masses_at_solar_metallicity_follow_equations_1_to_3() {
        let c = ZCoeffs::new(MetalFraction::new(0.02));
        assert!(
            (c.m_hook().value() - 1.02).abs() < 0.005,
            "{:?}",
            c.m_hook()
        );
        assert!((c.m_hef().value() - 1.995).abs() < 1e-12, "{:?}", c.m_hef());
        assert!((c.m_fgb().value() - 13.0).abs() < 0.05, "{:?}", c.m_fgb());
    }

    /// Critical masses from the published SSE code (see [`sse`](super) for the run), to 10⁻¹² for
    /// `M_hook` and `M_HeF` and 0.2% for `M_FGB`, whose printed constants are rounded.
    #[test]
    fn critical_masses_match_the_published_sse_code() {
        let sse = [
            (
                1.122_280_768_646_741_5,
                1.880_384_797_646_252_9,
                4.747_379_483_031_545,
            ),
            (
                0.961_127_017_420_287_2,
                1.817_005_578_400_720_2,
                10.345_837_521_227_383,
            ),
            (
                0.950_139_422_578_551_9,
                1.862_762_137_741_645_5,
                11.738_584_380_093_904,
            ),
            (1.0185, 1.995, 13.032_468_534_130_723),
            (
                1.049_466_940_468_978,
                2.041_720_522_205_796,
                13.359_831_740_975_5,
            ),
        ];
        for (z, (hook, hef, fgb)) in REFERENCE_Z.into_iter().zip(sse) {
            let c = ZCoeffs::new(MetalFraction::new(z));
            for (name, ours, theirs, tolerance) in [
                ("m_hook", c.m_hook().value(), hook, 1e-12),
                ("m_hef", c.m_hef().value(), hef, 1e-12),
                ("m_fgb", c.m_fgb().value(), fgb, 2e-3),
            ] {
                assert!(
                    (ours - theirs).abs() < tolerance * theirs,
                    "{name} at Z = {z}: {ours} against {theirs}"
                );
            }
        }
    }

    /// `M_FGB` rises with Z throughout. `M_hook` and `M_HeF` are quadratics in ζ with one minimum each,
    /// at ζ = −0.16015 ÷ 0.1784 (Z ≈ 0.0025) and ζ = −0.25 ÷ 0.174 (Z ≈ 0.00073), both inside the
    /// fitted range, so each falls and then rises; all three are continuous.
    #[test]
    fn critical_masses_are_continuous_and_turn_once_in_metallicity() {
        let masses: Vec<[f64; 3]> = z_sweep()
            .map(|z| {
                let c = ZCoeffs::new(MetalFraction::new(z));
                [c.m_hook().value(), c.m_hef().value(), c.m_fgb().value()]
            })
            .collect();
        for pair in masses.windows(2) {
            let [before, after] = [pair[0], pair[1]];
            assert!(after[2] > before[2], "`M_FGB` must rise with Z");
            for k in 0..3 {
                assert!(
                    (after[k] - before[k]).abs() < 0.2,
                    "critical mass {k} jumps between neighbouring Z: {before:?} → {after:?}"
                );
            }
        }
        for k in 0..2 {
            let turns = masses
                .windows(3)
                .filter(|w| (w[1][k] - w[0][k]) * (w[2][k] - w[1][k]) < 0.0)
                .count();
            assert_eq!(turns, 1, "critical mass {k} turns once");
        }
    }

    /// Coefficient `k` of a `ZCoeffs`: a1–a81, then b1–b57.
    fn coefficient(c: &ZCoeffs, k: usize) -> f64 {
        if k < 81 { c.a(k + 1) } else { c.b(k - 80) }
    }

    /// Coefficient `k`'s name.
    fn coefficient_name(k: usize) -> String {
        if k < 81 {
            format!("a{}", k + 1)
        } else {
            format!("b{}", k - 80)
        }
    }

    /// Every coefficient is finite at every metallicity of the range, and continuous in Z, across
    /// the Appendix's switches at Z = 0.004 and 0.01 too (no clamp is active there), except for
    /// a64 at the metallicity, about 0.016, where a68 crosses a66 and a64 switches to αR(a66):
    /// that jump is the paper's (and the SSE code's), and keeps αR continuous in mass at every Z.
    ///
    /// A neighbouring pair whose difference exceeds a Lipschitz bound set by the coefficient's own
    /// range is bisected 40 times towards the larger half: a continuous coefficient's difference
    /// vanishes, while a jump keeps its size down to an interval of 10⁻¹² in log Z.
    #[test]
    fn every_coefficient_is_finite_and_continuous_between_the_switches() {
        const COUNT: usize = 81 + 57;
        let coeffs: Vec<(f64, ZCoeffs)> = z_sweep()
            .map(|z| (z, ZCoeffs::new(MetalFraction::new(z))))
            .collect();
        let mut range = [(f64::INFINITY, f64::NEG_INFINITY); COUNT];
        for (z, c) in &coeffs {
            for (k, r) in range.iter_mut().enumerate() {
                let x = coefficient(c, k);
                assert!(x.is_finite(), "{} at Z = {z}", coefficient_name(k));
                *r = (r.0.min(x), r.1.max(x));
            }
        }
        let span = math::log10(0.03 / 1e-4);
        for pair in coeffs.windows(2) {
            let ((z0, c0), (z1, c1)) = (&pair[0], &pair[1]);
            let step = math::log10(z1 / z0);
            let capped = |c: &ZCoeffs| c.a(68) >= c.a(66);
            for (k, (lo, hi)) in range.iter().enumerate() {
                if k == 63 && capped(c0) != capped(c1) {
                    continue;
                }
                let (x0, x1) = (coefficient(c0, k), coefficient(c1, k));
                if (x1 - x0).abs() <= 20.0 * (hi - lo) * step / span {
                    continue;
                }
                let (mut a, mut b) = (math::log10(*z0), math::log10(*z1));
                let (mut xa, mut xb) = (x0, x1);
                for _ in 0..40 {
                    let mid = f64::midpoint(a, b);
                    let xm = coefficient(&ZCoeffs::new(MetalFraction::new(math::exp10(mid))), k);
                    if (xm - xa).abs() >= (xb - xm).abs() {
                        (b, xb) = (mid, xm);
                    } else {
                        (a, xa) = (mid, xm);
                    }
                }
                assert!(
                    (xb - xa).abs() < 1e-6 * (hi - lo).max(1e-3),
                    "{} jumps by {} near Z = {}",
                    coefficient_name(k),
                    xb - xa,
                    math::exp10(a)
                );
            }
        }
    }

    /// The derived coefficients take the forms the Appendix gives (spot checks at Z = 0.02, where
    /// ζ = 0 and σ = log₁₀ 0.02).
    #[test]
    fn derived_coefficients_follow_the_appendix_at_solar_metallicity() {
        let c = ZCoeffs::new(MetalFraction::new(0.02));
        // a11 = a′11 a14 at ζ = 0.
        assert!((c.a(11) - 1.031_538 * 3.858_911e3).abs() < 1e-9);
        // a17 ≈ 1.4 (HPT section 5.1).
        assert!((c.a(17) - 1.4).abs() < 0.03, "{}", c.a(17));
        // a33 = max(0.6355, max(1.25, min(1.4, 1.5135))) = 1.4.
        assert!((c.a(33) - 1.4).abs() < 1e-15);
        // b17 = 1 − 0.3880523 at ζ = 0.
        assert!((c.b(17) - (1.0 - 0.388_052_3)).abs() < 1e-15);
        // b45 at ρ = 1: 1 − (2.47162 − 5.401682 + 3.247361) = 0.682701, a fraction of the core
        // helium-burning time, so between 0 and 1.
        assert!((c.b(45) - 0.682_701).abs() < 1e-12);
        for z in REFERENCE_Z {
            let b45 = ZCoeffs::new(MetalFraction::new(z)).b(45);
            assert!((0.0..=1.0).contains(&b45), "b45 = {b45} at Z = {z}");
        }
        // b36 = b′36⁴ at ζ = 0.
        assert!((c.b(36) - math::powi(1.445_216e-1, 4)).abs() < 1e-15);
    }

    /// b14–b17 and `M_FGB` reproduce the published SSE code's `L_min,He` ÷ `L_HeI` (HPT
    /// equation 51, its `lHef`; see [`sse`](super) for the run) for intermediate masses at the
    /// metallicities where b17 is neither 1 nor its solar value, which the printed exponent of b17
    /// (2.862149) misses by 0.03–0.17 dex. The tolerance is `M_FGB`'s rounded constants, which move
    /// the ratio by up to 1.7 × 10⁻⁵ here (at Z = 0.001, where b17 is 1 in both forms).
    #[test]
    fn b14_to_b17_reproduce_the_minimum_helium_burning_luminosity_of_the_sse_code() {
        let sse = [
            (0.001, 3.0, 0.701_027_469_269_589_5),
            (0.004, 5.0, 0.676_755_950_997_596_8),
            (0.02, 4.0, 0.280_600_981_000_361_6),
            (0.03, 3.0, 0.196_165_061_010_434_5),
            (0.03, 8.0, 0.475_501_205_808_725_5),
        ];
        for (z, m, ratio_sse) in sse {
            let c = ZCoeffs::new(MetalFraction::new(z));
            let (b14, b15, b16, b17) = (c.b(14), c.b(15), c.b(16), c.b(17));
            let m_fgb = c.m_fgb().value();
            let c_coefficient =
                b17 / math::powf(m_fgb, 0.1) + (b16 * b17 - b14) / math::powf(m_fgb, b15 + 0.1);
            let ratio =
                (b14 + c_coefficient * math::powf(m, b15 + 0.1)) / (b16 + math::powf(m, b15));
            assert!(
                (ratio / ratio_sse - 1.0).abs() < 3e-5,
                "L_min,He ÷ L_HeI at Z = {z}, M = {m}: {ratio} against {ratio_sse}"
            );
        }
    }

    #[test]
    fn the_same_metallicity_gives_the_same_coefficients() {
        for z in REFERENCE_Z {
            assert_eq!(
                ZCoeffs::new(MetalFraction::new(z)),
                ZCoeffs::new(MetalFraction::new(z))
            );
        }
    }
}
