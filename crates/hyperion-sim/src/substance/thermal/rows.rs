//! The rows' thermal columns (P14.T49.j): each row's solid specific heat c(T) and thermal
//! conductivity k(T), with their sources, in registry order.
//!
//! Every figure is a source's, read by this task's research (2026-10-16) and checked against it,
//! or reduced from read figures by stated arithmetic. A figure whose source was not reached says
//! so ("not re-read"). A row that takes another's figure is a stand-in, the same constant as the
//! one it borrows, its source saying why. NIST's WebBook and JANAF serve only as asserted
//! cross-checks (decision-composition §5); U.S. Geological Survey and NBS publications are U.S.
//! public domain, and each use is described in its source. Molar heat capacities are converted
//! with the formula weight their source prints; a conductivity in mcal cm⁻¹ s⁻¹ °C⁻¹ ("CU") is
//! 0.4184 W m⁻¹ K⁻¹ ([`CU_W_PER_M_K`]).
//!
//! A dense silicate's conductivity was read only near room temperature, so its column is held
//! constant over 0–100 °C, as Robertson 1988 finds a plagioclase's roughly constant there while
//! other minerals' fall 10–30 per cent; below it a crystal's rises and a glass's falls. A heat
//! capacity is held below its lowest read point too, which overstates it in the cold: the lunar
//! samples' and the meteorites' series begin at 70–90 K, where c is a fifth to a third of its 273 K
//! value, and enstatite's c at 40 K is a tenth of its 100 K value.
//!
//! # Stand-ins
//!
//! - `Mg2SiO4`: conductivity, a dunite's: no single crystal's was read.
//! - `mars_dust`: both columns, `basalt_fe`'s, as its transition is.
//! - `feldspathic`: conductivity, the anorthosite rock's: no conductivity of the mixed highland
//!   crust was read.
//! - `basalt`: heat capacity, the mare basalt 15555's: no terrestrial basalt's below 298 K was
//!   read.
//! - `basalt_fe`: conductivity, the terrestrial basalt's: no lunar basalt's without cracks was
//!   read.
//! - `komatiite`: conductivity, the ultramafic rock's: no komatiite's was read.
//! - `basalt_reduced`: both columns, the basalts': Mercury's lid has no measured figure.
//! - `granite`: both columns: its quartz's heat capacity below 298 K was not read, so the alkali
//!   feldspars' stand in; its conductivity is the felsic rocks' typical figure.
//! - `ultramafic`: conductivity, a dunite's: no harzburgite's was read.
//! - `ferric`: both columns, `basalt_fe`'s, as its grain density is.
//! - `phyllosilicate`: both columns, `chondrite_cm`'s: serpentine's heat capacity below 298 K and
//!   its conductivity's temperature dependence were not read.
//! - `chondrite_ci`: conductivity, the carbonaceous chondrites': no CI chondrite's was measured.
//! - `chondrite_ll`: conductivity, the ordinary chondrites': no LL chondrite's was read.
//! - `cometary`: both columns: Winter and Saari's silicate heat capacity, which the Rosetta
//!   literature adopts for 67P's dust, and the carbonaceous chondrites' conductivity.
//!
//! # Not modelled
//!
//! The rows with condensed columns and no thermal columns, in registry order, each with its reason
//! (a test holds this list to the table):
//!
//! - `NH3`: no ammonia ice's heat capacity or conductivity was read.
//! - `Kr`: no krypton ice's figures were read.
//! - `Xe`: no xenon ice's figures were read.
//! - `N2O`: no nitrous oxide ice's figures were read.
//! - `SO2`: no sulphur dioxide frost's figures were read.
//! - `H2S`: no hydrogen sulphide ice's figures were read.
//! - `C2H6`: no ethane ice's figures were read.
//! - `CH3OH`: no methanol ice's figures were read.
//! - `Fe`: iron's heat capacity below room temperature was not read (Gundlach and Blum 2013 give an
//!   iron meteorite's conductivity, 12.63 + 0.051 T W m⁻¹ K⁻¹).
//! - `NH4SH`: a condensate aloft; no figures were read.
//! - `H2SO4`: a liquid aerosol, its solid's figures not read.
//! - `S8`: no sulphur's figures were read.
//! - `KCl`: no sylvite's figures were read.
//! - `NaCl`: no halite's figures were read.
//! - `ZnS`: a condensate aloft; no figures were read.
//! - `Na2S`: a condensate aloft; no figures were read.
//! - `MnS`: a condensate aloft; no figures were read.
//! - `Cr`: a condensate aloft; no figures were read.
//! - `FeS`: no troilite's figures were read.
//! - `SiO2`: quartz's heat capacity below 298 K was not reached (its conductivity, 6.15–10.2 W
//!   m⁻¹ K⁻¹ across and along its axis, was).
//! - `Al2O3`: no corundum's figures were read.
//! - `CaTiO3`: no perovskite's figures were read.
//! - `TiO2`: no rutile's figures were read.
//! - `graphite`: its heat capacity below 298 K was not reached (`DeSorbo` and Tyler 1953 closed);
//!   its conductivity was (NBS SP 260-89).
//! - `tholin`: no figures were read.
//! - `soot`: no figures were read.
//! - `chromophore`: never: a haze, which forms no surface.
//! - `ice_giant_haze`: never: a haze, which forms no surface.
//! - `acid_uv_absorber`: an aerosol of Venus's clouds; no figures were read.
//! - `brine_nacl`: never: a liquid, whose frozen form's solids are its components'.
//! - `brine_mgso4`: never: a liquid, whose frozen form's solids are its components'.
//! - `brine_naclo4`: never: a liquid, whose frozen form's solids are its components'.
//! - `brine_mgclo4`: never: a liquid, whose frozen form's solids are its components'.
//! - `nh3_h2o`: never: a liquid, whose frozen form's solids are its components'.
//! - `ch4_c2h6_n2`: never: a liquid, whose frozen form's solids are its components'.
//! - `lava_glass`: a basaltic glass's own figures were not read.
//! - `ca_al_residue`: no hibonite's or corundum's figures were read.
//! - `iron_nickel`: kamacite's heat capacity below room temperature was not read.
//! - `SiC`: no moissanite's figures were read.
//! - `MgSO4`: no anhydrous magnesium sulphate's figures were read.
//! - `Na2CO3`: its heat capacity below 298 K was not reached (Waterfield et al. 1968), nor its
//!   conductivity; USGS Bulletin 2131 gives no heat capacity for it.
//! - `CaCO3`: calcite's figures were not read.
//! - `carbonaceous`: no organic matter's heat capacity below 300 K was read; coal's at 30–100 °C
//!   would overstate it twofold at 100 K.
//!
//! Never modelled, and so not listed: a gas-only row, which has no condensed columns.

use hyperion_surface::substance_key::SubstanceKey;

use super::super::{Basis, Source};
use super::{Provenance, ThermalColumn, ThermalColumns, ThermalForm, ThermalPiece, source};
use crate::units::Kelvin;

/// One CU, a conductivity of 10⁻³ cal cm⁻¹ s⁻¹ °C⁻¹, in W m⁻¹ K⁻¹: "1 CU = 10⁻³ cal/cm sec °C =
/// 0.4184 W/m·K" (Robertson 1988, USGS Open-File Report 88-441, p. 3).
const CU_W_PER_M_K: f64 = 0.4184;

/// A piece of `form` over `low_k`–`high_k` kelvin, for a row's only solid.
#[must_use]
const fn only(low_k: f64, high_k: f64, form: ThermalForm) -> ThermalPiece {
    ThermalPiece {
        phase: "",
        low: Kelvin::new(low_k),
        high: Kelvin::new(high_k),
        form,
    }
}

/// The row's own column of `pieces`.
#[must_use]
const fn own(pieces: &'static [ThermalPiece]) -> ThermalColumn {
    ThermalColumn {
        pieces,
        provenance: Provenance::Own,
    }
}

/// Another row's or a class's column of `pieces`, standing in for the row's own.
#[must_use]
const fn stand_in(pieces: &'static [ThermalPiece]) -> ThermalColumn {
    ThermalColumn {
        pieces,
        provenance: Provenance::StandIn,
    }
}

/// A conductivity of `w_per_m_k` W m⁻¹ K⁻¹ held over 0–100 °C, where a dense rock's
/// room-temperature figure stands (Robertson 1988, p. 41).
#[must_use]
const fn room_temperature(w_per_m_k: f64) -> [ThermalPiece; 1] {
    [only(273.15, 373.15, ThermalForm::Constant(w_per_m_k))]
}

// ---------------------------------------------------------------------------------------------
// Water ice Ih.

/// Ice Ih's specific heat: IAPWS R10-06(2009)'s Gibbs function at one atmosphere, valid from 0 K
/// to the melting point (its §5), here from 10 K, below every surface, to 273.15 K.
const ICE_IH_HEAT_CAPACITY: [ThermalPiece; 1] = [phase("ice Ih", 10.0, 273.15, ThermalForm::IceIh)];

/// Ice Ih's thermal conductivity's a, W m⁻¹: 567, Klinger's k = 567 ÷ T.
pub(super) const ICE_IH_CONDUCTIVITY_A_W_PER_M: f64 = 567.0;

/// Ice Ih's thermal conductivity: 567 ÷ T, "T > 25 K", to the melting point.
const ICE_IH_CONDUCTIVITY: [ThermalPiece; 1] = [phase(
    "ice Ih",
    25.0,
    273.15,
    ThermalForm::Reciprocal {
        a: ICE_IH_CONDUCTIVITY_A_W_PER_M,
        b: 0.0,
    },
)];

/// Water ice Ih.
const WATER: ThermalColumns = ThermalColumns {
    heat_capacity: own(&ICE_IH_HEAT_CAPACITY),
    conductivity: own(&ICE_IH_CONDUCTIVITY),
    sources: &[
        source(
            "heat capacity",
            "IAPWS R10-06(2009), the Revised Release on the Equation of State 2006 for H2O Ice Ih \
             (Feistel and Wagner 2006, J. Phys. Chem. Ref. Data 35, 1021): c_p = −T ∂²g ÷ ∂T² of \
             its Gibbs function (Table 3, eq. 6; Table 4's g_TT) with its Table 2's constants at \
             one atmosphere, valid over 0–273.16 K within 2 per cent (Table 5), reproducing its \
             Table 6's check values to their printed digits; read",
            Basis::Fact,
        ),
        source(
            "conductivity",
            "k = 567 ÷ T W m⁻¹ K⁻¹ for crystalline ice, \"567/T (T > 25 K)\" (Huebner et al. 2006, \
             ISSI Scientific Report SR-004, Table 2.5, p. 17, after Klinger 1975, and App. B.3, \
             eq. B-10, \"as determined by Klinger (1980)\"; Ferrari and Lucas 2016, A&A 588, A133, \
             §2.1), Klinger 1980 (Science 209, 271) not re-read. Measurements agree within their \
             errors: Klinger 1975's own crystals (J. Glaciol. 14, 517, \"lower than 12 %\") at \
             63–100 K within 2 per cent, and Andersson and Suga 1994 at 82–252 K within 4 per \
             cent, both as digitised by Wolfenbarger et al. 2021 (Data in Brief, CC BY 4.0); \
             Slack 1980's best estimate (Phys. Rev. B 22, 3065, not re-read) is about 14 per cent \
             above at 40–200 K, and Carnahan et al. 2021's fit over 30–273 K is 612 ÷ T (EPSL \
             563, arXiv:2011.12502, supplement §1)",
            Basis::Fact,
        ),
    ],
};

// ---------------------------------------------------------------------------------------------
// The lunar samples (Hemingway, Robie and Wilson 1973, Proc. Lunar Sci. Conf. 4, 2481).

/// The Apollo 16 highland soil 60601,31's smoothed specific heats, "Lunar Highlands", Descartes
/// (Hemingway, Robie and Wilson 1973, Table 6), (K, J kg⁻¹ K⁻¹), their J g⁻¹ K⁻¹ times 1,000, at
/// ten of its fifteen temperatures from 90 to 360 K.
const HIGHLAND_SOIL_60601: [(f64, f64); 10] = [
    (90.0, 241.8),
    (100.0, 274.1),
    (120.0, 338.5),
    (140.0, 400.8),
    (160.0, 458.6),
    (200.0, 562.3),
    (240.0, 655.6),
    (280.0, 740.6),
    (320.0, 817.1),
    (360.0, 887.0),
];

/// The highland soil's specific heat, its read points from 90 to 360 K.
const HIGHLAND_HEAT_CAPACITY: [ThermalPiece; 1] =
    [only(90.0, 360.0, ThermalForm::Points(&HIGHLAND_SOIL_60601))];

/// The Apollo 15 mare basalt 15555,159's measured specific heats, Hadley–Apennine (Hemingway,
/// Robie and Wilson 1973, Table 4), (K, J kg⁻¹ K⁻¹), fourteen of its twenty-eight points from 83.56
/// to 363.53 K. Table 4's points, not Table 6's smoothed values, which stand 12 per cent above them
/// at 100 K and 2 per cent at 300 K.
const MARE_BASALT_15555: [(f64, f64); 14] = [
    (83.56, 208.8),
    (103.71, 271.5),
    (124.82, 338.9),
    (154.35, 427.6),
    (176.18, 487.9),
    (197.83, 545.6),
    (218.95, 598.3),
    (240.98, 648.1),
    (260.22, 687.4),
    (281.80, 733.0),
    (303.77, 770.3),
    (323.97, 807.1),
    (343.36, 839.7),
    (363.53, 872.8),
];

/// The mare basalt's specific heat, its read points from 83.56 to 363.53 K.
const MARE_BASALT_HEAT_CAPACITY: [ThermalPiece; 1] =
    [only(83.56, 363.53, ThermalForm::Points(&MARE_BASALT_15555))];

/// The highland soil's specific heat's source.
const HIGHLAND_SOIL: Source = source(
    "heat capacity",
    "the Apollo 16 highland soil 60601,31, \"Lunar Highlands\" (Descartes), by adiabatic \
     calorimetry: Hemingway, Robie and Wilson 1973, Proc. Lunar Sci. Conf. 4, 2481, Table 6's \
     smoothed values at ten of its temperatures from 90 to 360 K, read in the scan; within 4 per \
     cent of Hayne et al. 2017's lunar polynomial (JGR Planets 122, 2371, Table A1) from 100 to \
     350 K",
    Basis::Fact,
);

/// The mare basalt's specific heat's source.
const MARE_BASALT: Source = source(
    "heat capacity",
    "the Apollo 15 mare basalt 15555,159 (Hadley–Apennine), by adiabatic calorimetry: Hemingway, \
     Robie and Wilson 1973, Proc. Lunar Sci. Conf. 4, 2481, Table 4's measured points, fourteen \
     of twenty-eight from 83.56 to 363.53 K, read in the scan; its Table 6 smoothed column stands \
     12 per cent above them at 100 K, which the measurements do not support",
    Basis::Fact,
);

// ---------------------------------------------------------------------------------------------
// Minerals, by low-temperature adiabatic calorimetry, and the modes summed over them.

/// The temperatures, K, at which the minerals' smoothed molar heat capacities are read.
const MINERAL_TEMPERATURES_K: [f64; 12] = [
    20.0, 40.0, 60.0, 70.0, 80.0, 100.0, 150.0, 200.0, 250.0, 273.15, 298.15, 350.0,
];

/// A mineral's smoothed molar heat capacities, J mol⁻¹ K⁻¹, at [`MINERAL_TEMPERATURES_K`], and its
/// formula weight, g mol⁻¹, as its source prints them.
#[derive(Debug, Clone, Copy)]
struct Mineral {
    molar_j_per_mol_k: [f64; 12],
    formula_weight_g_per_mol: f64,
}

/// Forsterite, `Mg2SiO4` (Robie, Hemingway and Takei 1982, Am. Mineral. 67, 470, Table 5; formula
/// weight 140.694).
const FORSTERITE: Mineral = Mineral {
    molar_j_per_mol_k: [
        0.275, 2.843, 9.804, 14.69, 20.20, 32.26, 61.81, 85.95, 104.6, 111.8, 118.6, 130.2,
    ],
    formula_weight_g_per_mol: 140.694,
};

/// Fayalite, `Fe2SiO4` (Robie, Finch and Hemingway 1982, Am. Mineral. 67, 463, Table 2; formula
/// weight 203.778).
const FAYALITE: Mineral = Mineral {
    molar_j_per_mol_k: [
        7.892, 20.53, 51.25, 39.40, 43.16, 54.39, 80.95, 102.2, 118.9, 125.5, 131.9, 143.1,
    ],
    formula_weight_g_per_mol: 203.778,
};

/// Synthetic enstatite, `MgSiO3` (Krupka, Robie, Hemingway, Kerrick and Ito 1985, Am. Mineral. 70,
/// 249, Table 12; formula weight 100.389).
const ENSTATITE: Mineral = Mineral {
    molar_j_per_mol_k: [
        0.252, 2.471, 7.729, 11.21, 15.04, 23.21, 42.99, 59.28, 72.21, 77.24, 82.06, 90.35,
    ],
    formula_weight_g_per_mol: 100.389,
};

/// Diopside, CaMg(SiO3)2 (Krupka et al. 1985, Table 10; formula weight 216.553).
const DIOPSIDE: Mineral = Mineral {
    molar_j_per_mol_k: [
        0.755, 6.799, 19.29, 27.02, 35.28, 52.40, 92.11, 123.7, 148.4, 157.9, 166.8, 182.7,
    ],
    formula_weight_g_per_mol: 216.553,
};

/// Anorthite, `CaAl2Si2O8` (Robie, Hemingway and Wilson 1978, Am. Mineral. 63, 109, Table 9;
/// formula weight 278.210, the paper's for `CaAl2Si2O8`).
const ANORTHITE: Mineral = Mineral {
    molar_j_per_mol_k: [
        2.802, 16.98, 36.85, 47.19, 57.41, 77.29, 121.4, 157.8, 187.7, 199.8, 211.4, 232.2,
    ],
    formula_weight_g_per_mol: 278.210,
};

/// `CaAl2Si2O8` glass (Robie, Hemingway and Wilson 1978, Table 8; formula weight 278.210).
const ANORTHITE_GLASS: Mineral = Mineral {
    molar_j_per_mol_k: [
        3.346, 17.43, 36.68, 46.77, 56.83, 76.46, 120.3, 157.1, 186.8, 199.1, 210.6, 231.5,
    ],
    formula_weight_g_per_mol: 278.210,
};

/// Low albite, `NaAlSi3O8` (Openshaw, Hemingway, Robie, Waldbaum and Krupka 1976, J. Res. U.S.
/// Geol. Survey 4, 195, Table 7; gram-formula weight 262.224).
const LOW_ALBITE: Mineral = Mineral {
    molar_j_per_mol_k: [
        4.905, 22.50, 43.02, 53.06, 62.86, 81.66, 122.6, 155.9, 183.1, 194.2, 205.1, 225.5,
    ],
    formula_weight_g_per_mol: 262.224,
};

/// Microcline, `KAlSi3O8` (Openshaw et al. 1976, Table 9; gram-formula weight 278.337).
const MICROCLINE: Mineral = Mineral {
    molar_j_per_mol_k: [
        4.905, 25.47, 47.63, 57.90, 67.67, 85.87, 125.1, 157.5, 183.3, 193.0, 202.4, 222.2,
    ],
    formula_weight_g_per_mol: 278.337,
};

/// The specific heats, (K, J kg⁻¹ K⁻¹) at [`MINERAL_TEMPERATURES_K`], of a mixture of `parts` by
/// mass, (mass fraction, mineral): Σ wᵢ cᵢ, each cᵢ its molar figure over its formula weight in kg,
/// summed in the given order.
#[must_use]
const fn mode(parts: &[(f64, Mineral)]) -> [(f64, f64); 12] {
    let mut points = [(0.0, 0.0); 12];
    let mut i = 0;
    while i < MINERAL_TEMPERATURES_K.len() {
        let mut sum = 0.0;
        let mut k = 0;
        while k < parts.len() {
            let (fraction, mineral) = parts[k];
            sum += fraction * mineral.molar_j_per_mol_k[i]
                / (mineral.formula_weight_g_per_mol / 1000.0);
            k += 1;
        }
        points[i] = (MINERAL_TEMPERATURES_K[i], sum);
        i += 1;
    }
    points
}

/// A mass fraction of olivine's forsterite at a molar Mg# of `mg_number`, by the formula weights.
#[must_use]
const fn forsterite_by_mass(mg_number: f64) -> f64 {
    let forsterite = mg_number * FORSTERITE.formula_weight_g_per_mol;
    forsterite / (forsterite + (1.0 - mg_number) * FAYALITE.formula_weight_g_per_mol)
}

/// The ultramafic row's mode: Hu, Ehlmann and Seager 2012's "60% Olivine, 40% Enstatite" (ApJ 752,
/// 7, Table 2), taken by mass, with olivine of Mg# 90, as the row's grain density takes it, and
/// the orthopyroxene as enstatite, its ferrosilite's heat capacity not read.
const ULTRAMAFIC_MODE: [(f64, f64); 12] = mode(&[
    (0.6 * forsterite_by_mass(0.9), FORSTERITE),
    (0.6 * (1.0 - forsterite_by_mass(0.9)), FAYALITE),
    (0.4, ENSTATITE),
]);

/// Anorthosite's plagioclase by mass: 95 per cent plagioclase by volume at 2.753 g cm⁻³ with 5 per
/// cent low-calcium pyroxene at about 3.5, as the row's grain density takes them
/// (science-composition-crusts.md §7.2).
const ANORTHOSITE_PLAGIOCLASE: f64 = 0.95 * 2.753 / (0.95 * 2.753 + 0.05 * 3.5);

/// An95 plagioclase's anorthite by mass, by the formula weights.
const AN95_ANORTHITE: f64 = 0.95 * ANORTHITE.formula_weight_g_per_mol
    / (0.95 * ANORTHITE.formula_weight_g_per_mol + 0.05 * LOW_ALBITE.formula_weight_g_per_mol);

/// Anorthosite's mode by mass: its An95 plagioclase as anorthite and low albite, and its
/// low-calcium pyroxene as enstatite.
const ANORTHOSITE_MODE: [(f64, f64); 12] = mode(&[
    (ANORTHOSITE_PLAGIOCLASE * AN95_ANORTHITE, ANORTHITE),
    (ANORTHOSITE_PLAGIOCLASE * (1.0 - AN95_ANORTHITE), LOW_ALBITE),
    (1.0 - ANORTHOSITE_PLAGIOCLASE, ENSTATITE),
]);

/// Komatiite's olivine, Fo91, the middle of its Fo90–92, by ideal molar-volume mixing of
/// forsterite's 3.219 and fayalite's 4.408 g cm⁻³, as the row's grain density takes them.
const KOMATIITE_OLIVINE_DENSITY_G_PER_CM3: f64 = (0.91 * FORSTERITE.formula_weight_g_per_mol
    + 0.09 * FAYALITE.formula_weight_g_per_mol)
    / (0.91 * FORSTERITE.formula_weight_g_per_mol / 3.219
        + 0.09 * FAYALITE.formula_weight_g_per_mol / 4.408);

/// Komatiite SA 3091's mass by volume mode, per cubic centimetre: skeletal olivine 40, subcalcic
/// augite 45 at 3.31 and devitrified glass 15 per cent at 2.80 g cm⁻³ (Arndt 1976, as the row's
/// grain density takes them).
const KOMATIITE_MASS_G_PER_CM3: f64 =
    0.40 * KOMATIITE_OLIVINE_DENSITY_G_PER_CM3 + 0.45 * 3.31 + 0.15 * 2.80;

/// Komatiite's mode by mass: its olivine as Fo91 forsterite and fayalite, its augite as
/// diopside and its glass as `CaAl2Si2O8` glass, their own heat capacities not read.
const KOMATIITE_MODE: [(f64, f64); 12] = mode(&[
    (
        0.40 * KOMATIITE_OLIVINE_DENSITY_G_PER_CM3 / KOMATIITE_MASS_G_PER_CM3
            * forsterite_by_mass(0.91),
        FORSTERITE,
    ),
    (
        0.40 * KOMATIITE_OLIVINE_DENSITY_G_PER_CM3 / KOMATIITE_MASS_G_PER_CM3
            * (1.0 - forsterite_by_mass(0.91)),
        FAYALITE,
    ),
    (0.45 * 3.31 / KOMATIITE_MASS_G_PER_CM3, DIOPSIDE),
    (0.15 * 2.80 / KOMATIITE_MASS_G_PER_CM3, ANORTHITE_GLASS),
]);

/// The alkali feldspars, low albite and microcline in equal parts by mass.
const ALKALI_FELDSPARS: [(f64, f64); 12] = mode(&[(0.5, LOW_ALBITE), (0.5, MICROCLINE)]);

/// Forsterite alone.
const FORSTERITE_ALONE: [(f64, f64); 12] = mode(&[(1.0, FORSTERITE)]);

/// Enstatite alone.
const ENSTATITE_ALONE: [(f64, f64); 12] = mode(&[(1.0, ENSTATITE)]);

/// A mode's specific heat, its read points from 20 to 350 K.
#[must_use]
const fn mode_heat_capacity(points: &'static [(f64, f64); 12]) -> [ThermalPiece; 1] {
    [only(20.0, 350.0, ThermalForm::Points(points))]
}

/// The ultramafic rock's specific heat.
const ULTRAMAFIC_HEAT_CAPACITY: [ThermalPiece; 1] = mode_heat_capacity(&ULTRAMAFIC_MODE);

/// Anorthosite's specific heat.
const ANORTHOSITE_HEAT_CAPACITY: [ThermalPiece; 1] = mode_heat_capacity(&ANORTHOSITE_MODE);

/// Komatiite's specific heat.
const KOMATIITE_HEAT_CAPACITY: [ThermalPiece; 1] = mode_heat_capacity(&KOMATIITE_MODE);

/// The alkali feldspars' specific heat.
const ALKALI_FELDSPAR_HEAT_CAPACITY: [ThermalPiece; 1] = mode_heat_capacity(&ALKALI_FELDSPARS);

/// Forsterite's specific heat.
const FORSTERITE_HEAT_CAPACITY: [ThermalPiece; 1] = mode_heat_capacity(&FORSTERITE_ALONE);

/// Enstatite's specific heat.
const ENSTATITE_HEAT_CAPACITY: [ThermalPiece; 1] = mode_heat_capacity(&ENSTATITE_ALONE);

/// The minerals' calorimetry, as one source: each mineral's smoothed molar heat capacity, read in
/// its paper's table and converted with its printed formula weight.
const MINERAL_CALORIMETRY: &str = "the U.S. Geological Survey's low-temperature adiabatic \
     calorimetry of the minerals, each table's smoothed C_p at 20, 40, 60, 70, 80, 100, 150, \
     200, 250, 273.15, 298.15 and 350 K read and converted with its printed formula weight \
     (fayalite's λ peak near 65 K falls between the points): forsterite (Robie, \
     Hemingway and Takei 1982, Am. Mineral. 67, 470, Table 5), fayalite (Robie, Finch and \
     Hemingway 1982, Am. Mineral. 67, 463, Table 2), enstatite and diopside (Krupka, Robie, \
     Hemingway, Kerrick and Ito 1985, Am. Mineral. 70, 249, Tables 12 and 10), anorthite and \
     CaAl2Si2O8 glass (Robie, Hemingway and Wilson 1978, Am. Mineral. 63, 109, Tables 9 and 8), \
     low albite and microcline (Openshaw, Hemingway, Robie, Waldbaum and Krupka 1976, J. Res. U.S. \
     Geol. Survey 4, 195, Tables 7 and 9); accurate to about 0.15 per cent above 25 K";

// ---------------------------------------------------------------------------------------------
// The dense rocks' conductivities (Diment and Pratt 1988, USGS Open-File Report 88-690, Table 1;
// Robertson 1988, USGS Open-File Report 88-441).

/// Anorthosite's, W m⁻¹ K⁻¹: 4.3 CU, by Horai and Simmons 1969's mixture method (Diment and Pratt
/// 1988, Table 1, p. 4).
const ANORTHOSITE_CONDUCTIVITY: [ThermalPiece; 1] = room_temperature(4.3 * CU_W_PER_M_K);

/// A basalt's, W m⁻¹ K⁻¹: 6.1 CU, "the fully solid intrinsic K" of a basalt with 5 per cent
/// olivine (Robertson 1988, p. 6, after Robertson and Peck 1974).
const BASALT_CONDUCTIVITY: [ThermalPiece; 1] = room_temperature(6.1 * CU_W_PER_M_K);

/// A dunite's, W m⁻¹ K⁻¹: 11.1 CU, the mean of the Balsam Gap dunite's three, 10.6, 12.2 and 10.5
/// at 30 °C, 97 per cent olivine Fo92 (Birch and Clark 1940, as Diment and Pratt 1988, Table 1,
/// p. 5, list them).
const DUNITE_CONDUCTIVITY: [ThermalPiece; 1] =
    room_temperature((10.6 + 12.2 + 10.5) / 3.0 * CU_W_PER_M_K);

/// Enstatite's, W m⁻¹ K⁻¹: 11.8 CU, by Horai and Simmons 1969's mixture method (Diment and Pratt
/// 1988, Table 1, p. 5).
const ENSTATITE_CONDUCTIVITY: [ThermalPiece; 1] = room_temperature(11.8 * CU_W_PER_M_K);

/// The felsic rocks', W m⁻¹ K⁻¹: 7.5 CU, the middle of "between 6.5 and 8.5", where "a surprising
/// number of igneous and metamorphic rocks" fall (Diment and Pratt 1988, p. 2), and of Robertson
/// 1988's granites near 0 °C, Westerly's about 6 to Rockport's about 9 (Fig. 16, read off the
/// figure).
const FELSIC_CONDUCTIVITY: [ThermalPiece; 1] = room_temperature(7.5 * CU_W_PER_M_K);

/// The temperature dependence's source, for every dense rock's conductivity.
const ROOM_TEMPERATURE_HELD: &str = "held over 0–100 °C, where \"the K of plagioclase and \
     microcline is roughly constant with T\" and other minerals' \"decreases rather sharply … from \
     -10 to -30 percent\" (Robertson 1988, USGS Open-File Report 88-441, p. 41); below it a \
     crystal's rises and a glass's falls, not read";

// ---------------------------------------------------------------------------------------------
// The chondrites.

/// The ordinary chondrites' and an enstatite chondrite's conductivity range: "vary only slightly
/// with temperature above 100 K" (Opeil, Consolmagno and Britt 2010, Icarus 208, 449, abstract),
/// to 300 K.
#[must_use]
const fn chondrite_conductivity(form: ThermalForm) -> [ThermalPiece; 1] {
    [only(100.0, 300.0, form)]
}

/// Gundlach and Blum 2013's S-type solid conductivity, W m⁻¹ K⁻¹: 2.18, Opeil et al. 2010's
/// Cronstad (H5) and Lumpkin (L6) corrected to no porosity by Maxwell's formula (their Table 1,
/// eqs. 6 and 7).
const ORDINARY_CHONDRITE_CONDUCTIVITY: [ThermalPiece; 1] =
    chondrite_conductivity(ThermalForm::Constant(2.18));

/// Gundlach and Blum 2013's C-type solid conductivity, W m⁻¹ K⁻¹: 1.19 + 2.1 × 10⁻³ T, Opeil et
/// al. 2010's Cold Bokkeveld (CM2) and NWA 5515 (CK4) corrected to no porosity (Table 1, eq. 7).
const CARBONACEOUS_CHONDRITE_CONDUCTIVITY: [ThermalPiece; 1] =
    chondrite_conductivity(ThermalForm::Polynomial(&[1.19, 2.1e-3]));

/// Abee's porosity, the sample's: "Our sample of Abee has a porosity of 2.19 ± 1.5%" (Opeil,
/// Consolmagno, Safarik and Britt 2012, MAPS 47, 319).
const ABEE_POROSITY: f64 = 0.0219;

/// The enstatite chondrites' solid conductivity, W m⁻¹ K⁻¹: Abee's (EH4), 5.33 at 200 K (Opeil et
/// al. 2010, as Opeil et al. 2012's Table 2 restates it), corrected to no porosity by Maxwell's
/// formula, k (2 + ψ) ÷ (2 (1 − ψ)), as Gundlach and Blum 2013 correct theirs (eq. 6).
const ENSTATITE_CHONDRITE_CONDUCTIVITY: [ThermalPiece; 1] = chondrite_conductivity(
    ThermalForm::Constant(5.33 * (2.0 + ABEE_POROSITY) / (2.0 * (1.0 - ABEE_POROSITY))),
);

/// The ordinary chondrites' heat capacities, Macke, Consolmagno and Britt 2019's composition-based
/// class averages, A + BT + CT² + DT³ (MAPS 54, 2729, Table 4), valid over 70–300 K.
#[must_use]
const fn ordinary_chondrite_heat_capacity(coefficients: &'static [f64]) -> [ThermalPiece; 1] {
    [only(70.0, 300.0, ThermalForm::Polynomial(coefficients))]
}

/// The H chondrites'.
const H_CHONDRITE_HEAT_CAPACITY: [ThermalPiece; 1] =
    ordinary_chondrite_heat_capacity(&[-213.4, 5.659, -11.131e-3, 8.697e-6]);

/// The L chondrites'.
const L_CHONDRITE_HEAT_CAPACITY: [ThermalPiece; 1] =
    ordinary_chondrite_heat_capacity(&[-206.4, 5.507, -9.684e-3, 6.258e-6]);

/// The LL chondrites'.
const LL_CHONDRITE_HEAT_CAPACITY: [ThermalPiece; 1] =
    ordinary_chondrite_heat_capacity(&[-200.9, 5.394, -8.815e-3, 4.787e-6]);

/// The CM chondrite Cold Bokkeveld's heat capacity, G + HT + JT² + LT³ + MT⁴ (Opeil et al. 2020,
/// MAPS 55, E1, Table 3, each listed value times its column's power of ten), valid over 2–300 K.
const CM_CHONDRITE_HEAT_CAPACITY: [ThermalPiece; 1] = [only(
    2.0,
    300.0,
    ThermalForm::Polynomial(&[0.2168, 0.42581, 4.425e-2, -2.060e-4, 2.853e-7]),
)];

/// A meteorite's heat capacity by Piqueux et al. 2021, F + GT + HT² + IT³ (JGR Planets 126,
/// e2021JE007003, Table 1, each listed value times its column's power of ten), valid over
/// 90–290 K, the measurements within 4–5 per cent and the fits' residuals mostly under 1.
#[must_use]
const fn piqueux_heat_capacity(coefficients: &'static [f64]) -> [ThermalPiece; 1] {
    [only(90.0, 290.0, ThermalForm::Polynomial(coefficients))]
}

/// The CI chondrite Orgueil's.
const CI_CHONDRITE_HEAT_CAPACITY: [ThermalPiece; 1] =
    piqueux_heat_capacity(&[94.1, 0.355, 1.547e-2, -3.312e-5]);

/// The CV chondrite Allende's.
const CV_CHONDRITE_HEAT_CAPACITY: [ThermalPiece; 1] =
    piqueux_heat_capacity(&[-288.0, 6.540, -1.390e-2, 1.270e-5]);

/// The EH chondrite SAH 97158's.
const E_CHONDRITE_HEAT_CAPACITY: [ThermalPiece; 1] =
    piqueux_heat_capacity(&[-688.1, 12.984, -4.994e-2, 7.432e-5]);

/// The ordinary chondrites' conductivity's source.
const ORDINARY_CHONDRITE_K: Source = source(
    "conductivity",
    "2.18 W m⁻¹ K⁻¹, Gundlach and Blum 2013's S-type solid conductivity (Icarus 223, 479, \
     arXiv:1212.3108, Table 1 and eq. 7, read): Opeil, Consolmagno and Britt 2010's Cronstad (H5) \
     and Lumpkin (L6), 1.88 and 1.47 at 200 K (Icarus 208, 449, as Opeil et al. 2012, MAPS 47, \
     319, Table 2, restates them), corrected to no porosity by Maxwell's formula (their eq. 6) at \
     porosities of 0.167 and 0.191, whose source was not found; held over 100–300 K, where their \
     conductivities \"vary only slightly with temperature\" (Opeil et al. 2010, abstract)",
    Basis::Reduced,
);

/// The carbonaceous chondrites' conductivity's source.
const CARBONACEOUS_CHONDRITE_K: Source = source(
    "conductivity",
    "1.19 + 2.1 × 10⁻³ T W m⁻¹ K⁻¹, Gundlach and Blum 2013's C-type solid conductivity (Table 1 \
     and eq. 7, read): Opeil et al. 2010's Cold Bokkeveld (CM2) and NWA 5515 (CK4), 0.50 and 1.48 \
     at 200 K (as Opeil et al. 2012, Table 2, restates them), corrected to no porosity by \
     Maxwell's formula at porosities of 0.373 and 0.251; Opeil et al. 2012 and 2020 give Cold \
     Bokkeveld's as 15.0 per cent, which would lower the class's figure about a tenth, recorded; \
     over 100–300 K",
    Basis::Reduced,
);

// ---------------------------------------------------------------------------------------------
// A comet's dust (Winter and Saari 1969).

/// Winter and Saari 1969's silicate heat capacity, J kg⁻¹ K⁻¹: −34 T^½ + 8 T − 0.2 T^(3/2),
/// their −0.034, 0.008 and −0.0002 J g⁻¹ K⁻¹ times 1,000 (ApJ 156, 1135, eq. 16), valid "from a
/// few tens of degrees Kelvin to 400° K", here from 40 K.
const SILICATE_DUST_HEAT_CAPACITY: [ThermalPiece; 1] = [only(
    40.0,
    400.0,
    ThermalForm::HalfPowers(&[0.0, -34.0, 8.0, -0.2]),
)];

// ---------------------------------------------------------------------------------------------
// The volatile ices. Their heat capacities are Ziegler, Mullins and Kirk's polynomials, fitted
// under contract to the National Bureau of Standards to the calorimetry primaries, in cal mol⁻¹
// K⁻¹ ("Calculation of the vapor pressure and heats of vaporization and sublimation of liquids
// and solids, especially below one atmosphere", Georgia Institute of Technology Engineering
// Experiment Station), converted with the thermochemical calorie, 4.184 J, and the registry's
// molar mass; the primaries were not re-read. A solid with phases takes its phase diagram's
// transitions, which a test holds.

/// One thermochemical calorie, J.
const CALORIE_J: f64 = 4.184;

/// The coefficients `cal`, in cal mol⁻¹ K⁻¹ per power of kelvin, in J kg⁻¹ K⁻¹ per power of
/// kelvin, at a molar mass of `molar_mass_g_per_mol`.
#[must_use]
const fn per_kilogram<const N: usize>(cal: [f64; N], molar_mass_g_per_mol: f64) -> [f64; N] {
    let mut out = [0.0; N];
    let mut i = 0;
    while i < N {
        out[i] = cal[i] * CALORIE_J / (molar_mass_g_per_mol / 1000.0);
        i += 1;
    }
    out
}

/// The registry's molar masses of the volatile ices, g mol⁻¹, which a test holds to the rows'.
pub(super) const CARBON_DIOXIDE_MOLAR_MASS_G_PER_MOL: f64 = 44.009;
/// Nitrogen's.
pub(super) const NITROGEN_MOLAR_MASS_G_PER_MOL: f64 = 28.014;
/// Methane's.
pub(super) const METHANE_MOLAR_MASS_G_PER_MOL: f64 = 16.043;
/// Carbon monoxide's.
pub(super) const CARBON_MONOXIDE_MOLAR_MASS_G_PER_MOL: f64 = 28.010;
/// Oxygen's.
pub(super) const OXYGEN_MOLAR_MASS_G_PER_MOL: f64 = 31.998;
/// Argon's.
pub(super) const ARGON_MOLAR_MASS_G_PER_MOL: f64 = 39.95;

/// A piece of `form` over `low_k`–`high_k` kelvin in the solid phase `phase`.
#[must_use]
const fn phase(phase: &'static str, low_k: f64, high_k: f64, form: ThermalForm) -> ThermalPiece {
    ThermalPiece {
        phase,
        low: Kelvin::new(low_k),
        high: Kelvin::new(high_k),
        form,
    }
}

/// Carbon dioxide ice's heat capacity from 15 to 38 K (Mullins, Kirk and Ziegler 1963, Project
/// A-663, Technical Report No. 2, Table XIII).
const CO2_C_15_38: [f64; 6] = per_kilogram(
    [
        0.0,
        0.085_851_190,
        -0.016_808_205,
        1.362_099_5e-3,
        -3.520_565_1e-5,
        3.034_998_1e-7,
    ],
    CARBON_DIOXIDE_MOLAR_MASS_G_PER_MOL,
);
/// From 38 to 101 K.
const CO2_C_38_101: [f64; 4] = per_kilogram(
    [-5.594_707_6, 0.374_018_98, -3.379_441_9e-3, 1.150_739_7e-5],
    CARBON_DIOXIDE_MOLAR_MASS_G_PER_MOL,
);
/// From 101 to 195 K.
const CO2_C_101_195: [f64; 4] = per_kilogram(
    [2.337_682_1, 0.116_987_73, -5.988_331_3e-4, 1.476_079_2e-6],
    CARBON_DIOXIDE_MOLAR_MASS_G_PER_MOL,
);

/// Carbon dioxide ice's heat capacity, its three fitted pieces in its one phase.
const CO2_HEAT_CAPACITY: [ThermalPiece; 3] = [
    phase("I", 15.0, 38.0, ThermalForm::Polynomial(&CO2_C_15_38)),
    phase("I", 38.0, 101.0, ThermalForm::Polynomial(&CO2_C_38_101)),
    phase("I", 101.0, 195.0, ThermalForm::Polynomial(&CO2_C_101_195)),
];

/// Carbon dioxide ice's conductivity, ln κ = Σ Aᵢ (ln T)ⁱ at 10⁵ Pa (Saiduzzaman, Konstantinov
/// and Andersson 2025, Int. J. Thermophys. 46, 70, Table 1), from 110 K to just below the triple
/// point.
const CO2_CONDUCTIVITY: [ThermalPiece; 1] = [phase(
    "I",
    110.0,
    216.0,
    ThermalForm::LogPolynomial(&[166.564, -98.611, 19.560, -1.304_39]),
)];

/// Carbon dioxide ice.
const CARBON_DIOXIDE: ThermalColumns = ThermalColumns {
    heat_capacity: own(&CO2_HEAT_CAPACITY),
    conductivity: own(&CO2_CONDUCTIVITY),
    sources: &[
        source(
            "heat capacity",
            "three polynomials in cal mol⁻¹ K⁻¹, 15–38, 38–101 and 101–195 K, fitted to Giauque \
             and Egan 1937's calorimetry (J. Chem. Phys. 5, 45; not re-read), by Mullins, Kirk and \
             Ziegler 1963 (Georgia Institute of Technology, Project A-663, Technical Report No. 2, \
             Table XIII, under contract to the NBS); 195–216.6 K not covered, held",
            Basis::Fact,
        ),
        source(
            "conductivity",
            "ln κ = 166.564 − 98.611 ln T + 19.560 (ln T)² − 1.30439 (ln T)³ W m⁻¹ K⁻¹ at 0.1 MPa \
             over 110–220 K, derived from measurements at 0.107 GPa and the measured pressure \
             dependence, ±2 per cent, the fit within 0.5 (Saiduzzaman, Konstantinov and Andersson \
             2025, Int. J. Thermophys. 46, 70, Table 1, CC BY 4.0); below 110 K not reached \
             (Koloskova et al. 1974), held",
            Basis::Fact,
        ),
    ],
};

/// Nitrogen ice's heat capacity in its α phase (Ziegler and Mullins 1963, Project A-663,
/// Technical Report No. 1, Table IV).
const N2_C_ALPHA: [f64; 5] = per_kilogram(
    [
        3.024_410_8,
        -0.829_568_62,
        0.093_116_545,
        -3.139_776_2e-3,
        3.819_047_3e-5,
    ],
    NITROGEN_MOLAR_MASS_G_PER_MOL,
);
/// In its β phase.
const N2_C_BETA: [f64; 5] = per_kilogram(
    [
        24.326_576,
        -1.622_356_1,
        0.057_461_088,
        -8.466_459_4e-4,
        4.623_712_0e-6,
    ],
    NITROGEN_MOLAR_MASS_G_PER_MOL,
);

/// Nitrogen ice's heat capacity, α from 10 K and β to just below the triple point.
const N2_HEAT_CAPACITY: [ThermalPiece; 2] = [
    phase("α", 10.0, 35.61, ThermalForm::Polynomial(&N2_C_ALPHA)),
    phase("β", 35.61, 63.15, ThermalForm::Polynomial(&N2_C_BETA)),
];

/// α-nitrogen's conductivity under its saturated vapour, (K, W m⁻¹ K⁻¹), read off Konstantinov et
/// al. 2005's Fig. 2 (arXiv:cond-mat/0412058), the literature's curve.
const N2_K_ALPHA: [(f64, f64); 6] = [
    (19.0, 0.42),
    (20.0, 0.40),
    (22.5, 0.36),
    (25.0, 0.33),
    (30.0, 0.28),
    (33.5, 0.27),
];
/// β-nitrogen's.
const N2_K_BETA: [(f64, f64); 6] = [
    (38.0, 0.221),
    (40.0, 0.219),
    (45.0, 0.214),
    (50.0, 0.208),
    (55.0, 0.204),
    (58.0, 0.201),
];

/// Nitrogen ice's conductivity, its read points in each phase.
const N2_CONDUCTIVITY: [ThermalPiece; 2] = [
    phase("α", 19.0, 35.61, ThermalForm::Points(&N2_K_ALPHA)),
    phase("β", 35.61, 58.0, ThermalForm::Points(&N2_K_BETA)),
];

/// Nitrogen ice.
const NITROGEN: ThermalColumns = ThermalColumns {
    heat_capacity: own(&N2_HEAT_CAPACITY),
    conductivity: own(&N2_CONDUCTIVITY),
    sources: &[
        source(
            "heat capacity",
            "two polynomials in cal mol⁻¹ K⁻¹, α 10–35.62 K and β 35.62–63.152 K, fitted to \
             Clusius's and Giauque and Clayton 1933's calorimetry (JACS 55, 4875; not re-read) by \
             Ziegler and Mullins 1963 (Georgia Institute of Technology, Project A-663, Technical \
             Report No. 1, Table IV, under contract to the NBS), joined at the diagram's 35.61 K; \
             β's 1.35–1.65 kJ kg⁻¹ K⁻¹ (Umurhan et al. 2017, arXiv:1606.05734, Table 1, after \
             Scott 1976) agrees",
            Basis::Fact,
        ),
        source(
            "conductivity",
            "under its saturated vapour, the literature's curve in Konstantinov, Revyakin and \
             Sumarokov's Fig. 2 (arXiv:cond-mat/0412058, after the Manzhelii handbook, Roder, \
             Koloskova and Stachowiak; not re-read), read off the figure within about 2 per cent: \
             α 19–33.5 K and β 38–58 K; β's 0.215–0.200 (Umurhan et al. 2017, Table 1) agrees; \
             Cook and Davey 1976 not reached",
            Basis::Fact,
        ),
    ],
};

/// Methane ice's heat capacity in its phase I (Ziegler, Mullins and Kirk 1962, Project A-460,
/// Technical Report No. 3, Table IV).
const CH4_C_I: [f64; 5] = per_kilogram(
    [
        -0.208_860_34,
        0.268_328_81,
        -2.527_947_7e-3,
        5.395_418_2e-6,
        4.592_320_1e-8,
    ],
    METHANE_MOLAR_MASS_G_PER_MOL,
);

/// Methane ice's heat capacity, phase I from its 20.4 K transition to just below the triple point;
/// phase II's not read.
const CH4_HEAT_CAPACITY: [ThermalPiece; 1] =
    [phase("I", 20.4, 90.6, ThermalForm::Polynomial(&CH4_C_I))];

/// Methane ice's conductivity in phase I, (K, W m⁻¹ K⁻¹), read off NBS Monograph 131's Fig. 66
/// (Childs, Ericks and Powell 1973), Manzhelii and Krupskii 1968's curve.
const CH4_K_I: [(f64, f64); 7] = [
    (22.0, 0.33),
    (30.0, 0.37),
    (40.0, 0.41),
    (50.0, 0.42),
    (60.0, 0.41),
    (70.0, 0.38),
    (80.0, 0.32),
];

/// Methane ice's conductivity, phase I's read points.
const CH4_CONDUCTIVITY: [ThermalPiece; 1] = [phase("I", 22.0, 80.0, ThermalForm::Points(&CH4_K_I))];

/// Methane ice.
const METHANE: ThermalColumns = ThermalColumns {
    heat_capacity: own(&CH4_HEAT_CAPACITY),
    conductivity: own(&CH4_CONDUCTIVITY),
    sources: &[
        source(
            "heat capacity",
            "phase I's polynomial in cal mol⁻¹ K⁻¹ over 20.4–90.64 K, fitted to Clusius's and \
             Frank and Clusius's calorimetry (not re-read), \"less than 0.5 per cent\" off but \
             near the ends (Ziegler, Mullins and Kirk 1962, Georgia Institute of Technology, \
             Project A-460, Technical Report No. 3, Table IV, under contract to the NBS); phase \
             II's, below 20.4 K, not read, held",
            Basis::Fact,
        ),
        source(
            "conductivity",
            "phase I, Manzhelii and Krupskii 1968's crystal \"grown from vapor phase; transparent; \
             no visible defects\", read off the log scale of NBS Monograph 131's Fig. 66 (Childs, \
             Ericks and Powell 1973, p. 519, U.S. public domain) within about 5 per cent, 22–80 K; \
             phase II's not read",
            Basis::Fact,
        ),
    ],
};

/// Carbon monoxide ice's heat capacity in α below 44 K, the fit to Clayton and Giauque's data
/// (Mullins, Kirk and Ziegler 1963, Project A-663, Technical Report No. 2, Table VI).
const CO_C_ALPHA_LOW: [f64; 6] = per_kilogram(
    [
        0.0,
        -0.474_238_48,
        0.077_120_104,
        -3.363_457_8e-3,
        6.562_630_8e-5,
        -4.784_556_0e-7,
    ],
    CARBON_MONOXIDE_MOLAR_MASS_G_PER_MOL,
);
/// In α from 44 K to the α–β transition.
const CO_C_ALPHA_HIGH: [f64; 5] = per_kilogram(
    [
        0.0,
        -1.046_929_8,
        0.079_050_935,
        -1.672_037_0e-3,
        1.185_410_7e-5,
    ],
    CARBON_MONOXIDE_MOLAR_MASS_G_PER_MOL,
);
/// In β.
const CO_C_BETA: [f64; 2] = per_kilogram(
    [7.162_162_1, 0.077_562_9],
    CARBON_MONOXIDE_MOLAR_MASS_G_PER_MOL,
);

/// Carbon monoxide ice's heat capacity, α in two fitted pieces and β to just below the triple
/// point.
const CO_HEAT_CAPACITY: [ThermalPiece; 3] = [
    phase("α", 14.36, 44.0, ThermalForm::Polynomial(&CO_C_ALPHA_LOW)),
    phase("α", 44.0, 61.585, ThermalForm::Polynomial(&CO_C_ALPHA_HIGH)),
    phase("β", 61.585, 68.1, ThermalForm::Polynomial(&CO_C_BETA)),
];

/// Carbon monoxide ice's conductivity in α, W m⁻¹ K⁻¹: 0.315, the middle of Umurhan et al. 2017's
/// 0.350–0.280 over 45–65 K.
const CO_CONDUCTIVITY: [ThermalPiece; 1] = [phase("α", 45.0, 61.585, ThermalForm::Constant(0.315))];

/// Carbon monoxide ice.
const CARBON_MONOXIDE: ThermalColumns = ThermalColumns {
    heat_capacity: own(&CO_HEAT_CAPACITY),
    conductivity: own(&CO_CONDUCTIVITY),
    sources: &[
        source(
            "heat capacity",
            "polynomials in cal mol⁻¹ K⁻¹ (Mullins, Kirk and Ziegler 1963, Georgia Institute of \
             Technology, Project A-663, Technical Report No. 2, Table VI, under contract to the \
             NBS): α below 44 K, the fit to Clayton and Giauque 1932's data (not re-read; the \
             report's other fit, to Clusius and Eucken's, within 5 per cent), α 44–61.544 K and \
             β 61.544–68.127 K, joined at the diagram's 61.585 K",
            Basis::Fact,
        ),
        source(
            "conductivity",
            "0.315 W m⁻¹ K⁻¹, the middle of α's \"0.350-0.280\" between 45 and 65 K (Umurhan et \
             al. 2017, arXiv:1606.05734, Table 1, after Konstantinov et al. 2006's Fig. 4; not \
             re-read), ±0.035; β's and α's below 45 K not reached (Stachowiak et al. 1998)",
            Basis::Adopted,
        ),
    ],
};

/// Oxygen ice's heat capacity in α (Mullins, Ziegler and Kirk 1962, Project A-593, Technical
/// Report No. 2, Table IV).
const O2_C_ALPHA: [f64; 5] = per_kilogram(
    [
        1.933_359_7,
        -0.686_218_49,
        0.084_783_319,
        -3.636_083_5e-3,
        6.434_099_8e-5,
    ],
    OXYGEN_MOLAR_MASS_G_PER_MOL,
);
/// In β.
const O2_C_BETA: [f64; 5] = per_kilogram(
    [
        -13.911_685,
        2.094_514_9,
        -0.095_086_250,
        2.100_653_4e-3,
        -1.648_166_7e-5,
    ],
    OXYGEN_MOLAR_MASS_G_PER_MOL,
);
/// In γ.
const O2_C_GAMMA: [f64; 3] = per_kilogram(
    [16.908_081, -0.241_817_77, 2.480_908_9e-3],
    OXYGEN_MOLAR_MASS_G_PER_MOL,
);

/// Oxygen ice's heat capacity, α from 8 K, β and γ to just below the triple point.
const O2_HEAT_CAPACITY: [ThermalPiece; 3] = [
    phase("α", 8.0, 23.14, ThermalForm::Polynomial(&O2_C_ALPHA)),
    phase("β", 23.14, 43.78, ThermalForm::Polynomial(&O2_C_BETA)),
    phase("γ", 43.78, 54.35, ThermalForm::Polynomial(&O2_C_GAMMA)),
];

/// γ-oxygen's conductivity, (K, W m⁻¹ K⁻¹), read off Konstantinov et al.'s Fig. 2
/// (arXiv:cond-mat/0412059), their nearly free sample.
const O2_K_GAMMA: [(f64, f64); 4] = [(44.0, 0.209), (46.0, 0.197), (50.0, 0.185), (54.0, 0.183)];

/// Oxygen ice's conductivity, γ's read points; α's and β's not read.
const O2_CONDUCTIVITY: [ThermalPiece; 1] =
    [phase("γ", 44.0, 54.0, ThermalForm::Points(&O2_K_GAMMA))];

/// Oxygen ice.
const OXYGEN: ThermalColumns = ThermalColumns {
    heat_capacity: own(&O2_HEAT_CAPACITY),
    conductivity: own(&O2_CONDUCTIVITY),
    sources: &[
        source(
            "heat capacity",
            "polynomials in cal mol⁻¹ K⁻¹, α 8–23.781 K, β 23.781–43.772 K and γ 43.772–54.352 K \
             (Mullins, Ziegler and Kirk 1962, Georgia Institute of Technology, Project A-593, \
             Technical Report No. 2, Table IV, under contract to the NBS; γ's \"no larger than … \
             about 0.5%\" off, Roder 1977, NBSIR 77-859, §6.4), joined at the diagram's 23.14 and \
             43.78 K, where its sublimation pieces cross; α within about 4 per cent of \
             Kostriukova 1956's measurements over 8.7–19.4 K (Sov. Phys. JETP 3, 771)",
            Basis::Fact,
        ),
        source(
            "conductivity",
            "γ only, Konstantinov et al.'s nearly free sample, read off their Fig. 2 \
             (arXiv:cond-mat/0412059) within about 2 per cent, 44–54 K; α's and β's not read, so \
             γ's held below 44 K",
            Basis::Fact,
        ),
    ],
};

/// Argon ice's heat capacity from 11 to 25 K (Ziegler, Mullins and Kirk 1962, Project A-460,
/// Technical Report No. 2, Table VII).
const AR_C_11_25: [f64; 5] = per_kilogram(
    [
        -2.202_002_29,
        0.332_441_88,
        -8.194_598_0e-3,
        3.385_977_4e-4,
        -6.658_851_7e-6,
    ],
    ARGON_MOLAR_MASS_G_PER_MOL,
);
/// From 25 to 50 K.
const AR_C_25_50: [f64; 5] = per_kilogram(
    [
        -0.820_995_00,
        0.216_191_86,
        1.876_584e-4,
        -7.208_966e-5,
        7.283_838_1e-7,
    ],
    ARGON_MOLAR_MASS_G_PER_MOL,
);
/// From 50 K to the triple point.
const AR_C_50_83: [f64; 4] = per_kilogram(
    [-2.801_427_0, 0.388_329_82, -6.011_375_8e-3, 3.548_374_5e-5],
    ARGON_MOLAR_MASS_G_PER_MOL,
);

/// Argon ice's heat capacity, three fitted pieces of its fcc solid from 11 K.
const AR_HEAT_CAPACITY: [ThermalPiece; 3] = [
    phase("fcc", 11.0, 25.0, ThermalForm::Polynomial(&AR_C_11_25)),
    phase("fcc", 25.0, 50.0, ThermalForm::Polynomial(&AR_C_25_50)),
    phase("fcc", 50.0, 83.8, ThermalForm::Polynomial(&AR_C_50_83)),
];

/// Argon ice's conductivity, (K, W m⁻¹ K⁻¹): Krupskii and Manzhelii 1969's smoothed values from
/// their earlier argon data, six of their Table I's (Sov. Phys. JETP 28, 1097).
const AR_K: [(f64, f64); 6] = [
    (25.0, 1.43),
    (35.0, 0.923),
    (45.0, 0.645),
    (55.0, 0.469),
    (65.0, 0.346),
    (75.0, 0.255),
];

/// Argon ice's conductivity, its read points.
const AR_CONDUCTIVITY: [ThermalPiece; 1] = [phase("fcc", 25.0, 75.0, ThermalForm::Points(&AR_K))];

/// Argon ice.
const ARGON: ThermalColumns = ThermalColumns {
    heat_capacity: own(&AR_HEAT_CAPACITY),
    conductivity: own(&AR_CONDUCTIVITY),
    sources: &[
        source(
            "heat capacity",
            "polynomials in cal mol⁻¹ K⁻¹, 11–25, 25–50 and 50–83.8 K, fitted to Flubacher, \
             Leadbetter and Morrison 1961's and Figgins's calorimetry, which \"agree … to within \
             ± 0.7 per cent\" (not re-read), by Ziegler, Mullins and Kirk 1962 (Georgia Institute \
             of Technology, Project A-460, Technical Report No. 2, Table VII, under contract to \
             the NBS); the printed pieces step 5 per cent at 25 K, and the 2.2–11 K piece, 35 per \
             cent off at 11 K, is not used",
            Basis::Fact,
        ),
        source(
            "conductivity",
            "Krupskii and Manzhelii 1969's smoothed values for solid argon, from their earlier \
             argon data (Sov. Phys. JETP 28, 1097, Table I, six of its points over 25–75 K; the \
             paper's \"5-10%\" is its krypton and xenon runs'). Christen 1974's measurements \
             (AEC report, OSTI 4234979, Table 1) lie within 6 per cent at 25–45 K but 20–50 per \
             cent above at 65–76 K, and Ho, Powell and Liley 1972 recommend 0.99 at 25 K, 31 per \
             cent below: recorded",
            Basis::Fact,
        ),
    ],
};

// ---------------------------------------------------------------------------------------------
// The rows.

/// The anorthosite rock's conductivity's source.
const ANORTHOSITE_K: &str = "4.3 CU (1.80 W m⁻¹ K⁻¹), anorthosite by Horai and Simmons 1969's \
     mixture method, as Diment and Pratt 1988 list it (USGS Open-File Report 88-690, Table 1, p. \
     4; Birch and Clark 1940's anorthosite-bytownite 4.5 at 100 °C beside it)";

/// The basalt's conductivity's source.
const BASALT_K: &str = "6.1 CU (2.55 W m⁻¹ K⁻¹), \"the fully solid intrinsic K\" of a basalt \
     with 5 per cent olivine, its mode's by Robertson's eq. 4, which two powdered samples \
     corroborate (Robertson 1988, USGS Open-File Report 88-441, p. 6, after Robertson and Peck \
     1974); Diment and Pratt 1988 give unaltered basalts \"about 5\" (p. 2)";

/// Anorthosite: about 95 per cent An95 plagioclase.
const ANORTHOSITE: ThermalColumns = ThermalColumns {
    heat_capacity: own(&ANORTHOSITE_HEAT_CAPACITY),
    conductivity: own(&ANORTHOSITE_CONDUCTIVITY),
    sources: &[
        source(
            "heat capacity",
            "the mode by mass of 95 per cent An95 plagioclase and 5 per cent low-calcium \
             pyroxene by volume, at 2.753 and 3.5 g cm⁻³ (the row's grain density's), the \
             plagioclase as anorthite and low albite by their formula weights and the pyroxene as \
             enstatite, Σ wᵢ cᵢ over the minerals' calorimetry",
            Basis::Reduced,
        ),
        source("heat capacity", MINERAL_CALORIMETRY, Basis::Fact),
        source("conductivity", ANORTHOSITE_K, Basis::Fact),
        source("conductivity", ROOM_TEMPERATURE_HELD, Basis::Adopted),
    ],
};

/// The impact-mixed lunar highland crust.
const FELDSPATHIC: ThermalColumns = ThermalColumns {
    heat_capacity: own(&HIGHLAND_HEAT_CAPACITY),
    conductivity: stand_in(&ANORTHOSITE_CONDUCTIVITY),
    sources: &[
        HIGHLAND_SOIL,
        source(
            "conductivity",
            "a stand-in, the anorthosite rock's, the crust's purest part, no conductivity of the \
             mixed highland crust without cracks being read: its 20 per cent mafic minerals, \
             pyroxenes of about 10 CU, would raise it about a fifth by Robertson's geometric mean \
             (Robertson 1988, eq. 4)",
            Basis::Adopted,
        ),
        source("conductivity", ANORTHOSITE_K, Basis::Fact),
        source("conductivity", ROOM_TEMPERATURE_HELD, Basis::Adopted),
    ],
};

/// A terrestrial basalt.
const BASALT: ThermalColumns = ThermalColumns {
    heat_capacity: stand_in(&MARE_BASALT_HEAT_CAPACITY),
    conductivity: own(&BASALT_CONDUCTIVITY),
    sources: &[
        source(
            "heat capacity",
            "a stand-in, the lunar mare basalt 15555's, no terrestrial basalt's heat capacity \
             below 298 K being read",
            Basis::Adopted,
        ),
        MARE_BASALT,
        source("conductivity", BASALT_K, Basis::Fact),
        source("conductivity", ROOM_TEMPERATURE_HELD, Basis::Adopted),
    ],
};

/// The sources of `basalt_fe`'s columns, which `ferric` and `mars_dust` take.
const BASALT_FE_SOURCES: &[Source] = &[
    MARE_BASALT,
    source(
        "conductivity",
        "a stand-in, the terrestrial basalt's: the lunar basalts' measured conductivities are \
         their cracked samples', 0.2–1.5 W m⁻¹ K⁻¹ (Robertson 1988, Table 8, after Fujii and Osako \
         1973 and Horai and Winkler 1975), no dense solid's",
        Basis::Adopted,
    ),
    source("conductivity", BASALT_K, Basis::Fact),
    source("conductivity", ROOM_TEMPERATURE_HELD, Basis::Adopted),
];

/// An iron-rich, anhydrous basalt: the lunar mare basalt.
const BASALT_FE: ThermalColumns = ThermalColumns {
    heat_capacity: own(&MARE_BASALT_HEAT_CAPACITY),
    conductivity: stand_in(&BASALT_CONDUCTIVITY),
    sources: BASALT_FE_SOURCES,
};

/// Komatiite.
const KOMATIITE: ThermalColumns = ThermalColumns {
    heat_capacity: own(&KOMATIITE_HEAT_CAPACITY),
    conductivity: stand_in(&DUNITE_CONDUCTIVITY),
    sources: &[
        source(
            "heat capacity",
            "the mode by mass of SA 3091's 40 per cent skeletal olivine, 45 subcalcic augite and \
             15 devitrified glass by volume (Arndt 1976, as the row's grain density takes it, \
             with its densities), the olivine as Fo91 forsterite and fayalite, the augite as \
             diopside and the glass as CaAl2Si2O8 glass, Σ wᵢ cᵢ over the minerals' calorimetry",
            Basis::Reduced,
        ),
        source("heat capacity", MINERAL_CALORIMETRY, Basis::Fact),
        source(
            "conductivity",
            "a stand-in, the ultramafic rock's dunite: no komatiite's was read; its augite, \
             9.9–10.1 CU (Diment and Pratt 1988, Table 1, p. 5), is near the olivine's, and its \
             15 per cent glass, about 3 CU, would lower it",
            Basis::Adopted,
        ),
        source(
            "conductivity",
            "the Balsam Gap dunite's 10.6, 12.2 and 10.5 CU at 30 °C, 97 per cent olivine Fo92 \
             (Birch and Clark 1940, as Diment and Pratt 1988, Table 1, p. 5, list them), their \
             mean 11.1 CU, 4.64 W m⁻¹ K⁻¹",
            Basis::Reduced,
        ),
        source("conductivity", ROOM_TEMPERATURE_HELD, Basis::Adopted),
    ],
};

/// The basalts' columns standing for Mercury's reduced lid.
const BASALT_REDUCED: ThermalColumns = ThermalColumns {
    heat_capacity: stand_in(&MARE_BASALT_HEAT_CAPACITY),
    conductivity: stand_in(&BASALT_CONDUCTIVITY),
    sources: &[
        source(
            "heat capacity",
            "a stand-in, the lunar mare basalt 15555's, no measurement of a reduced, low-FeO \
             basalt being read",
            Basis::Adopted,
        ),
        MARE_BASALT,
        source(
            "conductivity",
            "a stand-in, the terrestrial basalt's, no reduced basalt's being read",
            Basis::Adopted,
        ),
        source("conductivity", BASALT_K, Basis::Fact),
        source("conductivity", ROOM_TEMPERATURE_HELD, Basis::Adopted),
    ],
};

/// Granite.
const GRANITE: ThermalColumns = ThermalColumns {
    heat_capacity: stand_in(&ALKALI_FELDSPAR_HEAT_CAPACITY),
    conductivity: stand_in(&FELSIC_CONDUCTIVITY),
    sources: &[
        source(
            "heat capacity",
            "a stand-in, the alkali feldspars', low albite and microcline in equal parts by mass, \
             Σ wᵢ cᵢ over the minerals' calorimetry: its quartz's below 298 K was not reached, and \
             quartz's 742 J kg⁻¹ K⁻¹ at 298.15 K (Robie and Hemingway 1995, USGS Bulletin 2131, \
             p. 49) is within 2 per cent of the feldspars' there",
            Basis::Adopted,
        ),
        source("heat capacity", MINERAL_CALORIMETRY, Basis::Fact),
        source(
            "conductivity",
            "a stand-in, the felsic rocks' typical 7.5 CU (3.14 W m⁻¹ K⁻¹), the middle of the \
             6.5–8.5 where \"a surprising number of igneous and metamorphic rocks\" fall \
             (Diment and Pratt 1988, p. 2), as Robertson 1988's granites near 0 °C span Westerly's \
             about 6 to Rockport's about 9 (Fig. 16, read off the figure)",
            Basis::Adopted,
        ),
        source("conductivity", ROOM_TEMPERATURE_HELD, Basis::Adopted),
    ],
};

/// An ultramafic rock, a harzburgite.
const ULTRAMAFIC: ThermalColumns = ThermalColumns {
    heat_capacity: own(&ULTRAMAFIC_HEAT_CAPACITY),
    conductivity: stand_in(&DUNITE_CONDUCTIVITY),
    sources: &[
        source(
            "heat capacity",
            "the mode by mass of Hu, Ehlmann and Seager 2012's \"60% Olivine, 40% Enstatite\" \
             (ApJ 752, 7, Table 2), the olivine of Mg# 90 as forsterite and fayalite by their \
             formula weights and the orthopyroxene as enstatite, Σ wᵢ cᵢ over the minerals' \
             calorimetry",
            Basis::Reduced,
        ),
        source("heat capacity", MINERAL_CALORIMETRY, Basis::Fact),
        source(
            "conductivity",
            "a stand-in, a dunite's, no harzburgite's being read: the Balsam Gap dunite's 10.6, \
             12.2 and 10.5 CU at 30 °C, 97 per cent olivine Fo92 (Birch and Clark 1940, as Diment \
             and Pratt 1988, Table 1, p. 5, list them), their mean 11.1 CU, 4.64 W m⁻¹ K⁻¹; its \
             enstatite, 11.8 CU (Horai and Simmons 1969, there), is within 7 per cent",
            Basis::Adopted,
        ),
        source("conductivity", ROOM_TEMPERATURE_HELD, Basis::Adopted),
    ],
};

/// An Fe-oxidised rock: `basalt_fe`'s columns, as its grain density.
const FERRIC: ThermalColumns = ThermalColumns {
    heat_capacity: stand_in(&MARE_BASALT_HEAT_CAPACITY),
    conductivity: stand_in(&BASALT_CONDUCTIVITY),
    sources: &[
        source(
            "heat capacity, conductivity",
            "a stand-in, `basalt_fe`'s columns, as its grain density is (decision-backlog-2 \
             §1.2): its few weight per cent of ferric phases were not measured",
            Basis::Adopted,
        ),
        MARE_BASALT,
        source("conductivity", BASALT_K, Basis::Fact),
        source("conductivity", ROOM_TEMPERATURE_HELD, Basis::Adopted),
    ],
};

/// The sources of the CM chondrite's columns, which `phyllosilicate` takes.
const CM_CHONDRITE_C: Source = source(
    "heat capacity",
    "the CM2 chondrite Cold Bokkeveld, by relaxation calorimetry in vacuum, the fourth-order fit \
     over 2–300 K (Opeil, Britt, Macke and Consolmagno 2020, MAPS 55, E1, Table 3: G 0.2168, H \
     0.42581, J 4.425 × 10⁻², L −2.060 × 10⁻⁴, M 2.853 × 10⁻⁷, each listed value times its \
     column's power of ten), 308, 664 and 859 J kg⁻¹ K⁻¹ at 100, 200 and 300 K; the sample's \
     porosity 15.0 ± 3.4 per cent (Table 1)",
    Basis::Fact,
);

/// A phyllosilicate-rich rock: a CM chondrite's columns.
const PHYLLOSILICATE: ThermalColumns = ThermalColumns {
    heat_capacity: stand_in(&CM_CHONDRITE_HEAT_CAPACITY),
    conductivity: stand_in(&CARBONACEOUS_CHONDRITE_CONDUCTIVITY),
    sources: &[
        source(
            "heat capacity, conductivity",
            "a stand-in, `chondrite_cm`'s columns, a CM chondrite being chiefly serpentine-group \
             phyllosilicate: serpentine's heat capacity below 298 K (King et al. 1967) and the \
             temperature dependence of its conductivity were not read. A serpentinite's \
             conductivity at room temperature, 6.0 CU (2.51 W m⁻¹ K⁻¹) extrapolated to no \
             porosity (Diment and Pratt 1988, Table 1, after Diment 1964), and chrysotile's 989 J \
             kg⁻¹ K⁻¹ at 298.15 K (Robie and Hemingway 1995, USGS Bulletin 2131, p. 65) are \
             recorded",
            Basis::Adopted,
        ),
        CM_CHONDRITE_C,
        CARBONACEOUS_CHONDRITE_K,
    ],
};

/// Mars-type dust: `basalt_fe`'s columns, as its transition.
const MARS_DUST: ThermalColumns = ThermalColumns {
    heat_capacity: stand_in(&MARE_BASALT_HEAT_CAPACITY),
    conductivity: stand_in(&BASALT_CONDUCTIVITY),
    sources: &[
        source(
            "heat capacity, conductivity",
            "a stand-in, `basalt_fe`'s columns, as its transition is: the dust is iron-rich and \
             basaltic, and no dust grain's figures were read",
            Basis::Adopted,
        ),
        MARE_BASALT,
        source("conductivity", BASALT_K, Basis::Fact),
        source("conductivity", ROOM_TEMPERATURE_HELD, Basis::Adopted),
    ],
};

/// Enstatite, `MgSiO3`.
const ENSTATITE_ROW: ThermalColumns = ThermalColumns {
    heat_capacity: own(&ENSTATITE_HEAT_CAPACITY),
    conductivity: own(&ENSTATITE_CONDUCTIVITY),
    sources: &[
        source("heat capacity", MINERAL_CALORIMETRY, Basis::Fact),
        source(
            "conductivity",
            "11.8 CU (4.94 W m⁻¹ K⁻¹), enstatite by Horai and Simmons 1969's mixture method, as \
             Diment and Pratt 1988 list it (Table 1, p. 5)",
            Basis::Fact,
        ),
        source("conductivity", ROOM_TEMPERATURE_HELD, Basis::Adopted),
    ],
};

/// Forsterite, `Mg2SiO4`.
const FORSTERITE_ROW: ThermalColumns = ThermalColumns {
    heat_capacity: own(&FORSTERITE_HEAT_CAPACITY),
    conductivity: stand_in(&DUNITE_CONDUCTIVITY),
    sources: &[
        source("heat capacity", MINERAL_CALORIMETRY, Basis::Fact),
        source(
            "conductivity",
            "a stand-in, a dunite's, no single crystal's being read: the Balsam Gap dunite's \
             mean 11.1 CU at 30 °C, 97 per cent olivine Fo92 (Birch and Clark 1940, as Diment and \
             Pratt 1988, Table 1, p. 5, list them)",
            Basis::Adopted,
        ),
        source("conductivity", ROOM_TEMPERATURE_HELD, Basis::Adopted),
    ],
};

/// A CI chondrite.
const CHONDRITE_CI: ThermalColumns = ThermalColumns {
    heat_capacity: own(&CI_CHONDRITE_HEAT_CAPACITY),
    conductivity: stand_in(&CARBONACEOUS_CHONDRITE_CONDUCTIVITY),
    sources: &[
        source(
            "heat capacity",
            "the CI chondrite Orgueil, by differential scanning calorimetry dried in vacuum, the \
             cubic fit over 90–290 K, measured within 4–5 per cent, the fit's residuals mostly \
             under 1 (Piqueux et al. 2021, JGR Planets 126, e2021JE007003, Table 1: F 0.941 × 10², \
             G 0.355, H 1.547 × 10⁻², I −3.312 × 10⁻⁵)",
            Basis::Fact,
        ),
        source(
            "conductivity",
            "a stand-in, the carbonaceous chondrites': \"no thermal conductivity data have been \
             reported for CI chondrites\" (Ishizaki et al. 2023, Ryugu, preprint)",
            Basis::Adopted,
        ),
        CARBONACEOUS_CHONDRITE_K,
    ],
};

/// A CM chondrite.
const CHONDRITE_CM: ThermalColumns = ThermalColumns {
    heat_capacity: own(&CM_CHONDRITE_HEAT_CAPACITY),
    conductivity: own(&CARBONACEOUS_CHONDRITE_CONDUCTIVITY),
    sources: &[CM_CHONDRITE_C, CARBONACEOUS_CHONDRITE_K],
};

/// A CV chondrite, standing for CV, CO and CK.
const CHONDRITE_CV: ThermalColumns = ThermalColumns {
    heat_capacity: own(&CV_CHONDRITE_HEAT_CAPACITY),
    conductivity: own(&CARBONACEOUS_CHONDRITE_CONDUCTIVITY),
    sources: &[
        source(
            "heat capacity",
            "the CV chondrite Allende, the cubic fit over 90–290 K, measured within 4–5 per cent, \
             the fit's residuals mostly under 1 (Piqueux et al. 2021, Table 1: F −2.880 × 10², G \
             6.540, H −1.390 × 10⁻², I 1.270 × 10⁻⁵)",
            Basis::Fact,
        ),
        CARBONACEOUS_CHONDRITE_K,
        source(
            "conductivity",
            "the row's own: Gundlach and Blum's C-type figure rests in part on NWA 5515, a CK4, \
             one of the groups the row stands for",
            Basis::Adopted,
        ),
    ],
};

/// An H chondrite.
const CHONDRITE_H: ThermalColumns = ThermalColumns {
    heat_capacity: own(&H_CHONDRITE_HEAT_CAPACITY),
    conductivity: own(&ORDINARY_CHONDRITE_CONDUCTIVITY),
    sources: &[
        source(
            "heat capacity",
            "the H chondrites' composition-based average, −213.4 + 5.659 T − 11.131 × 10⁻³ T² + \
             8.697 × 10⁻⁶ T³ (Macke, Consolmagno and Britt 2019, MAPS 54, 2729, Table 4), valid \
             over 70–300 K, within 1 per cent of Gao-Guenie's and Barbotan's measurements",
            Basis::Fact,
        ),
        ORDINARY_CHONDRITE_K,
    ],
};

/// An L chondrite.
const CHONDRITE_L: ThermalColumns = ThermalColumns {
    heat_capacity: own(&L_CHONDRITE_HEAT_CAPACITY),
    conductivity: own(&ORDINARY_CHONDRITE_CONDUCTIVITY),
    sources: &[
        source(
            "heat capacity",
            "the L chondrites' composition-based average, −206.4 + 5.507 T − 9.684 × 10⁻³ T² + \
             6.258 × 10⁻⁶ T³ (Macke, Consolmagno and Britt 2019, Table 4), valid over 70–300 K",
            Basis::Fact,
        ),
        ORDINARY_CHONDRITE_K,
    ],
};

/// An LL chondrite.
const CHONDRITE_LL: ThermalColumns = ThermalColumns {
    heat_capacity: own(&LL_CHONDRITE_HEAT_CAPACITY),
    conductivity: stand_in(&ORDINARY_CHONDRITE_CONDUCTIVITY),
    sources: &[
        source(
            "heat capacity",
            "the LL chondrites' composition-based average, −200.9 + 5.394 T − 8.815 × 10⁻³ T² + \
             4.787 × 10⁻⁶ T³ (Macke, Consolmagno and Britt 2019, Table 4), valid over 70–300 K",
            Basis::Fact,
        ),
        source(
            "conductivity",
            "a stand-in, the ordinary chondrites' S-type figure, from an H and an L: no LL \
             chondrite's conductivity was read",
            Basis::Adopted,
        ),
        ORDINARY_CHONDRITE_K,
    ],
};

/// An enstatite chondrite.
const CHONDRITE_E: ThermalColumns = ThermalColumns {
    heat_capacity: own(&E_CHONDRITE_HEAT_CAPACITY),
    conductivity: own(&ENSTATITE_CHONDRITE_CONDUCTIVITY),
    sources: &[
        source(
            "heat capacity",
            "the EH chondrite SAH 97158, the cubic fit over 90–290 K, measured within 4–5 per \
             cent, the fit's residuals mostly under 1 (Piqueux et al. 2021, Table 1: F −6.881 × \
             10², G 12.984, H −4.994 × 10⁻², I 7.432 × 10⁻⁵)",
            Basis::Fact,
        ),
        source(
            "conductivity",
            "Abee (EH4), 5.33 W m⁻¹ K⁻¹ at 200 K (Opeil, Consolmagno and Britt 2010, as Opeil, \
             Consolmagno, Safarik and Britt 2012, MAPS 47, 319, Table 2, restates it), corrected \
             to no porosity by Maxwell's formula at its sample's 2.19 per cent (the text; the \
             table gives the population's 3.0, which would give 5.58), as \
             Gundlach and Blum 2013 correct theirs (eq. 6): 5.51; held over 100–300 K, where the \
             2010 samples' conductivities \"vary only slightly with temperature\". Pillistfer \
             (EL6), 5.51 at 200 K, falls as 1 ÷ T above a peak near 50 K (2012), recorded",
            Basis::Reduced,
        ),
    ],
};

/// A comet's refractory material.
const COMETARY: ThermalColumns = ThermalColumns {
    heat_capacity: stand_in(&SILICATE_DUST_HEAT_CAPACITY),
    conductivity: stand_in(&CARBONACEOUS_CHONDRITE_CONDUCTIVITY),
    sources: &[
        source(
            "heat capacity",
            "a stand-in, Winter and Saari 1969's silicate heat capacity, −0.034 T^½ + 0.008 T − \
             0.0002 T^(3/2) J g⁻¹ K⁻¹ (ApJ 156, 1135, eq. 16, read in the scan), valid \"from a \
             few tens of degrees Kelvin to 400° K\", which Groussin et al. 2019 adopt for comet \
             67P's dust, 770 J kg⁻¹ K⁻¹ at 300 K (Space Sci. Rev. 215, 29, arXiv:1905.01156, §2.3, \
             CC BY): no comet refractory's own was measured, and its 45 wt% organics' not read",
            Basis::Adopted,
        ),
        source(
            "conductivity",
            "a stand-in, the carbonaceous chondrites': no comet refractory's dense solid was \
             measured; Davidsson's NIMBUS takes the H5 chondrite Wellman's (arXiv:2208.14628, \
             §2.3), recorded",
            Basis::Adopted,
        ),
        CARBONACEOUS_CHONDRITE_K,
    ],
};

/// The rows with thermal columns, in registry order, by key.
pub(super) static THERMAL_ROWS: &[(SubstanceKey, ThermalColumns)] = &[
    (SubstanceKey::new_const("H2O"), WATER),
    (SubstanceKey::new_const("CH4"), METHANE),
    (SubstanceKey::new_const("N2"), NITROGEN),
    (SubstanceKey::new_const("O2"), OXYGEN),
    (SubstanceKey::new_const("CO2"), CARBON_DIOXIDE),
    (SubstanceKey::new_const("Ar"), ARGON),
    (SubstanceKey::new_const("CO"), CARBON_MONOXIDE),
    (SubstanceKey::new_const("MgSiO3"), ENSTATITE_ROW),
    (SubstanceKey::new_const("Mg2SiO4"), FORSTERITE_ROW),
    (SubstanceKey::new_const("mars_dust"), MARS_DUST),
    (SubstanceKey::new_const("anorthosite"), ANORTHOSITE),
    (SubstanceKey::new_const("feldspathic"), FELDSPATHIC),
    (SubstanceKey::new_const("basalt"), BASALT),
    (SubstanceKey::new_const("basalt_fe"), BASALT_FE),
    (SubstanceKey::new_const("komatiite"), KOMATIITE),
    (SubstanceKey::new_const("basalt_reduced"), BASALT_REDUCED),
    (SubstanceKey::new_const("granite"), GRANITE),
    (SubstanceKey::new_const("ultramafic"), ULTRAMAFIC),
    (SubstanceKey::new_const("ferric"), FERRIC),
    (SubstanceKey::new_const("phyllosilicate"), PHYLLOSILICATE),
    (SubstanceKey::new_const("chondrite_ci"), CHONDRITE_CI),
    (SubstanceKey::new_const("chondrite_cm"), CHONDRITE_CM),
    (SubstanceKey::new_const("chondrite_cv"), CHONDRITE_CV),
    (SubstanceKey::new_const("chondrite_h"), CHONDRITE_H),
    (SubstanceKey::new_const("chondrite_l"), CHONDRITE_L),
    (SubstanceKey::new_const("chondrite_ll"), CHONDRITE_LL),
    (SubstanceKey::new_const("chondrite_e"), CHONDRITE_E),
    (SubstanceKey::new_const("cometary"), COMETARY),
];

/// The rows with condensed columns and no thermal columns, in registry order, each with its
/// reason: the module documentation's list, which a test holds to it.
#[cfg(test)]
pub(super) const NOT_MODELLED: &[&str] = &[
    "NH3",
    "Kr",
    "Xe",
    "N2O",
    "SO2",
    "H2S",
    "C2H6",
    "CH3OH",
    "Fe",
    "NH4SH",
    "H2SO4",
    "S8",
    "KCl",
    "NaCl",
    "ZnS",
    "Na2S",
    "MnS",
    "Cr",
    "FeS",
    "SiO2",
    "Al2O3",
    "CaTiO3",
    "TiO2",
    "graphite",
    "tholin",
    "soot",
    "chromophore",
    "ice_giant_haze",
    "acid_uv_absorber",
    "brine_nacl",
    "brine_mgso4",
    "brine_naclo4",
    "brine_mgclo4",
    "nh3_h2o",
    "ch4_c2h6_n2",
    "lava_glass",
    "ca_al_residue",
    "iron_nickel",
    "SiC",
    "MgSO4",
    "Na2CO3",
    "CaCO3",
    "carbonaceous",
];
