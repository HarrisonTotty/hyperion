//! The rows' condensed columns (P14.T49.b's second step): each row's grain density, transition
//! and mechanics family, with their sources.
//!
//! Every figure is a source's, read by the lane's research (2026-10-14) and checked here against
//! it, or reduced from read figures by the arithmetic its source states. Where a row's own figure
//! was not read, its source says what stands in and why, and an assumption is labelled one. One
//! figure from memory stays, labelled so where it is cited: granite's solidus,
//! `planetary::surface::reference`'s. The fourteen rows with a phase diagram take its densities
//! and triple point ([`GrainDensity::OfDiagram`]). R09's reference rows (`basalt`, `basalt_fe`,
//! `granite`, `feldspathic`, `phyllosilicate`, `Na2CO3`, `mars_dust`) are
//! `planetary::surface::reference`'s, to the bit, which its tests hold.
//!
//! NIST-JANAF (Chase 1998, NIST SRD 13) gives single transition temperatures, each a fact with
//! its table cited, and the two decompositions computed from its tables below; no table of it is
//! fitted or committed (decision-composition §5).

use hyperion_surface::field::MechanicsFamily;

use super::super::condensed::{
    CondensedColumns, DensityAt, GrainDensity, Transition, TransitionKind,
};
use super::super::diagram::PhaseDiagram;
use super::super::{Basis, Source};
use super::diagrams;
use crate::units::{Kelvin, KilogramsPerCubicMetre};

/// A density of `kg_m3` kg m⁻³ at `at_k` kelvin.
#[must_use]
const fn density_at(kg_m3: f64, at_k: f64) -> DensityAt {
    DensityAt {
        density: KilogramsPerCubicMetre::new(kg_m3),
        at: Kelvin::new(at_k),
    }
}

/// A [`GrainDensity::Fixed`] of the solid alone, `kg_m3` kg m⁻³ at `at_k` kelvin.
#[must_use]
const fn solid(kg_m3: f64, at_k: f64) -> GrainDensity {
    GrainDensity::Fixed {
        solid: Some(density_at(kg_m3, at_k)),
        liquid: None,
    }
}

/// A [`GrainDensity::Fixed`] of the liquid alone, `kg_m3` kg m⁻³ at `at_k` kelvin.
#[must_use]
const fn liquid(kg_m3: f64, at_k: f64) -> GrainDensity {
    GrainDensity::Fixed {
        solid: None,
        liquid: Some(density_at(kg_m3, at_k)),
    }
}

/// A transition of `kind` at `k` kelvin.
#[must_use]
const fn at(kind: TransitionKind, k: f64) -> Transition {
    Transition {
        kind,
        temperature: Kelvin::new(k),
    }
}

/// A source of the values `covers` names.
#[must_use]
const fn source(covers: &'static str, citation: &'static str, basis: Basis) -> Source {
    Source {
        covers,
        citation,
        basis,
    }
}

/// The condensed columns of a row with a phase diagram: its densities and its triple point.
#[must_use]
const fn of_diagram(
    diagram: &PhaseDiagram,
    mechanics: MechanicsFamily,
    sources: &'static [Source],
) -> CondensedColumns {
    CondensedColumns {
        grain_density: GrainDensity::OfDiagram,
        transition: Transition {
            kind: TransitionKind::TriplePoint,
            temperature: diagram.triple_temperature,
        },
        mechanics,
        sources,
    }
}

/// The source of a diagram row's grain density and transition.
const DIAGRAM: Source = source(
    "grain density, transition",
    "the row's phase diagram (`rows/diagrams.rs`, each value with its sources): its solid \
     phases' measured densities, not modelled in a phase whose density was not read, its \
     saturated liquid's ancillary density, and its triple point, the lowest pressure at which its \
     liquid exists",
    Basis::Fact,
);

/// Water ice's family.
const WATER_ICE_FAMILY: Source = source(
    "mechanics",
    "`WaterIce`, decision-composition §1.7's family of water ice and of a water liquid's frozen \
     form",
    Basis::Adopted,
);

/// Any other volatile's ice's family.
const VOLATILE_ICE_FAMILY: Source = source(
    "mechanics",
    "`VolatileIce`, decision-composition §1.7's family of the ices of volatiles other than water, \
     a liquid's being its frozen form's",
    Basis::Adopted,
);

/// The family of a silicate rock or a brittle mineral.
const SILICATE_FAMILY: Source = source(
    "mechanics",
    "`Silicate`, decision-composition §1.7's family of silicate rock, regolith and dust, which \
     takes the brittle minerals too",
    Basis::Adopted,
);

/// The family of a salt or an evaporite.
const SALT_FAMILY: Source = source(
    "mechanics",
    "`Salt`, decision-composition §1.7's family of salts and evaporites",
    Basis::Adopted,
);

/// The family of a metal.
const METAL_FAMILY: Source = source(
    "mechanics",
    "`Metal`, decision-composition §1.7's family of metals",
    Basis::Adopted,
);

/// The family of an organic solid.
const ORGANIC_FAMILY: Source = source(
    "mechanics",
    "`Organic`, decision-composition §1.7's family of organic solids, as tholin, the nearest \
     for a carbonaceous solid of no crystal",
    Basis::Adopted,
);

/// The diagram rows' sources, water's and the other volatiles'.
const WATER_SOURCES: &[Source] = &[DIAGRAM, WATER_ICE_FAMILY];
const VOLATILE_SOURCES: &[Source] = &[DIAGRAM, VOLATILE_ICE_FAMILY];

/// Water: ice Ih's density (`ice_ih_density`) and its triple point, 273.16 K; at one atmosphere
/// ice Ih melts 0.0075 K lower, which a palette's 273.15 K rounds.
pub(in crate::substance) const WATER: CondensedColumns =
    of_diagram(&diagrams::WATER, MechanicsFamily::WaterIce, WATER_SOURCES);
/// Methane.
pub(in crate::substance) const METHANE: CondensedColumns = of_diagram(
    &diagrams::METHANE,
    MechanicsFamily::VolatileIce,
    VOLATILE_SOURCES,
);
/// Ammonia.
pub(in crate::substance) const AMMONIA: CondensedColumns = of_diagram(
    &diagrams::AMMONIA,
    MechanicsFamily::VolatileIce,
    VOLATILE_SOURCES,
);
/// Nitrogen.
pub(in crate::substance) const NITROGEN: CondensedColumns = of_diagram(
    &diagrams::NITROGEN,
    MechanicsFamily::VolatileIce,
    VOLATILE_SOURCES,
);
/// Oxygen.
pub(in crate::substance) const OXYGEN: CondensedColumns = of_diagram(
    &diagrams::OXYGEN,
    MechanicsFamily::VolatileIce,
    VOLATILE_SOURCES,
);
/// Carbon dioxide.
pub(in crate::substance) const CARBON_DIOXIDE: CondensedColumns = of_diagram(
    &diagrams::CARBON_DIOXIDE,
    MechanicsFamily::VolatileIce,
    VOLATILE_SOURCES,
);
/// Argon, whose solid's density was not read: its solid is not modelled.
pub(in crate::substance) const ARGON: CondensedColumns = of_diagram(
    &diagrams::ARGON,
    MechanicsFamily::VolatileIce,
    VOLATILE_SOURCES,
);
/// Krypton.
pub(in crate::substance) const KRYPTON: CondensedColumns = of_diagram(
    &diagrams::KRYPTON,
    MechanicsFamily::VolatileIce,
    VOLATILE_SOURCES,
);
/// Xenon.
pub(in crate::substance) const XENON: CondensedColumns = of_diagram(
    &diagrams::XENON,
    MechanicsFamily::VolatileIce,
    VOLATILE_SOURCES,
);
/// Nitrous oxide.
pub(in crate::substance) const NITROUS_OXIDE: CondensedColumns = of_diagram(
    &diagrams::NITROUS_OXIDE,
    MechanicsFamily::VolatileIce,
    VOLATILE_SOURCES,
);
/// Carbon monoxide, whose β solid's density was not read.
pub(in crate::substance) const CARBON_MONOXIDE: CondensedColumns = of_diagram(
    &diagrams::CARBON_MONOXIDE,
    MechanicsFamily::VolatileIce,
    VOLATILE_SOURCES,
);
/// Sulphur dioxide.
pub(in crate::substance) const SULPHUR_DIOXIDE: CondensedColumns = of_diagram(
    &diagrams::SULPHUR_DIOXIDE,
    MechanicsFamily::VolatileIce,
    VOLATILE_SOURCES,
);
/// Hydrogen sulphide, whose solids' densities were not read.
pub(in crate::substance) const HYDROGEN_SULPHIDE: CondensedColumns = of_diagram(
    &diagrams::HYDROGEN_SULPHIDE,
    MechanicsFamily::VolatileIce,
    VOLATILE_SOURCES,
);
/// Ethane, whose solid's density was not read.
pub(in crate::substance) const ETHANE: CondensedColumns = of_diagram(
    &diagrams::ETHANE,
    MechanicsFamily::VolatileIce,
    VOLATILE_SOURCES,
);

/// Methanol, with no phase diagram: its α crystal at 122 K, the outer Solar System's methanol
/// ice's phase, and its melting point. Its liquid's density is not modelled: no reference equation
/// of state for it was read.
pub(in crate::substance) const METHANOL: CondensedColumns = CondensedColumns {
    grain_density: solid(1_027.94, 122.0),
    transition: at(TransitionKind::Melting, 175.37),
    mechanics: MechanicsFamily::VolatileIce,
    sources: &[
        source(
            "grain density",
            "the X-ray density of α-methanol (P2₁2₁2₁, Z 4) from Kirchner, Das and Boese 2008, \
             Cryst. Growth Des. 8, 763 (COD 4503066: a 4.6469, b 4.9285, c 9.0403 Å at 122 K), \
             with the row's molar mass and the exact Avogadro constant: 1,027.94 kg m⁻³ (the \
             CIF's 1.028); its β phase, 990.99 at 160 K (COD 4503067), not modelled, its \
             transition's temperature not read",
            Basis::Reduced,
        ),
        source(
            "transition",
            "175.37 K, the melting point COD 4503067's CIF gives for Kirchner, Das and Boese \
             2008's β-methanol (`_chemical_melting_point`)",
            Basis::Fact,
        ),
        VOLATILE_ICE_FAMILY,
    ],
};

/// Iron's condensed phase, a giant's metal cloud: bcc iron.
pub(in crate::substance) const IRON: CondensedColumns = CondensedColumns {
    grain_density: solid(7_874.0, 293.0),
    transition: at(TransitionKind::Melting, 1_811.15),
    mechanics: MechanicsFamily::Metal,
    sources: &[
        source(
            "grain density",
            "bcc iron's X-ray density, the Handbook of Mineralogy's (iron) D(calc.) 7.874 g cm⁻³ \
             at a 2.8664 Å, Z 2 (7.875 recomputed), ±0.1 per cent; its D(meas.) 7.3–7.87 \
             includes porous native iron",
            Basis::Fact,
        ),
        source(
            "transition",
            "1,538 °C, 1,811.15 K, as HSDB and DOE PAC give it through PubChem; NIST-JANAF's \
             table Fe-007 gives δ to liquid at 1,809 K (Chase 1998), its α–γ (1,184 K) and γ–δ \
             (1,665 K) steps being between solids",
            Basis::Fact,
        ),
        METAL_FAMILY,
    ],
};

/// Chromium's condensed phase, a brown dwarf's metal cloud: bcc chromium.
pub(in crate::substance) const CHROMIUM: CondensedColumns = CondensedColumns {
    grain_density: solid(7_200.0, 298.15),
    transition: at(TransitionKind::Melting, 2_180.15),
    mechanics: MechanicsFamily::Metal,
    sources: &[
        source(
            "grain density",
            "bcc chromium's X-ray density, 7,199.6 kg m⁻³, from Swanson, Gilfrich and Ugrinic \
             1955, NBS Circular 539 vol. 5 (COD 5000220: a 2.8839 Å at 25 °C, Z 2), as the \
             Handbook of Mineralogy's D(calc.) 7.20 (D(meas.) 7.17), ±0.4 per cent",
            Basis::Fact,
        ),
        source(
            "transition",
            "1,907 °C, 2,180.15 K, as HSDB and DOE PAC give it through PubChem; NIST-JANAF's \
             table Cr-004 gives 2,130 K (Chase 1998), an older evaluation that Robie and \
             Hemingway 1995 copy; its Néel point at 311.5 K is a λ, no change of phase",
            Basis::Fact,
        ),
        METAL_FAMILY,
    ],
};

/// Ammonium hydrosulphide, the giants' middle deck: it dissociates rather than melts.
pub(in crate::substance) const AMMONIUM_HYDROSULPHIDE: CondensedColumns = CondensedColumns {
    grain_density: solid(1_171.7, 293.0),
    transition: at(TransitionKind::Decomposition, 305.1),
    mechanics: MechanicsFamily::VolatileIce,
    sources: &[
        source(
            "grain density",
            "the X-ray density from West 1934, Z. Kristallogr. 88, 97 (COD 1010249: P4/nmm, \
             a 6.011, c 4.009 Å, Z 2, room temperature), 1.1717 g cm⁻³ (COD 9008022, the same \
             paper's rounder cell, 1.1722), the client's `NH4SH_MOLAR_VOLUME_CM3_PER_MOL`'s; \
             ±0.6 per cent, the cell possibly in kX units (1,165), and a cloud near 200 K denser \
             by an amount not measured",
            Basis::Reduced,
        ),
        source(
            "transition",
            "305.1 K, where the dissociation pressure P_NH3 + P_H2S of pure NH4SH, its two \
             vapours equal, reaches one atmosphere: K = P_NH3 P_H2S = 0.25 atm² in Lewis 1969a's \
             fit to the 1961 CRC's vapour pressures, log K = 14.82 − 4,705 ÷ T (305.45 K by \
             Kelley 1937's fit to Walker and Lumsden 1897, log K = 14.89 − 4,732 ÷ T), as \
             Lodders and Fegley 2024 (arXiv:2410.11138) print them; inside the measured \
             4.2–44.4 °C. A cloud's own condensation is set by the vapours' partial pressures",
            Basis::Reduced,
        ),
        source(
            "mechanics",
            "`VolatileIce`: a volatile condensate of the giants' clouds, soft and dissociating \
             near room temperature (`Salt`, as an ammonium salt, is defensible)",
            Basis::Adopted,
        ),
    ],
};

/// The density of aqueous sulphuric acid at 75.0 wt% and 300 K, kg m⁻³: the client's
/// `sulphuric-acid-75.json`'s acid (Palmer and Williams 1975), which `acid_uv_absorber` shares.
const SULPHURIC_ACID_75_DENSITY_KG_M3: f64 = 1_662.6;

/// The liquidus of aqueous sulphuric acid at 75.0 wt%, kelvin, which `acid_uv_absorber` shares.
const SULPHURIC_ACID_75_LIQUIDUS_K: f64 = 242.0;

/// The 75 wt% acid's density source.
const SULPHURIC_ACID_75_DENSITY: Source = source(
    "grain density",
    "aqueous sulphuric acid at 75.0 wt% H2SO4 and 300 K, the row's reference composition (the \
     client's `sulphuric-acid-75.json`, Palmer and Williams 1975, and the acid of Venus's clouds' \
     index): the International Critical Tables, vol. III (1928), p. 57, d₄ᵗ 1.6644 at 25 °C and \
     1.6597 at 30 °C, linear between them and in kg m⁻³ by the 1901 litre (× 0.999 972): \
     1,662.6, ±0.5 (NBS Circular 19, 6th ed., 1924, Table 13, agrees to 3 × 10⁻⁵ g cm⁻³ at \
     20 °C), at atmospheric pressure. The composition dominates, about 11.5 kg m⁻³ per wt% \
     from 75 to 80 wt% and 9.3 at 84–85, so Venus's 75–85 wt% spans 1,663–1,771",
    Basis::Reduced,
);

/// The 75 wt% acid's liquidus source.
const SULPHURIC_ACID_75_LIQUIDUS: Source = source(
    "transition",
    "the 75 wt% acid's liquidus, where its monohydrate first crystallises, 242 ± 2 K, read off \
     the calculated monohydrate curve of Zeleznik 1991, J. Phys. Chem. Ref. Data 20, 1157, \
     Fig. 1, at x = 0.3553 (the lane's research read 240 ± 4; Gable, Betz and Maron 1950; \
     Kunzler and Giauque 1952), between the dihydrate's congruent melting (73.13 wt%, 233.67 K) \
     and the monohydrate's (84.48 wt%, 281.63 K; Table 5); not the 98 wt% acid's 271.6 K. The \
     frozen acid first melts lower, at the dihydrate–monohydrate eutectic, between 228 and \
     233.67 K by the same figure, unresolved. Venus's droplets supercool well below both",
    Basis::Fact,
);

/// The frozen acid's family.
const SULPHURIC_ACID_FAMILY: Source = source(
    "mechanics",
    "`Salt`: the frozen acid is a hydrate, an oxonium hydrogen-sulphate salt",
    Basis::Adopted,
);

/// Aqueous sulphuric acid at 75 wt%, Venus's clouds (T24.g).
pub(in crate::substance) const SULPHURIC_ACID: CondensedColumns = CondensedColumns {
    grain_density: liquid(SULPHURIC_ACID_75_DENSITY_KG_M3, 300.0),
    transition: at(TransitionKind::Liquidus, SULPHURIC_ACID_75_LIQUIDUS_K),
    mechanics: MechanicsFamily::Salt,
    sources: &[
        SULPHURIC_ACID_75_DENSITY,
        SULPHURIC_ACID_75_LIQUIDUS,
        SULPHURIC_ACID_FAMILY,
    ],
};

/// Sulphuric acid carrying Venus's ultraviolet absorber: a sphere at `H2SO4`'s liquid density,
/// the same constant, with its transition (the absorber's own mass not counted).
pub(in crate::substance) const ACID_UV_ABSORBER: CondensedColumns = CondensedColumns {
    grain_density: liquid(SULPHURIC_ACID_75_DENSITY_KG_M3, 300.0),
    transition: at(TransitionKind::Liquidus, SULPHURIC_ACID_75_LIQUIDUS_K),
    mechanics: MechanicsFamily::Salt,
    sources: &[
        SULPHURIC_ACID_75_DENSITY,
        SULPHURIC_ACID_75_LIQUIDUS,
        SULPHURIC_ACID_FAMILY,
    ],
};

/// Elemental sulphur: α-S8, melting as β-sulphur.
///
/// Its liquid, Io's flows and lakes, has no density here yet: decision-r11-palette-liquids §1.8
/// asks for a measured liquid series (Meyer 1976, Chem. Rev. 76, 367, the finding aid), which was
/// not read, so it is not modelled until P14.T49.b's last piece reads one.
pub(in crate::substance) const SULPHUR: CondensedColumns = CondensedColumns {
    grain_density: solid(2_067.0, 298.0),
    transition: at(TransitionKind::Melting, 388.36),
    mechanics: MechanicsFamily::Salt,
    sources: &[
        source(
            "grain density",
            "α-S8's X-ray density from Rettig and Trotter 1987, Acta Cryst. C43, 2260, at 298 K \
             (COD 9011362: Fddd, a 10.4646, b 12.8660, c 24.4860 Å, Z 16): 2.0670 g cm⁻³, the \
             Handbook of Mineralogy's D(meas.) 2.07, ±0.1 per cent",
            Basis::Fact,
        ),
        source(
            "transition",
            "388.36 K, β-sulphur's melting (NIST-JANAF table S-005, Chase 1998; Robie and \
             Hemingway 1995, USGS Bulletin 2131); α to β at 368.3 K is a step between solids",
            Basis::Fact,
        ),
        source(
            "mechanics",
            "`Salt`: a soft molecular crystal (Mohs 1.5–2.5) laid down as a sublimate or an \
             evaporite; no closer family exists",
            Basis::Adopted,
        ),
    ],
};

/// Potassium chloride, sylvite.
pub(in crate::substance) const POTASSIUM_CHLORIDE: CondensedColumns = CondensedColumns {
    grain_density: solid(1_987.0, 293.0),
    transition: at(TransitionKind::Melting, 1_044.0),
    mechanics: MechanicsFamily::Salt,
    sources: &[
        source(
            "grain density",
            "sylvite's X-ray density from Wyckoff 1963 (COD 9008651: a 6.29294 Å, Z 4), \
             1.9869 g cm⁻³, the Handbook of Mineralogy's D(calc.) 1.987 (D(meas.) 1.993(5)), \
             ±0.3 per cent",
            Basis::Fact,
        ),
        source(
            "transition",
            "1,044 K (NIST-JANAF table Cl-038, Chase 1998)",
            Basis::Fact,
        ),
        SALT_FAMILY,
    ],
};

/// Sodium chloride, halite.
pub(in crate::substance) const SODIUM_CHLORIDE: CondensedColumns = CondensedColumns {
    grain_density: solid(2_163.0, 298.15),
    transition: at(TransitionKind::Melting, 1_073.8),
    mechanics: MechanicsFamily::Salt,
    sources: &[
        source(
            "grain density",
            "halite's X-ray density at 25 °C and 0 kbar from Walker et al. 2004, Am. Mineral. 89, \
             204 (COD 9003308: a 5.6401 Å), 2.1635 g cm⁻³ (Wyckoff 1963, COD 9008678, 2.1630), \
             ±0.2 per cent (the Handbook of Mineralogy's D(meas.) 2.168)",
            Basis::Fact,
        ),
        source(
            "transition",
            "1,073.8 K (NIST-JANAF table Cl-055, Chase 1998)",
            Basis::Fact,
        ),
        SALT_FAMILY,
    ],
};

/// Zinc sulphide, sphalerite.
pub(in crate::substance) const ZINC_SULPHIDE: CondensedColumns = CondensedColumns {
    grain_density: solid(4_089.0, 298.0),
    transition: at(TransitionKind::Melting, 1_973.15),
    mechanics: MechanicsFamily::Silicate,
    sources: &[
        source(
            "grain density",
            "pure synthetic sphalerite's X-ray density from Skinner 1961, Am. Mineral. 46, 1399 \
             (COD 9000107: a 5.4093 Å, Z 4), 4.0891 g cm⁻³, ±0.2 per cent; a natural, \
             iron-bearing sphalerite is lighter (D(meas.) 3.9–4.1, the Handbook of Mineralogy)",
            Basis::Fact,
        ),
        source(
            "transition",
            "1,700 °C, as HSDB and DOE PAC give it through PubChem; that ZnS sublimes and \
             dissociates before it melts at one bar is the lane's research's note, unsourced. \
             Sphalerite is tabulated to 1,300 K, wurtzite above it (Robie and Hemingway 1995), a \
             step between solids. Its condensation is its law aloft's",
            Basis::Fact,
        ),
        source(
            "mechanics",
            "`Silicate`: a brittle mineral (Mohs 3.5–4)",
            Basis::Adopted,
        ),
    ],
};

/// Sodium sulphide, a T dwarf's sulphide cloud.
pub(in crate::substance) const SODIUM_SULPHIDE: CondensedColumns = CondensedColumns {
    grain_density: solid(1_856.0, 293.0),
    transition: at(TransitionKind::Melting, 1_445.0),
    mechanics: MechanicsFamily::Salt,
    sources: &[
        source(
            "grain density",
            "the mean of two antifluorite cells' X-ray densities (Fm3̄m, Z 4): Zintl, Harder and \
             Dauth 1934, Z. Elektrochem. 40, 588 (COD 1011374: a 6.526, possibly kX), 1,865, and \
             Bonneau, Jarvis and Kaner 1992, Inorg. Chem. 31, 2127 (COD 4344366: a 6.547 Å), \
             1,847 kg m⁻³; ±0.5 per cent",
            Basis::Reduced,
        ),
        source(
            "transition",
            "1,445 K (NIST-JANAF table Na-033, Chase 1998), its λ at 1,276 K being in the solid",
            Basis::Fact,
        ),
        source(
            "mechanics",
            "`Salt`: an ionic, soluble alkali sulphide",
            Basis::Adopted,
        ),
    ],
};

/// Manganese sulphide, alabandite.
pub(in crate::substance) const MANGANESE_SULPHIDE: CondensedColumns = CondensedColumns {
    grain_density: solid(4_052.0, 298.0),
    transition: at(TransitionKind::Melting, 1_883.15),
    mechanics: MechanicsFamily::Silicate,
    sources: &[
        source(
            "grain density",
            "alabandite's X-ray density at 0 GPa and 298 K from McCammon 1991, Phys. Chem. \
             Minerals 17, 636 (COD 9005930: a 5.2245 Å, Z 4), 4.0521 g cm⁻³, the Handbook of \
             Mineralogy's D(calc.) 4.053, ±0.1 per cent",
            Basis::Fact,
        ),
        source(
            "transition",
            "1,610 °C, as DOE PAC gives it through PubChem (WebElements agrees); Robie \
             and Hemingway 1995 tabulate cubic MnS to 1,800 K with no transition; it is not in \
             NIST-JANAF",
            Basis::Fact,
        ),
        source(
            "mechanics",
            "`Silicate`: a brittle sulphide mineral (Mohs 3.5–4)",
            Basis::Adopted,
        ),
    ],
};

/// Iron sulphide, troilite.
pub(in crate::substance) const IRON_SULPHIDE: CondensedColumns = CondensedColumns {
    grain_density: solid(4_840.0, 294.0),
    transition: at(TransitionKind::Melting, 1_463.0),
    mechanics: MechanicsFamily::Silicate,
    sources: &[
        source(
            "grain density",
            "troilite's X-ray density at 294 K and 0.0001 GPa from King and Prewitt 1982, Acta \
             Cryst. B38, 1877 (COD 9007650: P6̄2c, a 5.963, c 11.754 Å, Z 12), 4.8395 g cm⁻³ \
             (Skala, Císařová and Drábek 2006, COD 9004034, 4.8350), ±0.1 per cent; natural \
             troilite measures 4.67–4.79 (the Handbook of Mineralogy)",
            Basis::Fact,
        ),
        source(
            "transition",
            "1,463 K, γ to liquid (NIST-JANAF table Fe-025, Chase 1998), its α–β (411 K) and β–γ \
             (598 K) steps being between solids. With metal present the Fe–FeS eutectic melts \
             first: 1,261 K without nickel and 1,223 K with it (Tomkins 2009 and Mare et al. \
             2014, as Gail and Trieloff 2019, A&A 628, A77, quote them), recorded, not used",
            Basis::Fact,
        ),
        source(
            "mechanics",
            "`Silicate`: a brittle sulphide (Mohs 3.5–4.5)",
            Basis::Adopted,
        ),
    ],
};

/// Enstatite, a silicate cloud: orthoenstatite.
pub(in crate::substance) const ENSTATITE: CondensedColumns = CondensedColumns {
    grain_density: solid(3_202.0, 298.0),
    transition: at(TransitionKind::Melting, 1_850.0),
    mechanics: MechanicsFamily::Silicate,
    sources: &[
        source(
            "grain density",
            "orthoenstatite's X-ray density at 0 GPa from Hugh-Jones and Angel 1994, Am. Mineral. \
             79, 405 (COD 9001593: Pbca, a 18.233, b 8.8191, c 5.1802 Å, Z 16), 3.2020 g cm⁻³, \
             the cell science-composition-crusts.md §10.1 used; +0.2/−0 per cent across five \
             room-temperature cells. The crystal's: a hot giant's silicates condense crystalline \
             (Grant et al. 2023), while the client's optics are amorphous enstatite glass's, \
             lighter by an amount not read",
            Basis::Fact,
        ),
        source(
            "transition",
            "1,850 K, III to liquid (NIST-JANAF table Mg-014, Chase 1998), which takes the melt as \
             congruent; enstatite melts incongruently, to forsterite and a liquid",
            Basis::Fact,
        ),
        SILICATE_FAMILY,
    ],
};

/// Forsterite, a silicate cloud.
pub(in crate::substance) const FORSTERITE: CondensedColumns = CondensedColumns {
    grain_density: solid(3_219.0, 298.15),
    transition: at(TransitionKind::Melting, 2_171.0),
    mechanics: MechanicsFamily::Silicate,
    sources: &[
        source(
            "grain density",
            "forsterite's X-ray density at 25 °C from Smyth and Hazen 1973, Am. Mineral. 58, 588 \
             (COD 9000319: Pbnm, a 4.756, b 10.207, c 5.980 Å, Z 4), 3.2191 g cm⁻³, the figure of \
             science-composition-crusts.md §10.1; +0.4/−0 per cent across cells (Hanafi et al. \
             2024; Kirfel et al. 2005; Takeuchi et al. 1984). The Handbook of Mineralogy's \
             D(meas.) 3.275 is a natural, iron-bearing olivine's. The crystal's, as enstatite's",
            Basis::Fact,
        ),
        source(
            "transition",
            "2,171 K (NIST-JANAF table Mg-030, Chase 1998)",
            Basis::Fact,
        ),
        SILICATE_FAMILY,
    ],
};

/// Silica, a hot giant's quartz cloud.
pub(in crate::substance) const SILICA: CondensedColumns = CondensedColumns {
    grain_density: solid(2_649.0, 298.0),
    transition: at(TransitionKind::Melting, 1_996.0),
    mechanics: MechanicsFamily::Silicate,
    sources: &[
        source(
            "grain density",
            "α-quartz's X-ray density at 298 K from Kihara 1990, Eur. J. Mineral. 2, 63 (COD \
             9005017: a 4.9137, c 5.4047 Å, Z 3), 2.6485 g cm⁻³, ±0.1 per cent across cells; \
             crystalline quartz, as WASP-17b's clouds (Grant et al. 2023, ApJL 956, L29). At a \
             cloud's temperatures the stable forms are β-quartz (2,537 at 848 K, Kihara 1990), \
             tridymite or cristobalite (2.27–2.33 g cm⁻³), so the room-temperature row overstates \
             a hot cloud's density by up to 15 per cent, recorded; fused silica is 2.20",
            Basis::Fact,
        ),
        source(
            "transition",
            "1,996 K, the start of NIST-JANAF's liquid SiO2, cristobalite's stable melting (Chase \
             1998, via the NIST WebBook); ICSC gives 1,713 °C and DOE PAC 1,722 °C through \
             PubChem",
            Basis::Fact,
        ),
        SILICATE_FAMILY,
    ],
};

/// Corundum, a hot giant's first condensate.
pub(in crate::substance) const CORUNDUM: CondensedColumns = CondensedColumns {
    grain_density: solid(3_989.0, 298.0),
    transition: at(TransitionKind::Melting, 2_327.0),
    mechanics: MechanicsFamily::Silicate,
    sources: &[
        source(
            "grain density",
            "corundum's X-ray density at 298 K from Schreyer et al. 2014, J. Appl. Cryst. 47, 659 \
             (COD 2300448: a 4.758152, c 12.98970 Å, Z 6), 3.9887 g cm⁻³, ±0.1 per cent (Lewis \
             et al. 1982, 3,984; Newnham and de Haan 1962, 3,987)",
            Basis::Fact,
        ),
        source(
            "transition",
            "2,327 K (NIST-JANAF table Al-101, Chase 1998); ICSC gives 2,054 °C through PubChem",
            Basis::Fact,
        ),
        SILICATE_FAMILY,
    ],
};

/// Perovskite, `CaTiO3`.
pub(in crate::substance) const PEROVSKITE: CondensedColumns = CondensedColumns {
    grain_density: solid(4_040.0, 298.0),
    transition: at(TransitionKind::Melting, 2_248.15),
    mechanics: MechanicsFamily::Silicate,
    sources: &[
        source(
            "grain density",
            "perovskite's X-ray density at 296–298 K from Ali and Yashima 2005, J. Solid State \
             Chem. 178, 2867 (COD 1567488: Pbnm, a 5.3789, b 5.4361, c 7.6388 Å, Z 4), 4,043, and \
             Knight 2009, Can. Mineral. 47, 381 (COD 9013383), 4,039 kg m⁻³; ±0.1 per cent across \
             four modern cells (Büttner and Maslen 1992's, the Handbook of Mineralogy's, gives \
             4,020)",
            Basis::Reduced,
        ),
        source(
            "transition",
            "1,975 °C, 2,248.15 K, Wikipedia's infobox for calcium titanate, which cites no source: \
             unverified, since the CaO–TiO2 phase diagram was not read and the value is in neither \
             NIST-JANAF nor PubChem's records. Ali and Yashima 2005 find the solid tetragonal at \
             1,598 K and cubic at 1,720 K, so it melts above those",
            Basis::Fact,
        ),
        SILICATE_FAMILY,
    ],
};

/// Rutile, `TiO2`.
pub(in crate::substance) const RUTILE: CondensedColumns = CondensedColumns {
    grain_density: solid(4_248.0, 293.0),
    transition: at(TransitionKind::Melting, 2_130.0),
    mechanics: MechanicsFamily::Silicate,
    sources: &[
        source(
            "grain density",
            "rutile's X-ray density from Howard, Sabine and Dickson 1991, Acta Cryst. B47, 462 \
             (COD 9015662: a 4.5937, c 2.9587 Å, Z 2, room temperature), 4.2482 g cm⁻³, ±0.1 \
             per cent (Baur and Khan 1971; Meagher and Lager 1979)",
            Basis::Fact,
        ),
        source(
            "transition",
            "2,130 K (NIST-JANAF table O-045, Chase 1998); DOE PAC gives 1,843 °C (2,116 K), and \
             HSDB and ICSC 1,855 °C, through PubChem",
            Basis::Fact,
        ),
        SILICATE_FAMILY,
    ],
};

/// The one-bar sublimation of graphite, kelvin, which `soot` shares.
const GRAPHITE_SUBLIMATION_K: f64 = 4_080.0;

/// Graphite, which sublimes at one bar.
pub(in crate::substance) const GRAPHITE: CondensedColumns = CondensedColumns {
    grain_density: solid(2_261.0, 293.0),
    transition: at(TransitionKind::Sublimation, GRAPHITE_SUBLIMATION_K),
    mechanics: MechanicsFamily::Silicate,
    sources: &[
        source(
            "grain density",
            "graphite-2H's X-ray density from Trucano and Chen 1975, Nature 258, 136 (COD \
             9011577: a 2.464, c 6.711 Å, Z 4), 2.2609 g cm⁻³, the Handbook of Mineralogy's \
             D(calc.) 2.26, ±0.3 per cent; natural graphite measures 2.09–2.23 with its pores",
            Basis::Fact,
        ),
        source(
            "transition",
            "4,080 K, where the summed vapour of C1–C5 over graphite reaches one bar, computed \
             from NIST-JANAF's log K_f (tables C-003, C-113, C-138, C-143 and C-148; Chase 1998): \
             4,079 K; TRC's boiling point in the NIST WebBook, 4,100 ± 100 K (Honig and Kramer \
             1969), agrees. Graphite does not melt at one bar",
            Basis::Reduced,
        ),
        source(
            "mechanics",
            "`Silicate`, as a crystalline mineral: no carbon family exists, and graphite is \
             soft (Mohs 1–2), which the silicate rows overstate",
            Basis::Adopted,
        ),
    ],
};

/// The material density of Titan's tholin, kg m⁻³, which the chromophore and the ice giants'
/// haze take as their stand-in.
const THOLIN_DENSITY_KG_M3: f64 = 1_350.0;

/// The onset of tholin's pyrolysis, kelvin, which the chromophore and the ice giants' haze take.
const THOLIN_DECOMPOSITION_K: f64 = 423.15;

/// Tholin's pyrolysis source.
const THOLIN_PYROLYSIS: Source = source(
    "transition",
    "150 °C, the onset of tholin's main pyrolysis stage under an inert gas, 150–575 °C, where its \
     endothermic decomposition begins, above which it carbonises (Nna-Mvondo et al. 2013, PSS 85, \
     279, abstract)",
    Basis::Fact,
);

/// Tholin, Titan's photochemical haze.
pub(in crate::substance) const THOLIN: CondensedColumns = CondensedColumns {
    grain_density: solid(THOLIN_DENSITY_KG_M3, 295.0),
    transition: at(TransitionKind::Decomposition, THOLIN_DECOMPOSITION_K),
    mechanics: MechanicsFamily::Organic,
    sources: &[
        source(
            "grain density",
            "the material density of cold-plasma Titan tholin, about 1.3–1.4 g cm⁻³ (Imanaka et \
             al. 2012, Icarus 218, 247, as Hörst and Tolbert 2013, ApJL 770, L10, and Yu et al. \
             2017, arXiv:1706.09986, report it): 1,350 ± 50 kg m⁻³. Hörst and Tolbert's in-situ \
             particle densities, 0.4–1.13 g cm⁻³, are porous particles' effective densities, a \
             form's",
            Basis::Reduced,
        ),
        THOLIN_PYROLYSIS,
        ORGANIC_FAMILY,
    ],
};

/// Soot, a hydrocarbon haze.
pub(in crate::substance) const SOOT: CondensedColumns = CondensedColumns {
    grain_density: solid(1_800.0, 295.0),
    transition: at(TransitionKind::Sublimation, GRAPHITE_SUBLIMATION_K),
    mechanics: MechanicsFamily::Organic,
    sources: &[
        source(
            "grain density",
            "black carbon's material density: pressed pellets 1.8–1.9 g cm⁻³ corrected for their \
             air (Medalia and Richards 1972; Janzen 1980), Park et al. 2004's 1.8 and Kondo et \
             al. 2011b's 1.718 ± 0.004 for fullerene soot, as Bond et al. 2013 review them (JGR \
             Atmos. 118, 5380, §3.7.2, para. 122): 1,800 ± 100 kg m⁻³",
            Basis::Reduced,
        ),
        source(
            "transition",
            "graphite's one-bar sublimation, the same constant, an assumption: in an anoxic gas \
             soot graphitises before it sublimes (in an oxidising air it burns lower, which is \
             chemistry, not the row's transition)",
            Basis::Adopted,
        ),
        ORGANIC_FAMILY,
    ],
};

/// The cold giants' red chromophore, which has no measured density.
pub(in crate::substance) const CHROMOPHORE: CondensedColumns = CondensedColumns {
    grain_density: solid(THOLIN_DENSITY_KG_M3, 295.0),
    transition: at(TransitionKind::Decomposition, THOLIN_DECOMPOSITION_K),
    mechanics: MechanicsFamily::Organic,
    sources: &[
        source(
            "grain density",
            "a stand-in, tholin's material density (Imanaka et al. 2012), the chromophore being a \
             nitrogen-bearing photochemical organic like tholin: Carlson et al. 2016 (Icarus 274, \
             106) made it by the UV photolysis of NH3 and C2H2 and measured only its film's \
             imaginary index, and Braude et al. 2020's 0.87 g cm⁻³ (Icarus 338, 113589) is \
             ammonia ice's, assumed for bookkeeping",
            Basis::Adopted,
        ),
        THOLIN_PYROLYSIS,
        ORGANIC_FAMILY,
    ],
};

/// The ice giants' haze, which has no measured density.
pub(in crate::substance) const ICE_GIANT_HAZE: CondensedColumns = CondensedColumns {
    grain_density: solid(THOLIN_DENSITY_KG_M3, 295.0),
    transition: at(TransitionKind::Decomposition, THOLIN_DECOMPOSITION_K),
    mechanics: MechanicsFamily::Organic,
    sources: &[
        source(
            "grain density",
            "a stand-in, tholin's material density (Imanaka et al. 2012), the haze being a \
             product of methane's photochemistry: Irwin et al. 2022 (JGR Planets 127, \
             e2022JE007189) state none, retrieving it by Mie at a real index of 1.4",
            Basis::Adopted,
        ),
        THOLIN_PYROLYSIS,
        ORGANIC_FAMILY,
    ],
};

/// The solidus of a basalt at about one atmosphere, 980 °C: `planetary::surface::reference`'s
/// `BASALT_SOLIDUS_K`, its sources there.
const BASALT_SOLIDUS_K: f64 = 1_253.15;

/// The solidus of an anhydrous, iron-rich basalt at about one atmosphere, 1,050 °C:
/// `planetary::surface::reference`'s `BASALT_FE_SOLIDUS_K`.
const BASALT_FE_SOLIDUS_K: f64 = 1_323.15;

/// The solidus of a plagioclase rock at one atmosphere, 1,260 °C: `planetary::surface::reference`'s
/// `PLAGIOCLASE_ROCK_SOLIDUS_K`.
const PLAGIOCLASE_ROCK_SOLIDUS_K: f64 = 1_533.15;

/// The solidus of an anhydrous chondritic silicate assemblage at about one atmosphere, 1,150 °C,
/// an upper bound: `planetary::surface::reference`'s `CHONDRITIC_SOLIDUS_K`.
const CHONDRITIC_SOLIDUS_K: f64 = 1_423.15;

/// The grain density of an iron-rich basalt, kg m⁻³: the shergottites' mean, which `ferric`
/// shares (decision-backlog-2 §1.2).
const BASALT_FE_GRAIN_DENSITY_KG_M3: f64 = 3_310.0;

/// The basalt solidus's source.
const BASALT_SOLIDUS: Source = source(
    "transition",
    "980 °C, a Hawaiian tholeiite's solidus at about one atmosphere, below which its residual \
     glass stops falling (Wright and Okamura 1977, USGS Professional Paper 1004, after Wright and \
     Weiblen 1967; ±10 °C), as `planetary::surface::reference`'s `BASALT_SOLIDUS_K` cites it",
    Basis::Fact,
);

/// The iron-rich basalt solidus's source.
const BASALT_FE_SOLIDUS: Source = source(
    "transition",
    "1,050 °C, where low-pressure experiments imply that solid mare basalts begin to melt, about \
     100 °C above a terrestrial basalt, \"chiefly because of the lack of H2O\" (Taylor et al. \
     1991, Lunar Sourcebook, ch. 6, p. 206), as `planetary::surface::reference`'s \
     `BASALT_FE_SOLIDUS_K` cites it",
    Basis::Fact,
);

/// The plagioclase rock solidus's source.
const PLAGIOCLASE_ROCK_SOLIDUS: Source = source(
    "transition",
    "1,260 °C, the reaction point anorthite + forsterite + clinoenstatite + liquid of the system \
     anorthite–forsterite–silica, \"1260° ± 2°\" (Andersen 1915, Am. J. Sci. s4-39, 407, p. 439), \
     where a plagioclase rock with a few per cent of low-calcium pyroxene first melts, as \
     `planetary::surface::reference`'s `PLAGIOCLASE_ROCK_SOLIDUS_K` cites it",
    Basis::Fact,
);

/// The chondritic solidus's source.
const CHONDRITIC_SOLIDUS: Source = source(
    "transition",
    "1,150 °C at about one atmosphere, an upper bound: the H and LL chondrites' \"~1150-1160°C\" \
     at IW−1 (Jurewicz et al., LPSC 24, 1993, pp. 739–740), whose charges lost their sodium; \
     anhydrous Murchison (CM) and Allende (CV) had more melt at 1,170–1,180 °C (Jurewicz, \
     Mittlefehldt and Jones 1993, GCA 57, 2123, as LPSC 24 reports it), so theirs lie at or below \
     it. A Ni-bearing metal's eutectic with troilite melts first, at 950 °C (1,261 K for Ni-free \
     Fe–FeS; Tomkins 2009 and Mare et al. 2014, as Gail and Trieloff 2019, A&A 628, A77, quote \
     them), recorded, not used. As `planetary::surface::reference`'s `CHONDRITIC_SOLIDUS_K`",
    Basis::Fact,
);

/// Mars-type dust: R09's reference row.
pub(in crate::substance) const MARS_DUST: CondensedColumns = CondensedColumns {
    grain_density: solid(2_600.0, 293.0),
    transition: at(TransitionKind::Solidus, BASALT_FE_SOLIDUS_K),
    mechanics: MechanicsFamily::Silicate,
    sources: &[
        source(
            "grain density",
            "2,600 kg m⁻³, the grain density that Viking Lander 1's fine material's 58 per cent \
             porosity assumes (Moore et al. 1987, USGS Professional Paper 1389, p. 121 and Table \
             36; an assumption there, labelled so), as `planetary::surface::reference`'s \
             `MARS_DUST`; no grain density of the dust was measured",
            Basis::Fact,
        ),
        source(
            "transition",
            "`basalt_fe`'s solidus, an assumption: the dust is iron-rich and basaltic, its \
             chlorides melting first and minor (science-composition-crusts.md §4), as \
             `planetary::surface::reference`'s `MARS_DUST`",
            Basis::Adopted,
        ),
        SILICATE_FAMILY,
    ],
};

/// An aqueous `NaCl` brine at its eutectic composition, 23.3 wt%.
pub(in crate::substance) const BRINE_NACL: CondensedColumns = CondensedColumns {
    grain_density: liquid(1_184.1, 273.15),
    transition: at(
        TransitionKind::Eutectic {
            solute_mass_fraction: 0.233,
        },
        251.9,
    ),
    mechanics: MechanicsFamily::WaterIce,
    sources: &[
        source(
            "grain density",
            "the liquid at the eutectic's 23.3 wt% NaCl and 0 °C: the International Critical \
             Tables, vol. III (1928), p. 79, d₄ᵗ 1.17318 at 22 and 1.18999 at 24 wt%, linear \
             between them and by the 1901 litre, at atmospheric pressure: 1,184.1 ± 0.3 kg m⁻³ \
             (Potter and Brown 1977, USGS Bulletin 1421-C, p. C5, find the tables within 0.3 kg \
             m⁻³ of their regression). At the \
             eutectic's temperature it would be about 1,191 ± 3 by a quadratic extrapolation of \
             the tables' 0, 10 and 20 °C, recorded",
            Basis::Reduced,
        ),
        source(
            "transition",
            "the NaCl–H2O eutectic, 251.9 K at 3.99 M, 23.3 per cent m/m (Tsironi et al. 2020, \
             arXiv:1904.03682, after Dickerson's Molecular Thermodynamics; 23.30 wt% in Zarriz, \
             Journaux and Powell-Palm 2024, arXiv:2403.01028, Supplementary Table 1); the plan's \
             251.95 K is Bodnar 1993's, not read. SC8.5 (2NaCl·17H2O) becomes the stable \
             hydrate at one bar only below about 235 K, under the eutectic, so it stands \
             (Journaux et al. 2023, PNAS 120, e2217125120)",
            Basis::Fact,
        ),
        source(
            "mechanics",
            "`WaterIce`, its frozen form being 62 wt% ice Ih with hydrohalite, NaCl·2H2O",
            Basis::Adopted,
        ),
    ],
};

/// An aqueous `MgSO4` brine at its eutectic composition, 17.30 wt%.
pub(in crate::substance) const BRINE_MGSO4: CondensedColumns = CondensedColumns {
    grain_density: liquid(1_193.3, 273.15),
    transition: at(
        TransitionKind::Eutectic {
            solute_mass_fraction: 0.173,
        },
        269.29,
    ),
    mechanics: MechanicsFamily::WaterIce,
    sources: &[
        source(
            "grain density",
            "the liquid at the eutectic's 17.30 wt% anhydrous MgSO4 and 0 °C: the International \
             Critical Tables, vol. III (1928), p. 72, d₄ᵗ 1.1777 at 16 and 1.2018 at 18 wt%, \
             linear between them and by the 1901 litre, at atmospheric pressure: 1,193.3 ± 0.5 kg \
             m⁻³; about 1,194.3 at the \
             eutectic's temperature by a quadratic extrapolation, recorded",
            Basis::Reduced,
        ),
        source(
            "transition",
            "the MgSO4–H2O eutectic at 0.1 MPa, 269.29 ± 0.10 K for the 17.30 wt% brine (Zarriz, \
             Journaux and Powell-Palm 2024, arXiv:2403.01028, Table 1; the composition their \
             Supplementary Table 1's, after Pillay et al. 2005, J. Chem. Eng. Data 50)",
            Basis::Fact,
        ),
        source(
            "mechanics",
            "`WaterIce`, narrowly, its frozen form being 54 wt% ice with 46 wt% meridianiite, \
             MgSO4·11H2O (`Salt` is defensible)",
            Basis::Adopted,
        ),
    ],
};

/// An aqueous sodium perchlorate brine at its eutectic composition, 52 wt%.
pub(in crate::substance) const BRINE_NACLO4: CondensedColumns = CondensedColumns {
    grain_density: liquid(1_445.0, 291.15),
    transition: at(
        TransitionKind::Eutectic {
            solute_mass_fraction: 0.52,
        },
        236.0,
    ),
    mechanics: MechanicsFamily::Salt,
    sources: &[
        source(
            "grain density",
            "the liquid at 52 wt% NaClO4 and 18 °C, extrapolated from the International Critical \
             Tables, vol. III (1928), p. 80 (d₄¹⁸, 1–38 wt%), with an apparent molar volume \
             linear in √m (43.54 + 2.20 √m cm³ mol⁻¹, residual at most 0.12 over 10–38 wt%; \
             linear 1,442, quadratic 1,449): 1,445 ± 10 kg m⁻³ at atmospheric pressure. Not \
             corrected to the eutectic, 55 K colder, where it is denser by an amount not \
             measured",
            Basis::Reduced,
        ),
        source(
            "transition",
            "Chevrier, Hanley and Altheide 2009 (GRL 36, L10202, abstract): \"theoretical eutectic \
             values … 236 ± 1 K for 52 wt% sodium perchlorate\"; a measured eutectic, −34.1 °C \
             (239.05 K; Hennings, Heinz, Schmidt and Voigt 2013, Z. Anorg. Allg. Chem. 639, 922, \
             as Bravenec and Catling 2023 quote it), recorded",
            Basis::Fact,
        ),
        source(
            "mechanics",
            "`Salt`, its frozen form being 67 wt% NaClO4·2H2O with ice",
            Basis::Adopted,
        ),
    ],
};

/// An aqueous magnesium perchlorate brine at its eutectic composition, 44.0 wt%.
pub(in crate::substance) const BRINE_MGCLO4: CondensedColumns = CondensedColumns {
    grain_density: liquid(1_394.0, 298.15),
    transition: at(
        TransitionKind::Eutectic {
            solute_mass_fraction: 0.44,
        },
        206.0,
    ),
    mechanics: MechanicsFamily::Salt,
    sources: &[
        source(
            "grain density",
            "the liquid at 44.0 wt% Mg(ClO4)2 and 25 °C, extrapolated from Lumme 1970's highest \
             measurement (Acta Chem. Scand. 24, 1727, Table 1, pycnometer: 1.2876 g cm⁻³ at \
             1.9985 mol L⁻¹, 34.6 wt%) with an apparent molar volume 62.8 + 10.3 √c cm³ mol⁻¹ \
             fitted over 0.4–2 mol L⁻¹ (1,391), or held at its 2 mol L⁻¹ value, 78.0 (1,398): \
             1,394 ± 15 kg m⁻³ at atmospheric pressure. Not corrected to the eutectic, 92 K colder",
            Basis::Reduced,
        ),
        source(
            "transition",
            "Chevrier, Hanley and Altheide 2009 (GRL 36, L10202, abstract): \"theoretical eutectic \
             values … 206 ± 1 K for 44.0 wt% magnesium perchlorate\"; the literature spans \
             205–216 K (Nair and Unnikrishnan 2020, ACS Omega; 212 K, Möhlmann and Thomsen 2011, \
             as Pál and Kereszturi 2020 quote it), so ±1 K understates the spread",
            Basis::Fact,
        ),
        source(
            "mechanics",
            "`Salt`, its frozen form being 65 wt% Mg(ClO4)2·6H2O with ice",
            Basis::Adopted,
        ),
    ],
};

/// The ammonia–water system: the ammonia-rich melt at its eutectic.
pub(in crate::substance) const AMMONIA_WATER: CondensedColumns = CondensedColumns {
    grain_density: liquid(946.0, 176.15),
    transition: at(
        TransitionKind::Eutectic {
            solute_mass_fraction: 0.321,
        },
        176.15,
    ),
    mechanics: MechanicsFamily::VolatileIce,
    sources: &[
        source(
            "grain density",
            "\"At the eutectic temperature, the density of the ammonia-rich melt is 0.946 g/cm3\" \
             (Hammond, Parmentier and Barr 2018, arXiv:1811.11257, §1, after Croft, Lunine and \
             Kargel 1988, Icarus 73, 279), ±11 kg m⁻³: Hammond et al.'s own quadratic fit to \
             Croft et al.'s data (their eq. 12) gives 957 there; its pressure is not stated",
            Basis::Fact,
        ),
        source(
            "transition",
            "the eutectic, 176.15 K (Hammond et al. 2018, §2, after Kargel 1992 and \
             Leliwa-Kopystyński et al. 2002; 176 K at one bar, Chua et al. 2023, ACS Earth Space \
             Chem.); its composition, 0.321, is ammonia dihydrate's stoichiometry (computed), an \
             assumption: the ice–dihydrate eutectic's own composition was not read (Kargel 1992 \
             not reached)",
            Basis::Adopted,
        ),
        source(
            "mechanics",
            "`VolatileIce`, its frozen form at the eutectic being nearly all ammonia dihydrate \
             (`WaterIce` is defensible)",
            Basis::Adopted,
        ),
    ],
};

/// Titan's seas' liquid at Ligeia Mare's composition and Titan's surface temperature.
pub(in crate::substance) const TITAN_LIQUID: CondensedColumns = CondensedColumns {
    grain_density: liquid(518.9, 93.65),
    transition: at(
        TransitionKind::Eutectic {
            solute_mass_fraction: 0.509,
        },
        71.15,
    ),
    mechanics: MechanicsFamily::VolatileIce,
    sources: &[
        source(
            "grain density",
            "the components' saturated liquids by ideal mixing (volume additivity, ρ = Σ φᵢ ρᵢ) at \
             the reference composition, Ligeia Mare's best fit to its measured loss tangent, by \
             volume \"71% CH4, 12% C2H6, and 17% N2\" (Mastrogiuseppe, Hayes, Poggiali, Seu, \
             Lunine and Hofgartner 2016, IEEE Trans. Geosci. Remote Sens. 54, 5646, §V.B; \
             decision-r11-palette-liquids §1.8 gives these figures as Le Gall et al. 2016's, which \
             states no such composition), at Titan's surface temperature, 93.65 K (Huygens's \
             HASI, Fulchignoni et al. 2005, Nature 438, 785): the rows' own ancillary densities \
             there, each under its own vapour's pressure, methane's 447.53 (Setzmann and Wagner \
             1991), ethane's 647.97 (CoolProp's fit to Bücker and Wagner 2006) and nitrogen's \
             725.71 kg m⁻³ (Span et al. 2000), give 518.9 kg m⁻³. Ideal mixing errs: methane and \
             ethane mix with a negative excess volume (Engle et al. 2021), and nitrogen at \
             93.65 K is a liquid only in solution. Tan, Kargel and Marion 2013's cross-check not \
             read",
            Basis::Reduced,
        ),
        source(
            "transition",
            "the CH4–C2H6 eutectic, 71.15 ± 0.5 K at 0.644 ± 0.018 CH4 by mole from the liquidus \
             (Engle, Hanley, Dustrud, Thompson, Lindberg, Grundy and Tegler 2021, PSJ 2, 118), \
             50.9 wt% ethane in the nitrogen-free binary; the ternary's liquidus is not mapped \
             (Steckloff et al. 2020, PSJ 1, 26), so 71.15 K bounds it above if nitrogen lowers it",
            Basis::Fact,
        ),
        source(
            "mechanics",
            "`VolatileIce`, its frozen form being methane and ethane ices",
            Basis::Adopted,
        ),
    ],
};

/// Anorthosite, a plagioclase rock of about 95 per cent plagioclase.
pub(in crate::substance) const ANORTHOSITE: CondensedColumns = CondensedColumns {
    grain_density: solid(2_790.0, 293.0),
    transition: at(TransitionKind::Solidus, PLAGIOCLASE_ROCK_SOLIDUS_K),
    mechanics: MechanicsFamily::Silicate,
    sources: &[
        source(
            "grain density",
            "95 per cent An95 plagioclase (2.753 g cm⁻³ from the Handbook of Mineralogy's \
             anorthite and albite) with 5 per cent low-calcium pyroxene of about 3.5 \
             (science-composition-crusts.md §7.2): 2,790 kg m⁻³, inside 2,776–2,828 at 97–90 per \
             cent plagioclase (§7.1). Apollo 60025, over 98 per cent An96 plagioclase, measures \
             2,709–2,751 (+1–2 per cent) by toluene (Jeanloz and Ahrens 1978, Proc. LPSC 9, 2789, \
             p. 2793): the pure end of the rock's range",
            Basis::Reduced,
        ),
        PLAGIOCLASE_ROCK_SOLIDUS,
        SILICATE_FAMILY,
    ],
};

/// A feldspathic crust: R09's reference row.
pub(in crate::substance) const FELDSPATHIC: CondensedColumns = CondensedColumns {
    grain_density: solid(2_900.0, 293.0),
    transition: at(TransitionKind::Solidus, PLAGIOCLASE_ROCK_SOLIDUS_K),
    mechanics: MechanicsFamily::Silicate,
    sources: &[
        source(
            "grain density",
            "2,900 kg m⁻³, implied by the lunar highlands' GRAIL bulk density, 2,550, at 12 per \
             cent porosity (Wieczorek et al. 2013, Science 339, 671), inside the four feldspathic \
             highland rocks' 2,840–2,910 by helium pycnometry (Kiefer et al. 2012, GRL 39, L07201, \
             Table 1), as `planetary::surface::reference`'s `FELDSPATHIC`",
            Basis::Reduced,
        ),
        PLAGIOCLASE_ROCK_SOLIDUS,
        SILICATE_FAMILY,
    ],
};

/// Basalt, a terrestrial tholeiite: R09's reference row.
pub(in crate::substance) const BASALT: CondensedColumns = CondensedColumns {
    grain_density: solid(3_010.0, 293.0),
    transition: at(TransitionKind::Solidus, BASALT_SOLIDUS_K),
    mechanics: MechanicsFamily::Silicate,
    sources: &[
        source(
            "grain density",
            "3,010 kg m⁻³, the mean of eight Alae lava lake tholeiites, 2,920–3,040 (Peck 1978, \
             USGS Professional Paper 935-B, p. B20; science-r09-drainage.md §3), as \
             `planetary::surface::reference`'s `BASALT`",
            Basis::Reduced,
        ),
        BASALT_SOLIDUS,
        SILICATE_FAMILY,
    ],
};

/// An iron-rich, anhydrous basalt: R09's reference row.
pub(in crate::substance) const BASALT_FE: CondensedColumns = CondensedColumns {
    grain_density: solid(BASALT_FE_GRAIN_DENSITY_KG_M3, 293.0),
    transition: at(TransitionKind::Solidus, BASALT_FE_SOLIDUS_K),
    mechanics: MechanicsFamily::Silicate,
    sources: &[
        source(
            "grain density",
            "3,310 kg m⁻³, the shergottites' mean, 3.31 g cm⁻³ (Macke, Britt and Consolmagno \
             2011, MAPS 46, 311, abstract), inside the lunar basalts' 3,270–3,460 (Kiefer et al. \
             2012, GRL 39, L07201, Table 1), as `planetary::surface::reference`'s `BASALT_FE`",
            Basis::Fact,
        ),
        BASALT_FE_SOLIDUS,
        SILICATE_FAMILY,
    ],
};

/// Komatiite, an ultramafic lava.
pub(in crate::substance) const KOMATIITE: CondensedColumns = CondensedColumns {
    grain_density: solid(3_240.0, 295.0),
    transition: at(TransitionKind::Solidus, 1_428.15),
    mechanics: MechanicsFamily::Silicate,
    sources: &[
        source(
            "grain density",
            "the mode of the spinifex-textured peridotitic komatiite SA 3091, Munro Township, by \
             volume: skeletal olivine 40, subcalcic augite 45 and devitrified glass 15 per cent \
             (Arndt 1976, Carnegie Inst. Washington Year Book 75, 555, p. 555), with olivine \
             Fo90–92 by ideal molar-volume mixing of forsterite's 3.219 and fayalite's 4.408 g \
             cm⁻³ (COD 9000319 and 9000469), augite 3.31 and diopside 3.278 (Handbook of \
             Mineralogy) and the glass at Alae's 2.76 (Peck 1978, USGS Professional Paper 935-B, \
             p. B37) raised to 2.80 for a more mafic residue: 3,240 ± 40 kg m⁻³, of its fresh \
             anhydrous rock (Fo94 3,207 to Fo85 3,288). The olivine's Fo90–92 and the glass's \
             2.80 are assumptions",
            Basis::Reduced,
        ),
        source(
            "transition",
            "1,155 °C: SA 3091 at one atmosphere holds plagioclase and liquid down to about \
             1,160 °C and is below its solidus at about 1,150 °C (Arndt 1976, Fig. 27, p. 558, \
             read off the figure, ±10 °C), its residual liquid's late plagioclase and pyroxene \
             setting it near a basalt's; Kinzler and Grove 1985 (Am. Mineral. 70, 40) find glass \
             at 1,173 °C (Table 1)",
            Basis::Fact,
        ),
        SILICATE_FAMILY,
    ],
};

/// A reduced, low-FeO basalt, Mercury's lid.
pub(in crate::substance) const BASALT_REDUCED: CondensedColumns = CondensedColumns {
    grain_density: solid(2_970.0, 295.0),
    transition: at(TransitionKind::Solidus, 1_473.15),
    mechanics: MechanicsFamily::Silicate,
    sources: &[
        source(
            "grain density",
            "Mercury's surface grain density from the normative mineralogy of MESSENGER's \
             elemental abundances, \"average 2974 ± 89 kg/m3\", \"between approximately \
             2900–3000 kg/m3, depending on assumptions\" (Sori 2018, EPSL 489, 92, abstract and \
             conclusions): 2,970 ± 90 kg m⁻³",
            Basis::Fact,
        ),
        source(
            "transition",
            "at or below 1,200 °C, an upper bound: in one-atmosphere runs on Mercury's high-Mg \
             and low-Mg lavas plagioclase joins at 1,200 °C, the melt near the solidus not \
             measured (Charlier, Grove and Zuber 2013, EPSL 363, 50, §4, p. 55); a natural, \
             sodium- and sulphide-bearing lid likely begins lower",
            Basis::Fact,
        ),
        SILICATE_FAMILY,
    ],
};

/// Granite, a tertiary crust: R09's reference row.
pub(in crate::substance) const GRANITE: CondensedColumns = CondensedColumns {
    grain_density: solid(2_650.0, 293.0),
    transition: at(TransitionKind::Solidus, 1_233.15),
    mechanics: MechanicsFamily::Silicate,
    sources: &[
        source(
            "grain density",
            "2,650 kg m⁻³, 2,654 from the measured mode of the USGS standard granite G-1 (Chayes \
             in Fairbairn et al. 1951, USGS Bulletin 980, Table 26) with the Handbook of \
             Mineralogy's densities, 2,635–2,671 across their ranges, as \
             `planetary::surface::reference`'s `GRANITE`",
            Basis::Reduced,
        ),
        source(
            "transition",
            "a dry solidus at one atmosphere of about 960 °C, `planetary::surface::reference`'s \
             `GRANITE`, unverified there (from memory, low confidence) and here: below the \
             albite–quartz join's invariant point, 1,062 ± 3 °C (Schairer and Bowen 1956, AJS \
             254, 129, Table 3, p. 160), an upper bound, which potassium feldspar lowers; Huang \
             and Wyllie 1975's one-atmosphere end not read",
            Basis::Adopted,
        ),
        SILICATE_FAMILY,
    ],
};

/// An ultramafic rock, a harzburgite: an exposed mantle.
pub(in crate::substance) const ULTRAMAFIC: CondensedColumns = CondensedColumns {
    grain_density: solid(3_320.0, 295.0),
    transition: at(TransitionKind::Solidus, 1_393.81),
    mechanics: MechanicsFamily::Silicate,
    sources: &[
        source(
            "grain density",
            "Hu, Ehlmann and Seager 2012's ultramafic type, \"60% Olivine, 40% Enstatite\" (ApJ \
             752, 7, Table 2), with olivine and orthopyroxene of Mg# 90 (an assumption) by ideal \
             molar-volume mixing of forsterite's 3.219, fayalite's 4.408, orthoenstatite's 3.202 \
             and orthoferrosilite's 4.003 g cm⁻³ (science-composition-crusts.md §10.1): olivine \
             3.344 and orthopyroxene 3.286, so 3,320 ± 55 kg m⁻³ by mass or volume (Mg# 95 \
             3,267 to Mg# 85 3,374)",
            Basis::Reduced,
        ),
        source(
            "transition",
            "1,120.66 °C, the nominally dry peridotite solidus T(°C) = −5.104 P² + 132.899 P + \
             1,120.661 at P = 0 GPa (Hirschmann 2000, G³ 1, 2000GC000070, abstract), a fertile \
             peridotite's, whose alkalis set it: a lower bound for a depleted harzburgite",
            Basis::Fact,
        ),
        SILICATE_FAMILY,
    ],
};

/// An Fe-oxidised (ferric) rock: `basalt_fe` oxidised at the surface (decision-backlog-2 §1).
///
/// Its grain density is `basalt_fe`'s 3,310 kg m⁻³, the same constant, recorded +120/−240. The
/// measured ferric phases are a few weight per cent: Rocknest's soil holds hematite 1.0(1) and
/// magnetite 1.8(3) wt% with an X-ray-amorphous component of 35(15) wt% (Achilles et al. 2017,
/// JGR Planets 122, 2344, Table 1), whose nanophase ferric oxide is 6.8 wt% of the bulk (Morris
/// et al. 2013, LPSC 44, 1653, Table 3); Meridiani's basaltic soils hold 18 per cent of their iron
/// in nanophase oxide, 6 in magnetite and 4 in hematite (Morris et al. 2006, JGR 111, E12S15).
/// With hematite's 5.26 g cm⁻³ (Handbook of Mineralogy), magnetite's 5.175 and ferrihydrite's
/// 3.5–3.9 (Hiemstra and Van Riemsdijk 2009, GCA 73, 4423) by volume additivity, they raise the
/// shergottites' 3,310 (Macke, Britt and Consolmagno 2011) by at most about 100 kg m⁻³, while the
/// unmeasured amorphous component may lower it as much or more: 3,070–3,430. Hu, Ehlmann and
/// Seager 2012's 50 per cent nanophase hematite, a single-scattering-albedo weight with no stated
/// basis, would give 4,063–4,285 and is refused.
pub(in crate::substance) const FERRIC: CondensedColumns = CondensedColumns {
    grain_density: solid(BASALT_FE_GRAIN_DENSITY_KG_M3, 293.0),
    transition: at(TransitionKind::Solidus, BASALT_FE_SOLIDUS_K),
    mechanics: MechanicsFamily::Silicate,
    sources: &[
        source(
            "grain density",
            "`basalt_fe`'s 3,310 kg m⁻³ at room temperature, the same constant, recorded \
             +120/−240 (decision-backlog-2 §1.2): the measured ferric phases, hematite 1.0(1) and \
             magnetite 1.8(3) wt% with a 35(15) wt% amorphous component (Achilles et al. 2017, JGR \
             Planets 122, 2344, Table 1), nanophase ferric oxide 6.8 wt% (Morris et al. 2013, \
             LPSC 44, 1653, Table 3) and Meridiani's 18, 6 and 4 per cent of the iron (Morris et \
             al. 2006, JGR 111, E12S15), at hematite's 5.26, magnetite's 5.175 (Handbook of \
             Mineralogy) and ferrihydrite's 3.5–3.9 g cm⁻³ (Hiemstra and Van Riemsdijk 2009, GCA \
             73, 4423), move the shergottites' 3,310 (Macke, Britt and Consolmagno 2011) by at \
             most about +100, and the amorphous component as much or more down. Hu, Ehlmann and \
             Seager 2012's 50 per cent hematite, a spectral weight, is refused",
            Basis::Reduced,
        ),
        source(
            "transition",
            "`basalt_fe`'s solidus, an assumption: the basalt melts first, hematite being \
             refractory",
            Basis::Adopted,
        ),
        SILICATE_FAMILY,
    ],
};

/// A phyllosilicate-rich rock: R09's reference row.
pub(in crate::substance) const PHYLLOSILICATE: CondensedColumns = CondensedColumns {
    grain_density: solid(2_500.0, 293.0),
    transition: at(TransitionKind::Solidus, CHONDRITIC_SOLIDUS_K),
    mechanics: MechanicsFamily::Silicate,
    sources: &[
        source(
            "grain density",
            "2,500 kg m⁻³, between saponite's 2,240–2,300 and lizardite's 2,550 (Anthony et al., \
             Handbook of Mineralogy), as `planetary::surface::reference`'s `PHYLLOSILICATE`",
            Basis::Reduced,
        ),
        source(
            "transition",
            "the chondritic solidus, its dehydrated residue's, as \
             `planetary::surface::reference`'s `PHYLLOSILICATE` takes it",
            Basis::Adopted,
        ),
        CHONDRITIC_SOLIDUS,
        SILICATE_FAMILY,
    ],
};

/// Quenched lava glass, a lava world's dark surface.
pub(in crate::substance) const LAVA_GLASS: CondensedColumns = CondensedColumns {
    grain_density: solid(2_760.0, 295.0),
    transition: at(TransitionKind::Solidus, BASALT_SOLIDUS_K),
    mechanics: MechanicsFamily::Silicate,
    sources: &[
        source(
            "grain density",
            "\"The measured density at room temperature of glass separated from two samples of \
             pumice formed during the Alae eruption … is 2.76 g/cc\", and vesicle- and \
             crystal-free Makaopuhi glass 2.78 (Peck 1978, USGS Professional Paper 935-B, p. \
             B37): 2,760 ± 20 kg m⁻³, 8 per cent below the basalt it crystallises to",
            Basis::Fact,
        ),
        source(
            "transition",
            "the basalt's solidus, where it becomes melt; above its glass transition the glass \
             relaxes and devitrifies but does not flow on a surface's timescale",
            Basis::Adopted,
        ),
        BASALT_SOLIDUS,
        SILICATE_FAMILY,
    ],
};

/// A Ca–Al refractory residue, a lava world's lag.
pub(in crate::substance) const CA_AL_RESIDUE: CondensedColumns = CondensedColumns {
    grain_density: solid(3_810.0, 295.0),
    transition: at(TransitionKind::Solidus, 2_150.0),
    mechanics: MechanicsFamily::Silicate,
    sources: &[
        source(
            "grain density",
            "Léger et al. 2011's residue, \"13% for CaO and 87% for Al2O3 at 2200 K (close to the \
             composition CaO.6Al2O3)\" (Icarus 213, 1, §6.1), crystallised as hibonite with 9.56 \
             wt% corundum by mass: hibonite 3.796 g cm⁻³ from COD 9004447's cell, a natural \
             (Ce, La, Fe, Mg, Ti)-bearing hibonite's (Bermanec et al. 1996, V 608.042 Å³, Z 2), \
             standing in for pure CaAl12O19, whose cell was not read, and corundum 3.997 \
             (Handbook of Mineralogy): 3,810 ± 50 kg m⁻³ (3,855 with the Handbook's natural \
             hibonite, 3.84). The assemblage is an inference from the composition",
            Basis::Reduced,
        ),
        source(
            "transition",
            "2,150 K: \"The lowest possible temperature of the ocean is that of the fusion of its \
             material at zero pressure: 2150 K\" (Léger et al. 2011, §5), taken as its first \
             melt, though the paper also calls it a liquidus, and gives the liquidus as 2,200 ± \
             20 K (§6.2). Kite et al. 2016 (ApJ 828, 80) put a CaO-richer lag's lock-up near \
             1,673 K, recorded",
            Basis::Fact,
        ),
        SILICATE_FAMILY,
    ],
};

/// Fe–Ni metal, kamacite: an exposed core.
pub(in crate::substance) const IRON_NICKEL: CondensedColumns = CondensedColumns {
    grain_density: solid(7_900.0, 295.0),
    transition: at(TransitionKind::Solidus, 1_811.15),
    mechanics: MechanicsFamily::Metal,
    sources: &[
        source(
            "grain density",
            "bcc Fe–Ni's X-ray density from α-iron's cell at one bar, a 2.866 Å, Z 2 (COD \
             9000657, Wilburn and Bassett 1978, Am. Mineral. 63, 591), with kamacite's 5.43 and \
             6.69 wt% Ni (the Handbook of Mineralogy's analyses): 7,899–7,904 kg m⁻³, the \
             Handbook's kamacite D(calc.) 7.90; ±20 (taenite measures 7.8–8.22)",
            Basis::Reduced,
        ),
        source(
            "transition",
            "at most 1,811.15 K, pure iron's melting point (1,538 °C, HSDB and DOE PAC through \
             PubChem), an upper bound: kamacite's nickel lowers the alloy's solidus by an amount \
             not read (Swartzendruber, Itkin and Alcock 1991, J. Phase Equilib. 12, 288, not \
             reached), and sulphur or phosphorus far more",
            Basis::Adopted,
        ),
        METAL_FAMILY,
    ],
};

/// Silicon carbide, moissanite.
pub(in crate::substance) const SILICON_CARBIDE: CondensedColumns = CondensedColumns {
    grain_density: solid(3_213.0, 293.0),
    transition: at(TransitionKind::Decomposition, 3_103.15),
    mechanics: MechanicsFamily::Silicate,
    sources: &[
        source(
            "grain density",
            "moissanite-6H's X-ray density from Capitani, Di Pierro and Tempesta 2007, Am. \
             Mineral. 92, 403 (COD 9010158: a 3.0810, c 15.1248 Å, Z 6), 3.2129 g cm⁻³, ±0.1 per \
             cent; HSDB and NIOSH give 3.23 through PubChem",
            Basis::Fact,
        ),
        source(
            "transition",
            "2,830 °C, 3,103.15 K, which DOE PAC gives through PubChem as a melting point (HSDB \
             gives 2,600 °C, and NIOSH that it sublimes at 2,700 °C); its kind is a \
             decomposition, an assumption, the peritectic to graphite and a silicon-rich liquid \
             being from memory, not read. Its vapour over SiC and graphite reaches one bar at \
             3,242 K by NIST-JANAF's tables (C-100, Si-005, Si-008, Si-009, C-103 and C-137; \
             Chase 1998), recorded",
            Basis::Fact,
        ),
        source(
            "mechanics",
            "`Silicate`: a hard, brittle mineral",
            Basis::Adopted,
        ),
    ],
};

/// Anhydrous magnesium sulphate.
pub(in crate::substance) const MAGNESIUM_SULPHATE: CondensedColumns = CondensedColumns {
    grain_density: solid(3_020.0, 300.0),
    transition: at(TransitionKind::Decomposition, 1_272.0),
    mechanics: MechanicsFamily::Salt,
    sources: &[
        source(
            "grain density",
            "α-MgSO4's X-ray density at 300 K from Fortes, Wood, Vočadlo, Brand and Knight 2007, \
             J. Appl. Cryst. 40, 761 (COD 2300128: Cmcm, a 5.174713, b 7.87563, c 6.49517 Å, Z \
             4), 3.0202 g cm⁻³, ±0.3 per cent (β-MgSO4 2,925–2,934); the handbooks' 2.66 is no \
             X-ray density and is not used. The anhydrous salt: icy and Martian surfaces hold \
             hydrates, which this formula row does not represent",
            Basis::Fact,
        ),
        source(
            "transition",
            "1,272 K, where MgSO4's decomposition to MgO with SO3, SO2 and O2 reaches one bar in \
             all, computed from NIST-JANAF's tables Mg-018, Mg-008, O-058 and O-034 (Chase \
             1998), in an open system; JANAF's MgSO4 melts at 1,400 K (table Mg-020), which a \
             dense SO2-bearing air approaches",
            Basis::Reduced,
        ),
        SALT_FAMILY,
    ],
};

/// Sodium carbonate: R09's reference row.
pub(in crate::substance) const SODIUM_CARBONATE: CondensedColumns = CondensedColumns {
    grain_density: solid(2_540.0, 295.0),
    transition: at(TransitionKind::Melting, 1_123.15),
    mechanics: MechanicsFamily::Salt,
    sources: &[
        source(
            "grain density",
            "2,540 kg m⁻³: γ-Na2CO3's X-ray density at 295 K, 2,537, from Dusek et al. 2003's \
             cell (Acta Cryst. B59, 337; COD 2103920), and natrite's, 2,547 (Zubkova et al. \
             2002; the Handbook of Mineralogy's D(meas.) 2.54); 2,575 at 110 K, recorded; as \
             `planetary::surface::reference`'s `SODIUM_CARBONATE`",
            Basis::Reduced,
        ),
        source(
            "transition",
            "850 °C, where NIST-JANAF's solid ends and its liquid begins (table C-090, Chase \
             1998), as `planetary::surface::reference`'s `SODIUM_CARBONATE`",
            Basis::Fact,
        ),
        SALT_FAMILY,
    ],
};

/// Calcite.
pub(in crate::substance) const CALCITE: CondensedColumns = CondensedColumns {
    grain_density: solid(2_710.0, 293.0),
    transition: at(TransitionKind::Decomposition, 1_161.0),
    mechanics: MechanicsFamily::Salt,
    sources: &[
        source(
            "grain density",
            "calcite's X-ray density from Graf 1961, Am. Mineral. 46, 1283 (COD 9000095: a \
             4.9900, c 17.0615 Å, Z 6), 2.7103 g cm⁻³, the Handbook of Mineralogy's D(meas.) \
             2.7102(2)",
            Basis::Fact,
        ),
        source(
            "transition",
            "1,161 K, where CaCO3 → CaO + CO2 at one bar of CO2 has ΔG_r = 0, interpolated \
             between +8.8 kJ at 1,100 K and −5.6 kJ at 1,200 K in Robie and Hemingway 1995's \
             tables (USGS Bulletin 2131), at one bar of CO2; under a dense CO2 air it dissociates \
             higher",
            Basis::Reduced,
        ),
        SALT_FAMILY,
    ],
};

/// The onset of carbonaceous matter's decomposition, kelvin, which `cometary`'s organics share.
const CARBONACEOUS_DECOMPOSITION_K: f64 = 673.15;

/// Carbonaceous matter: refractory organics.
pub(in crate::substance) const CARBONACEOUS: CondensedColumns = CondensedColumns {
    grain_density: solid(1_400.0, 293.0),
    transition: at(TransitionKind::Decomposition, CARBONACEOUS_DECOMPOSITION_K),
    mechanics: MechanicsFamily::Organic,
    sources: &[
        source(
            "grain density",
            "\"In the absence of data for IOM and COM, density is estimated to be similar to that \
             of terrestrial coals of 1400 ± 200 kg/m3\" (Reynard and Sotin 2023, EPSL 612, \
             118172, §2); coal analogues of carbonaceous matter about 1,350 ± 100 (Delarue et al. \
             2026, EPSL 691, 120227, abstract) and type II kerogen 1.18–1.35 g cm⁻³ with \
             maturity (Okiongbo, Aplin and Larter 2005, Energy Fuels 19, 2495, abstract): no \
             meteoritic IOM was measured",
            Basis::Fact,
        ),
        source(
            "transition",
            "673 K: \"Density of coals is constant up to heating temperatures of 673 K \
             (Franklin, 1949)\", and IOM loses about 40 per cent of its mass between 700 and \
             1,200 K (Okumura and Mimura 2011), both as Reynard and Sotin 2023 quote them; \
             Murchison's IOM releases its main pyrolysates at 450–550 °C (Mimura, Okumura and \
             Harada 2020, Geochem. J. 54, 255, abstract). Heated, it becomes graphite-like, not \
             a melt",
            Basis::Fact,
        ),
        ORGANIC_FAMILY,
    ],
};

/// A CI chondrite.
pub(in crate::substance) const CHONDRITE_CI: CondensedColumns = CondensedColumns {
    grain_density: solid(2_590.0, 293.0),
    transition: at(TransitionKind::Solidus, CHONDRITIC_SOLIDUS_K),
    mechanics: MechanicsFamily::Silicate,
    sources: &[
        source(
            "grain density",
            "Ryugu's particles, 2,594 ± 32 kg m⁻³ (Nakamura E. et al. 2022, Proc. Jpn. Acad. B \
             98, 227), as the composition ruling read it: 2,590; the group spans Orgueil's one \
             stone, 2.42 ± 0.06 g cm⁻³ and altered on Earth (Macke 2010, Table 13), to Bennu's \
             2,800 ± 100",
            Basis::Reduced,
        ),
        CHONDRITIC_SOLIDUS,
        SILICATE_FAMILY,
    ],
};

/// A CM chondrite.
pub(in crate::substance) const CHONDRITE_CM: CondensedColumns = CondensedColumns {
    grain_density: solid(2_920.0, 293.0),
    transition: at(TransitionKind::Solidus, CHONDRITIC_SOLIDUS_K),
    mechanics: MechanicsFamily::Silicate,
    sources: &[
        source(
            "grain density",
            "the CM chondrites' mean by meteorite, 2.92 ± 0.04 g cm⁻³ (13 meteorites, s.d. 0.14, \
             2.74–3.26; 2.96 by stone), from Macke 2010, Table 22, pp. 288–289, the data of \
             Macke, Consolmagno and Britt 2011 (MAPS 46, 1842) and Macke et al. 2010 (MAPS 45, \
             1513)",
            Basis::Fact,
        ),
        CHONDRITIC_SOLIDUS,
        SILICATE_FAMILY,
    ],
};

/// A CV chondrite, standing for CV, CO and CK.
pub(in crate::substance) const CHONDRITE_CV: CondensedColumns = CondensedColumns {
    grain_density: solid(3_540.0, 293.0),
    transition: at(TransitionKind::Solidus, CHONDRITIC_SOLIDUS_K),
    mechanics: MechanicsFamily::Silicate,
    sources: &[
        source(
            "grain density",
            "the CV chondrites' mean by meteorite, 3.54 ± 0.04 g cm⁻³ (12 meteorites, s.d. 0.12; \
             3.61 by stone, Allende's 52 stones weighting it), the CV, CO and CK groups pooled \
             3,530 ± 150 (28), from Macke 2010, Table 22, pp. 288–289, the data of Macke, \
             Consolmagno and Britt 2011 (MAPS 46, 1842); the plan's 3,420 was from memory, and \
             no CV, CO or CK mean is as low",
            Basis::Fact,
        ),
        CHONDRITIC_SOLIDUS,
        SILICATE_FAMILY,
    ],
};

/// An H chondrite.
pub(in crate::substance) const CHONDRITE_H: CondensedColumns = CondensedColumns {
    grain_density: solid(3_720.0, 293.0),
    transition: at(TransitionKind::Solidus, CHONDRITIC_SOLIDUS_K),
    mechanics: MechanicsFamily::Silicate,
    sources: &[
        source(
            "grain density",
            "the H chondrite falls' mean, 3.72 g cm⁻³, s.d. 0.12 (Consolmagno, Britt and Macke \
             2008, Chem. Erde 68, 1, as Kohout et al. 2014, arXiv:1404.1245, quote it); H falls \
             3.71 ± 0.01 by stone and 3.70 by meteorite in Macke 2010, Table 22",
            Basis::Fact,
        ),
        CHONDRITIC_SOLIDUS,
        SILICATE_FAMILY,
    ],
};

/// An L chondrite.
pub(in crate::substance) const CHONDRITE_L: CondensedColumns = CondensedColumns {
    grain_density: solid(3_580.0, 293.0),
    transition: at(TransitionKind::Solidus, CHONDRITIC_SOLIDUS_K),
    mechanics: MechanicsFamily::Silicate,
    sources: &[
        source(
            "grain density",
            "the L chondrite falls' mean by stone, 3.58 ± 0.01 g cm⁻³ (216 stones, 3.39–3.90; \
             3.57 by meteorite), from Macke 2010, Table 22 and p. 60, the data of Macke, \
             Consolmagno and Britt 2011 (MAPS 46, 1842); Gail and Trieloff 2019 adopt 3.58. The \
             plan's 3,560 was from memory",
            Basis::Fact,
        ),
        CHONDRITIC_SOLIDUS,
        SILICATE_FAMILY,
    ],
};

/// An LL chondrite.
pub(in crate::substance) const CHONDRITE_LL: CondensedColumns = CondensedColumns {
    grain_density: solid(3_540.0, 293.0),
    transition: at(TransitionKind::Solidus, CHONDRITIC_SOLIDUS_K),
    mechanics: MechanicsFamily::Silicate,
    sources: &[
        source(
            "grain density",
            "the LL chondrite falls' mean, 3.54 g cm⁻³, s.d. 0.13 (Consolmagno et al. 2008, as \
             Kohout et al. 2013, arXiv:1309.6081, quote it); LL falls 3.52 ± 0.01 by stone and \
             3.51 by meteorite in Macke 2010, Table 22",
            Basis::Fact,
        ),
        CHONDRITIC_SOLIDUS,
        SILICATE_FAMILY,
    ],
};

/// An enstatite chondrite, EH and EL in one row.
pub(in crate::substance) const CHONDRITE_E: CondensedColumns = CondensedColumns {
    grain_density: solid(3_660.0, 293.0),
    transition: at(TransitionKind::Solidus, 1_273.15),
    mechanics: MechanicsFamily::Silicate,
    sources: &[
        source(
            "grain density",
            "\"For falls, grain densities range from 3.45 to 4.17 g cm-3, averaging 3.66 g cm-3\" \
             (Macke, Consolmagno, Britt and Hutson 2010, MAPS 45, 1513, abstract; EH falls \
             3.61–3.62 and EL falls 3.64–3.66 in Macke 2010, Table 22)",
            Basis::Fact,
        ),
        source(
            "transition",
            "at most 1,000 °C, an upper bound, the experiments' lowest temperature already holding \
             melt: in Indarch (EH4) \"Silicate melting begins at 1000 C, and Indarch is \
             completely melted by 1500 C. The metal-sulfide component melts completely at 1000 \
             C\" (McCoy, Dickinson and Lofgren 1999, MAPS 34, 735, abstract; the runs' pressure \
             not read), 150 °C below the other chondrites' shared figure, which is no bound for \
             this row",
            Basis::Fact,
        ),
        SILICATE_FAMILY,
    ],
};

/// A comet's refractory material: anhydrous silicates and sulphides with organics.
pub(in crate::substance) const COMETARY: CondensedColumns = CondensedColumns {
    grain_density: solid(2_000.0, 293.0),
    transition: at(TransitionKind::Solidus, CHONDRITIC_SOLIDUS_K),
    mechanics: MechanicsFamily::Organic,
    sources: &[
        source(
            "grain density",
            "1 ÷ ρ = 0.45 ÷ ρ_org + 0.55 ÷ ρ_min with 45 wt% organics (Bardyn et al. 2017, MNRAS \
             469, S712, §3.4), the organics at the carbonaceous row's 1,400 ± 200 (Reynard and \
             Sotin 2023) or Fulle et al.'s 1,200, and the minerals at 3,430–3,480 from olivines \
             and pyroxenes of 3,200 with iron sulphides of 4,600, 4–5 to 1 by volume (Fulle et al. \
             2016, MNRAS 462, S132): 1,870–2,090, so 2,000 ± 200 kg m⁻³; GIADA's compacted dried \
             dust, 1,925 (+2,030/−560), agrees (Fulle et al. 2017, as Engrand et al. 2023, \
             arXiv:2305.03417, §3.5, quote it)",
            Basis::Reduced,
        ),
        source(
            "transition",
            "the chondrites' silicate solidus for its silicates, an upper bound; its organics \
             pyrolyse first, from about 673 K (the carbonaceous row's)",
            Basis::Adopted,
        ),
        CHONDRITIC_SOLIDUS,
        ORGANIC_FAMILY,
    ],
};

#[cfg(test)]
mod tests {
    use super::*;

    /// The rows that share a figure share its constant: `ferric` is `basalt_fe`'s density
    /// (decision-backlog-2 §1.4), the acid's absorber is the acid's, and the hazes' stand-ins are
    /// tholin's.
    #[test]
    fn condensed_shared_figures_are_one_constant() {
        assert_eq!(FERRIC.grain_density, BASALT_FE.grain_density);
        assert_eq!(FERRIC.transition, BASALT_FE.transition);
        assert_eq!(ACID_UV_ABSORBER.grain_density, SULPHURIC_ACID.grain_density);
        assert_eq!(ACID_UV_ABSORBER.transition, SULPHURIC_ACID.transition);
        for haze in [&CHROMOPHORE, &ICE_GIANT_HAZE] {
            assert_eq!(haze.grain_density, THOLIN.grain_density);
            assert_eq!(haze.transition, THOLIN.transition);
        }
        assert_eq!(SOOT.transition, GRAPHITE.transition);
        assert_eq!(MARS_DUST.transition, BASALT_FE.transition);
    }

    /// The anhydrous chondritic solidus the composition ruling shares, which `chondrite_e` leaves
    /// for its own run.
    #[test]
    fn condensed_chondrites_share_their_solidus_but_the_enstatite_one() {
        for row in [
            &CHONDRITE_CI,
            &CHONDRITE_CM,
            &CHONDRITE_CV,
            &CHONDRITE_H,
            &CHONDRITE_L,
            &CHONDRITE_LL,
            &COMETARY,
            &PHYLLOSILICATE,
        ] {
            assert_eq!(
                row.transition.temperature,
                Kelvin::new(CHONDRITIC_SOLIDUS_K)
            );
        }
        assert!(CHONDRITE_E.transition.temperature < Kelvin::new(CHONDRITIC_SOLIDUS_K));
        // Carbonaceous matter's decomposition is where a comet's organics go first.
        assert!(Kelvin::new(CARBONACEOUS_DECOMPOSITION_K) < COMETARY.transition.temperature);
    }
}
