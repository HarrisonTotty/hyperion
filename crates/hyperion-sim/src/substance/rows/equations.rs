//! The liquids' reference equations of state beyond water's (plan 14, P14.T49.h): methane,
//! nitrogen, ethane, ammonia and carbon dioxide (its second step), and oxygen, argon, krypton,
//! xenon, carbon monoxide, nitrous oxide and hydrogen sulphide (its step 2b), the residual part of
//! each one's dimensionless Helmholtz energy ([`HelmholtzResidual`]), for their isothermal
//! compressibility and so their scattering (decision-r11-palette-liquids §1.8;
//! decision-p14-t49h-liquids §9; decision-p14-t49h-gas-index §1.2). Sulphur dioxide has none here:
//! its scattering stays absent until a depolarisation is read (§4.3), so its equation would be
//! read by nothing.
//!
//! Every coefficient is the published one as CoolProp's fluid files hold it (Bell, Wronski, Quoilin
//! and Lemort 2014, Ind. Eng. Chem. Res. 53, 2498; master of 2026-10-14, the files the registry's
//! phase diagrams read, under the MIT licence, `NOTICE`; their SHA-256 in plan 14's T49.h step 2
//! and step 2b records), each number written as the file's shortest round-trip decimal, in the
//! paper's term order: CoolProp's carbon dioxide file rounds Span and Wagner's Table 31, printed to
//! 14 figures, to 12 (an effect under 10⁻¹⁰). Each equation is checked against its paper's own
//! table where the paper was read, and against NIST's Chemistry WebBook (SRD 69), whose fluid
//! tables evaluate the same equations, where it was not: the isothermal compressibility `κ_T` = 1 ÷
//! [ρ (∂p ÷ ∂ρ)_T] that this crate computes is a table's `c_p` ÷ (`c_v` ρ w²), so a saturation
//! table's heat capacities and sound speed check it at each of its saturated-liquid states (the
//! tests in `liquid/tests.rs`). Step 2b's seven papers were not read: their constants are
//! CoolProp's reading of them, and the WebBook is their only check.
//!
//! The gas constant of each is the one its coefficients were fitted with, divided by the molar
//! mass the paper takes, J kg⁻¹ K⁻¹. The WebBook evaluates Lemmon and Span 2006's equations
//! (krypton, xenon, carbon monoxide, nitrous oxide and hydrogen sulphide) with CODATA 2014's R =
//! 8.314 459 8 J mol⁻¹ K⁻¹ in place of the 8.314 472 they were fitted with, so its `κ_T` lies a
//! constant 1.47 × 10⁻⁶ above theirs, the ratio of the two; each is kept with its own.
//!
//! Step 2's five equations carry their ideal-gas parts beside them ([`HelmholtzIdeal`]), for their
//! saturated liquids' specific heats (rendering plan R09, R09.T13.i; decision-r09-t13i-cl-licence
//! §6): the coefficients of the alpha0 blocks of the same CoolProp files (MIT, `NOTICE`), each
//! number as the file writes it, of the terms whose second τ-derivative is not zero (the ln τ,
//! power and Planck–Einstein terms; the lead and the reference-state offset terms fix only the zeros
//! of energy and entropy). Each is checked with its residual: `c_p` and `c_v` at the states where
//! `κ_T` is.

use super::super::helmholtz::{
    AssociatingTerm, GaussianTerm, HelmholtzIdeal, HelmholtzResidual, IdealPowerTerm,
    NonAnalyticTerm, PlanckEinsteinTerm, PowerTerm,
};
use crate::units::{Kelvin, KilogramsPerCubicMetre};

/// Methane: Setzmann and Wagner 1991 (J. Phys. Chem. Ref. Data 20, 1061), valid from the melting
/// line to 625 K, its Eq. (5.1)'s residual, 36 power and exponential terms and 4 Gaussian: `T_c` =
/// 190.564 K, `ρ_c` = 162.66 kg m⁻³, R = 8.314 51 J mol⁻¹ K⁻¹ and M = 16.0428 g mol⁻¹. Its `κ_T`
/// reproduces Table 39's `c_p` ÷ (`c_v` ρ′ w²) at 90.694, 100 and 120 K within 2 × 10⁻⁴, its five
/// printed figures, and the WebBook's within 10⁻⁶.
pub(in crate::substance) const METHANE: HelmholtzResidual = HelmholtzResidual {
    reducing_temperature: Kelvin::new(190.564),
    reducing_density: KilogramsPerCubicMetre::new(162.66),
    gas_constant_j_per_kg_k: 8.314_51 / 0.016_042_8,
    power: &[
        PowerTerm::new(0, 1, -0.5, 0.043_679_010_28),
        PowerTerm::new(0, 1, 0.5, 0.670_923_619_9),
        PowerTerm::new(0, 1, 1.0, -1.765_577_859),
        PowerTerm::new(0, 2, 0.5, 0.858_233_024_1),
        PowerTerm::new(0, 2, 1.0, -1.206_513_052),
        PowerTerm::new(0, 2, 1.5, 0.512_046_722),
        PowerTerm::new(0, 2, 4.5, -0.000_400_001_079_1),
        PowerTerm::new(0, 3, 0.0, -0.012_478_424_23),
        PowerTerm::new(0, 4, 1.0, 0.031_002_697_01),
        PowerTerm::new(0, 4, 3.0, 0.001_754_748_522),
        PowerTerm::new(0, 8, 1.0, -3.171_921_605e-6),
        PowerTerm::new(0, 9, 3.0, -2.240_346_84e-6),
        PowerTerm::new(0, 10, 3.0, 2.947_056_156e-7),
        PowerTerm::new(1, 1, 0.0, 0.183_048_790_9),
        PowerTerm::new(1, 1, 1.0, 0.151_188_367_9),
        PowerTerm::new(1, 1, 2.0, -0.428_936_387_7),
        PowerTerm::new(1, 2, 0.0, 0.068_940_024_46),
        PowerTerm::new(1, 4, 0.0, -0.014_083_139_96),
        PowerTerm::new(1, 5, 2.0, -0.030_630_548_3),
        PowerTerm::new(1, 6, 2.0, -0.029_699_067_08),
        PowerTerm::new(2, 1, 5.0, -0.019_320_408_31),
        PowerTerm::new(2, 2, 5.0, -0.110_573_995_9),
        PowerTerm::new(2, 3, 5.0, 0.099_525_489_95),
        PowerTerm::new(2, 4, 2.0, 0.008_548_437_825),
        PowerTerm::new(2, 4, 4.0, -0.061_505_556_62),
        PowerTerm::new(3, 3, 12.0, -0.042_917_924_23),
        PowerTerm::new(3, 5, 8.0, -0.018_132_072_9),
        PowerTerm::new(3, 5, 10.0, 0.034_459_047_6),
        PowerTerm::new(3, 8, 10.0, -0.002_385_919_45),
        PowerTerm::new(4, 2, 10.0, -0.011_590_949_39),
        PowerTerm::new(4, 3, 14.0, 0.066_416_936_02),
        PowerTerm::new(4, 4, 12.0, -0.023_715_495_9),
        PowerTerm::new(4, 4, 18.0, -0.039_616_249_05),
        PowerTerm::new(4, 4, 22.0, -0.013_872_920_44),
        PowerTerm::new(4, 5, 18.0, 0.033_894_895_99),
        PowerTerm::new(4, 6, 14.0, -0.002_927_378_753),
    ],
    gaussian: &[
        GaussianTerm::new(2, 2.0, 9.324_799_946e-5, 20.0, 200.0, 1.07, 1.0),
        GaussianTerm::new(0, 0.0, -6.287_171_518, 40.0, 250.0, 1.11, 1.0),
        GaussianTerm::new(0, 1.0, 12.710_694_67, 40.0, 250.0, 1.11, 1.0),
        GaussianTerm::new(0, 2.0, -6.423_953_466, 40.0, 250.0, 1.11, 1.0),
    ],
    non_analytic: &[],
    associating: &[],
};

/// Methane's ideal-gas part, [`METHANE`]'s (Setzmann and Wagner 1991), as CoolProp's `Methane.json`
/// holds it (its alpha0 block): the ln τ coefficient 3.0016 and five Planck–Einstein terms, each
/// θ the file's v in kelvin over its `Tcrit` of 190.564 K, the residual's `T_c`. Its `c_p` and
/// `c_v` reproduce Table 39's at 90.694, 100 and 120 K within 2 × 10⁻⁴ and the WebBook's within
/// 10⁻⁶ (`liquid/tests.rs`).
pub(in crate::substance) const METHANE_IDEAL: HelmholtzIdeal = HelmholtzIdeal {
    log_tau: 3.0016,
    power: &[],
    planck_einstein: &[
        PlanckEinsteinTerm::new(0.008_449, 648.0 / 190.564),
        PlanckEinsteinTerm::new(4.6942, 1_957.0 / 190.564),
        PlanckEinsteinTerm::new(3.4865, 3_895.0 / 190.564),
        PlanckEinsteinTerm::new(1.6572, 5_705.0 / 190.564),
        PlanckEinsteinTerm::new(1.4115, 15_080.0 / 190.564),
    ],
};

/// Nitrogen: Span, Lemmon, Jacobsen, Wagner and Yokozeki 2000 (J. Phys. Chem. Ref. Data 29, 1361),
/// valid from 63.151 to 1000 K, its residual part, 32 power and exponential terms and 4 Gaussian:
/// `T_c` = 126.192 K, `ρ_c` = 11.1839 mol dm⁻³, R = 8.314 51 J mol⁻¹ K⁻¹ and M = 28.013 48 g mol⁻¹.
/// Its `κ_T` reproduces the saturation table's (p. 1403) at 63.151, 70 and 77 K within 3 × 10⁻⁴,
/// its four printed figures, and the WebBook's within 10⁻⁶.
pub(in crate::substance) const NITROGEN: HelmholtzResidual = HelmholtzResidual {
    reducing_temperature: Kelvin::new(126.192),
    reducing_density: KilogramsPerCubicMetre::new(11_183.9 * 0.028_013_48),
    gas_constant_j_per_kg_k: 8.314_51 / 0.028_013_48,
    power: &[
        PowerTerm::new(0, 1, 0.25, 0.924_803_575_275),
        PowerTerm::new(0, 1, 0.875, -0.492_448_489_428),
        PowerTerm::new(0, 2, 0.5, 0.661_883_336_938),
        PowerTerm::new(0, 2, 0.875, -1.929_026_492_01),
        PowerTerm::new(0, 3, 0.375, -0.062_246_930_962_9),
        PowerTerm::new(0, 3, 0.75, 0.349_943_957_581),
        PowerTerm::new(1, 1, 0.5, 0.564_857_472_498),
        PowerTerm::new(1, 1, 0.75, -1.617_200_059_87),
        PowerTerm::new(1, 1, 2.0, -0.481_395_031_883),
        PowerTerm::new(1, 3, 1.25, 0.421_150_636_384),
        PowerTerm::new(1, 3, 3.5, -0.016_196_223_082_5),
        PowerTerm::new(1, 4, 1.0, 0.172_100_994_165),
        PowerTerm::new(1, 6, 0.5, 0.007_354_489_249_33),
        PowerTerm::new(1, 6, 3.0, 0.016_807_730_547_9),
        PowerTerm::new(1, 7, 0.0, -0.001_076_266_641_79),
        PowerTerm::new(1, 7, 2.75, -0.013_731_808_851_3),
        PowerTerm::new(1, 8, 0.75, 0.000_635_466_899_859),
        PowerTerm::new(1, 8, 2.5, 0.003_044_322_794_19),
        PowerTerm::new(2, 1, 4.0, -0.043_576_233_604_5),
        PowerTerm::new(2, 2, 6.0, -0.072_317_488_931_6),
        PowerTerm::new(2, 3, 6.0, 0.038_964_431_527_2),
        PowerTerm::new(2, 4, 3.0, -0.021_220_136_391),
        PowerTerm::new(2, 5, 3.0, 0.004_088_229_815_09),
        PowerTerm::new(2, 8, 6.0, -5.519_900_179_84e-5),
        PowerTerm::new(3, 4, 16.0, -0.046_201_671_647_9),
        PowerTerm::new(3, 5, 11.0, -0.003_003_117_160_11),
        PowerTerm::new(3, 5, 15.0, 0.036_882_589_120_8),
        PowerTerm::new(3, 8, 12.0, -0.002_558_568_462_2),
        PowerTerm::new(4, 3, 12.0, 0.008_969_152_645_58),
        PowerTerm::new(4, 5, 7.0, -0.004_415_133_703_5),
        PowerTerm::new(4, 6, 4.0, 0.001_337_229_248_58),
        PowerTerm::new(4, 9, 16.0, 0.000_264_832_491_957),
    ],
    gaussian: &[
        GaussianTerm::new(1, 0.0, 19.668_819_401_5, 20.0, 325.0, 1.16, 1.0),
        GaussianTerm::new(1, 1.0, -20.911_560_073, 20.0, 325.0, 1.16, 1.0),
        GaussianTerm::new(3, 2.0, 0.016_778_830_698_9, 15.0, 300.0, 1.13, 1.0),
        GaussianTerm::new(2, 3.0, 2_627.675_662_74, 25.0, 275.0, 1.25, 1.0),
    ],
    non_analytic: &[],
    associating: &[],
};

/// Nitrogen's ideal-gas part, [`NITROGEN`]'s (Span et al. 2000), as CoolProp's `Nitrogen.json`
/// holds it (its alpha0 block): the ln τ coefficient 2.5, three power terms in τ⁻¹, τ⁻² and τ⁻³,
/// and one Planck–Einstein term, θ the file's v of 3364.011 K over its `Tcrit` of 126.192 K, the
/// residual's `T_c`. Its `c_p` and `c_v` reproduce the saturation table's (p. 1403) at 63.151, 70
/// and 77 K within 3 × 10⁻⁴ and the WebBook's within 10⁻⁶ (`liquid/tests.rs`).
pub(in crate::substance) const NITROGEN_IDEAL: HelmholtzIdeal = HelmholtzIdeal {
    log_tau: 2.5,
    power: &[
        IdealPowerTerm::new(-0.000_193_481_9, -1.0),
        IdealPowerTerm::new(-1.247_742e-5, -2.0),
        IdealPowerTerm::new(6.678_326e-8, -3.0),
    ],
    planck_einstein: &[PlanckEinsteinTerm::new(1.012_941, 3_364.011 / 126.192)],
};

/// Ethane: Bücker and Wagner 2006 (J. Phys. Chem. Ref. Data 35, 205), valid from the melting line
/// to 675 K, its residual, 39 power and exponential terms and 5 Gaussian: `T_c` = 305.322 K, `ρ_c`
/// = 6.856 886 685 mol dm⁻³, R = 8.314 472 J mol⁻¹ K⁻¹ and M = 30.069 04 g mol⁻¹. The paper was not
/// reachable (T49.b's research), so its constants are CoolProp's reading of it; its `κ_T`
/// reproduces the WebBook's evaluation of the same equation within 3 × 10⁻⁶ at four
/// saturated-liquid states from the triple point to 303.7 K.
pub(in crate::substance) const ETHANE: HelmholtzResidual = HelmholtzResidual {
    reducing_temperature: Kelvin::new(305.322),
    reducing_density: KilogramsPerCubicMetre::new(6_856.886_685 * 0.030_069_04),
    gas_constant_j_per_kg_k: 8.314_472 / 0.030_069_04,
    power: &[
        PowerTerm::new(0, 1, 0.25, 0.834_407_457_352_41),
        PowerTerm::new(0, 1, 1.0, -1.428_736_060_717_1),
        PowerTerm::new(0, 2, 0.25, 0.344_302_422_109_27),
        PowerTerm::new(0, 2, 0.75, -0.420_966_779_202_65),
        PowerTerm::new(0, 4, 0.75, 0.012_094_500_886_549),
        PowerTerm::new(1, 1, 2.0, -0.579_762_015_973_41),
        PowerTerm::new(1, 1, 4.25, -0.033_127_037_870_838),
        PowerTerm::new(1, 2, 0.75, -0.117_516_548_941_3),
        PowerTerm::new(1, 2, 2.25, -0.111_609_578_330_67),
        PowerTerm::new(1, 3, 3.0, 0.062_181_592_654_406),
        PowerTerm::new(1, 6, 1.0, 0.098_481_795_434_443),
        PowerTerm::new(1, 6, 1.25, -0.098_268_582_682_358),
        PowerTerm::new(1, 7, 2.75, -0.000_239_778_310_070_49),
        PowerTerm::new(1, 9, 1.0, 0.000_698_856_633_288_21),
        PowerTerm::new(1, 10, 2.0, 1.966_598_780_330_5e-5),
        PowerTerm::new(2, 2, 2.5, -0.014_586_152_207_928),
        PowerTerm::new(2, 4, 5.5, 0.046_354_100_536_781),
        PowerTerm::new(2, 4, 7.0, 0.006_076_462_218_064_5),
        PowerTerm::new(2, 5, 0.5, -0.002_644_733_014_782_8),
        PowerTerm::new(2, 5, 5.5, -0.042_931_872_689_904),
        PowerTerm::new(2, 6, 2.5, 0.002_998_778_651_726_3),
        PowerTerm::new(2, 8, 4.0, 0.005_291_933_517_501),
        PowerTerm::new(2, 9, 2.0, -0.001_038_389_779_819_8),
        PowerTerm::new(3, 2, 10.0, -0.054_260_348_214_694),
        PowerTerm::new(3, 3, 16.0, -0.219_593_629_184_93),
        PowerTerm::new(3, 3, 18.0, 0.353_624_566_503_54),
        PowerTerm::new(3, 3, 20.0, -0.124_773_901_737_14),
        PowerTerm::new(3, 4, 14.0, 0.184_256_935_915_17),
        PowerTerm::new(3, 4, 18.0, -0.161_922_564_367_54),
        PowerTerm::new(3, 5, 12.0, -0.082_770_876_149_064),
        PowerTerm::new(3, 5, 19.0, 0.050_160_758_096_437),
        PowerTerm::new(3, 6, 7.0, 0.009_361_432_633_665_5),
        PowerTerm::new(3, 11, 15.0, -0.000_278_391_862_428_64),
        PowerTerm::new(3, 14, 9.0, 2.356_027_407_148_1e-5),
        PowerTerm::new(4, 3, 26.0, 0.003_923_832_973_852_7),
        PowerTerm::new(4, 3, 28.0, -0.000_764_883_258_136_18),
        PowerTerm::new(4, 4, 28.0, -0.004_994_430_444_073),
        PowerTerm::new(4, 8, 22.0, 0.001_859_338_640_718_6),
        PowerTerm::new(4, 10, 13.0, -0.000_614_043_533_311_99),
    ],
    gaussian: &[
        GaussianTerm::new(1, 0.0, -0.002_331_217_936_792_4, 15.0, 150.0, 1.05, 1.0),
        GaussianTerm::new(1, 3.0, 0.002_930_104_790_876, 15.0, 150.0, 1.05, 1.0),
        GaussianTerm::new(3, 3.0, -0.000_269_124_728_428_83, 15.0, 150.0, 1.05, 1.0),
        GaussianTerm::new(3, 0.0, 184.138_341_118_14, 20.0, 275.0, 1.22, 1.0),
        GaussianTerm::new(2, 3.0, -10.397_127_984_854, 20.0, 400.0, 1.16, 1.0),
    ],
    non_analytic: &[],
    associating: &[],
};

/// Ethane's ideal-gas part, [`ETHANE`]'s (Bücker and Wagner 2006, its paper not read), as
/// CoolProp's `Ethane.json` holds it (its alpha0 block): the ln τ coefficient 3.003 039 265 and
/// four Planck–Einstein terms, θ reduced by the residual's `T_c`. Its `c_p` and `c_v` reproduce the
/// WebBook's evaluation of the same equation within 3 × 10⁻⁶ (`liquid/tests.rs`).
pub(in crate::substance) const ETHANE_IDEAL: HelmholtzIdeal = HelmholtzIdeal {
    log_tau: 3.003_039_265,
    power: &[],
    planck_einstein: &[
        PlanckEinsteinTerm::new(1.117_433_359, 1.409_105_233),
        PlanckEinsteinTerm::new(3.467_773_215, 4.009_917_071),
        PlanckEinsteinTerm::new(6.941_944_64, 6.596_709_834),
        PlanckEinsteinTerm::new(5.970_850_948, 13.979_810_27),
    ],
};

/// Ammonia: Gao, Wu, Bell, Harvey and Lemmon 2023 (J. Phys. Chem. Ref. Data 52, 013102), valid from
/// the triple point to 725 K, its residual of Table 5, 8 power and exponential terms, 10 Gaussian
/// and 2 associating: `T_c` = 405.56 K, `ρ_c` = 13.696 mol dm⁻³, R = 8.314 462 618 J mol⁻¹ K⁻¹
/// (their Table 1, where CoolProp's file holds 8.314 459 8) and M = 17.030 52 g mol⁻¹. Read in
/// NIST's author manuscript: the associating terms' η as its Table 5 prints it, positive in
/// exp(−η(δ − ε)² …), where CoolProp's file holds its negation. Its pressure and `κ_T` reproduce
/// Table 7's values for computer verification (seven figures) within 2 × 10⁻⁶ at all eight liquid,
/// vapour and supercritical states.
pub(in crate::substance) const AMMONIA: HelmholtzResidual = HelmholtzResidual {
    reducing_temperature: Kelvin::new(405.56),
    reducing_density: KilogramsPerCubicMetre::new(13_696.0 * 0.017_030_52),
    gas_constant_j_per_kg_k: 8.314_462_618 / 0.017_030_52,
    power: &[
        PowerTerm::new(0, 4, 1.0, 0.006_132_232),
        PowerTerm::new(0, 1, 0.382, 1.739_586_6),
        PowerTerm::new(0, 1, 1.0, -2.226_179_2),
        PowerTerm::new(0, 2, 1.0, -0.301_275_53),
        PowerTerm::new(0, 3, 0.677, 0.089_670_23),
        PowerTerm::new(2, 3, 2.915, -0.076_387_037),
        PowerTerm::new(2, 2, 3.51, -0.840_639_63),
        PowerTerm::new(1, 3, 1.063, -0.270_263_27),
    ],
    gaussian: &[
        GaussianTerm::new(1, 0.655, 6.212_578, 0.427_76, 1.708, 1.036, -0.072_6),
        GaussianTerm::new(1, 1.3, -5.784_435_7, 0.642_4, 1.486_5, 1.277_7, -0.127_4),
        GaussianTerm::new(1, 3.1, 2.481_754_2, 0.817_5, 2.091_5, 1.083, 0.752_7),
        GaussianTerm::new(2, 1.439_5, -2.373_916_8, 0.799_5, 2.43, 1.290_6, 0.57),
        GaussianTerm::new(2, 1.623, 0.014_936_97, 0.91, 0.488, 0.928, 2.2),
        GaussianTerm::new(1, 0.643, -3.774_926_4, 0.357_4, 1.1, 0.934, -0.243),
        GaussianTerm::new(3, 1.13, 0.000_625_434_8, 1.21, 0.85, 0.919, 2.96),
        GaussianTerm::new(3, 4.5, -1.735_9e-5, 4.14, 1.14, 1.852, 3.02),
        GaussianTerm::new(1, 1.0, -0.134_620_33, 22.56, 945.64, 1.058_97, 0.957_4),
        GaussianTerm::new(1, 4.0, 0.077_490_728_39, 22.68, 993.85, 1.052_77, 0.957_6),
    ],
    non_analytic: &[],
    associating: &[
        AssociatingTerm::new(
            1,
            4.331_5,
            -1.690_985_8,
            2.845_2,
            0.369_6,
            1.108,
            0.447_8,
            1.244,
        ),
        AssociatingTerm::new(
            1,
            4.015,
            0.937_390_74,
            2.834_2,
            0.296_2,
            1.313,
            0.446_89,
            0.682_6,
        ),
    ],
};

/// Ammonia's ideal-gas part, [`AMMONIA`]'s (Gao et al. 2023), as CoolProp's `Ammonia.json` holds
/// it (its first equation's alpha0 block, Gao et al.'s): the ln τ coefficient 3 and three
/// Planck–Einstein terms, θ reduced by the residual's `T_c`, the file's decimals of 1646, 3965
/// and 7231 K over 405.56 K. Its `c_p` and `c_v` reproduce Table 7's within 2 × 10⁻⁶ at all eight
/// states (`liquid/tests.rs`).
pub(in crate::substance) const AMMONIA_IDEAL: HelmholtzIdeal = HelmholtzIdeal {
    log_tau: 3.0,
    power: &[],
    planck_einstein: &[
        PlanckEinsteinTerm::new(2.224, 4.058_585_659_335_240_5),
        PlanckEinsteinTerm::new(3.148, 9.776_605_187_888_352),
        PlanckEinsteinTerm::new(0.957_9, 17.829_667_620_080_876),
    ],
};

/// Carbon dioxide: Span and Wagner 1996 (J. Phys. Chem. Ref. Data 25, 1509), valid from the triple
/// point to 1100 K, its residual part, 34 power and exponential terms, 5 Gaussian
/// and 3 non-analytic: `T_c` = 304.1282 K, `ρ_c` = 467.6 kg m⁻³, R = 8.314 51 J mol⁻¹ K⁻¹ and M =
/// 44.0098 g mol⁻¹ (its R = 0.188 924 1 kJ kg⁻¹ K⁻¹). Every non-analytic term has β = 0.3 < ½ and
/// a ≥ 3 > 1, the condition under which [`NonAnalyticTerm`]'s limit at δ = 1 holds. Its `κ_T`
/// reproduces Table 34's at 216.592, 250, 270, 290 and 300 K within 10⁻⁴ (five figures), and its
/// pressure Table 34's at 300 K, 4.1 K from the critical point, within 10⁻⁵.
pub(in crate::substance) const CARBON_DIOXIDE: HelmholtzResidual = HelmholtzResidual {
    reducing_temperature: Kelvin::new(304.128_2),
    reducing_density: KilogramsPerCubicMetre::new(467.6),
    gas_constant_j_per_kg_k: 8.314_51 / 0.044_009_8,
    power: &[
        PowerTerm::new(0, 1, 0.0, 0.388_568_232_032),
        PowerTerm::new(0, 1, 0.75, 2.938_547_594_27),
        PowerTerm::new(0, 1, 1.0, -5.586_718_853_5),
        PowerTerm::new(0, 1, 2.0, -0.767_531_995_925),
        PowerTerm::new(0, 2, 0.75, 0.317_290_055_804),
        PowerTerm::new(0, 2, 2.0, 0.548_033_158_978),
        PowerTerm::new(0, 3, 0.75, 0.122_794_112_203),
        PowerTerm::new(1, 1, 1.5, 2.165_896_154_32),
        PowerTerm::new(1, 2, 1.5, 1.584_173_510_97),
        PowerTerm::new(1, 4, 2.5, -0.231_327_054_055),
        PowerTerm::new(1, 5, 0.0, 0.058_116_916_431_4),
        PowerTerm::new(1, 5, 1.5, -0.553_691_372_054),
        PowerTerm::new(1, 5, 2.0, 0.489_466_159_094),
        PowerTerm::new(1, 6, 0.0, -0.024_275_739_843_5),
        PowerTerm::new(1, 6, 1.0, 0.062_494_790_501_7),
        PowerTerm::new(1, 6, 2.0, -0.121_758_602_252),
        PowerTerm::new(2, 1, 3.0, -0.370_556_852_701),
        PowerTerm::new(2, 1, 6.0, -0.016_775_879_700_4),
        PowerTerm::new(2, 4, 3.0, -0.119_607_366_38),
        PowerTerm::new(2, 4, 6.0, -0.045_619_362_508_8),
        PowerTerm::new(2, 4, 8.0, 0.035_612_789_270_3),
        PowerTerm::new(2, 7, 6.0, -0.007_442_772_713_21),
        PowerTerm::new(2, 8, 0.0, -0.001_739_570_490_24),
        PowerTerm::new(3, 2, 7.0, -0.021_810_121_289_5),
        PowerTerm::new(3, 3, 12.0, 0.024_332_166_559_2),
        PowerTerm::new(3, 3, 16.0, -0.037_440_133_423_5),
        PowerTerm::new(4, 5, 22.0, 0.143_387_157_569),
        PowerTerm::new(4, 5, 24.0, -0.134_919_690_833),
        PowerTerm::new(4, 6, 16.0, -0.023_151_225_053_5),
        PowerTerm::new(4, 7, 24.0, 0.012_363_125_492_9),
        PowerTerm::new(4, 8, 8.0, 0.002_105_832_197_29),
        PowerTerm::new(4, 10, 2.0, -0.000_339_585_190_264),
        PowerTerm::new(5, 4, 28.0, 0.005_599_365_177_16),
        PowerTerm::new(6, 8, 14.0, -0.000_303_351_180_556),
    ],
    gaussian: &[
        GaussianTerm::new(2, 1.0, -213.654_886_883, 25.0, 325.0, 1.16, 1.0),
        GaussianTerm::new(2, 0.0, 26_641.569_149_3, 25.0, 300.0, 1.19, 1.0),
        GaussianTerm::new(2, 1.0, -24_027.212_204_6, 25.0, 300.0, 1.19, 1.0),
        GaussianTerm::new(3, 3.0, -283.416_034_24, 15.0, 275.0, 1.25, 1.0),
        GaussianTerm::new(3, 3.0, 212.472_844_002, 20.0, 275.0, 1.22, 1.0),
    ],
    non_analytic: &[
        NonAnalyticTerm::new(3.5, 0.875, 0.3, -0.666_422_765_408, 10.0, 275.0, 0.7, 0.3),
        NonAnalyticTerm::new(3.5, 0.925, 0.3, 0.726_086_323_499, 10.0, 275.0, 0.7, 0.3),
        NonAnalyticTerm::new(3.0, 0.875, 1.0, 0.055_068_668_612_8, 12.5, 275.0, 0.7, 0.3),
    ],
    associating: &[],
};

/// Carbon dioxide's ideal-gas part, [`CARBON_DIOXIDE`]'s (Span and Wagner 1996), as CoolProp's
/// `CarbonDioxide.json` holds it (its alpha0 block): the ln τ coefficient 2.5 and five
/// Planck–Einstein terms, θ reduced by the residual's `T_c`. Its `c_p` and `c_v` reproduce Table
/// 34's at its five states within 10⁻⁴ and the WebBook's within 10⁻⁶ (`liquid/tests.rs`).
pub(in crate::substance) const CARBON_DIOXIDE_IDEAL: HelmholtzIdeal = HelmholtzIdeal {
    log_tau: 2.5,
    power: &[],
    planck_einstein: &[
        PlanckEinsteinTerm::new(1.994_270_42, 3.151_63),
        PlanckEinsteinTerm::new(0.621_052_48, 6.111_9),
        PlanckEinsteinTerm::new(0.411_952_93, 6.777_08),
        PlanckEinsteinTerm::new(1.040_289_22, 11.323_84),
        PlanckEinsteinTerm::new(0.083_276_78, 27.087_92),
    ],
};

/// Oxygen: Schmidt and Wagner 1985 (Fluid Phase Equilib. 19, 175), as Stewart, Jacobsen and Wagner
/// 1991 present it (J. Phys. Chem. Ref. Data 20, 917), its residual, 32 power and exponential
/// terms: `T_c` = 154.581 K, `ρ_c` = 13.63 mol dm⁻³, R = 8.314 34 J mol⁻¹ K⁻¹ and M = 31.9988 g
/// mol⁻¹, as CoolProp's `Oxygen.json` holds them (its stated limits 2000 K and 80 megapascals; the
/// papers' range was not read). Its `κ_T` reproduces the WebBook's evaluation of the same equation
/// within 10⁻⁶ (3 × 10⁻¹¹ as run) at four saturated-liquid states from 54.37 to 127.21 K.
pub(in crate::substance) const OXYGEN: HelmholtzResidual = HelmholtzResidual {
    reducing_temperature: Kelvin::new(154.581),
    reducing_density: KilogramsPerCubicMetre::new(13_630.0 * 0.031_998_8),
    gas_constant_j_per_kg_k: 8.314_34 / 0.031_998_8,
    power: &[
        PowerTerm::new(0, 1, 0.0, 0.398_376_874_9),
        PowerTerm::new(0, 1, 1.5, -1.846_157_454),
        PowerTerm::new(0, 1, 2.5, 0.418_347_319_7),
        PowerTerm::new(0, 2, -0.5, 0.023_706_207_11),
        PowerTerm::new(0, 2, 1.5, 0.097_717_305_73),
        PowerTerm::new(0, 2, 2.0, 0.030_178_912_94),
        PowerTerm::new(0, 3, 0.0, 0.022_733_532_12),
        PowerTerm::new(0, 3, 1.0, 0.013_572_540_86),
        PowerTerm::new(0, 3, 2.5, -0.040_526_989_43),
        PowerTerm::new(0, 6, 0.0, 0.000_545_462_851_5),
        PowerTerm::new(0, 7, 2.0, 0.000_511_318_227_7),
        PowerTerm::new(0, 7, 5.0, 2.953_466_883e-7),
        PowerTerm::new(0, 8, 2.0, -8.687_645_072e-5),
        PowerTerm::new(2, 1, 5.0, -0.212_708_258_9),
        PowerTerm::new(2, 1, 6.0, 0.087_359_419_58),
        PowerTerm::new(2, 2, 3.5, 0.127_550_919),
        PowerTerm::new(2, 2, 5.5, -0.090_677_010_64),
        PowerTerm::new(2, 3, 3.0, -0.035_400_842_06),
        PowerTerm::new(2, 3, 7.0, -0.036_232_780_59),
        PowerTerm::new(2, 5, 6.0, 0.013_276_992_9),
        PowerTerm::new(2, 6, 8.5, -0.000_325_411_186_5),
        PowerTerm::new(2, 7, 4.0, -0.008_313_582_932),
        PowerTerm::new(2, 8, 6.5, 0.002_124_570_559),
        PowerTerm::new(2, 10, 5.5, -0.000_832_520_623_2),
        PowerTerm::new(4, 2, 22.0, -2.626_173_276e-5),
        PowerTerm::new(4, 3, 11.0, 0.002_599_581_482),
        PowerTerm::new(4, 3, 18.0, 0.009_984_649_663),
        PowerTerm::new(4, 4, 11.0, 0.002_199_923_153),
        PowerTerm::new(4, 4, 23.0, -0.025_913_504_86),
        PowerTerm::new(4, 5, 17.0, -0.125_963_084_8),
        PowerTerm::new(4, 5, 18.0, 0.147_835_563_7),
        PowerTerm::new(4, 5, 23.0, -0.010_112_510_78),
    ],
    gaussian: &[],
    non_analytic: &[],
    associating: &[],
};

/// Argon: Tegeler, Span and Wagner 1999 (J. Phys. Chem. Ref. Data 28, 779), its residual, 37 power
/// and exponential terms and 4 Gaussian: `T_c` = 150.687 K, `ρ_c` = 535.6 kg m⁻³, R =
/// 8.314 51 J mol⁻¹ K⁻¹ and M = 39.948 g mol⁻¹, as CoolProp's `Argon.json` holds them (its stated
/// limits 2000 K and 1000 megapascals; the paper's range was not read). Its `κ_T` reproduces the
/// WebBook's evaluation of the same equation within 10⁻⁶ at four saturated-liquid states, 83.81,
/// 99.775, 115.74 and 131.705 K: within 7 × 10⁻⁹ as run at those four, at most 6.7 × 10⁻⁹ (the
/// lane's `step2b.json`, its per-state rows; 1.0 × 10⁻⁸ at 147.67 K, outside them).
pub(in crate::substance) const ARGON: HelmholtzResidual = HelmholtzResidual {
    reducing_temperature: Kelvin::new(150.687),
    reducing_density: KilogramsPerCubicMetre::new(535.6),
    gas_constant_j_per_kg_k: 8.314_51 / 0.039_948,
    power: &[
        PowerTerm::new(0, 1, 0.0, 0.088_722_304_990_011),
        PowerTerm::new(0, 1, 0.25, 0.705_148_051_672_98),
        PowerTerm::new(0, 1, 1.0, -1.682_011_565_409),
        PowerTerm::new(0, 1, 2.75, -0.149_090_144_314_86),
        PowerTerm::new(0, 1, 4.0, -0.120_248_046_009_4),
        PowerTerm::new(0, 2, 0.0, -0.121_649_787_985_99),
        PowerTerm::new(0, 2, 0.25, 0.400_359_336_267_52),
        PowerTerm::new(0, 2, 0.75, -0.271_360_626_991_29),
        PowerTerm::new(0, 2, 2.75, 0.242_119_245_796_45),
        PowerTerm::new(0, 3, 0.0, 0.005_788_958_318_557),
        PowerTerm::new(0, 3, 2.0, -0.041_097_335_615_341),
        PowerTerm::new(0, 4, 0.75, 0.024_710_761_541_614),
        PowerTerm::new(1, 1, 3.0, -0.321_813_917_507_02),
        PowerTerm::new(1, 1, 3.5, 0.332_300_176_957_94),
        PowerTerm::new(1, 3, 1.0, 0.031_019_986_287_345),
        PowerTerm::new(1, 4, 2.0, -0.030_777_086_002_437),
        PowerTerm::new(1, 4, 4.0, 0.093_891_137_419_581),
        PowerTerm::new(1, 5, 3.0, -0.090_643_210_682_031),
        PowerTerm::new(1, 7, 0.0, -0.000_457_783_492_766_54),
        PowerTerm::new(1, 10, 0.5, -8.265_972_902_519_7e-5),
        PowerTerm::new(1, 10, 1.0, 0.000_130_134_156_031_47),
        PowerTerm::new(2, 2, 1.0, -0.011_397_840_001_996),
        PowerTerm::new(2, 2, 7.0, -0.024_455_169_960_535),
        PowerTerm::new(2, 4, 5.0, -0.064_324_067_175_955),
        PowerTerm::new(2, 4, 6.0, 0.058_889_471_093_674),
        PowerTerm::new(2, 8, 6.0, -0.000_649_335_521_129_65),
        PowerTerm::new(3, 3, 10.0, -0.013_889_862_158_435),
        PowerTerm::new(3, 5, 13.0, 0.404_898_392_969_1),
        PowerTerm::new(3, 5, 14.0, -0.386_125_195_947_49),
        PowerTerm::new(3, 6, 11.0, -0.188_171_423_322_33),
        PowerTerm::new(3, 6, 14.0, 0.159_776_475_964_82),
        PowerTerm::new(3, 7, 8.0, 0.053_985_518_513_856),
        PowerTerm::new(3, 7, 14.0, -0.028_953_417_958_014),
        PowerTerm::new(3, 8, 6.0, -0.013_025_413_381_384),
        PowerTerm::new(3, 9, 7.0, 0.002_894_869_677_577_8),
        PowerTerm::new(4, 5, 24.0, -0.002_264_713_430_479_6),
        PowerTerm::new(4, 6, 22.0, 0.001_761_645_619_636_8),
    ],
    gaussian: &[
        GaussianTerm::new(2, 3.0, 0.005_855_245_448_277_4, 20.0, 250.0, 1.11, 1.0),
        GaussianTerm::new(1, 1.0, -0.692_519_082_700_28, 20.0, 375.0, 1.14, 1.0),
        GaussianTerm::new(2, 0.0, 1.531_549_003_051_6, 20.0, 300.0, 1.17, 1.0),
        GaussianTerm::new(3, 0.0, -0.002_738_044_744_978_3, 20.0, 225.0, 1.11, 1.0),
    ],
    non_analytic: &[],
    associating: &[],
};

/// Krypton: Lemmon and Span 2006 (J. Chem. Eng. Data 51, 785), its short form, its residual, 12
/// power and exponential terms: `T_c` = 209.48 K, `ρ_c` = 10.85 mol dm⁻³, R = 8.314 472 J mol⁻¹
/// K⁻¹ and M = 83.798 g mol⁻¹, as CoolProp's `Krypton.json` holds them (its stated limits 750 K and
/// 200 megapascals; the paper's range was not read). Its `κ_T` meets the WebBook's at four
/// saturated-liquid states from 115.78 to 182.9125 K within 2 × 10⁻⁶, and, scaled by the ratio of
/// R to CODATA 2014's 8.314 459 8, which the WebBook evaluates it with, within 10⁻⁹.
pub(in crate::substance) const KRYPTON: HelmholtzResidual = HelmholtzResidual {
    reducing_temperature: Kelvin::new(209.48),
    reducing_density: KilogramsPerCubicMetre::new(10_850.0 * 0.083_798),
    gas_constant_j_per_kg_k: 8.314_472 / 0.083_798,
    power: &[
        PowerTerm::new(0, 1, 0.25, 0.835_61),
        PowerTerm::new(0, 1, 1.125, -2.372_5),
        PowerTerm::new(0, 1, 1.5, 0.545_67),
        PowerTerm::new(0, 2, 1.375, 0.014_361),
        PowerTerm::new(0, 3, 0.25, 0.066_502),
        PowerTerm::new(0, 7, 0.875, 0.000_193_1),
        PowerTerm::new(1, 2, 0.625, 0.168_18),
        PowerTerm::new(1, 5, 1.75, -0.033_133),
        PowerTerm::new(2, 1, 3.625, -0.150_08),
        PowerTerm::new(2, 4, 3.625, -0.022_897),
        PowerTerm::new(3, 3, 14.5, -0.021_454),
        PowerTerm::new(3, 4, 12.0, 0.006_939_7),
    ],
    gaussian: &[],
    non_analytic: &[],
    associating: &[],
};

/// Xenon: Lemmon and Span 2006 (J. Chem. Eng. Data 51, 785), its short form, its residual, 12 power
/// and exponential terms: `T_c` = 289.733 K, `ρ_c` = 8.4 mol dm⁻³, R = 8.314 472 J mol⁻¹ K⁻¹ and M
/// = 131.293 g mol⁻¹, as CoolProp's `Xenon.json` holds them (its stated limits 750 K and 700
/// megapascals; the paper's range was not read). Its `κ_T` meets the WebBook's at four
/// saturated-liquid states from 161.41 to 253.3075 K within 2 × 10⁻⁶, and within 10⁻⁹ scaled by
/// the ratio of the two R, as [`KRYPTON`]'s does.
pub(in crate::substance) const XENON: HelmholtzResidual = HelmholtzResidual {
    reducing_temperature: Kelvin::new(289.733),
    reducing_density: KilogramsPerCubicMetre::new(8_400.0 * 0.131_293),
    gas_constant_j_per_kg_k: 8.314_472 / 0.131_293,
    power: &[
        PowerTerm::new(0, 1, 0.25, 0.831_15),
        PowerTerm::new(0, 1, 1.125, -2.355_3),
        PowerTerm::new(0, 1, 1.5, 0.539_04),
        PowerTerm::new(0, 2, 1.375, 0.014_382),
        PowerTerm::new(0, 3, 0.25, 0.066_309),
        PowerTerm::new(0, 7, 0.875, 0.000_196_49),
        PowerTerm::new(1, 2, 0.625, 0.149_96),
        PowerTerm::new(1, 5, 1.75, -0.035_319),
        PowerTerm::new(2, 1, 3.625, -0.159_29),
        PowerTerm::new(2, 4, 3.625, -0.027_521),
        PowerTerm::new(3, 3, 14.5, -0.023_305),
        PowerTerm::new(3, 4, 12.0, 0.008_694_1),
    ],
    gaussian: &[],
    non_analytic: &[],
    associating: &[],
};

/// Carbon monoxide: Lemmon and Span 2006 (J. Chem. Eng. Data 51, 785), its short form, its
/// residual, 12 power and exponential terms: `T_c` = 132.86 K, `ρ_c` = 10.85 mol dm⁻³, R =
/// 8.314 472 J mol⁻¹ K⁻¹ and M = 28.0101 g mol⁻¹, as CoolProp's `CarbonMonoxide.json` holds them
/// (its stated limits 500 K and 100 megapascals; the paper's range was not read). Its `κ_T` meets
/// the WebBook's at four saturated-liquid states from 68.16 to 114.69 K within 2 × 10⁻⁶, and within
/// 10⁻⁹ scaled by the ratio of the two R, as [`KRYPTON`]'s does.
pub(in crate::substance) const CARBON_MONOXIDE: HelmholtzResidual = HelmholtzResidual {
    reducing_temperature: Kelvin::new(132.86),
    reducing_density: KilogramsPerCubicMetre::new(10_850.0 * 0.028_010_1),
    gas_constant_j_per_kg_k: 8.314_472 / 0.028_010_1,
    power: &[
        PowerTerm::new(0, 1, 0.25, 0.905_54),
        PowerTerm::new(0, 1, 1.125, -2.451_5),
        PowerTerm::new(0, 1, 1.5, 0.531_49),
        PowerTerm::new(0, 2, 1.375, 0.024_173),
        PowerTerm::new(0, 3, 0.25, 0.072_156),
        PowerTerm::new(0, 7, 0.875, 0.000_188_18),
        PowerTerm::new(1, 2, 0.625, 0.194_05),
        PowerTerm::new(1, 5, 1.75, -0.043_268),
        PowerTerm::new(2, 1, 3.625, -0.127_78),
        PowerTerm::new(2, 4, 3.625, -0.027_896),
        PowerTerm::new(3, 3, 14.5, -0.034_154),
        PowerTerm::new(3, 4, 12.0, 0.016_329),
    ],
    gaussian: &[],
    non_analytic: &[],
    associating: &[],
};

/// Nitrous oxide: Lemmon and Span 2006 (J. Chem. Eng. Data 51, 785), its short form, its residual,
/// 12 power and exponential terms: `T_c` = 309.52 K, `ρ_c` = 10.27 mol dm⁻³, R =
/// 8.314 472 J mol⁻¹ K⁻¹ and M = 44.0128 g mol⁻¹, as CoolProp's `NitrousOxide.json` holds them (its
/// stated limits 525 K and 50 megapascals; the paper's range was not read). Its `κ_T` meets the
/// WebBook's at four saturated-liquid states from 182.34 to 273.0825 K within 2 × 10⁻⁶, and within
/// 10⁻⁹ scaled by the ratio of the two R, as [`KRYPTON`]'s does.
pub(in crate::substance) const NITROUS_OXIDE: HelmholtzResidual = HelmholtzResidual {
    reducing_temperature: Kelvin::new(309.52),
    reducing_density: KilogramsPerCubicMetre::new(10_270.0 * 0.044_012_8),
    gas_constant_j_per_kg_k: 8.314_472 / 0.044_012_8,
    power: &[
        PowerTerm::new(0, 1, 0.25, 0.880_45),
        PowerTerm::new(0, 1, 1.25, -2.423_5),
        PowerTerm::new(0, 1, 1.5, 0.382_37),
        PowerTerm::new(0, 3, 0.25, 0.068_917),
        PowerTerm::new(0, 7, 0.875, 0.000_203_67),
        PowerTerm::new(1, 1, 2.375, 0.131_22),
        PowerTerm::new(1, 2, 2.0, 0.460_32),
        PowerTerm::new(1, 5, 2.125, -0.003_698_5),
        PowerTerm::new(2, 1, 3.5, -0.232_63),
        PowerTerm::new(2, 1, 6.5, -0.000_428_59),
        PowerTerm::new(2, 4, 4.75, -0.042_81),
        PowerTerm::new(3, 2, 12.5, -0.023_038),
    ],
    gaussian: &[],
    non_analytic: &[],
    associating: &[],
};

/// Hydrogen sulphide: Lemmon and Span 2006 (J. Chem. Eng. Data 51, 785), its short form, its
/// residual, 12 power and exponential terms: `T_c` = 373.1 K, `ρ_c` = 10.19 mol dm⁻³, R = 8.314 472
/// J mol⁻¹ K⁻¹ and M = 34.080 88 g mol⁻¹, as CoolProp's `HydrogenSulfide.json` holds them (its
/// stated limits 760 K and 170 megapascals; the paper's range was not read). Its `κ_T` meets the
/// WebBook's at four saturated-liquid states from 187.7 to 321.155 K within 2 × 10⁻⁶, and within
/// 10⁻⁹ scaled by the ratio of the two R, as [`KRYPTON`]'s does.
pub(in crate::substance) const HYDROGEN_SULPHIDE: HelmholtzResidual = HelmholtzResidual {
    reducing_temperature: Kelvin::new(373.1),
    reducing_density: KilogramsPerCubicMetre::new(10_190.0 * 0.034_080_88),
    gas_constant_j_per_kg_k: 8.314_472 / 0.034_080_88,
    power: &[
        PowerTerm::new(0, 1, 0.25, 0.876_41),
        PowerTerm::new(0, 1, 1.125, -2.036_7),
        PowerTerm::new(0, 1, 1.5, 0.216_34),
        PowerTerm::new(0, 2, 1.375, -0.050_199),
        PowerTerm::new(0, 3, 0.25, 0.066_994),
        PowerTerm::new(0, 7, 0.875, 0.000_190_76),
        PowerTerm::new(1, 2, 0.625, 0.202_27),
        PowerTerm::new(1, 5, 1.75, -0.004_534_8),
        PowerTerm::new(2, 1, 3.625, -0.222_3),
        PowerTerm::new(2, 4, 3.625, -0.034_714),
        PowerTerm::new(3, 3, 14.5, -0.014_885),
        PowerTerm::new(3, 4, 12.0, 0.007_415_4),
    ],
    gaussian: &[],
    non_analytic: &[],
    associating: &[],
};
