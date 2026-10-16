//! The rows' liquid columns (P14.T49.h): each pure liquid with an R08 file, its index and
//! absorption from that file; water's scattering, surface tension and viscosity from its IAPWS
//! releases (the first step); and methane's, nitrogen's, ethane's and ammonia's from their
//! reference equations of state (`rows/equations.rs`), Mulero, Cachadiña and Parra 2012's
//! surface tensions and each one's reference viscosity correlation (the second step), with carbon
//! dioxide's columns but its index, which waits on R08.T5.k's liquid file
//! (decision-p14-t49h-gas-index §1.3), so that row answers `NotALiquid` until then.
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

use hyperion_surface::field::LiquidBasis;
use hyperion_surface::substance_key::SubstanceKey;

use super::super::helmholtz::{GaussianTerm, HelmholtzResidual, NonAnalyticTerm, PowerTerm};
use super::super::liquid::{
    CorrelationLength, CriticalRegion, Friend1991Viscosity, Huber2024Viscosity, Iapws2008Viscosity,
    LaeseckeMuzny2017Viscosity, LiquidColumns, LiquidOptics, LiquidScattering,
    Monogenidou2018Viscosity, Sotiriadou2026Viscosity, SupercooledViscosity,
    SurfaceTensionEquation, ViscosityCorrelation, ViscosityRange,
};
use super::super::{Basis, Source};
use super::equations;
use crate::tables::liquid_optics as bins;
use crate::units::{Kelvin, KilogramsPerCubicMetre, Pascals};

/// Every row with liquid columns, in registry order, by key. Carbon dioxide's columns
/// ([`CARBON_DIOXIDE`]) join when its index does, with the eight other rows R08.T5.k gives a liquid
/// file (decision-p14-t49h-gas-index §1.2–§1.3: P14.T49.h's step 2b); until then those nine answer
/// `NotALiquid`.
pub(in crate::substance) const LIQUIDS: [(SubstanceKey, LiquidColumns); 6] = [
    (SubstanceKey::new_const("H2O"), WATER),
    (SubstanceKey::new_const("CH4"), METHANE),
    (SubstanceKey::new_const("NH3"), AMMONIA),
    (SubstanceKey::new_const("N2"), NITROGEN),
    (SubstanceKey::new_const("C2H6"), ETHANE),
    (SubstanceKey::new_const("H2SO4"), SULPHURIC_ACID),
];

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
    sources: &[
        Source {
            covers: "n, a",
            citation: "R08's `methane-liquid.json`: Martonchik and Orton 1994 (Appl. Opt. 33, \
                       8306), the liquid at 90 K, through refractiveindex.info (CC0): its visible \
                       k Table 2's local maxima only, nothing measured shortward of 552.5 nm, the \
                       400 nm zero an anchor, and the 90 K values the 111 K values scaled by \
                       density, 25% (decision-p14-t49h-liquids §2.1); reduced to the bake bins by \
                       `tables::liquid_optics`",
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

/// Liquid carbon dioxide, for any world above its triple point's 518 kPa: its scattering by
/// ES–Cabannes from Span and Wagner 1996's `κ_T`, its surface tension Mulero et al.'s and its
/// viscosity Laesecke and Muzny 2017's. Its index waits on R08.T5.k's `carbon-dioxide-liquid.json`
/// (decision-p14-t49h-gas-index §1.2), so these columns are not yet in [`LIQUIDS`] and the row
/// answers `NotALiquid` until P14.T49.h's step 2b gives it its optics; its absorption is not
/// modelled.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "carbon dioxide's columns join LIQUIDS with its index, P14.T49.h's step 2b \
                  (decision-p14-t49h-gas-index §1.3); its tests read them now"
    )
)]
pub(in crate::substance) const CARBON_DIOXIDE: LiquidColumns = LiquidColumns {
    optics: None,
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
    sources: &[
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
    supercooled_viscosity: None,
    sources: &[Source {
        covers: "n, a",
        citation: "R08's `sulphuric-acid-75.json`: Palmer and Williams 1975 (Appl. Opt. 14, 208), \
                   the 75 wt% acid at 300 K, through Oxford's ARIA; in the visible n at five \
                   wavelengths and k held at its shortest measured value, an upper bound; reduced \
                   to the bake bins by `tables::liquid_optics`",
        basis: Basis::Reduced,
    }],
};
