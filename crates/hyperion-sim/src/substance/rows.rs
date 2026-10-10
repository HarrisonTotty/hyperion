//! The registry's rows, in their append-only order, each value with its sources.
//!
//! A row is appended, never inserted, renumbered, renamed or removed (decision-composition §1.1);
//! a changed value is a change of generated output and bumps the generator version. The registry's
//! golden pins each row's identity as a prefix that only grows, and its fixture mirrors it for the
//! client.
//!
//! Rows 0–8 are P14.T13's nine gases in the order of
//! [`Gas::ALL`](crate::planetary::derive::atmosphere::Gas::ALL) (P14.T49.a). Their molar masses
//! are P14.T13's, moved here so that `Gas` reads them; their phase data, for the five that
//! condense, are P14.T13.c's Clausius–Clapeyron constants moved verbatim, with each critical
//! pressure and triple-point liquid density added, and methane's for P14.T24.f. Each reference
//! equation of state is cited as the primary; NIST's Chemistry Web Book (SRD 69) is only the
//! check of the values asserted from it (decision-composition §5). Where P14.T13.c adopted an
//! effective enthalpy rather than a source's value, its source says so ([`Basis::Adopted`]) and
//! names the reference value beside it, for P14.T49.b's fitted curves.

use super::mix::{GasColumns, HeatCapacityClass};
use super::phase::PhaseColumns;
use super::{Basis, ClientOptics, Element, Source, Substance, SubstanceKey, SubstanceKind};
use crate::units::{Kelvin, KilogramsPerCubicMetre, Pascals};

/// The rows, in registry order.
pub(super) static ROWS: [Substance; 9] = [
    HYDROGEN,
    HELIUM,
    WATER,
    METHANE,
    AMMONIA,
    NITROGEN,
    OXYGEN,
    CARBON_DIOXIDE,
    ARGON,
];

/// The molar masses' source: P14.T13's `Gas::molar_mass_g_per_mol`, the sums of IUPAC's abridged
/// standard atomic weights.
const ATOMIC_WEIGHTS: Source = Source {
    covers: "stoichiometry, molar mass",
    citation: "the formula's IUPAC 2021 abridged standard atomic weights, summed (Prohaska et al. \
               2022, Pure Appl. Chem. 94, 573: H 1.0080, He 4.0026, C 12.011, N 14.007, O 15.999, \
               Ar 39.95), He to 4.003; P14.T13's Gas::molar_mass_g_per_mol",
    basis: Basis::Fact,
};

/// The identity's sources, for every row 0–8.
const IDENTITY: &[Source] = &[ATOMIC_WEIGHTS];

/// An atom's `c_p` ÷ R.
const ATOM_HEAT_CAPACITY: GasColumns = GasColumns {
    heat_capacity: HeatCapacityClass::Atom,
    sources: &[Source {
        covers: "c_p ÷ R",
        citation: "1 + N ÷ 2 with N = 3 for an atom, equipartition with fixed degrees of freedom \
                   in the form of Robinson and Catling 2012, ApJ 757, 104, eq. 9; exact for a \
                   monatomic ideal gas",
        basis: Basis::Fact,
    }],
};

/// A linear molecule's `c_p` ÷ R.
const LINEAR_HEAT_CAPACITY: GasColumns = GasColumns {
    heat_capacity: HeatCapacityClass::Linear,
    sources: &[Source {
        covers: "c_p ÷ R",
        citation: "1 + N ÷ 2 with N = 5 for a linear molecule (Robinson and Catling 2012, ApJ \
                   757, 104, §2.3 and eq. 9)",
        basis: Basis::Fact,
    }],
};

/// A non-linear molecule's `c_p` ÷ R.
const NON_LINEAR_HEAT_CAPACITY: GasColumns = GasColumns {
    heat_capacity: HeatCapacityClass::NonLinear,
    sources: &[Source {
        covers: "c_p ÷ R",
        citation: "1 + N ÷ 2 with N = 6 for a non-linear molecule, equipartition in the form of \
                   Robinson and Catling 2012, ApJ 757, 104, eq. 9; NIST-JANAF's ideal-gas c_p at \
                   298.15 K exceeds it by 1.0% for H₂O and 7.2% for CH₄ and NH₃ (Chase 1998)",
        basis: Basis::Fact,
    }],
};

/// Row 0, molecular hydrogen: a well-mixed gas, which condenses only near 20 K.
const HYDROGEN: Substance = Substance {
    key: SubstanceKey::new_const("H2"),
    kind: SubstanceKind::Molecule,
    elements: &[(Element::H, 2)],
    charge: 0,
    molar_mass_g_per_mol: Some(2.016),
    gas: Some(LINEAR_HEAT_CAPACITY),
    phase: None,
    optics: ClientOptics {
        rayleigh: true,
        absorber: false,
        aerosol_material: false,
    },
    sources: IDENTITY,
};

/// Row 1, helium, which never condenses at the surface of a body.
const HELIUM: Substance = Substance {
    key: SubstanceKey::new_const("He"),
    kind: SubstanceKind::Atom,
    elements: &[(Element::He, 1)],
    charge: 0,
    molar_mass_g_per_mol: Some(4.003),
    gas: Some(ATOM_HEAT_CAPACITY),
    phase: None,
    optics: ClientOptics {
        rayleigh: true,
        absorber: false,
        aerosol_material: false,
    },
    sources: IDENTITY,
};

/// Row 2, water: vapour, the liquid and ice. Its visible bands are an absorber the client draws
/// (rendering plan R08's R08.T4.c), and its droplets and crystals a cloud material (R08.T5.b).
const WATER: Substance = Substance {
    key: SubstanceKey::new_const("H2O"),
    kind: SubstanceKind::Molecule,
    elements: &[(Element::H, 2), (Element::O, 1)],
    charge: 0,
    molar_mass_g_per_mol: Some(18.015),
    gas: Some(NON_LINEAR_HEAT_CAPACITY),
    phase: Some(PhaseColumns {
        triple_temperature: Kelvin::new(273.16),
        triple_pressure: Pascals::new(611.657),
        critical_temperature: Kelvin::new(647.1),
        critical_pressure: Pascals::new(22.064e6),
        sublimation_enthalpy_j_per_mol: 51_059.0,
        vaporisation_enthalpy_j_per_mol: 43_500.0,
        triple_liquid_density: KilogramsPerCubicMetre::new(999.793),
        sources: &[
            Source {
                covers: "T_t, p_t",
                citation: "273.16 K and 611.657 Pa, the triple point: Guildner, Johnson and Jones \
                           1976's measured pressure (J. Res. Natl. Bur. Stand. 80A, 505), as \
                           Wagner and Pruss 2002 (JPCRD 31, 387) and IAPWS R10-06 quote it; \
                           IAPWS-95's own value is 611.655 Pa",
                basis: Basis::Fact,
            },
            Source {
                covers: "T_c, p_c",
                citation: "647.096 K, held at P14.T13.c's 647.1 K, and 22.064 MPa (Wagner and \
                           Pruss 2002; NIST's WebBook, SRD 69, as the check: 647.096 K, 22,064.0 \
                           kPa)",
                basis: Basis::Fact,
            },
            Source {
                covers: "L_sub",
                citation: "51.059 kJ mol⁻¹, P14.T13.c's, ice Ih's enthalpy of sublimation at the \
                           triple point: Murphy and Koop 2005 (Q. J. R. Meteorol. Soc. 131, 1539), \
                           eq. 5, gives 51.059, and IAPWS-95 with Feistel and Wagner 2006's ice \
                           Ih (IAPWS-06) 51.062",
                basis: Basis::Fact,
            },
            Source {
                covers: "L_vap",
                citation: "43.5 kJ mol⁻¹, P14.T13.c's effective value, between the liquid's \
                           45.055 at 273.16 K and 40.651 at 373.124 K (Wagner and Pruss 2002; \
                           NIST's WebBook as the check); it puts the saturation pressure at \
                           373.124 K at 103.5 kPa, 2.2% above one atmosphere, where 43.32 would \
                           meet it exactly",
                basis: Basis::Adopted,
            },
            Source {
                covers: "ρ_l(T_t)",
                citation: "999.793 kg m⁻³, the saturated liquid at the triple point (Wagner and \
                           Pruss 2002; NIST's WebBook as the check: 999.7925)",
                basis: Basis::Fact,
            },
        ],
    }),
    optics: ClientOptics {
        rayleigh: true,
        absorber: true,
        aerosol_material: true,
    },
    sources: IDENTITY,
};

/// Row 3, methane: Titan's second gas, its lakes and its clouds, and an absorber in every giant
/// (R08.T4.b) and a cloud material (R08.T5.b).
const METHANE: Substance = Substance {
    key: SubstanceKey::new_const("CH4"),
    kind: SubstanceKind::Molecule,
    elements: &[(Element::C, 1), (Element::H, 4)],
    charge: 0,
    molar_mass_g_per_mol: Some(16.043),
    gas: Some(NON_LINEAR_HEAT_CAPACITY),
    phase: Some(PhaseColumns {
        triple_temperature: Kelvin::new(90.6941),
        triple_pressure: Pascals::new(11_696.0),
        critical_temperature: Kelvin::new(190.564),
        critical_pressure: Pascals::new(4.5992e6),
        sublimation_enthalpy_j_per_mol: 9_670.7,
        vaporisation_enthalpy_j_per_mol: 8_731.5,
        triple_liquid_density: KilogramsPerCubicMetre::new(451.475),
        sources: &[
            Source {
                covers: "T_t, p_t, T_c, p_c",
                citation: "90.6941 K and 11.696 kPa; 190.564 K and 4.5992 MPa (Setzmann and Wagner \
                           1991, JPCRD 20, 1061; NIST's WebBook, SRD 69, as the check: 90.6941 K, \
                           11.6961 kPa, 190.564 K, 4,599.20 kPa)",
                basis: Basis::Fact,
            },
            Source {
                covers: "L_vap",
                citation: "8.7315 kJ mol⁻¹, the saturated vapour's enthalpy less the liquid's at \
                           the triple point (Setzmann and Wagner 1991; NIST's WebBook as the \
                           check: 7.57930 + 1.15220 kJ mol⁻¹)",
                basis: Basis::Fact,
            },
            Source {
                covers: "L_sub",
                citation: "9.6707 kJ mol⁻¹, L_vap plus the enthalpy of fusion at the triple point, \
                           0.9392 kJ mol⁻¹ (Vogt and Pitzer 1976, J. Chem. Thermodyn. 8, 1011, as \
                           NIST's WebBook lists it); Stull 1947's 9.62 at 77 K and Stephenson and \
                           Malanowski 1987's 9.7 at 72 K agree",
                basis: Basis::Reduced,
            },
            Source {
                covers: "ρ_l(T_t)",
                citation: "451.475 kg m⁻³, the saturated liquid at the triple point (Setzmann and \
                           Wagner 1991; NIST's WebBook as the check: 451.475)",
                basis: Basis::Fact,
            },
        ],
    }),
    optics: ClientOptics {
        rayleigh: true,
        absorber: true,
        aerosol_material: true,
    },
    sources: IDENTITY,
};

/// Row 4, ammonia: the giants' upper cloud deck (R08.T5.b) and visible bands (R08.T4.c). Its
/// phase data are P14.T49.b's.
const AMMONIA: Substance = Substance {
    key: SubstanceKey::new_const("NH3"),
    kind: SubstanceKind::Molecule,
    elements: &[(Element::N, 1), (Element::H, 3)],
    charge: 0,
    molar_mass_g_per_mol: Some(17.031),
    gas: Some(NON_LINEAR_HEAT_CAPACITY),
    phase: None,
    optics: ClientOptics {
        rayleigh: true,
        absorber: true,
        aerosol_material: true,
    },
    sources: IDENTITY,
};

/// Row 5, molecular nitrogen: Earth's, Titan's and Triton's air, and Pluto's and Triton's ice
/// (a cloud material from R08.T5.d).
const NITROGEN: Substance = Substance {
    key: SubstanceKey::new_const("N2"),
    kind: SubstanceKind::Molecule,
    elements: &[(Element::N, 2)],
    charge: 0,
    molar_mass_g_per_mol: Some(28.014),
    gas: Some(LINEAR_HEAT_CAPACITY),
    phase: Some(PhaseColumns {
        triple_temperature: Kelvin::new(63.15),
        triple_pressure: Pascals::new(12_520.0),
        critical_temperature: Kelvin::new(126.19),
        critical_pressure: Pascals::new(3.3958e6),
        sublimation_enthalpy_j_per_mol: 6_900.0,
        vaporisation_enthalpy_j_per_mol: 5_570.0,
        triple_liquid_density: KilogramsPerCubicMetre::new(867.222),
        sources: &[
            Source {
                covers: "T_t, p_t, T_c, p_c",
                citation: "63.151 K and 12.5198 kPa, held at P14.T13.c's 63.15 K and 12.52 kPa; \
                           126.192 K, held at 126.19 K, and 3.3958 MPa (Span, Lemmon, Jacobsen, \
                           Wagner and Yokozeki 2000, JPCRD 29, 1361; NIST's WebBook, SRD 69, as \
                           the check)",
                basis: Basis::Fact,
            },
            Source {
                covers: "L_sub",
                citation: "6.9 kJ mol⁻¹, P14.T13.c's effective value; at the triple point L_vap, \
                           6.037 kJ mol⁻¹ (Span et al. 2000), plus the enthalpy of fusion, about \
                           0.72 (Giauque and Clayton 1933, JACS 55, 4875; not read here), gives \
                           6.76",
                basis: Basis::Adopted,
            },
            Source {
                covers: "L_vap",
                citation: "5.57 kJ mol⁻¹, P14.T13.c's, near the normal boiling point's: Span et \
                           al. 2000 give 5.580 at 77.355 K, and Giauque and Clayton 1933 5.6 at 77 \
                           K (as NIST's WebBook lists it); 6.037 at the triple point",
                basis: Basis::Adopted,
            },
            Source {
                covers: "ρ_l(T_t)",
                citation: "867.222 kg m⁻³, the saturated liquid at the triple point (Span et al. \
                           2000; NIST's WebBook as the check: 867.222)",
                basis: Basis::Fact,
            },
        ],
    }),
    optics: ClientOptics {
        rayleigh: true,
        absorber: false,
        aerosol_material: true,
    },
    sources: IDENTITY,
};

/// Row 6, molecular oxygen: abiotic or biotic. Its A and B bands at 762 and 688 nm lie in the
/// client's visible channels, and it condenses near 90 K under a bar; its phase data are
/// P14.T49.b's.
const OXYGEN: Substance = Substance {
    key: SubstanceKey::new_const("O2"),
    kind: SubstanceKind::Molecule,
    elements: &[(Element::O, 2)],
    charge: 0,
    molar_mass_g_per_mol: Some(31.998),
    gas: Some(LINEAR_HEAT_CAPACITY),
    phase: None,
    optics: ClientOptics {
        rayleigh: true,
        absorber: true,
        aerosol_material: true,
    },
    sources: IDENTITY,
};

/// Row 7, carbon dioxide: Venus's and Mars's air, Mars's caps and clouds (R08.T5.b).
const CARBON_DIOXIDE: Substance = Substance {
    key: SubstanceKey::new_const("CO2"),
    kind: SubstanceKind::Molecule,
    elements: &[(Element::C, 1), (Element::O, 2)],
    charge: 0,
    molar_mass_g_per_mol: Some(44.009),
    gas: Some(GasColumns {
        heat_capacity: HeatCapacityClass::LinearTriatomic,
        sources: &[Source {
            covers: "c_p ÷ R",
            citation: "γ = 1.3, so 13 ÷ 3 (Robinson and Catling 2012, ApJ 757, 104, §2.3: N ≈ 7 \
                       with about two vibrational degrees, after Bent 1965); NIST-JANAF's 37.129 \
                       J mol⁻¹ K⁻¹ at 298.15 K gives 4.47 (Chase 1998, table C-095)",
            basis: Basis::Fact,
        }],
    }),
    phase: Some(PhaseColumns {
        triple_temperature: Kelvin::new(216.58),
        triple_pressure: Pascals::new(518_500.0),
        critical_temperature: Kelvin::new(304.13),
        critical_pressure: Pascals::new(7.3773e6),
        sublimation_enthalpy_j_per_mol: 26_100.0,
        vaporisation_enthalpy_j_per_mol: 15_300.0,
        triple_liquid_density: KilogramsPerCubicMetre::new(1_178.46),
        sources: &[
            Source {
                covers: "T_t, p_t",
                citation: "216.58 K and 518.5 kPa, P14.T13.c's (Angus, Armstrong and de Reuck \
                           1976, IUPAC's tables, as NIST's WebBook, SRD 69, lists them); Span and \
                           Wagner 1996, JPCRD 25, 1509, give 216.592 K and 517.95 kPa",
                basis: Basis::Fact,
            },
            Source {
                covers: "T_c, p_c",
                citation: "304.1282 K, held at P14.T13.c's 304.13 K, and 7.3773 MPa (Span and \
                           Wagner 1996; NIST's WebBook as the check)",
                basis: Basis::Fact,
            },
            Source {
                covers: "L_sub",
                citation: "26.1 kJ mol⁻¹ at 207 K, not at the triple point (Stephenson and \
                           Malanowski 1987, from data over 198–216 K, as NIST's WebBook lists \
                           it), P14.T13.c's",
                basis: Basis::Fact,
            },
            Source {
                covers: "L_vap",
                citation: "15.3 kJ mol⁻¹, P14.T13.c's effective value, near Span and Wagner 1996's \
                           15.420 at the triple point (NIST's WebBook as the check: 18.9425 − \
                           3.52235 kJ mol⁻¹)",
                basis: Basis::Adopted,
            },
            Source {
                covers: "ρ_l(T_t)",
                citation: "1,178.46 kg m⁻³, the saturated liquid at the triple point (Span and \
                           Wagner 1996; NIST's WebBook as the check: 1,178.46)",
                basis: Basis::Fact,
            },
        ],
    }),
    optics: ClientOptics {
        rayleigh: true,
        absorber: false,
        aerosol_material: true,
    },
    sources: IDENTITY,
};

/// Row 8, argon: radiogenic ⁴⁰Ar, which condenses on the coldest worlds.
const ARGON: Substance = Substance {
    key: SubstanceKey::new_const("Ar"),
    kind: SubstanceKind::Atom,
    elements: &[(Element::Ar, 1)],
    charge: 0,
    molar_mass_g_per_mol: Some(39.95),
    gas: Some(ATOM_HEAT_CAPACITY),
    phase: Some(PhaseColumns {
        triple_temperature: Kelvin::new(83.81),
        triple_pressure: Pascals::new(68_890.0),
        critical_temperature: Kelvin::new(150.69),
        critical_pressure: Pascals::new(4.863e6),
        sublimation_enthalpy_j_per_mol: 7_800.0,
        vaporisation_enthalpy_j_per_mol: 6_430.0,
        triple_liquid_density: KilogramsPerCubicMetre::new(1_416.77),
        sources: &[
            Source {
                covers: "T_t, p_t, T_c, p_c",
                citation: "83.8058 K and 68.891 kPa, held at P14.T13.c's 83.81 K and 68.89 kPa; \
                           150.687 K, held at 150.69 K, and 4.863 MPa (Tegeler, Span and Wagner \
                           1999, JPCRD 28, 779; NIST's WebBook, SRD 69, as the check)",
                basis: Basis::Fact,
            },
            Source {
                covers: "L_sub",
                citation: "7.8 kJ mol⁻¹, P14.T13.c's effective value; at the triple point L_vap, \
                           6.540 kJ mol⁻¹ (Tegeler et al. 1999), plus a handbook enthalpy of \
                           fusion of about 1.18 (its primary not reached), gives about 7.72",
                basis: Basis::Adopted,
            },
            Source {
                covers: "L_vap",
                citation: "6.43 kJ mol⁻¹, P14.T13.c's, near Tegeler et al. 1999's 6.437 at the \
                           normal boiling point, 87.302 K; 6.540 at the triple point",
                basis: Basis::Adopted,
            },
            Source {
                covers: "ρ_l(T_t)",
                citation: "1,416.77 kg m⁻³, the saturated liquid at the triple point (Tegeler et \
                           al. 1999; NIST's WebBook as the check: 1,416.77)",
                basis: Basis::Fact,
            },
        ],
    }),
    optics: ClientOptics {
        rayleigh: true,
        absorber: false,
        aerosol_material: true,
    },
    sources: IDENTITY,
};
