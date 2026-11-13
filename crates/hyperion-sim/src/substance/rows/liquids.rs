//! The rows' liquid columns (P14.T49.h): each pure liquid with an R08 file, its index and
//! absorption from that file; water's scattering, surface tension and viscosity from its IAPWS
//! releases (the first step); methane's, nitrogen's, ethane's, ammonia's and carbon dioxide's from
//! their reference equations of state (`rows/equations.rs`), Mulero, Cachadiña and Parra 2012's
//! surface tensions and each one's reference viscosity correlation (the second step); the
//! solutions (the third step): Titan's seas, an ideal mixture of three of them at Ligeia Mare's
//! composition (decision-p14-t49b-condensed §1), with its scattering; the chloride and sulphate
//! brines, aqueous electrolytes of water and a salt row; the perchlorate brines, whose solutes have
//! no rows yet; ammonia–water at its eutectic; and aqueous sulphuric acid, the `H2SO4` row's
//! liquid, from 25 to 85 wt% between Palmer and Williams's two measured compositions
//! (decision-p14-t49h-step3-composition); and the nine rows R08.T5.k gives liquid files (step 2b,
//! decision-p14-t49h-gas-index §1.2): carbon dioxide's index joins its columns, and oxygen, argon,
//! krypton, xenon, nitrous oxide, carbon monoxide, sulphur dioxide and hydrogen sulphide take their
//! files' index (oxygen its absorption too), their reference equations' scattering, but sulphur
//! dioxide's, and Mulero et al.'s surface tensions, with no viscosity, none being ruled for them.
//! The lithologies' silicate melts (the fourth step) are `rows/melts.rs`'s, listed here in registry
//! order.
//!
//! Every water coefficient is its release's, as printed: IAPWS R6-95(2018)'s Table 2 for the
//! residual Helmholtz energy, R1-76(2014) for the surface tension and R12-08's Tables 1 and 2 for
//! the viscosity, each read in the release (IAPWS: "Publication in whole or in part is allowed in
//! all countries provided that attribution is given to the International Association for the
//! Properties of Water and Steam", attributed here and in `NOTICE`). Each viscosity correlation's
//! coefficients are its paper's, read in it and checked against its own values for computer
//! verification. The surface tensions are Mulero et al.'s coefficients as CoolProp's fluid files
//! hold them (MIT, `NOTICE`), the paper being closed, each checked against NIST's WebBook, which
//! evaluates the same correlation with the reference equation's critical temperature, as this
//! does. The files' bins are `tables::liquid_optics`, reduced by `hyperion-fit` from rendering plan
//! R08's committed files.
//!
//! The six pure liquids' saturated-liquid specific heats `c_l`(T) (decision-r09-t13i §2.1) are
//! their reference equations of state's, evaluated here (decision-r09-t13i-cl-licence §6): each
//! one's `c_p` at its saturated liquid, at the row's diagram saturation pressure, over the ruling's
//! range and held at its ends ([`LiquidSpecificHeat`]). Water's ideal-gas part is IAPWS
//! R6-95(2018)'s Table 1, read in the release; the other five's are `rows/equations.rs`'s, beside
//! their residuals.

use hyperion_surface::field::LiquidBasis;
use hyperion_surface::substance_key::SubstanceKey;

use super::super::element::Element;
use super::super::helmholtz::{
    GaussianTerm, HelmholtzIdeal, HelmholtzResidual, NonAnalyticTerm, PlanckEinsteinTerm, PowerTerm,
};
use super::super::liquid::{
    AcidOptics, AqueousAcid, AqueousElectrolyte, CorrelationLength, CriticalRegion,
    Friend1991Viscosity, Huber2024Viscosity, Iapws2008Viscosity, IdealLiquidMixing,
    LaeseckeMuzny2017Viscosity, LaliberteCooperDensity, LaliberteViscosity, LiquidColumns,
    LiquidConstitution, LiquidOptics, LiquidScattering, LiquidSpecificHeat, MixingModel,
    Monogenidou2018Viscosity, MyhreDensity, RowlessSolute, SoluteAbsorption, SolutionColumns,
    Sotiriadou2026Viscosity, SupercooledViscosity, SurfaceTensionEquation, VehkamakiSurfaceTension,
    ViscosityCorrelation, ViscosityRange, WilliamsLongViscosity,
};
use super::super::{Basis, Source, SubstanceId};
use super::{equations, melts};
use crate::tables::liquid_optics as bins;
use crate::units::{Kelvin, KilogramsPerCubicMetre, Pascals};

/// Every row with liquid columns, in registry order, by key: the pure liquids, among them the nine
/// rows R08.T5.k gives a liquid file (decision-p14-t49h-gas-index §1.2: P14.T49.h's step 2b), the
/// solutions and the lithologies' silicate melts (`rows/melts.rs`).
pub(in crate::substance) const LIQUIDS: [(SubstanceKey, LiquidColumns); 32] = [
    (SubstanceKey::new_const("H2O"), WATER),
    (SubstanceKey::new_const("CH4"), METHANE),
    (SubstanceKey::new_const("NH3"), AMMONIA),
    (SubstanceKey::new_const("N2"), NITROGEN),
    (SubstanceKey::new_const("O2"), OXYGEN),
    (SubstanceKey::new_const("CO2"), CARBON_DIOXIDE),
    (SubstanceKey::new_const("Ar"), ARGON),
    (SubstanceKey::new_const("Kr"), KRYPTON),
    (SubstanceKey::new_const("Xe"), XENON),
    (SubstanceKey::new_const("N2O"), NITROUS_OXIDE),
    (SubstanceKey::new_const("CO"), CARBON_MONOXIDE),
    (SubstanceKey::new_const("SO2"), SULPHUR_DIOXIDE),
    (SubstanceKey::new_const("H2S"), HYDROGEN_SULPHIDE),
    (SubstanceKey::new_const("C2H6"), ETHANE),
    (SubstanceKey::new_const("H2SO4"), SULPHURIC_ACID),
    (SubstanceKey::new_const("mars_dust"), melts::MARS_DUST),
    (SubstanceKey::new_const("brine_nacl"), BRINE_NACL),
    (SubstanceKey::new_const("brine_mgso4"), BRINE_MGSO4),
    (SubstanceKey::new_const("brine_naclo4"), BRINE_NACLO4),
    (SubstanceKey::new_const("brine_mgclo4"), BRINE_MGCLO4),
    (SubstanceKey::new_const("nh3_h2o"), AMMONIA_WATER),
    (SubstanceKey::new_const("ch4_c2h6_n2"), TITAN_SEA),
    (SubstanceKey::new_const("anorthosite"), melts::ANORTHOSITE),
    (SubstanceKey::new_const("feldspathic"), melts::FELDSPATHIC),
    (SubstanceKey::new_const("basalt"), melts::BASALT),
    (SubstanceKey::new_const("basalt_fe"), melts::BASALT_FE),
    (SubstanceKey::new_const("komatiite"), melts::KOMATIITE),
    (
        SubstanceKey::new_const("basalt_reduced"),
        melts::BASALT_REDUCED,
    ),
    (SubstanceKey::new_const("granite"), melts::GRANITE),
    (SubstanceKey::new_const("ultramafic"), melts::ULTRAMAFIC),
    (SubstanceKey::new_const("ferric"), melts::FERRIC),
    (SubstanceKey::new_const("lava_glass"), melts::LAVA_GLASS),
];

/// The rows the liquid columns name as components, by registry index, which is append-only and
/// never renumbered: `each_liquid_s_components_are_its_rows` holds each to its key.
const WATER_ID: SubstanceId = SubstanceId(2);
/// [`WATER_ID`]'s methane.
const METHANE_ID: SubstanceId = SubstanceId(3);
/// [`WATER_ID`]'s ammonia.
const AMMONIA_ID: SubstanceId = SubstanceId(4);
/// [`WATER_ID`]'s nitrogen.
const NITROGEN_ID: SubstanceId = SubstanceId(5);
/// [`WATER_ID`]'s oxygen.
const OXYGEN_ID: SubstanceId = SubstanceId(6);
/// [`WATER_ID`]'s carbon dioxide.
const CARBON_DIOXIDE_ID: SubstanceId = SubstanceId(7);
/// [`WATER_ID`]'s argon.
const ARGON_ID: SubstanceId = SubstanceId(8);
/// [`WATER_ID`]'s krypton.
const KRYPTON_ID: SubstanceId = SubstanceId(10);
/// [`WATER_ID`]'s xenon.
const XENON_ID: SubstanceId = SubstanceId(11);
/// [`WATER_ID`]'s nitrous oxide.
const NITROUS_OXIDE_ID: SubstanceId = SubstanceId(12);
/// [`WATER_ID`]'s carbon monoxide.
const CARBON_MONOXIDE_ID: SubstanceId = SubstanceId(13);
/// [`WATER_ID`]'s sulphur dioxide.
const SULPHUR_DIOXIDE_ID: SubstanceId = SubstanceId(14);
/// [`WATER_ID`]'s hydrogen sulphide.
const HYDROGEN_SULPHIDE_ID: SubstanceId = SubstanceId(15);
/// [`WATER_ID`]'s ethane.
const ETHANE_ID: SubstanceId = SubstanceId(20);
/// [`WATER_ID`]'s sulphuric acid, the molecule, whose liquid is its aqueous acid.
const SULPHURIC_ACID_ID: SubstanceId = SubstanceId(58);
/// [`WATER_ID`]'s sodium chloride.
const SODIUM_CHLORIDE_ID: SubstanceId = SubstanceId(61);
/// [`WATER_ID`]'s magnesium sulphate.
const MAGNESIUM_SULPHATE_ID: SubstanceId = SubstanceId(100);

/// A molecular liquid's interaction-induced anisotropy ratio, 0.006: methane's, whose own
/// anisotropy is 0, calibrated on liquid `CCl₄` (a spherical top of nearly the same reduced
/// polarisability and `S_0`), whose measured `ρ_u` = 0.036 at 23 °C and 632.8 nm (Kaye and McDaniel
/// 1974, Appl. Opt. 13, 1934, Tables II and III) puts a cancellation of 25–33 on the
/// dipole-induced-dipole pair estimate; taken as every molecular row's interaction part
/// (decision-p14-t49h-gas-index §3 and §4.3). The calibration's `CCl₄` density, `κ_T` and σ are
/// from memory (about ±50%), so methane's band is x × (0.4–2).
const INTERACTION_ANISOTROPY: f64 = 0.006;

/// A molecule's own anisotropy ratio from its gas's measured depolarisation of vertically polarised
/// light, `ρ_v` (the Rayleigh line with its rotational wings, the vibrational Raman excluded): 45
/// `ρ_v` ÷ (3 − 4`ρ_v`), the inverse of [`LiquidScattering`]'s `ρ_v` at `S_0` = 1, the gas
/// (decision-p14-t49h-gas-index §4.3).
#[must_use]
const fn molecular_anisotropy(vertical_depolarisation: f64) -> f64 {
    45.0 * vertical_depolarisation / (3.0 - 4.0 * vertical_depolarisation)
}

/// Nitrogen's anisotropy ratio, 0.164: its molecular part from the gas's `ρ_v` = 0.0104 at 550 nm
/// (Bates 1984's King factor) plus [`INTERACTION_ANISOTROPY`], giving δ about 0.38 at the triple
/// point and 0.25 at 77 K (decision-p14-t49h-gas-index §4.3). An upper estimate, within −40% / +0%:
/// it neglects the orientational pair correlation (the same method gives benzene's `ρ_u` 0.56
/// against its measured 0.42), and no measured total δ of the liquid was read (§4.4).
const NITROGEN_ANISOTROPY: f64 = molecular_anisotropy(0.010_4) + INTERACTION_ANISOTROPY;

/// Carbon dioxide's anisotropy ratio, about 0.66: the gas's `ρ_v` = 0.041 at 550 nm (Sneep and
/// Ubachs 2005, Eq. 14) plus [`INTERACTION_ANISOTROPY`], giving δ about 0.46 at the triple point;
/// an upper estimate within −40% / +0%, as [`NITROGEN_ANISOTROPY`]'s.
const CARBON_DIOXIDE_ANISOTROPY: f64 = molecular_anisotropy(0.041) + INTERACTION_ANISOTROPY;

/// Ethane's anisotropy ratio, about 0.030: the gas's `ρ_v` = 0.001 59 at 632.8 nm (Keir 1995,
/// Table 6.3) plus [`INTERACTION_ANISOTROPY`], giving δ about 0.29 at the triple point, where `S_0`
/// is small; an upper estimate within −40% / +0%, as [`NITROGEN_ANISOTROPY`]'s.
const ETHANE_ANISOTROPY: f64 = molecular_anisotropy(0.001_59) + INTERACTION_ANISOTROPY;

/// Ammonia's anisotropy ratio, about 0.020: the gas's `ρ_v` = 0.000 91 at 632.8 nm (Keir 1995)
/// plus [`INTERACTION_ANISOTROPY`], giving δ about 0.07 at the triple point; an upper estimate
/// within −40% / +0%, as [`NITROGEN_ANISOTROPY`]'s, though hydrogen bonding may add, as in water,
/// whose liquid x is about four times its gas's.
const AMMONIA_ANISOTROPY: f64 = molecular_anisotropy(0.000_91) + INTERACTION_ANISOTROPY;

/// Mulero, Cachadiña and Parra 2012's citation, the source of every surface tension but water's.
const MULERO: &str = "Mulero, Cachadiña and Parra 2012, \"Recommended Correlations for the \
                      Surface Tension of Common Fluids\" (J. Phys. Chem. Ref. Data 41, 043105), σ \
                      = Σ σᵢ (1 − T ÷ T_c)^nᵢ, its coefficients as CoolProp's fluid files hold \
                      them (MIT; the paper is closed, so its fitted ranges were not read), with \
                      the reference equation of state's T_c, as NIST's WebBook evaluates them; \
                      within 2 × 10⁻⁶ of the WebBook's saturated-liquid column to 0.98 T_c";

/// The reduced box about a critical point inside which a viscosity correlation that states none
/// is `Derived`: Vesovic et al.'s bound on where carbon dioxide's enhancement passes 1%, 0.986 <
/// `T_r` < 1.019 and 0.642 < `ρ_r` < 1.283, as Sotiriadou et al. 2026 (their Sec. 2 and ref. 36)
/// quote it for methane's correlation, which sets the enhancement to zero; in each fluid's own
/// reduced units.
#[must_use]
const fn reduced_critical_region(
    critical: Kelvin,
    density: KilogramsPerCubicMetre,
) -> CriticalRegion {
    CriticalRegion::new(
        (
            Kelvin::new(0.986 * critical.value()),
            Kelvin::new(1.019 * critical.value()),
        ),
        (
            KilogramsPerCubicMetre::new(0.642 * density.value()),
            KilogramsPerCubicMetre::new(1.283 * density.value()),
        ),
    )
}

/// IAPWS R12-08's correlation-length amplitudes for water (its Sec. 2.7 and Table 3: `ξ_0` = 0.13
/// nm, `Γ_0` = 0.06, ν = 0.630 and γ = 1.239, `T_R` = 1.5 `T_c`), which methane takes in want of
/// its own (decision-p14-t49h-liquids §7(c) names them; no methane crossover amplitudes were read).
const R12_08_XI_0_M: f64 = 0.13e-9;

/// [`R12_08_XI_0_M`]'s susceptibility amplitude `Γ_0`.
const R12_08_GAMMA_0: f64 = 0.06;

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
    associating: &[],
};

/// IAPWS-95's ideal-gas part, for [`IAPWS_95`]: IAPWS R6-95(2018), its §3's Eq. (5) and Table 1 as
/// printed, read in the release (from Cooper 1982's ideal-gas heat capacity, Int. J. Thermophys.
/// 3, 35, its reference 6): the ln τ coefficient n°₃ = 3.006 32 and the five Planck–Einstein terms
/// n°ᵢ ln(1 − e^(−γ°ᵢτ)), i = 4 to 8, in the residual's τ = 647.096 K ÷ T; n°₁ and n°₂, which fix
/// only the zeros of energy and entropy, are not held. A test reproduces Table 6's `φ°_ττ` at 500
/// K and 838.025 kg m⁻³, −1.932 491 85.
pub(in crate::substance) const IAPWS_95_IDEAL: HelmholtzIdeal = HelmholtzIdeal {
    log_tau: 3.006_32,
    power: &[],
    planck_einstein: &[
        PlanckEinsteinTerm::new(0.012_436, 1.287_289_67),
        PlanckEinsteinTerm::new(0.973_15, 3.537_342_22),
        PlanckEinsteinTerm::new(1.279_50, 7.740_737_08),
        PlanckEinsteinTerm::new(0.969_56, 9.244_377_96),
        PlanckEinsteinTerm::new(0.248_73, 27.507_510_5),
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

/// Water's anisotropy ratio x (decision-p14-t49h-gas-index §4.2), set so that its depolarisation
/// ratio at 90° is 0.039 at 298.15 K (decision-p14-t49h-liquids §7(g)): x = 45 `ρ_v` `S_0` ÷ (3 −
/// 4`ρ_v`), `ρ_v` = δ ÷ (2 − δ), with `S_0` = 0.062 09 from IAPWS-95 at the record's saturated
/// density there: this closed form at that state, which a test evaluates and holds this value to,
/// bit for bit. δ then follows the state: 0.0378 at the triple point, 0.0391 at 20 °C and 0.0302 at
/// 100 °C. The 0.039:
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
const WATER_ANISOTROPY: f64 = 0.019_026_970_219_429_53;

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
        anisotropy_ratio: WATER_ANISOTROPY,
        basis: LiquidBasis::Measured,
        fluctuations: CorrelationLength {
            amplitude_m: R12_08_XI_0_M,
            susceptibility_amplitude: R12_08_GAMMA_0,
            nu: 0.630,
            gamma: 1.239,
            reference_temperature: Kelvin::new(1.5 * 647.096),
            critical_pressure: Pascals::new(22.064e6),
            critical_density: KilogramsPerCubicMetre::new(322.0),
        },
    }),
    surface_tension: Some(IAPWS_SURFACE_TENSION),
    viscosity: Some(ViscosityCorrelation::Iapws2008(&IAPWS_VISCOSITY)),
    supercooled_viscosity: None,
    specific_heat: Some(LiquidSpecificHeat {
        equation: &IAPWS_95,
        ideal: &IAPWS_95_IDEAL,
        low: Kelvin::new(275.0),
        high: Kelvin::new(305.0),
    }),
    constitution: LiquidConstitution::Pure {
        components: &[WATER_ID],
        reference: &[(WATER_ID, 1.0)],
    },
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
                       doi:10.1021/j100187a058, not read), δ at the record's S_0 from water's \
                       anisotropy ratio x = 0.019 027, set to give δ = 0.039 at 298.15 K \
                       (decision-p14-t49h-gas-index §4.2; 0.039 is Jonasz and Fournier 2007's \
                       from Farinato and Rowell 1976's data, as Twardowski et al. 2007, \
                       Biogeosciences 4, 1041, p. 1055, report and ZHH adopt it; 0.051, Buiteveld \
                       et al. 1994, the alternative, +2.1% in b; decision-p14-t49h-liquids §7(g)), \
                       and κ_T from IAPWS-95 (IAPWS R6-95(2018)) at the record's density; \
                       `Derived` where q_max ξ passes 0.1, ξ by IAPWS R12-08's Eqs. (20)–(21a) and \
                       Table 3 (decision-p14-t49h-liquids §7(c))",
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
        Source {
            covers: "c_l",
            citation: "IAPWS-95 (IAPWS R6-95(2018); Wagner and Pruß 2002, J. Phys. Chem. Ref. \
                       Data 31, 387), evaluated here with Table 1's ideal-gas part and Table 2's \
                       residual: the saturated liquid's c_p, at the row's diagram saturation \
                       pressure on the equation's liquid branch, over 275–305 K and held at the \
                       ends (decision-r09-t13i §2.1; decision-r09-t13i-cl-licence §6); checked \
                       against the release's Table 6 and against NIST's Chemistry WebBook (SRD \
                       69), which evaluates the same equation, at asserted states",
            basis: Basis::Fact,
        },
    ],
};

/// Liquid methane: the index from Martonchik and Orton's 90 K liquid, carried; its absorption the
/// file's; its scattering by ES–Cabannes from Setzmann and Wagner 1991's `κ_T`; its surface tension
/// Mulero et al. 2012's; and its viscosity Sotiriadou et al. 2026's, continued below its triple
/// point by the fitted Vogel–Fulcher–Tammann form.
///
/// The file's 90 K lies 0.694 K below methane's triple point, where the row's ancillary density
/// is not evaluated: its density there is the line at the ancillary's triple-point slope
/// (decision-p14-t49b-condensed §2.5), so the index is `Derived`. Its absorption is `Derived`
/// (decision-p14-t49h-liquids §2; the file re-reduced by R08.T5.j, §2.4, and `derived`):
///
/// - over 555.6–780 nm it is Martonchik and Orton's Fig. 3 best estimate, read from the paper's
///   scan, not their Table 2, whose visible k are local maxima only (every one marked "+", none a
///   minimum). The figure's level over Patel, Nelson and Kerl's range, from 13,300 cm⁻¹, is put on
///   Table 2's (divided by 1.780), and its spike at 616–623 nm, ten times their own points, is
///   taken as an exponent slip (divided by 10), as main ruled on 2026-10-19;
/// - nothing was measured shortward of 18,100 cm⁻¹ (552.5 nm): "We could find no absorption
///   measurements of liquid methane in this spectral region" (p. 8309). Over 380–555.6 nm it is
///   Karkoschka 1998's gas coefficients times the liquid's number density, 630.8 amagat, the
///   method Martonchik and Orton used for their unmeasured far ultraviolet (p. 8309); the
///   coefficients are inferred from planetary spectra, their continuum uncertain by 0.0003 (km
///   amagat)⁻¹ at 400 nm, doubling every 100 nm, so 100% or more for the smallest;
/// - the 90 K values are their 111 K spectrum density-scaled (p. 8310), uncertain by 25% (p. 8309).
///
/// The table was refitted for it at 22 by decision-p14-t49h-liquids §5.
const METHANE: LiquidColumns = LiquidColumns {
    optics: Some(LiquidOptics {
        index: &bins::METHANE_LIQUID_N,
        absorption: Some((&bins::METHANE_LIQUID_A, LiquidBasis::Derived)),
        state: Kelvin::new(90.0),
        index_basis: LiquidBasis::Measured,
        index_checked: None,
        absorption_checked: None,
    }),
    scattering: Some(LiquidScattering {
        equation: &equations::METHANE,
        anisotropy_ratio: INTERACTION_ANISOTROPY,
        basis: LiquidBasis::Derived,
        fluctuations: CorrelationLength {
            amplitude_m: R12_08_XI_0_M,
            susceptibility_amplitude: R12_08_GAMMA_0,
            nu: 0.630,
            gamma: 1.239,
            reference_temperature: Kelvin::new(1.5 * 190.564),
            critical_pressure: Pascals::new(4.5992e6),
            critical_density: KilogramsPerCubicMetre::new(162.66),
        },
    }),
    surface_tension: Some(SurfaceTensionEquation {
        critical_temperature: Kelvin::new(190.564),
        terms: &[
            (0.038_25, 1.191),
            (-0.006_024, 5.422),
            (-0.000_706_5, 0.616_1),
        ],
        valid_from: Kelvin::new(90.694_1),
        valid_to: Kelvin::new(190.564),
        basis: LiquidBasis::Measured,
    }),
    viscosity: Some(ViscosityCorrelation::Sotiriadou2026(&SOTIRIADOU_2026)),
    supercooled_viscosity: Some(SupercooledViscosity {
        b_kelvin: bins::METHANE_SUPERCOOLED_VISCOSITY_B_K,
        divergence: Kelvin::new(bins::METHANE_SUPERCOOLED_VISCOSITY_T0_K),
        triple: Kelvin::new(90.694_1),
    }),
    specific_heat: Some(LiquidSpecificHeat {
        equation: &equations::METHANE,
        ideal: &equations::METHANE_IDEAL,
        low: Kelvin::new(91.0),
        high: Kelvin::new(116.0),
    }),
    constitution: LiquidConstitution::Pure {
        components: &[METHANE_ID],
        reference: &[(METHANE_ID, 1.0)],
    },
    sources: &[
        Source {
            covers: "n, a",
            citation: "R08's `methane-liquid.json` (`derived`; R08.T5.j): n Martonchik and Orton \
                       1994 (Appl. Opt. 33, 8306), the liquid at 90 K, through \
                       refractiveindex.info (CC0); a over 555.6–780 nm their Fig. 3 best estimate, \
                       read from the scan (NASA NTRS 19960016949), not Table 2's local maxima, its \
                       level over Patel, Nelson and Kerl's range (from 13,300 cm⁻¹) put on Table \
                       2's and its 616–623 nm spike taken as an exponent slip; \
                       nothing measured shortward of 552.5 nm, so a over 380–555.6 nm Karkoschka \
                       1998's gas coefficients (Icarus 133, 134; PDS GBAT_0001, CC0) at the \
                       liquid's number density; the 90 K values the 111 K values scaled by \
                       density, 25% (decision-p14-t49h-liquids §2.1 and §2.4); reduced to the bake \
                       bins by `tables::liquid_optics`",
            basis: Basis::Reduced,
        },
        Source {
            covers: "b",
            citation: "Einstein–Smoluchowski–Cabannes as for water (Zhang, Hu and He 2009's \
                       density term, the PMH model's ρ ∂n² ÷ ∂ρ), κ_T from Setzmann and Wagner \
                       1991 (`rows/equations.rs`) at the record's density, δ from the anisotropy \
                       ratio x = 0.006 at the record's S_0 (about 0.025 at the triple point), the \
                       interaction part calibrated on liquid CCl₄ (Kaye and McDaniel 1974), band x \
                       × (0.4–2) (decision-p14-t49h-gas-index §3–§4), so `Derived`; `Derived` too \
                       where q_max ξ passes 0.1, ξ by IAPWS R12-08's Eqs. (20)–(21a) with its own \
                       amplitudes (ξ_0 = 0.13 nm, Γ_0 = 0.06), in want of methane's",
            basis: Basis::Fact,
        },
        Source {
            covers: "σ",
            citation: MULERO,
            basis: Basis::Raw { licence: "MIT" },
        },
        Source {
            covers: "μ",
            citation: "Sotiriadou, Antoniadis, Assael, Martinek and Huber 2026 (Int. J. \
                       Thermophys. 47:18, open access), Eqs. 1–6 with Tables 3–5 and Δη_c = 0, at \
                       the record's density; triple point to 625 K, 3% (k = 2) in the liquid to 33 \
                       MPa",
            basis: Basis::Fact,
        },
        Source {
            covers: "μ below T_t",
            citation: "`tables::liquid_optics`' Vogel–Fulcher–Tammann B and T_0, fitted by \
                       `hyperion-fit` to this viscosity along the saturated liquid from the triple \
                       point to the normal boiling point and anchored at the triple point, for a \
                       solution's component below it (decision-p14-t49b-condensed §2.5), `Derived`",
            basis: Basis::Fitted,
        },
        Source {
            covers: "c_l",
            citation: "Setzmann and Wagner 1991 (J. Phys. Chem. Ref. Data 20, 1061), its equation \
                       of state evaluated here: the saturated liquid's c_p, at the row's diagram \
                       saturation pressure on the equation's liquid branch, over 91–116 K and \
                       held at the ends (decision-r09-t13i §2.1; decision-r09-t13i-cl-licence \
                       §6); the residual as `rows/equations.rs` holds it, the ideal part as \
                       CoolProp's Methane.json holds it (MIT, `NOTICE`); checked against its \
                       Table 39 and against NIST's Chemistry WebBook (SRD 69), which evaluates \
                       the same equation, at asserted states",
            basis: Basis::Fact,
        },
    ],
};

/// Sotiriadou et al. 2026's viscosity of methane: Table 3's aᵢ (a₃ = 3.29256 × 10⁻⁴, the reading
/// that reproduces their Table 8), Table 4's Rainwater–Friend parameters σ = 0.4115 nm and ε ÷
/// `k_B` = 112.2 K, M = 16.0428 g mol⁻¹ (Setzmann and Wagner's), and Table 5's fᵢ; valid from the
/// triple point to 625 K, and `Derived` in the reduced critical box their Sec. 2 quotes. A test
/// reproduces their Sec. 3.2's three points for computer verification and Table 8.
pub(in crate::substance) const SOTIRIADOU_2026: Sotiriadou2026Viscosity = Sotiriadou2026Viscosity {
    dilute_at_reference_micro_pa_s: 11.063_1,
    dilute: &[
        8.739_63e-1,
        -1.172_13e-1,
        3.292_56e-4,
        2.650_91e-2,
        -9.116_59e-3,
        -1.650_50e-3,
        1.788_860e-3,
        -3.388_60e-4,
    ],
    sigma_m: 0.411_5e-9,
    epsilon_k: 112.2,
    molar_mass_kg_per_mol: 16.042_8e-3,
    critical_temperature: Kelvin::new(190.564),
    critical_density: KilogramsPerCubicMetre::new(162.66),
    residual: [
        0.293_514,
        0.923,
        2.57,
        0.513,
        -1.48,
        1.653_650_300_856,
        4.218_535_015_2,
        -2.846_307_84,
        0.775_32,
    ],
    range: ViscosityRange {
        valid_from: Kelvin::new(90.694_1),
        valid_to: Kelvin::new(625.0),
        critical_region: Some(reduced_critical_region(
            Kelvin::new(190.564),
            KilogramsPerCubicMetre::new(162.66),
        )),
    },
};

/// Liquid ammonia: the index levelled to Wong and Anderson 1972's measurement, `Derived` (its slope
/// a Lorentz–Lorenz estimate from the gas, decision-r08-t5g §3). Its absorption is not modelled:
/// the file's k = 0 lies under Robertson and Williams 1973's total attenuation, which bounds the
/// liquid's k only to 6.7 × 10⁻⁷ at 400 nm, and the palette's ammonia absorption stays absent so
/// that no ammonia sea is drawn clear to any depth (decision-r08-t5g §5). Its scattering is
/// ES–Cabannes from Gao et al. 2023's `κ_T`, its surface tension Mulero et al.'s and its viscosity
/// Monogenidou, Assael and Huber 2018's.
const AMMONIA: LiquidColumns = LiquidColumns {
    optics: Some(LiquidOptics {
        index: &bins::AMMONIA_LIQUID_N,
        absorption: None,
        state: Kelvin::new(239.80),
        index_basis: LiquidBasis::Derived,
        index_checked: None,
        absorption_checked: None,
    }),
    scattering: Some(LiquidScattering {
        equation: &equations::AMMONIA,
        anisotropy_ratio: AMMONIA_ANISOTROPY,
        basis: LiquidBasis::Derived,
        fluctuations: CorrelationLength {
            amplitude_m: 0.14e-9,
            susceptibility_amplitude: 0.053,
            nu: 0.630,
            gamma: 1.239,
            reference_temperature: Kelvin::new(608.34),
            critical_pressure: Pascals::new(11.3634e6),
            critical_density: KilogramsPerCubicMetre::new(13_696.0 * 0.017_030_52),
        },
    }),
    surface_tension: Some(SurfaceTensionEquation {
        critical_temperature: Kelvin::new(405.56),
        terms: &[(0.102_8, 1.211), (-0.094_53, 5.585)],
        valid_from: Kelvin::new(195.49),
        valid_to: Kelvin::new(405.56),
        basis: LiquidBasis::Measured,
    }),
    viscosity: Some(ViscosityCorrelation::Monogenidou2018(&MONOGENIDOU_2018)),
    supercooled_viscosity: None,
    specific_heat: Some(LiquidSpecificHeat {
        equation: &equations::AMMONIA,
        ideal: &equations::AMMONIA_IDEAL,
        low: Kelvin::new(196.0),
        high: Kelvin::new(241.0),
    }),
    constitution: LiquidConstitution::Pure {
        components: &[AMMONIA_ID],
        reference: &[(AMMONIA_ID, 1.0)],
    },
    sources: &[
        Source {
            covers: "n",
            citation: "R08's `ammonia-liquid.json`: a Cauchy line levelled to Wong and Anderson \
                       1972's 1.360 at 632.8 nm and 239.80 K (J. Opt. Soc. Am. 62, 219), its slope \
                       Cuthbertson 1914's gas dispersion carried by Lorentz–Lorenz; the file's 240 \
                       K stands for 239.80 K, the state the index is carried from",
            basis: Basis::Reduced,
        },
        Source {
            covers: "b",
            citation: "Einstein–Smoluchowski–Cabannes as for water, κ_T from Gao et al. 2023 \
                       (`rows/equations.rs`), δ from x = 45ρ_v ÷ (3 − 4ρ_v) + 0.006 with the gas's \
                       ρ_v = 0.000 91 at 632.8 nm (Keir 1995, Table 6.3), about 0.07 at the triple \
                       point; hydrogen bonding may add, as in water; `Derived`; `Derived` too \
                       where q_max ξ passes 0.1, ξ_0 = 0.14 nm, Γ_0 = 0.053 and T_R = 608.34 K \
                       from Monogenidou, Assael and Huber 2018's thermal-conductivity correlation \
                       (J. Phys. Chem. Ref. Data 47, 043101, Sec. 2.3) as CoolProp's \
                       `Ammonia.json` quotes it, not read",
            basis: Basis::Raw { licence: "MIT" },
        },
        Source {
            covers: "σ",
            citation: MULERO,
            basis: Basis::Raw { licence: "MIT" },
        },
        Source {
            covers: "μ",
            citation: "Monogenidou, Assael and Huber 2018 (J. Phys. Chem. Ref. Data 47, 023102; \
                       NIST's author manuscript, PMC6512859), Eqs. 1–8 with Tables 2 and 3, at the \
                       record's density; triple point to 725 K at pressures to 50 MPa, 2% (k = 2) \
                       in the saturated liquid to 285 K, 3.3% to 335 K, 5% elsewhere",
            basis: Basis::Fact,
        },
        Source {
            covers: "c_l",
            citation: "Gao, Wu, Bell, Harvey and Lemmon 2023 (J. Phys. Chem. Ref. Data 52, \
                       013102), its equation of state evaluated here: the saturated liquid's c_p, \
                       at the row's diagram saturation pressure on the equation's liquid branch, \
                       over 196–241 K and held at the ends (decision-r09-t13i §2.1; \
                       decision-r09-t13i-cl-licence §6); the residual as `rows/equations.rs` \
                       holds it, the ideal part as CoolProp's Ammonia.json holds it (MIT, \
                       `NOTICE`); checked against its Table 7 and against NIST's Chemistry \
                       WebBook (SRD 69), which evaluates the same equation, at asserted states; \
                       the WebBook, which labels it Gao, Wu, Bell and Lemmon 2018, evaluates it \
                       with CODATA 2014's R",
            basis: Basis::Fact,
        },
    ],
};

/// Monogenidou, Assael and Huber 2018's viscosity of ammonia: Table 2's M = 17.030 52 g mol⁻¹, ε ÷
/// `k_B` = 386.0 K, σ = 0.2957 nm and αᵢ, Table 3's cᵢ, and Gao et al.'s critical point, 405.56 K
/// and 233.250 kg m⁻³, as their Sec. 2 adopts it; valid from the triple point to 725 K, and
/// `Derived` in the reduced critical box. A test reproduces their three points for computer
/// verification and their Table 6's saturated liquid.
pub(in crate::substance) const MONOGENIDOU_2018: Monogenidou2018Viscosity =
    Monogenidou2018Viscosity {
        molar_mass_g_per_mol: 17.030_52,
        sigma_nm: 0.295_7,
        epsilon_k: 386.0,
        cross_section: [0.391_75, -0.599_18, -0.000_22, 0.198_71, -0.069_42],
        critical_temperature: Kelvin::new(405.56),
        critical_density: KilogramsPerCubicMetre::new(233.25),
        residual: [
            0.039_330_8,
            16.724_735_0,
            1.197_593_4,
            0.001_699_5,
            -4.239_979_4,
        ],
        range: ViscosityRange {
            valid_from: Kelvin::new(195.49),
            valid_to: Kelvin::new(725.0),
            critical_region: Some(reduced_critical_region(
                Kelvin::new(405.56),
                KilogramsPerCubicMetre::new(233.25),
            )),
        },
    };

/// Liquid nitrogen: the index a Cauchy fit to Augustin 1915's measured liquid at 77.32 K,
/// extrapolated beyond 435.9–656.3 nm, so `Derived`. Its absorption is not modelled: the file holds
/// k = 0 because no measurement of the liquid's visible absorption was found, and Liveing and
/// Dewar's "colourless" bounds it only over a laboratory path, not a sea's depth
/// (decision-p14-t49h-liquids §3). Its scattering is ES–Cabannes from Span et al. 2000's `κ_T`,
/// its surface tension Mulero et al.'s and its viscosity Huber, Perkins and Lemmon 2024's.
const NITROGEN: LiquidColumns = LiquidColumns {
    optics: Some(LiquidOptics {
        index: &bins::NITROGEN_LIQUID_N,
        absorption: None,
        state: Kelvin::new(77.32),
        index_basis: LiquidBasis::Derived,
        index_checked: None,
        absorption_checked: None,
    }),
    scattering: Some(LiquidScattering {
        equation: &equations::NITROGEN,
        anisotropy_ratio: NITROGEN_ANISOTROPY,
        basis: LiquidBasis::Derived,
        fluctuations: CorrelationLength {
            amplitude_m: 0.16e-9,
            susceptibility_amplitude: 0.075,
            nu: 0.630,
            gamma: 1.239,
            reference_temperature: Kelvin::new(1.5 * 126.192),
            critical_pressure: Pascals::new(3.3958e6),
            critical_density: KilogramsPerCubicMetre::new(11_183.9 * 0.028_013_48),
        },
    }),
    surface_tension: Some(SurfaceTensionEquation {
        critical_temperature: Kelvin::new(126.192),
        terms: &[(0.028_98, 1.246)],
        valid_from: Kelvin::new(63.151),
        valid_to: Kelvin::new(126.192),
        basis: LiquidBasis::Measured,
    }),
    viscosity: Some(ViscosityCorrelation::Huber2024(&HUBER_2024)),
    supercooled_viscosity: None,
    specific_heat: Some(LiquidSpecificHeat {
        equation: &equations::NITROGEN,
        ideal: &equations::NITROGEN_IDEAL,
        low: Kelvin::new(64.0),
        high: Kelvin::new(78.0),
    }),
    constitution: LiquidConstitution::Pure {
        components: &[NITROGEN_ID],
        reference: &[(NITROGEN_ID, 1.0)],
    },
    sources: &[
        Source {
            covers: "n",
            citation: "R08's `nitrogen-liquid.json`: a two-term Cauchy fit to Augustin 1915's \
                       indices of argon-free liquid N2 at 77.32 K (Ann. Phys. 351, 419; Zenodo \
                       1514490, CC0), the state the index is carried from",
            basis: Basis::Reduced,
        },
        Source {
            covers: "b",
            citation: "Einstein–Smoluchowski–Cabannes as for water, κ_T from Span et al. 2000 \
                       (`rows/equations.rs`), δ from x = 45ρ_v ÷ (3 − 4ρ_v) + 0.006 with the gas's \
                       ρ_v = 0.0104 at 550 nm (Bates 1984's King factor), about 0.38 at the triple \
                       point and 0.25 at 77 K; `Derived`; `Derived` too where q_max ξ passes 0.1, \
                       ξ by Huber, Perkins and Lemmon 2024's Eqs. 20–22 (ξ_0 = 0.16 nm, Γ_0 = \
                       0.075, T_R = 1.5 T_c), which reproduce their Table 8's ξ",
            basis: Basis::Fact,
        },
        Source {
            covers: "σ",
            citation: MULERO,
            basis: Basis::Raw { licence: "MIT" },
        },
        Source {
            covers: "μ",
            citation: "Huber, Perkins and Lemmon 2024 (Int. J. Thermophys. 45:146, open access), \
                       Eqs. 1, 3 and 5 with Tables 3 and 4 and Δη_c = 1, at the record's density; \
                       triple point to 1000 K, 4% (k = 2) in the liquid to 34 MPa",
            basis: Basis::Fact,
        },
        Source {
            covers: "c_l",
            citation: "Span, Lemmon, Jacobsen, Wagner and Yokozeki 2000 (J. Phys. Chem. Ref. Data \
                       29, 1361), its equation of state evaluated here: the saturated liquid's \
                       c_p, at the row's diagram saturation pressure on the equation's liquid \
                       branch, over 64–78 K and held at the ends (decision-r09-t13i §2.1; \
                       decision-r09-t13i-cl-licence §6); the residual as `rows/equations.rs` \
                       holds it, the ideal part as CoolProp's Nitrogen.json holds it (MIT, \
                       `NOTICE`); checked against its saturation table (p. 1403) and against \
                       NIST's Chemistry WebBook (SRD 69), which evaluates the same equation, at \
                       asserted states",
            basis: Basis::Fact,
        },
    ],
};

/// Huber, Perkins and Lemmon 2024's viscosity of nitrogen: Table 3's aᵢ at η₀(298.15 K) = 17.7494
/// μPa s and Table 4's Nᵢ, tᵢ and dᵢ, with Span et al.'s critical point, 126.192 K and 11.1839 mol
/// dm⁻³ at 28.013 48 g mol⁻¹; valid from the triple point to 1000 K, and `Derived` within 3% of
/// the critical temperature and 25% of the critical density, where their Sec. 2.3 says the
/// enhancement may no longer be ignored. A test reproduces their Table 7.
pub(in crate::substance) const HUBER_2024: Huber2024Viscosity = Huber2024Viscosity {
    dilute_at_reference_micro_pa_s: 17.749_4,
    dilute: &[
        7.734_578e-1,
        -9.310_761e-2,
        2.716_958e-2,
        6.175_553e-3,
        -7.201_594e-3,
        2.094_372e-3,
        1.922_676e-4,
        -3.454_323e-4,
        1.051_771e-4,
        -1.126_739e-5,
    ],
    critical_temperature: Kelvin::new(126.192),
    critical_density: KilogramsPerCubicMetre::new(11_183.9 * 0.028_013_48),
    residual: &[
        (9.955_235_691_668, -0.775_122_186_31, 1.0),
        (-6.165_266_404_871, -2.001_096_088_05, 1.0),
        (0.213_120_936_996, -5.814_455, 2.0),
        (-8.473_713_006_806, -0.676_025_969_96, 2.0),
        (10.013_103_356_639, 0.0, 2.0),
        (0.638_966_874_603, -0.716_131_854_56, 6.964_233_632_49),
        (0.311_620_258_213, -1.140_693_995_97, 8.386_512_575_01),
        (9.241_856_768_911, -2.365_258_359_8, 2.992_248_574_71),
        (-5.252_828_814_854, -2.546_369_925_5, 3.427_446_179_75),
        (-0.667_072_279_228, -1.007_940_345_15, 7.973_717_674_11),
    ],
    range: ViscosityRange {
        valid_from: Kelvin::new(63.151),
        valid_to: Kelvin::new(1_000.0),
        critical_region: Some(CriticalRegion::new(
            (Kelvin::new(0.97 * 126.192), Kelvin::new(1.03 * 126.192)),
            (
                KilogramsPerCubicMetre::new(0.75 * 11_183.9 * 0.028_013_48),
                KilogramsPerCubicMetre::new(1.25 * 11_183.9 * 0.028_013_48),
            ),
        )),
    },
};

/// Liquid carbon dioxide, for any world above its triple point's 518 kPa: its index R08.T5.k's
/// `carbon-dioxide-liquid.json`, the gas's dispersion scaled by Phillips 1920's measured density
/// law of the Lorentz–Lorenz function, at 216.6 K (decision-p14-t49h-gas-index §2.3), `Derived`,
/// its provenance, and carried by Lorentz–Lorenz at constant specific refraction from there (a
/// carry that misses Phillips's own law by about 0.0025 at warm states, recorded there, §2.2); its
/// absorption not modelled (the file's k = 0, the liquid colourless in the visible); its
/// scattering by ES–Cabannes from Span and Wagner 1996's `κ_T`, its surface tension Mulero et al.'s
/// and its viscosity Laesecke and Muzny 2017's (P14.T49.h's second step).
const CARBON_DIOXIDE: LiquidColumns = LiquidColumns {
    optics: Some(LiquidOptics {
        index: &bins::CARBON_DIOXIDE_LIQUID_N,
        absorption: None,
        state: Kelvin::new(216.6),
        index_basis: LiquidBasis::Derived,
        index_checked: None,
        absorption_checked: None,
    }),
    scattering: Some(LiquidScattering {
        equation: &equations::CARBON_DIOXIDE,
        anisotropy_ratio: CARBON_DIOXIDE_ANISOTROPY,
        basis: LiquidBasis::Derived,
        fluctuations: CorrelationLength {
            amplitude_m: 0.15e-9,
            susceptibility_amplitude: 0.048_1,
            nu: 0.630,
            gamma: 1.239,
            reference_temperature: Kelvin::new(1.5 * 304.128_2),
            critical_pressure: Pascals::new(7.3773e6),
            critical_density: KilogramsPerCubicMetre::new(467.6),
        },
    }),
    surface_tension: Some(SurfaceTensionEquation {
        critical_temperature: Kelvin::new(304.128_2),
        terms: &[(0.078_63, 1.254)],
        valid_from: Kelvin::new(216.592),
        valid_to: Kelvin::new(304.128_2),
        basis: LiquidBasis::Measured,
    }),
    viscosity: Some(ViscosityCorrelation::LaeseckeMuzny2017(
        &LAESECKE_MUZNY_2017,
    )),
    supercooled_viscosity: None,
    specific_heat: Some(LiquidSpecificHeat {
        equation: &equations::CARBON_DIOXIDE,
        ideal: &equations::CARBON_DIOXIDE_IDEAL,
        low: Kelvin::new(217.0),
        high: Kelvin::new(272.0),
    }),
    constitution: LiquidConstitution::Pure {
        components: &[CARBON_DIOXIDE_ID],
        reference: &[(CARBON_DIOXIDE_ID, 1.0)],
    },
    sources: &[
        Source {
            covers: "n",
            citation: "R08's `carbon-dioxide-liquid.json` (R08.T5.k; decision-p14-t49h-gas-index \
                       §2.3): rayleigh.ts's gas dispersion (Bideau-Mehu et al. 1973 as Sneep and \
                       Ubachs 2005 correct it) at the NIST WebBook's 26.776 63 mol/l at 216.6 K \
                       (the triple point rounded up to 0.01 K, the state the index is carried \
                       from), its molar refraction scaled by Phillips 1920's measured density law \
                       (Proc. R. Soc. A 97, 225), c = 1 ÷ (1 + 0.0172 ρ²), 0.976 672 there, \
                       extrapolated beyond his 0.737 g cm⁻³; derived, band ±0.007 in n",
            basis: Basis::Reduced,
        },
        Source {
            covers: "b",
            citation: "Einstein–Smoluchowski–Cabannes as for water, κ_T from Span and Wagner 1996 \
                       (`rows/equations.rs`), δ from x = 45ρ_v ÷ (3 − 4ρ_v) + 0.006 with the gas's \
                       ρ_v = 0.041 at 550 nm (Sneep and Ubachs 2005, Eq. 14), about 0.46 at the \
                       triple point; `Derived`; `Derived` too where q_max ξ passes 0.1, ξ_0 = 0.15 \
                       nm and Γ_0 = 0.0481 from Laesecke and Muzny 2017's Table 5, which gives no \
                       T_R, so R12-08's convention T_R = 1.5 T_c (CoolProp's CO₂ conductivity \
                       block holds the same 456.19 K, after Huber et al. 2016, not read)",
            basis: Basis::Fact,
        },
        Source {
            covers: "σ",
            citation: MULERO,
            basis: Basis::Raw { licence: "MIT" },
        },
        Source {
            covers: "μ",
            citation: "Laesecke and Muzny 2017 (J. Phys. Chem. Ref. Data 46, 013107; NIST's author \
                       manuscript, OSTI 1463125), Eqs. 1, 4–6, 8 and 9 with Tables 2–4, \
                       without Δη_c, at the record's density; the liquid from the triple point to \
                       the critical point, 4% (k = 2, 'rather conservative')",
            basis: Basis::Fact,
        },
        Source {
            covers: "c_l",
            citation: "Span and Wagner 1996 (J. Phys. Chem. Ref. Data 25, 1509), its equation of \
                       state evaluated here: the saturated liquid's c_p, at the row's diagram \
                       saturation pressure on the equation's liquid branch, over 217–272 K and \
                       held at the ends (decision-r09-t13i §2.1; decision-r09-t13i-cl-licence \
                       §6); the residual as `rows/equations.rs` holds it, the ideal part as \
                       CoolProp's CarbonDioxide.json holds it (MIT, `NOTICE`); checked against \
                       its Table 34 and against NIST's Chemistry WebBook (SRD 69), which \
                       evaluates the same equation, at asserted states",
            basis: Basis::Fact,
        },
    ],
};

/// Laesecke and Muzny 2017's viscosity of carbon dioxide: Table 2's aᵢ (their Eq. 4), Table 3's
/// bᵢ (Eq. 6, Rainwater–Friend's) with σ = 0.378 421 nm and ε ÷ `k_B` = 200.760 K at M = 44.0095 g
/// mol⁻¹ (their Sec. 5.2), and Table 4's γ, c₁ and c₂ (Eq. 8) with the triple point's 216.592 K and
/// 1178.53 kg m⁻³ and R = 8.314 459 8 J mol⁻¹ K⁻¹ (Sec. 5.3);
/// the liquid valid from the triple point to the critical point, and `Derived` where Vesovic et
/// al.'s bound, as their Sec. 5.6 quotes it, puts the enhancement above 1%: 300–310 K and 300–600
/// kg m⁻³. A test reproduces their Table 6.
pub(in crate::substance) const LAESECKE_MUZNY_2017: LaeseckeMuzny2017Viscosity =
    LaeseckeMuzny2017Viscosity {
        dilute: [
            1_749.354_893_188_35,
            -369.069_300_007_128,
            5_423_856.348_876_91,
            -2.212_838_521_683_56,
            -269_503.247_933_569,
            73_145.021_531_826,
            5.343_686_495_092_78,
        ],
        sigma_m: 0.378_421e-9,
        epsilon_k: 200.760,
        molar_mass_kg_per_mol: 44.009_5e-3,
        gas_constant_j_per_mol_k: 8.314_459_8,
        triple_temperature: Kelvin::new(216.592),
        triple_liquid_density: KilogramsPerCubicMetre::new(1_178.53),
        residual: (
            8.062_827_374_812_77,
            0.360_603_235_428_487,
            0.121_550_806_591_497,
        ),
        range: ViscosityRange {
            valid_from: Kelvin::new(216.592),
            valid_to: Kelvin::new(304.128_2),
            critical_region: Some(CriticalRegion::new(
                (Kelvin::new(300.0), Kelvin::new(310.0)),
                (
                    KilogramsPerCubicMetre::new(300.0),
                    KilogramsPerCubicMetre::new(600.0),
                ),
            )),
        },
    };

/// A gas's depolarisation ratio for vertically polarised light from its King factor: `ρ_v` = 3
/// (`F_K` − 1) ÷ (6 + 4 `F_K`), the inverse of `F_K` = (3 + 6 `ρ_v`) ÷ (3 − 4 `ρ_v`), so that
/// [`molecular_anisotropy`] of it is 4.5 (`F_K` − 1).
#[must_use]
const fn vertical_depolarisation(king_factor: f64) -> f64 {
    3.0 * (king_factor - 1.0) / (6.0 + 4.0 * king_factor)
}

/// Oxygen's anisotropy ratio, about 0.466 (decision-p14-t49h-gas-index §4.3): its molecular part
/// from the gas's King factor at 550 nm, Bates 1984's `F_K` = 1.096 + 1.385 × 10⁻³ λ⁻² + 1.448 ×
/// 10⁻⁴ λ⁻⁴ (λ in µm; Bodhaine et al. 1999, Eq. 6, as `rayleigh.ts`'s King-factor row cites it),
/// 1.102 16, so `ρ_v` = 0.029 45, plus [`INTERACTION_ANISOTROPY`], giving δ about 0.706 at the
/// triple point and 0.499 at 90.19 K, where `S_0` is 0.0155 and 0.0521 (the lane's research,
/// `.git/rm45-scratch/p14-liquids/step2b/step2b.json`). An upper estimate within −40% / +0%, as
/// [`NITROGEN_ANISOTROPY`]'s; here the anisotropic part dominates b.
const OXYGEN_ANISOTROPY: f64 = molecular_anisotropy(vertical_depolarisation(
    1.096 + 1.385e-3 / (0.55 * 0.55) + 1.448e-4 / (0.55 * 0.55 * 0.55 * 0.55),
)) + INTERACTION_ANISOTROPY;

/// Nitrous oxide's anisotropy ratio, about 1.013: its molecular part from the gas's `ρ_v` at 550
/// nm, 0.0577 + 11.8 × 10⁻¹² ν² (ν in cm⁻¹; Sneep and Ubachs 2005, Eq. 19, after Alms, Burnham and
/// Flygare 1975, as `rayleigh.ts` cites it), 0.061 60, plus [`INTERACTION_ANISOTROPY`], giving δ
/// about 0.665 at the triple point (`S_0` 0.0454) and 0.652 at 184.67 K (the lane's research). An
/// upper estimate within −40% / +0%, as [`OXYGEN_ANISOTROPY`]'s.
const NITROUS_OXIDE_ANISOTROPY: f64 =
    molecular_anisotropy(0.057_7 + 11.8e-12 * (1.0e7 / 550.0) * (1.0e7 / 550.0))
        + INTERACTION_ANISOTROPY;

/// Carbon monoxide's anisotropy ratio, 0.0825: its molecular part from the gas's King factor, 1.017
/// over the visible (`rayleigh.ts`'s row, between Bogaard, Buckingham, Pierens and White 1978's
/// 1.0161–1.0175 and Couling and Graham 1994's 1.0176), plus [`INTERACTION_ANISOTROPY`], giving δ
/// about 0.230 at the triple point (`S_0` 0.0351; the lane's research); Couling and Graham's
/// measured `ρ_v` = 0.005 251 at 514.5 nm would give 0.0853, and δ 0.235. An upper estimate within
/// −40% / +0%, as [`NITROGEN_ANISOTROPY`]'s.
const CARBON_MONOXIDE_ANISOTROPY: f64 =
    molecular_anisotropy(vertical_depolarisation(1.017)) + INTERACTION_ANISOTROPY;

/// Hydrogen sulphide's anisotropy ratio, about 0.0067: its molecular part from the gas's King
/// factor, 1 + (2 ÷ 9)(Δα ÷ ᾱ)² with Δα = 0.6735 au, the vibrational Raman lines excluded, over ᾱ
/// = 25.5 au at 632.8 nm (Bogaard, Buckingham and Ritchie 1982 and Monan, Bribes and Gaufrès 1982,
/// as `rayleigh.ts`'s row cites them), 1.000 155, plus [`INTERACTION_ANISOTROPY`], which dominates
/// it, giving δ about 0.025 at the triple point (`S_0` 0.0344; the lane's research). An upper
/// estimate within −40% / +0%, as [`NITROGEN_ANISOTROPY`]'s.
const HYDROGEN_SULPHIDE_ANISOTROPY: f64 = molecular_anisotropy(vertical_depolarisation(
    1.0 + 2.0 / 9.0 * (0.673_5 / 25.5) * (0.673_5 / 25.5),
)) + INTERACTION_ANISOTROPY;

/// Methane's Lorentz–Lorenz function (n² − 1) ÷ (n² + 2) at its triple point at 550 nm, 0.182 858:
/// Martonchik and Orton's 90 K file's 1.293 45 there, carried to 90.694 K by the row's density
/// (the lane's research), the calibration [`atomic_anisotropy`] scales from.
const METHANE_TRIPLE_LORENTZ_LORENZ: f64 = 0.182_858_302;

/// An atomic liquid's anisotropy ratio, its interaction part alone, since an atom has none of its
/// own (decision-p14-t49h-gas-index §4.3): methane's calibrated [`INTERACTION_ANISOTROPY`] scaled
/// by the square of the ratio of the liquid's Lorentz–Lorenz function `lorentz_lorenz` at its
/// triple point at 550 nm to methane's, as the dipole-induced-dipole pair estimate, (27 ÷ 2π) L² ÷
/// ρ*, scales at corresponding reduced densities ρ*, with one cancellation factor for all (the
/// lane's research's proposal). A stand-in, with methane's band, × (0.4–2): at the triple points it
/// gives `ρ_u` inside the ruling's estimates, 0.009–0.012 for argon and 0.021–0.027 for xenon (at
/// its top).
#[must_use]
const fn atomic_anisotropy(lorentz_lorenz: f64) -> f64 {
    let ratio = lorentz_lorenz / METHANE_TRIPLE_LORENTZ_LORENZ;
    INTERACTION_ANISOTROPY * ratio * ratio
}

/// Argon's anisotropy ratio, about 0.003 93, [`atomic_anisotropy`] at its Lorentz–Lorenz function
/// at the triple point at 550 nm, 0.148 021 (its file's 83.81 K index there carried to 83.806 K):
/// δ about 0.0109 at the triple point (`S_0` 0.0476; the lane's research).
const ARGON_ANISOTROPY: f64 = atomic_anisotropy(0.148_020_740);

/// Krypton's anisotropy ratio, about 0.006 39, [`atomic_anisotropy`] at 0.188 766 (its file's index
/// at 550 nm at the triple point, 1.3031): δ about 0.0171 at the triple point (`S_0` 0.0489).
const KRYPTON_ANISOTROPY: f64 = atomic_anisotropy(0.188_765_946);

/// Xenon's anisotropy ratio, about 0.010 16, [`atomic_anisotropy`] at 0.237 901 (its file's index
/// at 550 nm at the triple point, 1.391 58): δ about 0.0270 at the triple point (`S_0` 0.0486).
const XENON_ANISOTROPY: f64 = atomic_anisotropy(0.237_900_701);

/// The temperatures over which argon's carried index was checked against Sinnock and Smith 1969's
/// tables of the liquid (Phys. Rev. 181, 1297, as refractiveindex.info's CC0 copies give them) and
/// keeps its basis, 83.81–90 K: from the file's state to the warmest of their tables, every row
/// inside the file's grid of every table met within 5 × 10⁻⁴ by the file's own index at its
/// wavelength carried by the row's density, 1.5 × 10⁻⁴ at 83.81 K (the file's fit to that table),
/// 3.7 × 10⁻⁴ at 86 K, 3.4 × 10⁻⁴ at 88 K and 4.2 × 10⁻⁴ at 90 K (decision-p14-t49h-gas-index
/// §1.2; `hyperion-fit`'s `liquid_optics_the_atoms_index_windows_are_sinnock_and_smith_s`).
const ARGON_INDEX_CHECKED: (Kelvin, Kelvin) = (Kelvin::new(83.81), Kelvin::new(90.0));

/// Krypton's window, as [`ARGON_INDEX_CHECKED`]'s, 115.77–115.95 K: from the file's state, the
/// row's triple point, to Sinnock and Smith's melting-point table, met within 4.7 × 10⁻⁵. Their
/// 118, 122 and 126 K tables miss by 1.25 × 10⁻³, 1.72 × 10⁻³ and 1.70 × 10⁻³, past 5 × 10⁻⁴, for
/// two causes: the row's ancillary density (CoolProp's fit, −0.22% to +0.06% against Lemmon and
/// Span's equation), and their krypton itself, which falls faster than constant specific
/// refraction (1.5 × 10⁻³ at 126 K even at the equation's densities; Sinnock 1980 finds krypton's
/// two methods disagree). So the ruled 115.95–126 K is narrowed to where the check holds, as
/// water's window is.
const KRYPTON_INDEX_CHECKED: (Kelvin, Kelvin) = (Kelvin::new(115.77), Kelvin::new(115.95));

/// Xenon's window, as [`ARGON_INDEX_CHECKED`]'s: the file's state alone, 161.4 K, the row's triple
/// point. Sinnock and Smith's warmer tables, 166–178 K, miss by 6.3 × 10⁻⁴ to 8.1 × 10⁻⁴ (2.1 ×
/// 10⁻⁴ to 5.2 × 10⁻⁴ at the equation's densities: the row's ancillary density, CoolProp's fit,
/// lies −0.09% to +0.07% from Lemmon and Span's equation), and their melting-point table, at
/// 161.35 K, lies below the row's triple point, so the ruled 161.35–178 K is narrowed to the state,
/// away from which the index is `Derived`.
const XENON_INDEX_CHECKED: (Kelvin, Kelvin) = (Kelvin::new(161.4), Kelvin::new(161.4));

/// The viscosity the eight rows beside carbon dioxide that R08.T5.k gives a liquid file lack.
const NO_RULED_VISCOSITY: Source = Source {
    covers: "μ",
    citation: "absent: no viscosity correlation is ruled for the row (decision-p14-t49h-gas-index \
               §1.2: its other members only where a ruled source exists); CoolProp's files hold \
               Lemmon and Jacobsen 2004 for oxygen and argon, Polychroniadou et al. 2022 for \
               krypton, Velliadou et al. 2021 for xenon and Quiñones-Cisneros et al. 2012 for \
               hydrogen sulphide, and none for carbon monoxide, nitrous oxide or sulphur dioxide",
    basis: Basis::Adopted,
};

/// Liquid oxygen (decision-p14-t49h-gas-index §1.2): its index R08.T5.k's `oxygen-liquid.json`, the
/// gas's dispersion scaled to Johns and Wilhelm 1937's 1.2242 at 546.1 nm and 90.19 K, `Derived`
/// (its provenance; band ±0.0015). Its absorption is the file's, `Derived` (§2.4): Thalman and
/// Volkamer 2013's O₂–O₂ collision-induced bands near 477, 577 and 630 nm times the liquid's number
/// density squared, which make the liquid blue, unmeasured in the liquid; 0 in the bins from 658.7
/// nm, where the source's set ends (654 nm) and neither the B nor the A band is drawn. Its
/// scattering is ES–Cabannes from Schmidt and Wagner 1985's `κ_T`, its surface tension Mulero et
/// al.'s; no viscosity is ruled.
const OXYGEN: LiquidColumns = LiquidColumns {
    optics: Some(LiquidOptics {
        index: &bins::OXYGEN_LIQUID_N,
        absorption: Some((&bins::OXYGEN_LIQUID_A, LiquidBasis::Derived)),
        state: Kelvin::new(90.19),
        index_basis: LiquidBasis::Derived,
        index_checked: None,
        absorption_checked: None,
    }),
    scattering: Some(LiquidScattering {
        equation: &equations::OXYGEN,
        anisotropy_ratio: OXYGEN_ANISOTROPY,
        basis: LiquidBasis::Derived,
        fluctuations: CorrelationLength {
            amplitude_m: 0.24e-9,
            susceptibility_amplitude: 0.055,
            nu: 0.63,
            gamma: 1.241_5,
            reference_temperature: Kelvin::new(309.162),
            critical_pressure: Pascals::new(5.043e6),
            critical_density: KilogramsPerCubicMetre::new(13_630.0 * 0.031_998_8),
        },
    }),
    surface_tension: Some(SurfaceTensionEquation {
        critical_temperature: Kelvin::new(154.581),
        terms: &[(0.038_43, 1.225)],
        valid_from: Kelvin::new(54.361),
        valid_to: Kelvin::new(154.581),
        basis: LiquidBasis::Measured,
    }),
    viscosity: None,
    supercooled_viscosity: None,
    specific_heat: None,
    constitution: LiquidConstitution::Pure {
        components: &[OXYGEN_ID],
        reference: &[(OXYGEN_ID, 1.0)],
    },
    sources: &[
        Source {
            covers: "n, a",
            citation: "R08's `oxygen-liquid.json` (R08.T5.k; decision-p14-t49h-gas-index §2.3 and \
                       §2.4): n rayleigh.ts's gas dispersion (Křen 2011's refit of Zhang, Lu and \
                       Wang 2008) at the NIST WebBook's 35.662 63 mol/l at 90.19 K, the normal \
                       boiling point and the state the index is carried from, its molar \
                       refraction scaled by c = 0.986 155 to meet Johns and Wilhelm 1937's 1.2242 \
                       at 546.1 nm (Can. J. Res. 15a, 101, their abstract), derived, band \
                       ±0.0015; k Thalman and Volkamer 2013's O₂–O₂ collision-induced \
                       cross-sections at 203 K (Phys. Chem. Chem. Phys. 15, 15371, HITRAN2024's \
                       set) times the liquid's number density squared, derived and unmeasured in \
                       the liquid, 0 from 655 nm, where the set ends and the B and A bands are not \
                       drawn; reduced to the bake bins by `tables::liquid_optics`",
            basis: Basis::Reduced,
        },
        Source {
            covers: "b",
            citation: "Einstein–Smoluchowski–Cabannes as for water, κ_T from Schmidt and Wagner \
                       1985 (`rows/equations.rs`), δ from x = 45ρ_v ÷ (3 − 4ρ_v) + 0.006 with the \
                       gas's ρ_v = 0.029 45 from Bates 1984's King factor at 550 nm, about 0.71 at \
                       the triple point and 0.50 at 90.19 K; `Derived`; `Derived` too where q_max \
                       ξ passes 0.1, ξ_0 = 0.24 nm, Γ_0 = 0.055, γ = 1.2415 and T_R = 309.162 K \
                       from Lemmon and Jacobsen 2004's oxygen correlations (Int. J. Thermophys. \
                       25, 21) as CoolProp's `Oxygen.json` quotes them, not read, with ν = 0.63; \
                       p_c = 5.043 MPa, the equation's critical point as CoolProp's `Oxygen.json` \
                       holds it",
            basis: Basis::Raw { licence: "MIT" },
        },
        Source {
            covers: "σ",
            citation: MULERO,
            basis: Basis::Raw { licence: "MIT" },
        },
        NO_RULED_VISCOSITY,
    ],
};

/// Liquid argon (decision-p14-t49h-gas-index §1.2): its index R08.T5.k's `argon-liquid.json`,
/// Sinnock and Smith 1969's table of the liquid at 83.81 K, `Measured` over its checked window,
/// 83.81–90 K ([`ARGON_INDEX_CHECKED`]), and `Derived` beyond it; its absorption not modelled (the
/// file's k = 0, the liquid colourless); its scattering ES–Cabannes from Tegeler, Span and Wagner
/// 1999's `κ_T` with its interaction anisotropy alone, a stand-in; its surface tension Mulero et
/// al.'s; no viscosity is ruled.
const ARGON: LiquidColumns = LiquidColumns {
    optics: Some(LiquidOptics {
        index: &bins::ARGON_LIQUID_N,
        absorption: None,
        state: Kelvin::new(83.81),
        index_basis: LiquidBasis::Measured,
        index_checked: Some(ARGON_INDEX_CHECKED),
        absorption_checked: None,
    }),
    scattering: Some(LiquidScattering {
        equation: &equations::ARGON,
        anisotropy_ratio: ARGON_ANISOTROPY,
        basis: LiquidBasis::Derived,
        fluctuations: CorrelationLength {
            amplitude_m: 0.13e-9,
            susceptibility_amplitude: 0.055,
            nu: 0.63,
            gamma: 1.241_5,
            reference_temperature: Kelvin::new(301.374),
            critical_pressure: Pascals::new(4.863e6),
            critical_density: KilogramsPerCubicMetre::new(535.6),
        },
    }),
    surface_tension: Some(SurfaceTensionEquation {
        critical_temperature: Kelvin::new(150.687),
        terms: &[(0.037, 1.25)],
        valid_from: Kelvin::new(83.805_8),
        valid_to: Kelvin::new(150.687),
        basis: LiquidBasis::Measured,
    }),
    viscosity: None,
    supercooled_viscosity: None,
    specific_heat: None,
    constitution: LiquidConstitution::Pure {
        components: &[ARGON_ID],
        reference: &[(ARGON_ID, 1.0)],
    },
    sources: &[
        Source {
            covers: "n",
            citation: "R08's `argon-liquid.json` (R08.T5.k; decision-p14-t49h-gas-index §2.3): \
                       Sinnock and Smith 1969's table of the liquid at its melting point, 83.81 K \
                       (Phys. Rev. 181, 1297, through refractiveindex.info, CC0), nine rows over \
                       361.2–643.9 nm, met within 1.5 × 10⁻⁴ by the gas's dispersion (Peck and \
                       Fisher 1964) scaled by c = 0.990 053 at the WebBook's density, which fills \
                       the grid; measured, band ±0.0003 (+0.0002 beyond 643.9 nm); 83.81 K, \
                       0.0042 K above the row's triple point, is the state the index is carried \
                       from, `Measured` to 90 K, where the carry meets the source's warmer tables \
                       within 5 × 10⁻⁴",
            basis: Basis::Reduced,
        },
        Source {
            covers: "b",
            citation: "Einstein–Smoluchowski–Cabannes as for water, κ_T from Tegeler, Span and \
                       Wagner 1999 (`rows/equations.rs`), δ from argon's interaction anisotropy \
                       alone, x = 0.006 (L ÷ L_CH4)² = 0.003 93 with L the Lorentz–Lorenz function \
                       at the triple point at 550 nm, a stand-in scaled from methane's \
                       calibration on liquid CCl₄ (decision-p14-t49h-gas-index §4.3), about 0.011 \
                       at the triple point; `Derived`; `Derived` too where q_max ξ passes 0.1, ξ_0 \
                       = 0.13 nm, Γ_0 = 0.055, γ = 1.2415 and T_R = 301.374 K from Lemmon and \
                       Jacobsen 2004 (Int. J. Thermophys. 25, 21) as CoolProp's `Argon.json` \
                       quotes them, not read, with ν = 0.63; p_c = 4.863 MPa, the equation's \
                       critical point as CoolProp's `Argon.json` holds it",
            basis: Basis::Raw { licence: "MIT" },
        },
        Source {
            covers: "σ",
            citation: MULERO,
            basis: Basis::Raw { licence: "MIT" },
        },
        NO_RULED_VISCOSITY,
    ],
};

/// Liquid krypton (decision-p14-t49h-gas-index §1.2): its index R08.T5.k's `krypton-liquid.json`,
/// Sinnock and Smith 1969's table of the liquid at its 115.95 K melting point, labelled with the
/// row's 115.77 K triple point, `Measured` over its checked window, 115.77–115.95 K
/// ([`KRYPTON_INDEX_CHECKED`]), narrowed from the ruled 115.95–126 K, and `Derived` beyond it; its
/// absorption not modelled; its scattering ES–Cabannes from Lemmon and Span 2006's `κ_T` with its
/// interaction anisotropy alone, a stand-in; its surface tension Mulero et al.'s; no viscosity is
/// ruled.
const KRYPTON: LiquidColumns = LiquidColumns {
    optics: Some(LiquidOptics {
        index: &bins::KRYPTON_LIQUID_N,
        absorption: None,
        state: Kelvin::new(115.77),
        index_basis: LiquidBasis::Measured,
        index_checked: Some(KRYPTON_INDEX_CHECKED),
        absorption_checked: None,
    }),
    scattering: Some(LiquidScattering {
        equation: &equations::KRYPTON,
        anisotropy_ratio: KRYPTON_ANISOTROPY,
        basis: LiquidBasis::Derived,
        fluctuations: CorrelationLength {
            amplitude_m: R12_08_XI_0_M,
            susceptibility_amplitude: R12_08_GAMMA_0,
            nu: 0.630,
            gamma: 1.239,
            reference_temperature: Kelvin::new(1.5 * 209.48),
            critical_pressure: Pascals::new(5.525e6),
            critical_density: KilogramsPerCubicMetre::new(10_850.0 * 0.083_798),
        },
    }),
    surface_tension: Some(SurfaceTensionEquation {
        critical_temperature: Kelvin::new(209.48),
        terms: &[(0.044_7, 1.245)],
        valid_from: Kelvin::new(115.77),
        valid_to: Kelvin::new(209.48),
        basis: LiquidBasis::Measured,
    }),
    viscosity: None,
    supercooled_viscosity: None,
    specific_heat: None,
    constitution: LiquidConstitution::Pure {
        components: &[KRYPTON_ID],
        reference: &[(KRYPTON_ID, 1.0)],
    },
    sources: &[
        Source {
            covers: "n",
            citation: "R08's `krypton-liquid.json` (R08.T5.k; decision-p14-t49h-gas-index §2.3 and \
                       A.3): Sinnock and Smith 1969's table of the liquid at its melting point, \
                       115.95 K (Phys. Rev. 181, 1297, through refractiveindex.info, CC0), eight \
                       rows over 363.1–643.9 nm, linear between them, and beyond 643.9 nm a \
                       two-term Cauchy fit to the six rows from 435.8 nm, within 1.1 × 10⁻⁴; \
                       measured, band ±0.0005; labelled with the row's triple point, 115.77 K, \
                       0.18 K below the source's, the state the index is carried from, \
                       `Measured` to 115.95 K only, where the carry meets the source within 5 × \
                       10⁻⁴ (its 118–126 K tables miss by 1.25–1.72 × 10⁻³)",
            basis: Basis::Reduced,
        },
        Source {
            covers: "b",
            citation: "Einstein–Smoluchowski–Cabannes as for water, κ_T from Lemmon and Span 2006 \
                       (`rows/equations.rs`), δ from krypton's interaction anisotropy alone, x = \
                       0.006 (L ÷ L_CH4)² = 0.006 39, as argon's, about 0.017 at the triple point, \
                       a stand-in; `Derived`; `Derived` too where q_max ξ passes 0.1, ξ by IAPWS \
                       R12-08's Eqs. (20)–(21a) with its own amplitudes (ξ_0 = 0.13 nm, Γ_0 = \
                       0.06, T_R = 1.5 T_c), in want of krypton's; p_c = 5.525 MPa, the \
                       equation's critical point as CoolProp's `Krypton.json` holds it",
            basis: Basis::Fact,
        },
        Source {
            covers: "σ",
            citation: MULERO,
            basis: Basis::Raw { licence: "MIT" },
        },
        NO_RULED_VISCOSITY,
    ],
};

/// Liquid xenon (decision-p14-t49h-gas-index §1.2): its index R08.T5.k's `xenon-liquid.json`,
/// Sinnock and Smith 1969's table of the liquid at its 161.35 K melting point, labelled with the
/// row's 161.4 K triple point, `Measured` at that state alone ([`XENON_INDEX_CHECKED`]), narrowed
/// from the ruled 161.35–178 K, and `Derived` above it; its absorption not modelled; its scattering
/// ES–Cabannes from Lemmon and Span 2006's `κ_T` with its interaction anisotropy alone, a stand-in;
/// its surface tension Mulero et al.'s, whose two terms' sum turns negative 0.12 K below the
/// critical point, where it is absent; no viscosity is ruled.
const XENON: LiquidColumns = LiquidColumns {
    optics: Some(LiquidOptics {
        index: &bins::XENON_LIQUID_N,
        absorption: None,
        state: Kelvin::new(161.4),
        index_basis: LiquidBasis::Measured,
        index_checked: Some(XENON_INDEX_CHECKED),
        absorption_checked: None,
    }),
    scattering: Some(LiquidScattering {
        equation: &equations::XENON,
        anisotropy_ratio: XENON_ANISOTROPY,
        basis: LiquidBasis::Derived,
        fluctuations: CorrelationLength {
            amplitude_m: 0.182e-9,
            susceptibility_amplitude: 0.058,
            nu: 0.63,
            gamma: 1.239,
            reference_temperature: Kelvin::new(434.6),
            critical_pressure: Pascals::new(5.842e6),
            critical_density: KilogramsPerCubicMetre::new(8_400.0 * 0.131_293),
        },
    }),
    surface_tension: Some(SurfaceTensionEquation {
        critical_temperature: Kelvin::new(289.733),
        terms: &[(-0.115_38, 1.051_2), (0.165_98, 1.098)],
        valid_from: Kelvin::new(161.4),
        valid_to: Kelvin::new(289.733),
        basis: LiquidBasis::Measured,
    }),
    viscosity: None,
    supercooled_viscosity: None,
    specific_heat: None,
    constitution: LiquidConstitution::Pure {
        components: &[XENON_ID],
        reference: &[(XENON_ID, 1.0)],
    },
    sources: &[
        Source {
            covers: "n",
            citation: "R08's `xenon-liquid.json` (R08.T5.k; decision-p14-t49h-gas-index §2.3 and \
                       A.3): Sinnock and Smith 1969's table of the liquid at its melting point, \
                       161.35 K (Phys. Rev. 181, 1297, through refractiveindex.info, CC0), ten \
                       rows over 361.2–643.9 nm, linear between them, and beyond 643.9 nm a \
                       two-term Cauchy fit to the seven rows from 435.8 nm, within 1.8 × 10⁻⁴; \
                       measured, band ±0.0005; labelled with the row's triple point, 161.4 K, \
                       0.05 K above the source's, the state the index is carried from and its \
                       only `Measured` one, the carry missing the source's 166–178 K tables by \
                       6.3–8.1 × 10⁻⁴",
            basis: Basis::Reduced,
        },
        Source {
            covers: "b",
            citation: "Einstein–Smoluchowski–Cabannes as for water, κ_T from Lemmon and Span 2006 \
                       (`rows/equations.rs`), δ from xenon's interaction anisotropy alone, x = \
                       0.006 (L ÷ L_CH4)² = 0.010 16, as argon's, about 0.027 at the triple point, \
                       a stand-in; `Derived`; `Derived` too where q_max ξ passes 0.1, ξ_0 = 0.182 \
                       nm, Γ_0 = 0.058 and T_R = 434.6 K from Velliadou, Tasidou, Antoniadis, \
                       Assael, Perkins and Huber 2021's xenon thermal-conductivity correlation \
                       (Int. J. Thermophys., Sec. 2.3) as CoolProp's `Xenon.json` quotes it, not \
                       read, with ν = 0.63; p_c = 5.842 MPa, the equation's critical point as \
                       CoolProp's `Xenon.json` holds it",
            basis: Basis::Raw { licence: "MIT" },
        },
        Source {
            covers: "σ",
            citation: MULERO,
            basis: Basis::Raw { licence: "MIT" },
        },
        NO_RULED_VISCOSITY,
    ],
};

/// Liquid nitrous oxide (decision-p14-t49h-gas-index §1.2): its index R08.T5.k's
/// `nitrous-oxide-liquid.json`, the gas's dispersion scaled to Liveing and Dewar 1892's five lines
/// at the 184.67 K boiling point, `Derived` (band ±0.003); its absorption not modelled; its
/// scattering ES–Cabannes from Lemmon and Span 2006's `κ_T`; its surface tension Mulero et al.'s;
/// no viscosity is ruled.
const NITROUS_OXIDE: LiquidColumns = LiquidColumns {
    optics: Some(LiquidOptics {
        index: &bins::NITROUS_OXIDE_LIQUID_N,
        absorption: None,
        state: Kelvin::new(184.67),
        index_basis: LiquidBasis::Derived,
        index_checked: None,
        absorption_checked: None,
    }),
    scattering: Some(LiquidScattering {
        equation: &equations::NITROUS_OXIDE,
        anisotropy_ratio: NITROUS_OXIDE_ANISOTROPY,
        basis: LiquidBasis::Derived,
        fluctuations: CorrelationLength {
            amplitude_m: R12_08_XI_0_M,
            susceptibility_amplitude: R12_08_GAMMA_0,
            nu: 0.630,
            gamma: 1.239,
            reference_temperature: Kelvin::new(1.5 * 309.52),
            critical_pressure: Pascals::new(7.245e6),
            critical_density: KilogramsPerCubicMetre::new(10_270.0 * 0.044_012_8),
        },
    }),
    surface_tension: Some(SurfaceTensionEquation {
        critical_temperature: Kelvin::new(309.52),
        terms: &[(0.070_87, 1.204)],
        valid_from: Kelvin::new(182.33),
        valid_to: Kelvin::new(309.52),
        basis: LiquidBasis::Measured,
    }),
    viscosity: None,
    supercooled_viscosity: None,
    specific_heat: None,
    constitution: LiquidConstitution::Pure {
        components: &[NITROUS_OXIDE_ID],
        reference: &[(NITROUS_OXIDE_ID, 1.0)],
    },
    sources: &[
        Source {
            covers: "n",
            citation: "R08's `nitrous-oxide-liquid.json` (R08.T5.k; decision-p14-t49h-gas-index \
                       §2.3): rayleigh.ts's gas dispersion (He, Fang, Shoshanim, Brown and Rudich \
                       2021) at the NIST WebBook's 27.957 49 mol/l at 184.67 K, the normal boiling \
                       point and the state the index is carried from, its molar refraction scaled \
                       by c = 0.969 096, fitted by least squares to Liveing and Dewar 1892's C, D, \
                       F, G and In lines (Phil. Mag. (5) 34, 205); derived, band ±0.003",
            basis: Basis::Reduced,
        },
        Source {
            covers: "b",
            citation: "Einstein–Smoluchowski–Cabannes as for water, κ_T from Lemmon and Span 2006 \
                       (`rows/equations.rs`), δ from x = 45ρ_v ÷ (3 − 4ρ_v) + 0.006 with the gas's \
                       ρ_v = 0.0616 at 550 nm (Sneep and Ubachs 2005, Eq. 19), about 0.67 at the \
                       triple point; `Derived`; `Derived` too where q_max ξ passes 0.1, ξ by IAPWS \
                       R12-08's Eqs. (20)–(21a) with its own amplitudes (ξ_0 = 0.13 nm, Γ_0 = \
                       0.06, T_R = 1.5 T_c), in want of nitrous oxide's; p_c = 7.245 MPa, the \
                       equation's critical point as CoolProp's `NitrousOxide.json` holds it",
            basis: Basis::Fact,
        },
        Source {
            covers: "σ",
            citation: MULERO,
            basis: Basis::Raw { licence: "MIT" },
        },
        NO_RULED_VISCOSITY,
    ],
};

/// Liquid carbon monoxide (decision-p14-t49h-gas-index §1.2 and A.4): its index R08.T5.k's
/// `carbon-monoxide-liquid.json`, an unscaled Lorentz–Lorenz estimate from the gas at the
/// liquid's density, which the file calls a stand-in (`standIn`) and the record `Derived`, an
/// estimation method by decision-r11-palette-liquids §1.1's vocabulary, which keeps `StandIn` for a
/// named analogue or a stated constant: the two vocabularies differ on purpose. Its band is +0 /
/// −0.015, likely high. Its absorption is not modelled; its scattering ES–Cabannes from Lemmon and
/// Span 2006's `κ_T`; its surface tension Mulero et al.'s; no viscosity is ruled.
const CARBON_MONOXIDE: LiquidColumns = LiquidColumns {
    optics: Some(LiquidOptics {
        index: &bins::CARBON_MONOXIDE_LIQUID_N,
        absorption: None,
        state: Kelvin::new(68.16),
        index_basis: LiquidBasis::Derived,
        index_checked: None,
        absorption_checked: None,
    }),
    scattering: Some(LiquidScattering {
        equation: &equations::CARBON_MONOXIDE,
        anisotropy_ratio: CARBON_MONOXIDE_ANISOTROPY,
        basis: LiquidBasis::Derived,
        fluctuations: CorrelationLength {
            amplitude_m: R12_08_XI_0_M,
            susceptibility_amplitude: R12_08_GAMMA_0,
            nu: 0.630,
            gamma: 1.239,
            reference_temperature: Kelvin::new(1.5 * 132.86),
            critical_pressure: Pascals::new(3.494e6),
            critical_density: KilogramsPerCubicMetre::new(10_850.0 * 0.028_010_1),
        },
    }),
    surface_tension: Some(SurfaceTensionEquation {
        critical_temperature: Kelvin::new(132.86),
        terms: &[(0.028_43, 1.148)],
        valid_from: Kelvin::new(68.16),
        valid_to: Kelvin::new(132.86),
        basis: LiquidBasis::Measured,
    }),
    viscosity: None,
    supercooled_viscosity: None,
    specific_heat: None,
    constitution: LiquidConstitution::Pure {
        components: &[CARBON_MONOXIDE_ID],
        reference: &[(CARBON_MONOXIDE_ID, 1.0)],
    },
    sources: &[
        Source {
            covers: "n",
            citation: "R08's `carbon-monoxide-liquid.json` (R08.T5.k; decision-p14-t49h-gas-index \
                       §2.3 and A.4): an unscaled Lorentz–Lorenz estimate, rayleigh.ts's gas \
                       dispersion (C. and M. Cuthbertson 1920) at the NIST WebBook's 30.329 71 \
                       mol/l at 68.16 K, the triple point and the state the index is carried \
                       from; the file a stand-in, since Gagnon, Gammon, Kiefte and Clouter 1979's \
                       1.2213 at 514.5 nm and 1.2122 at 632.8 nm (Appl. Opt. 18, 1237, their \
                       abstract) contradict each other beyond their errors; band +0 / −0.015, \
                       likely high; `Derived` here, the record's word for an estimation method",
            basis: Basis::Reduced,
        },
        Source {
            covers: "b",
            citation: "Einstein–Smoluchowski–Cabannes as for water, κ_T from Lemmon and Span 2006 \
                       (`rows/equations.rs`), δ from x = 45ρ_v ÷ (3 − 4ρ_v) + 0.006 with the gas's \
                       ρ_v from its King factor, 1.017 (Bogaard et al. 1978; Couling and Graham \
                       1994's measured 1.0176 would give x 0.0853 against 0.0825), about 0.23 at \
                       the triple point; `Derived`; `Derived` too where q_max ξ passes 0.1, ξ by \
                       IAPWS R12-08's Eqs. (20)–(21a) with its own amplitudes (ξ_0 = 0.13 nm, Γ_0 \
                       = 0.06, T_R = 1.5 T_c), in want of carbon monoxide's; p_c = 3.494 MPa, \
                       the equation's critical point as CoolProp's `CarbonMonoxide.json` holds it",
            basis: Basis::Fact,
        },
        Source {
            covers: "σ",
            citation: MULERO,
            basis: Basis::Raw { licence: "MIT" },
        },
        NO_RULED_VISCOSITY,
    ],
};

/// Liquid sulphur dioxide (decision-p14-t49h-gas-index §1.2): its index R08.T5.k's
/// `sulphur-dioxide-liquid.json`, the gas's dispersion scaled to three room-temperature
/// measurements, `Derived` (band ±0.003); its absorption not modelled (the file draws no violet
/// edge of the gas's ã–X̃ band); its surface tension Mulero et al.'s at the row's critical point,
/// whose three terms' sum turns negative 13.1 K below it, at 417.55 K, where it is absent. Its
/// scattering is absent: no depolarisation of the gas or the liquid was read, so its anisotropy
/// ratio has no source (§4.3), and Gao, Wu, Zhang and Lemmon 2016's equation of state, which
/// CoolProp's files hold, is not wired, as nothing would read it. No viscosity is ruled.
const SULPHUR_DIOXIDE: LiquidColumns = LiquidColumns {
    optics: Some(LiquidOptics {
        index: &bins::SULPHUR_DIOXIDE_LIQUID_N,
        absorption: None,
        state: Kelvin::new(197.7),
        index_basis: LiquidBasis::Derived,
        index_checked: None,
        absorption_checked: None,
    }),
    scattering: None,
    surface_tension: Some(SurfaceTensionEquation {
        critical_temperature: Kelvin::new(430.64),
        terms: &[(0.080_3, 0.928), (0.013_9, 1.57), (-0.011_4, 0.364)],
        valid_from: Kelvin::new(197.7),
        valid_to: Kelvin::new(430.64),
        basis: LiquidBasis::Measured,
    }),
    viscosity: None,
    supercooled_viscosity: None,
    specific_heat: None,
    constitution: LiquidConstitution::Pure {
        components: &[SULPHUR_DIOXIDE_ID],
        reference: &[(SULPHUR_DIOXIDE_ID, 1.0)],
    },
    sources: &[
        Source {
            covers: "n",
            citation: "R08's `sulphur-dioxide-liquid.json` (R08.T5.k; decision-p14-t49h-gas-index \
                       §2.3): rayleigh.ts's estimated gas dispersion (C. and M. Cuthbertson 1910) \
                       at the NIST WebBook's 25.411 32 mol/l at 197.7 K, the triple point and the \
                       state the index is carried from, 0.44% past Gao et al. 2016's stated 25.3 \
                       mol/l, its molar refraction scaled by c = 0.994 335, the mean ratio of \
                       Ketteler 1865's, Bichat 1880's and Bleekrode 1884's liquid at 589.3 nm \
                       (Proc. R. Soc. Lond. 37, 339); derived, band ±0.003",
            basis: Basis::Reduced,
        },
        Source {
            covers: "b",
            citation: "absent: no depolarisation of the gas or the liquid was read (rayleigh.ts \
                       holds none: 'F_K lies near 1.02–1.07'), so the anisotropy ratio has no \
                       source (decision-p14-t49h-gas-index §4.3)",
            basis: Basis::Adopted,
        },
        Source {
            covers: "σ",
            citation: MULERO,
            basis: Basis::Raw { licence: "MIT" },
        },
        NO_RULED_VISCOSITY,
    ],
};

/// Liquid hydrogen sulphide (decision-p14-t49h-gas-index §1.2 and A.4): its index R08.T5.k's
/// `hydrogen-sulphide-liquid.json`, an unscaled Lorentz–Lorenz estimate, the file's stand-in and
/// the record's `Derived`, as [`CARBON_MONOXIDE`]'s; its band −0.005 / +0.021, likely low. Its
/// absorption is not modelled; its scattering ES–Cabannes from Lemmon and Span 2006's `κ_T`; its
/// surface tension Mulero et al.'s; no viscosity is ruled.
const HYDROGEN_SULPHIDE: LiquidColumns = LiquidColumns {
    optics: Some(LiquidOptics {
        index: &bins::HYDROGEN_SULPHIDE_LIQUID_N,
        absorption: None,
        state: Kelvin::new(187.7),
        index_basis: LiquidBasis::Derived,
        index_checked: None,
        absorption_checked: None,
    }),
    scattering: Some(LiquidScattering {
        equation: &equations::HYDROGEN_SULPHIDE,
        anisotropy_ratio: HYDROGEN_SULPHIDE_ANISOTROPY,
        basis: LiquidBasis::Derived,
        fluctuations: CorrelationLength {
            amplitude_m: R12_08_XI_0_M,
            susceptibility_amplitude: R12_08_GAMMA_0,
            nu: 0.630,
            gamma: 1.239,
            reference_temperature: Kelvin::new(1.5 * 373.1),
            critical_pressure: Pascals::new(9.0e6),
            critical_density: KilogramsPerCubicMetre::new(10_190.0 * 0.034_080_88),
        },
    }),
    surface_tension: Some(SurfaceTensionEquation {
        critical_temperature: Kelvin::new(373.1),
        terms: &[(0.078_557, 1.207_4)],
        valid_from: Kelvin::new(187.7),
        valid_to: Kelvin::new(373.1),
        basis: LiquidBasis::Measured,
    }),
    viscosity: None,
    supercooled_viscosity: None,
    specific_heat: None,
    constitution: LiquidConstitution::Pure {
        components: &[HYDROGEN_SULPHIDE_ID],
        reference: &[(HYDROGEN_SULPHIDE_ID, 1.0)],
    },
    sources: &[
        Source {
            covers: "n",
            citation: "R08's `hydrogen-sulphide-liquid.json` (R08.T5.k; \
                       decision-p14-t49h-gas-index §2.3 and A.4): an unscaled Lorentz–Lorenz \
                       estimate, rayleigh.ts's gas dispersion (C. and M. Cuthbertson 1910) at the \
                       NIST WebBook's 29.116 33 mol/l at 187.7 K, the triple point and the state \
                       the index is carried from; the file a stand-in, since Bleekrode 1884's \
                       1.380 at 12.5 °C and 1.384 at 18.5 °C run the wrong way with temperature, \
                       0.010 and 0.021 above it; band −0.005 / +0.021, likely low; `Derived` here, \
                       the record's word for an estimation method",
            basis: Basis::Reduced,
        },
        Source {
            covers: "b",
            citation: "Einstein–Smoluchowski–Cabannes as for water, κ_T from Lemmon and Span 2006 \
                       (`rows/equations.rs`), δ from x = 45ρ_v ÷ (3 − 4ρ_v) + 0.006 with the gas's \
                       ρ_v from its King factor, 1.000 155 (Bogaard et al. 1982 and Monan et al. \
                       1982, without the vibrational Raman lines), about 0.025 at the triple \
                       point, its interaction part dominating; `Derived`; `Derived` too where \
                       q_max ξ passes 0.1, ξ by IAPWS R12-08's Eqs. (20)–(21a) with its own \
                       amplitudes (ξ_0 = 0.13 nm, Γ_0 = 0.06, T_R = 1.5 T_c), in want of hydrogen \
                       sulphide's; p_c = 9.000 MPa, the equation's critical point as CoolProp's \
                       `HydrogenSulfide.json` holds it",
            basis: Basis::Fact,
        },
        Source {
            covers: "σ",
            citation: MULERO,
            basis: Basis::Raw { licence: "MIT" },
        },
        NO_RULED_VISCOSITY,
    ],
};

/// Liquid ethane: the index levelled to Hagermann et al. 2005's measured 1.3887 at 670 nm, 94 K and
/// 1.5 bar, its slope a Lorentz–Lorenz estimate, so `Derived` (decision-r08-t5g §3). Its
/// absorption is not modelled until a measured visible spectrum is found
/// (decision-r11-palette-liquids §1.8). Its scattering is ES–Cabannes from Bücker and Wagner 2006's
/// `κ_T`, its surface tension Mulero et al.'s and its viscosity Friend, Ingham and Ely 1991's,
/// continued below its triple point by the fitted Vogel–Fulcher–Tammann form.
const ETHANE: LiquidColumns = LiquidColumns {
    optics: Some(LiquidOptics {
        index: &bins::ETHANE_LIQUID_N,
        absorption: None,
        state: Kelvin::new(94.0),
        index_basis: LiquidBasis::Derived,
        index_checked: None,
        absorption_checked: None,
    }),
    scattering: Some(LiquidScattering {
        equation: &equations::ETHANE,
        anisotropy_ratio: ETHANE_ANISOTROPY,
        basis: LiquidBasis::Derived,
        fluctuations: CorrelationLength {
            amplitude_m: 0.19e-9,
            susceptibility_amplitude: 0.056_3,
            nu: 0.63,
            gamma: 1.242,
            reference_temperature: Kelvin::new(2.0 * 305.322),
            critical_pressure: Pascals::new(4.8722e6),
            critical_density: KilogramsPerCubicMetre::new(6_856.886_685 * 0.030_069_04),
        },
    }),
    surface_tension: Some(SurfaceTensionEquation {
        critical_temperature: Kelvin::new(305.322),
        terms: &[(0.076_02, 1.32), (-0.029_12, 1.676)],
        valid_from: Kelvin::new(90.368),
        valid_to: Kelvin::new(305.322),
        basis: LiquidBasis::Measured,
    }),
    viscosity: Some(ViscosityCorrelation::Friend1991(&FRIEND_1991)),
    supercooled_viscosity: Some(SupercooledViscosity {
        b_kelvin: bins::ETHANE_SUPERCOOLED_VISCOSITY_B_K,
        divergence: Kelvin::new(bins::ETHANE_SUPERCOOLED_VISCOSITY_T0_K),
        triple: Kelvin::new(90.368),
    }),
    specific_heat: Some(LiquidSpecificHeat {
        equation: &equations::ETHANE,
        ideal: &equations::ETHANE_IDEAL,
        low: Kelvin::new(95.0),
        high: Kelvin::new(150.0),
    }),
    constitution: LiquidConstitution::Pure {
        components: &[ETHANE_ID],
        reference: &[(ETHANE_ID, 1.0)],
    },
    sources: &[
        Source {
            covers: "n",
            citation: "R08's `ethane-liquid.json` (R08.T5.h): a Cauchy line levelled to 1.3887 at \
                       670 nm, Hagermann et al. 2005's pure-ethane index (MNRAS 359, 637, Table 3, \
                       at Table 2's 94 K and 1.5 bar), its slope Loria 1909's gas dispersion \
                       carried by Lorentz–Lorenz; 94 K is the state the index is carried from",
            basis: Basis::Reduced,
        },
        Source {
            covers: "b",
            citation: "Einstein–Smoluchowski–Cabannes as for water, κ_T from Bücker and Wagner \
                       2006 (`rows/equations.rs`), δ from x = 45ρ_v ÷ (3 − 4ρ_v) + 0.006 with the \
                       gas's ρ_v = 0.001 59 at 632.8 nm (Keir 1995, Table 6.3), about 0.29 at the \
                       triple point; `Derived`; `Derived` too where q_max ξ passes 0.1, ξ by \
                       Friend, Ingham and Ely 1991's Eq. 19 with their Table 10 (ξ_0 = 0.19 nm, \
                       Γ_0 = 0.0563, γ = 1.242, ν = 0.63, T_R = 2 T_c)",
            basis: Basis::Fact,
        },
        Source {
            covers: "σ",
            citation: MULERO,
            basis: Basis::Raw { licence: "MIT" },
        },
        Source {
            covers: "μ",
            citation: "Friend, Ingham and Ely 1991 (J. Phys. Chem. Ref. Data 20, 275; NIST's scan, \
                       srd.nist.gov jpcrd410), Eqs. 8, 10a, 12 and 15a with Tables 1, 8 and 9, at \
                       the record's density; approximately 90–500 K below 60 MPa (their \
                       abstract; their data's 95–500 K and 0.1–69 MPa, p. 280), about 2%. Vogel, \
                       Span and Herrmann 2015 (J. Phys. Chem. Ref. Data 44, 043101), the newer \
                       reference correlation decision-p14-t49h-liquids §6 prefers, is closed \
                       (OpenAlex, 2026-10-16: no open or repository copy), so §6's fallback is \
                       taken",
            basis: Basis::Fact,
        },
        Source {
            covers: "μ below T_t",
            citation: "`tables::liquid_optics`' Vogel–Fulcher–Tammann B and T_0, fitted by \
                       `hyperion-fit` as methane's; T_0 = 0, an Arrhenius form: along the \
                       record's saturated liquid the correlation's effective activation energy \
                       rises with temperature over the window, and the ancillary density, 0.066% \
                       low at the triple point, flattens it further below about 100 K; `Derived`",
            basis: Basis::Fitted,
        },
        Source {
            covers: "c_l",
            citation: "Bücker and Wagner 2006 (J. Phys. Chem. Ref. Data 35, 205; its paper not \
                       read), its equation of state evaluated here: the saturated liquid's c_p, \
                       at the row's diagram saturation pressure on the equation's liquid branch, \
                       over 95–150 K and held at the ends (decision-r09-t13i §2.1; \
                       decision-r09-t13i-cl-licence §6); the residual as `rows/equations.rs` \
                       holds it, the ideal part as CoolProp's Ethane.json holds it (MIT, \
                       `NOTICE`); checked against NIST's Chemistry WebBook (SRD 69) alone, which \
                       evaluates the same equation, at asserted states",
            basis: Basis::Fact,
        },
    ],
};

/// Friend, Ingham and Ely 1991's viscosity of ethane: Table 1's ε ÷ `k_B` = 245.0 K and critical
/// point (305.33 K and 6.87 mol dm⁻³ at 30.070 g mol⁻¹, on IPTS-68), Table 8's Cᵢ and Table 9's gᵢ,
/// rᵢ and sᵢ; valid over approximately 90–500 K below 60 megapascals (their abstract), and
/// `Derived` in the reduced critical box. A test reproduces their Table A2's saturated liquid
/// within its densities' four figures.
pub(in crate::substance) const FRIEND_1991: Friend1991Viscosity = Friend1991Viscosity {
    epsilon_k: 245.0,
    collision: [
        -3.032_813_828_1,
        16.918_880_086,
        -37.189_364_917,
        41.288_861_858,
        -24.615_921_14,
        8.948_843_095_9,
        -1.873_924_504_2,
        0.209_661_013_9,
        -9.657_043_707_4e-3,
    ],
    critical_temperature: Kelvin::new(305.33),
    critical_density: KilogramsPerCubicMetre::new(6.87 * 30.070),
    numerator: &[
        (0.471_770_03, 1, 0.0),
        (-0.239_503_11, 1, 1.0),
        (0.398_083_01, 2, 0.0),
        (-0.273_433_35, 2, 1.0),
        (0.351_922_60, 2, 1.5),
        (-0.211_013_08, 3, 0.0),
        (-0.004_785_79, 3, 2.0),
        (0.073_781_29, 4, 0.0),
        (-0.030_425_255, 4, 1.0),
    ],
    denominator: &[(-0.304_352_86, 1, 0.0), (0.001_215_675, 1, 1.0)],
    range: ViscosityRange {
        valid_from: Kelvin::new(90.0),
        valid_to: Kelvin::new(500.0),
        critical_region: Some(reduced_critical_region(
            Kelvin::new(305.322),
            KilogramsPerCubicMetre::new(6_856.886_685 * 0.030_069_04),
        )),
    },
};

/// The aqueous acid's reference composition, the row's own 75.0 wt% `H2SO4`
/// (decision-p14-t49b-condensed §2.6; decision-p14-t49h-step3-composition §5, item 7) as mole
/// fractions in registry order with the rows' molar masses, 18.015 and 98.072 g mol⁻¹, converted
/// as [`BRINE_NACL_REFERENCE`] is: H2O 0.644 714 and `H2SO4` 0.355 286, held as that arithmetic's
/// own values, which a test recomputes. (The research's brief took 98.079 g mol⁻¹, the older
/// atomic weights' sum; the row's IUPAC 2021 98.072 moves the fraction by 2 × 10⁻⁵.)
const SULPHURIC_ACID_REFERENCE: [(SubstanceId, f64); 2] = [
    (WATER_ID, 0.644_714_265_992_624_2),
    (SULPHURIC_ACID_ID, 0.355_285_734_007_375_9),
];

/// Aqueous sulphuric acid (decision-r11-palette-liquids §1.8's acid row), from 25 to 85 wt%
/// ([`AqueousAcid`]). Every figure is the lane's research,
/// `.git/rm45-scratch/p14-liquids/step3/acid/acid.json` (from `scripts/acid_calc.py`), with the
/// SHA-256 of every file it read.
///
/// - **The range in w**, 0.25 to 0.85: where both the index and the density are checked, the
///   index against Palmer and Williams's own 25, 38 and 50 wt% files within ±0.005 at 555.6–701.8
///   nm (−0.0125 to −0.0058 at 359.7–449.4 nm, worst at 38 wt%), the density against the
///   International Critical Tables to 85 wt%.
/// - **The density**, [`MyhreDensity`] with Myhre et al.'s printed T⁴ column dropped: as printed
///   (three open transcriptions agree) that column makes the density rise with temperature above
///   about 295 K at 75 wt%, +7.1 kg m⁻³ at 300 K and +85 at 323 K against the International
///   Critical Tables (vol. III, 1928, p. 57), and +109 at 220 K against Vehkamäki et al. 2002's
///   Eq. 10, which reproduces Myhre's low-temperature densities; a least-squares scale on it
///   against the tables' 0–50 °C at 70–85 wt% comes out −0.004, nil. Without it the form meets the
///   tables there within 0.44 kg m⁻³ rms (worst 1.61), and gives 1,662.59 kg m⁻³ at 75 wt% and 300
///   K, against the condensed columns' 1,662.6 (the tables'), and 1,766.66 at 84.5 wt%, against
///   the tables' 1,766.50.
///   Oca, Campillo-Robles and Bou-Ali 2018 (their accepted manuscript, arXiv:2501.07258, SHA-256
///   45b4421f…, Sec. 2.2.1 and Table 2) state that the 1998 equation "contained errors resulting
///   in minor inaccuracies at low temperatures" and that Lund Myhre, Christensen, Nicolaisen and
///   Nielsen 2003 (J. Phys. Chem. A 107, 1979, doi:10.1021/jp026576n; closed, unread) refitted it
///   with 32 constants and powers of t to 3, the 1998 set less those three terms (whether the rest
///   were refitted is unread). The coefficients are chempy's transcription
///   (`sulfuric_acid_density_myhre_1998.py`, bjodah/chempy, SHA-256 947fadda…), which aerosolpy's
///   (`growth/h2so4.py`, SHA-256 804be89b…, in the paper's "× 10ⁿ" notation) and CARMA's
///   (`freezaerl_tabazadeh2000.F90`, `ESCOMP/CARMA_base`, SHA-256 31a6ceb4…) match in all 35 but
///   `k₆,₁`, CARMA's 4,083.717 against their 4,083.714 (under 0.02 kg m⁻³ at 300 K). Myhre's own
///   measurements span w 0.12–0.75 at 220–300 K, fitted with the tables' 0–50 °C values; the form
///   is stated for w 0.1–0.9 (chempy; Oca's Table 2).
/// - **The index and absorption** between R08's two files, Palmer and Williams 1975's 75 and 84.5
///   wt% acid at 300 K ([`AcidOptics`]).
/// - **The surface tension**, [`VehkamakiSurfaceTension`]: Myhre et al. 1998's own fit, the source
///   decision-r11-palette-liquids §1.8 names, is unread (the paper is closed and no open source
///   gives its form or coefficients), and Vehkamäki et al. 2002's open fit to Myhre's data and
///   older series stands in for it: 71.30 mN m⁻¹ at 80 wt% and 273 K against Myhre's 71.11 (as
///   Dai et al. 2022, arXiv:2207.10238, quote it).
/// - **The viscosity**, [`WilliamsLongViscosity`], the mass percentage held in its stated 30–80:
///   extrapolated to 84.5 wt% its cubics overshoot the International Critical Tables (vol. V, 1929,
///   p. 13, relative to water) by +32% at 25 °C, where holding W at 80 undershoots by −15%.
const SULPHURIC_ACID_IN_WATER: AqueousAcid = AqueousAcid {
    solvent: WATER_ID,
    solute: SULPHURIC_ACID_ID,
    lowest_mass_fraction: 0.25,
    highest_mass_fraction: 0.85,
    density: MyhreDensity {
        coefficients: [
            [999.842_6, 0.033_454_02, -0.005_691_304, 0.0],
            [547.265_9, -5.300_445, 0.011_876_71, 0.000_599_000_8],
            [5_262.95, 37.204_45, 0.120_190_9, -0.004_148_594],
            [-62_139.58, -287.767, -0.406_463_8, 0.011_194_88],
            [409_029.3, 1_270.854, 0.326_971, -0.013_774_35],
            [-1_596_989.0, -3_062.836, 0.136_649_9, 0.006_373_031],
            [3_857_411.0, 4_083.714, -0.192_778_5, 0.0],
            [-5_808_064.0, -2_844.401, 0.0, 0.0],
            [5_301_976.0, 809.105_3, 0.0, 0.0],
            [-2_682_616.0, 0.0, 0.0, 0.0],
            [576_428.8, 0.0, 0.0, 0.0],
        ],
    },
    files: [
        AcidOptics {
            mass_fraction: 0.75,
            index: &bins::SULPHURIC_ACID_75_N,
            absorption: &bins::SULPHURIC_ACID_75_A,
            state: Kelvin::new(300.0),
        },
        AcidOptics {
            mass_fraction: 0.845,
            index: &bins::SULPHURIC_ACID_84_N,
            absorption: &bins::SULPHURIC_ACID_84_A,
            state: Kelvin::new(300.0),
        },
    ],
    surface_tension: VehkamakiSurfaceTension {
        a: [
            0.118_64, -0.116_51, 0.768_52, -2.409_09, 2.954_34, -1.258_52,
        ],
        b: [
            -0.000_157_09,
            0.000_401_02,
            -0.002_399_5,
            0.007_611_235,
            -0.009_373_86,
            0.003_897_22,
        ],
    },
    viscosity: WilliamsLongViscosity {
        amplitude: [169.5, 5.18, -0.082_5, 3.27e-3],
        divergence: [144.11, 0.166, -0.015, 2.18e-4],
        exponent: 1.43,
        activation_k: 448.0,
        lowest_mass_percent: 30.0,
        highest_mass_percent: 80.0,
    },
};

/// Aqueous sulphuric acid, the liquid of the molecule's row (decision-p14-t49h-step3-composition
/// §1, item 4): water and `H2SO4` ([`SULPHURIC_ACID_IN_WATER`]) at the row's 75.0 wt%
/// ([`SULPHURIC_ACID_REFERENCE`]) or a given composition from 25 to 85 wt%, from its 233.67 K
/// first melt to below water's critical point (§3).
const SULPHURIC_ACID: LiquidColumns = LiquidColumns {
    optics: None,
    scattering: None,
    surface_tension: None,
    viscosity: None,
    supercooled_viscosity: None,
    specific_heat: None,
    constitution: LiquidConstitution::Solution(SolutionColumns {
        components: &[WATER_ID, SULPHURIC_ACID_ID],
        reference: &SULPHURIC_ACID_REFERENCE,
        model: MixingModel::AqueousAcid(&SULPHURIC_ACID_IN_WATER),
    }),
    sources: &[
        Source {
            covers: "composition",
            citation: "the row's 75.0 wt% H2SO4 (the condensed columns' reference composition), \
                       as mole fractions with the rows' molar masses: H2O 0.644714 and H2SO4 \
                       0.355286 (decision-p14-t49h-step3-composition §5, item 7); a stand-in for \
                       a body's own until P14.T51.a carries one. A given composition runs from 25 \
                       to 85 wt%, H2SO4 0.057698 to 0.510025, where both the index (Palmer and \
                       Williams's 25, 38 and 50 wt% files, within ±0.005 at 555.6–701.8 nm) and \
                       the density (the International Critical Tables to 85 wt%) are checked",
            basis: Basis::Reduced,
        },
        Source {
            covers: "ρ",
            citation: "Myhre, Nielsen and Saastad 1998 (J. Chem. Eng. Data 43, 617, \
                       doi:10.1021/je980013g; closed), Eq. 2, ρ = Σᵢ Σⱼ kᵢⱼ wⁱ tʲ (t in °C), \
                       without its printed T⁴ column (32 terms): as printed it makes the density \
                       rise with temperature above about 295 K, +7.1 kg m⁻³ at 75 wt% and 300 K \
                       against the International Critical Tables (vol. III, 1928, p. 57); without \
                       it the form meets their 0–50 °C at 70–85 wt% within 0.44 kg m⁻³ rms, and \
                       1,662.59 at 75 wt% and 300 K against the condensed columns' 1,662.6. Oca, \
                       Campillo-Robles and Bou-Ali 2018 (arXiv:2501.07258, SHA-256 45b4421f…, \
                       Sec. 2.2.1 and Table 2) state that Lund Myhre et al. 2003 (J. Phys. Chem. \
                       A 107, 1979; closed, unread) refitted it with powers of t to 3. The \
                       coefficients as chempy transcribes them (SHA-256 947fadda…), matched by \
                       aerosolpy's (804be89b…) and, but k₆,₁ (4,083.717 against 4,083.714), \
                       CARMA's (31a6ceb4…). Above about 390 K at 25–40 and 84.5–85 wt% the cubic \
                       in t turns over and the density rises again, an extrapolation far beyond \
                       its 0–50 °C fit; Derived throughout",
            basis: Basis::Reduced,
        },
        Source {
            covers: "n",
            citation: "R08's `sulphuric-acid-75.json` and `sulphuric-acid-84.json`: Palmer and \
                       Williams 1975 (Appl. Opt. 14, 208), the 75 and 84.5 wt% acid at 300 K, \
                       through Oxford's ARIA (SHA-256 ca84ce9a… and 6ae3eceb…), reduced to the \
                       bake bins by `tables::liquid_optics`; specific refraction r = (n² − 1) ÷ \
                       ((n² + 2) ρ) linear in w between them, each file's r at the density form's \
                       value at its own state, then n at the record's ρ(w, T): both files \
                       reproduced at 300 K, and 1.43599 at 550 nm, 80 wt% and 300 K. Checks: \
                       Palmer and Williams's 25, 38 and 50 wt% files within ±0.005 at \
                       555.6–701.8 nm, −0.013 to −0.006 at 359.7–449.4 nm; Biermann, Luo and \
                       Peter 2000's 80 wt% at 610.4 nm and 293 K within 10⁻⁴, their 75 wt% \
                       0.006 lower (the source's offset), and their 293 → 215 K step, +0.0241, \
                       against the rule's +0.0242 at 75 wt%; Hansen and Hovenier 1974's 1.44 ± \
                       0.015 at 550 nm for Venus's droplets. Extrapolated in w outside 75–84.5 \
                       wt%",
            basis: Basis::Adopted,
        },
        Source {
            covers: "a",
            citation: "the same files' k, each held to the blue at its shortest measured value \
                       (2.07 × 10⁻⁸ at 701.8 nm and 1.14 × 10⁻⁸ at 714.3 nm), upper bounds set by \
                       the measurement's sensitivity; by Beer's additivity by mass between them, \
                       α = ρ(w, T) ((1 − s) α₇₅ ÷ ρ₇₅ + s α₈₄.₅ ÷ ρ₈₄.₅), s = (w − 0.75) ÷ \
                       0.095, an upper bound; outside 75–84.5 wt% the larger mass coefficient \
                       of the two in each bin, held (the 75 wt% file's in every bin), since an \
                       extrapolation would weight one bound negatively, so it steps at 84.5 wt%. \
                       Water alone at its share would give k about 17–20 times less at 550 nm",
            basis: Basis::Adopted,
        },
        Source {
            covers: "σ",
            citation: "Vehkamäki et al. 2002 (J. Geophys. Res. 107 (D22), 4622, \
                       doi:10.1029/2002JD002184), Eq. 9, σ = a(w) + T b(w), its signs read off \
                       the page image (the MPI PuRe copy, \
                       SHA-256 0ab40420…; its text layer drops them; the 2013 correction \
                       concerns Eq. 15 only), fitted over 233–323 K to Myhre et al. 1998's and \
                       older data: a stand-in for Myhre et al. 1998's own fit, the ruled source, \
                       which is unread (closed, and no open copy of its form or coefficients); \
                       71.30 mN m⁻¹ at 80 wt% and 273 K against Myhre's 71.11 (Dai et al. 2022, \
                       arXiv:2207.10238); Mills's Sabinina and Terpugow 1935 (CARMA's \
                       `sulfate_utils.F90`) within 0.4–0.8 mN m⁻¹",
            basis: Basis::Fact,
        },
        Source {
            covers: "μ",
            citation: "Williams and Long 1995 (J. Phys. Chem. 99, 3748, doi:10.1021/j100011a050; \
                       closed), μ = A T^−1.43 exp(448 K ÷ (T − T_0)) mPa s, A and T_0 cubic in \
                       the mass percentage W, as Shi et al. 2001 (J. Geophys. Res. 106, 24259; \
                       closed) reproduce it and CAM (`mo_strato_rates.F90`, lines 627–629, SHA-256 \
                       0e119d5b…), GEOS-Chem (`ucx_mod.F90`, lines 3245–3250, 9ca25847…) and UKCA \
                       (`ukca_hetero_mod.F90`, lines 974–985, 9c53f47e…) print it to every digit; \
                       stated for 200–300 K and 30–80 wt%, W held in [30, 80] outside them, as \
                       UKCA holds it at 80: inside it −5.8% to +3.6% of the International \
                       Critical Tables (vol. V, 1929, p. 13); at 84.5 wt% and 25 °C held −15%, \
                       where the cubics extrapolated would be +32% (the tables' ratio 1.163)",
            basis: Basis::Fact,
        },
        Source {
            covers: "b, range",
            citation: "absent: the acid's κ_T is not sourced (Valleau 1965, J. Acoust. Soc. Am. \
                       38, 669, doi:10.1121/1.1909778, its sound speeds, is closed). The range \
                       runs from the condensed columns' 233.67 K first melt to below water's \
                       647.096 K (decision-p14-t49h-step3-composition §3), every member Derived \
                       outside its source's range",
            basis: Basis::Adopted,
        },
    ],
};

/// Titan's seas' components in registry order, the order every mixing rule sums them in.
const TITAN_SEA_COMPONENTS: [SubstanceId; 3] = [METHANE_ID, NITROGEN_ID, ETHANE_ID];

/// Titan's seas' reference composition, Ligeia Mare's, as mole fractions in registry order: CH4
/// 0.739146, N2 0.164352 and C2H6 0.096503 to six decimals, 0.7391, 0.1644 and 0.0965 to four
/// (decision-p14-t49b-condensed §1.2 and §1.4, ruling 1; decision-p14-t49h-step3-composition §5,
/// item 7). They are derived from the volume fractions of Mastrogiuseppe, Hayes, Poggiali, Seu,
/// Lunine and Hofgartner 2016 (IEEE Trans. Geosci. Remote Sens. 54, 5646, §V.B, p. 5654,
/// doi:10.1109/TGRS.2016.2563426: "a best fit composition (volume fraction) of 71% CH4, 12% C2H6,
/// and 17% N2"), never read as mole fractions: xᵢ = (φᵢ ρᵢ ÷ Mᵢ) ÷ Σⱼ (φⱼ ρⱼ ÷ Mⱼ) at Titan's
/// surface temperature, 93.65 K (the condensed columns' reference state, Huygens's HASI), with the
/// rows' own saturated-liquid densities there (methane's 447.53, nitrogen's 725.71 and ethane's
/// 647.97 kg m⁻³) and molar masses (16.043, 28.014 and 30.07 g mol⁻¹). They are held as that
/// arithmetic's own values, not the six printed decimals, which sum to 1.000 001, so that the
/// reference is a composition [`LiquidComposition::new`](super::super::LiquidComposition::new)
/// accepts; a test recomputes them from the volume fractions. Le Gall et al. 2016 (JGR Planets
/// 121, 233) corroborates only that the sea is methane-dominated. The composition stands in for a
/// body's own until P14.T51.a carries one, so the record is `Derived`.
const TITAN_SEA_REFERENCE: [(SubstanceId, f64); 3] = [
    (METHANE_ID, TITAN_SEA_METHANE),
    (NITROGEN_ID, TITAN_SEA_NITROGEN),
    (ETHANE_ID, TITAN_SEA_ETHANE),
];

/// [`TITAN_SEA_REFERENCE`]'s methane, 0.739146 to six decimals.
const TITAN_SEA_METHANE: f64 = 0.739_145_604_441_499_8;
/// [`TITAN_SEA_REFERENCE`]'s nitrogen, 0.164352 to six decimals.
const TITAN_SEA_NITROGEN: f64 = 0.164_351_884_675_753_3;
/// [`TITAN_SEA_REFERENCE`]'s ethane, 0.096503 to six decimals.
const TITAN_SEA_ETHANE: f64 = 0.096_502_510_882_746_89;

/// Titan's seas' Grunberg–Nissan interaction parameters, each pair of its components once: every
/// one 0 until a sourced value is read, so the mixture's ln μ is the mole-fraction mean of its
/// components' (Arrhenius's logarithmic rule). Tan, Kargel and Marion 2013 (Icarus 222, 53), the
/// cross-check decision-r11-palette-liquids §1.8 names, was not read.
const TITAN_SEA_MIXING: IdealLiquidMixing = IdealLiquidMixing {
    grunberg_nissan: &[
        (METHANE_ID, NITROGEN_ID, 0.0),
        (METHANE_ID, ETHANE_ID, 0.0),
        (NITROGEN_ID, ETHANE_ID, 0.0),
    ],
};

/// Titan's seas' liquid (`ch4_c2h6_n2`): an ideal mixture of liquid methane, nitrogen and ethane
/// ([`TITAN_SEA_COMPONENTS`]) at Ligeia Mare's composition ([`TITAN_SEA_REFERENCE`]) or a given
/// one, each component its own record at the
/// record's temperature, or below its triple point decision-p14-t49b-condensed §2.5's continuation
/// (methane's 90.694 K and ethane's 90.368 K lie inside the sea's range from its 71.15 K first
/// melt). Its absorption is absent while ethane's and nitrogen's are (decision-r11-palette-liquids
/// §1.4); its scattering is the mixture's density and concentration fluctuations'.
const TITAN_SEA: LiquidColumns = LiquidColumns {
    optics: None,
    scattering: None,
    surface_tension: None,
    viscosity: None,
    supercooled_viscosity: None,
    specific_heat: None,
    constitution: LiquidConstitution::Solution(SolutionColumns {
        components: &TITAN_SEA_COMPONENTS,
        reference: &TITAN_SEA_REFERENCE,
        model: MixingModel::IdealLiquids(&TITAN_SEA_MIXING),
    }),
    sources: &[
        Source {
            covers: "composition",
            citation: "Ligeia Mare's best-fit composition by volume fraction, \"71% CH4, 12% \
                       C2H6, and 17% N2\" (Mastrogiuseppe, Hayes, Poggiali, Seu, Lunine and \
                       Hofgartner 2016, IEEE Trans. Geosci. Remote Sens. 54, 5646, §V.B, p. 5654, \
                       doi:10.1109/TGRS.2016.2563426), converted to mole fractions at 93.65 K with \
                       the rows' own saturated-liquid densities and molar masses, xᵢ ∝ φᵢ ρᵢ ÷ Mᵢ: \
                       CH4 0.739146, N2 0.164352 and C2H6 0.096503 (decision-p14-t49b-condensed \
                       §1.2 and §1.4); a stand-in for a body's own composition until P14.T51.a \
                       carries one",
            basis: Basis::Reduced,
        },
        Source {
            covers: "ρ, n",
            citation: "ideal mixing of the components' liquids at the record's temperature: ρ = Σ \
                       xᵢ Mᵢ ÷ Σ xᵢ Mᵢ ÷ ρᵢ, volume additivity, the basis the retrieval's \
                       fractions are defined on (decision-p14-t49b-condensed §1.2), so 518.9 kg \
                       m⁻³ at 93.65 K as the condensed columns hold it; the index by \
                       Lorentz–Lorenz at that molar volume from each component's molar refraction \
                       (nᵢ² − 1) ÷ (nᵢ² + 2) × Mᵢ ÷ ρᵢ in each bin. Methane and ethane's negative \
                       excess volume (Engle et al. 2021) is neglected, as the fractions' \
                       definition requires",
            basis: Basis::Reduced,
        },
        Source {
            covers: "σ",
            citation: "the Macleod–Sugden parachor in Weinaug and Katz's mixture form, the \
                       vapour neglected: σ^(1/4) = (ρ ÷ M) Σ xᵢ Pᵢ, Pᵢ = σᵢ^(1/4) Mᵢ ÷ ρᵢ from \
                       each component's Mulero et al. 2012 tension and density at the record's \
                       temperature (Macleod 1923, Trans. Faraday Soc. 19, 38, \
                       doi:10.1039/tf9231900038; Sugden 1924, J. Chem. Soc., Trans. 125, 1177, \
                       doi:10.1039/ct9242501177; Weinaug and Katz 1943, Ind. Eng. Chem. 35, 239, \
                       doi:10.1021/ie50398a028; their bibliographic records checked, their texts \
                       not read)",
            basis: Basis::Reduced,
        },
        Source {
            covers: "μ",
            citation: "Grunberg and Nissan 1949 (Nature 164, 799, doi:10.1038/164799b0; its \
                       bibliographic record checked, its text not read), ln μ = Σ xᵢ ln μᵢ + ½ ΣΣ \
                       xᵢ xⱼ Gᵢⱼ, of the components' correlations (Sotiriadou et al. 2026, Huber, \
                       Perkins and Lemmon 2024, Friend, Ingham and Ely 1991), every Gᵢⱼ 0 until \
                       sourced values are read; Tan, Kargel and Marion 2013 (Icarus 222, 53), the \
                       cross-check, not read",
            basis: Basis::Adopted,
        },
        Source {
            covers: "a",
            citation: "Beer's additivity in the components' volume fractions at the record's \
                       temperature where every component present has one, so absent while \
                       ethane's and nitrogen's are (decision-r11-palette-liquids §1.4)",
            basis: Basis::Adopted,
        },
        Source {
            covers: "b",
            citation: "Einstein–Smoluchowski–Cabannes with ideal-mixing κ_T plus an ideal-solution \
                       concentration term (decision-r11-palette-liquids §1.8): the density term \
                       the pure liquid's form (Zhang, Hu and He 2009, Opt. Express 17, 5698, their \
                       published code betasw_ZHH2009.m, SHA-256 089a2d5d…) with κ_T,mix = Σ φᵢ \
                       κ_T,i by volume fraction and PMH's ρ ∂n²/∂ρ at the mixture's index; the \
                       concentration term (π²/2) λ⁻⁴ (V_m/N_A) [Σ xᵢ εᵢ'² − (Σ xᵢ εᵢ')²], εᵢ' = \
                       ((ε + 2)²/3) (Vᵢ/V_m) (Lᵢ − L), from an ideal solution's multinomial \
                       composition fluctuations ⟨δxᵢ δxⱼ⟩ = (xᵢ δᵢⱼ − xᵢ xⱼ)/N (Kirkwood and \
                       Goldberg 1950, J. Chem. Phys. 18, 54; Stockmayer 1950, J. Chem. Phys. 18, \
                       58; not read); one depolarisation ratio from the mixture's anisotropy ratio \
                       Σ xᵢ xᵢ' Rᵢ² / (Σ xᵢ Rᵢ)² and isotropic structure factor S_0,mix (1 + \
                       β_c/β_d) on both terms; derived and checked in the lane's research \
                       (rm45-scratch/p14-liquids/step3/mixtures/, scripts/mixtures.py, SHA-256 \
                       7f0b8304…, and mixtures.json): 6.96 × 10⁻³ and 2.91 × 10⁻³ m⁻¹ in the bins \
                       across 443 and 545 nm at 93.65 K, about half of each the concentration \
                       term's; a lower bound, methane–nitrogen deviating positively from Raoult's \
                       law",
            basis: Basis::Adopted,
        },
        Source {
            covers: "below T_t",
            citation: "decision-p14-t49b-condensed §2.5 for a component below its triple point: \
                       its density linear at its ancillary's slope there, its index by \
                       Lorentz–Lorenz at that density, κ_T held at its triple-point value, its \
                       viscosity its fitted Vogel–Fulcher–Tammann form and its surface tension \
                       Mulero et al.'s form; and between the 71.15 K first melt and the \
                       reference composition's liquidus, unmapped for the ternary, the supercooled \
                       liquid at the reference composition (§2.4); `Derived`",
            basis: Basis::Adopted,
        },
    ],
};

/// The salts' specific refraction's dispersion, a stand-in (decision-r11-palette-liquids §1.8's
/// brine row; the lane's research, `.git/rm45-scratch/p14-liquids/step3/brines/brines.json`, its
/// `NaCl_r_salt_lambda_standin_QF_shape`, from `scripts/brines_compute.py`, SHA-256 b4695646…):
/// each wavelength's factor on the salt's specific refraction at 589.3 nm, nm and dimensionless,
/// linear between them. It is the factor that makes a 3.5 wt% `NaCl` brine's index increment over
/// water at 25 °C follow Quan and Fry 1995's seawater shape, (n₁ + n₂ t + n₃ t²) S + n₆ S ÷ λ
/// (Appl. Opt. 34, 3477; their coefficients as R08's `seawater.json` reads them from the Ocean
/// Optics Web Book), with IAPWS R9-97's water: a wavelength-independent salt refraction gives the
/// increment a weak red-rising dispersion (0.985 at 380 nm and 1.005 at 780 nm of its 589 nm
/// value), the opposite of seawater's measured 1.060 and 0.974 (brines.json's finding 2). `NaCl`'s
/// own dispersion (Tan and Huang 2015) is unread, and none was found for `MgSO4`, which takes the
/// same factor as the only proxy.
const SALT_REFRACTION_DISPERSION: [(f64, f64); 9] = [
    (380.0, 1.0437),
    (400.0, 1.0374),
    (450.0, 1.0242),
    (500.0, 1.0139),
    (550.0, 1.0055),
    (600.0, 0.998_62),
    (650.0, 0.992_81),
    (700.0, 0.987_83),
    (780.0, 0.981_15),
];

/// The chloride brine's reference composition, its eutectic's: the condensed columns' 23.3 wt%
/// `NaCl` (`rows/condensed.rs`'s `BRINE_NACL`) as mole fractions in registry order, with the rows'
/// molar masses, 18.015 and 58.44 g mol⁻¹: `x_s` = (w ÷ `M_s`) ÷ (w ÷ `M_s` + (1 − w) ÷ `M_w`)
/// and `x_w` = ((1 − w) ÷ `M_w`) ÷ (the same sum), H2O 0.914 373 and `NaCl` 0.085 627
/// (decision-p14-t49h-step3-composition §5, item 7), held as that arithmetic's own values, which a
/// test recomputes. It stands in for a body's own until P14.T51.a carries one.
const BRINE_NACL_REFERENCE: [(SubstanceId, f64); 2] = [
    (WATER_ID, 0.914_373_490_123_360_6),
    (SODIUM_CHLORIDE_ID, 0.085_626_509_876_639_56),
];

/// The sulphate brine's reference composition, its eutectic's 17.30 wt% `MgSO4` converted as
/// [`BRINE_NACL_REFERENCE`] is, with `MgSO4`'s 120.361 g mol⁻¹: H2O 0.969 640 and `MgSO4`
/// 0.030 360.
const BRINE_MGSO4_REFERENCE: [(SubstanceId, f64); 2] = [
    (WATER_ID, 0.969_640_147_202_757_8),
    (MAGNESIUM_SULPHATE_ID, 0.030_359_852_797_242_248),
];

/// Sodium chloride in water (decision-r11-palette-liquids §1.8's brine row). Laliberté and Cooper
/// 2004's density coefficients are their supporting information's (`je0498659_si_003.zip`,
/// SHA-256 c9f1c650…, `NaCl.xls`, sheet 'Density', cells C656–C664, "Curve fit for the model"),
/// which the form reproduces over the workbook's 637 rows to 2.1 × 10⁻¹⁶: fitted over 0–140 °C
/// and to w = 0.2603 (630 points, standard residual 0.33 kg m⁻³). Laliberté 2007's viscosity
/// coefficients are its corrected supporting information's (`je0604075_si_003.zip`, SHA-256
/// 37ef670f…, `NaCl.xls`, 'Viscosity', C581–C590; unchanged since first published and in the 2009
/// set): fitted over 5–154 °C and to w = 0.2644 (552 points, standard residual 0.0045 mPa s). The
/// salt's specific refraction, 1.582 06 × 10⁻⁴ m³ kg⁻¹ at 589.3 nm relative to vacuum (±0.3% over
/// 5–20 wt%), is the mean over the International Critical Tables vol. VII (1930), p. 73, Table 1,
/// entry 156, authority (10)'s `n_D` at 25 °C with its densities at 5.3562, 5.4131 and 14.344 wt%,
/// their air-relative indices taken to vacuum with standard air's index (Ciddor 1996, as
/// `betasw_ZHH2009.m` codes it; brines.json's finding 1). Every figure is the lane's research,
/// `.git/rm45-scratch/p14-liquids/step3/brines/brines.json`.
const NACL_IN_WATER: AqueousElectrolyte = AqueousElectrolyte {
    solvent: WATER_ID,
    solute: SODIUM_CHLORIDE_ID,
    density: LaliberteCooperDensity {
        coefficients: [
            -0.004_328_898_508_227_606,
            0.064_713_889_611_520_23,
            1.016_610_631_357_016_7,
            0.014_624_371_895_388_913,
            3_315.634_221_133_296_7,
        ],
        valid_from: Kelvin::new(273.15),
        valid_to: Kelvin::new(413.15),
        highest_mass_fraction: 0.260_310_803_875_142_66,
    },
    viscosity: LaliberteViscosity {
        coefficients: [
            16.221_788_633_395_988,
            1.322_930_867_700_113,
            1.484_859_850_104_306_7,
            0.007_469_125_596_573_769,
            30.780_200_754_057_528,
            2.058_268_523_225_583,
        ],
        valid_from: Kelvin::new(278.15),
        valid_to: Kelvin::new(427.15),
        highest_mass_fraction: 0.264_388_084_411_621_1,
    },
    specific_refraction_m3_per_kg: 1.582_06e-4,
    dispersion: &SALT_REFRACTION_DISPERSION,
    absorption: SoluteAbsorption::Transparent,
};

/// Magnesium sulphate in water (decision-r11-palette-liquids §1.8's brine row). Laliberté and
/// Cooper 2004's density coefficients are their supporting information's (`je0498659_si_003.zip`,
/// SHA-256 c9f1c650…, `MgSO4.xls`, 'Density', C352–C360), reproducing the workbook's 348 rows
/// exactly: fitted over 0–125 °C and to w = 0.26 (331 points, standard residual 0.75 kg m⁻³).
/// Laliberté 2007's viscosity coefficients are the corrected set (Laliberté 2007, "Additions and
/// Corrections", J. Chem. Eng. Data 52, 1507, doi:10.1021/je700232s, its text unread; its
/// supporting information's `MgSO4.xls`, 'Viscosity', C195–C204, SHA-256 dfeb7e9a…): fitted over
/// 15–150 °C and to w = 0.303 (180 points, standard residual 0.0127 mPa s). The salt's specific
/// refraction, 1.162 65 × 10⁻⁴ m³ kg⁻¹ at 589.3 nm relative to vacuum (±2%), is from the
/// International Critical Tables vol. VII (1930), p. 71, Table 1, entry 119 (`MgSO4`·7H2O,
/// Abbe–Zeiss), authority (14): `n_D` = 1.3450 at 20 °C and 1 N (0.5 mol L⁻¹), with Laliberté and
/// Cooper's density, taken to vacuum as [`NACL_IN_WATER`]'s is, and extrapolated untested from 5.7
/// wt% to the eutectic's 17.3 (n about ±0.0009). No measurement of the solutions' visible
/// absorption was read. Every figure is the lane's research, brines.json.
const MGSO4_IN_WATER: AqueousElectrolyte = AqueousElectrolyte {
    solvent: WATER_ID,
    solute: MAGNESIUM_SULPHATE_ID,
    density: LaliberteCooperDensity {
        coefficients: [
            3.941_189_794_689_959e-7,
            1.442_453_616_432_747_3e-6,
            -0.053_722_038_681_468_72,
            0.002_061_978_091_795_318,
            4_563.249_748_952_078,
        ],
        valid_from: Kelvin::new(273.15),
        valid_to: Kelvin::new(398.15),
        highest_mass_fraction: 0.26,
    },
    viscosity: LaliberteViscosity {
        coefficients: [
            72.268_883_369_539_48,
            2.223_750_077_609_783,
            6.603_702_409_691_118,
            0.007_900_413_679_183_978,
            3_340.054_544_070_852,
            6.130_387_676_863_941,
        ],
        valid_from: Kelvin::new(288.15),
        valid_to: Kelvin::new(423.15),
        highest_mass_fraction: 0.302_998_542_785_644_53,
    },
    specific_refraction_m3_per_kg: 1.162_65e-4,
    dispersion: &SALT_REFRACTION_DISPERSION,
    absorption: SoluteAbsorption::NotModelled,
};

/// What the chloride and sulphate brines' records lack, and why.
const BRINE_ABSENT: Source = Source {
    covers: "σ, b",
    citation: "absent. The surface tension: Dutcher, Wexler and Clegg 2010 (J. Phys. Chem. A 114, \
               12216, doi:10.1021/jp105191z), the source decision-r11-palette-liquids §1.8 names, \
               is closed, with no supporting information and no open transcription of its \
               coefficients found, and a reduction of its authors' E-AIM calculator would be an \
               offline fit outside hyperion-fit (the lane's research made one, recorded, not \
               used). The scattering: the solution's κ_T is not sourced (Rogers and Pitzer 1982, \
               J. Phys. Chem. Ref. Data 11, 15, and Archer 1992, 21, 793, for NaCl, are leads not \
               transcribed; Millero, Ward, Lepple and Hoff 1974, J. Phys. Chem. 78, 1636, for \
               MgSO4, closed)",
    basis: Basis::Adopted,
};

/// Where the chloride and sulphate brines' water comes from, and its range.
const BRINE_WATER: Source = Source {
    covers: "water, range",
    citation: "the sim's own water in place of Laliberté's at every state, so that a composition \
               of water alone is water's record (decision-p14-t49h-step3-composition §4 and §5): \
               its density T49.b's saturated-liquid ancillary and its index R08's file carried, \
               below 273.16 K by decision-p14-t49b-condensed §2.5's line at the triple point's \
               slope (998.34 kg m⁻³ at 251.9 K, where Laliberté and Cooper's water, Kell's \
               polynomial, gives 992.69, which raises the brine there by about 6 kg m⁻³); its \
               viscosity IAPWS R12-08's at that density, and below 273.16 K, where R12-08 is not \
               evaluated, Laliberté's water equation, (t + 246) ÷ ((0.055 94 t + 5.2842) t + \
               137.37) mPa s (as thermo's Laliberte_viscosity_w writes it, citing Laliberté 2009, \
               J. Chem. Eng. Data 54, 1725; CalebBell/thermo at 2bb466e9, SHA-256 632ed681…), \
               scaled by R12-08's value over its own at 273.16 K (1.000 67) to be continuous \
               there, an extrapolation into supercooled water, unchecked. The range runs from the \
               eutectic to below water's 647.096 K (decision-p14-t49h-step3-composition §3), each \
               empirical member Derived outside its fit's",
    basis: Basis::Adopted,
};

/// The chloride brine (`brine_nacl`): water and sodium chloride ([`NACL_IN_WATER`]) at its
/// eutectic composition ([`BRINE_NACL_REFERENCE`]) or a given one, from pure water to the density
/// model's highest salt fraction, from its 251.9 K eutectic to below water's critical point.
const BRINE_NACL: LiquidColumns = LiquidColumns {
    optics: None,
    scattering: None,
    surface_tension: None,
    viscosity: None,
    supercooled_viscosity: None,
    specific_heat: None,
    constitution: LiquidConstitution::Solution(SolutionColumns {
        components: &[WATER_ID, SODIUM_CHLORIDE_ID],
        reference: &BRINE_NACL_REFERENCE,
        model: MixingModel::AqueousElectrolyte(&NACL_IN_WATER),
    }),
    sources: &[
        Source {
            covers: "composition",
            citation: "the eutectic's 23.3 wt% NaCl (the condensed columns' transition), as mole \
                       fractions with the rows' molar masses: H2O 0.914373 and NaCl 0.085627 \
                       (decision-p14-t49h-step3-composition §5, item 7); a stand-in for a body's \
                       own until P14.T51.a carries one. A given composition runs from water alone \
                       to NaCl's 0.097867, Laliberté and Cooper's highest fitted fraction, 0.2603 \
                       by mass",
            basis: Basis::Reduced,
        },
        Source {
            covers: "ρ",
            citation: "Laliberté and Cooper 2004 (J. Chem. Eng. Data 49, 1141, \
                       doi:10.1021/je0498659; closed): ρ = 1 ÷ (w_w ÷ ρ_w + w_s v_s), the salt's \
                       apparent specific volume v_s = (w + c₂ + c₃ t) ÷ ((c₀ w + c₁) exp(10⁻⁶ (t \
                       + c₄)²)), its form and NaCl's coefficients from their supporting \
                       information (je0498659_si_003.zip, SHA-256 c9f1c650…, NaCl.xls, 'Density', \
                       C656–C664), fitted over 0–140 °C to w = 0.2603, with the sim's water; never \
                       scaled to the condensed columns' value (decision-p14-t49h-step3-composition \
                       §4): 1,184.84 kg m⁻³ at 273.15 K and the reference, 0.74 above their \
                       1,184.1 (ICT's), as a test holds",
            basis: Basis::Reduced,
        },
        Source {
            covers: "n",
            citation: "additivity of specific refraction by mass, (n² − 1) ÷ (n² + 2) = ρ (w_w r_w \
                       + w_s r_s), r_w water's carried index over its density and r_s NaCl's, \
                       1.582 06 × 10⁻⁴ m³ kg⁻¹ at 589.3 nm relative to vacuum, from the \
                       International Critical Tables vol. VII (1930), p. 73, Table 1, entry 156, \
                       authority (10), at 5–14 wt% and 25 °C (archive.org \
                       international-critical-tables_1930_7, page image n91, SHA-256 58121e91…), \
                       their air-relative indices taken to vacuum with Ciddor 1996's standard \
                       air at 15 °C, though ICT's 25 °C indices are relative to room air, which \
                       moves r_s by about 0.08% at 5 wt% and 0.03% at 14 wt%, inside its ±0.3%; \
                       its dispersion a stand-in shaped on Quan and Fry 1995's seawater (Appl. \
                       Opt. 34, 3477), +4.4% at 380 nm and −1.9% at 780 nm of its 589.3 nm value: \
                       a 3.5 wt% brine's increment at 25 °C is +0.006 06 in the bin across 545 nm \
                       against seawater's +0.006 39 at S = 35, sea salt being about 78% NaCl by \
                       mass",
            basis: Basis::Adopted,
        },
        Source {
            covers: "a",
            citation: "water's by Beer's additivity at its volume fraction w_w ρ ÷ ρ_w, the salt \
                       transparent: sea salt leaves pure water's visible absorption unchanged \
                       within measurement (Röttgers, McKee and Utschig 2014, Opt. Express 22, \
                       25093, doi:10.1364/OE.22.025093, its abstract read; its salinity \
                       coefficients as WOPP v3 transcribes them, \
                       purewater_abs_coefficients_v3.txt, SHA-256 bda82058…), NaCl not separated \
                       and the measurement's S of about 40 extrapolated to the eutectic's 233 g \
                       kg⁻¹, unmeasured (about −6% of water's at 700 nm and +5% at 750 nm by a \
                       linear extrapolation of the coefficients)",
            basis: Basis::Adopted,
        },
        Source {
            covers: "μ",
            citation: "Laliberté 2007 (J. Chem. Eng. Data 52, 321, doi:10.1021/je0604075; \
                       closed): ln μ = w_w ln μ_w + w_s ln μ_s, μ_s = exp((v₁ w^v₂ + v₃) ÷ (v₄ t + \
                       1)) ÷ (v₅ w^v₆ + 1) mPa s, its form and NaCl's coefficients from its \
                       corrected supporting information (je0604075_si_003.zip, SHA-256 37ef670f…, \
                       NaCl.xls, 'Viscosity', C581–C590), fitted over 5–154 °C to w = 0.2644, with \
                       the sim's water: 3.09 mPa s at 273.15 K and the reference, an \
                       extrapolation below 5 °C that meets ICT V's 0 °C data at 2–4 mol kg⁻¹ \
                       within +0.8 to +1.6% (brines.json)",
            basis: Basis::Reduced,
        },
        BRINE_WATER,
        BRINE_ABSENT,
    ],
};

/// The sulphate brine (`brine_mgso4`): water and magnesium sulphate ([`MGSO4_IN_WATER`]) at its
/// eutectic composition ([`BRINE_MGSO4_REFERENCE`]) or a given one, from pure water to the density
/// model's highest salt fraction, from its 269.29 K eutectic to below water's critical point.
const BRINE_MGSO4: LiquidColumns = LiquidColumns {
    optics: None,
    scattering: None,
    surface_tension: None,
    viscosity: None,
    supercooled_viscosity: None,
    specific_heat: None,
    constitution: LiquidConstitution::Solution(SolutionColumns {
        components: &[WATER_ID, MAGNESIUM_SULPHATE_ID],
        reference: &BRINE_MGSO4_REFERENCE,
        model: MixingModel::AqueousElectrolyte(&MGSO4_IN_WATER),
    }),
    sources: &[
        Source {
            covers: "composition",
            citation: "the eutectic's 17.30 wt% MgSO4 (the condensed columns' transition), as mole \
                       fractions with the rows' molar masses: H2O 0.969640 and MgSO4 0.030360 \
                       (decision-p14-t49h-step3-composition §5, item 7); a stand-in for a body's \
                       own until P14.T51.a carries one. A given composition runs from water alone \
                       to MgSO4's 0.049961, Laliberté and Cooper's highest fitted fraction, 0.26 \
                       by mass",
            basis: Basis::Reduced,
        },
        Source {
            covers: "ρ",
            citation: "Laliberté and Cooper 2004 (J. Chem. Eng. Data 49, 1141, \
                       doi:10.1021/je0498659; closed), as for the chloride brine, MgSO4's \
                       coefficients from their supporting information (je0498659_si_003.zip, \
                       SHA-256 c9f1c650…, MgSO4.xls, 'Density', C352–C360), fitted over 0–125 °C \
                       to w = 0.26, with the sim's water; never scaled \
                       (decision-p14-t49h-step3-composition §4): 1,191.14 kg m⁻³ at 273.15 K and \
                       the reference, 2.16 below the condensed columns' 1,193.3 (ICT's), as a test \
                       holds",
            basis: Basis::Reduced,
        },
        Source {
            covers: "n",
            citation: "additivity of specific refraction by mass, as for the chloride brine, with \
                       MgSO4's 1.162 65 × 10⁻⁴ m³ kg⁻¹ at 589.3 nm relative to vacuum from the \
                       International Critical Tables vol. VII (1930), p. 71, Table 1, entry 119 \
                       (MgSO4·7H2O, Abbe–Zeiss), authority (14): n_D = 1.3450 at 20 °C and 0.5 mol \
                       L⁻¹, ±2%, extrapolated untested from 5.7 to 17.3 wt%; its dispersion the \
                       chloride's stand-in, the only proxy found",
            basis: Basis::Adopted,
        },
        Source {
            covers: "a",
            citation: "absent: no measurement of MgSO4 solutions' visible absorption was read \
                       (sea salt's, Röttgers, McKee and Utschig 2014, holds MgSO4 only as a minor \
                       part)",
            basis: Basis::Adopted,
        },
        Source {
            covers: "μ",
            citation: "Laliberté 2007 (J. Chem. Eng. Data 52, 321), as for the chloride brine, \
                       MgSO4's coefficients the corrected set (Laliberté 2007, \"Additions and \
                       Corrections\", J. Chem. Eng. Data 52, 1507, doi:10.1021/je700232s, its text \
                       unread; its supporting information's MgSO4.xls, 'Viscosity', C195–C204, \
                       SHA-256 dfeb7e9a…), fitted over 15–150 °C to w = 0.303, with the sim's \
                       water: 6.46 mPa s at 273.15 K and the reference, an unchecked extrapolation \
                       below 15 °C",
            basis: Basis::Reduced,
        },
        BRINE_WATER,
        BRINE_ABSENT,
    ],
};

/// Sodium perchlorate, `NaClO4`, `brine_naclo4`'s solute, which has no registry row yet
/// (decision-p14-t49h-step3-composition §1.3): 122.436 g mol⁻¹ by IUPAC 2021's abridged weights
/// (Na 22.990, Cl 35.45, O 15.999), the registry's own arithmetic, as a test holds.
const SODIUM_PERCHLORATE: RowlessSolute = RowlessSolute {
    solvent: WATER_ID,
    elements: &[(Element::Na, 1), (Element::Cl, 1), (Element::O, 4)],
    molar_mass_g_per_mol: 122.436,
};

/// Magnesium perchlorate, Mg(ClO4)2, `brine_mgclo4`'s solute, which has no registry row yet, its
/// key waiting for the key grammar's parenthesised groups (decision-p14-t49h-step3-composition §1.3
/// and §6): 223.197 g mol⁻¹ by IUPAC 2021's abridged weights (Mg 24.305, Cl 35.45, O 15.999), the
/// registry's own arithmetic, as a test holds.
const MAGNESIUM_PERCHLORATE: RowlessSolute = RowlessSolute {
    solvent: WATER_ID,
    elements: &[(Element::Mg, 1), (Element::Cl, 2), (Element::O, 8)],
    molar_mass_g_per_mol: 223.197,
};

/// What the perchlorate brines' records lack, and why.
const PERCHLORATE_ABSENT: Source = Source {
    covers: "n, a, b, σ, μ, composition",
    citation: "absent, so the record is NotModelled for its index: no open measured index of a \
               perchlorate solution was found (not in the International Critical Tables vol. VII, \
               Tables 1 or 5; leads unread: J. Am. Chem. Soc. 79, 1789, 1957; Fajans and Johnson \
               1942, J. Am. Chem. Soc. 64, 668; Lumme and Högnäs 1968); no perchlorate is among \
               Laliberté's 59, 74 and 109 solutes (2004, 2007 and 2009) or Dutcher, Wexler and \
               Clegg 2010's ions, and no absorption, compressibility or water activity was read \
               (the lane's research, brines.json). A given composition waits for the solute's row \
               (decision-p14-t49h-step3-composition §1.3). The range runs from the eutectic to \
               below water's 647.096 K, as the other brines' does (§3), so that above it the \
               record is OutOfRange before it is NotModelled",
    basis: Basis::Adopted,
};

/// The sodium perchlorate brine (`brine_naclo4`) at its eutectic, 52 wt%: its condensed columns'
/// stated density and no index, so its record is `NotModelled` for the index from its eutectic to
/// below water's critical point (and `OutOfRange` outside), and no components until its solute
/// ([`SODIUM_PERCHLORATE`]) has a row.
const BRINE_NACLO4: LiquidColumns = LiquidColumns {
    optics: None,
    scattering: None,
    surface_tension: None,
    viscosity: None,
    supercooled_viscosity: None,
    specific_heat: None,
    constitution: LiquidConstitution::AwaitingSolute(&SODIUM_PERCHLORATE),
    sources: &[
        Source {
            covers: "ρ",
            citation: "the condensed columns' 1,445 ± 10 kg m⁻³ at 291.15 K, held at every \
                       temperature and Derived away from it: no temperature law was found \
                       (Hnedkovsky and Hefter 2021, J. Chem. Eng. Data, \
                       doi:10.1021/acs.jced.1c00495, NaClO4(aq) at 293–343 K to near saturation, \
                       closed, would test it); viscosity leads unread: Miller and Doran 1956, J. \
                       Phys. Chem. 60, 186; J. Phys. Chem. 63, 742 (1959); Janz et al. 1970, J. \
                       Phys. Chem. 74, 1285",
            basis: Basis::Fact,
        },
        PERCHLORATE_ABSENT,
    ],
};

/// The magnesium perchlorate brine (`brine_mgclo4`) at its eutectic, 44.0 wt%: as
/// [`BRINE_NACLO4`], its solute [`MAGNESIUM_PERCHLORATE`].
const BRINE_MGCLO4: LiquidColumns = LiquidColumns {
    optics: None,
    scattering: None,
    surface_tension: None,
    viscosity: None,
    supercooled_viscosity: None,
    specific_heat: None,
    constitution: LiquidConstitution::AwaitingSolute(&MAGNESIUM_PERCHLORATE),
    sources: &[
        Source {
            covers: "ρ",
            citation: "the condensed columns' 1,394 ± 15 kg m⁻³ at 298.15 K, held at every \
                       temperature and Derived away from it: no temperature law was found beyond \
                       Lumme 1970 (Acta Chem. Scand. 24, 1727, 25 °C to 2 mol L⁻¹), which the \
                       condensed columns extrapolate; viscosity lead unread: Toner et al. 2014, \
                       Icarus 233, 36",
            basis: Basis::Fact,
        },
        PERCHLORATE_ABSENT,
    ],
};

/// Ammonia–water's reference composition, its eutectic's: the condensed columns' 0.321 mass
/// fraction of NH3 (`rows/condensed.rs`'s `AMMONIA_WATER`, ammonia dihydrate's stoichiometry) as
/// mole fractions in registry order with the rows' molar masses, 18.015 and 17.031 g mol⁻¹,
/// converted as [`BRINE_NACL_REFERENCE`] is: H2O 0.666 636 and NH3 0.333 364
/// (decision-p14-t49h-step3-composition §5, item 7), held as that arithmetic's own values, which a
/// test recomputes. It is the only composition the row evaluates ([`MixingModel::ReferenceOnly`]).
const AMMONIA_WATER_REFERENCE: [(SubstanceId, f64); 2] = [
    (WATER_ID, 0.666_636_286_535_710_4),
    (AMMONIA_ID, 0.333_363_713_464_289_6),
];

/// The ammonia–water liquid (`nh3_h2o`) at its eutectic composition ([`AMMONIA_WATER_REFERENCE`])
/// alone ([`MixingModel::ReferenceOnly`]): its condensed columns' stated density, with the index
/// and surface tension of its components (water and ammonia, each below its triple point by
/// decision-p14-t49b-condensed §2.5) mixed at that density, from its 176.15 K eutectic to below
/// ammonia's 405.56 K critical point.
const AMMONIA_WATER: LiquidColumns = LiquidColumns {
    optics: None,
    scattering: None,
    surface_tension: None,
    viscosity: None,
    supercooled_viscosity: None,
    specific_heat: None,
    constitution: LiquidConstitution::Solution(SolutionColumns {
        components: &[WATER_ID, AMMONIA_ID],
        reference: &AMMONIA_WATER_REFERENCE,
        model: MixingModel::ReferenceOnly,
    }),
    sources: &[
        Source {
            covers: "composition",
            citation: "the eutectic's 0.321 mass fraction of NH3 (the condensed columns' \
                       transition: ammonia dihydrate's stoichiometry, an assumption there), as \
                       mole fractions with the rows' molar masses: H2O 0.666636 and NH3 0.333364 \
                       (decision-p14-t49h-step3-composition §5, item 7), the only composition \
                       evaluated, within 10⁻⁹: Croft, Lunine and Kargel 1988 (Icarus 73, 279, \
                       doi:10.1016/0019-1035(88)90098-X) and Kargel, Croft, Lunine and Lewis 1991 \
                       (Icarus 89, 93, doi:10.1016/0019-1035(91)90090-G), whose composition laws \
                       decision-r11-palette-liquids §1.8 names, are closed and unread",
            basis: Basis::Reduced,
        },
        Source {
            covers: "ρ",
            citation: "the condensed columns' 946 kg m⁻³ at 176.15 K (Hammond, Parmentier and \
                       Barr 2018, arXiv:1811.11257, §1, after Croft et al. 1988; SHA-256 \
                       7d1bc606…), held at every temperature and Derived away from 176.15 K: no \
                       law in temperature at fixed composition was read (Hammond et al.'s Eq. 12 \
                       fits Croft et al.'s densities along the liquidus and gives 957 at the \
                       eutectic); ideal volume additivity of the components would give 902, 4.6% \
                       low, the mixing contracting strongly (the lane's research, \
                       rm45-scratch/p14-liquids/step3/mixtures/mixtures.json)",
            basis: Basis::Fact,
        },
        Source {
            covers: "n",
            citation: "Lorentz–Lorenz from the components' molar refractions at that density's \
                       molar volume, L = Σ xᵢ Rᵢ ÷ V_m, each Rᵢ = Lᵢ Vᵢ water's and ammonia's \
                       own, from their carried indices and densities at the temperature \
                       (decision-p14-t49b-condensed §2.5 below their triple points): 1.3814 in \
                       the bin across 545 nm at 176.15 K; no measured index of concentrated \
                       aqueous ammonia was read (ICT VII, p. 66, holds only 0.5–1 N)",
            basis: Basis::Adopted,
        },
        Source {
            covers: "σ",
            citation: "the Macleod–Sugden parachor as for Titan's seas, at that density's molar \
                       volume, each component's tension and density at the temperature by \
                       decision-p14-t49b-condensed §2.5 (water's R1-76 form and line 97 K below \
                       its triple point): 83.8 mN m⁻¹ at 176.15 K, where the components' \
                       parachors at their own triple points would give 76.8 (mixtures.json); no \
                       measured tension of aqueous ammonia was read (King, Hall and Ware 1930 and \
                       Rice 1928 are closed), and ammonia's enrichment at the surface probably \
                       makes a parachor of the bulk composition high",
            basis: Basis::Adopted,
        },
        Source {
            covers: "μ, a, b, range",
            citation: "absent: Kargel et al. 1991's measured viscosity is unread, and Hammond et \
                       al.'s Eq. 13 is a fit along the liquidus, 2.75 Pa s at the eutectic, where \
                       their text says about 5 Pa s; ammonia's absorption is absent \
                       (decision-r08-t5g §5); the solution's compressibility is not sourced, so no \
                       scattering. The range runs from the eutectic to below NH3's 405.56 K \
                       (decision-p14-t49h-step3-composition §3), every member Derived; at one \
                       atmosphere the 32 wt% liquid boils near room temperature",
            basis: Basis::Adopted,
        },
    ],
};
