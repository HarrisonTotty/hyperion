//! Extinction by wavelength: the Cardelli–Clayton–Mathis law and a small set of bands (plan 07,
//! P07.T7 and Design note 20).
//!
//! Cardelli, Clayton and Mathis (1989, ApJ 345, 245) fit the mean extinction curve of the diffuse
//! interstellar medium as `A_λ ÷ A_V = a(x) + b(x) ÷ R_V`, with `x = 1 ÷ λ` in µm⁻¹, in four
//! ranges (their equations 2–5):
//!
//! - infrared, `0.3 ≤ x < 1.1`: `a = 0.574 x^1.61`, `b = −0.527 x^1.61`;
//! - optical and near infrared, `1.1 ≤ x < 3.3`, with `y = x − 1.82`: two seventh-order
//!   polynomials in `y`;
//! - ultraviolet, `3.3 ≤ x ≤ 8`: a linear term, a Drude-like bump at 4.6 µm⁻¹, and the far-UV
//!   curvature terms `F_a`, `F_b` above `x = 5.9`;
//! - far ultraviolet, `8 < x ≤ 10`: two cubics in `x − 8`.
//!
//! At `R_V = 3.1`, the diffuse medium's mean, the Milky Way's centre lies behind some thirty
//! magnitudes in V and about three in K (brainstorm, "What dust and gas do"). Above `x = 10`
//! (below 0.1 µm, the Lyman limit's side of the far ultraviolet) the fit has no data and
//! [`extinction_ratio`] refuses rather than extrapolates. Radio is untouched.
//!
//! From 1.1 µm the curve is Gordon et al.'s (2023, ApJ 950, 86, eqs. 8–13 and 15, Table 4) near-
//! and mid-infrared intercept at `R_V = 3.1`, where their `b` term drops out, and over 0.9–1.1 µm
//! it is joined to Cardelli, Clayton and Mathis's law (the infrared power law below `x = 1.1`,
//! at 0.909–1.1 µm, and the optical polynomial above) by their eq. 17's weight
//! `W₂ = W(λ, 1.0 µm, 0.2 µm)`: `(1 − W₂) CCM + W₂ G23` (ruling 98 of 2026-09-22). Cardelli,
//! Clayton and Mathis's infrared power law, `λ^−1.61`, is the shallowest of the measured laws
//! (Decleir et al. 2022, ApJ 930, 15, Table 5: 1.71 for the diffuse average) and runs 5–16% high
//! over 1–4 µm against both Gordon et al. and Decleir et al.'s measured curve, whose `A_K ÷ A_V`
//! is 0.102 ± 0.010; Gordon et al. give 0.1016 at 2.2 µm, about a tenth, where Cardelli, Clayton
//! and Mathis gave 0.1135. The two laws differ by 4–5% across the join, so it has no step; ruling
//! 91 joined them at 3.3 µm instead, with a step of 16%.
//!
//! Gordon et al.'s form is two power laws, `g₁ λ^−α₁` and `g₁ λ_b^(α₂−α₁) λ^−α₂`, joined by the
//! smooth step `W = 3z² − 2z³` with `z = (λ − λ_b + δ ÷ 2) ÷ δ` clamped to 0–1 (their eq. 10), plus
//! the silicate features at 10 and 20 µm as two modified Drude profiles `S (γ ÷ λ₀)² ÷ ((λ ÷ λ₀ −
//! λ₀ ÷ λ)² + (γ ÷ λ₀)²)` with `γ = 2γ₀ ÷ (1 + e^(a (λ − λ₀)))`. Continuing Cardelli, Clayton and
//! Mathis's power law into the mid infrared, as Design note 20 first chose, gave 0.010 of `A_V` at
//! 10 µm against the measured 0.08 (ruling 91). The coefficients are Table 4's as the authors'
//! `dust_extinction` package (`G23`) carries them, without the package's 0.9854 renormalisation,
//! which is not in the paper. The second feature's centre is the package's 19.58294 µm; Table 4 as
//! typeset prints 19.258294 µm, and which is the intended value cannot be settled from the paper
//! (ruling 98; the effect is at most 6% at 15 µm and 0.06% at 10 µm, and Gordon et al.'s 2021
//! sightlines fit 19.3–20.0 µm). The 10 µm feature alone gives `A_V ÷ τ_9.7` = 16.6, Rieke and
//! Lebofsky's (1985, ApJ 288, 618) 16.6 ± 2.1, against Roche and Aitken's (1984) 18 ± 1 as Chiar
//! and Tielens (2006, ApJ 637, 774, §7) adopt it for the local medium.
//!
//! [`HYDROGEN_COLUMN_PER_MAG`] ties the curve to the gas: Bohlin, Savage and Drake (1978, ApJ 224,
//! 132) measured `N(H) ÷ E(B − V) = 5.8 × 10²¹` atoms cm⁻² mag⁻¹ for the diffuse medium, so at
//! `R_V = 3.1` one magnitude of visual extinction is `1.87 × 10²¹` hydrogen nuclei per cm² of gas
//! with the solar dust-to-gas ratio, the brainstorm's "about 2 × 10²¹".

use std::error::Error;
use std::fmt;

use crate::math;
use crate::units::Micrometres;

/// The ratio of total to selective extinction, `R_V = A_V ÷ E(B − V)`, of the diffuse
/// interstellar medium: 3.1, the value Cardelli, Clayton and Mathis's mean curve takes and Rieke
/// and Lebofsky (1985, ApJ 288, 618) measured, 3.09 ± 0.03.
pub const R_V: f64 = 3.1;

/// Bohlin, Savage and Drake's (1978) gas-to-reddening ratio `N(H) ÷ E(B − V)`, hydrogen nuclei
/// (atomic and in molecules) per cm² per magnitude of reddening.
pub const HYDROGEN_COLUMN_PER_REDDENING: f64 = 5.8e21;

/// The hydrogen column per magnitude of visual extinction at the solar dust-to-gas ratio,
/// `N(H) ÷ A_V = 5.8 × 10²¹ ÷ R_V ≈ 1.87 × 10²¹` cm⁻² mag⁻¹ (Bohlin, Savage and Drake 1978, at
/// `R_V = 3.1`). Gas of dust-to-gas ratio `ζ` needs `1 ÷ ζ` times as much (Design note 13).
pub const HYDROGEN_COLUMN_PER_MAG: f64 = HYDROGEN_COLUMN_PER_REDDENING / R_V;

/// The largest inverse wavelength the fit covers, 10 µm⁻¹ (0.1 µm).
const FAR_ULTRAVIOLET_LIMIT: f64 = 10.0;

/// The centre of the join between Cardelli, Clayton and Mathis's law and Gordon et al.'s (2023)
/// infrared form, 1.0 µm, and its width, 0.2 µm: their eq. 17's `W₂` (ruling 98 of 2026-09-22).
const INFRARED_JOIN_UM: (f64, f64) = (1.0, 0.2);

/// Gordon et al.'s (2023) Table 4 intercept `a_ir`: the power laws' `g₁`, `α₁` and `α₂`, the break
/// `λ_b` and the transition's width `δ` (µm).
const G23_POWER: [f64; 5] = [0.385_26, 1.684_67, 0.787_91, 4.305_78, 4.783_38];

/// Gordon et al.'s (2023) Table 4 silicate features: amplitude `S`, centre `λ₀` (µm), width `γ₀`
/// (µm) and asymmetry `a` (µm⁻¹) of the 10 and 20 µm features. The second's width and asymmetry
/// are fixed there after Gordon et al. (2021). Its centre is `dust_extinction`'s 19.58294 µm; the
/// typeset Table 4 prints 19.258294, and the paper alone cannot say which is meant (module
/// documentation).
const G23_SILICATES: [[f64; 4]; 2] = [
    [0.066_52, 9.843_4, 2.212_05, -0.247_03],
    [0.026_7, 19.582_94, 17.0, -0.27],
];

/// An extinction ratio was asked for at a wavelength the law does not cover.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EvaluateExtinctionError {
    /// The wavelength is shorter than 0.1 µm, beyond the far-ultraviolet end of the fit.
    BeyondFarUltraviolet {
        /// The wavelength asked for.
        wavelength: Micrometres,
    },
    /// The wavelength is not a positive number: zero, negative or a NaN.
    NotAWavelength {
        /// The value given.
        wavelength: Micrometres,
    },
}

impl fmt::Display for EvaluateExtinctionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BeyondFarUltraviolet { wavelength } => write!(
                f,
                "{} µm is shorter than the 0.1 µm the extinction law reaches",
                wavelength.value()
            ),
            Self::NotAWavelength { wavelength } => {
                write!(f, "{} µm is not a wavelength", wavelength.value())
            }
        }
    }
}

impl Error for EvaluateExtinctionError {}

/// `A_λ ÷ A_V` at `wavelength`, at `R_V = 3.1`: Cardelli, Clayton and Mathis (1989) to 0.9 µm,
/// Gordon et al. (2023) from 1.1 µm and a smooth blend between (module documentation).
///
/// # Errors
///
/// [`EvaluateExtinctionError::BeyondFarUltraviolet`] below 0.1 µm, and
/// [`EvaluateExtinctionError::NotAWavelength`] for a wavelength that is not positive. An infinite
/// wavelength is the long-wavelength limit and gives 0.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::gas::ccm::extinction_ratio;
/// use hyperion_sim::units::Micrometres;
///
/// // In K, at 2.2 µm, about a tenth of the visual extinction: why the centre is seen there.
/// let k = extinction_ratio(Micrometres::new(2.2))?;
/// assert!((0.090..0.115).contains(&k));
/// // Blue light is reddened: A_B − A_V is E(B − V), and A_V ÷ E(B − V) is R_V.
/// let b = extinction_ratio(Micrometres::new(0.44))?;
/// assert!(((b - 1.0) * 3.1 - 1.0).abs() < 0.03);
/// assert!(extinction_ratio(Micrometres::new(0.05)).is_err());
/// # Ok::<(), hyperion_sim::galaxy::gas::ccm::EvaluateExtinctionError>(())
/// ```
pub fn extinction_ratio(wavelength: Micrometres) -> Result<f64, EvaluateExtinctionError> {
    let lambda = wavelength.value();
    if lambda.is_nan() || lambda <= 0.0 {
        return Err(EvaluateExtinctionError::NotAWavelength { wavelength });
    }
    if lambda.is_infinite() {
        return Ok(0.0);
    }
    let (centre, width) = INFRARED_JOIN_UM;
    if lambda >= centre + 0.5 * width {
        return Ok(infrared(lambda));
    }
    let x = 1.0 / lambda;
    if x > FAR_ULTRAVIOLET_LIMIT {
        return Err(EvaluateExtinctionError::BeyondFarUltraviolet { wavelength });
    }
    let (a, b) = coefficients(x);
    let optical = a + b / R_V;
    if lambda <= centre - 0.5 * width {
        return Ok(optical);
    }
    let weight = smooth_step(lambda, centre, width);
    Ok((1.0 - weight) * optical + weight * infrared(lambda))
}

/// Gordon et al.'s (2023) eq. 10 weight `W(λ, λ_b, δ) = 3z² − 2z³`, `z = (λ − λ_b + δ ÷ 2) ÷ δ`
/// clamped to 0–1: 0 below `λ_b − δ ÷ 2`, 1 above `λ_b + δ ÷ 2`.
fn smooth_step(lambda: f64, centre: f64, width: f64) -> f64 {
    let z = ((lambda - (centre - 0.5 * width)) / width).clamp(0.0, 1.0);
    z * z * (3.0 - 2.0 * z)
}

/// `A_λ ÷ A_V` at `lambda` µm from 1.1 µm: Gordon et al.'s (2023) intercept, eqs. 8–13, which is
/// the whole ratio at `R_V = 3.1` (module documentation).
fn infrared(lambda: f64) -> f64 {
    let [g1, alpha1, alpha2, break_at, width] = G23_POWER;
    let first = g1 * math::powf(lambda, -alpha1);
    let second = g1 * math::powf(break_at, alpha2 - alpha1) * math::powf(lambda, -alpha2);
    let step = smooth_step(lambda, break_at, width);
    let features: f64 = G23_SILICATES
        .iter()
        .map(|&feature| silicate(lambda, feature))
        .sum();
    first * (1.0 - step) + second * step + features
}

/// One modified Drude profile of Gordon et al. (2023, eqs. 12–13) at `lambda` µm: `[S, λ₀, γ₀,
/// a]` as [`G23_SILICATES`] holds them.
fn silicate(lambda: f64, [amplitude, centre, width, asymmetry]: [f64; 4]) -> f64 {
    let gamma = 2.0 * width / (1.0 + math::exp(asymmetry * (lambda - centre)));
    let g = gamma / centre;
    let offset = lambda / centre - centre / lambda;
    amplitude * g * g / (offset * offset + g * g)
}

/// `a(x)` and `b(x)` of Cardelli, Clayton and Mathis (1989), equations 2–5, for `1 ÷ 1.1 < x ≤
/// 10` µm⁻¹ (the infrared branch, below `x = 1.1`, is read only across the join and by the tests).
fn coefficients(x: f64) -> (f64, f64) {
    if x < 1.1 {
        // Equation 2, the infrared.
        let power = math::powf(x, 1.61);
        (0.574 * power, -0.527 * power)
    } else if x < 3.3 {
        // Equation 3, the optical and near infrared, by Horner's rule in y = x − 1.82.
        let y = x - 1.82;
        let a = horner(
            &[
                1.0, 0.176_99, -0.504_47, -0.024_27, 0.720_85, 0.019_79, -0.775_30, 0.329_99,
            ],
            y,
        );
        let b = horner(
            &[
                0.0, 1.413_38, 2.283_05, 1.072_33, -5.384_34, -0.622_51, 5.302_60, -2.090_02,
            ],
            y,
        );
        (a, b)
    } else if x <= 8.0 {
        // Equation 4, the ultraviolet, with the far-ultraviolet curvature above 5.9 µm⁻¹.
        let (f_a, f_b) = if x >= 5.9 {
            let y = x - 5.9;
            (
                -0.044_73 * y * y - 0.009_779 * y * y * y,
                0.213_0 * y * y + 0.120_7 * y * y * y,
            )
        } else {
            (0.0, 0.0)
        };
        let bump_a = x - 4.67;
        let bump_b = x - 4.62;
        (
            1.752 - 0.316 * x - 0.104 / (bump_a * bump_a + 0.341) + f_a,
            -3.090 + 1.825 * x + 1.206 / (bump_b * bump_b + 0.263) + f_b,
        )
    } else {
        // Equation 5, the far ultraviolet, cubics in x − 8.
        let y = x - 8.0;
        (
            horner(&[-1.073, -0.628, 0.137, -0.070], y),
            horner(&[13.670, 4.257, -0.420, 0.374], y),
        )
    }
}

/// `Σ c_k y^k` by Horner's rule, the coefficients lowest order first.
fn horner(coefficients: &[f64], y: f64) -> f64 {
    coefficients.iter().rev().fold(0.0, |sum, c| sum * y + c)
}

/// The bands a console reads extinction in (Design note 20): Johnson–Cousins and near-infrared
/// photometric bands at their effective wavelengths, a mid-infrared point, and radio.
///
/// The effective wavelengths are those of the SVO Filter Profile Service (Generic Johnson U, B and
/// V, Cousins R and I; 2MASS J, H and Ks) to two figures: Design note 20's 0.66 and 0.81 µm for R
/// and I are 4% long against the Cousins bands' 0.636 and 0.783 µm and are replaced by 0.64 and
/// 0.79. The 10 µm point is on Gordon et al.'s (2023) mid-infrared curve, in the silicate
/// feature: 0.082 of `A_V` (ruling 91 of 2026-09-22).
///
/// [`extinction_ratio`] is the real interface; the bands are a convenience set. Plan 06's
/// photometry works in V and B − V only and defines no band type, so this one lives here until a
/// sensor plan needs more.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Band {
    /// Johnson U, 0.36 µm.
    U,
    /// Johnson B, 0.44 µm.
    B,
    /// Johnson V, 0.55 µm: visual.
    V,
    /// Cousins R, 0.64 µm.
    R,
    /// Cousins I, 0.79 µm.
    I,
    /// J, 1.25 µm.
    J,
    /// H, 1.65 µm.
    H,
    /// K, 2.2 µm.
    K,
    /// The mid infrared at 10 µm, in the silicate feature (Gordon et al. 2023).
    MidInfrared,
    /// Radio, which dust does not dim.
    Radio,
}

impl Band {
    /// Every band, shortest wavelength first and radio last.
    pub const ALL: [Self; 10] = [
        Self::U,
        Self::B,
        Self::V,
        Self::R,
        Self::I,
        Self::J,
        Self::H,
        Self::K,
        Self::MidInfrared,
        Self::Radio,
    ];

    /// The band's effective wavelength; `None` for [`Radio`](Self::Radio), which is no one
    /// wavelength.
    #[must_use]
    pub fn wavelength(self) -> Option<Micrometres> {
        let microns = match self {
            Self::U => 0.36,
            Self::B => 0.44,
            Self::V => 0.55,
            Self::R => 0.64,
            Self::I => 0.79,
            Self::J => 1.25,
            Self::H => 1.65,
            Self::K => 2.2,
            Self::MidInfrared => 10.0,
            Self::Radio => return None,
        };
        Some(Micrometres::new(microns))
    }

    /// `A_band ÷ A_V`: [`extinction_ratio`] at the band's wavelength, and exactly 0 for radio.
    ///
    /// # Panics
    ///
    /// Never: every band's wavelength lies inside the law's range.
    #[must_use]
    pub fn ratio(self) -> f64 {
        self.wavelength().map_or(0.0, |wavelength| {
            extinction_ratio(wavelength).expect("every band lies between 0.36 and 10 µm")
        })
    }

    /// A short snake-case name, such as `mid_infrared`, for logs and golden files.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::U => "u",
            Self::B => "b",
            Self::V => "v",
            Self::R => "r",
            Self::I => "i",
            Self::J => "j",
            Self::H => "h",
            Self::K => "k",
            Self::MidInfrared => "mid_infrared",
            Self::Radio => "radio",
        }
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;

    fn ratio(microns: f64) -> f64 {
        extinction_ratio(Micrometres::new(microns)).unwrap()
    }

    fn at_x(x: f64) -> f64 {
        let (a, b) = coefficients(x);
        a + b / R_V
    }

    /// V is 1 and K about a tenth (ruling 98 of 2026-09-22: T7's window becomes 0.090–0.115):
    /// Gordon et al.'s (2023) 0.1016 at 2.2 µm, against Decleir et al.'s (2022, Table 4) measured
    /// 0.102 ± 0.010.
    #[test]
    fn v_is_one_and_k_is_about_a_tenth() {
        assert!((Band::V.ratio() - 1.0).abs() < 0.01, "{}", Band::V.ratio());
        let k = Band::K.ratio();
        assert!((0.090..=0.115).contains(&k), "{k}");
        assert!((k - 0.1016).abs() < 5e-4, "{k}");
        // At CCM's own normalisation, x = 1.82 µm⁻¹, the ratio is 1 exactly.
        assert_same_bits(at_x(1.82), 1.0);
    }

    /// Ruling 98: from 1.1 µm the curve is Gordon et al.'s, which agrees with Decleir et al.'s
    /// (2022, Table 4) measured J, H, K and 3.3 µm points within their errors: 0.264 ± 0.017,
    /// 0.162 ± 0.013, 0.102 ± 0.010 and 0.054 ± 0.009, where Cardelli, Clayton and Mathis run
    /// 5–16% high.
    #[test]
    fn the_near_infrared_is_gordon_et_als() {
        for (microns, gordon, measured, error) in [
            (1.25, 0.2645, 0.264, 0.017),
            (1.65, 0.166, 0.162, 0.013),
            (2.2, 0.1016, 0.102, 0.010),
            (3.3, 0.0494, 0.054, 0.009),
        ] {
            let value = ratio(microns);
            assert_same_bits(value, infrared(microns));
            assert!((value - gordon).abs() < 5e-4, "{value} at {microns} µm");
            assert!((value - measured).abs() < error, "{value} at {microns} µm");
            let ccm = at_x(1.0 / microns);
            assert!(
                (1.04..1.20).contains(&(ccm / value)),
                "CCM {ccm} at {microns} µm"
            );
        }
        assert_same_bits(Band::J.ratio(), infrared(1.25));
        assert_same_bits(Band::H.ratio(), infrared(1.65));
    }

    /// Ruling 98: the two laws are joined over 0.9–1.1 µm by Gordon et al.'s eq. 17 weight, with
    /// no step: Cardelli, Clayton and Mathis's to 0.9 µm, bit for bit, Gordon et al.'s from 1.1,
    /// halfway at 1.0, and across each end of the join the curve moves by under 10⁻⁹.
    #[test]
    fn the_laws_are_joined_smoothly_over_nine_tenths_to_one_point_one_microns() {
        assert_same_bits(ratio(0.9), at_x(1.0 / 0.9));
        assert_same_bits(ratio(0.85), at_x(1.0 / 0.85));
        assert_same_bits(Band::I.ratio(), at_x(1.0 / 0.79));
        assert_same_bits(ratio(1.1), infrared(1.1));
        let middle = f64::midpoint(at_x(1.0), infrared(1.0));
        assert!((ratio(1.0) - middle).abs() < 1e-15, "{}", ratio(1.0));
        // The laws differ by 4–5% there.
        assert!((0.94..0.97).contains(&(infrared(1.0) / at_x(1.0))));
        for edge in [0.9, 1.1] {
            let (below, above) = (ratio(edge - 1e-10), ratio(edge + 1e-10));
            assert!(
                (above - below).abs() < 1e-9,
                "{below} and {above} at {edge} µm"
            );
        }
    }

    /// `A_B ÷ A_V − 1 = E(B − V) ÷ A_V = 1 ÷ R_V`, to 3%: the fit reproduces its own `R_V`.
    #[test]
    fn b_minus_v_is_one_over_r_v() {
        let reddening = Band::B.ratio() - Band::V.ratio();
        assert!(
            (reddening * R_V - 1.0).abs() < 0.03,
            "E(B − V) ÷ A_V = {reddening}"
        );
        let from_one = Band::B.ratio() - 1.0;
        assert!((from_one * R_V - 1.0).abs() < 0.03, "{from_one}");
    }

    /// The four ranges meet: across each boundary the two sides agree to 2%.
    #[test]
    fn the_ranges_meet_at_their_boundaries() {
        for boundary in [1.1, 3.3, 5.9, 8.0] {
            let below = at_x(boundary - 1e-9);
            let above = at_x(boundary + 1e-9);
            assert!(
                (above / below - 1.0).abs() < 0.02,
                "at x = {boundary}: {below} below and {above} above"
            );
        }
    }

    /// Cardelli, Clayton and Mathis's law rises with inverse wavelength from the infrared through
    /// the optical to the ultraviolet bump's near side, and so does the curve as built, joined to
    /// Gordon et al.'s, from 6.8 µm (below the silicate feature's blue wing) to 0.22 µm.
    #[test]
    fn the_curve_rises_with_inverse_wavelength_to_the_bump() {
        let mut previous = 0.0;
        for i in 0..=4_200 {
            let x = 0.3 + 0.001 * f64::from(i);
            let value = at_x(x);
            assert!(value > previous, "{value} at x = {x} after {previous}");
            previous = value;
        }
        let mut previous = 0.0;
        for i in 0..=4_400 {
            let x = 1.0 / 6.8 + 0.001 * f64::from(i);
            let value = ratio(1.0 / x);
            assert!(value > previous, "{value} at x = {x} after {previous}");
            previous = value;
        }
    }

    #[test]
    fn radio_is_untouched() {
        assert_same_bits(Band::Radio.ratio(), 0.0);
        assert_eq!(Band::Radio.wavelength(), None);
        assert_same_bits(ratio(f64::INFINITY), 0.0);
        assert!(ratio(1e6) < 1e-5);
    }

    /// Ruling 91: the mid infrared carries the silicate feature. `A_λ ÷ A_V` is 0.075–0.090 at 9.7
    /// and 10 µm (Gordon et al. 2023 at `R_V` = 3.1: 0.0829 and 0.0824), the feature peaks at 9.8 ±
    /// 0.2 µm, and the feature alone, as an optical depth `τ_9.7 = A ÷ 1.086`, gives `A_V ÷ τ_9.7`
    /// in 15–20 (Rieke and Lebofsky 1985: 16.6 ± 2.1; Roche and Aitken 1984: 18 ± 1).
    #[test]
    fn the_mid_infrared_carries_the_silicate_feature() {
        for microns in [9.7, 10.0] {
            let value = ratio(microns);
            assert!((0.075..=0.090).contains(&value), "{value} at {microns} µm");
        }
        assert!((ratio(9.7) - 0.0829).abs() < 5e-4 && (ratio(10.0) - 0.0824).abs() < 5e-4);
        assert!((Band::MidInfrared.ratio() - ratio(10.0)).abs() < f64::EPSILON);
        let peak = (0..=4_000)
            .map(|i| 8.0 + 0.001 * f64::from(i))
            .max_by(|a, b| ratio(*a).total_cmp(&ratio(*b)))
            .expect("a non-empty scan");
        assert!((peak - 9.8).abs() <= 0.2, "the feature peaks at {peak} µm");
        let feature = silicate(9.7, G23_SILICATES[0]);
        let a_v_over_tau = 2.5 * core::f64::consts::LOG10_E / feature;
        assert!(
            (15.0..=20.0).contains(&a_v_over_tau),
            "A_V ÷ τ_9.7 = {a_v_over_tau}"
        );
        // Between the features the continuum falls, and the 20 µm feature is the weaker.
        assert!(ratio(15.0) < ratio(9.8) && ratio(15.0) < ratio(18.0));
        assert!(ratio(18.0) < ratio(9.8));
    }

    /// The continuum falls to about 6.8 µm, where the silicate feature's blue wing takes over, and
    /// crosses 3.3 µm, where ruling 91 joined the laws with a step of 16%, without one.
    #[test]
    fn the_infrared_continuum_falls_to_the_silicate_feature() {
        let (near, far) = (ratio(3.3 * (1.0 - 1e-12)), ratio(3.3 * (1.0 + 1e-12)));
        assert!((far / near - 1.0).abs() < 1e-9, "{near}, {far}");
        let mut previous = f64::INFINITY;
        for i in 0..=570 {
            let microns = 1.1 + 0.01 * f64::from(i);
            let value = ratio(microns);
            assert!(value < previous, "{value} at {microns} µm");
            previous = value;
        }
        assert!(ratio(7.5) > ratio(6.8));
    }

    #[test]
    fn the_law_refuses_what_it_does_not_cover() {
        for microns in [0.099, 0.01] {
            assert_eq!(
                extinction_ratio(Micrometres::new(microns)),
                Err(EvaluateExtinctionError::BeyondFarUltraviolet {
                    wavelength: Micrometres::new(microns)
                })
            );
        }
        assert!(extinction_ratio(Micrometres::new(0.1)).is_ok());
        for microns in [0.0, -1.0, f64::NAN] {
            assert!(matches!(
                extinction_ratio(Micrometres::new(microns)),
                Err(EvaluateExtinctionError::NotAWavelength { .. })
            ));
        }
        let text = EvaluateExtinctionError::BeyondFarUltraviolet {
            wavelength: Micrometres::new(0.05),
        }
        .to_string();
        assert_eq!(
            text,
            "0.05 µm is shorter than the 0.1 µm the extinction law reaches"
        );
    }

    /// One magnitude of visual extinction is about 2 × 10²¹ hydrogen nuclei per cm², the
    /// brainstorm's figure: 1.87 × 10²¹ at `R_V` = 3.1.
    #[test]
    fn a_magnitude_is_about_two_times_ten_to_the_twenty_one_atoms() {
        assert!((HYDROGEN_COLUMN_PER_MAG / 1.870_97e21 - 1.0).abs() < 1e-5);
    }

    #[test]
    fn the_bands_are_listed_by_wavelength() {
        let wavelengths: Vec<f64> = Band::ALL
            .iter()
            .filter_map(|band| band.wavelength().map(Micrometres::value))
            .collect();
        assert_eq!(wavelengths.len(), 9);
        assert!(wavelengths.windows(2).all(|pair| pair[0] < pair[1]));
        let mut names: Vec<_> = Band::ALL.iter().map(|b| b.name()).collect();
        names.dedup();
        assert_eq!(names.len(), 10);
    }
}
