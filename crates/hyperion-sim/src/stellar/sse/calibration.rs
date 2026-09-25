//! The giant radii between HPT's calibration metallicities (galaxy-generation ruling 92): the
//! radius laws of equations 46 and 74 are evaluated with Hurley, Pols and Tout's (2000, MNRAS 315,
//! 543; "HPT") coefficients at the seven metallicities they fitted, and log R is interpolated in
//! log Z between them by a monotone cubic.
//!
//! **A departure from the printed form.** HPT fitted the models of Pols et al. (1998) at Z = 10⁻⁴,
//! 3 × 10⁻⁴, 10⁻³, 0.004, 0.01, 0.02 and 0.03 (HPT section 3). Between those points the Appendix's
//! min and max clamps on b1, b2 and b3 switch at [Fe/H] −2.010, −1.301, −0.859, −0.592, −0.567 and
//! −0.340, so the printed radius departs from its own node values by up to +0.22/−0.14 dex (the
//! asymptotic giant at 1.4 M☉ and 5,000 L☉). Through the Vassiliadis and Wood superwind that
//! makes the white dwarf mass swing ±0.08 M☉ about its trend, with three sign reversals in 0.25
//! dex, and metal-poor giants cooler than solar ones by up to about 700 K. Detailed models fall
//! monotonically instead (Meng, Chen and Han 2008, A&A 487, 625, App. A; Romero, Campos and
//! Kepler 2015, MNRAS 450, 3708, Table 1), and at its seven nodes the printed law is monotone in Z
//! too. The published SSE code reproduces the printed form, so this is a departure from both;
//! the research behind it is `_orchestration/research/wd-ifmr/NOTES.md`.
//!
//! At a calibration metallicity the laws are HPT's own, bit for bit, so P06.T12's comparison with
//! the SSE code at 10⁻⁴, 10⁻³, 0.004, 0.02 and 0.03 is unchanged. Nothing else of HPT is
//! interpolated.
//!
//! **The cubic** is the shape-preserving piecewise cubic Hermite of Fritsch and Carlson (1980, SIAM
//! J. Numer. Anal. 17, 238): the slope at an interior node is Brodlie's weighted harmonic mean of
//! the two secants, zero where they differ in sign (Fritsch and Butland 1984, SIAM J. Sci. Stat.
//! Comput. 5, 300), and at the two ends the three-point formula held to the secant's sign and to
//! three times it (Moler 2004, _Numerical Computing with MATLAB_, section 3.4, `pchip`). It is
//! monotone wherever the node values are, and C¹. The Hermite weights, which depend on Z alone,
//! are fixed in [`ZCoeffs::new`](super::ZCoeffs::new) as a [`ZBlend`]; the slopes depend on the
//! node radii, which depend on the mass and luminosity, and are formed at each evaluation (no
//! scheme with weights fixed in advance of the data can be monotone for every data set).

use crate::math;

use super::coeffs::{LesserPowerLaw, PowerForm};

/// HPT's seven calibration metallicities, the metal fractions of Pols et al.'s (1998) models that
/// HPT fitted (HPT section 3).
pub(crate) const CALIBRATION_Z: [f64; 7] = [1e-4, 3e-4, 1e-3, 4e-3, 0.01, 0.02, 0.03];

/// How close, relatively, a metal fraction must be to a calibration metallicity to take its laws
/// alone: Z reaches the formulae as 0.02 × 10^[Fe/H], which returns a calibration metallicity
/// only to within a few ulps, and the cubic there would differ from the node's law in its last
/// bits. The cubic's value moves by at most its slope times 10⁻¹² in ln Z over this window, some
/// 10⁻¹² dex, so the snap is continuous to that.
const SNAP: f64 = 1e-12;

/// The coefficients of the giant's and the asymptotic giant's radius laws (HPT equations 46 and
/// 74) at one metallicity: b1–b7 and b51–b57 after the Appendix's special cases, and `M_HeF`, which
/// bounds equation 74's mass blend.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RadiusCoeffs {
    /// b1.
    pub(crate) b1: f64,
    /// b2.
    pub(crate) b2: f64,
    /// b3.
    pub(crate) b3: f64,
    /// A = min(b4 M^−b5, b6 M^−b7) of equation 46.
    pub(crate) giant_scale: LesserPowerLaw,
    /// A = min(b51 M^−b52, b53 M^−b54) of equation 74 from `M_HeF` up.
    pub(crate) agb_scale: LesserPowerLaw,
    /// b55.
    pub(crate) b55: f64,
    /// b56.
    pub(crate) b56: f64,
    /// b57.
    pub(crate) b57: f64,
    /// `M_HeF`, M☉ (HPT equation 2).
    pub(crate) m_hef: f64,
}

/// One calibration metallicity: its ζ = log₁₀(Z ÷ 0.02) and its radius coefficients.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Calibration {
    /// ζ = log₁₀(Z ÷ 0.02).
    pub(crate) zeta: f64,
    /// The radius coefficients at Z.
    pub(crate) coeffs: RadiusCoeffs,
    /// ln k₁ and ln k₂ of [`RadiusCoeffs::giant_scale`], whose coefficients are positive.
    pub(crate) ln_giant: [f64; 2],
    /// ln k₁ and ln k₂ of [`RadiusCoeffs::agb_scale`], whose coefficients are positive.
    pub(crate) ln_agb: [f64; 2],
}

impl Calibration {
    /// ln A of equation 46 at a mass whose logarithm is `ln_m`: ln min(b4 M^−b5, b6 M^−b7) as the
    /// lesser of ln b4 − b5 ln M and ln b6 − b7 ln M.
    #[must_use]
    pub(crate) fn ln_giant_scale(&self, ln_m: f64) -> f64 {
        ln_lesser(self.ln_giant, self.coeffs.giant_scale.exponents(), ln_m)
    }

    /// ln A of equation 74 from `M_HeF` up at a mass whose logarithm is `ln_m`, as
    /// [`Calibration::ln_giant_scale`] gives equation 46's.
    #[must_use]
    pub(crate) fn ln_agb_scale(&self, ln_m: f64) -> f64 {
        ln_lesser(self.ln_agb, self.coeffs.agb_scale.exponents(), ln_m)
    }
}

/// ln min(k₁ x^−e₁, k₂ x^−e₂) from ln k₁ and ln k₂, e₁ and e₂, and ln x.
#[must_use]
fn ln_lesser(ln_k: [f64; 2], exponents: [f64; 2], ln_x: f64) -> f64 {
    (ln_k[0] - exponents[0] * ln_x).min(ln_k[1] - exponents[1] * ln_x)
}

/// A product power law k x^−e for [`CALIBRATION`]'s literals.
const fn product(
    first: [f64; 2],
    second: [f64; 2],
    crossing: Option<(f64, bool)>,
) -> LesserPowerLaw {
    LesserPowerLaw::from_parts(PowerForm::Product, first, second, crossing)
}

/// [`CALIBRATION_Z`]'s ζ and radius coefficients, as [`ZCoeffs::new`](super::ZCoeffs::new)
/// evaluates them there: literals, so that a metallicity between two costs no extra evaluation of
/// the Appendix, and a test holds them to `ZCoeffs::new` bit for bit.
pub(crate) const CALIBRATION: [Calibration; 7] = [
    // Z = 0.0001
    Calibration {
        zeta: -2.301_029_995_663_981_3,
        coeffs: RadiusCoeffs {
            b1: 0.54,
            b2: 0.121_255_096_057_148_72,
            b3: 0.715_978_531_081_961_7,
            giant_scale: product(
                [0.921_960_958_280_275, 0.133_278_165_602_687_07],
                [0.948_701_334_996_085_8, 0.177_744_654_190_283_2],
                Some((1.902_144_786_399_451_6, true)),
            ),
            agb_scale: product(
                [2.763_983_395_411_163, 0.261_199_534_900_665_7],
                [0.953_069_432_918_557_2, 0.155_321_605_757_470_5],
                Some((23_302.172_894_155_7, false)),
            ),
            b55: 0.977_739_750_612_988_8,
            b56: 0.973_679_226_231_028_3,
            b57: -0.074_056_665_396_960_65,
            m_hef: 1.880_384_797_646_252_9,
        },
        ln_giant: [-0.081_252_400_920_501_23, -0.052_661_245_399_637_215],
        ln_agb: [1.016_672_898_438_450_5, -0.048_067_520_774_592_08],
    },
    // Z = 0.0003
    Calibration {
        zeta: -1.823_908_740_944_318_9,
        coeffs: RadiusCoeffs {
            b1: 0.54,
            b2: 0.043_200_850_143_187,
            b3: 0.820_837_395_811_516_6,
            giant_scale: product(
                [1.019_950_339_490_76, 0.148_486_622_527_464_42],
                [1.195_863_498_833_376_3, 0.270_576_743_662_737_2],
                Some((3.681_260_213_598_945_6, true)),
            ),
            agb_scale: product(
                [1.071_981_094_750_852_6, 0.152_255_145_097_620_6],
                [1.454_847_368_607_481_4, 0.293_992_104_075_500_14],
                Some((8.624_811_808_269_854, true)),
            ),
            b55: 0.951_945_505_329_554_5,
            b56: 1.083_301_225_876_615_7,
            b57: -0.093_772_927_674_624_58,
            m_hef: 1.828_440_764_054_419_4,
        },
        ln_giant: [0.019_753_939_337_175_647, 0.178_868_517_604_734_5],
        ln_agb: [0.069_508_426_999_495_43, 0.374_900_993_820_229_5],
    },
    // Z = 0.001
    Calibration {
        zeta: -1.301_029_995_663_981_3,
        coeffs: RadiusCoeffs {
            b1: 0.54,
            b2: 0.014_000_000_000_000_005,
            b3: 0.970_733_461_153_508_1,
            giant_scale: product(
                [1.088_238_093_527_167, 0.160_963_435_616_414_4],
                [1.573_640_289_151_144_5, 0.444_000_019_097_995_64],
                Some((3.680_775_727_168_148_5, true)),
            ),
            agb_scale: product(
                [1.196_128_522_800_500_7, 0.232_773_965_687_050_5],
                [2.010_672_710_242_92, 0.491_502_011_307_211_55],
                Some((7.444_184_175_870_576, true)),
            ),
            b55: 0.947_207_051_003_230_4,
            b56: 1.207_192_898_504_572_6,
            b57: -0.141_116_950_373_129_93,
            m_hef: 1.817_005_578_400_720_2,
        },
        ln_giant: [0.084_559_960_455_123_98, 0.453_391_590_936_756_7],
        ln_agb: [0.179_090_110_290_506, 0.698_469_347_790_448_4],
    },
    // Z = 0.004
    Calibration {
        zeta: -0.698_970_004_336_018_7,
        coeffs: RadiusCoeffs {
            b1: 0.454_109_220_692_818_06,
            b2: 0.181_01,
            b3: 0.792_399_926_752_767,
            giant_scale: product(
                [1.016_362_411_730_029_4, 0.196_650_275_278_848_26],
                [1.327_098_234_589_588, 0.446_790_743_720_369_64],
                Some((2.905_077_141_903_617, true)),
            ),
            agb_scale: product(
                [1.187_894_792_807_965_5, 0.326_747_450_388_039_76],
                [1.793_818_964_566_548_8, 0.512_200_257_343_765_7],
                Some((9.230_145_293_136_71, true)),
            ),
            b55: 0.972_228_664_242_820_6,
            b56: 1.131_144_609_064_832_3,
            b57: -0.149_793_382_601_247_22,
            m_hef: 1.862_762_137_741_645_5,
        },
        ln_giant: [0.016_229_990_011_069_82, 0.282_994_780_183_328_46],
        ln_agb: [0.172_182_658_776_487_22, 0.584_346_846_931_835_9],
    },
    // Z = 0.01
    Calibration {
        zeta: -core::f64::consts::LOG10_2,
        coeffs: RadiusCoeffs {
            b1: 0.358_189_761_002_510_15,
            b2: 0.464_801_706_052_092_2,
            b3: 0.733_013_528_321_983_5,
            giant_scale: product(
                [0.912_411_792_720_279_6, 0.231_352_469_618_528_23],
                [1.025_396_544_877_008_4, 0.380_568_220_115_980_35],
                Some((2.186_668_237_553_309, true)),
            ),
            agb_scale: product(
                [0.991_929_250_383_951_7, 0.338_193_232_358_200_26],
                [1.264_129_065_775_019_3, 0.450_802_083_053_888_6],
                Some((8.613_713_727_708_15, true)),
            ),
            b55: 0.990_014_224_018_725_7,
            b56: 1.009_298_804_607_793,
            b57: -0.137_126_252_077_865_52,
            m_hef: 1.927_626_359_155_187_4,
        },
        ln_giant: [-0.091_663_863_709_924_55, 0.025_079_410_825_028_28],
        ln_agb: [-0.008_103_494_418_137_047, 0.234_383_399_511_120_78],
    },
    // Z = 0.02
    Calibration {
        zeta: 0.0,
        coeffs: RadiusCoeffs {
            b1: 0.397,
            b2: 0.382_721_503_094_855_86,
            b3: 0.755_037_258_540_172_6,
            giant_scale: product(
                [0.996_028_3, 0.256_106_2],
                [1.157_338, 0.402_276_5],
                Some((2.792_393_443_763_743, true)),
            ),
            agb_scale: product(
                [1.125_124, 0.334_948_9],
                [1.467_794, 0.465_851_2],
                Some((7.621_982_004_227_363, true)),
            ),
            b55: 0.980_079_527_342_623_7,
            b56: 1.110_866,
            b57: -0.158_433_3,
            m_hef: 1.995,
        },
        ln_giant: [-0.003_979_608_146_580_605, 0.146_122_540_379_929],
        ln_agb: [0.117_893_251_804_582_88, 0.383_760_593_370_095_8],
    },
    // Z = 0.03
    Calibration {
        zeta: 0.176_091_259_055_681_24,
        coeffs: RadiusCoeffs {
            b1: 0.464_172_670_346_711_6,
            b2: 0.166_228_178_401_862_71,
            b3: 0.827_105_426_484_825_7,
            giant_scale: product(
                [1.226_706_189_583_537_4, 0.266_542_676_922_743_64],
                [1.566_912_235_517_33, 0.491_362_686_510_493_6],
                Some((2.970_578_182_531_973_7, true)),
            ),
            agb_scale: product(
                [1.481_983_522_525_969_7, 0.340_585_629_839_457_17],
                [2.305_953_578_042_081_5, 0.543_838_869_374_733_5],
                Some((8.803_791_346_422_862, true)),
            ),
            b55: 0.955_222_187_633_752_8,
            b56: 1.379_377_619_179_876_3,
            b57: -0.204_979_382_507_065_85,
            m_hef: 2.041_720_522_205_796,
        },
        ln_giant: [0.204_332_682_762_872_34, 0.449_106_953_839_412_24],
        ln_agb: [0.393_381_408_408_556, 0.835_494_290_699_779_8],
    },
];

/// Where a metal fraction falls among the calibration metallicities.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum RadiusInZ {
    /// At a calibration metallicity (to [`SNAP`]): the laws are HPT's own at Z.
    Calibrated,
    /// Between two: the monotone cubic through the nodes of the stencil.
    Between(ZBlend),
}

impl RadiusInZ {
    /// The place of metal fraction `z`, within 10⁻⁴–0.03, whose ζ = log₁₀(Z ÷ 0.02) is `zeta`.
    #[must_use]
    pub(crate) fn new(z: f64, zeta: f64) -> Self {
        if CALIBRATION_Z
            .iter()
            .any(|node| (z / node - 1.0).abs() <= SNAP)
        {
            return Self::Calibrated;
        }
        // The interval [Z_i, Z_i+1) that holds z; z lies strictly inside it, having missed the
        // nodes, and within the range, which `ZCoeffs::new` has clamped it to.
        let below = CALIBRATION_Z
            .iter()
            .rposition(|node| *node < z)
            .unwrap_or(0)
            .min(CALIBRATION_Z.len() - 2);
        Self::Between(ZBlend::new(below, zeta))
    }
}

/// The fixed part of the monotone cubic at one ζ between two calibration metallicities: the
/// stencil of nodes whose radii it reads, their spacings and the Hermite basis at ζ.
///
/// The interval is [ζᵢ, ζᵢ₊₁]; the slope at each end needs the nodes on either side of it, so the
/// stencil runs from i − 1 to i + 2, cut to the range: three nodes in the first and last intervals,
/// four elsewhere.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ZBlend {
    /// The stencil's first node, an index of [`CALIBRATION`].
    first: usize,
    /// The number of nodes in the stencil, 3 or 4.
    count: usize,
    /// The stencil's index of the node below ζ.
    below: usize,
    /// The spacings in ζ of the stencil's nodes; the unused last is zero.
    h: [f64; 3],
    /// The Hermite basis at ζ, h₀₀, h₀₁, H h₁₀ and H h₁₁, with H the interval's width: the weights
    /// of the lower and upper node values and slopes.
    basis: [f64; 4],
}

impl ZBlend {
    /// The blend at `zeta`, in the interval above calibration node `below` (0–5).
    fn new(below: usize, zeta: f64) -> Self {
        let first = below.saturating_sub(1);
        let last = (below + 2).min(CALIBRATION.len() - 1);
        let count = last - first + 1;
        let mut h = [0.0; 3];
        for (j, spacing) in h.iter_mut().enumerate().take(count - 1) {
            *spacing = CALIBRATION[first + j + 1].zeta - CALIBRATION[first + j].zeta;
        }
        let width = CALIBRATION[below + 1].zeta - CALIBRATION[below].zeta;
        let t = (zeta - CALIBRATION[below].zeta) / width;
        let s = 1.0 - t;
        Self {
            first,
            count,
            below: below - first,
            h,
            basis: [
                (1.0 + 2.0 * t) * s * s,
                t * t * (3.0 - 2.0 * t),
                width * t * s * s,
                -width * t * t * s,
            ],
        }
    }

    /// The calibration metallicities of the stencil, in order.
    pub(crate) fn nodes(&self) -> &'static [Calibration] {
        &CALIBRATION[self.first..self.first + self.count]
    }

    /// The number of nodes in the stencil, 3 or 4.
    #[must_use]
    pub(crate) const fn count(&self) -> usize {
        self.count
    }

    /// The monotone cubic at ζ through `y`, the stencil's node values in order (the first
    /// [`ZBlend::count`] of them).
    #[must_use]
    pub(crate) fn at(&self, y: &[f64; 4]) -> f64 {
        let n = self.count;
        let mut secant = [0.0; 3];
        for (j, d) in secant.iter_mut().enumerate().take(n - 1) {
            *d = (y[j + 1] - y[j]) / self.h[j];
        }
        let slope = |j: usize| {
            if j == 0 {
                edge_slope(self.h[0], self.h[1], secant[0], secant[1])
            } else if j == n - 1 {
                edge_slope(self.h[n - 2], self.h[n - 3], secant[n - 2], secant[n - 3])
            } else {
                interior_slope(self.h[j - 1], self.h[j], secant[j - 1], secant[j])
            }
        };
        let (lo, hi) = (self.below, self.below + 1);
        self.basis[0] * y[lo]
            + self.basis[1] * y[hi]
            + self.basis[2] * slope(lo)
            + self.basis[3] * slope(hi)
    }
}

/// The slope at an interior node from the secants `before` and `after` over spacings `h_before`
/// and `h_after`: Brodlie's weighted harmonic mean (Fritsch and Butland 1984), zero where the
/// secants differ in sign or one is zero.
#[must_use]
fn interior_slope(h_before: f64, h_after: f64, before: f64, after: f64) -> f64 {
    if before * after <= 0.0 {
        return 0.0;
    }
    let w1 = 2.0 * h_after + h_before;
    let w2 = h_after + 2.0 * h_before;
    (w1 + w2) / (w1 / before + w2 / after)
}

/// The slope at an end node from the secant `near` it and the next, `far`, over spacings `h_near`
/// and `h_far`: the three-point formula, zero where it opposes `near`, and at most 3 `near` where
/// the secants differ in sign (Moler 2004, `pchip`).
#[must_use]
fn edge_slope(h_near: f64, h_far: f64, near: f64, far: f64) -> f64 {
    let d = ((2.0 * h_near + h_far) * near - h_near * far) / (h_near + h_far);
    if d * near <= 0.0 {
        0.0
    } else if near * far <= 0.0 && d.abs() > 3.0 * near.abs() {
        3.0 * near
    } else {
        d
    }
}

/// e^(`exponent` × `ln_x`): a power x^e from x's logarithm, which is
/// [`math::powf_positive`]`(x, e)` bit for bit, since that is `exp(e × ln x)`.
#[must_use]
pub(crate) fn power_from_ln(ln_x: f64, exponent: f64) -> f64 {
    math::exp(exponent * ln_x)
}

#[cfg(test)]
mod tests {
    use super::super::ZCoeffs;
    use super::*;
    use crate::units::MetalFraction;

    /// log₁₀ Z at `n` points from 10⁻⁴ to 0.03.
    fn z_sweep(n: u32) -> impl Iterator<Item = f64> {
        let (lo, hi) = (math::log10(1e-4), math::log10(0.03));
        (0..n).map(move |i| {
            math::exp10(lo + (hi - lo) * f64::from(i) / f64::from(n - 1)).clamp(1e-4, 0.03)
        })
    }

    /// The table's literals are what `ZCoeffs::new` evaluates at each calibration metallicity, bit
    /// for bit (their `Debug` forms print every bit of each `f64`, the sign of zero included).
    #[test]
    fn the_calibration_table_is_the_appendix_at_each_calibration_metallicity() {
        for (z, calibration) in CALIBRATION_Z.into_iter().zip(CALIBRATION) {
            let c = ZCoeffs::new(MetalFraction::new(z));
            assert_eq!(
                format!("{:?}", (c.zeta(), c.radius_coeffs())),
                format!("{:?}", (calibration.zeta, calibration.coeffs)),
                "Z = {z}"
            );
            assert_eq!(*c.radius_in_z(), RadiusInZ::Calibrated, "Z = {z}");
            let coeffs = calibration.coeffs;
            for (ln_k, law) in [
                (calibration.ln_giant, coeffs.giant_scale),
                (calibration.ln_agb, coeffs.agb_scale),
            ] {
                let k = law.coefficients();
                assert!(k[0] > 0.0 && k[1] > 0.0, "Z = {z}: {law:?}");
                let expected = [math::ln(k[0]), math::ln(k[1])];
                assert_eq!(format!("{ln_k:?}"), format!("{expected:?}"), "Z = {z}");
            }
        }
    }

    /// A calibration metallicity reached through [Fe/H], a few ulps off, takes its node's laws.
    #[test]
    fn a_calibration_metallicity_through_fe_h_is_calibrated() {
        for z in CALIBRATION_Z {
            let through = 0.02 * math::exp10(math::log10(z / 0.02));
            let zeta = math::log10(through / 0.02);
            assert_eq!(
                RadiusInZ::new(through, zeta),
                RadiusInZ::Calibrated,
                "Z = {z}"
            );
        }
    }

    /// Between the calibration metallicities the stencil holds the interval's two nodes and one
    /// more on each side where there is one, and the Hermite basis at ζ is a partition of unity
    /// in the node values.
    #[test]
    fn a_metallicity_between_two_calibrations_blends_its_stencil() {
        let mut between = 0;
        for z in z_sweep(400) {
            let zeta = math::log10(z / 0.02);
            let RadiusInZ::Between(blend) = RadiusInZ::new(z, zeta) else {
                assert!(
                    CALIBRATION_Z.iter().any(|n| (z / n - 1.0).abs() < 1e-9),
                    "Z = {z}"
                );
                continue;
            };
            between += 1;
            let nodes = blend.nodes();
            let (lo, hi) = (&nodes[blend.below], &nodes[blend.below + 1]);
            assert!(lo.zeta < zeta && zeta < hi.zeta, "Z = {z}: {blend:?}");
            let expected = if blend.first == 0 && blend.below == 0
                || blend.first + blend.count == 7 && blend.below + 2 == blend.count
            {
                3
            } else {
                4
            };
            assert_eq!(blend.count, expected, "Z = {z}: {blend:?}");
            assert!(
                (blend.basis[0] + blend.basis[1] - 1.0).abs() < 1e-15,
                "Z = {z}: {blend:?}"
            );
        }
        assert!(between > 380, "{between} of 400 between the nodes");
    }

    /// The cubic through the node values of a linear function of ζ is that function, and through
    /// monotone node values it is monotone at every ζ between them, whatever the steps.
    #[test]
    fn the_cubic_keeps_lines_and_monotone_data() {
        let line = |zeta: f64| 0.7 - 0.31 * zeta;
        // Monotone falling, with a nearly flat stretch and a steep step.
        let falling = [2.0, 1.99, 1.989_9, 1.2, 1.1, 0.4, 0.39];
        let mut last = f64::INFINITY;
        for z in z_sweep(2_000) {
            let zeta = math::log10(z / 0.02);
            let RadiusInZ::Between(blend) = RadiusInZ::new(z, zeta) else {
                continue;
            };
            let values = |f: &dyn Fn(usize) -> f64| {
                let mut y = [0.0; 4];
                for (j, v) in y.iter_mut().enumerate().take(blend.count) {
                    *v = f(blend.first + j);
                }
                y
            };
            let on_line = blend.at(&values(&|k| line(CALIBRATION[k].zeta)));
            assert!((on_line - line(zeta)).abs() < 1e-13, "Z = {z}: {on_line}");
            let y = blend.at(&values(&|k| falling[k]));
            assert!(y <= last + 1e-15, "Z = {z}: {y} after {last}");
            let (lo, hi) = (
                falling[blend.first + blend.below],
                falling[blend.first + blend.below + 1],
            );
            assert!(hi <= y && y <= lo, "Z = {z}: {y} outside {hi}–{lo}");
            last = y;
        }
    }

    /// Next to a calibration metallicity the cubic tends to the node's value, which is where the
    /// laws switch to the node's own.
    #[test]
    fn the_cubic_meets_the_node_values() {
        let y_of =
            |k: usize| math::ln(1.0 + f64::from(u8::try_from(k).expect("seven nodes")) * 0.3) * 0.8;
        for (k, z) in CALIBRATION_Z.into_iter().enumerate() {
            for side in [1.0 - 1e-9, 1.0 + 1e-9] {
                let near = z * side;
                if !(1e-4..=0.03).contains(&near) {
                    continue;
                }
                let RadiusInZ::Between(blend) = RadiusInZ::new(near, math::log10(near / 0.02))
                else {
                    panic!("Z = {near} is between the nodes");
                };
                let mut y = [0.0; 4];
                for (j, v) in y.iter_mut().enumerate().take(blend.count) {
                    *v = y_of(blend.first + j);
                }
                assert!((blend.at(&y) - y_of(k)).abs() < 1e-8, "Z = {near}");
            }
        }
    }
}
