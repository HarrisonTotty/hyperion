//! The rows' liquid columns (P14.T49.h's first step): each pure liquid with an R08 file, its index
//! and absorption from that file, and water's scattering, surface tension and viscosity from its
//! IAPWS releases, with their sources.
//!
//! Every coefficient is its release's, as printed: IAPWS R6-95(2018)'s Table 2 for the residual
//! Helmholtz energy, R1-76(2014) for the surface tension and R12-08's Tables 1 and 2 for the
//! viscosity, each read in the release (IAPWS: "Publication in whole or in part is allowed in all
//! countries provided that attribution is given to the International Association for the
//! Properties of Water and Steam", attributed here and in `NOTICE`). The files' bins are
//! `tables::liquid_optics`, reduced by `hyperion-fit` from rendering plan R08's committed files.

use hyperion_surface::field::LiquidBasis;
use hyperion_surface::substance_key::SubstanceKey;

use super::super::helmholtz::{GaussianTerm, HelmholtzResidual, NonAnalyticTerm, PowerTerm};
use super::super::liquid::{
    Iapws2008Viscosity, LiquidColumns, LiquidOptics, LiquidScattering, SurfaceTensionEquation,
    ViscosityCorrelation,
};
use super::super::{Basis, Source};
use crate::tables::liquid_optics as bins;
use crate::units::{Kelvin, KilogramsPerCubicMetre};

/// Every row with liquid columns, in registry order, by key.
pub(in crate::substance) const LIQUIDS: [(SubstanceKey, LiquidColumns); 6] = [
    (SubstanceKey::new_const("H2O"), WATER),
    (SubstanceKey::new_const("CH4"), METHANE),
    (SubstanceKey::new_const("NH3"), AMMONIA),
    (SubstanceKey::new_const("N2"), NITROGEN),
    (SubstanceKey::new_const("C2H6"), ETHANE),
    (SubstanceKey::new_const("H2SO4"), SULPHURIC_ACID),
];

/// The IAPWS releases' attribution, which every water value cites beside its release.
const IAPWS_ATTRIBUTION: &str = "the International Association for the Properties of Water and \
                                 Steam (IAPWS), whose releases allow \"publication in whole or in \
                                 part … provided that attribution is given\"";

/// IAPWS-95's residual Helmholtz energy: IAPWS R6-95(2018), "Revised Release on the IAPWS
/// Formulation 1995 for the Thermodynamic Properties of Ordinary Water Substance for General and
/// Scientific Use" (Prague, 2018; Wagner and Pruß 2002, J. Phys. Chem. Ref. Data 31, 387), Eqs.
/// (1)–(3) and Table 2's 56 terms as printed: `T_c` = 647.096 K, `ρ_c` = 322 kg m⁻³ and R =
/// 461.51805 J kg⁻¹ K⁻¹, the value its coefficients were fitted with (its §2). A test reproduces
/// Table 6's derivatives at 500 K and 838.025 kg m⁻³ and Table 7's pressures.
pub(in crate::substance) const IAPWS_95: HelmholtzResidual = HelmholtzResidual {
    reducing_temperature: Kelvin::new(647.096),
    reducing_density: KilogramsPerCubicMetre::new(322.0),
    gas_constant_j_per_kg_k: 461.518_05,
    power: &[
        PowerTerm::new(0, 1, -0.5, 0.125_335_479_355_23e-1),
        PowerTerm::new(0, 1, 0.875, 0.789_576_347_228_28e1),
        PowerTerm::new(0, 1, 1.0, -0.878_032_033_035_61e1),
        PowerTerm::new(0, 2, 0.5, 0.318_025_093_454_18),
        PowerTerm::new(0, 2, 0.75, -0.261_455_338_593_58),
        PowerTerm::new(0, 3, 0.375, -0.781_997_516_879_81e-2),
        PowerTerm::new(0, 4, 1.0, 0.880_894_931_021_34e-2),
        PowerTerm::new(1, 1, 4.0, -0.668_565_723_079_65),
        PowerTerm::new(1, 1, 6.0, 0.204_338_109_509_65),
        PowerTerm::new(1, 1, 12.0, -0.662_126_050_396_87e-4),
        PowerTerm::new(1, 2, 1.0, -0.192_327_211_560_02),
        PowerTerm::new(1, 2, 5.0, -0.257_090_430_034_38),
        PowerTerm::new(1, 3, 4.0, 0.160_748_684_862_51),
        PowerTerm::new(1, 4, 2.0, -0.400_928_289_258_07e-1),
        PowerTerm::new(1, 4, 13.0, 0.393_434_226_032_54e-6),
        PowerTerm::new(1, 5, 9.0, -0.759_413_770_881_44e-5),
        PowerTerm::new(1, 7, 3.0, 0.562_509_793_518_88e-3),
        PowerTerm::new(1, 9, 4.0, -0.156_086_522_571_35e-4),
        PowerTerm::new(1, 10, 11.0, 0.115_379_964_229_51e-8),
        PowerTerm::new(1, 11, 4.0, 0.365_821_651_442_04e-6),
        PowerTerm::new(1, 13, 13.0, -0.132_511_800_746_68e-11),
        PowerTerm::new(1, 15, 1.0, -0.626_395_869_124_54e-9),
        PowerTerm::new(2, 1, 7.0, -0.107_936_009_089_32),
        PowerTerm::new(2, 2, 1.0, 0.176_114_910_087_52e-1),
        PowerTerm::new(2, 2, 9.0, 0.221_322_951_675_46),
        PowerTerm::new(2, 2, 10.0, -0.402_476_697_635_28),
        PowerTerm::new(2, 3, 10.0, 0.580_833_999_857_59),
        PowerTerm::new(2, 4, 3.0, 0.499_691_469_908_06e-2),
        PowerTerm::new(2, 4, 7.0, -0.313_587_007_125_49e-1),
        PowerTerm::new(2, 4, 10.0, -0.743_159_297_103_41),
        PowerTerm::new(2, 5, 10.0, 0.478_073_299_154_80),
        PowerTerm::new(2, 6, 6.0, 0.205_279_408_959_48e-1),
        PowerTerm::new(2, 6, 10.0, -0.136_364_351_103_43),
        PowerTerm::new(2, 7, 10.0, 0.141_806_344_006_17e-1),
        PowerTerm::new(2, 9, 1.0, 0.833_265_048_807_13e-2),
        PowerTerm::new(2, 9, 2.0, -0.290_523_360_095_85e-1),
        PowerTerm::new(2, 9, 3.0, 0.386_150_855_742_06e-1),
        PowerTerm::new(2, 9, 4.0, -0.203_934_865_137_04e-1),
        PowerTerm::new(2, 9, 8.0, -0.165_540_500_637_34e-2),
        PowerTerm::new(2, 10, 6.0, 0.199_555_719_795_41e-2),
        PowerTerm::new(2, 10, 9.0, 0.158_703_083_241_57e-3),
        PowerTerm::new(2, 12, 8.0, -0.163_885_683_425_30e-4),
        PowerTerm::new(3, 3, 16.0, 0.436_136_157_238_11e-1),
        PowerTerm::new(3, 4, 22.0, 0.349_940_054_637_65e-1),
        PowerTerm::new(3, 4, 23.0, -0.767_881_978_446_21e-1),
        PowerTerm::new(3, 5, 23.0, 0.224_462_773_320_06e-1),
        PowerTerm::new(4, 14, 10.0, -0.626_897_104_146_85e-4),
        PowerTerm::new(6, 3, 50.0, -0.557_111_185_656_45e-9),
        PowerTerm::new(6, 6, 44.0, -0.199_057_183_544_08),
        PowerTerm::new(6, 6, 46.0, 0.317_774_973_307_38),
        PowerTerm::new(6, 6, 50.0, -0.118_411_824_259_81),
    ],
    gaussian: &[
        GaussianTerm::new(3, 0.0, -0.313_062_603_234_35e2, 20.0, 150.0, 1.21, 1.0),
        GaussianTerm::new(3, 1.0, 0.315_461_402_377_81e2, 20.0, 150.0, 1.21, 1.0),
        GaussianTerm::new(3, 4.0, -0.252_131_543_416_95e4, 20.0, 250.0, 1.25, 1.0),
    ],
    non_analytic: &[
        NonAnalyticTerm::new(
            3.5,
            0.85,
            0.2,
            -0.148_746_408_567_24,
            28.0,
            700.0,
            0.32,
            0.3,
        ),
        NonAnalyticTerm::new(3.5, 0.95, 0.2, 0.318_061_108_784_44, 32.0, 800.0, 0.32, 0.3),
    ],
};

/// Water's surface tension against its vapour, IAPWS R1-76(2014), "Revised Release on Surface
/// Tension of Ordinary Water Substance" (Moscow, 2014): σ = B τ^μ (1 + bτ), τ = 1 − T ÷ `T_c`, with
/// B = 235.8 mN m⁻¹, b = −0.625, μ = 1.256 and `T_c` = 647.096 K, valid from the triple point to
/// the critical point (and, extrapolated, to −25 °C). Written in Mulero et al.'s two-term form, σ₁
/// = B and n₁ = μ, σ₂ = bB = −0.147 375 N m⁻¹ and n₂ = μ + 1, so it reproduces the release's Table
/// 1 (its calculated column) to its printed 0.01 mN m⁻¹, as a test holds.
const IAPWS_SURFACE_TENSION: SurfaceTensionEquation = SurfaceTensionEquation {
    critical_temperature: Kelvin::new(647.096),
    terms: &[(0.235_8, 1.256), (-0.147_375, 2.256)],
    valid_from: Kelvin::new(273.16),
    valid_to: Kelvin::new(647.096),
    basis: LiquidBasis::Measured,
};

/// IAPWS R12-08's viscosity of water (Berlin, 2008; Huber et al. 2009, J. Phys. Chem. Ref. Data 38,
/// 101): its reference constants (Eqs. 1–4), Table 1's Hᵢ and Table 2's non-zero Hᵢⱼ, as printed.
/// Valid at stable states from 273.16 to 1,173.15 K up to 300 megapascals (Eq. 9); the critical
/// enhancement μ₂ is omitted (set to 1, its §2.8), which its Eq. (13) bounds under 2% outside
/// 645.91–650.77 K and 245.8–405.3 kg m⁻³. A test reproduces its Table 4.
pub(in crate::substance) const IAPWS_VISCOSITY: Iapws2008Viscosity = Iapws2008Viscosity {
    reference_temperature: Kelvin::new(647.096),
    reference_density: KilogramsPerCubicMetre::new(322.0),
    reference_viscosity_pa_s: 1.00e-6,
    dilute: [1.677_52, 2.204_62, 0.636_656_4, -0.241_605],
    residual: &[
        (0, 0, 5.200_94e-1),
        (1, 0, 8.508_95e-2),
        (2, 0, -1.083_74),
        (3, 0, -2.895_55e-1),
        (0, 1, 2.225_31e-1),
        (1, 1, 9.991_15e-1),
        (2, 1, 1.887_97),
        (3, 1, 1.266_13),
        (5, 1, 1.205_73e-1),
        (0, 2, -2.813_78e-1),
        (1, 2, -9.068_51e-1),
        (2, 2, -7.724_79e-1),
        (3, 2, -4.898_37e-1),
        (4, 2, -2.570_40e-1),
        (0, 3, 1.619_13e-1),
        (1, 3, 2.573_99e-1),
        (0, 4, -3.253_72e-2),
        (3, 4, 6.984_52e-2),
        (4, 5, 8.721_02e-3),
        (3, 6, -4.356_73e-3),
        (5, 6, -5.932_64e-4),
    ],
    valid_from: Kelvin::new(273.16),
    valid_to: Kelvin::new(1_173.15),
    critical_region: (
        Kelvin::new(645.91),
        Kelvin::new(650.77),
        KilogramsPerCubicMetre::new(245.8),
        KilogramsPerCubicMetre::new(405.3),
    ),
};

/// Water's depolarisation ratio at 90°, 0.039 (decision-p14-t49h-liquids §7(g)):
///
/// - it is Jonasz and Fournier 2007's value (Light Scattering by Particles in Water, Academic
///   Press) from Farinato and Rowell 1976's data ("New values of the light scattering
///   depolarization and anisotropy of water", J. Chem. Phys. 65, 593, doi:10.1063/1.433115), as
///   Twardowski et al. 2007 (Biogeosciences 4, 1041, p. 1055) report it, read by that decision;
/// - Zhang, Hu and He 2009 adopt it, their published code's default (`betasw_ZHH2009.m`:
///   `delta = 0.039; % Farinato and Roswell (1976)`), and with it Zhang and Hu 2009 (Opt. Express
///   17, 1671, doi:10.1364/OE.17.001671, its abstract) find the PMH model's pure water within 2% of
///   Morel 1968's measurement;
/// - the alternative is Buiteveld et al. 1994's 0.051, which raises b by 2.1%, consistent with
///   Farinato and Rowell's unfiltered `ρ_v` = 0.026 (their abstract) as 2`ρ_v` ÷ (1 + `ρ_v`);
///   Morel's and Shifrin's 0.09 would raise it by 9.3%. Farinato and Rowell's body and Jonasz and
///   Fournier's book are closed.
const WATER_DEPOLARISATION: f64 = 0.039;

/// The temperatures over which water's carried index was checked against IAPWS R9-97 at the
/// record's own state, 273.16–373.15 K: there the carry's worst bin misses by −4.59 × 10⁻⁴ at the
/// triple point (380–405 nm), −1.33 × 10⁻⁴ at 288.15 K and +2.69 × 10⁻⁴ at 353.15 K, inside
/// decision-r11-palette-liquids §1.8's 5 × 10⁻⁴, and above it the miss grows (−4.0 × 10⁻⁴ at 423
/// K, −1.0 × 10⁻³ at 473 K, past R9-97's own 10⁻³ there), so a hotter record's index is
/// `Derived`.
const WATER_INDEX_CHECKED: (Kelvin, Kelvin) = (Kelvin::new(273.16), Kelvin::new(373.15));

/// The temperatures over which water's absorption, measured at 22–23 °C, keeps its basis,
/// 273.16–303.15 K, decision-r11-palette-liquids §1.8's checking range for water's record
/// (decision-p14-t49h-liquids §1). Röttgers, McKee and Utschig 2014 (Opt. Express 22, 25093,
/// doi:10.1364/OE.22.025093), as IOCCG 2018's Table 1.1 reprints their da ÷ dT, put the held
/// absorption's departure, per bake bin from 22.5 °C, at none below 431 nm (below their detection)
/// and at most 0.45% °C⁻¹ (735–760 nm): 3.4% at 15 and 30 °C and 10.1% at the triple point in the
/// worst bin, 17% at 60 °C. Outside the window the absorption, and so the record, is `Derived`;
/// carrying da ÷ dT is deferred (§7(b)).
const WATER_ABSORPTION_CHECKED: (Kelvin, Kelvin) = (Kelvin::new(273.16), Kelvin::new(303.15));

/// Liquid water: every member measured, so a record inside its releases' ranges and its index's
/// and absorption's checked windows is `Measured`.
const WATER: LiquidColumns = LiquidColumns {
    optics: Some(LiquidOptics {
        index: &bins::WATER_N,
        absorption: Some((&bins::WATER_A, LiquidBasis::Measured)),
        state: Kelvin::new(298.15),
        index_basis: LiquidBasis::Measured,
        index_checked: Some(WATER_INDEX_CHECKED),
        absorption_checked: Some(WATER_ABSORPTION_CHECKED),
    }),
    scattering: Some(LiquidScattering {
        equation: &IAPWS_95,
        depolarisation_ratio: WATER_DEPOLARISATION,
        basis: LiquidBasis::Measured,
    }),
    surface_tension: Some(IAPWS_SURFACE_TENSION),
    viscosity: Some(ViscosityCorrelation::Iapws2008(&IAPWS_VISCOSITY)),
    sources: &[
        Source {
            covers: "n, a",
            citation: "R08's `water.json` (decision-r08-t5g; R08.T5.h): n from IAPWS R9-97's Eq. 1 \
                       at 298.15 K and 997.0476 kg m⁻³ (its reduction), the state the index is \
                       carried from; k from Mason, Cone and Fry 2016 (Appl. Opt. 55, 7163) over \
                       380–495 nm at 23 °C, Pope and Fry 1997 (Appl. Opt. 36, 8710) over 500–725 \
                       nm and Kou, Labrie and Chýlek 1993 (Appl. Opt. 32, 3531) over 730–780 nm at \
                       22 °C, as IOCCG 2018's Table 1.1 lists them; reduced to the bake bins by \
                       `tables::liquid_optics`. The absorption's temperature dependence is not \
                       carried (near 740 nm 0.0161 m⁻¹ °C⁻¹, IOCCG's Table 1.1)",
            basis: Basis::Reduced,
        },
        Source {
            covers: "b",
            citation: "Einstein–Smoluchowski–Cabannes by Zhang, Hu and He 2009 (Opt. Express 17, \
                       5698, doi:10.1364/OE.17.005698), their published code's density term \
                       (`betasw_ZHH2009.m`, as ooici/ion-functions mirrors it at commit aaf3cfa3, \
                       SHA-256 089a2d5d…6db9fdb), at S = 0, with the PMH model's ρ ∂n² ÷ ∂ρ \
                       (Proutière, Megnassan and Hucteau 1992, J. Phys. Chem. 96, 3485, \
                       doi:10.1021/j100187a058, not read), δ = 0.039 (Jonasz and Fournier 2007's \
                       from Farinato and Rowell 1976's data, as Twardowski et al. 2007, \
                       Biogeosciences 4, 1041, p. 1055, report and ZHH adopt it; 0.051, Buiteveld \
                       et al. 1994, the alternative, +2.1% in b; decision-p14-t49h-liquids §7(g)) \
                       and κ_T from IAPWS-95 (IAPWS R6-95(2018)) at the record's density",
            basis: Basis::Fact,
        },
        Source {
            covers: "σ",
            citation: "IAPWS R1-76(2014), σ = 235.8 mN m⁻¹ τ^1.256 (1 − 0.625 τ), τ = 1 − T ÷ \
                       647.096 K, written as Mulero et al. 2012's two terms",
            basis: Basis::Reduced,
        },
        Source {
            covers: "μ",
            citation: "IAPWS R12-08 (Huber et al. 2009, J. Phys. Chem. Ref. Data 38, 101), Eqs. \
                       10–12 with μ₂ = 1 (its §2.8), at the record's density",
            basis: Basis::Fact,
        },
        Source {
            covers: "attribution",
            citation: IAPWS_ATTRIBUTION,
            basis: Basis::Fact,
        },
    ],
};

/// Liquid methane: the index from Martonchik and Orton's 90 K liquid, carried; its absorption the
/// file's. The scattering, surface tension and viscosity are P14.T49.h's next step's (Setzmann
/// and Wagner 1991's equation of state, Mulero et al. 2012, a reference correlation).
///
/// The file's 90 K lies 0.694 K below methane's triple point, where the row's ancillary density
/// is not evaluated: its density there is the line at the ancillary's triple-point slope
/// (decision-p14-t49b-condensed §2.5), so the index is `Derived`. Its absorption is `Derived`
/// (decision-p14-t49h-liquids §2, as Martonchik and Orton's paper reads):
///
/// - their Table 2's visible k are local maxima only, every one marked "+", none a minimum, and
///   the file interpolates linearly between them;
/// - nothing was measured shortward of 18,100 cm⁻¹ (552.5 nm): "We could find no absorption
///   measurements of liquid methane in this spectral region" (p. 8309), and the table's 400 nm
///   zero is an unmarked anchor, from which the file interpolates to the 583.1 nm maximum;
/// - the 90 K values are their 111 K spectrum scaled by the density ratio (p. 8310), uncertain by
///   25% (p. 8309).
///
/// R08's re-reduction of the file's visible k (decision-p14-t49h-liquids §2.4) refits the table
/// before P14.T54.a.
const METHANE: LiquidColumns = LiquidColumns {
    optics: Some(LiquidOptics {
        index: &bins::METHANE_LIQUID_N,
        absorption: Some((&bins::METHANE_LIQUID_A, LiquidBasis::Derived)),
        state: Kelvin::new(90.0),
        index_basis: LiquidBasis::Measured,
        index_checked: None,
        absorption_checked: None,
    }),
    scattering: None,
    surface_tension: None,
    viscosity: None,
    sources: &[Source {
        covers: "n, a",
        citation: "R08's `methane-liquid.json`: Martonchik and Orton 1994 (Appl. Opt. 33, 8306), \
                   the liquid at 90 K, through refractiveindex.info (CC0): its visible k Table 2's \
                   local maxima only, nothing measured shortward of 552.5 nm, the 400 nm zero an \
                   anchor, and the 90 K values the 111 K values scaled by density, 25% \
                   (decision-p14-t49h-liquids §2.1); reduced to the bake bins by \
                   `tables::liquid_optics`",
        basis: Basis::Reduced,
    }],
};

/// Liquid ammonia: the index levelled to Wong and Anderson 1972's measurement, `Derived` (its slope
/// a Lorentz–Lorenz estimate from the gas, decision-r08-t5g §3). Its absorption is not modelled:
/// the file's k = 0 lies under Robertson and Williams 1973's total attenuation, which bounds the
/// liquid's k only to 6.7 × 10⁻⁷ at 400 nm, and the palette's ammonia absorption stays absent so
/// that no ammonia sea is drawn clear to any depth (decision-r08-t5g §5).
const AMMONIA: LiquidColumns = LiquidColumns {
    optics: Some(LiquidOptics {
        index: &bins::AMMONIA_LIQUID_N,
        absorption: None,
        state: Kelvin::new(239.80),
        index_basis: LiquidBasis::Derived,
        index_checked: None,
        absorption_checked: None,
    }),
    scattering: None,
    surface_tension: None,
    viscosity: None,
    sources: &[Source {
        covers: "n",
        citation: "R08's `ammonia-liquid.json`: a Cauchy line levelled to Wong and Anderson 1972's \
                   1.360 at 632.8 nm and 239.80 K (J. Opt. Soc. Am. 62, 219), its slope \
                   Cuthbertson 1914's gas dispersion carried by Lorentz–Lorenz; the file's 240 K \
                   stands for 239.80 K, the state the index is carried from",
        basis: Basis::Reduced,
    }],
};

/// Liquid nitrogen: the index a Cauchy fit to Augustin 1915's measured liquid at 77.32 K,
/// extrapolated beyond 435.9–656.3 nm, so `Derived`. Its absorption is not modelled: the file holds
/// k = 0 because no measurement of the liquid's visible absorption was found, and Liveing and
/// Dewar's "colourless" bounds it only over a laboratory path, not a sea's depth (§1.4; recorded as
/// a choice in plan 14's Risks).
const NITROGEN: LiquidColumns = LiquidColumns {
    optics: Some(LiquidOptics {
        index: &bins::NITROGEN_LIQUID_N,
        absorption: None,
        state: Kelvin::new(77.32),
        index_basis: LiquidBasis::Derived,
        index_checked: None,
        absorption_checked: None,
    }),
    scattering: None,
    surface_tension: None,
    viscosity: None,
    sources: &[Source {
        covers: "n",
        citation: "R08's `nitrogen-liquid.json`: a two-term Cauchy fit to Augustin 1915's indices \
                   of argon-free liquid N2 at 77.32 K (Ann. Phys. 351, 419; Zenodo 1514490, CC0), \
                   the state the index is carried from",
        basis: Basis::Reduced,
    }],
};

/// Liquid ethane: the index levelled to Hagermann et al. 2005's measured 1.3887 at 670 nm, 94 K and
/// 1.5 bar, its slope a Lorentz–Lorenz estimate, so `Derived` (decision-r08-t5g §3). Its
/// absorption is not modelled until a measured visible spectrum is found
/// (decision-r11-palette-liquids §1.8).
const ETHANE: LiquidColumns = LiquidColumns {
    optics: Some(LiquidOptics {
        index: &bins::ETHANE_LIQUID_N,
        absorption: None,
        state: Kelvin::new(94.0),
        index_basis: LiquidBasis::Derived,
        index_checked: None,
        absorption_checked: None,
    }),
    scattering: None,
    surface_tension: None,
    viscosity: None,
    sources: &[Source {
        covers: "n",
        citation: "R08's `ethane-liquid.json` (R08.T5.h): a Cauchy line levelled to 1.3887 at 670 \
                   nm, Hagermann et al. 2005's pure-ethane index (MNRAS 359, 637, Table 3, at \
                   Table 2's 94 K and 1.5 bar), its slope Loria 1909's gas dispersion carried by \
                   Lorentz–Lorenz; 94 K is the state the index is carried from",
        basis: Basis::Reduced,
    }],
};

/// Aqueous sulphuric acid at 75 wt%, the row's own composition (P14.T49.b;
/// decision-p14-t49b-condensed §2.6): Palmer and Williams's 300 K index, `Measured` at the row's
/// stated density's 300 K and held elsewhere, and their visible k, an upper bound (the shortest
/// measured value, 2.07 × 10⁻⁸ at 701.8 nm, held to the blue), so the absorption is `Derived`.
const SULPHURIC_ACID: LiquidColumns = LiquidColumns {
    optics: Some(LiquidOptics {
        index: &bins::SULPHURIC_ACID_75_N,
        absorption: Some((&bins::SULPHURIC_ACID_75_A, LiquidBasis::Derived)),
        state: Kelvin::new(300.0),
        index_basis: LiquidBasis::Measured,
        index_checked: None,
        absorption_checked: None,
    }),
    scattering: None,
    surface_tension: None,
    viscosity: None,
    sources: &[Source {
        covers: "n, a",
        citation: "R08's `sulphuric-acid-75.json`: Palmer and Williams 1975 (Appl. Opt. 14, 208), \
                   the 75 wt% acid at 300 K, through Oxford's ARIA; in the visible n at five \
                   wavelengths and k held at its shortest measured value, an upper bound; reduced \
                   to the bake bins by `tables::liquid_optics`",
        basis: Basis::Reduced,
    }],
};
