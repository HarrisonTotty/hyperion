//! The condensables' phase diagrams (P14.T49.b's second step), one for each row whose reference
//! equation of state was read: water, methane, ammonia, nitrogen, oxygen, carbon dioxide, argon,
//! krypton, xenon, nitrous oxide, carbon monoxide, sulphur dioxide, hydrogen sulphide and ethane.
//!
//! Every coefficient is the published one, written as printed. Each reference equation of state is
//! the primary; NIST's Chemistry Web Book (SRD 69) is only the check of values asserted from it
//! (decision-composition §5). For krypton, xenon, nitrous oxide, carbon monoxide, sulphur dioxide,
//! hydrogen sulphide and ethane the papers (Lemmon and Span 2006, Gao et al. 2016, Bücker and
//! Wagner 2006) were not reachable, so their fixed points, ancillary equations and triple-point
//! enthalpies are CoolProp's (Bell, Wronski, Quoilin and Lemort 2014, Ind. Eng. Chem. Res. 53,
//! 2498; its fluid files under the MIT licence, `NOTICE`): its reading of each paper's critical and
//! triple points, its own fits to each paper's equation of state (Bell's, not the paper's
//! ancillaries), and its states at the triple point from that equation. The solid branches are
//! Fray and Schmitt 2009's polynomials where the reference gives none, read through Lisse et al.
//! 2021's reproduction of their table, since Fray and Schmitt's own paper was not reachable; their
//! ranges and uncertainties were not read, so each lower bound is this builder's, checked to keep
//! the polynomial's Clausius–Clapeyron enthalpy steady down to it.

use super::super::diagram::{
    DensityEquation, DensityForm, MeltingCurve, MeltingEquation, MeltingPiece, PhaseDiagram,
    SolidDensity, SolidPhase, SublimationEquation, SublimationPiece, WagnerEquation,
};
use super::super::{Basis, Source};
use crate::tables::saturation;
use crate::units::{Kelvin, KilogramsPerCubicMetre, Pascals};

/// One bar, the unit of Fray and Schmitt's polynomials.
const BAR: Pascals = Pascals::new(1e5);

/// One megapascal, the unit of Gao et al. 2023's sublimation equation.
const MEGAPASCAL: Pascals = Pascals::new(1e6);

/// The anchors' source, common to every diagram.
const ANCHORED: Source = Source {
    covers: "anchors",
    citation: "`tables::saturation`, fitted by `hyperion-fit`'s `saturation_curves`: the amount \
               added to each sublimation equation's ln p so that it meets the liquid branch at the \
               triple temperature, and each colder one the warmer at the transition between them \
               (science-p14-t24f §6.3)",
    basis: Basis::Fitted,
};

/// Fray and Schmitt 2009's polynomials, read through Lisse et al. 2021: the source every diagram
/// with a Fray and Schmitt piece lists beside its own.
const FRAY_SCHMITT: Source = Source {
    covers: "p_solid",
    citation: "Fray and Schmitt 2009 (Planet. Space Sci. 57, 2053), ln(p ÷ 1 bar) = A₀ + Σ Aᵢ ÷ \
               Tⁱ, read in Lisse et al. 2021's reproduction of their table (Icarus 356, 114072, \
               Table 2; B. Schmitt a co-author), their own paper not being reachable, in bar as \
               Schorghofer's Planetary-Code-Collection evaluates them (`Lunar/subl_species.f90`, \
               ×10⁵ to pascals). Their ranges \
               and uncertainties were not read, so each piece's lower bound is this builder's, \
               where the polynomial's Clausius–Clapeyron enthalpy still holds steady; below it \
               the branch continues at constant enthalpy",
    basis: Basis::Fact,
};

/// CoolProp's fluid files, the source every diagram whose paper was not reachable lists beside
/// its own.
const COOLPROP: Source = Source {
    covers: "T_t, p_t, T_c, p_c, p_liquid, ρ_l, L_vap",
    citation: "CoolProp's fluid files (Bell, Wronski, Quoilin and Lemort 2014, Ind. Eng. Chem. \
               Res. 53, 2498; master of 2026-10-14, MIT licence, `NOTICE`): its reading of the \
               paper's critical and triple points, Bell's fits to the paper's equation of state, \
               and its states of that equation at the triple point; each ancillary checked here \
               against the equation of state solved for phase equilibrium from CoolProp's \
               coefficients",
    basis: Basis::Raw { licence: "MIT" },
};

/// Water: IAPWS-95 (Wagner and Pruss 2002), its supplementary saturation release, IAPWS R14-08's
/// sublimation and melting equations and IAPWS R10-06's ice Ih.
pub(in crate::substance) const WATER: PhaseDiagram = PhaseDiagram {
    triple_temperature: Kelvin::new(273.16),
    triple_pressure: Pascals::new(611.657),
    critical_temperature: Kelvin::new(647.096),
    critical_pressure: Pascals::new(22.064e6),
    vapour_pressure: WagnerEquation {
        reducing_temperature: Kelvin::new(647.096),
        reducing_pressure: Pascals::new(22.064e6),
        terms: &[
            (-7.859_517_83, 1.0),
            (1.844_082_59, 1.5),
            (-11.786_649_7, 3.0),
            (22.680_741_1, 3.5),
            (-15.961_871_9, 4.0),
            (1.801_225_02, 7.5),
        ],
    },
    liquid_density: DensityEquation {
        form: DensityForm::Linear,
        reducing_temperature: Kelvin::new(647.096),
        reducing_density: KilogramsPerCubicMetre::new(322.0),
        terms: &[
            (1.992_740_64, 1.0 / 3.0),
            (1.099_653_42, 2.0 / 3.0),
            (-0.510_839_303, 5.0 / 3.0),
            (-1.754_934_79, 16.0 / 3.0),
            (-45.517_035_2, 43.0 / 3.0),
            (-674_694.45, 110.0 / 3.0),
        ],
    },
    sublimation: &[SublimationPiece {
        phase: "ice Ih",
        lower: Kelvin::new(50.0),
        equation: SublimationEquation::ScaledPowers {
            temperature: Kelvin::new(273.16),
            pressure: Pascals::new(611.657),
            terms: &[
                (-21.214_400_6, 0.003_333_333_33),
                (27.320_381_9, 1.206_666_67),
                (-6.105_981_3, 1.703_333_33),
            ],
        },
    }],
    anchors: &saturation::H2O_ANCHORS,
    melting: Some(MeltingCurve::Falling {
        lowest: Kelvin::new(251.165),
        equation: MeltingEquation::IceIh {
            temperature: Kelvin::new(273.16),
            pressure: Pascals::new(611.657),
            terms: &[
                (1_195_393.37, 3.0),
                (80_818.315_9, 25.75),
                (3_338.268_6, 103.75),
            ],
        },
        rising: &[
            MeltingPiece {
                start: Kelvin::new(251.165),
                end: None,
                equation: MeltingEquation::Simon {
                    temperature: Kelvin::new(251.165),
                    pressure: Pascals::new(208.566e6),
                    terms: &[(0.299_948, 60.0)],
                },
            },
            MeltingPiece {
                start: Kelvin::new(256.164),
                end: None,
                equation: MeltingEquation::Simon {
                    temperature: Kelvin::new(256.164),
                    pressure: Pascals::new(350.1e6),
                    terms: &[(1.187_21, 8.0)],
                },
            },
            MeltingPiece {
                start: Kelvin::new(273.31),
                end: None,
                equation: MeltingEquation::Simon {
                    temperature: Kelvin::new(273.31),
                    pressure: Pascals::new(632.4e6),
                    terms: &[(1.074_76, 4.6)],
                },
            },
            MeltingPiece {
                start: Kelvin::new(355.0),
                end: Some(Kelvin::new(715.0)),
                equation: MeltingEquation::Logarithmic {
                    temperature: Kelvin::new(355.0),
                    pressure: Pascals::new(2_216e6),
                    terms: &[(1.736_83, -1.0), (-0.054_460_6, 5.0), (0.806_106e-7, 22.0)],
                },
            },
        ],
    }),
    solids: &[SolidPhase {
        name: "ice Ih",
        start: Kelvin::ZERO,
        density: Some(SolidDensity::IceIh),
    }],
    triple_vaporisation_enthalpy_j_per_mol: 45_054.726,
    triple_fusion_enthalpy_j_per_mol: Some(6_007.099),
    sources: &[
        Source {
            covers: "T_t, p_t, T_c, p_c",
            citation: "273.16 K and 611.657 Pa, the measured triple point (Guildner, Johnson and \
                       Jones 1976, as IAPWS's releases take it; IAPWS-95 itself solves to 611.655 \
                       Pa), and 647.096 K and 22.064 MPa (Wagner and Pruss 2002, JPCRD 31, 387, \
                       Eqs. 2.1–2.2; IAPWS SR1-86(1992), Sec. 2)",
            basis: Basis::Fact,
        },
        Source {
            covers: "p_liquid, ρ_l",
            citation: "IAPWS SR1-86(1992), the supplementary release on saturation properties, \
                       Eqs. (1) and (2) (Wagner and Pruss 1993, JPCRD 22, 783; Wagner and Pruss \
                       2002, Eqs. 2.5 and 2.6): within 0.0022% of IAPWS-95's saturation pressure \
                       from 275 to 625 K (IAPWS R6-95(2018), Table 8)",
            basis: Basis::Fact,
        },
        Source {
            covers: "p_solid",
            citation: "ice Ih's sublimation pressure, IAPWS R14-08(2011), Eq. (6) (Wagner, \
                       Riethmann, Feistel and Harvey 2011, JPCRD 40, 043103), for 50–273.16 K, \
                       0.0011% at the triple point (k = 2); below 50 K the branch continues at \
                       constant enthalpy",
            basis: Basis::Fact,
        },
        ANCHORED,
        Source {
            covers: "p_melt",
            citation: "IAPWS R14-08(2011): ice Ih's melting pressure, Eq. (1), falling from the \
                       triple point to 251.165 K, the triple point of ice Ih, ice III and the \
                       liquid; and ices III, V, VI and VII's, Eqs. (2)–(5) as π = 1 − a(1 − θ^b), \
                       rising from it through the triple points of Table 1 (256.164 K and 350.1 \
                       MPa, 273.31 K and 632.4 MPa, 355 K and 2,216 MPa), Eq. (5) measured to 715 \
                       K and extrapolated beyond; within 2%, 3% and 7% (Table 2), and Table 3's \
                       values as the check",
            basis: Basis::Fact,
        },
        Source {
            covers: "ρ_s",
            citation: "ice Ih at one atmosphere, IAPWS R10-06(2009)'s Gibbs function (Feistel and \
                       Wagner 2006, JPCRD 35, 1021), as `ice_ih_density`",
            basis: Basis::Fact,
        },
        Source {
            covers: "L_vap",
            citation: "45.054726 kJ mol⁻¹, h″ − h′ = 2,500.92 − 0.001 kJ kg⁻¹ at 273.16 K \
                       (Wagner and Pruss 2002, Table 13.1), at M = 18.015268 g mol⁻¹",
            basis: Basis::Reduced,
        },
        Source {
            covers: "L_fus",
            citation: "6.007099 kJ mol⁻¹: ice Ih's h at the triple point, −333,444.254 J kg⁻¹ \
                       (IAPWS R10-06(2009), Table 6), against the liquid's p_t ÷ ρ′ = 0.612 J \
                       kg⁻¹ (IAPWS-95's reference state, u′ = s′ = 0 at the triple point), at M = \
                       18.015268 g mol⁻¹; Osborne 1939's measured 333.5 ± 0.2 international J g⁻¹ \
                       at 0 °C (J. Res. NBS 23, 643, its abstract) is the check",
            basis: Basis::Reduced,
        },
    ],
};

/// Methane: Setzmann and Wagner 1991, with Fray and Schmitt's solid and Maynard-Casely et al.'s
/// lattice.
pub(in crate::substance) const METHANE: PhaseDiagram = PhaseDiagram {
    triple_temperature: Kelvin::new(90.6941),
    triple_pressure: Pascals::new(11_696.0),
    critical_temperature: Kelvin::new(190.564),
    critical_pressure: Pascals::new(4.5992e6),
    vapour_pressure: WagnerEquation {
        reducing_temperature: Kelvin::new(190.564),
        reducing_pressure: Pascals::new(4.5992e6),
        terms: &[
            (-6.036_219, 1.0),
            (1.409_353, 1.5),
            (-0.494_519_9, 2.0),
            (-1.443_048, 4.5),
        ],
    },
    liquid_density: DensityEquation {
        form: DensityForm::Logarithmic,
        reducing_temperature: Kelvin::new(190.564),
        reducing_density: KilogramsPerCubicMetre::new(162.66),
        terms: &[
            (1.990_638_9, 0.354),
            (-0.787_561_97, 0.5),
            (0.036_976_723, 2.5),
        ],
    },
    sublimation: &[SublimationPiece {
        phase: "I",
        lower: Kelvin::new(20.0),
        equation: SublimationEquation::InversePowers {
            unit: BAR,
            coefficients: &[10.51, -1.110e3, -4.341e3, 1.035e5, -7.910e5],
        },
    }],
    anchors: &saturation::CH4_ANCHORS,
    melting: Some(MeltingCurve::Rising(&[MeltingPiece {
        start: Kelvin::new(90.6941),
        end: None,
        equation: MeltingEquation::Simon {
            temperature: Kelvin::new(90.6941),
            pressure: Pascals::new(11_696.0),
            terms: &[(24_756.8, 1.85), (-7_366.02, 2.1)],
        },
    }])),
    solids: &[
        SolidPhase {
            name: "II",
            start: Kelvin::ZERO,
            density: Some(SolidDensity::CellVolume {
                formula_units: 32.0,
                molar_mass_g_per_mol: 16.043,
                volume_cubic_angstrom: &[1_597.17, 0.0, 0.0, -1.57e-3, 1.04e-4],
            }),
        },
        SolidPhase {
            name: "I",
            start: Kelvin::new(20.4),
            density: Some(SolidDensity::CellVolume {
                formula_units: 4.0,
                molar_mass_g_per_mol: 16.043,
                volume_cubic_angstrom: &[200.2, 0.03, 0.0015],
            }),
        },
    ],
    triple_vaporisation_enthalpy_j_per_mol: 8_731.45,
    triple_fusion_enthalpy_j_per_mol: Some(939.2),
    sources: &[
        Source {
            covers: "T_t, p_t, T_c, p_c",
            citation: "90.6941 K and 11.696 kPa, and 190.564 K and 4.5992 MPa (Setzmann and \
                       Wagner 1991, JPCRD 20, 1061, Eq. 2.1, Tables 4, 26 and 39; their p. 1064 and \
                       Eq. 2.3 misprint 4.5922 MPa, with which their Eq. 3.2 would miss p_t by \
                       0.15%)",
            basis: Basis::Fact,
        },
        Source {
            covers: "p_liquid, ρ_l",
            citation: "Setzmann and Wagner 1991, Eqs. (3.2) and (3.4) with Table 26: within \
                       0.007% of their equation of state's Table 39 from 90.694 to 180 K, and \
                       NIST's WebBook as the check (34,375.892 Pa at 100 K, 1.0399613 MPa at 150 \
                       K, 3.2851807 MPa at 180 K)",
            basis: Basis::Fact,
        },
        Source {
            covers: "p_solid",
            citation: "methane I, Fray and Schmitt's CH4 polynomial, held from 20 \
                       K, where its Clausius–Clapeyron enthalpy stays 9.65–9.85 kJ mol⁻¹; it runs \
                       1.6% above the liquid branch at the triple point before its anchor",
            basis: Basis::Fact,
        },
        FRAY_SCHMITT,
        ANCHORED,
        Source {
            covers: "p_melt",
            citation: "Setzmann and Wagner 1991, Eq. (3.7), fitted to 367 K and within 0.2% of \
                       Prydz and Goodwin's and Cheng's data but near the triple point; extrapolated \
                       beyond",
            basis: Basis::Fact,
        },
        Source {
            covers: "ρ_s",
            citation: "the cell volumes of methane II (Z = 32, fitted 6–20 K) and I (Z = 4, \
                       fitted 22–80 K, extrapolated to the triple point) from neutron diffraction, \
                       Maynard-Casely, Hester and Brand 2020 (IUCrJ 7, 844, Sec. 3.1.1 and Fig. \
                       2), with the transition at 20.4 K, a 0.6% step",
            basis: Basis::Fact,
        },
        Source {
            covers: "L_vap",
            citation: "8.73145 kJ mol⁻¹, h″ − h′ = −438.50 + 982.76 kJ kg⁻¹ at 90.694 K \
                       (Setzmann and Wagner 1991, Table 39), at M = 16.0428 g mol⁻¹",
            basis: Basis::Reduced,
        },
        Source {
            covers: "L_fus",
            citation: "0.9392 kJ mol⁻¹, Vogt and Pitzer 1976 (J. Chem. Thermodyn. 8, 1011), as \
                       NIST's WebBook lists it and the held phase columns cite it; the anchored \
                       solid branch's slope implies 0.935",
            basis: Basis::Fact,
        },
    ],
};

/// Ammonia: Gao, Wu, Bell, Harvey and Lemmon 2023.
pub(in crate::substance) const AMMONIA: PhaseDiagram = PhaseDiagram {
    triple_temperature: Kelvin::new(195.49),
    triple_pressure: Pascals::new(6_053.39),
    critical_temperature: Kelvin::new(405.56),
    critical_pressure: Pascals::new(11.3634e6),
    vapour_pressure: WagnerEquation {
        reducing_temperature: Kelvin::new(405.56),
        reducing_pressure: Pascals::new(11.3634e6),
        terms: &[
            (-7.3128, 1.0),
            (3.8888, 1.5),
            (-2.9908, 1.6),
            (-2.8636, 3.7),
        ],
    },
    liquid_density: DensityEquation {
        form: DensityForm::Linear,
        reducing_temperature: Kelvin::new(405.56),
        reducing_density: KilogramsPerCubicMetre::new(13.696 * 17.030_52),
        terms: &[
            (0.051_236, 0.07),
            (3.7925, 0.46),
            (-3.5929, 0.77),
            (4.6409, 1.05),
            (-1.9893, 1.25),
            (1.5978, 8.0),
        ],
    },
    sublimation: &[SublimationPiece {
        phase: "I",
        lower: Kelvin::new(162.0),
        equation: SublimationEquation::InversePowers {
            unit: MEGAPASCAL,
            coefficients: &[13.639_32, -3_537.0, -33_100.0, 1_742_000.0, -29_950_000.0],
        },
    }],
    anchors: &saturation::NH3_ANCHORS,
    melting: Some(MeltingCurve::Rising(&[
        MeltingPiece {
            start: Kelvin::new(195.49),
            end: None,
            equation: MeltingEquation::SimonGlatzel {
                temperature: Kelvin::new(195.49),
                pressure: Pascals::new(6_053.39),
                terms: &[(516.613_763e6, 4.4)],
            },
        },
        MeltingPiece {
            start: Kelvin::new(217.35),
            end: None,
            equation: MeltingEquation::SimonGlatzel {
                temperature: Kelvin::new(217.35),
                pressure: Pascals::new(307e6),
                terms: &[(1_135e6, 1.51)],
            },
        },
    ])),
    solids: &[SolidPhase {
        name: "I",
        start: Kelvin::ZERO,
        density: Some(SolidDensity::Fixed {
            density: KilogramsPerCubicMetre::new(822.0),
            at: Some(Kelvin::new(195.49)),
        }),
    }],
    triple_vaporisation_enthalpy_j_per_mol: 25_359.65,
    triple_fusion_enthalpy_j_per_mol: Some(5_655.1),
    sources: &[
        Source {
            covers: "T_t, p_t, T_c, p_c",
            citation: "195.49 K (Filipe et al.) and 6.05339 kPa, and 405.56 K and 11.3634 MPa, \
                       Gao, Wu, Bell, Harvey and Lemmon 2023 (JPCRD 52, 013102, Table 1; read in \
                       NIST's author manuscript), the published equation REFPROP 10.0 cited as \
                       \"to be submitted, 2018\"",
            basis: Basis::Fact,
        },
        Source {
            covers: "p_liquid, ρ_l",
            citation: "Gao et al. 2023's ancillary vapour-pressure and saturated-liquid-density \
                       equations (Sec. 2.2), within 0.02% and 0.04% of their equation of state; \
                       the reducing density 13.696 mol dm⁻³ at M = 17.03052 g mol⁻¹",
            basis: Basis::Fact,
        },
        Source {
            covers: "p_solid",
            citation: "Gao et al. 2023, Sec. 2.3: Fray and Schmitt 2009's ammonia expression \
                       (from Overstreet and Giauque's c_p of the solid and enthalpy of \
                       sublimation), its A₀ set to meet their triple point, ln(p ÷ 1 MPa); 0.8–3.5% \
                       in mean absolute deviation from the five data sets of 162–195 K (their \
                       Table 3), from 162 K",
            basis: Basis::Fact,
        },
        ANCHORED,
        Source {
            covers: "p_melt",
            citation: "Gao et al. 2023, Sec. 2.3: ammonia I's Simon–Glatzel line to the I–II–liquid \
                       point at 217.35 K and 307 MPa (Mills et al.), then their modification of \
                       Ninet and Datchi 2008's for phases II and III, fitted to data to 904 K; \
                       extrapolated beyond",
            basis: Basis::Fact,
        },
        Source {
            covers: "ρ_s",
            citation: "822 ± 5 kg m⁻³ at the triple point, Kiefte, Breckon, Penney and Clouter \
                       1985's recommendation (J. Chem. Phys. 83, 4738), as Gao et al. 2023, Sec. \
                       2.3 take it for their melting curve's slope; Olovsson and Templeton 1959's \
                       cubic cell, a = 5.138 Å and Z = 4 (Acta Cryst. 12, 832; COD 2310927), gives \
                       834.0 at a temperature not read, the check",
            basis: Basis::Fact,
        },
        Source {
            covers: "L_vap",
            citation: "25.35965 kJ mol⁻¹, Clapeyron's T Δv dp ÷ dT at the triple point from Gao et \
                       al.'s ancillary vapour-pressure equation and Table 1's saturated densities \
                       (the paper prints no saturation table)",
            basis: Basis::Reduced,
        },
        Source {
            covers: "L_fus",
            citation: "5.6551 kJ mol⁻¹ near the triple point, Overstreet and Giauque 1937 (JACS \
                       59, 254), as Gao et al. 2023, Sec. 2.3 quote it",
            basis: Basis::Fact,
        },
    ],
};

/// Nitrogen: Span, Lemmon, Jacobsen, Wagner and Yokozeki 2000, with Fray and Schmitt's solids and
/// Maynard-Casely et al.'s lattices.
pub(in crate::substance) const NITROGEN: PhaseDiagram = PhaseDiagram {
    triple_temperature: Kelvin::new(63.151),
    triple_pressure: Pascals::new(12_523.0),
    critical_temperature: Kelvin::new(126.192),
    critical_pressure: Pascals::new(3.3958e6),
    vapour_pressure: WagnerEquation {
        reducing_temperature: Kelvin::new(126.192),
        reducing_pressure: Pascals::new(3.3958e6),
        terms: &[
            (-6.124_452_84, 1.0),
            (1.263_272_2, 1.5),
            (-0.765_910_082, 2.5),
            (-1.775_705_64, 5.0),
        ],
    },
    liquid_density: DensityEquation {
        form: DensityForm::Logarithmic,
        reducing_temperature: Kelvin::new(126.192),
        reducing_density: KilogramsPerCubicMetre::new(313.3),
        terms: &[
            (1.486_542_37, 0.3294),
            (-0.280_476_066, 2.0 / 3.0),
            (0.089_414_308_5, 8.0 / 3.0),
            (-0.119_879_866, 35.0 / 6.0),
        ],
    },
    sublimation: &[
        SublimationPiece {
            phase: "β",
            lower: Kelvin::new(35.61),
            equation: SublimationEquation::InversePowers {
                unit: BAR,
                coefficients: &[8.514, -4.584e2, -1.987e4, 4.800e5, -4.524e6],
            },
        },
        SublimationPiece {
            phase: "α",
            lower: Kelvin::new(10.0),
            equation: SublimationEquation::InversePowers {
                unit: BAR,
                coefficients: &[12.40, -8.074e2, -3.926e3, 6.297e4, -4.633e5, 1.325e6],
            },
        },
    ],
    anchors: &saturation::N2_ANCHORS,
    melting: Some(MeltingCurve::Rising(&[MeltingPiece {
        start: Kelvin::new(63.151),
        end: None,
        equation: MeltingEquation::Simon {
            temperature: Kelvin::new(63.151),
            pressure: Pascals::new(12_523.0),
            terms: &[(12_798.61, 1.789_63)],
        },
    }])),
    solids: &[
        SolidPhase {
            name: "α",
            start: Kelvin::ZERO,
            density: Some(SolidDensity::CellVolume {
                formula_units: 4.0,
                molar_mass_g_per_mol: 28.014,
                volume_cubic_angstrom: &[179.484, 0.0, 0.0, 8.51e-5],
            }),
        },
        SolidPhase {
            name: "β",
            start: Kelvin::new(35.61),
            density: Some(SolidDensity::CellVolume {
                formula_units: 2.0,
                molar_mass_g_per_mol: 28.014,
                volume_cubic_angstrom: &[86.5, 0.170],
            }),
        },
    ],
    triple_vaporisation_enthalpy_j_per_mol: 6_037.3,
    triple_fusion_enthalpy_j_per_mol: None,
    sources: &[
        Source {
            covers: "T_t, p_t, T_c, p_c",
            citation: "63.151 K and 12.523 kPa, and 126.192 K and 3.3958 MPa (Span, Lemmon, \
                       Jacobsen, Wagner and Yokozeki 2000, JPCRD 29, 1361, Eqs. 2 and 3)",
            basis: Basis::Fact,
        },
        Source {
            covers: "p_liquid, ρ_l",
            citation: "Span et al. 2000, Eqs. (4) and (5), Nowak et al. 1997's ancillaries, within \
                       the data's 0.015% (0.075% at the triple point)",
            basis: Basis::Fact,
        },
        Source {
            covers: "p_solid",
            citation: "β-nitrogen above the α–β transition at 35.61 K (Brookeman and Scott 1973, as \
                       Raposa et al. 2024, PSJ 5, 275, Table A3 quote it) and α below it, from 10 \
                       K, Fray and Schmitt's N2 polynomials; the β polynomial runs \
                       2.8% below the liquid branch at the triple point before its anchor, and its \
                       slope there implies an enthalpy of fusion of about 530 J mol⁻¹, against \
                       about 870 by Clapeyron from Span et al.'s Eq. (7), the β cell and ρ′",
            basis: Basis::Fact,
        },
        FRAY_SCHMITT,
        ANCHORED,
        Source {
            covers: "p_melt",
            citation: "Span et al. 2000, Eq. (7), fitted to data to 273 K and 2,040 MPa; \
                       extrapolated beyond",
            basis: Basis::Fact,
        },
        Source {
            covers: "ρ_s",
            citation: "the cell volumes of α (Z = 4) and β (Z = 2) nitrogen from neutron \
                       diffraction, Maynard-Casely, Hester and Brand 2020 (IUCrJ 7, 844, Sec. \
                       3.2.1 and Fig. 4b), a 0.96% step at the transition",
            basis: Basis::Fact,
        },
        Source {
            covers: "L_vap",
            citation: "6.0373 kJ mol⁻¹, h″ − h′ = 1,814.7 + 4,222.6 J mol⁻¹ at 63.151 K (Span et \
                       al. 2000, the saturation table, p. 1403)",
            basis: Basis::Reduced,
        },
    ],
};

/// Oxygen: Schmidt and Wagner 1985's equation as Stewart, Jacobsen and Wagner 1991 present it (on
/// IPTS-68), with Fray and Schmitt's solids.
pub(in crate::substance) const OXYGEN: PhaseDiagram = PhaseDiagram {
    triple_temperature: Kelvin::new(54.361),
    triple_pressure: Pascals::new(146.33),
    critical_temperature: Kelvin::new(154.581),
    critical_pressure: Pascals::new(5.043e6),
    vapour_pressure: WagnerEquation {
        reducing_temperature: Kelvin::new(154.581),
        reducing_pressure: Pascals::new(5.043e6),
        terms: &[
            (-6.043_938, 1.0),
            (1.175_627, 1.5),
            (-0.994_086, 3.0),
            (-3.456_781, 7.0),
            (3.361_499, 9.0),
        ],
    },
    liquid_density: DensityEquation {
        form: DensityForm::Linear,
        reducing_temperature: Kelvin::new(154.581),
        reducing_density: KilogramsPerCubicMetre::new(13.63 * 31.9988),
        terms: &[
            (1.507_678, 1.0 / 3.0),
            (0.858_108_05, 2.0 / 3.0),
            (0.190_355_04, 3.0),
        ],
    },
    sublimation: &[
        SublimationPiece {
            phase: "γ",
            lower: Kelvin::new(43.78),
            equation: SublimationEquation::InversePowers {
                unit: BAR,
                coefficients: &[10.18, -8.035e2, -7.080e3, 7.553e4],
            },
        },
        SublimationPiece {
            phase: "β",
            lower: Kelvin::new(23.14),
            equation: SublimationEquation::InversePowers {
                unit: BAR,
                coefficients: &[13.35, -1.012e3, -2.971e3, 2.926e4],
            },
        },
        SublimationPiece {
            phase: "α",
            lower: Kelvin::new(10.0),
            equation: SublimationEquation::InversePowers {
                unit: BAR,
                coefficients: &[15.41, -1.148e3, 3.349e2, 6.021e1],
            },
        },
    ],
    anchors: &saturation::O2_ANCHORS,
    melting: None,
    solids: &[
        SolidPhase {
            name: "α",
            start: Kelvin::ZERO,
            density: Some(SolidDensity::Fixed {
                density: KilogramsPerCubicMetre::new(1_530.39),
                at: None,
            }),
        },
        SolidPhase {
            name: "β",
            start: Kelvin::new(23.14),
            density: None,
        },
        SolidPhase {
            name: "γ",
            start: Kelvin::new(43.78),
            density: Some(SolidDensity::Fixed {
                density: KilogramsPerCubicMetre::new(1_363.87),
                at: Some(Kelvin::new(46.0)),
            }),
        },
    ],
    triple_vaporisation_enthalpy_j_per_mol: 7_766.8,
    triple_fusion_enthalpy_j_per_mol: None,
    sources: &[
        Source {
            covers: "T_t, p_t, T_c, p_c",
            citation: "54.361 K and 146.33 Pa (Wagner, Ewers and Pentermann 1976), and 154.581 K \
                       and 5.043 MPa (Weber), on IPTS-68, as Stewart, Jacobsen and Wagner 1991 \
                       (JPCRD 20, 917, Secs. 2.5–2.6) give Schmidt and Wagner 1985's constants; \
                       Schmidt and Wagner's own paper was not reachable",
            basis: Basis::Fact,
        },
        Source {
            covers: "p_liquid, ρ_l",
            citation: "Stewart et al. 1991, Eq. (7), Wagner, Ewers and Pentermann 1976's \
                       vapour-pressure equation, and Eq. (10), Schmidt's saturated-liquid density \
                       (the reducing density 13.63 mol dm⁻³ at M = 31.9988 g mol⁻¹); Stewart et \
                       al.'s Table 10 as the check",
            basis: Basis::Fact,
        },
        Source {
            covers: "p_solid",
            citation: "γ-, β- and α-oxygen, Fray and Schmitt's three O2 polynomials, \
                       from 10 K; the transitions unread, the pieces meet where \
                       their polynomials cross, 23.14 and 43.78 K",
            basis: Basis::Fact,
        },
        FRAY_SCHMITT,
        ANCHORED,
        Source {
            covers: "ρ_s",
            citation: "α: 1,530.39 kg m⁻³ from Barrett, Meyer and Wasserman 1967's monoclinic cell \
                       (J. Chem. Phys. 47, 592; COD 1528336), its temperature not read; γ: \
                       1,363.87 kg m⁻³ at 46 K from Cox, Samuelsen and Beckurts 1973's neutron \
                       cell, a = 6.78 Å, Z = 8 (Phys. Rev. B 7, 3102; COD 9011647); β's not read",
            basis: Basis::Reduced,
        },
        Source {
            covers: "L_vap",
            citation: "7.7668 kJ mol⁻¹, h″ − h′ = 1,573.4 + 6,193.4 J mol⁻¹ at 54.361 K (Stewart \
                       et al. 1991, Table 10)",
            basis: Basis::Reduced,
        },
    ],
};

/// Carbon dioxide: Span and Wagner 1996, with Fray and Schmitt's solid below Span and Wagner's
/// sublimation equation and Mangan et al.'s density.
pub(in crate::substance) const CARBON_DIOXIDE: PhaseDiagram = PhaseDiagram {
    triple_temperature: Kelvin::new(216.592),
    triple_pressure: Pascals::new(517_950.0),
    critical_temperature: Kelvin::new(304.1282),
    critical_pressure: Pascals::new(7.3773e6),
    vapour_pressure: WagnerEquation {
        reducing_temperature: Kelvin::new(304.1282),
        reducing_pressure: Pascals::new(7.3773e6),
        terms: &[
            (-7.060_208_7, 1.0),
            (1.939_121_8, 1.5),
            (-1.646_359_7, 2.0),
            (-3.299_563_4, 4.0),
        ],
    },
    liquid_density: DensityEquation {
        form: DensityForm::Logarithmic,
        reducing_temperature: Kelvin::new(304.1282),
        reducing_density: KilogramsPerCubicMetre::new(467.6),
        terms: &[
            (1.924_510_8, 0.34),
            (-0.623_855_55, 0.5),
            (-0.327_311_27, 10.0 / 6.0),
            (0.392_451_42, 11.0 / 6.0),
        ],
    },
    sublimation: &[
        SublimationPiece {
            phase: "I",
            lower: Kelvin::new(154.0),
            equation: SublimationEquation::Reduced(WagnerEquation {
                reducing_temperature: Kelvin::new(216.592),
                reducing_pressure: Pascals::new(517_950.0),
                terms: &[(-14.740_846, 1.0), (2.432_701_5, 1.9), (-5.306_177_8, 2.9)],
            }),
        },
        SublimationPiece {
            phase: "I",
            lower: Kelvin::new(40.0),
            equation: SublimationEquation::InversePowers {
                unit: BAR,
                coefficients: &[14.76, -2.571e3, -7.781e4, 4.325e6, -1.207e8, 1.350e9],
            },
        },
    ],
    anchors: &saturation::CO2_ANCHORS,
    melting: Some(MeltingCurve::Rising(&[MeltingPiece {
        start: Kelvin::new(216.592),
        end: None,
        equation: MeltingEquation::Polynomial {
            temperature: Kelvin::new(216.592),
            pressure: Pascals::new(517_950.0),
            terms: &[(1_955.539, 1.0), (2_055.459_3, 2.0)],
        },
    }])),
    solids: &[SolidPhase {
        name: "I",
        start: Kelvin::ZERO,
        density: Some(SolidDensity::Polynomial {
            coefficients: &[1_723.91, -0.253, -2.87e-3],
            held_below: Kelvin::new(80.0),
        }),
    }],
    triple_vaporisation_enthalpy_j_per_mol: 15_419.98,
    triple_fusion_enthalpy_j_per_mol: None,
    sources: &[
        Source {
            covers: "T_t, p_t, T_c, p_c",
            citation: "216.592 K and 0.51795 MPa, and 304.1282 K and 7.3773 MPa (Span and Wagner \
                       1996, JPCRD 25, 1509, Eqs. 3.1–3.2 and 3.5–3.7)",
            basis: Basis::Fact,
        },
        Source {
            covers: "p_liquid, ρ_l",
            citation: "Span and Wagner 1996, Eqs. (3.13), within 0.012%, and (3.14), within \
                       0.015% from the triple point to 295 K",
            basis: Basis::Fact,
        },
        Source {
            covers: "p_solid",
            citation: "Span and Wagner 1996, Eq. (3.12), fitted above 154 K (±250 Pa near the \
                       triple point, ±50 Pa below 170 K; it passes 101,325 Pa at 194.6855 K, Sec. \
                       3.4); below 154 K, to 40 K, Fray and Schmitt's CO2 polynomial (\
                       40–194.7 K as Schorghofer's Planetary-Code-Collection quotes it), which runs \
                       0.35% below Eq. (3.12) at 154 K",
            basis: Basis::Fact,
        },
        FRAY_SCHMITT,
        ANCHORED,
        Source {
            covers: "p_melt",
            citation: "Span and Wagner 1996, Eq. (3.10), within 1.5% to 225 K and 0.5% to 270 K; \
                       extrapolated beyond",
            basis: Basis::Fact,
        },
        Source {
            covers: "ρ_s",
            citation: "1,723.91 − 0.253 T − 0.00287 T² kg m⁻³, Mangan, Salzmann, Plane and Murray \
                       2017 (Icarus 294, 201, Eq. 1, in g cm⁻³) over 80–195 K: held below 80 K and \
                       extrapolated above 195 K to the triple point",
            basis: Basis::Fact,
        },
        Source {
            covers: "L_vap",
            citation: "15.41998 kJ mol⁻¹, h″ − h′ = −76.364 + 426.74 kJ kg⁻¹ at 216.592 K (Span \
                       and Wagner 1996, Table 34), at M = 44.0098 g mol⁻¹",
            basis: Basis::Reduced,
        },
    ],
};

/// Argon: Tegeler, Span and Wagner 1999.
pub(in crate::substance) const ARGON: PhaseDiagram = PhaseDiagram {
    triple_temperature: Kelvin::new(83.8058),
    triple_pressure: Pascals::new(68_891.0),
    critical_temperature: Kelvin::new(150.687),
    critical_pressure: Pascals::new(4.863e6),
    vapour_pressure: WagnerEquation {
        reducing_temperature: Kelvin::new(150.687),
        reducing_pressure: Pascals::new(4.863e6),
        terms: &[
            (-5.940_978_5, 1.0),
            (1.355_388_8, 1.5),
            (-0.464_976_07, 2.0),
            (-1.539_904_3, 4.5),
        ],
    },
    liquid_density: DensityEquation {
        form: DensityForm::Logarithmic,
        reducing_temperature: Kelvin::new(150.687),
        reducing_density: KilogramsPerCubicMetre::new(535.6),
        terms: &[
            (1.500_426_2, 0.334),
            (-0.313_812_9, 2.0 / 3.0),
            (0.086_461_622, 7.0 / 3.0),
            (-0.041_477_525, 4.0),
        ],
    },
    sublimation: &[SublimationPiece {
        phase: "fcc",
        lower: Kelvin::new(25.0),
        equation: SublimationEquation::Reduced(WagnerEquation {
            reducing_temperature: Kelvin::new(83.8058),
            reducing_pressure: Pascals::new(68_891.0),
            terms: &[(-11.391_604, 1.0), (-0.395_134_31, 2.7)],
        }),
    }],
    anchors: &saturation::AR_ANCHORS,
    melting: Some(MeltingCurve::Rising(&[MeltingPiece {
        start: Kelvin::new(83.8058),
        end: None,
        equation: MeltingEquation::Simon {
            temperature: Kelvin::new(83.8058),
            pressure: Pascals::new(68_891.0),
            terms: &[(-7_476.266_5, 1.05), (9_959.061_3, 1.275)],
        },
    }])),
    solids: &[SolidPhase {
        name: "fcc",
        start: Kelvin::ZERO,
        density: None,
    }],
    triple_vaporisation_enthalpy_j_per_mol: 6_539.89,
    triple_fusion_enthalpy_j_per_mol: None,
    sources: &[
        Source {
            covers: "T_t, p_t, T_c, p_c",
            citation: "83.8058 K, the ITS-90 fixed point, and 68.891 kPa, and 150.687 K and 4.863 \
                       MPa (Tegeler, Span and Wagner 1999, JPCRD 28, 779, Eqs. 2.1–2.5)",
            basis: Basis::Fact,
        },
        Source {
            covers: "p_liquid, ρ_l",
            citation: "Tegeler et al. 1999, Eqs. (2.9) and (2.10), Gilgen et al. 1994's \
                       equations, within their data's 0.01–0.04% and 0.015%",
            basis: Basis::Fact,
        },
        Source {
            covers: "p_solid",
            citation: "Tegeler et al. 1999, Eq. (2.8), within about 0.05% of Chen et al.'s data; \
                       from 25 K, its lowest data",
            basis: Basis::Fact,
        },
        ANCHORED,
        Source {
            covers: "p_melt",
            citation: "Tegeler et al. 1999, Eq. (2.7), fitted to Hardy et al. 1971 over 84–273 K \
                       and reasonable to 717 K; extrapolated beyond",
            basis: Basis::Fact,
        },
        Source {
            covers: "L_vap",
            citation: "6.53989 kJ mol⁻¹, h″ − h′ = −112.85 + 276.56 kJ kg⁻¹ at 83.8058 K \
                       (Tegeler et al. 1999, Table 33), at M = 39.948 g mol⁻¹",
            basis: Basis::Reduced,
        },
    ],
};

/// Krypton: Lemmon and Span 2006, through CoolProp.
pub(in crate::substance) const KRYPTON: PhaseDiagram = PhaseDiagram {
    triple_temperature: Kelvin::new(115.77),
    triple_pressure: Pascals::new(73_502.837_48),
    critical_temperature: Kelvin::new(209.48),
    critical_pressure: Pascals::new(5.525e6),
    vapour_pressure: WagnerEquation {
        reducing_temperature: Kelvin::new(209.48),
        reducing_pressure: Pascals::new(5.525e6),
        terms: &[
            (0.628_638_039_478_556_6, 0.822),
            (-6.084_114_621_397_644, 0.961),
            (0.924_363_766_928_452_3, 1.811),
            (-1.217_729_162_331_677_5, 2.937),
            (-484.840_850_752_389_77, 13.131),
            (2_458.926_650_746_003, 15.629),
        ],
    },
    liquid_density: DensityEquation {
        form: DensityForm::Linear,
        reducing_temperature: Kelvin::new(209.48),
        reducing_density: KilogramsPerCubicMetre::new(10_850.0 * 0.083_798),
        terms: &[
            (2.546_150_113_785_161, 0.423),
            (-5.378_194_265_123_711, 1.611),
            (8.373_146_326_331_726, 2.063),
            (-116.812_799_011_709_06, 6.632),
            (18_068.951_065_200_818, 12.833),
            (-1_719_409.740_660_462, 19.438),
        ],
    },
    sublimation: &[SublimationPiece {
        phase: "fcc",
        lower: Kelvin::new(20.0),
        equation: SublimationEquation::InversePowers {
            unit: BAR,
            coefficients: &[
                10.77, -1.223e3, -8.903e3, 2.635e5, -4.260e6, 3.575e7, -1.210e8,
            ],
        },
    }],
    anchors: &saturation::KR_ANCHORS,
    melting: None,
    solids: &[SolidPhase {
        name: "fcc",
        start: Kelvin::ZERO,
        density: Some(SolidDensity::Fixed {
            density: KilogramsPerCubicMetre::new(2_972.53),
            at: Some(Kelvin::new(58.0)),
        }),
    }],
    triple_vaporisation_enthalpy_j_per_mol: 9_083.644,
    triple_fusion_enthalpy_j_per_mol: None,
    sources: &[
        Source {
            covers: "T_t, p_t, T_c, p_c",
            citation: "115.77 K and 209.48 K, 5.525 MPa, Lemmon and Span 2006 (J. Chem. Eng. Data \
                       51, 785, not reached) as CoolProp reads them, and p_t, 73,502.837 Pa, \
                       CoolProp's triple-point state of their equation",
            basis: Basis::Raw { licence: "MIT" },
        },
        Source {
            covers: "p_liquid, ρ_l",
            citation: "CoolProp's ancillaries, Bell's fits to Lemmon and Span's equation of \
                       state, within 0.0091% and 0.83% of it; the reducing density 10,850 mol m⁻³ \
                       at M = 83.798 g mol⁻¹",
            basis: Basis::Raw { licence: "MIT" },
        },
        Source {
            covers: "p_solid",
            citation: "Fray and Schmitt's Kr polynomial (Lisse et al. print its A₆ \
                       as \"-s1.210\", read −1.210 × 10⁸), from 20 K",
            basis: Basis::Fact,
        },
        COOLPROP,
        FRAY_SCHMITT,
        ANCHORED,
        Source {
            covers: "ρ_s",
            citation: "2,972.53 kg m⁻³ at 58 K, the fcc cell a = 5.721 Å (Wyckoff 1963, Crystal \
                       Structures 1, 7; COD 9008471)",
            basis: Basis::Reduced,
        },
        Source {
            covers: "L_vap",
            citation: "9.083644 kJ mol⁻¹, h″ − h′ of CoolProp's triple-point states of Lemmon and \
                       Span's equation",
            basis: Basis::Raw { licence: "MIT" },
        },
    ],
};

/// Xenon: Lemmon and Span 2006, through CoolProp.
pub(in crate::substance) const XENON: PhaseDiagram = PhaseDiagram {
    triple_temperature: Kelvin::new(161.4),
    triple_pressure: Pascals::new(81_747.799_07),
    critical_temperature: Kelvin::new(289.733),
    critical_pressure: Pascals::new(5.842e6),
    vapour_pressure: WagnerEquation {
        reducing_temperature: Kelvin::new(289.733),
        reducing_pressure: Pascals::new(5.842e6),
        terms: &[
            (-16.196_250_750_396_99, 1.057),
            (11.578_285_552_184_98, 1.12),
            (-0.803_972_233_808_915_7, 3.837),
            (-0.448_712_609_233_083_76, 4.253),
            (-3.294_390_183_323_457_7, 9.273),
            (-0.388_714_761_002_593_06, 1.349),
        ],
    },
    liquid_density: DensityEquation {
        form: DensityForm::Linear,
        reducing_temperature: Kelvin::new(289.733),
        reducing_density: KilogramsPerCubicMetre::new(8_400.0 * 0.131_293),
        terms: &[
            (-0.077_014_257_351_918_12, 0.084),
            (2.124_910_237_054_726, 0.346),
            (3.020_526_068_066_373_2, 2.447),
            (-192.083_484_729_982_13, 5.49),
            (269.924_532_194_783_8, 6.019),
            (-15_087.387_586_364_559, 15.158),
        ],
    },
    sublimation: &[SublimationPiece {
        phase: "fcc",
        lower: Kelvin::new(20.0),
        equation: SublimationEquation::InversePowers {
            unit: BAR,
            coefficients: &[10.98, -1.737e3, -1.332e4, 4.349e5, -7.027e6, 4.447e7],
        },
    }],
    anchors: &saturation::XE_ANCHORS,
    melting: None,
    solids: &[SolidPhase {
        name: "fcc",
        start: Kelvin::ZERO,
        density: Some(SolidDensity::Measured(&[
            (Kelvin::new(5.0), KilogramsPerCubicMetre::new(3_782.57)),
            (Kelvin::new(10.0), KilogramsPerCubicMetre::new(3_781.27)),
            (Kelvin::new(15.0), KilogramsPerCubicMetre::new(3_778.50)),
            (Kelvin::new(20.0), KilogramsPerCubicMetre::new(3_774.07)),
            (Kelvin::new(25.0), KilogramsPerCubicMetre::new(3_767.99)),
            (Kelvin::new(35.0), KilogramsPerCubicMetre::new(3_751.28)),
            (Kelvin::new(40.0), KilogramsPerCubicMetre::new(3_741.41)),
            (Kelvin::new(45.0), KilogramsPerCubicMetre::new(3_731.04)),
            (Kelvin::new(50.0), KilogramsPerCubicMetre::new(3_719.98)),
            (Kelvin::new(55.0), KilogramsPerCubicMetre::new(3_707.88)),
            (Kelvin::new(60.0), KilogramsPerCubicMetre::new(3_695.47)),
            (Kelvin::new(65.0), KilogramsPerCubicMetre::new(3_682.40)),
            (Kelvin::new(70.0), KilogramsPerCubicMetre::new(3_668.87)),
            (Kelvin::new(75.0), KilogramsPerCubicMetre::new(3_655.04)),
        ])),
    }],
    triple_vaporisation_enthalpy_j_per_mol: 12_657.589,
    triple_fusion_enthalpy_j_per_mol: None,
    sources: &[
        Source {
            covers: "T_t, p_t, T_c, p_c",
            citation: "161.4 K and 289.733 K, 5.842 MPa, Lemmon and Span 2006 (not reached) as \
                       CoolProp reads them, and p_t, 81,747.799 Pa, CoolProp's triple-point state \
                       of their equation",
            basis: Basis::Raw { licence: "MIT" },
        },
        Source {
            covers: "p_liquid, ρ_l",
            citation: "CoolProp's ancillaries, Bell's fits to Lemmon and Span's equation of \
                       state, within 0.0033% and 0.44% of it; the reducing density 8,400 mol m⁻³ \
                       at M = 131.293 g mol⁻¹",
            basis: Basis::Raw { licence: "MIT" },
        },
        Source {
            covers: "p_solid",
            citation: "Fray and Schmitt's Xe polynomial, from 20 K",
            basis: Basis::Fact,
        },
        COOLPROP,
        FRAY_SCHMITT,
        ANCHORED,
        Source {
            covers: "ρ_s",
            citation: "the fcc cells of Sears and Klug 1962 (J. Chem. Phys. 37, 3002; COD \
                       9012450–9012464) at 5–75 K, interpolated and held above 75 K, their 30 K \
                       cell, 0.25% out of sequence, left out",
            basis: Basis::Reduced,
        },
        Source {
            covers: "L_vap",
            citation: "12.657589 kJ mol⁻¹, h″ − h′ of CoolProp's triple-point states of Lemmon and \
                       Span's equation",
            basis: Basis::Raw { licence: "MIT" },
        },
    ],
};

/// Nitrous oxide: Lemmon and Span 2006, through CoolProp.
pub(in crate::substance) const NITROUS_OXIDE: PhaseDiagram = PhaseDiagram {
    triple_temperature: Kelvin::new(182.33),
    triple_pressure: Pascals::new(87_837.439_23),
    critical_temperature: Kelvin::new(309.52),
    critical_pressure: Pascals::new(7.245e6),
    vapour_pressure: WagnerEquation {
        reducing_temperature: Kelvin::new(309.52),
        reducing_pressure: Pascals::new(7.245e6),
        terms: &[
            (-24.198_485_663_914_695, 1.06),
            (18.980_157_921_169_518, 1.105),
            (-0.991_563_758_476_008_2, 1.754),
            (-3.605_225_645_253_564_7, 4.415),
            (52.201_261_130_558_01, 9.887),
            (-340.239_546_388_968_5, 12.55),
        ],
    },
    liquid_density: DensityEquation {
        form: DensityForm::Linear,
        reducing_temperature: Kelvin::new(309.52),
        reducing_density: KilogramsPerCubicMetre::new(10_270.0 * 0.044_012_8),
        terms: &[
            (2.248_014_962_727_216, 0.374),
            (1_354.881_440_798_547_6, 6.796),
            (-227_009_237.431_836_37, 13.027),
            (312_935_961.072_078_7, 13.205),
            (-139_155_040.774_357_6, 14.362),
            (295_107_831.042_958_5, 18.638),
        ],
    },
    sublimation: &[SublimationPiece {
        phase: "cubic",
        lower: Kelvin::new(140.0),
        equation: SublimationEquation::InversePowers {
            unit: BAR,
            coefficients: &[16.22, -2.971e3],
        },
    }],
    anchors: &saturation::N2O_ANCHORS,
    melting: None,
    solids: &[SolidPhase {
        name: "cubic",
        start: Kelvin::ZERO,
        density: Some(SolidDensity::Fixed {
            density: KilogramsPerCubicMetre::new(1_562.08),
            at: None,
        }),
    }],
    triple_vaporisation_enthalpy_j_per_mol: 16_589.429,
    triple_fusion_enthalpy_j_per_mol: None,
    sources: &[
        Source {
            covers: "T_t, p_t, T_c, p_c",
            citation: "182.33 K and 309.52 K, 7.245 MPa, Lemmon and Span 2006 (not reached) as \
                       CoolProp reads them, and p_t, 87,837.439 Pa, CoolProp's triple-point state \
                       of their equation",
            basis: Basis::Raw { licence: "MIT" },
        },
        Source {
            covers: "p_liquid, ρ_l",
            citation: "CoolProp's ancillaries, Bell's fits to Lemmon and Span's equation of \
                       state, within 0.0068% and 0.68% of it; the reducing density 10,270 mol m⁻³ \
                       at M = 44.0128 g mol⁻¹",
            basis: Basis::Raw { licence: "MIT" },
        },
        Source {
            covers: "p_solid",
            citation: "Fray and Schmitt's two-term N2O polynomial, from 140 K: it \
                       runs 5.7% above the liquid branch at the triple point before its anchor; \
                       their second N2O polynomial is unphysical as Lisse et al. print it",
            basis: Basis::Fact,
        },
        COOLPROP,
        FRAY_SCHMITT,
        ANCHORED,
        Source {
            covers: "ρ_s",
            citation: "1,562.08 kg m⁻³, the cubic cell a = 5.72 Å, Z = 4, of de Smedt and Keesom \
                       1924 (Proc. KNAW 27, 839; COD 1010059), its temperature not read",
            basis: Basis::Reduced,
        },
        Source {
            covers: "L_vap",
            citation: "16.589429 kJ mol⁻¹, h″ − h′ of CoolProp's triple-point states of Lemmon and \
                       Span's equation",
            basis: Basis::Raw { licence: "MIT" },
        },
    ],
};

/// Carbon monoxide: Lemmon and Span 2006, through CoolProp.
pub(in crate::substance) const CARBON_MONOXIDE: PhaseDiagram = PhaseDiagram {
    triple_temperature: Kelvin::new(68.16),
    triple_pressure: Pascals::new(15_536.876_66),
    critical_temperature: Kelvin::new(132.86),
    critical_pressure: Pascals::new(3.494e6),
    vapour_pressure: WagnerEquation {
        reducing_temperature: Kelvin::new(132.86),
        reducing_pressure: Pascals::new(3.494e6),
        terms: &[
            (0.003_165_559_471_116_198_2, 0.052),
            (-5.386_279_334_978_247, 0.968),
            (-0.440_894_361_777_012_97, 1.972),
            (1.540_340_181_748_666_2, 2.761),
            (-5.692_323_589_833_758, 4.199),
            (3.030_819_590_672_28, 5.126),
        ],
    },
    liquid_density: DensityEquation {
        form: DensityForm::Linear,
        reducing_temperature: Kelvin::new(132.86),
        reducing_density: KilogramsPerCubicMetre::new(10_850.0 * 0.028_010_1),
        terms: &[
            (3.973_336_124_071_812, 0.44),
            (-3.262_687_458_485_425, 0.672),
            (3.349_943_207_065_930_3, 1.334),
            (-42.129_174_166_388_935, 3.874),
            (56.528_594_323_084_81, 4.413),
            (-110.700_227_561_206_96, 9.257),
        ],
    },
    sublimation: &[
        SublimationPiece {
            phase: "β",
            lower: Kelvin::new(61.585),
            equation: SublimationEquation::InversePowers {
                unit: BAR,
                coefficients: &[10.25, -7.482e2, -5.843e3, 3.939e4],
            },
        },
        SublimationPiece {
            phase: "α",
            lower: Kelvin::new(14.0),
            equation: SublimationEquation::InversePowers {
                unit: BAR,
                coefficients: &[10.43, -7.213e2, -1.074e4, 2.341e5, -2.392e6, 9.478e6],
            },
        },
    ],
    anchors: &saturation::CO_ANCHORS,
    melting: None,
    solids: &[
        SolidPhase {
            name: "α",
            start: Kelvin::ZERO,
            density: Some(SolidDensity::Fixed {
                density: KilogramsPerCubicMetre::new(1_042.55),
                at: None,
            }),
        },
        SolidPhase {
            name: "β",
            start: Kelvin::new(61.585),
            density: None,
        },
    ],
    triple_vaporisation_enthalpy_j_per_mol: 6_496.968,
    triple_fusion_enthalpy_j_per_mol: None,
    sources: &[
        Source {
            covers: "T_t, p_t, T_c, p_c",
            citation: "68.16 K and 132.86 K, 3.494 MPa, Lemmon and Span 2006 (not reached) as \
                       CoolProp reads them, and p_t, 15,536.877 Pa, CoolProp's triple-point state \
                       of their equation; the equation itself gives 3.4982 MPa at the critical \
                       density",
            basis: Basis::Raw { licence: "MIT" },
        },
        Source {
            covers: "p_liquid, ρ_l",
            citation: "CoolProp's ancillaries, Bell's fits to Lemmon and Span's equation of \
                       state, within 0.066% and 0.36% of it; the reducing density 10,850 mol m⁻³ \
                       at M = 28.0101 g mol⁻¹",
            basis: Basis::Raw { licence: "MIT" },
        },
        Source {
            covers: "p_solid",
            citation: "β-carbon monoxide above the α–β transition at 61.585 K (Morrison, Newsham \
                       and Weir 1968's 61.584–61.586 K, as Raposa et al. 2024, PSJ 5, 275, Table \
                       A3 quote it, at 3.74 kPa) and α below it, from 14 K, Fray and Schmitt's \
                       two CO polynomials",
            basis: Basis::Fact,
        },
        COOLPROP,
        FRAY_SCHMITT,
        ANCHORED,
        Source {
            covers: "ρ_s",
            citation: "α: 1,042.55 kg m⁻³, Vegard 1930's cubic cell a = 5.63 Å, Z = 4 (Z. Phys. \
                       61, 185; COD 1010305), its temperature not read; β's not read",
            basis: Basis::Reduced,
        },
        Source {
            covers: "L_vap",
            citation: "6.496968 kJ mol⁻¹, h″ − h′ of CoolProp's triple-point states of Lemmon and \
                       Span's equation",
            basis: Basis::Raw { licence: "MIT" },
        },
    ],
};

/// Sulphur dioxide: Gao, Wu, Zhang and Lemmon 2016, through CoolProp.
pub(in crate::substance) const SULPHUR_DIOXIDE: PhaseDiagram = PhaseDiagram {
    triple_temperature: Kelvin::new(197.7),
    triple_pressure: Pascals::new(1_666.144_079),
    critical_temperature: Kelvin::new(430.64),
    critical_pressure: Pascals::new(7.886_587_6e6),
    vapour_pressure: WagnerEquation {
        reducing_temperature: Kelvin::new(430.64),
        reducing_pressure: Pascals::new(7.8866e6),
        terms: &[
            (-7.303, 1.0),
            (1.9794, 1.5),
            (-2.078, 2.2),
            (-3.5446, 4.7),
            (0.517_76, 6.0),
        ],
    },
    liquid_density: DensityEquation {
        form: DensityForm::Linear,
        reducing_temperature: Kelvin::new(430.64),
        reducing_density: KilogramsPerCubicMetre::new(8_078.0 * 0.064_063_8),
        terms: &[
            (7.2296, 0.54),
            (-16.928, 0.88),
            (29.832, 1.23),
            (-27.901, 1.6),
            (11.085, 2.0),
        ],
    },
    sublimation: &[SublimationPiece {
        phase: "orthorhombic",
        lower: Kelvin::new(100.0),
        equation: SublimationEquation::InversePowers {
            unit: BAR,
            coefficients: &[15.60, -3.508e3, -9.401e4, 4.152e6, -6.946e7],
        },
    }],
    anchors: &saturation::SO2_ANCHORS,
    melting: None,
    solids: &[SolidPhase {
        name: "orthorhombic",
        start: Kelvin::ZERO,
        density: Some(SolidDensity::Fixed {
            density: KilogramsPerCubicMetre::new(1_921.94),
            at: Some(Kelvin::new(143.15)),
        }),
    }],
    triple_vaporisation_enthalpy_j_per_mol: 28_462.016,
    triple_fusion_enthalpy_j_per_mol: None,
    sources: &[
        Source {
            covers: "T_t, p_t, T_c, p_c",
            citation: "197.7 K and 430.64 K, Gao, Wu, Zhang and Lemmon 2016 (J. Chem. Eng. Data 61, \
                       2859, not reached) as CoolProp reads them, and p_t, 1,666.144 Pa, and p_c, \
                       7.8865876 MPa, CoolProp's triple-point and critical states of their \
                       equation",
            basis: Basis::Raw { licence: "MIT" },
        },
        Source {
            covers: "p_liquid, ρ_l",
            citation: "CoolProp's ancillaries, within 0.008% and 0.077% of the equation of state, \
                       whose rounded coefficients and reducing pressure of 7.8866 MPa suggest Gao \
                       et al.'s own; the reducing density 8,078 mol m⁻³ at M = 64.0638 g mol⁻¹",
            basis: Basis::Raw { licence: "MIT" },
        },
        Source {
            covers: "p_solid",
            citation: "Fray and Schmitt's SO2 polynomial (Lisse et al. print its A₁ \
                       as \"-3.5.08\", read −3.508 × 10³), from 100 K: it runs 3.8% above the \
                       liquid branch at the triple point before its anchor",
            basis: Basis::Fact,
        },
        COOLPROP,
        FRAY_SCHMITT,
        ANCHORED,
        Source {
            covers: "ρ_s",
            citation: "1,921.94 kg m⁻³ at 143.15 K, the orthorhombic cell of Post, Schwartz and \
                       Fankuchen 1952 (Acta Cryst. 5, 372; COD 9010972), Z = 4",
            basis: Basis::Reduced,
        },
        Source {
            covers: "L_vap",
            citation: "28.462016 kJ mol⁻¹, h″ − h′ of CoolProp's triple-point states of Gao et al.'s \
                       equation",
            basis: Basis::Raw { licence: "MIT" },
        },
    ],
};

/// Hydrogen sulphide: Lemmon and Span 2006, through CoolProp.
pub(in crate::substance) const HYDROGEN_SULPHIDE: PhaseDiagram = PhaseDiagram {
    triple_temperature: Kelvin::new(187.7),
    triple_pressure: Pascals::new(23_258.855_84),
    critical_temperature: Kelvin::new(373.1),
    critical_pressure: Pascals::new(9.0e6),
    vapour_pressure: WagnerEquation {
        reducing_temperature: Kelvin::new(373.1),
        reducing_pressure: Pascals::new(9.0e6),
        terms: &[
            (-6.761_826_097_619_247, 1.003),
            (-0.005_988_209_650_772_923, 1.146),
            (-6.882_637_639_835_571, 1.668),
            (-2.825_455_087_691_035_6, 4.886),
            (7.704_835_506_786_447, 1.558),
            (1.741_789_150_934_569_8, 9.098),
        ],
    },
    liquid_density: DensityEquation {
        form: DensityForm::Linear,
        reducing_temperature: Kelvin::new(373.1),
        reducing_density: KilogramsPerCubicMetre::new(10_190.0 * 0.034_080_88),
        terms: &[
            (51.601_033_693_983_716, 0.572),
            (-49.935_957_197_942_71, 0.586),
            (6.987_088_261_412_919, 2.075),
            (-24.784_697_586_038_508, 3.069),
            (84.077_926_767_685_13, 4.223),
            (-68.551_338_802_008_39, 4.619),
        ],
    },
    sublimation: &[
        SublimationPiece {
            phase: "I",
            lower: Kelvin::new(173.7),
            equation: SublimationEquation::InversePowers {
                unit: BAR,
                coefficients: &[8.933, -7.260e2, -3.504e5, 2.724e7, -8.582e8],
            },
        },
        SublimationPiece {
            phase: "I to III",
            lower: Kelvin::new(60.0),
            equation: SublimationEquation::InversePowers {
                unit: BAR,
                coefficients: &[12.98, -2.707e3],
            },
        },
    ],
    anchors: &saturation::H2S_ANCHORS,
    melting: None,
    solids: &[],
    triple_vaporisation_enthalpy_j_per_mol: 19_596.339,
    triple_fusion_enthalpy_j_per_mol: None,
    sources: &[
        Source {
            covers: "T_t, p_t, T_c, p_c",
            citation: "187.7 K and 373.1 K, 9.0 MPa, Lemmon and Span 2006 (not reached) as \
                       CoolProp reads them, and p_t, 23,258.856 Pa, CoolProp's triple-point state \
                       of their equation",
            basis: Basis::Raw { licence: "MIT" },
        },
        Source {
            covers: "p_liquid, ρ_l",
            citation: "CoolProp's ancillaries, Bell's fits to Lemmon and Span's equation of \
                       state, within 0.014% and 0.13% of it; the reducing density 10,190 mol m⁻³ \
                       at M = 34.08088 g mol⁻¹",
            basis: Basis::Raw { licence: "MIT" },
        },
        Source {
            covers: "p_solid",
            citation: "Fray and Schmitt's two H2S polynomials, their ranges not \
                       read: the five-term one near the triple point, whose enthalpy climbs \
                       unphysically below about 100 K, down to 173.7 K, where the two come \
                       closest (within 0.09%, their slopes equal), and the two-term one below it, \
                       a constant 22.5 kJ mol⁻¹, from 60 K",
            basis: Basis::Fact,
        },
        COOLPROP,
        FRAY_SCHMITT,
        ANCHORED,
        Source {
            covers: "L_vap",
            citation: "19.596339 kJ mol⁻¹, h″ − h′ of CoolProp's triple-point states of Lemmon and \
                       Span's equation",
            basis: Basis::Raw { licence: "MIT" },
        },
    ],
};

/// Ethane: Bücker and Wagner 2006, through CoolProp.
pub(in crate::substance) const ETHANE: PhaseDiagram = PhaseDiagram {
    triple_temperature: Kelvin::new(90.368),
    triple_pressure: Pascals::new(1.142_107_639),
    critical_temperature: Kelvin::new(305.322),
    critical_pressure: Pascals::new(4.8722e6),
    vapour_pressure: WagnerEquation {
        reducing_temperature: Kelvin::new(305.322),
        reducing_pressure: Pascals::new(4.8722e6),
        terms: &[
            (9.344_483_558_228_339, 0.871),
            (-14.812_636_376_983_514, 0.902),
            (-0.687_307_721_008_648_3, 1.663),
            (0.794_240_057_232_257_3, 2.045),
            (-2.447_274_904_672_986, 3.946),
            (0.040_475_914_784_306_41, 14.704),
        ],
    },
    liquid_density: DensityEquation {
        form: DensityForm::Linear,
        reducing_temperature: Kelvin::new(305.322),
        reducing_density: KilogramsPerCubicMetre::new(6_856.886_685 * 0.030_069_04),
        terms: &[
            (0.202_952_671_644_754_3, 0.211),
            (2.027_464_725_614_132, 0.408),
            (0.437_897_643_241_199_1, 1.789),
            (-13.592_018_000_960_584, 10.003),
            (57.550_314_354_740_73, 12.669),
            (-88.689_046_121_380_73, 16.309),
        ],
    },
    sublimation: &[SublimationPiece {
        phase: "solid",
        lower: Kelvin::new(20.0),
        equation: SublimationEquation::InversePowers {
            unit: BAR,
            coefficients: &[15.11, -2.207e3, -2.411e4, 7.744e5, -1.161e7, 6.763e7],
        },
    }],
    anchors: &saturation::C2H6_ANCHORS,
    melting: Some(MeltingCurve::Rising(&[MeltingPiece {
        start: Kelvin::new(90.368),
        end: None,
        equation: MeltingEquation::Polynomial {
            temperature: Kelvin::new(90.368),
            pressure: Pascals::new(1.14),
            terms: &[(223_626_315.0, 1.0), (105_262_374.0, 2.55)],
        },
    }])),
    solids: &[],
    triple_vaporisation_enthalpy_j_per_mol: 17_884.191,
    triple_fusion_enthalpy_j_per_mol: None,
    sources: &[
        Source {
            covers: "T_t, p_t, T_c, p_c",
            citation: "90.368 K and 305.322 K, 4.8722 MPa, Bücker and Wagner 2006 (JPCRD 35, 205, \
                       not reached) as CoolProp reads them, and p_t, 1.1421076 Pa, CoolProp's \
                       triple-point state of their equation",
            basis: Basis::Raw { licence: "MIT" },
        },
        Source {
            covers: "p_liquid, ρ_l",
            citation: "CoolProp's ancillaries, Bell's fits to Bücker and Wagner's equation of \
                       state, within 0.024% and 0.26% of it; the reducing density 6,856.886685 mol \
                       m⁻³ at M = 30.06904 g mol⁻¹",
            basis: Basis::Raw { licence: "MIT" },
        },
        Source {
            covers: "p_solid",
            citation: "Fray and Schmitt's C2H6 polynomial, from 20 K, one equation for the \
                       whole solid",
            basis: Basis::Fact,
        },
        COOLPROP,
        FRAY_SCHMITT,
        ANCHORED,
        Source {
            covers: "p_melt",
            citation: "Bücker and Wagner 2006's melting equation as CoolProp transcribes it \
                       (p₀ = 1.14 Pa), stated to 190 K; extrapolated beyond",
            basis: Basis::Raw { licence: "MIT" },
        },
        Source {
            covers: "L_vap",
            citation: "17.884191 kJ mol⁻¹, h″ − h′ of CoolProp's triple-point states of Bücker and \
                       Wagner's equation",
            basis: Basis::Raw { licence: "MIT" },
        },
    ],
};

// Each diagram holds one anchor per sublimation piece: a regenerated table that disagreed with its
// rows would otherwise be cut short by the zip that pairs them.
const _: () = assert!(WATER.sublimation.len() == WATER.anchors.len());
const _: () = assert!(METHANE.sublimation.len() == METHANE.anchors.len());
const _: () = assert!(AMMONIA.sublimation.len() == AMMONIA.anchors.len());
const _: () = assert!(NITROGEN.sublimation.len() == NITROGEN.anchors.len());
const _: () = assert!(OXYGEN.sublimation.len() == OXYGEN.anchors.len());
const _: () = assert!(CARBON_DIOXIDE.sublimation.len() == CARBON_DIOXIDE.anchors.len());
const _: () = assert!(ARGON.sublimation.len() == ARGON.anchors.len());
const _: () = assert!(KRYPTON.sublimation.len() == KRYPTON.anchors.len());
const _: () = assert!(XENON.sublimation.len() == XENON.anchors.len());
const _: () = assert!(NITROUS_OXIDE.sublimation.len() == NITROUS_OXIDE.anchors.len());
const _: () = assert!(CARBON_MONOXIDE.sublimation.len() == CARBON_MONOXIDE.anchors.len());
const _: () = assert!(SULPHUR_DIOXIDE.sublimation.len() == SULPHUR_DIOXIDE.anchors.len());
const _: () = assert!(HYDROGEN_SULPHIDE.sublimation.len() == HYDROGEN_SULPHIDE.anchors.len());
const _: () = assert!(ETHANE.sublimation.len() == ETHANE.anchors.len());
