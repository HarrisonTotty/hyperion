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
//!
//! P14.T49.b appends rows 9–111 in its plan's order and gives every gas row its molecular
//! constants ([`MolecularConstants`]), over which [`thermo`](super::thermo) sums its heat
//! capacity. Rows 0–8 keep every value generator version 22 publishes until the 22 → 23 bump
//! (P14.T54.a; decision-bump-plan-after-22 §1.2): their held `c_p` ÷ R class, their phase
//! columns and their client flags (P14.T49.b's flags for `H2`, `He`, `N2` and `CO2`, `absorber`
//! true for their collision-induced and band absorption, wait for the bump).
//!
//! Each gas's harmonic fundamentals are Shimanouchi's selected gas-phase values (NSRDS-NBS 39,
//! 1972, a US-government work, and its 1977 Volume II for ozone), a diatomic's are Huber and
//! Herzberg 1979's `ω_e − 2 ω_e x_e`, and an atom's or ion's levels are the critical compilations
//! NIST's Atomic Spectra Database holds, each row naming its own (science-r08-gases-2.md §1.2–§1.3).
//! The appended rows' molar masses are the sums of IUPAC 2021's abridged standard atomic weights
//! over their formulae, an ion's less or more the electron's.
//!
//! P14.T49.b's second step gives the fourteen rows whose reference equations of state were read
//! their phase diagrams ([`diagram`](super::diagram), the data in `rows/diagrams.rs`): rows 0–8's
//! five held condensables beside their held phase columns, and ammonia, oxygen, krypton, xenon,
//! nitrous oxide, carbon monoxide, sulphur dioxide, hydrogen sulphide and ethane.

mod diagrams;

use super::mix::{GasColumns, HeatCapacityClass};
use super::phase::PhaseColumns;
use super::thermo::{
    Level, LevelSet, MolecularConstants, NuclearSpin, Rotation, RovibrationalTerms, Vibration,
};
use super::{Basis, ClientOptics, Element, Source, Substance, SubstanceKey, SubstanceKind};
use crate::units::{Kelvin, KilogramsPerCubicMetre, Pascals};

/// The rows, in registry order.
pub(super) static ROWS: [Substance; 112] = [
    HYDROGEN,
    HELIUM,
    WATER,
    METHANE,
    AMMONIA,
    NITROGEN,
    OXYGEN,
    CARBON_DIOXIDE,
    ARGON,
    NEON,
    KRYPTON,
    XENON,
    NITROUS_OXIDE,
    CARBON_MONOXIDE,
    SULPHUR_DIOXIDE,
    HYDROGEN_SULPHIDE,
    OZONE,
    HYDROGEN_CYANIDE,
    ACETYLENE,
    ETHYLENE,
    ETHANE,
    PHOSPHINE,
    METHANOL,
    ATOMIC_HYDROGEN,
    ATOMIC_OXYGEN,
    ELECTRON,
    HYDRIDE,
    SODIUM,
    POTASSIUM,
    IRON,
    MAGNESIUM,
    SILICON,
    CALCIUM,
    TITANIUM,
    SILICON_MONOXIDE,
    TITANIUM_MONOXIDE,
    VANADIUM_MONOXIDE,
    IRON_HYDRIDE,
    ALUMINIUM,
    VANADIUM,
    MANGANESE,
    NICKEL,
    ZINC,
    SODIUM_ION,
    MAGNESIUM_ION,
    ALUMINIUM_ION,
    SILICON_ION,
    POTASSIUM_ION,
    CALCIUM_ION,
    TITANIUM_ION,
    VANADIUM_ION,
    CHROMIUM_ION,
    MANGANESE_ION,
    IRON_ION,
    NICKEL_ION,
    ZINC_ION,
    HYDROGEN_CHLORIDE,
    AMMONIUM_HYDROSULPHIDE,
    SULPHURIC_ACID,
    SULPHUR,
    POTASSIUM_CHLORIDE,
    SODIUM_CHLORIDE,
    ZINC_SULPHIDE,
    SODIUM_SULPHIDE,
    MANGANESE_SULPHIDE,
    CHROMIUM,
    IRON_SULPHIDE,
    ENSTATITE,
    FORSTERITE,
    SILICA,
    CORUNDUM,
    PEROVSKITE,
    RUTILE,
    GRAPHITE,
    THOLIN,
    SOOT,
    CHROMOPHORE,
    ICE_GIANT_HAZE,
    MARS_DUST,
    ACID_UV_ABSORBER,
    BRINE_NACL,
    BRINE_MGSO4,
    BRINE_NACLO4,
    BRINE_MGCLO4,
    AMMONIA_WATER,
    TITAN_LIQUID,
    ANORTHOSITE,
    FELDSPATHIC,
    BASALT,
    BASALT_FE,
    KOMATIITE,
    BASALT_REDUCED,
    GRANITE,
    ULTRAMAFIC,
    FERRIC,
    PHYLLOSILICATE,
    LAVA_GLASS,
    CA_AL_RESIDUE,
    IRON_NICKEL,
    SILICON_CARBIDE,
    MAGNESIUM_SULPHATE,
    SODIUM_CARBONATE,
    CALCITE,
    CARBONACEOUS,
    CHONDRITE_CI,
    CHONDRITE_CM,
    CHONDRITE_CV,
    CHONDRITE_H,
    CHONDRITE_L,
    CHONDRITE_LL,
    CHONDRITE_E,
    COMETARY,
];

/// The molar masses' source: P14.T13's `Gas::molar_mass_g_per_mol` for rows 0–8, and for every
/// row the sum of its formula's IUPAC 2021 abridged standard atomic weights, an ion's less (a
/// cation) or more (an anion) the electron's molar mass.
const ATOMIC_WEIGHTS: Source = Source {
    covers: "stoichiometry, molar mass",
    citation: "the formula's IUPAC 2021 abridged standard atomic weights, summed (Prohaska et al. \
               2022, Pure Appl. Chem. 94, 573, as CIAAW's table prints them: H 1.0080, He \
               4.0026, C 12.011, N 14.007, O 15.999, Ne 20.180, Na 22.990, Mg 24.305, Al \
               26.982, Si 28.085, P 30.974, S 32.06, Cl 35.45, Ar 39.95, K 39.098, Ca 40.078, Ti \
               47.867, V 50.942, Cr 51.996, Mn 54.938, Fe 55.845, Ni 58.693, Zn 65.38, Kr 83.798, \
               Xe 131.29), He to 4.003 in P14.T13's Gas::molar_mass_g_per_mol; an ion's with the \
               electron's 5.485 799 088 8 × 10⁻⁴ g mol⁻¹ (CODATA 2018, Tiesinga et al. 2021, Rev. \
               Mod. Phys. 93, 025010)",
    basis: Basis::Fact,
};

/// The identity's sources, for every row of one formula.
const IDENTITY: &[Source] = &[ATOMIC_WEIGHTS];

/// An atom's held `c_p` ÷ R class.
const ATOM_CLASS: Source = Source {
    covers: "c_p ÷ R class",
    citation: "1 + N ÷ 2 with N = 3 for an atom, equipartition with fixed degrees of freedom in the \
               form of Robinson and Catling 2012, ApJ 757, 104, eq. 9; exact for a monatomic ideal \
               gas; generator version 22's published value, held until the 22 → 23 bump",
    basis: Basis::Fact,
};

/// A linear molecule's held `c_p` ÷ R class.
const LINEAR_CLASS: Source = Source {
    covers: "c_p ÷ R class",
    citation: "1 + N ÷ 2 with N = 5 for a linear molecule (Robinson and Catling 2012, ApJ 757, 104, \
               §2.3 and eq. 9); generator version 22's published value, held until the 22 → 23 \
               bump",
    basis: Basis::Fact,
};

/// A non-linear molecule's held `c_p` ÷ R class.
const NON_LINEAR_CLASS: Source = Source {
    covers: "c_p ÷ R class",
    citation: "1 + N ÷ 2 with N = 6 for a non-linear molecule, equipartition in the form of Robinson \
               and Catling 2012, ApJ 757, 104, eq. 9; NIST-JANAF's ideal-gas c_p at 298.15 K exceeds \
               it by 1.0% for H₂O and 7.2% for CH₄ and NH₃ (Chase 1998); generator version 22's \
               published value, held until the 22 → 23 bump",
    basis: Basis::Fact,
};

/// Carbon dioxide's held `c_p` ÷ R class.
const CARBON_DIOXIDE_CLASS: Source = Source {
    covers: "c_p ÷ R class",
    citation: "γ = 1.3, so 13 ÷ 3 (Robinson and Catling 2012, ApJ 757, 104, §2.3: N ≈ 7 with about \
               two vibrational degrees, after Bent 1965); NIST-JANAF's 37.129 J mol⁻¹ K⁻¹ at 298.15 \
               K gives 4.47 (Chase 1998, table C-095); generator version 22's published value, held \
               until the 22 → 23 bump",
    basis: Basis::Fact,
};

/// A listed level `energy_cm` cm⁻¹ above its set's lowest, of `degeneracy`.
#[must_use]
const fn level(energy_cm: f64, degeneracy: f64) -> Level {
    Level {
        energy_cm,
        degeneracy,
    }
}

/// A harmonic fundamental of `wavenumber_cm` cm⁻¹ and `degeneracy`.
#[must_use]
const fn vibration(wavenumber_cm: f64, degeneracy: u8) -> Vibration {
    Vibration {
        wavenumber_cm,
        degeneracy,
    }
}

/// H₂'s `X¹Σg⁺` term values (Huber and Herzberg 1979, Constants of Diatomic Molecules, the NIST
/// WebBook's transcription and its notes 118–121 as the finding aid), cm⁻¹.
const HYDROGEN_TERMS: RovibrationalTerms = RovibrationalTerms {
    we_cm: 4_401.213,
    wexe_cm: 121.336,
    weye_cm: 0.812_9,
    be_cm: 60.853,
    ae_cm: 3.062_2,
    ge_cm: 0.057_7,
    de_cm: 0.047_1,
    beta_e_cm: -0.002_74,
};

/// The source of H₂'s molecular constants.
const HYDROGEN_TERMS_SOURCE: Source = Source {
    covers: "rotation, vibrations",
    citation: "H₂'s X¹Σg⁺ rovibrational ladder, v ≤ 5 and J ≤ 20, from Huber and Herzberg 1979's \
               ω_e 4,401.213, ω_e x_e 121.336, ω_e y_e 0.8129, B_e 60.853, α_e 3.0622, γ_e 0.0577, \
               D_e 0.0471 and β_e −0.002 74 cm⁻¹ (the NIST WebBook's transcription as the finding \
               aid), frozen at 3:1 ortho to para, normal hydrogen, the adiabat's \
               (science-r08-gases-2.md §1.3). It meets Woolley, Scott and Brickwedde 1948's normal \
               hydrogen (J. Res. NBS 41, 379, Table 8) within 0.03 per cent from 30 to 300 K",
    basis: Basis::Fact,
};

/// Normal hydrogen's molecular constants: ortho- and para-hydrogen frozen at 3:1, the 3:1 of high
/// temperature that a parcel rising from depth carries (science-r08-gases-2.md §1.3). Row 0's.
const NORMAL_HYDROGEN: MolecularConstants = MolecularConstants {
    rotation: Rotation::InLevels,
    vibrations: &[],
    levels: &[
        LevelSet::Rovibrational {
            weight: 0.75,
            terms: &HYDROGEN_TERMS,
            spin: NuclearSpin::Ortho,
        },
        LevelSet::Rovibrational {
            weight: 0.25,
            terms: &HYDROGEN_TERMS,
            spin: NuclearSpin::Para,
        },
    ],
};

/// Equilibrium hydrogen's molecular constants: ortho- and para-hydrogen in equilibrium with each
/// other, the recorded other bound of a hydrogen atmosphere's adiabat, near which the giants'
/// upper tropospheres lie (science-r08-gases-2.md §1.3), and the hydrogen NASA Glenn's
/// polynomials describe. Row 0 holds normal hydrogen ([`heat_capacity_over_r_at`]); a reader that
/// wants this bound sums these.
///
/// [`heat_capacity_over_r_at`]: super::heat_capacity_over_r_at
pub const EQUILIBRIUM_HYDROGEN: MolecularConstants = MolecularConstants {
    rotation: Rotation::InLevels,
    vibrations: &[],
    levels: &[LevelSet::Rovibrational {
        weight: 1.0,
        terms: &HYDROGEN_TERMS,
        spin: NuclearSpin::Equilibrium,
    }],
};

/// The client optics of a gas the client scatters and that absorbs in its channels.
const RAYLEIGH_ABSORBER: ClientOptics = ClientOptics {
    rayleigh: true,
    absorber: true,
    aerosol_material: false,
};

/// The client optics of a gas that scatters, absorbs and can be a cloud's material.
const RAYLEIGH_ABSORBER_AEROSOL: ClientOptics = ClientOptics {
    rayleigh: true,
    absorber: true,
    aerosol_material: true,
};

/// The client optics of a gas the client scatters and that has no visible absorption: the noble
/// gases.
const RAYLEIGH_ONLY: ClientOptics = ClientOptics {
    rayleigh: true,
    absorber: false,
    aerosol_material: false,
};

/// The client optics of a charged species, which absorbs (its lines, or the electron's and H⁻'s
/// continua) and which the client does not scatter by Rayleigh's law.
const ABSORBER_ONLY: ClientOptics = ClientOptics {
    rayleigh: false,
    absorber: true,
    aerosol_material: false,
};

/// The client optics of a gas the client scatters, whose absorption R08 has not yet answered:
/// the atoms `Al`, `V`, `Mn`, `Ni` and `Zn` and `HCl`, which science-r08-unowned-absorbers adds.
/// The client's test holds every flagged row to a line table, a pending entry or a stated none,
/// which R08.T4.c and T4.e owe these rows (R08.T19's parity extension); their `absorber` flag
/// turns on at the 22 → 23 bump (P14.T54.a), with rows 0–8's.
const RAYLEIGH_AWAITING_ABSORPTION: ClientOptics = ClientOptics {
    rayleigh: true,
    absorber: false,
    aerosol_material: false,
};

/// The client optics of a first ion whose absorption R08 has not yet answered (`Na+`, `Al+`,
/// `Si+`, `K+`, `V+`, `Cr+`, `Mn+`, `Ni+` and `Zn+`): no Rayleigh row, and `absorber` held as for
/// [`RAYLEIGH_AWAITING_ABSORPTION`], owed T4.e's lines or T4.c's stated none.
const ION_AWAITING_ABSORPTION: ClientOptics = ClientOptics {
    rayleigh: false,
    absorber: false,
    aerosol_material: false,
};

/// The client optics of a condensate aloft, a haze or a dust: a cloud's or a dust's material
/// and nothing else, with an index file of its own (R08.T5.d).
const AEROSOL_ONLY: ClientOptics = ClientOptics {
    rayleigh: false,
    absorber: false,
    aerosol_material: true,
};

/// The client optics of a surface material, which the client draws by none of its atmospheric
/// optics.
const NO_CLIENT_OPTICS: ClientOptics = ClientOptics {
    rayleigh: false,
    absorber: false,
    aerosol_material: false,
};

/// Row 0, molecular hydrogen: a well-mixed gas, which condenses only near 20 K.
const HYDROGEN: Substance = Substance {
    key: SubstanceKey::new_const("H2"),
    kind: SubstanceKind::Molecule,
    elements: &[(Element::H, 2)],
    charge: 0,
    molar_mass_g_per_mol: Some(2.016),
    gas: Some(GasColumns {
        molecular: NORMAL_HYDROGEN,
        held_class: Some(HeatCapacityClass::Linear),
        sources: &[LINEAR_CLASS, HYDROGEN_TERMS_SOURCE],
    }),
    phase: None,
    diagram: None,
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
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::None,
            vibrations: &[],
            levels: &[],
        },
        held_class: Some(HeatCapacityClass::Atom),
        sources: &[
            ATOM_CLASS,
            Source {
                covers: "electronic levels",
                citation: "an atom whose first excited level lies 19.82 eV up (Martin 1973, J. Phys. Chem. Ref. Data 2, 257, with Kandula et al. 2010 and Drake 2023, as NIST's Atomic Spectra Database v5.12 lists it), so `c_p` ÷ R is 5/2 at every temperature the generator reaches",
                basis: Basis::Fact,
            },
        ],
    }),
    phase: None,
    diagram: None,
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
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::NonLinear,
            vibrations: &[
                vibration(3657.0, 1),
                vibration(1595.0, 1),
                vibration(3756.0, 1),
            ],
            levels: &[],
        },
        held_class: Some(HeatCapacityClass::NonLinear),
        sources: &[
            NON_LINEAR_CLASS,
            Source {
                covers: "rotation, vibrations",
                citation: "the fundamentals 3657, 1595, 3756 cm⁻¹ (Shimanouchi 1972, Tables of Molecular Vibrational Frequencies, Consolidated Volume I, NSRDS-NBS 39, p. 10, its selected values), as harmonic oscillators; a non-linear rotor",
                basis: Basis::Fact,
            },
        ],
    }),
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
    diagram: Some(diagrams::WATER),
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
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::NonLinear,
            vibrations: &[
                vibration(2917.0, 1),
                vibration(1534.0, 2),
                vibration(3019.0, 3),
                vibration(1306.0, 3),
            ],
            levels: &[],
        },
        held_class: Some(HeatCapacityClass::NonLinear),
        sources: &[
            NON_LINEAR_CLASS,
            Source {
                covers: "rotation, vibrations",
                citation: "the fundamentals 2917, 1534 (×2), 3019 (×3), 1306 (×3) cm⁻¹ (Shimanouchi 1972, Tables of Molecular Vibrational Frequencies, Consolidated Volume I, NSRDS-NBS 39, p. 45, its selected values), as harmonic oscillators; a non-linear rotor",
                basis: Basis::Fact,
            },
        ],
    }),
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
    diagram: Some(diagrams::METHANE),
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
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::NonLinear,
            vibrations: &[
                vibration(3337.0, 1),
                vibration(950.0, 1),
                vibration(3444.0, 2),
                vibration(1627.0, 2),
            ],
            levels: &[],
        },
        held_class: Some(HeatCapacityClass::NonLinear),
        sources: &[
            NON_LINEAR_CLASS,
            Source {
                covers: "rotation, vibrations",
                citation: "the fundamentals 3337, 950, 3444 (×2), 1627 (×2) cm⁻¹ (Shimanouchi 1972, Tables of Molecular Vibrational Frequencies, Consolidated Volume I, NSRDS-NBS 39, p. 15, its selected values), as harmonic oscillators; a non-linear rotor",
                basis: Basis::Fact,
            },
        ],
    }),
    phase: None,
    diagram: Some(diagrams::AMMONIA),
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
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::Linear,
            vibrations: &[vibration(2329.922, 1)],
            levels: &[],
        },
        held_class: Some(HeatCapacityClass::Linear),
        sources: &[
            LINEAR_CLASS,
            Source {
                covers: "rotation, vibrations, electronic levels",
                citation: "Huber and Herzberg 1979, Constants of Diatomic Molecules (the NIST WebBook's transcription as the finding aid): ω_e 2,358.57 and ω_e x_e 14.324 cm⁻¹, so the fundamental ω_e − 2ω_e x_e is 2,329.922 cm⁻¹; its X¹Σg⁺ ground alone lies below 15,000 cm⁻¹",
                basis: Basis::Fact,
            },
        ],
    }),
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
    diagram: Some(diagrams::NITROGEN),
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
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::Linear,
            vibrations: &[vibration(1556.231, 1)],
            levels: &[LevelSet::Listed {
                weight: 1.0,
                levels: &OXYGEN_STATES,
            }],
        },
        held_class: Some(HeatCapacityClass::Linear),
        sources: &[
            LINEAR_CLASS,
            Source {
                covers: "rotation, vibrations, electronic levels",
                citation: "Huber and Herzberg 1979 (the NIST WebBook's transcription as the finding aid): ω_e 1,580.193 and ω_e x_e 11.981 cm⁻¹, so the fundamental is 1,556.231 cm⁻¹; the X³Σg⁻ ground (g = 3, its spin–spin splitting of about 4 cm⁻¹ left out) and the a¹Δg (7,918.1 cm⁻¹, g = 2) and b¹Σg⁺ (13,195.1 cm⁻¹, g = 1) states at their T_e",
                basis: Basis::Fact,
            },
        ],
    }),
    phase: None,
    diagram: Some(diagrams::OXYGEN),
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
        molecular: MolecularConstants {
            rotation: Rotation::Linear,
            vibrations: &[
                vibration(1333.0, 1),
                vibration(667.0, 2),
                vibration(2349.0, 1),
            ],
            levels: &[],
        },
        held_class: Some(HeatCapacityClass::LinearTriatomic),
        sources: &[
            CARBON_DIOXIDE_CLASS,
            Source {
                covers: "rotation, vibrations",
                citation: "the fundamentals 1333, 667 (×2), 2349 cm⁻¹ (Shimanouchi 1972, Tables of Molecular Vibrational Frequencies, Consolidated Volume I, NSRDS-NBS 39, p. 39, its selected values), as harmonic oscillators; a linear rotor",
                basis: Basis::Fact,
            },
        ],
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
    diagram: Some(diagrams::CARBON_DIOXIDE),
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
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::None,
            vibrations: &[],
            levels: &[],
        },
        held_class: Some(HeatCapacityClass::Atom),
        sources: &[
            ATOM_CLASS,
            Source {
                covers: "electronic levels",
                citation: "an atom whose first excited level lies 11.55 eV up (Minnhagen 1973, J. Opt. Soc. Am. 63, 1185, as NIST's Atomic Spectra Database v5.12 lists it), so `c_p` ÷ R is 5/2 at every temperature the generator reaches",
                basis: Basis::Fact,
            },
        ],
    }),
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
    diagram: Some(diagrams::ARGON),
    optics: ClientOptics {
        rayleigh: true,
        absorber: false,
        aerosol_material: true,
    },
    sources: IDENTITY,
};

/// O2's electronic states below 15,000 cm⁻¹, each with its energy in cm⁻¹ above
/// the ground's lowest component (its source in its row's gas columns).
const OXYGEN_STATES: [Level; 3] = [level(0.0, 3.0), level(7918.1, 2.0), level(13_195.1, 1.0)];

/// Ethane's internal rotor, `(F, V₃)` in cm⁻¹: the reduced rotational constant F = 4 `A_R` =
/// 10.684 for two identical methyl tops and the barrier V₃ = 1,012.0, from Moazzen-Ahmadi et al.
/// 1988's `A_R` = 2.6711 and V₃ (J. Chem. Phys. 88, 563, read in its abstract); their V₆ = 10.71
/// is left out, the ruling's Hamiltonian being V₃'s alone (science-r08-gases-2.md §1.3), which
/// moves `c_p` by under 0.2 per cent. [`ETHANE_TORSION_LEVELS`] are its levels.
#[cfg(test)]
pub(super) const ETHANE_TORSION: (f64, f64) = (10.684, 1_012.0);

/// Methanol's internal rotor, `(F, V₃)` in cm⁻¹: F = 28.182 from Chen, Wilhoit and Zwolinski 1977's
/// reduced moment, 0.993 × 10⁻⁴⁰ g cm² (J. Phys. Chem. Ref. Data 6, 105, Table 1), and V₃ =
/// 373.21, Kwan and Dennison's 1,067.06 cal mol⁻¹ as they quote it; their V₆ = −0.52 is left out.
/// [`METHANOL_TORSION_LEVELS`] are its levels.
#[cfg(test)]
pub(super) const METHANOL_TORSION: (f64, f64) = (28.182, 373.21);

/// The ethane torsion's levels below 25,000 cm⁻¹, each with its energy in cm⁻¹
/// above the lowest: the eigenvalues of F p² + (V₃ ÷ 2)(1 − cos 3φ) in the free rotor's basis
/// e^{imφ}, m = r + 3k with |k| ≤ 60, whose classes r are tridiagonal: r = 0 (degeneracy 1) and
/// r = ±1 (degeneracy 2, the E levels), computed once by bisection on their Sturm sequences,
/// which a test repeats.
const ETHANE_TORSION_LEVELS: [Level; 65] = [
    level(0.0, 1.0),
    level(0.005_783_556_155_165_57, 2.0),
    level(285.438_675_877_235_65, 2.0),
    level(285.676_459_617_251_06, 1.0),
    level(537.396_238_880_202_6, 1.0),
    level(541.271_806_386_042_7, 2.0),
    level(742.934_386_156_078_7, 2.0),
    level(774.680_723_673_835_2, 1.0),
    level(875.610_451_618_627_8, 1.0),
    level(950.534_802_345_967_4, 2.0),
    level(1_092.466_595_342_965_3, 2.0),
    level(1_259.366_221_252_973_4, 1.0),
    level(1_263.005_684_346_626_4, 1.0),
    level(1_456.042_157_157_532_5, 2.0),
    level(1_674.660_252_314_961_3, 2.0),
    level(1_916.133_327_705_950_3, 1.0),
    level(1_916.149_523_220_551_6, 1.0),
    level(2_179.990_122_627_657, 2.0),
    level(2_465.908_225_548_100_7, 2.0),
    level(2_773.703_997_814_747_7, 1.0),
    level(2_773.704_020_258_244, 1.0),
    level(3_103.248_339_591_699, 2.0),
    level(3_454.451_149_208_135_6, 2.0),
    level(3_827.247_835_978_434_5, 1.0),
    level(3_827.247_835_991_471, 1.0),
    level(4_221.590_977_066_662, 2.0),
    level(4_637.445_067_882_398, 2.0),
    level(5_074.783_070_002_501, 1.0),
    level(5_074.783_070_002_504, 1.0),
    level(5_533.584_082_253_532, 2.0),
    level(6_013.831_732_031_389, 2.0),
    level(6_515.513_041_043_354, 1.0),
    level(6_515.513_041_043_354, 1.0),
    level(7_038.617_610_972_864, 2.0),
    level(7_583.137_029_348_414, 2.0),
    level(8_149.064_429_763_041, 1.0),
    level(8_149.064_429_763_041, 1.0),
    level(8_736.394_162_062_71, 2.0),
    level(9_345.121_542_044_99, 2.0),
    level(9_975.242_659_419_984, 1.0),
    level(9_975.242_659_419_984, 1.0),
    level(10_626.754_228_988_988, 2.0),
    level(11_299.653_474_243_541, 2.0),
    level(11_993.938_035_538_626, 1.0),
    level(11_993.938_035_538_626, 1.0),
    level(12_709.605_897_072_41, 2.0),
    level(13_446.655_328_387_804, 2.0),
    level(14_205.084_837_180_964, 1.0),
    level(14_205.084_837_180_964, 1.0),
    level(14_984.893_130_982_508, 2.0),
    level(15_786.079_085_852_145, 2.0),
    level(16_608.641_720_655_316, 1.0),
    level(16_608.641_720_655_316, 1.0),
    level(17_452.580_175_811_243, 2.0),
    level(18_317.893_695_644_772, 2.0),
    level(19_204.581_613_659_37, 1.0),
    level(19_204.581_613_659_37, 1.0),
    level(20_112.643_340_191_004, 2.0),
    level(21_042.078_352_012_584, 2.0),
    level(21_992.886_183_544_244, 1.0),
    level(21_992.886_183_544_244, 1.0),
    level(22_965.066_419_391_893, 2.0),
    level(23_958.618_687_989_4, 2.0),
    level(24_973.542_656_161_46, 1.0),
    level(24_973.542_656_161_46, 1.0),
];

/// The methanol torsion's levels below 25,000 cm⁻¹, each with its energy in cm⁻¹
/// above the lowest: the eigenvalues of F p² + (V₃ ÷ 2)(1 − cos 3φ) in the free rotor's basis
/// e^{imφ}, m = r + 3k with |k| ≤ 60, whose classes r are tridiagonal: r = 0 (degeneracy 1) and
/// r = ±1 (degeneracy 2, the E levels), computed once by bisection on their Sturm sequences,
/// which a test repeats.
const METHANOL_TORSION_LEVELS: [Level; 39] = [
    level(0.0, 1.0),
    level(9.557_013_860_719_735, 2.0),
    level(210.500_468_343_533, 2.0),
    level(299.771_603_631_604_25, 1.0),
    level(356.986_256_833_023_45, 1.0),
    level(520.001_314_143_579_1, 2.0),
    level(768.897_448_221_418, 2.0),
    level(1_076.490_543_086_181_6, 1.0),
    level(1_076.744_827_449_639_4, 1.0),
    level(1_441.697_559_592_403_9, 2.0),
    level(1_863.620_976_399_210_8, 2.0),
    level(2_342.173_505_752_247_4, 1.0),
    level(2_342.173_592_732_023_3, 1.0),
    level(2_877.249_707_170_553, 2.0),
    level(3_468.791_997_187_311_6, 2.0),
    level(4_116.766_813_565_07, 1.0),
    level(4_116.766_813_571_757, 1.0),
    level(4_821.153_377_514_324, 2.0),
    level(5_581.938_257_532_288, 2.0),
    level(6_399.112_448_306_533, 1.0),
    level(6_399.112_448_306_533, 1.0),
    level(7_272.669_724_054_006, 2.0),
    level(8_202.605_666_542_57, 2.0),
    level(9_188.917_068_638_215, 1.0),
    level(9_188.917_068_638_215, 1.0),
    level(10_231.601_555_923_562, 2.0),
    level(11_330.657_339_636_511, 2.0),
    level(12_486.083_051_224_092, 1.0),
    level(12_486.083_051_224_092, 1.0),
    level(13_697.877_629_045_242, 2.0),
    level(14_966.040_239_223_033, 2.0),
    level(16_290.570_219_355_004, 1.0),
    level(16_290.570_219_355_004, 1.0),
    level(17_671.467_037_828_11, 2.0),
    level(19_108.730_263_978_014, 2.0),
    level(20_602.359_545_907_526, 1.0),
    level(20_602.359_545_907_526, 1.0),
    level(22_152.354_593_795_073, 2.0),
    level(23_758.715_167_192_215, 2.0),
];

/// The O atom's levels below 15,000 cm⁻¹, `(energy, 2J + 1)` with the energy in cm⁻¹
/// above the ground (Moore 1993, CRC Series in Evaluated Data in Atomic Physics (Tables of Spectra of H, C, N and O), as NIST's Atomic Spectra Database v5.12 lists them).
const ATOMIC_OXYGEN_LEVELS: [Level; 3] =
    [level(0.0, 5.0), level(158.265, 3.0), level(226.977, 1.0)];

/// The K atom's levels below 15,000 cm⁻¹, `(energy, 2J + 1)` with the energy in cm⁻¹
/// above the ground (Sansonetti 2008, J. Phys. Chem. Ref. Data 37, 7, its 4p levels from Falke et al. 2006 (Phys. Rev. A 74, 032503), as NIST's Atomic Spectra Database v5.12 lists them).
const POTASSIUM_LEVELS: [Level; 3] = [
    level(0.0, 2.0),
    level(12_985.185_724, 2.0),
    level(13_042.896_027, 4.0),
];

/// The Fe atom's levels below 15,000 cm⁻¹, `(energy, 2J + 1)` with the energy in cm⁻¹
/// above the ground (Nave et al. 1994, ApJS 94, 221, as NIST's Atomic Spectra Database v5.12 lists them).
const IRON_LEVELS: [Level; 13] = [
    level(0.0, 9.0),
    level(415.933, 7.0),
    level(704.007, 5.0),
    level(888.132, 3.0),
    level(978.074, 1.0),
    level(6928.268, 11.0),
    level(7376.764, 9.0),
    level(7728.06, 7.0),
    level(7985.785, 5.0),
    level(8154.714, 3.0),
    level(11_976.239, 9.0),
    level(12_560.934, 7.0),
    level(12_968.554, 5.0),
];

/// The Si atom's levels below 15,000 cm⁻¹, `(energy, 2J + 1)` with the energy in cm⁻¹
/// above the ground (Martin and Zalubas 1983, J. Phys. Chem. Ref. Data 12, 323, as NIST's Atomic Spectra Database v5.12 lists them).
const SILICON_LEVELS: [Level; 4] = [
    level(0.0, 1.0),
    level(77.115, 3.0),
    level(223.157, 5.0),
    level(6298.85, 5.0),
];

/// The Ti atom's levels below 15,000 cm⁻¹, `(energy, 2J + 1)` with the energy in cm⁻¹
/// above the ground (Saloman 2012, J. Phys. Chem. Ref. Data 41, 013101, as NIST's Atomic Spectra Database v5.12 lists them).
const TITANIUM_LEVELS: [Level; 19] = [
    level(0.0, 5.0),
    level(170.1328, 7.0),
    level(386.874, 9.0),
    level(6556.833, 3.0),
    level(6598.765, 5.0),
    level(6661.006, 7.0),
    level(6742.756, 9.0),
    level(6842.962, 11.0),
    level(7255.355, 5.0),
    level(8436.618, 1.0),
    level(8492.422, 3.0),
    level(8602.3441, 5.0),
    level(11_531.761, 5.0),
    level(11_639.810_9, 7.0),
    level(11_776.812, 9.0),
    level(12_118.393, 9.0),
    level(13_981.773, 3.0),
    level(14_028.436, 5.0),
    level(14_105.634, 7.0),
];

/// `TiO`'s electronic states below 15,000 cm⁻¹, each with its energy in cm⁻¹ above
/// the ground's lowest component (its source in its row's gas columns).
const TITANIUM_MONOXIDE_STATES: [Level; 10] = [
    level(0.0, 2.0),
    level(96.4, 2.0),
    level(197.5, 2.0),
    level(3440.0, 2.0),
    level(5655.6, 1.0),
    level(12_025.0, 6.0),
    level(14_089.91, 2.0),
    level(14_262.8, 2.0),
    level(14_431.0, 2.0),
    level(14_762.0, 2.0),
];

/// VO's electronic states below 15,000 cm⁻¹, each with its energy in cm⁻¹ above
/// the ground's lowest component (its source in its row's gas columns).
const VANADIUM_MONOXIDE_STATES: [Level; 5] = [
    level(0.0, 4.0),
    level(12_567.0, 2.0),
    level(12_625.0, 2.0),
    level(12_689.0, 2.0),
    level(12_760.8, 2.0),
];

/// `FeH`'s electronic states below 15,000 cm⁻¹, each with its energy in cm⁻¹ above
/// the ground's lowest component (its source in its row's gas columns).
const IRON_HYDRIDE_STATES: [Level; 29] = [
    level(0.0, 2.0),
    level(191.0, 2.0),
    level(426.0, 2.0),
    level(695.0, 2.0),
    level(910.0, 2.0),
    level(1068.0, 2.0),
    level(1228.0, 2.0),
    level(1388.0, 2.0),
    level(1766.0, 2.0),
    level(1981.0, 2.0),
    level(2177.0, 2.0),
    level(2356.0, 2.0),
    level(2518.0, 2.0),
    level(2656.0, 2.0),
    level(3174.0, 2.0),
    level(3375.0, 2.0),
    level(3575.0, 2.0),
    level(3735.0, 2.0),
    level(3889.0, 2.0),
    level(3939.0, 2.0),
    level(3989.0, 2.0),
    level(4039.0, 2.0),
    level(4089.0, 2.0),
    level(4139.0, 2.0),
    level(4750.0, 6.0),
    level(5089.0, 4.0),
    level(7158.0, 4.0),
    level(7300.0, 8.0),
    level(9995.0, 8.0),
];

/// The Al atom's levels below 15,000 cm⁻¹, `(energy, 2J + 1)` with the energy in cm⁻¹
/// above the ground (Martin and Zalubas 1979, J. Phys. Chem. Ref. Data 8, 817, as NIST's Atomic Spectra Database v5.12 lists them).
const ALUMINIUM_LEVELS: [Level; 2] = [level(0.0, 2.0), level(112.061, 4.0)];

/// The V atom's levels below 15,000 cm⁻¹, `(energy, 2J + 1)` with the energy in cm⁻¹
/// above the ground (Saloman and Kramida 2017, ApJS 231, 18, as NIST's Atomic Spectra Database v5.12 lists them).
const VANADIUM_LEVELS: [Level; 24] = [
    level(0.0, 4.0),
    level(137.383, 6.0),
    level(323.432, 8.0),
    level(552.955, 10.0),
    level(2112.282, 2.0),
    level(2153.221, 4.0),
    level(2220.156, 6.0),
    level(2311.369, 8.0),
    level(2424.809, 10.0),
    level(8413.009, 2.0),
    level(8476.234, 4.0),
    level(8578.542, 6.0),
    level(8715.747, 8.0),
    level(9544.635, 2.0),
    level(9637.039, 4.0),
    level(9824.626, 6.0),
    level(10_892.52, 8.0),
    level(11_100.596, 10.0),
    level(13_801.551, 4.0),
    level(13_810.91, 2.0),
    level(14_514.756, 4.0),
    level(14_548.816, 6.0),
    level(14_909.958, 8.0),
    level(14_949.359, 10.0),
];

/// The Ni atom's levels below 15,000 cm⁻¹, `(energy, 2J + 1)` with the energy in cm⁻¹
/// above the ground (Litzén, Brault and Thorne 1993, Phys. Scr. 47, 628, as NIST's Atomic Spectra Database v5.12 lists them).
const NICKEL_LEVELS: [Level; 9] = [
    level(0.0, 9.0),
    level(204.787, 7.0),
    level(879.816, 5.0),
    level(1332.164, 7.0),
    level(1713.087, 3.0),
    level(2216.55, 5.0),
    level(3409.937, 5.0),
    level(13_521.347, 5.0),
    level(14_728.84, 1.0),
];

/// The Si+ ion's levels below 15,000 cm⁻¹, `(energy, 2J + 1)` with the energy in cm⁻¹
/// above the ground (Martin and Zalubas 1983, J. Phys. Chem. Ref. Data 12, 323, as NIST's Atomic Spectra Database v5.12 lists them).
const SILICON_ION_LEVELS: [Level; 2] = [level(0.0, 2.0), level(287.24, 4.0)];

/// The Ca+ ion's levels below 15,000 cm⁻¹, `(energy, 2J + 1)` with the energy in cm⁻¹
/// above the ground (Sugar and Corliss 1985, J. Phys. Chem. Ref. Data 14, Suppl. 2, as NIST's Atomic Spectra Database v5.12 lists them).
const CALCIUM_ION_LEVELS: [Level; 3] = [
    level(0.0, 2.0),
    level(13_650.19, 4.0),
    level(13_710.88, 6.0),
];

/// The Ti+ ion's levels below 15,000 cm⁻¹, `(energy, 2J + 1)` with the energy in cm⁻¹
/// above the ground (Saloman 2012, J. Phys. Chem. Ref. Data 41, 013101, as NIST's Atomic Spectra Database v5.12 lists them).
const TITANIUM_ION_LEVELS: [Level; 26] = [
    level(0.0, 4.0),
    level(94.1142, 6.0),
    level(225.7039, 8.0),
    level(393.4459, 10.0),
    level(907.9672, 4.0),
    level(983.9157, 6.0),
    level(1087.3561, 8.0),
    level(1215.8329, 10.0),
    level(4628.6571, 6.0),
    level(4897.7179, 8.0),
    level(8710.5675, 4.0),
    level(8744.3406, 6.0),
    level(8997.7874, 8.0),
    level(9118.2849, 10.0),
    level(9363.7425, 2.0),
    level(9395.802, 4.0),
    level(9518.1516, 6.0),
    level(9851.0145, 2.0),
    level(9872.899, 2.0),
    level(9930.7766, 4.0),
    level(9975.9994, 4.0),
    level(10_024.800_9, 6.0),
    level(12_628.845_5, 4.0),
    level(12_677.105, 10.0),
    level(12_758.259_7, 6.0),
    level(12_774.816_8, 12.0),
];

/// The V+ ion's levels below 15,000 cm⁻¹, `(energy, 2J + 1)` with the energy in cm⁻¹
/// above the ground (Saloman and Kramida 2017, ApJS 231, 19, as NIST's Atomic Spectra Database v5.12 lists them).
const VANADIUM_ION_LEVELS: [Level; 28] = [
    level(0.0, 1.0),
    level(36.1017, 3.0),
    level(106.6421, 5.0),
    level(208.7901, 7.0),
    level(339.1256, 9.0),
    level(2605.04, 3.0),
    level(2687.209, 5.0),
    level(2808.959, 7.0),
    level(2968.3883, 9.0),
    level(3162.9673, 11.0),
    level(8640.3641, 5.0),
    level(8842.0507, 7.0),
    level(9097.8891, 9.0),
    level(11_295.511, 1.0),
    level(11_514.784_5, 3.0),
    level(11_908.259_3, 5.0),
    level(12_545.102_4, 9.0),
    level(12_621.485_7, 11.0),
    level(12_706.079_8, 13.0),
    level(13_490.884_3, 5.0),
    level(13_511.799_3, 3.0),
    level(13_542.646_1, 7.0),
    level(13_594.726, 5.0),
    level(13_608.939_3, 9.0),
    level(13_741.639_4, 7.0),
    level(14_461.749_5, 7.0),
    level(14_556.068_9, 9.0),
    level(14_655.608_2, 11.0),
];

/// The Cr+ ion's levels below 15,000 cm⁻¹, `(energy, 2J + 1)` with the energy in cm⁻¹
/// above the ground (Sansonetti and Nave 2014, ApJS 213, 28, as NIST's Atomic Spectra Database v5.12 lists them).
const CHROMIUM_ION_LEVELS: [Level; 6] = [
    level(0.0, 6.0),
    level(11_961.746_4, 2.0),
    level(12_032.544_7, 4.0),
    level(12_147.771_3, 6.0),
    level(12_303.82, 8.0),
    level(12_496.456_5, 10.0),
];

/// The Mn+ ion's levels below 15,000 cm⁻¹, `(energy, 2J + 1)` with the energy in cm⁻¹
/// above the ground (Kramida and Sansonetti 2013, ApJS 205, 14, as NIST's Atomic Spectra Database v5.12 lists them).
const MANGANESE_ION_LEVELS: [Level; 7] = [
    level(0.0, 7.0),
    level(9472.993, 5.0),
    level(14_325.866, 9.0),
    level(14_593.835, 7.0),
    level(14_781.205, 5.0),
    level(14_901.203, 3.0),
    level(14_959.876, 1.0),
];

/// The Fe+ ion's levels below 15,000 cm⁻¹, `(energy, 2J + 1)` with the energy in cm⁻¹
/// above the ground (Nave and Johansson 2013, ApJS 204, 1, as NIST's Atomic Spectra Database v5.12 lists them).
const IRON_ION_LEVELS: [Level; 16] = [
    level(0.0, 10.0),
    level(384.7872, 8.0),
    level(667.6829, 6.0),
    level(862.6118, 4.0),
    level(977.0498, 2.0),
    level(1872.5998, 10.0),
    level(2430.1369, 8.0),
    level(2837.9807, 6.0),
    level(3117.4877, 4.0),
    level(7955.3186, 8.0),
    level(8391.9554, 6.0),
    level(8680.4706, 4.0),
    level(8846.7837, 2.0),
    level(13_474.447_4, 6.0),
    level(13_673.204_5, 4.0),
    level(13_904.860_4, 2.0),
];

/// The Ni+ ion's levels below 15,000 cm⁻¹, `(energy, 2J + 1)` with the energy in cm⁻¹
/// above the ground (Sugar and Corliss 1985, J. Phys. Chem. Ref. Data 14, Suppl. 2, as NIST's Atomic Spectra Database v5.12 lists them).
const NICKEL_ION_LEVELS: [Level; 8] = [
    level(0.0, 6.0),
    level(1506.94, 4.0),
    level(8393.9, 10.0),
    level(9330.04, 8.0),
    level(10_115.66, 6.0),
    level(10_663.89, 4.0),
    level(13_550.39, 8.0),
    level(14_995.57, 6.0),
];

/// The Cr atom's levels below 15,000 cm⁻¹, `(energy, 2J + 1)` with the energy in cm⁻¹
/// above the ground (Saloman 2012, J. Phys. Chem. Ref. Data 41, 043103, as NIST's Atomic Spectra Database v5.12 lists them).
const CHROMIUM_LEVELS: [Level; 7] = [
    level(0.0, 7.0),
    level(7593.1484, 5.0),
    level(7750.7465, 1.0),
    level(7810.7795, 3.0),
    level(7927.441, 5.0),
    level(8095.1842, 7.0),
    level(8307.5753, 9.0),
];

/// Row 9, neon, a noble gas R08 measures.
const NEON: Substance = Substance {
    key: SubstanceKey::new_const("Ne"),
    kind: SubstanceKind::Atom,
    elements: &[(Element::Ne, 1)],
    charge: 0,
    molar_mass_g_per_mol: Some(20.18),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::None,
            vibrations: &[],
            levels: &[],
        },
        held_class: None,
        sources: &[Source {
            covers: "electronic levels",
            citation: "an atom whose first excited level lies 16.62 eV up (Saloman and Sansonetti 2004, J. Phys. Chem. Ref. Data 33, 1113, as NIST's Atomic Spectra Database v5.12 lists it), so `c_p` ÷ R is 5/2 at every temperature the generator reaches",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: RAYLEIGH_ONLY,
    sources: IDENTITY,
};

/// Row 10, krypton, a noble gas R08 measures.
const KRYPTON: Substance = Substance {
    key: SubstanceKey::new_const("Kr"),
    kind: SubstanceKind::Atom,
    elements: &[(Element::Kr, 1)],
    charge: 0,
    molar_mass_g_per_mol: Some(83.798),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::None,
            vibrations: &[],
            levels: &[],
        },
        held_class: None,
        sources: &[Source {
            covers: "electronic levels",
            citation: "an atom whose first excited level lies 9.92 eV up (Saloman 2007, J. Phys. Chem. Ref. Data 36, 215, as NIST's Atomic Spectra Database v5.12 lists it), so `c_p` ÷ R is 5/2 at every temperature the generator reaches",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: Some(diagrams::KRYPTON),
    optics: RAYLEIGH_ONLY,
    sources: IDENTITY,
};

/// Row 11, xenon, a noble gas R08 measures.
const XENON: Substance = Substance {
    key: SubstanceKey::new_const("Xe"),
    kind: SubstanceKind::Atom,
    elements: &[(Element::Xe, 1)],
    charge: 0,
    molar_mass_g_per_mol: Some(131.29),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::None,
            vibrations: &[],
            levels: &[],
        },
        held_class: None,
        sources: &[Source {
            covers: "electronic levels",
            citation: "an atom whose first excited level lies 8.32 eV up (Saloman 2004, J. Phys. Chem. Ref. Data 33, 765, as NIST's Atomic Spectra Database v5.12 lists it), so `c_p` ÷ R is 5/2 at every temperature the generator reaches",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: Some(diagrams::XENON),
    optics: RAYLEIGH_ONLY,
    sources: IDENTITY,
};

/// Row 12, nitrous oxide, a biotic or photochemical trace gas.
const NITROUS_OXIDE: Substance = Substance {
    key: SubstanceKey::new_const("N2O"),
    kind: SubstanceKind::Molecule,
    elements: &[(Element::N, 2), (Element::O, 1)],
    charge: 0,
    molar_mass_g_per_mol: Some(44.013),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::Linear,
            vibrations: &[
                vibration(2224.0, 1),
                vibration(589.0, 2),
                vibration(1285.0, 1),
            ],
            levels: &[],
        },
        held_class: None,
        sources: &[Source {
            covers: "rotation, vibrations",
            citation: "the fundamentals 2224, 589 (×2), 1285 cm⁻¹ (Shimanouchi 1972, Tables of Molecular Vibrational Frequencies, Consolidated Volume I, NSRDS-NBS 39, p. 9, its selected values), as harmonic oscillators; a linear rotor",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: Some(diagrams::NITROUS_OXIDE),
    optics: RAYLEIGH_ABSORBER,
    sources: IDENTITY,
};

/// Row 13, carbon monoxide: a desiccated CO2 world's, a hot giant's carbon, Pluto's and Triton's ice.
const CARBON_MONOXIDE: Substance = Substance {
    key: SubstanceKey::new_const("CO"),
    kind: SubstanceKind::Molecule,
    elements: &[(Element::C, 1), (Element::O, 1)],
    charge: 0,
    molar_mass_g_per_mol: Some(28.01),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::Linear,
            vibrations: &[vibration(2143.237, 1)],
            levels: &[],
        },
        held_class: None,
        sources: &[Source {
            covers: "rotation, vibrations, electronic levels",
            citation: "Huber and Herzberg 1979, Constants of Diatomic Molecules (the NIST WebBook's transcription as the finding aid): ω_e 2,169.813 58 and ω_e x_e 13.288 31 cm⁻¹, so the fundamental ω_e − 2ω_e x_e is 2,143.237 cm⁻¹; its X¹Σ⁺ ground alone lies below 15,000 cm⁻¹",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: Some(diagrams::CARBON_MONOXIDE),
    optics: RAYLEIGH_ABSORBER_AEROSOL,
    sources: IDENTITY,
};

/// Row 14, sulphur dioxide: Venus's and Io's, and Io's frost.
const SULPHUR_DIOXIDE: Substance = Substance {
    key: SubstanceKey::new_const("SO2"),
    kind: SubstanceKind::Molecule,
    elements: &[(Element::S, 1), (Element::O, 2)],
    charge: 0,
    molar_mass_g_per_mol: Some(64.058),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::NonLinear,
            vibrations: &[
                vibration(1151.0, 1),
                vibration(518.0, 1),
                vibration(1362.0, 1),
            ],
            levels: &[],
        },
        held_class: None,
        sources: &[Source {
            covers: "rotation, vibrations",
            citation: "the fundamentals 1151, 518, 1362 cm⁻¹ (Shimanouchi 1972, Tables of Molecular Vibrational Frequencies, Consolidated Volume I, NSRDS-NBS 39, p. 13, its selected values), as harmonic oscillators; a non-linear rotor",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: Some(diagrams::SULPHUR_DIOXIDE),
    optics: RAYLEIGH_ABSORBER,
    sources: IDENTITY,
};

/// Row 15, hydrogen sulphide: a giant's sulphur, and the ice giants' deck.
const HYDROGEN_SULPHIDE: Substance = Substance {
    key: SubstanceKey::new_const("H2S"),
    kind: SubstanceKind::Molecule,
    elements: &[(Element::H, 2), (Element::S, 1)],
    charge: 0,
    molar_mass_g_per_mol: Some(34.076),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::NonLinear,
            vibrations: &[
                vibration(2615.0, 1),
                vibration(1183.0, 1),
                vibration(2626.0, 1),
            ],
            levels: &[],
        },
        held_class: None,
        sources: &[Source {
            covers: "rotation, vibrations",
            citation: "the fundamentals 2615, 1183, 2626 cm⁻¹ (Shimanouchi 1972, Tables of Molecular Vibrational Frequencies, Consolidated Volume I, NSRDS-NBS 39, p. 12, its selected values), as harmonic oscillators; a non-linear rotor",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: Some(diagrams::HYDROGEN_SULPHIDE),
    optics: RAYLEIGH_ABSORBER_AEROSOL,
    sources: IDENTITY,
};

/// Row 16, ozone, from an oxygen-rich air's photochemistry.
const OZONE: Substance = Substance {
    key: SubstanceKey::new_const("O3"),
    kind: SubstanceKind::Molecule,
    elements: &[(Element::O, 3)],
    charge: 0,
    molar_mass_g_per_mol: Some(47.997),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::NonLinear,
            vibrations: &[
                vibration(1103.0, 1),
                vibration(701.0, 1),
                vibration(1042.0, 1),
            ],
            levels: &[],
        },
        held_class: None,
        sources: &[Source {
            covers: "rotation, vibrations",
            citation: "the fundamentals 1103, 701, 1042 cm⁻¹ (Shimanouchi 1977, Tables of Molecular Vibrational Frequencies, Consolidated Volume II, J. Phys. Chem. Ref. Data 6, 993, p. 1001, its selected values), as harmonic oscillators; a non-linear rotor",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: RAYLEIGH_ABSORBER,
    sources: IDENTITY,
};

/// Row 17, hydrogen cyanide: Titan's photochemistry and its stratospheric ice.
const HYDROGEN_CYANIDE: Substance = Substance {
    key: SubstanceKey::new_const("HCN"),
    kind: SubstanceKind::Molecule,
    elements: &[(Element::H, 1), (Element::C, 1), (Element::N, 1)],
    charge: 0,
    molar_mass_g_per_mol: Some(27.026),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::Linear,
            vibrations: &[
                vibration(3311.0, 1),
                vibration(712.0, 2),
                vibration(2097.0, 1),
            ],
            levels: &[],
        },
        held_class: None,
        sources: &[Source {
            covers: "rotation, vibrations",
            citation: "the fundamentals 3311, 712 (×2), 2097 cm⁻¹ (Shimanouchi 1972, Tables of Molecular Vibrational Frequencies, Consolidated Volume I, NSRDS-NBS 39, p. 41, its selected values), as harmonic oscillators; a linear rotor",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: RAYLEIGH_ABSORBER,
    sources: IDENTITY,
};

/// Row 18, acetylene, a hydrocarbon of the giants' and Titan's photochemistry.
const ACETYLENE: Substance = Substance {
    key: SubstanceKey::new_const("C2H2"),
    kind: SubstanceKind::Molecule,
    elements: &[(Element::C, 2), (Element::H, 2)],
    charge: 0,
    molar_mass_g_per_mol: Some(26.038),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::Linear,
            vibrations: &[
                vibration(3374.0, 1),
                vibration(1974.0, 1),
                vibration(3289.0, 1),
                vibration(612.0, 2),
                vibration(730.0, 2),
            ],
            levels: &[],
        },
        held_class: None,
        sources: &[Source {
            covers: "rotation, vibrations",
            citation: "the fundamentals 3374, 1974, 3289, 612 (×2), 730 (×2) cm⁻¹ (Shimanouchi 1972, Tables of Molecular Vibrational Frequencies, Consolidated Volume I, NSRDS-NBS 39, p. 71, its selected values), as harmonic oscillators; a linear rotor",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: RAYLEIGH_ABSORBER,
    sources: IDENTITY,
};

/// Row 19, ethylene, a hydrocarbon of the giants' and Titan's photochemistry.
const ETHYLENE: Substance = Substance {
    key: SubstanceKey::new_const("C2H4"),
    kind: SubstanceKind::Molecule,
    elements: &[(Element::C, 2), (Element::H, 4)],
    charge: 0,
    molar_mass_g_per_mol: Some(28.054),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::NonLinear,
            vibrations: &[
                vibration(3026.0, 1),
                vibration(1623.0, 1),
                vibration(1342.0, 1),
                vibration(1023.0, 1),
                vibration(3103.0, 1),
                vibration(1236.0, 1),
                vibration(949.0, 1),
                vibration(943.0, 1),
                vibration(3106.0, 1),
                vibration(826.0, 1),
                vibration(2989.0, 1),
                vibration(1444.0, 1),
            ],
            levels: &[],
        },
        held_class: None,
        sources: &[Source {
            covers: "rotation, vibrations",
            citation: "the fundamentals 3026, 1623, 1342, 1023, 3103, 1236, 949, 943, 3106, 826, 2989, 1444 cm⁻¹ (Shimanouchi 1972, Tables of Molecular Vibrational Frequencies, Consolidated Volume I, NSRDS-NBS 39, p. 74, its selected values), as harmonic oscillators; a non-linear rotor",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: RAYLEIGH_ABSORBER,
    sources: IDENTITY,
};

/// Row 20, ethane: Titan's second hydrocarbon, its lakes and its clouds.
const ETHANE: Substance = Substance {
    key: SubstanceKey::new_const("C2H6"),
    kind: SubstanceKind::Molecule,
    elements: &[(Element::C, 2), (Element::H, 6)],
    charge: 0,
    molar_mass_g_per_mol: Some(30.07),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::NonLinear,
            vibrations: &[
                vibration(2954.0, 1),
                vibration(1388.0, 1),
                vibration(995.0, 1),
                vibration(2896.0, 1),
                vibration(1379.0, 1),
                vibration(2969.0, 2),
                vibration(1468.0, 2),
                vibration(1190.0, 2),
                vibration(2985.0, 2),
                vibration(1469.0, 2),
                vibration(822.0, 2),
            ],
            levels: &[LevelSet::Listed {
                weight: 1.0,
                levels: &ETHANE_TORSION_LEVELS,
            }],
        },
        held_class: None,
        sources: &[
            Source {
                covers: "rotation, vibrations",
                citation: "the fundamentals but the 289 cm⁻¹ torsion, ν₄, 2954, 1388, 995, 2896, 1379, 2969 (×2), 1468 (×2), 1190 (×2), 2985 (×2), 1469 (×2), 822 (×2) cm⁻¹ (Shimanouchi 1972, Tables of Molecular Vibrational Frequencies, Consolidated Volume I, NSRDS-NBS 39, p. 92, its selected values), as harmonic oscillators; a non-linear rotor",
                basis: Basis::Fact,
            },
            Source {
                covers: "torsion",
                citation: "the internal rotation as the levels of F p² + (V₃ ÷ 2)(1 − cos 3φ), [`ETHANE_TORSION_LEVELS`], with V₃ = 1,012.0 cm⁻¹ and F = 4 A_R = 10.684 cm⁻¹ from Moazzen-Ahmadi et al. 1988's A_R = 2.6711 cm⁻¹ (J. Chem. Phys. 88, 563; read in its abstract), F being four times the reduced constant for two identical methyl tops; their V₆ = 10.71 cm⁻¹ is left out, as the ruling's Hamiltonian is V₃'s, which moves `c_p` by under 0.2 per cent. Chao, Wilhoit and Zwolinski 1973 (J. Phys. Chem. Ref. Data 2, 427, Table 6) give `c_p` ÷ R 4.287 at 100 K, against the sum's 4.298",
                basis: Basis::Fact,
            },
        ],
    }),
    phase: None,
    // A cloud's material since R08.T5.g's `ethane-liquid.json`, the client's index file for it.
    diagram: Some(diagrams::ETHANE),
    optics: RAYLEIGH_ABSORBER_AEROSOL,
    sources: IDENTITY,
};

/// Row 21, phosphine, the giants' phosphorus, quenched from depth.
const PHOSPHINE: Substance = Substance {
    key: SubstanceKey::new_const("PH3"),
    kind: SubstanceKind::Molecule,
    elements: &[(Element::P, 1), (Element::H, 3)],
    charge: 0,
    molar_mass_g_per_mol: Some(33.998),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::NonLinear,
            vibrations: &[
                vibration(2323.0, 1),
                vibration(992.0, 1),
                vibration(2328.0, 2),
                vibration(1118.0, 2),
            ],
            levels: &[],
        },
        held_class: None,
        sources: &[Source {
            covers: "rotation, vibrations",
            citation: "the fundamentals 2323, 992, 2328 (×2), 1118 (×2) cm⁻¹ (Shimanouchi 1972, Tables of Molecular Vibrational Frequencies, Consolidated Volume I, NSRDS-NBS 39, p. 16, its selected values), as harmonic oscillators; a non-linear rotor",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: RAYLEIGH_ABSORBER,
    sources: IDENTITY,
};

/// Row 22, methanol, an outgassed and cometary organic.
const METHANOL: Substance = Substance {
    key: SubstanceKey::new_const("CH3OH"),
    kind: SubstanceKind::Molecule,
    elements: &[(Element::C, 1), (Element::H, 4), (Element::O, 1)],
    charge: 0,
    molar_mass_g_per_mol: Some(32.042),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::NonLinear,
            vibrations: &[
                vibration(3681.0, 1),
                vibration(3000.0, 1),
                vibration(2844.0, 1),
                vibration(1477.0, 1),
                vibration(1455.0, 1),
                vibration(1345.0, 1),
                vibration(1060.0, 1),
                vibration(1033.0, 1),
                vibration(2960.0, 1),
                vibration(1477.0, 1),
                vibration(1165.0, 1),
            ],
            levels: &[LevelSet::Listed {
                weight: 1.0,
                levels: &METHANOL_TORSION_LEVELS,
            }],
        },
        held_class: None,
        sources: &[
            Source {
                covers: "rotation, vibrations",
                citation: "the fundamentals but the 295 cm⁻¹ torsion, ν₁₂, 3681, 3000, 2844, 1477, 1455, 1345, 1060, 1033, 2960, 1477, 1165 cm⁻¹ (Shimanouchi 1972, Tables of Molecular Vibrational Frequencies, Consolidated Volume I, NSRDS-NBS 39, p. 63, its selected values), as harmonic oscillators; a non-linear rotor",
                basis: Basis::Fact,
            },
            Source {
                covers: "torsion",
                citation: "the internal rotation as the levels of F p² + (V₃ ÷ 2)(1 − cos 3φ), [`METHANOL_TORSION_LEVELS`], with V₃ = 373.21 cm⁻¹ (1,067.06 cal mol⁻¹, Kwan and Dennison's, as Chen, Wilhoit and Zwolinski 1977, J. Phys. Chem. Ref. Data 6, 105, Table 1, quote it) and F = 28.182 cm⁻¹ (their reduced moment, 0.993 × 10⁻⁴⁰ g cm²); their V₆ = −0.52 cm⁻¹ is left out. Their Table 4 gives `c_p` ÷ R 4.438 at 100 K, as the sum does",
                basis: Basis::Fact,
            },
        ],
    }),
    phase: None,
    diagram: None,
    optics: RAYLEIGH_ABSORBER,
    sources: IDENTITY,
};

/// Row 23, atomic hydrogen, an ultra-hot dayside's dissociated H2.
const ATOMIC_HYDROGEN: Substance = Substance {
    key: SubstanceKey::new_const("H"),
    kind: SubstanceKind::Atom,
    elements: &[(Element::H, 1)],
    charge: 0,
    molar_mass_g_per_mol: Some(1.008),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::None,
            vibrations: &[],
            levels: &[],
        },
        held_class: None,
        sources: &[Source {
            covers: "electronic levels",
            citation: "its ground level alone below 15,000 cm⁻¹, so its `c_p` ÷ R is 5/2 (Kramida 2010, At. Data Nucl. Data Tables 96, 586; NIST's Atomic Spectra Database v5.12, Kramida et al. 2024, as the finding aid, NIST holding the compilations' values; Moore's NBS Circular 467 agrees for Fe I, Ti I and O I to 0.5 cm⁻¹). A cut at 15,000 cm⁻¹ holds `c_p` within 0.8 per cent to 2,000 K, but misses up to 14 per cent at 3,000 K (Ca I), recorded",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: RAYLEIGH_ABSORBER,
    sources: IDENTITY,
};

/// Row 24, atomic oxygen, dissociated on ultra-hot daysides.
const ATOMIC_OXYGEN: Substance = Substance {
    key: SubstanceKey::new_const("O"),
    kind: SubstanceKind::Atom,
    elements: &[(Element::O, 1)],
    charge: 0,
    molar_mass_g_per_mol: Some(15.999),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::None,
            vibrations: &[],
            levels: &[LevelSet::Listed {
                weight: 1.0,
                levels: &ATOMIC_OXYGEN_LEVELS,
            }],
        },
        held_class: None,
        sources: &[Source {
            covers: "electronic levels",
            citation: "its 3 levels below 15,000 cm⁻¹ (Moore 1993, CRC Series in Evaluated Data in Atomic Physics (Tables of Spectra of H, C, N and O); NIST's Atomic Spectra Database v5.12, Kramida et al. 2024, as the finding aid, NIST holding the compilations' values; Moore's NBS Circular 467 agrees for Fe I, Ti I and O I to 0.5 cm⁻¹). A cut at 15,000 cm⁻¹ holds `c_p` within 0.8 per cent to 2,000 K, but misses up to 14 per cent at 3,000 K (Ca I), recorded",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: RAYLEIGH_ABSORBER,
    sources: IDENTITY,
};

/// Row 25, the free electron, balancing the first ions' charge.
const ELECTRON: Substance = Substance {
    key: SubstanceKey::new_const("e-"),
    kind: SubstanceKind::Ion,
    elements: &[],
    charge: -1,
    molar_mass_g_per_mol: Some(5.485_799_088_8e-4),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::None,
            vibrations: &[],
            levels: &[],
        },
        held_class: None,
        sources: &[Source {
            covers: "molecular constants",
            citation: "the free electron, translation alone: `c_p` ÷ R is 5/2 (science-r08-gases-2.md §1.2)",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: ABSORBER_ONLY,
    sources: IDENTITY,
};

/// Row 26, the hydride ion, the cool stars' and hot giants' continuum.
const HYDRIDE: Substance = Substance {
    key: SubstanceKey::new_const("H-"),
    kind: SubstanceKind::Ion,
    elements: &[(Element::H, 1)],
    charge: -1,
    molar_mass_g_per_mol: Some(1.008_548_579_908_88),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::None,
            vibrations: &[],
            levels: &[],
        },
        held_class: None,
        sources: &[Source {
            covers: "molecular constants",
            citation: "the hydride ion, whose one bound state is its ¹S ground: `c_p` ÷ R is 5/2 (science-r08-gases-2.md §1.2)",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: ABSORBER_ONLY,
    sources: IDENTITY,
};

/// Row 27, sodium vapour, a warm giant's alkali.
const SODIUM: Substance = Substance {
    key: SubstanceKey::new_const("Na"),
    kind: SubstanceKind::Atom,
    elements: &[(Element::Na, 1)],
    charge: 0,
    molar_mass_g_per_mol: Some(22.99),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::None,
            vibrations: &[],
            levels: &[],
        },
        held_class: None,
        sources: &[Source {
            covers: "electronic levels",
            citation: "its ground level alone below 15,000 cm⁻¹, so its `c_p` ÷ R is 5/2 (Sansonetti 2008, J. Phys. Chem. Ref. Data 37, 1659; NIST's Atomic Spectra Database v5.12, Kramida et al. 2024, as the finding aid, NIST holding the compilations' values; Moore's NBS Circular 467 agrees for Fe I, Ti I and O I to 0.5 cm⁻¹). A cut at 15,000 cm⁻¹ holds `c_p` within 0.8 per cent to 2,000 K, but misses up to 14 per cent at 3,000 K (Ca I), recorded",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: RAYLEIGH_ABSORBER,
    sources: IDENTITY,
};

/// Row 28, potassium vapour, a warm giant's alkali.
const POTASSIUM: Substance = Substance {
    key: SubstanceKey::new_const("K"),
    kind: SubstanceKind::Atom,
    elements: &[(Element::K, 1)],
    charge: 0,
    molar_mass_g_per_mol: Some(39.098),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::None,
            vibrations: &[],
            levels: &[LevelSet::Listed {
                weight: 1.0,
                levels: &POTASSIUM_LEVELS,
            }],
        },
        held_class: None,
        sources: &[Source {
            covers: "electronic levels",
            citation: "its 3 levels below 15,000 cm⁻¹ (Sansonetti 2008, J. Phys. Chem. Ref. Data 37, 7, its 4p levels from Falke et al. 2006 (Phys. Rev. A 74, 032503); NIST's Atomic Spectra Database v5.12, Kramida et al. 2024, as the finding aid, NIST holding the compilations' values; Moore's NBS Circular 467 agrees for Fe I, Ti I and O I to 0.5 cm⁻¹). A cut at 15,000 cm⁻¹ holds `c_p` within 0.8 per cent to 2,000 K, but misses up to 14 per cent at 3,000 K (Ca I), recorded",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: RAYLEIGH_ABSORBER,
    sources: IDENTITY,
};

/// Row 29, iron: its vapour, and its condensate, the iron clouds.
const IRON: Substance = Substance {
    key: SubstanceKey::new_const("Fe"),
    kind: SubstanceKind::Atom,
    elements: &[(Element::Fe, 1)],
    charge: 0,
    molar_mass_g_per_mol: Some(55.845),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::None,
            vibrations: &[],
            levels: &[LevelSet::Listed {
                weight: 1.0,
                levels: &IRON_LEVELS,
            }],
        },
        held_class: None,
        sources: &[Source {
            covers: "electronic levels",
            citation: "its 13 levels below 15,000 cm⁻¹ (Nave et al. 1994, ApJS 94, 221; NIST's Atomic Spectra Database v5.12, Kramida et al. 2024, as the finding aid, NIST holding the compilations' values; Moore's NBS Circular 467 agrees for Fe I, Ti I and O I to 0.5 cm⁻¹). A cut at 15,000 cm⁻¹ holds `c_p` within 0.8 per cent to 2,000 K, but misses up to 14 per cent at 3,000 K (Ca I), recorded",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: RAYLEIGH_ABSORBER_AEROSOL,
    sources: IDENTITY,
};

/// Row 30, magnesium vapour, above the silicate clouds.
const MAGNESIUM: Substance = Substance {
    key: SubstanceKey::new_const("Mg"),
    kind: SubstanceKind::Atom,
    elements: &[(Element::Mg, 1)],
    charge: 0,
    molar_mass_g_per_mol: Some(24.305),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::None,
            vibrations: &[],
            levels: &[],
        },
        held_class: None,
        sources: &[Source {
            covers: "electronic levels",
            citation: "its ground level alone below 15,000 cm⁻¹, so its `c_p` ÷ R is 5/2 (Martin and Zalubas 1980, J. Phys. Chem. Ref. Data 9, 1; NIST's Atomic Spectra Database v5.12, Kramida et al. 2024, as the finding aid, NIST holding the compilations' values; Moore's NBS Circular 467 agrees for Fe I, Ti I and O I to 0.5 cm⁻¹). A cut at 15,000 cm⁻¹ holds `c_p` within 0.8 per cent to 2,000 K, but misses up to 14 per cent at 3,000 K (Ca I), recorded",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: RAYLEIGH_ABSORBER,
    sources: IDENTITY,
};

/// Row 31, silicon vapour, above the silicate clouds.
const SILICON: Substance = Substance {
    key: SubstanceKey::new_const("Si"),
    kind: SubstanceKind::Atom,
    elements: &[(Element::Si, 1)],
    charge: 0,
    molar_mass_g_per_mol: Some(28.085),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::None,
            vibrations: &[],
            levels: &[LevelSet::Listed {
                weight: 1.0,
                levels: &SILICON_LEVELS,
            }],
        },
        held_class: None,
        sources: &[Source {
            covers: "electronic levels",
            citation: "its 4 levels below 15,000 cm⁻¹ (Martin and Zalubas 1983, J. Phys. Chem. Ref. Data 12, 323; NIST's Atomic Spectra Database v5.12, Kramida et al. 2024, as the finding aid, NIST holding the compilations' values; Moore's NBS Circular 467 agrees for Fe I, Ti I and O I to 0.5 cm⁻¹). A cut at 15,000 cm⁻¹ holds `c_p` within 0.8 per cent to 2,000 K, but misses up to 14 per cent at 3,000 K (Ca I), recorded",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: RAYLEIGH_ABSORBER,
    sources: IDENTITY,
};

/// Row 32, calcium vapour, an ultra-hot dayside's.
const CALCIUM: Substance = Substance {
    key: SubstanceKey::new_const("Ca"),
    kind: SubstanceKind::Atom,
    elements: &[(Element::Ca, 1)],
    charge: 0,
    molar_mass_g_per_mol: Some(40.078),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::None,
            vibrations: &[],
            levels: &[],
        },
        held_class: None,
        sources: &[Source {
            covers: "electronic levels",
            citation: "its ground level alone below 15,000 cm⁻¹, so its `c_p` ÷ R is 5/2 (Sugar and Corliss 1985, J. Phys. Chem. Ref. Data 14, Suppl. 2; NIST's Atomic Spectra Database v5.12, Kramida et al. 2024, as the finding aid, NIST holding the compilations' values; Moore's NBS Circular 467 agrees for Fe I, Ti I and O I to 0.5 cm⁻¹). A cut at 15,000 cm⁻¹ holds `c_p` within 0.8 per cent to 2,000 K, but misses up to 14 per cent at 3,000 K (Ca I), recorded",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: RAYLEIGH_ABSORBER,
    sources: IDENTITY,
};

/// Row 33, titanium vapour, an ultra-hot dayside's.
const TITANIUM: Substance = Substance {
    key: SubstanceKey::new_const("Ti"),
    kind: SubstanceKind::Atom,
    elements: &[(Element::Ti, 1)],
    charge: 0,
    molar_mass_g_per_mol: Some(47.867),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::None,
            vibrations: &[],
            levels: &[LevelSet::Listed {
                weight: 1.0,
                levels: &TITANIUM_LEVELS,
            }],
        },
        held_class: None,
        sources: &[Source {
            covers: "electronic levels",
            citation: "its 19 levels below 15,000 cm⁻¹ (Saloman 2012, J. Phys. Chem. Ref. Data 41, 013101; NIST's Atomic Spectra Database v5.12, Kramida et al. 2024, as the finding aid, NIST holding the compilations' values; Moore's NBS Circular 467 agrees for Fe I, Ti I and O I to 0.5 cm⁻¹). A cut at 15,000 cm⁻¹ holds `c_p` within 0.8 per cent to 2,000 K, but misses up to 14 per cent at 3,000 K (Ca I), recorded",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: RAYLEIGH_ABSORBER,
    sources: IDENTITY,
};

/// Row 34, silicon monoxide, the silicate clouds' vapour.
const SILICON_MONOXIDE: Substance = Substance {
    key: SubstanceKey::new_const("SiO"),
    kind: SubstanceKind::Molecule,
    elements: &[(Element::Si, 1), (Element::O, 1)],
    charge: 0,
    molar_mass_g_per_mol: Some(44.084),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::Linear,
            vibrations: &[vibration(1229.625, 1)],
            levels: &[],
        },
        held_class: None,
        sources: &[Source {
            covers: "rotation, vibrations, electronic levels",
            citation: "Huber and Herzberg 1979 (the NIST WebBook's transcription as the finding aid): ω_e 1,241.557 and ω_e x_e 5.966 cm⁻¹, so the fundamental is 1,229.625 cm⁻¹; its X¹Σ⁺ ground alone lies below 15,000 cm⁻¹",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: RAYLEIGH_ABSORBER,
    sources: IDENTITY,
};

/// Row 35, titanium monoxide, the hot giants' optical absorber.
const TITANIUM_MONOXIDE: Substance = Substance {
    key: SubstanceKey::new_const("TiO"),
    kind: SubstanceKind::Molecule,
    elements: &[(Element::Ti, 1), (Element::O, 1)],
    charge: 0,
    molar_mass_g_per_mol: Some(63.866),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::Linear,
            vibrations: &[vibration(1000.024, 1)],
            levels: &[LevelSet::Listed {
                weight: 1.0,
                levels: &TITANIUM_MONOXIDE_STATES,
            }],
        },
        held_class: None,
        sources: &[Source {
            covers: "rotation, vibrations, electronic levels",
            citation: "Huber and Herzberg 1979 (the NIST WebBook's transcription as the finding aid): ω_e 1,009.02 and ω_e x_e 4.498 cm⁻¹, so the fundamental is 1,000.024 cm⁻¹; the X³Δ_r ground's Ω = 1, 2 and 3 at 0, 96.4 and 197.5 cm⁻¹ (Phillips 1971, Huber and Herzberg's note 18), each doubly degenerate, and the a¹Δ (3,440), d¹Σ⁺ (5,655.6), E³Π (12,025, its components unresolved, g = 6), A³Φ_r (Ω = 2, 3, 4 at 14,089.91, 14,262.8 and 14,431.0) and b¹Π (14,762.0 cm⁻¹) states below 15,000 cm⁻¹ at Huber and Herzberg's T_e (the a¹Δ at 3,440 ± 10, their note 17, and d and b above it), McKemmish et al. 2019's term table (MNRAS 488, 2836, arXiv:1905.04587, Tables 1 and 2) as the check",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: RAYLEIGH_ABSORBER,
    sources: IDENTITY,
};

/// Row 36, vanadium monoxide, the hot giants' optical absorber.
const VANADIUM_MONOXIDE: Substance = Substance {
    key: SubstanceKey::new_const("VO"),
    kind: SubstanceKind::Molecule,
    elements: &[(Element::V, 1), (Element::O, 1)],
    charge: 0,
    molar_mass_g_per_mol: Some(66.941),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::Linear,
            vibrations: &[vibration(1001.58, 1)],
            levels: &[LevelSet::Listed {
                weight: 1.0,
                levels: &VANADIUM_MONOXIDE_STATES,
            }],
        },
        held_class: None,
        sources: &[Source {
            covers: "rotation, vibrations, electronic levels",
            citation: "Huber and Herzberg 1979 (the NIST WebBook's transcription as the finding aid): ω_e 1,011.3 (uncertain) and ω_e x_e 4.86 cm⁻¹, so the fundamental is 1,001.58 cm⁻¹; the X⁴Σ⁻ ground, its spin components within about 3 cm⁻¹, as one level of g = 4, and the B⁴Π_r components at 12,567, 12,625, 12,689 and 12,760.8 cm⁻¹, the only states Huber and Herzberg tabulate below 15,000 cm⁻¹. McKemmish, Yurchenko and Tennyson 2016's A′⁴Φ (7,293) and A⁴Π (9,562 cm⁻¹) and their doublets are left out, as P14.T49.b's rule takes Huber and Herzberg's states; with them the sum would be 1.0% below NASA Glenn at 1,000 K, against 1.35% without",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: RAYLEIGH_ABSORBER,
    sources: IDENTITY,
};

/// Row 37, iron hydride, the cool dwarfs' absorber.
const IRON_HYDRIDE: Substance = Substance {
    key: SubstanceKey::new_const("FeH"),
    kind: SubstanceKind::Molecule,
    elements: &[(Element::Fe, 1), (Element::H, 1)],
    charge: 0,
    molar_mass_g_per_mol: Some(56.853),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::Linear,
            vibrations: &[vibration(1762.08, 1)],
            levels: &[LevelSet::Listed {
                weight: 1.0,
                levels: &IRON_HYDRIDE_STATES,
            }],
        },
        held_class: None,
        sources: &[Source {
            covers: "rotation, vibrations, electronic levels",
            citation: "Dulick et al. 2003, ApJ 594, 651 (arXiv:astro-ph/0305162, Tables 1, 7 and 9a), Huber and Herzberg 1979 tabulating no ground state: ω_e 1,831.8 and ω_e x_e 34.86 cm⁻¹, so the fundamental is 1,762.08 cm⁻¹; and their Table 9a's 29 term values for thermochemistry below 15,000 cm⁻¹ above the X⁴Δ_i Ω = 7/2 ground: the X⁴Δ_i, A⁴Π, a⁶Δ, C⁴Φ and b⁶Π components at g = 2 each, and the c⁶Σ⁺, B⁴Σ⁻, D⁴Σ⁺, E⁴Π and F⁴Δ_i states, whose components the table does not list, at 2S + 1 for a Σ state and 2(2S + 1) otherwise (the degeneracies assumed, the table printing none; F⁴Δ_i at its Ω = 7/2 origin, so its other components lie up to about 750 cm⁻¹ too low). Each state takes the ground's vibration and rotation, the ruling's product form",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: RAYLEIGH_ABSORBER,
    sources: IDENTITY,
};

/// Row 38, aluminium vapour, one of T50.b's metals.
const ALUMINIUM: Substance = Substance {
    key: SubstanceKey::new_const("Al"),
    kind: SubstanceKind::Atom,
    elements: &[(Element::Al, 1)],
    charge: 0,
    molar_mass_g_per_mol: Some(26.982),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::None,
            vibrations: &[],
            levels: &[LevelSet::Listed {
                weight: 1.0,
                levels: &ALUMINIUM_LEVELS,
            }],
        },
        held_class: None,
        sources: &[Source {
            covers: "electronic levels",
            citation: "its 2 levels below 15,000 cm⁻¹ (Martin and Zalubas 1979, J. Phys. Chem. Ref. Data 8, 817; NIST's Atomic Spectra Database v5.12, Kramida et al. 2024, as the finding aid, NIST holding the compilations' values; Moore's NBS Circular 467 agrees for Fe I, Ti I and O I to 0.5 cm⁻¹). A cut at 15,000 cm⁻¹ holds `c_p` within 0.8 per cent to 2,000 K, but misses up to 14 per cent at 3,000 K (Ca I), recorded",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: RAYLEIGH_AWAITING_ABSORPTION,
    sources: IDENTITY,
};

/// Row 39, vanadium vapour, one of T50.b's metals.
const VANADIUM: Substance = Substance {
    key: SubstanceKey::new_const("V"),
    kind: SubstanceKind::Atom,
    elements: &[(Element::V, 1)],
    charge: 0,
    molar_mass_g_per_mol: Some(50.942),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::None,
            vibrations: &[],
            levels: &[LevelSet::Listed {
                weight: 1.0,
                levels: &VANADIUM_LEVELS,
            }],
        },
        held_class: None,
        sources: &[Source {
            covers: "electronic levels",
            citation: "its 24 levels below 15,000 cm⁻¹ (Saloman and Kramida 2017, ApJS 231, 18; NIST's Atomic Spectra Database v5.12, Kramida et al. 2024, as the finding aid, NIST holding the compilations' values; Moore's NBS Circular 467 agrees for Fe I, Ti I and O I to 0.5 cm⁻¹). A cut at 15,000 cm⁻¹ holds `c_p` within 0.8 per cent to 2,000 K, but misses up to 14 per cent at 3,000 K (Ca I), recorded",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: RAYLEIGH_AWAITING_ABSORPTION,
    sources: IDENTITY,
};

/// Row 40, manganese vapour, one of T50.b's metals, `MnS`'s.
const MANGANESE: Substance = Substance {
    key: SubstanceKey::new_const("Mn"),
    kind: SubstanceKind::Atom,
    elements: &[(Element::Mn, 1)],
    charge: 0,
    molar_mass_g_per_mol: Some(54.938),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::None,
            vibrations: &[],
            levels: &[],
        },
        held_class: None,
        sources: &[Source {
            covers: "electronic levels",
            citation: "its ground level alone below 15,000 cm⁻¹, so its `c_p` ÷ R is 5/2 (Sugar and Corliss 1985, J. Phys. Chem. Ref. Data 14, Suppl. 2; NIST's Atomic Spectra Database v5.12, Kramida et al. 2024, as the finding aid, NIST holding the compilations' values; Moore's NBS Circular 467 agrees for Fe I, Ti I and O I to 0.5 cm⁻¹). A cut at 15,000 cm⁻¹ holds `c_p` within 0.8 per cent to 2,000 K, but misses up to 14 per cent at 3,000 K (Ca I), recorded",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: RAYLEIGH_AWAITING_ABSORPTION,
    sources: IDENTITY,
};

/// Row 41, nickel vapour, one of T50.b's metals.
const NICKEL: Substance = Substance {
    key: SubstanceKey::new_const("Ni"),
    kind: SubstanceKind::Atom,
    elements: &[(Element::Ni, 1)],
    charge: 0,
    molar_mass_g_per_mol: Some(58.693),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::None,
            vibrations: &[],
            levels: &[LevelSet::Listed {
                weight: 1.0,
                levels: &NICKEL_LEVELS,
            }],
        },
        held_class: None,
        sources: &[Source {
            covers: "electronic levels",
            citation: "its 9 levels below 15,000 cm⁻¹ (Litzén, Brault and Thorne 1993, Phys. Scr. 47, 628; NIST's Atomic Spectra Database v5.12, Kramida et al. 2024, as the finding aid, NIST holding the compilations' values; Moore's NBS Circular 467 agrees for Fe I, Ti I and O I to 0.5 cm⁻¹). A cut at 15,000 cm⁻¹ holds `c_p` within 0.8 per cent to 2,000 K, but misses up to 14 per cent at 3,000 K (Ca I), recorded",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: RAYLEIGH_AWAITING_ABSORPTION,
    sources: IDENTITY,
};

/// Row 42, zinc vapour, one of T50.b's metals, `ZnS`'s.
const ZINC: Substance = Substance {
    key: SubstanceKey::new_const("Zn"),
    kind: SubstanceKind::Atom,
    elements: &[(Element::Zn, 1)],
    charge: 0,
    molar_mass_g_per_mol: Some(65.38),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::None,
            vibrations: &[],
            levels: &[],
        },
        held_class: None,
        sources: &[Source {
            covers: "electronic levels",
            citation: "its ground level alone below 15,000 cm⁻¹, so its `c_p` ÷ R is 5/2 (Sugar and Musgrove 1995, J. Phys. Chem. Ref. Data 24, 1803; NIST's Atomic Spectra Database v5.12, Kramida et al. 2024, as the finding aid, NIST holding the compilations' values; Moore's NBS Circular 467 agrees for Fe I, Ti I and O I to 0.5 cm⁻¹). A cut at 15,000 cm⁻¹ holds `c_p` within 0.8 per cent to 2,000 K, but misses up to 14 per cent at 3,000 K (Ca I), recorded",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: RAYLEIGH_AWAITING_ABSORPTION,
    sources: IDENTITY,
};

/// Row 43, the first ion of Na, for T52.a's charge balance.
const SODIUM_ION: Substance = Substance {
    key: SubstanceKey::new_const("Na+"),
    kind: SubstanceKind::Ion,
    elements: &[(Element::Na, 1)],
    charge: 1,
    molar_mass_g_per_mol: Some(22.989_451_420_091_12),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::None,
            vibrations: &[],
            levels: &[],
        },
        held_class: None,
        sources: &[Source {
            covers: "electronic levels",
            citation: "its ground level alone below 15,000 cm⁻¹, so its `c_p` ÷ R is 5/2 (Sansonetti 2008, J. Phys. Chem. Ref. Data 37, 1659; NIST's Atomic Spectra Database v5.12, Kramida et al. 2024, as the finding aid, NIST holding the compilations' values; Moore's NBS Circular 467 agrees for Fe I, Ti I and O I to 0.5 cm⁻¹). A cut at 15,000 cm⁻¹ holds `c_p` within 0.8 per cent to 2,000 K, but misses up to 14 per cent at 3,000 K (Ca I), recorded",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: ION_AWAITING_ABSORPTION,
    sources: IDENTITY,
};

/// Row 44, the first ion of Mg, for T52.a's charge balance.
const MAGNESIUM_ION: Substance = Substance {
    key: SubstanceKey::new_const("Mg+"),
    kind: SubstanceKind::Ion,
    elements: &[(Element::Mg, 1)],
    charge: 1,
    molar_mass_g_per_mol: Some(24.304_451_420_091_12),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::None,
            vibrations: &[],
            levels: &[],
        },
        held_class: None,
        sources: &[Source {
            covers: "electronic levels",
            citation: "its ground level alone below 15,000 cm⁻¹, so its `c_p` ÷ R is 5/2 (Martin and Zalubas 1980, J. Phys. Chem. Ref. Data 9, 1; NIST's Atomic Spectra Database v5.12, Kramida et al. 2024, as the finding aid, NIST holding the compilations' values; Moore's NBS Circular 467 agrees for Fe I, Ti I and O I to 0.5 cm⁻¹). A cut at 15,000 cm⁻¹ holds `c_p` within 0.8 per cent to 2,000 K, but misses up to 14 per cent at 3,000 K (Ca I), recorded",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: ABSORBER_ONLY,
    sources: IDENTITY,
};

/// Row 45, the first ion of Al, for T52.a's charge balance.
const ALUMINIUM_ION: Substance = Substance {
    key: SubstanceKey::new_const("Al+"),
    kind: SubstanceKind::Ion,
    elements: &[(Element::Al, 1)],
    charge: 1,
    molar_mass_g_per_mol: Some(26.981_451_420_091_12),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::None,
            vibrations: &[],
            levels: &[],
        },
        held_class: None,
        sources: &[Source {
            covers: "electronic levels",
            citation: "its ground level alone below 15,000 cm⁻¹, so its `c_p` ÷ R is 5/2 (Martin and Zalubas 1979, J. Phys. Chem. Ref. Data 8, 817; NIST's Atomic Spectra Database v5.12, Kramida et al. 2024, as the finding aid, NIST holding the compilations' values; Moore's NBS Circular 467 agrees for Fe I, Ti I and O I to 0.5 cm⁻¹). A cut at 15,000 cm⁻¹ holds `c_p` within 0.8 per cent to 2,000 K, but misses up to 14 per cent at 3,000 K (Ca I), recorded",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: ION_AWAITING_ABSORPTION,
    sources: IDENTITY,
};

/// Row 46, the first ion of Si, for T52.a's charge balance.
const SILICON_ION: Substance = Substance {
    key: SubstanceKey::new_const("Si+"),
    kind: SubstanceKind::Ion,
    elements: &[(Element::Si, 1)],
    charge: 1,
    molar_mass_g_per_mol: Some(28.084_451_420_091_12),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::None,
            vibrations: &[],
            levels: &[LevelSet::Listed {
                weight: 1.0,
                levels: &SILICON_ION_LEVELS,
            }],
        },
        held_class: None,
        sources: &[Source {
            covers: "electronic levels",
            citation: "its 2 levels below 15,000 cm⁻¹ (Martin and Zalubas 1983, J. Phys. Chem. Ref. Data 12, 323; NIST's Atomic Spectra Database v5.12, Kramida et al. 2024, as the finding aid, NIST holding the compilations' values; Moore's NBS Circular 467 agrees for Fe I, Ti I and O I to 0.5 cm⁻¹). A cut at 15,000 cm⁻¹ holds `c_p` within 0.8 per cent to 2,000 K, but misses up to 14 per cent at 3,000 K (Ca I), recorded",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: ION_AWAITING_ABSORPTION,
    sources: IDENTITY,
};

/// Row 47, the first ion of K, for T52.a's charge balance.
const POTASSIUM_ION: Substance = Substance {
    key: SubstanceKey::new_const("K+"),
    kind: SubstanceKind::Ion,
    elements: &[(Element::K, 1)],
    charge: 1,
    molar_mass_g_per_mol: Some(39.097_451_420_091_12),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::None,
            vibrations: &[],
            levels: &[],
        },
        held_class: None,
        sources: &[Source {
            covers: "electronic levels",
            citation: "its ground level alone below 15,000 cm⁻¹, so its `c_p` ÷ R is 5/2 (Sansonetti 2008, J. Phys. Chem. Ref. Data 37, 7; NIST's Atomic Spectra Database v5.12, Kramida et al. 2024, as the finding aid, NIST holding the compilations' values; Moore's NBS Circular 467 agrees for Fe I, Ti I and O I to 0.5 cm⁻¹). A cut at 15,000 cm⁻¹ holds `c_p` within 0.8 per cent to 2,000 K, but misses up to 14 per cent at 3,000 K (Ca I), recorded",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: ION_AWAITING_ABSORPTION,
    sources: IDENTITY,
};

/// Row 48, the first ion of Ca, for T52.a's charge balance.
const CALCIUM_ION: Substance = Substance {
    key: SubstanceKey::new_const("Ca+"),
    kind: SubstanceKind::Ion,
    elements: &[(Element::Ca, 1)],
    charge: 1,
    molar_mass_g_per_mol: Some(40.077_451_420_091_12),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::None,
            vibrations: &[],
            levels: &[LevelSet::Listed {
                weight: 1.0,
                levels: &CALCIUM_ION_LEVELS,
            }],
        },
        held_class: None,
        sources: &[Source {
            covers: "electronic levels",
            citation: "its 3 levels below 15,000 cm⁻¹ (Sugar and Corliss 1985, J. Phys. Chem. Ref. Data 14, Suppl. 2; NIST's Atomic Spectra Database v5.12, Kramida et al. 2024, as the finding aid, NIST holding the compilations' values; Moore's NBS Circular 467 agrees for Fe I, Ti I and O I to 0.5 cm⁻¹). A cut at 15,000 cm⁻¹ holds `c_p` within 0.8 per cent to 2,000 K, but misses up to 14 per cent at 3,000 K (Ca I), recorded",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: ABSORBER_ONLY,
    sources: IDENTITY,
};

/// Row 49, the first ion of Ti, for T52.a's charge balance.
const TITANIUM_ION: Substance = Substance {
    key: SubstanceKey::new_const("Ti+"),
    kind: SubstanceKind::Ion,
    elements: &[(Element::Ti, 1)],
    charge: 1,
    molar_mass_g_per_mol: Some(47.866_451_420_091_12),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::None,
            vibrations: &[],
            levels: &[LevelSet::Listed {
                weight: 1.0,
                levels: &TITANIUM_ION_LEVELS,
            }],
        },
        held_class: None,
        sources: &[Source {
            covers: "electronic levels",
            citation: "its 26 levels below 15,000 cm⁻¹ (Saloman 2012, J. Phys. Chem. Ref. Data 41, 013101; NIST's Atomic Spectra Database v5.12, Kramida et al. 2024, as the finding aid, NIST holding the compilations' values; Moore's NBS Circular 467 agrees for Fe I, Ti I and O I to 0.5 cm⁻¹). A cut at 15,000 cm⁻¹ holds `c_p` within 0.8 per cent to 2,000 K, but misses up to 14 per cent at 3,000 K (Ca I), recorded",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: ABSORBER_ONLY,
    sources: IDENTITY,
};

/// Row 50, the first ion of V, for T52.a's charge balance.
const VANADIUM_ION: Substance = Substance {
    key: SubstanceKey::new_const("V+"),
    kind: SubstanceKind::Ion,
    elements: &[(Element::V, 1)],
    charge: 1,
    molar_mass_g_per_mol: Some(50.941_451_420_091_12),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::None,
            vibrations: &[],
            levels: &[LevelSet::Listed {
                weight: 1.0,
                levels: &VANADIUM_ION_LEVELS,
            }],
        },
        held_class: None,
        sources: &[Source {
            covers: "electronic levels",
            citation: "its 28 levels below 15,000 cm⁻¹ (Saloman and Kramida 2017, ApJS 231, 19; NIST's Atomic Spectra Database v5.12, Kramida et al. 2024, as the finding aid, NIST holding the compilations' values; Moore's NBS Circular 467 agrees for Fe I, Ti I and O I to 0.5 cm⁻¹). A cut at 15,000 cm⁻¹ holds `c_p` within 0.8 per cent to 2,000 K, but misses up to 14 per cent at 3,000 K (Ca I), recorded",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: ION_AWAITING_ABSORPTION,
    sources: IDENTITY,
};

/// Row 51, the first ion of Cr, for T52.a's charge balance.
const CHROMIUM_ION: Substance = Substance {
    key: SubstanceKey::new_const("Cr+"),
    kind: SubstanceKind::Ion,
    elements: &[(Element::Cr, 1)],
    charge: 1,
    molar_mass_g_per_mol: Some(51.995_451_420_091_12),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::None,
            vibrations: &[],
            levels: &[LevelSet::Listed {
                weight: 1.0,
                levels: &CHROMIUM_ION_LEVELS,
            }],
        },
        held_class: None,
        sources: &[Source {
            covers: "electronic levels",
            citation: "its 6 levels below 15,000 cm⁻¹ (Sansonetti and Nave 2014, ApJS 213, 28; NIST's Atomic Spectra Database v5.12, Kramida et al. 2024, as the finding aid, NIST holding the compilations' values; Moore's NBS Circular 467 agrees for Fe I, Ti I and O I to 0.5 cm⁻¹). A cut at 15,000 cm⁻¹ holds `c_p` within 0.8 per cent to 2,000 K, but misses up to 14 per cent at 3,000 K (Ca I), recorded",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: ION_AWAITING_ABSORPTION,
    sources: IDENTITY,
};

/// Row 52, the first ion of Mn, for T52.a's charge balance.
const MANGANESE_ION: Substance = Substance {
    key: SubstanceKey::new_const("Mn+"),
    kind: SubstanceKind::Ion,
    elements: &[(Element::Mn, 1)],
    charge: 1,
    molar_mass_g_per_mol: Some(54.937_451_420_091_12),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::None,
            vibrations: &[],
            levels: &[LevelSet::Listed {
                weight: 1.0,
                levels: &MANGANESE_ION_LEVELS,
            }],
        },
        held_class: None,
        sources: &[Source {
            covers: "electronic levels",
            citation: "its 7 levels below 15,000 cm⁻¹ (Kramida and Sansonetti 2013, ApJS 205, 14; NIST's Atomic Spectra Database v5.12, Kramida et al. 2024, as the finding aid, NIST holding the compilations' values; Moore's NBS Circular 467 agrees for Fe I, Ti I and O I to 0.5 cm⁻¹). A cut at 15,000 cm⁻¹ holds `c_p` within 0.8 per cent to 2,000 K, but misses up to 14 per cent at 3,000 K (Ca I), recorded",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: ION_AWAITING_ABSORPTION,
    sources: IDENTITY,
};

/// Row 53, the first ion of Fe, for T52.a's charge balance.
const IRON_ION: Substance = Substance {
    key: SubstanceKey::new_const("Fe+"),
    kind: SubstanceKind::Ion,
    elements: &[(Element::Fe, 1)],
    charge: 1,
    molar_mass_g_per_mol: Some(55.844_451_420_091_12),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::None,
            vibrations: &[],
            levels: &[LevelSet::Listed {
                weight: 1.0,
                levels: &IRON_ION_LEVELS,
            }],
        },
        held_class: None,
        sources: &[Source {
            covers: "electronic levels",
            citation: "its 16 levels below 15,000 cm⁻¹ (Nave and Johansson 2013, ApJS 204, 1; NIST's Atomic Spectra Database v5.12, Kramida et al. 2024, as the finding aid, NIST holding the compilations' values; Moore's NBS Circular 467 agrees for Fe I, Ti I and O I to 0.5 cm⁻¹). A cut at 15,000 cm⁻¹ holds `c_p` within 0.8 per cent to 2,000 K, but misses up to 14 per cent at 3,000 K (Ca I), recorded",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: ABSORBER_ONLY,
    sources: IDENTITY,
};

/// Row 54, the first ion of Ni, for T52.a's charge balance.
const NICKEL_ION: Substance = Substance {
    key: SubstanceKey::new_const("Ni+"),
    kind: SubstanceKind::Ion,
    elements: &[(Element::Ni, 1)],
    charge: 1,
    molar_mass_g_per_mol: Some(58.692_451_420_091_12),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::None,
            vibrations: &[],
            levels: &[LevelSet::Listed {
                weight: 1.0,
                levels: &NICKEL_ION_LEVELS,
            }],
        },
        held_class: None,
        sources: &[Source {
            covers: "electronic levels",
            citation: "its 8 levels below 15,000 cm⁻¹ (Sugar and Corliss 1985, J. Phys. Chem. Ref. Data 14, Suppl. 2; NIST's Atomic Spectra Database v5.12, Kramida et al. 2024, as the finding aid, NIST holding the compilations' values; Moore's NBS Circular 467 agrees for Fe I, Ti I and O I to 0.5 cm⁻¹). A cut at 15,000 cm⁻¹ holds `c_p` within 0.8 per cent to 2,000 K, but misses up to 14 per cent at 3,000 K (Ca I), recorded",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: ION_AWAITING_ABSORPTION,
    sources: IDENTITY,
};

/// Row 55, the first ion of Zn, for T52.a's charge balance.
const ZINC_ION: Substance = Substance {
    key: SubstanceKey::new_const("Zn+"),
    kind: SubstanceKind::Ion,
    elements: &[(Element::Zn, 1)],
    charge: 1,
    molar_mass_g_per_mol: Some(65.379_451_420_091_12),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::None,
            vibrations: &[],
            levels: &[],
        },
        held_class: None,
        sources: &[Source {
            covers: "electronic levels",
            citation: "its ground level alone below 15,000 cm⁻¹, so its `c_p` ÷ R is 5/2 (Sugar and Musgrove 1995, J. Phys. Chem. Ref. Data 24, 1803; NIST's Atomic Spectra Database v5.12, Kramida et al. 2024, as the finding aid, NIST holding the compilations' values; Moore's NBS Circular 467 agrees for Fe I, Ti I and O I to 0.5 cm⁻¹). A cut at 15,000 cm⁻¹ holds `c_p` within 0.8 per cent to 2,000 K, but misses up to 14 per cent at 3,000 K (Ca I), recorded",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: ION_AWAITING_ABSORPTION,
    sources: IDENTITY,
};

/// Row 56, hydrogen chloride, chlorine's gas in T52.a's hydrogen envelopes.
const HYDROGEN_CHLORIDE: Substance = Substance {
    key: SubstanceKey::new_const("HCl"),
    kind: SubstanceKind::Molecule,
    elements: &[(Element::H, 1), (Element::Cl, 1)],
    charge: 0,
    molar_mass_g_per_mol: Some(36.458),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::Linear,
            vibrations: &[vibration(2885.309, 1)],
            levels: &[],
        },
        held_class: None,
        sources: &[Source {
            covers: "rotation, vibrations, electronic levels",
            citation: "Huber and Herzberg 1979 (the NIST WebBook's transcription as the finding aid), H³⁵Cl: ω_e 2,990.946 3 and ω_e x_e 52.818 6 cm⁻¹, so the fundamental is 2,885.309 cm⁻¹; its X¹Σ⁺ ground alone lies below 15,000 cm⁻¹",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: RAYLEIGH_AWAITING_ABSORPTION,
    sources: IDENTITY,
};

/// Row 57, ammonium hydrosulphide, the giants' middle deck.
const AMMONIUM_HYDROSULPHIDE: Substance = Substance {
    key: SubstanceKey::new_const("NH4SH"),
    kind: SubstanceKind::Mineral,
    elements: &[(Element::N, 1), (Element::H, 5), (Element::S, 1)],
    charge: 0,
    molar_mass_g_per_mol: Some(51.107),
    gas: None,
    phase: None,
    diagram: None,
    optics: AEROSOL_ONLY,
    sources: IDENTITY,
};

/// Row 58, sulphuric acid, Venus's clouds (T24.g).
const SULPHURIC_ACID: Substance = Substance {
    key: SubstanceKey::new_const("H2SO4"),
    kind: SubstanceKind::Molecule,
    elements: &[(Element::H, 2), (Element::S, 1), (Element::O, 4)],
    charge: 0,
    molar_mass_g_per_mol: Some(98.072),
    gas: None,
    phase: None,
    diagram: None,
    optics: AEROSOL_ONLY,
    sources: IDENTITY,
};

/// Row 59, elemental sulphur as S8 rings: a warm giant's haze, Io's flows.
const SULPHUR: Substance = Substance {
    key: SubstanceKey::new_const("S8"),
    kind: SubstanceKind::Molecule,
    elements: &[(Element::S, 8)],
    charge: 0,
    molar_mass_g_per_mol: Some(256.48),
    gas: None,
    phase: None,
    diagram: None,
    optics: AEROSOL_ONLY,
    sources: IDENTITY,
};

/// Row 60, potassium chloride (sylvite), a cool giant's salt cloud.
const POTASSIUM_CHLORIDE: Substance = Substance {
    key: SubstanceKey::new_const("KCl"),
    kind: SubstanceKind::Mineral,
    elements: &[(Element::K, 1), (Element::Cl, 1)],
    charge: 0,
    molar_mass_g_per_mol: Some(74.548),
    gas: None,
    phase: None,
    diagram: None,
    optics: AEROSOL_ONLY,
    sources: IDENTITY,
};

/// Row 61, sodium chloride (halite), a salt cloud and an evaporite.
const SODIUM_CHLORIDE: Substance = Substance {
    key: SubstanceKey::new_const("NaCl"),
    kind: SubstanceKind::Mineral,
    elements: &[(Element::Na, 1), (Element::Cl, 1)],
    charge: 0,
    molar_mass_g_per_mol: Some(58.44),
    gas: None,
    phase: None,
    diagram: None,
    optics: AEROSOL_ONLY,
    sources: IDENTITY,
};

/// Row 62, zinc sulphide (sphalerite), a sulphide cloud.
const ZINC_SULPHIDE: Substance = Substance {
    key: SubstanceKey::new_const("ZnS"),
    kind: SubstanceKind::Mineral,
    elements: &[(Element::Zn, 1), (Element::S, 1)],
    charge: 0,
    molar_mass_g_per_mol: Some(97.44),
    gas: None,
    phase: None,
    diagram: None,
    optics: AEROSOL_ONLY,
    sources: IDENTITY,
};

/// Row 63, sodium sulphide, a T-dwarf's sulphide cloud.
const SODIUM_SULPHIDE: Substance = Substance {
    key: SubstanceKey::new_const("Na2S"),
    kind: SubstanceKind::Mineral,
    elements: &[(Element::Na, 2), (Element::S, 1)],
    charge: 0,
    molar_mass_g_per_mol: Some(78.04),
    gas: None,
    phase: None,
    diagram: None,
    optics: AEROSOL_ONLY,
    sources: IDENTITY,
};

/// Row 64, manganese sulphide (alabandite), a sulphide cloud.
const MANGANESE_SULPHIDE: Substance = Substance {
    key: SubstanceKey::new_const("MnS"),
    kind: SubstanceKind::Mineral,
    elements: &[(Element::Mn, 1), (Element::S, 1)],
    charge: 0,
    molar_mass_g_per_mol: Some(86.998),
    gas: None,
    phase: None,
    diagram: None,
    optics: AEROSOL_ONLY,
    sources: IDENTITY,
};

/// Row 65, chromium: its metal cloud, and its vapour.
const CHROMIUM: Substance = Substance {
    key: SubstanceKey::new_const("Cr"),
    kind: SubstanceKind::Atom,
    elements: &[(Element::Cr, 1)],
    charge: 0,
    molar_mass_g_per_mol: Some(51.996),
    gas: Some(GasColumns {
        molecular: MolecularConstants {
            rotation: Rotation::None,
            vibrations: &[],
            levels: &[LevelSet::Listed {
                weight: 1.0,
                levels: &CHROMIUM_LEVELS,
            }],
        },
        held_class: None,
        sources: &[Source {
            covers: "electronic levels",
            citation: "its 7 levels below 15,000 cm⁻¹ (Saloman 2012, J. Phys. Chem. Ref. Data 41, 043103; NIST's Atomic Spectra Database v5.12, Kramida et al. 2024, as the finding aid, NIST holding the compilations' values; Moore's NBS Circular 467 agrees for Fe I, Ti I and O I to 0.5 cm⁻¹). A cut at 15,000 cm⁻¹ holds `c_p` within 0.8 per cent to 2,000 K, but misses up to 14 per cent at 3,000 K (Ca I), recorded",
            basis: Basis::Fact,
        }],
    }),
    phase: None,
    diagram: None,
    optics: RAYLEIGH_ABSORBER_AEROSOL,
    sources: IDENTITY,
};

/// Row 66, iron sulphide (troilite).
const IRON_SULPHIDE: Substance = Substance {
    key: SubstanceKey::new_const("FeS"),
    kind: SubstanceKind::Mineral,
    elements: &[(Element::Fe, 1), (Element::S, 1)],
    charge: 0,
    molar_mass_g_per_mol: Some(87.905),
    gas: None,
    phase: None,
    diagram: None,
    optics: AEROSOL_ONLY,
    sources: IDENTITY,
};

/// Row 67, enstatite, a silicate cloud.
const ENSTATITE: Substance = Substance {
    key: SubstanceKey::new_const("MgSiO3"),
    kind: SubstanceKind::Mineral,
    elements: &[(Element::Mg, 1), (Element::Si, 1), (Element::O, 3)],
    charge: 0,
    molar_mass_g_per_mol: Some(100.387),
    gas: None,
    phase: None,
    diagram: None,
    optics: AEROSOL_ONLY,
    sources: IDENTITY,
};

/// Row 68, forsterite, a silicate cloud.
const FORSTERITE: Substance = Substance {
    key: SubstanceKey::new_const("Mg2SiO4"),
    kind: SubstanceKind::Mineral,
    elements: &[(Element::Mg, 2), (Element::Si, 1), (Element::O, 4)],
    charge: 0,
    molar_mass_g_per_mol: Some(140.691),
    gas: None,
    phase: None,
    diagram: None,
    optics: AEROSOL_ONLY,
    sources: IDENTITY,
};

/// Row 69, silica, a silicate cloud.
const SILICA: Substance = Substance {
    key: SubstanceKey::new_const("SiO2"),
    kind: SubstanceKind::Mineral,
    elements: &[(Element::Si, 1), (Element::O, 2)],
    charge: 0,
    molar_mass_g_per_mol: Some(60.083),
    gas: None,
    phase: None,
    diagram: None,
    optics: AEROSOL_ONLY,
    sources: IDENTITY,
};

/// Row 70, corundum, the first refractory cloud.
const CORUNDUM: Substance = Substance {
    key: SubstanceKey::new_const("Al2O3"),
    kind: SubstanceKind::Mineral,
    elements: &[(Element::Al, 2), (Element::O, 3)],
    charge: 0,
    molar_mass_g_per_mol: Some(101.961),
    gas: None,
    phase: None,
    diagram: None,
    optics: AEROSOL_ONLY,
    sources: IDENTITY,
};

/// Row 71, perovskite, a refractory cloud that cold-traps `TiO`.
const PEROVSKITE: Substance = Substance {
    key: SubstanceKey::new_const("CaTiO3"),
    kind: SubstanceKind::Mineral,
    elements: &[(Element::Ca, 1), (Element::Ti, 1), (Element::O, 3)],
    charge: 0,
    molar_mass_g_per_mol: Some(135.942),
    gas: None,
    phase: None,
    diagram: None,
    optics: AEROSOL_ONLY,
    sources: IDENTITY,
};

/// Row 72, rutile, a refractory cloud.
const RUTILE: Substance = Substance {
    key: SubstanceKey::new_const("TiO2"),
    kind: SubstanceKind::Mineral,
    elements: &[(Element::Ti, 1), (Element::O, 2)],
    charge: 0,
    molar_mass_g_per_mol: Some(79.865),
    gas: None,
    phase: None,
    diagram: None,
    optics: AEROSOL_ONLY,
    sources: IDENTITY,
};

/// Row 73, graphite, carbon's polymorph: a carbon-rich world's condensate and Mercury's darkening.
const GRAPHITE: Substance = Substance {
    key: SubstanceKey::new_const("graphite"),
    kind: SubstanceKind::Mineral,
    elements: &[(Element::C, 1)],
    charge: 0,
    molar_mass_g_per_mol: Some(12.011),
    gas: None,
    phase: None,
    diagram: None,
    optics: AEROSOL_ONLY,
    sources: IDENTITY,
};

/// Row 74, tholin, the photochemical organic haze of Titan and the outer bodies.
const THOLIN: Substance = Substance {
    key: SubstanceKey::new_const("tholin"),
    kind: SubstanceKind::Organic,
    elements: &[],
    charge: 0,
    molar_mass_g_per_mol: None,
    gas: None,
    phase: None,
    diagram: None,
    optics: AEROSOL_ONLY,
    sources: &[THOLIN_IDENTITY],
};

/// Row 75, soot, a hydrocarbon haze.
const SOOT: Substance = Substance {
    key: SubstanceKey::new_const("soot"),
    kind: SubstanceKind::Organic,
    elements: &[],
    charge: 0,
    molar_mass_g_per_mol: None,
    gas: None,
    phase: None,
    diagram: None,
    optics: AEROSOL_ONLY,
    sources: &[SOOT_IDENTITY],
};

/// Row 76, the cold giants' red chromophore (Braude et al. 2020).
const CHROMOPHORE: Substance = Substance {
    key: SubstanceKey::new_const("chromophore"),
    kind: SubstanceKind::Organic,
    elements: &[],
    charge: 0,
    molar_mass_g_per_mol: None,
    gas: None,
    phase: None,
    diagram: None,
    optics: AEROSOL_ONLY,
    sources: &[CHROMOPHORE_IDENTITY],
};

/// Row 77, the ice giants' haze above their CH4 layer (Irwin et al. 2022).
const ICE_GIANT_HAZE: Substance = Substance {
    key: SubstanceKey::new_const("ice_giant_haze"),
    kind: SubstanceKind::Organic,
    elements: &[],
    charge: 0,
    molar_mass_g_per_mol: None,
    gas: None,
    phase: None,
    diagram: None,
    optics: AEROSOL_ONLY,
    sources: &[ICE_GIANT_HAZE_IDENTITY],
};

/// Row 78, Mars-type dust: weathered, iron-rich basaltic fines.
const MARS_DUST: Substance = Substance {
    key: SubstanceKey::new_const("mars_dust"),
    kind: SubstanceKind::Lithology,
    elements: &[],
    charge: 0,
    molar_mass_g_per_mol: None,
    gas: None,
    phase: None,
    diagram: None,
    optics: AEROSOL_ONLY,
    sources: &[MARS_DUST_IDENTITY],
};

/// Row 79, sulphuric acid carrying Venus's unidentified ultraviolet absorber (T24.g's mode 2).
const ACID_UV_ABSORBER: Substance = Substance {
    key: SubstanceKey::new_const("acid_uv_absorber"),
    kind: SubstanceKind::Solution,
    elements: &[],
    charge: 0,
    molar_mass_g_per_mol: None,
    gas: None,
    phase: None,
    diagram: None,
    optics: AEROSOL_ONLY,
    sources: &[ACID_UV_ABSORBER_IDENTITY],
};

/// Row 80, an aqueous `NaCl` brine.
const BRINE_NACL: Substance = Substance {
    key: SubstanceKey::new_const("brine_nacl"),
    kind: SubstanceKind::Solution,
    elements: &[],
    charge: 0,
    molar_mass_g_per_mol: None,
    gas: None,
    phase: None,
    diagram: None,
    optics: NO_CLIENT_OPTICS,
    sources: &[BRINE_NACL_IDENTITY],
};

/// Row 81, an aqueous `MgSO4` brine.
const BRINE_MGSO4: Substance = Substance {
    key: SubstanceKey::new_const("brine_mgso4"),
    kind: SubstanceKind::Solution,
    elements: &[],
    charge: 0,
    molar_mass_g_per_mol: None,
    gas: None,
    phase: None,
    diagram: None,
    optics: NO_CLIENT_OPTICS,
    sources: &[BRINE_MGSO4_IDENTITY],
};

/// Row 82, an aqueous sodium perchlorate brine.
const BRINE_NACLO4: Substance = Substance {
    key: SubstanceKey::new_const("brine_naclo4"),
    kind: SubstanceKind::Solution,
    elements: &[],
    charge: 0,
    molar_mass_g_per_mol: None,
    gas: None,
    phase: None,
    diagram: None,
    optics: NO_CLIENT_OPTICS,
    sources: &[BRINE_NACLO4_IDENTITY],
};

/// Row 83, an aqueous magnesium perchlorate brine.
const BRINE_MGCLO4: Substance = Substance {
    key: SubstanceKey::new_const("brine_mgclo4"),
    kind: SubstanceKind::Solution,
    elements: &[],
    charge: 0,
    molar_mass_g_per_mol: None,
    gas: None,
    phase: None,
    diagram: None,
    optics: NO_CLIENT_OPTICS,
    sources: &[BRINE_MGCLO4_IDENTITY],
};

/// Row 84, the ammonia–water system, in every phase.
const AMMONIA_WATER: Substance = Substance {
    key: SubstanceKey::new_const("nh3_h2o"),
    kind: SubstanceKind::Solution,
    elements: &[],
    charge: 0,
    molar_mass_g_per_mol: None,
    gas: None,
    phase: None,
    diagram: None,
    optics: NO_CLIENT_OPTICS,
    sources: &[AMMONIA_WATER_IDENTITY],
};

/// Row 85, Titan's seas' methane–ethane–nitrogen liquid.
const TITAN_LIQUID: Substance = Substance {
    key: SubstanceKey::new_const("ch4_c2h6_n2"),
    kind: SubstanceKind::Solution,
    elements: &[],
    charge: 0,
    molar_mass_g_per_mol: None,
    gas: None,
    phase: None,
    diagram: None,
    optics: NO_CLIENT_OPTICS,
    sources: &[TITAN_LIQUID_IDENTITY],
};

/// Row 86, anorthosite, a plagioclase rock.
const ANORTHOSITE: Substance = Substance {
    key: SubstanceKey::new_const("anorthosite"),
    kind: SubstanceKind::Lithology,
    elements: &[],
    charge: 0,
    molar_mass_g_per_mol: None,
    gas: None,
    phase: None,
    diagram: None,
    optics: NO_CLIENT_OPTICS,
    sources: &[ANORTHOSITE_IDENTITY],
};

/// Row 87, a feldspathic crust, a flotation crust as impacts mixed it.
const FELDSPATHIC: Substance = Substance {
    key: SubstanceKey::new_const("feldspathic"),
    kind: SubstanceKind::Lithology,
    elements: &[],
    charge: 0,
    molar_mass_g_per_mol: None,
    gas: None,
    phase: None,
    diagram: None,
    optics: NO_CLIENT_OPTICS,
    sources: &[FELDSPATHIC_IDENTITY],
};

/// Row 88, basalt, a terrestrial tholeiite.
const BASALT: Substance = Substance {
    key: SubstanceKey::new_const("basalt"),
    kind: SubstanceKind::Lithology,
    elements: &[],
    charge: 0,
    molar_mass_g_per_mol: None,
    gas: None,
    phase: None,
    diagram: None,
    optics: AEROSOL_ONLY,
    sources: &[BASALT_IDENTITY],
};

/// Row 89, an iron-rich (`FeO` about 18–22 wt%), anhydrous basalt.
const BASALT_FE: Substance = Substance {
    key: SubstanceKey::new_const("basalt_fe"),
    kind: SubstanceKind::Lithology,
    elements: &[],
    charge: 0,
    molar_mass_g_per_mol: None,
    gas: None,
    phase: None,
    diagram: None,
    optics: AEROSOL_ONLY,
    sources: &[BASALT_FE_IDENTITY],
};

/// Row 90, komatiite, an ultramafic lava (`MgO` at least 18 wt%).
const KOMATIITE: Substance = Substance {
    key: SubstanceKey::new_const("komatiite"),
    kind: SubstanceKind::Lithology,
    elements: &[],
    charge: 0,
    molar_mass_g_per_mol: None,
    gas: None,
    phase: None,
    diagram: None,
    optics: NO_CLIENT_OPTICS,
    sources: &[KOMATIITE_IDENTITY],
};

/// Row 91, a reduced, low-FeO basalt, Mercury's lid.
const BASALT_REDUCED: Substance = Substance {
    key: SubstanceKey::new_const("basalt_reduced"),
    kind: SubstanceKind::Lithology,
    elements: &[],
    charge: 0,
    molar_mass_g_per_mol: None,
    gas: None,
    phase: None,
    diagram: None,
    optics: NO_CLIENT_OPTICS,
    sources: &[BASALT_REDUCED_IDENTITY],
};

/// Row 92, granite, a tertiary (continental) crust.
const GRANITE: Substance = Substance {
    key: SubstanceKey::new_const("granite"),
    kind: SubstanceKind::Lithology,
    elements: &[],
    charge: 0,
    molar_mass_g_per_mol: None,
    gas: None,
    phase: None,
    diagram: None,
    optics: NO_CLIENT_OPTICS,
    sources: &[GRANITE_IDENTITY],
};

/// Row 93, an ultramafic rock (peridotite).
const ULTRAMAFIC: Substance = Substance {
    key: SubstanceKey::new_const("ultramafic"),
    kind: SubstanceKind::Lithology,
    elements: &[],
    charge: 0,
    molar_mass_g_per_mol: None,
    gas: None,
    phase: None,
    diagram: None,
    optics: NO_CLIENT_OPTICS,
    sources: &[ULTRAMAFIC_IDENTITY],
};

/// Row 94, an Fe-oxidised (ferric) rock: [`BASALT_FE`]'s Fe-rich basalt oxidised at the surface,
/// as Mars's red soils and weathered rock (decision-backlog-2 §1.1).
const FERRIC: Substance = Substance {
    key: SubstanceKey::new_const("ferric"),
    kind: SubstanceKind::Lithology,
    elements: &[],
    charge: 0,
    molar_mass_g_per_mol: None,
    gas: None,
    phase: None,
    diagram: None,
    optics: NO_CLIENT_OPTICS,
    sources: &[FERRIC_IDENTITY],
};

/// Row 95, a phyllosilicate-rich rock.
const PHYLLOSILICATE: Substance = Substance {
    key: SubstanceKey::new_const("phyllosilicate"),
    kind: SubstanceKind::Lithology,
    elements: &[],
    charge: 0,
    molar_mass_g_per_mol: None,
    gas: None,
    phase: None,
    diagram: None,
    optics: NO_CLIENT_OPTICS,
    sources: &[PHYLLOSILICATE_IDENTITY],
};

/// Row 96, quenched lava glass.
const LAVA_GLASS: Substance = Substance {
    key: SubstanceKey::new_const("lava_glass"),
    kind: SubstanceKind::Lithology,
    elements: &[],
    charge: 0,
    molar_mass_g_per_mol: None,
    gas: None,
    phase: None,
    diagram: None,
    optics: AEROSOL_ONLY,
    sources: &[LAVA_GLASS_IDENTITY],
};

/// Row 97, a Ca–Al refractory residue.
const CA_AL_RESIDUE: Substance = Substance {
    key: SubstanceKey::new_const("ca_al_residue"),
    kind: SubstanceKind::Lithology,
    elements: &[],
    charge: 0,
    molar_mass_g_per_mol: None,
    gas: None,
    phase: None,
    diagram: None,
    optics: NO_CLIENT_OPTICS,
    sources: &[CA_AL_RESIDUE_IDENTITY],
};

/// Row 98, Fe–Ni metal.
const IRON_NICKEL: Substance = Substance {
    key: SubstanceKey::new_const("iron_nickel"),
    kind: SubstanceKind::Lithology,
    elements: &[],
    charge: 0,
    molar_mass_g_per_mol: None,
    gas: None,
    phase: None,
    diagram: None,
    optics: NO_CLIENT_OPTICS,
    sources: &[IRON_NICKEL_IDENTITY],
};

/// Row 99, silicon carbide, a carbon-rich world's mineral.
const SILICON_CARBIDE: Substance = Substance {
    key: SubstanceKey::new_const("SiC"),
    kind: SubstanceKind::Mineral,
    elements: &[(Element::Si, 1), (Element::C, 1)],
    charge: 0,
    molar_mass_g_per_mol: Some(40.096),
    gas: None,
    phase: None,
    diagram: None,
    optics: NO_CLIENT_OPTICS,
    sources: IDENTITY,
};

/// Row 100, magnesium sulphate, an evaporite and an icy moon's salt.
const MAGNESIUM_SULPHATE: Substance = Substance {
    key: SubstanceKey::new_const("MgSO4"),
    kind: SubstanceKind::Mineral,
    elements: &[(Element::Mg, 1), (Element::S, 1), (Element::O, 4)],
    charge: 0,
    molar_mass_g_per_mol: Some(120.361),
    gas: None,
    phase: None,
    diagram: None,
    optics: NO_CLIENT_OPTICS,
    sources: IDENTITY,
};

/// Row 101, sodium carbonate, the salt of Ceres's faculae.
const SODIUM_CARBONATE: Substance = Substance {
    key: SubstanceKey::new_const("Na2CO3"),
    kind: SubstanceKind::Mineral,
    elements: &[(Element::Na, 2), (Element::C, 1), (Element::O, 3)],
    charge: 0,
    molar_mass_g_per_mol: Some(105.988),
    gas: None,
    phase: None,
    diagram: None,
    optics: NO_CLIENT_OPTICS,
    sources: IDENTITY,
};

/// Row 102, calcium carbonate (calcite), a carbonate sediment.
const CALCITE: Substance = Substance {
    key: SubstanceKey::new_const("CaCO3"),
    kind: SubstanceKind::Mineral,
    elements: &[(Element::Ca, 1), (Element::C, 1), (Element::O, 3)],
    charge: 0,
    molar_mass_g_per_mol: Some(100.086),
    gas: None,
    phase: None,
    diagram: None,
    optics: NO_CLIENT_OPTICS,
    sources: IDENTITY,
};

/// Row 103, carbonaceous matter, refractory organics.
const CARBONACEOUS: Substance = Substance {
    key: SubstanceKey::new_const("carbonaceous"),
    kind: SubstanceKind::Organic,
    elements: &[],
    charge: 0,
    molar_mass_g_per_mol: None,
    gas: None,
    phase: None,
    diagram: None,
    optics: NO_CLIENT_OPTICS,
    sources: &[CARBONACEOUS_IDENTITY],
};

/// Row 104, a CI chondrite, fully aqueously altered.
const CHONDRITE_CI: Substance = Substance {
    key: SubstanceKey::new_const("chondrite_ci"),
    kind: SubstanceKind::Lithology,
    elements: &[],
    charge: 0,
    molar_mass_g_per_mol: None,
    gas: None,
    phase: None,
    diagram: None,
    optics: NO_CLIENT_OPTICS,
    sources: &[CHONDRITE_CI_IDENTITY],
};

/// Row 105, a CM chondrite, partially altered.
const CHONDRITE_CM: Substance = Substance {
    key: SubstanceKey::new_const("chondrite_cm"),
    kind: SubstanceKind::Lithology,
    elements: &[],
    charge: 0,
    molar_mass_g_per_mol: None,
    gas: None,
    phase: None,
    diagram: None,
    optics: NO_CLIENT_OPTICS,
    sources: &[CHONDRITE_CM_IDENTITY],
};

/// Row 106, a CV chondrite, anhydrous or dehydrated and oxidised, standing for CV, CO and CK.
const CHONDRITE_CV: Substance = Substance {
    key: SubstanceKey::new_const("chondrite_cv"),
    kind: SubstanceKind::Lithology,
    elements: &[],
    charge: 0,
    molar_mass_g_per_mol: None,
    gas: None,
    phase: None,
    diagram: None,
    optics: NO_CLIENT_OPTICS,
    sources: &[CHONDRITE_CV_IDENTITY],
};

/// Row 107, an H chondrite, the most metal-rich ordinary chondrite.
const CHONDRITE_H: Substance = Substance {
    key: SubstanceKey::new_const("chondrite_h"),
    kind: SubstanceKind::Lithology,
    elements: &[],
    charge: 0,
    molar_mass_g_per_mol: None,
    gas: None,
    phase: None,
    diagram: None,
    optics: NO_CLIENT_OPTICS,
    sources: &[CHONDRITE_H_IDENTITY],
};

/// Row 108, an L chondrite.
const CHONDRITE_L: Substance = Substance {
    key: SubstanceKey::new_const("chondrite_l"),
    kind: SubstanceKind::Lithology,
    elements: &[],
    charge: 0,
    molar_mass_g_per_mol: None,
    gas: None,
    phase: None,
    diagram: None,
    optics: NO_CLIENT_OPTICS,
    sources: &[CHONDRITE_L_IDENTITY],
};

/// Row 109, an LL chondrite, the most oxidised ordinary chondrite.
const CHONDRITE_LL: Substance = Substance {
    key: SubstanceKey::new_const("chondrite_ll"),
    kind: SubstanceKind::Lithology,
    elements: &[],
    charge: 0,
    molar_mass_g_per_mol: None,
    gas: None,
    phase: None,
    diagram: None,
    optics: NO_CLIENT_OPTICS,
    sources: &[CHONDRITE_LL_IDENTITY],
};

/// Row 110, an enstatite chondrite, EH and EL in one row.
const CHONDRITE_E: Substance = Substance {
    key: SubstanceKey::new_const("chondrite_e"),
    kind: SubstanceKind::Lithology,
    elements: &[],
    charge: 0,
    molar_mass_g_per_mol: None,
    gas: None,
    phase: None,
    diagram: None,
    optics: NO_CLIENT_OPTICS,
    sources: &[CHONDRITE_E_IDENTITY],
};

/// Row 111, cometary refractory: anhydrous silicates and sulphides with about 45 wt% organics.
const COMETARY: Substance = Substance {
    key: SubstanceKey::new_const("cometary"),
    kind: SubstanceKind::Lithology,
    elements: &[],
    charge: 0,
    molar_mass_g_per_mol: None,
    gas: None,
    phase: None,
    diagram: None,
    optics: NO_CLIENT_OPTICS,
    sources: &[COMETARY_IDENTITY],
};

/// What a material with no single formula is, and where the generator takes it from: its identity's
/// source, since it has no stoichiometry or molar mass (decision-composition §1.1).
const fn material(citation: &'static str) -> Source {
    Source {
        covers: "kind",
        citation,
        basis: Basis::Fact,
    }
}

/// [`THOLIN`]'s identity.
const THOLIN_IDENTITY: Source = material(
    "the photochemical organic haze of Titan and the outer Solar System's bodies, laboratory tholin \
     (Khare et al. 1984, Icarus 60, 127) the analogue; the colouring agent Cruikshank, Imanaka and \
     Dalle Ore 2005 (Adv. Space Res. 36, 178) review",
);

/// [`SOOT`]'s identity.
const SOOT_IDENTITY: Source = material(
    "a hydrocarbon (soot) haze, which warm giants form below about 950 K (Gao et al. 2020, Nat. \
     Astron. 4, 951; P14.T52.b's rule)",
);

/// [`CHROMOPHORE`]'s identity.
const CHROMOPHORE_IDENTITY: Source = material(
    "the cold giants' red chromophore, no deeper than about 0.2 bar, an unidentified material known \
     by its retrieved absorption (Braude et al. 2020, Icarus 338, 113589; P14.T52.b's rule)",
);

/// [`ICE_GIANT_HAZE`]'s identity.
const ICE_GIANT_HAZE_IDENTITY: Source = material(
    "the haze above the ice giants' methane layer (Irwin et al. 2022, JGR Planets 127, \
     e2022JE007189; P14.T52.b's rule)",
);

/// [`MARS_DUST`]'s identity.
const MARS_DUST_IDENTITY: Source = material(
    "Mars's bright dust, weathered iron-rich basaltic fines, an assemblage placed on any Mars-like \
     world (R09's reference rows, `planetary::surface::reference`; decision-substance-names §4.3)",
);

/// [`ACID_UV_ABSORBER`]'s identity.
const ACID_UV_ABSORBER_IDENTITY: Source = material(
    "P14.T24.g's mode 2, sulphuric acid carrying Venus's unidentified ultraviolet absorber, an \
     aerosol whose column is a sphere at H2SO4's liquid density (Pérez-Hoyos et al. 2018; Lee et \
     al. 2021 and 2022; Spacek et al. 2026; science-r08-unowned-absorbers.md §1)",
);

/// [`BRINE_NACL`]'s identity.
const BRINE_NACL_IDENTITY: Source = material(
    "an aqueous solution of NaCl, which lowers water's freezing point to its eutectic (the surfaces \
     audit, audit-composition-surfaces.md §2.5)",
);

/// [`BRINE_MGSO4`]'s identity.
const BRINE_MGSO4_IDENTITY: Source = material(
    "an aqueous solution of MgSO4, Europa's and Mars's sulphate brine (McCord et al. 1998, Science \
     280, 1242; the surfaces audit §2.4–§2.5)",
);

/// [`BRINE_NACLO4`]'s identity.
const BRINE_NACLO4_IDENTITY: Source = material(
    "an aqueous solution of sodium perchlorate, Mars's (Chevrier, Hanley and Altheide 2009, GRL 36, \
     L10202; Hecht et al. 2009, Science 325, 64)",
);

/// [`BRINE_MGCLO4`]'s identity.
const BRINE_MGCLO4_IDENTITY: Source = material(
    "an aqueous solution of magnesium perchlorate, Mars's coldest brine (Chevrier, Hanley and \
     Altheide 2009, GRL 36, L10202)",
);

/// [`AMMONIA_WATER`]'s identity.
const AMMONIA_WATER_IDENTITY: Source = material(
    "the ammonia–water system in every phase: the liquid of the icy moons' oceans and \
     cryovolcanism, to its eutectic (Kargel 1992, Icarus 100, 556), and its hydrates' ices (Brown \
     and Calvin 2000, Science 287, 107)",
);

/// [`TITAN_LIQUID`]'s identity.
const TITAN_LIQUID_IDENTITY: Source = material(
    "the methane–ethane liquid of Titan's seas, nitrogen dissolved in it, its composition varying \
     lake by lake (Mastrogiuseppe et al. 2014, GRL 41, 1432, and 2019, Nat. Astron. 3, 535; Malaska \
     et al. 2017, Icarus 289, 94)",
);

/// [`ANORTHOSITE`]'s identity.
const ANORTHOSITE_IDENTITY: Source = material(
    "anorthosite, a plagioclase rock (90–97 per cent plagioclase in science-composition-crusts.md \
     §7's reading of the Handbook of Mineralogy), a magma ocean's flotation cumulate",
);

/// [`FELDSPATHIC`]'s identity.
const FELDSPATHIC_IDENTITY: Source = material(
    "a feldspathic crust, a primary flotation crust as basin-scale impacts mixed it, about 75–80 per \
     cent plagioclase by mass, as the lunar highlands (Hu, Ehlmann and Seager 2012, ApJ 752, 7, \
     their surface type; science-composition-crusts.md §7)",
);

/// [`BASALT`]'s identity.
const BASALT_IDENTITY: Source = material(
    "basalt, a secondary crust from the mantle's partial melting, as a terrestrial tholeiite (R09's \
     reference rows; the surfaces audit §2.1)",
);

/// [`BASALT_FE`]'s identity.
const BASALT_FE_IDENTITY: Source = material(
    "an iron-rich (FeO about 18–22 wt%), anhydrous basalt with variable titanium, as the lunar \
     maria's and Mars's (science-composition-crusts.md §2)",
);

/// [`KOMATIITE`]'s identity.
const KOMATIITE_IDENTITY: Source = material(
    "komatiite, an ultramafic lava with MgO at least 18 wt% (the IUGS rules, Le Bas 2000, J. Petrol. \
     41, 1467), a hot mantle's secondary crust (the surfaces audit §2.1)",
);

/// [`BASALT_REDUCED`]'s identity.
const BASALT_REDUCED_IDENTITY: Source = material(
    "a reduced, low-FeO basalt, Mercury's lid: high Mg/Si, low Al/Si and Ca/Si and little iron \
     (Nittler et al. 2011, Science 333, 1847), at 2.6–6.3 log units below the iron–wüstite buffer \
     (McCubbin et al. 2012, GRL 39)",
);

/// [`GRANITE`]'s identity.
const GRANITE_IDENTITY: Source = material(
    "granite, a tertiary (continental) crust, which needs water to form (Campbell and Taylor 1983, \
     GRL 10, 1061; the surfaces audit §2.1)",
);

/// [`ULTRAMAFIC`]'s identity.
const ULTRAMAFIC_IDENTITY: Source = material(
    "an ultramafic rock (peridotite), Hu, Ehlmann and Seager 2012's ultramafic surface type (ApJ 752, \
     7, Table 2), an exposed mantle",
);

/// [`FERRIC`]'s identity.
const FERRIC_IDENTITY: Source = material(
    "an Fe-oxidised (ferric) rock, `basalt_fe`'s Fe-rich basalt oxidised at the surface, as Mars's \
     red soils and weathered rock (decision-backlog-2 §1.1): Hu, Ehlmann and Seager 2012's \
     Fe-oxidised surface type by name and spectral role (ApJ 752, 7, Table 2), not their 50% \
     nanophase hematite, a spectral weight, as a mineral mode; the dry oxidising age's weathering \
     (Bibring et al. 2006, Science 312, 400)",
);

/// [`PHYLLOSILICATE`]'s identity.
const PHYLLOSILICATE_IDENTITY: Source = material(
    "a phyllosilicate-rich rock, neutral water's weathering product (Poulet et al. 2005, Nature 438, \
     623; Bibring et al. 2006), Ceres's crust (De Sanctis et al. 2015, Nature 528, 241)",
);

/// [`LAVA_GLASS`]'s identity.
const LAVA_GLASS_IDENTITY: Source = material(
    "quenched lava glass, a lava world's dark surface (Essack, Seager and Pajusalu 2020, ApJ 898, \
     160)",
);

/// [`CA_AL_RESIDUE`]'s identity.
const CA_AL_RESIDUE_IDENTITY: Source = material(
    "a Ca–Al refractory residue, the lag Léger et al. 2011 propose for a lava world (Icarus 213, 1) \
     and Kite et al. 2016 find only where recycling is slow (ApJ 828, 80)",
);

/// [`IRON_NICKEL`]'s identity.
const IRON_NICKEL_IDENTITY: Source = material(
    "Fe–Ni metal, Hu, Ehlmann and Seager 2012's metal-rich surface type (ApJ 752, 7, Table 2), an \
     exposed core",
);

/// [`CARBONACEOUS`]'s identity.
const CARBONACEOUS_IDENTITY: Source = material(
    "carbonaceous matter, refractory organics with no single formula, as a carbon-rich world's \
     (Bergin et al. 2023, ApJL 949, L17) and a primitive body's",
);

/// [`CHONDRITE_CI`]'s identity.
const CHONDRITE_CI_IDENTITY: Source = material(
    "a CI chondrite, fully aqueously altered: phyllosilicates, magnetite, carbonates, sulphides and \
     organics, an undifferentiated body's material (science-composition-crusts.md §1.3)",
);

/// [`CHONDRITE_CM`]'s identity.
const CHONDRITE_CM_IDENTITY: Source =
    material("a CM chondrite, partially altered (science-composition-crusts.md §1.3)");

/// [`CHONDRITE_CV`]'s identity.
const CHONDRITE_CV_IDENTITY: Source = material(
    "a CV chondrite, anhydrous or dehydrated and oxidised, standing for CV, CO and CK \
     (science-composition-crusts.md §1.3)",
);

/// [`CHONDRITE_H`]'s identity.
const CHONDRITE_H_IDENTITY: Source = material(
    "an H chondrite, the most metal-rich and least oxidised ordinary chondrite \
     (science-composition-crusts.md §1.3)",
);

/// [`CHONDRITE_L`]'s identity.
const CHONDRITE_L_IDENTITY: Source =
    material("an L chondrite, an ordinary chondrite (science-composition-crusts.md §1.3)");

/// [`CHONDRITE_LL`]'s identity.
const CHONDRITE_LL_IDENTITY: Source = material(
    "an LL chondrite, the most oxidised ordinary chondrite (science-composition-crusts.md §1.3)",
);

/// [`CHONDRITE_E`]'s identity.
const CHONDRITE_E_IDENTITY: Source = material(
    "an enstatite chondrite, EH and EL in one row, their properties similar (Macke et al. 2010, \
     MAPS 45, 1513; science-composition-crusts.md §1.3)",
);

/// [`COMETARY`]'s identity.
const COMETARY_IDENTITY: Source = material(
    "the refractory material of a body that accreted ice and never altered it: anhydrous silicates \
     and sulphides with about 45 wt% organics (Bardyn et al. 2017, MNRAS 469, S712), a comet's dust \
     mantle and the dark material of D- and P-type surfaces (science-composition-crusts.md §1.3)",
);
