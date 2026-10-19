//! The lithologies' silicate melts (P14.T49.h's step 4; decision-p14-t49h-step4-melts;
//! decision-r11-palette-liquids §1.8, its "Molten silicate" row and "Lithology rows need oxides"):
//! each lithology that can take a melt record, its major oxides by weight with their source and
//! basis, and the members the melt holds as stated values. `substance::liquid`'s melt models make
//! the record from them: the viscosity by Giordano, Russell and Dingwell 2008, or Giordano and
//! Russell 2007 for an alkali-poor, aluminium-rich melt; the density by Lange and Carmichael's
//! partial molar volumes; the index and absorption R08's basaltic glass's, held; the surface
//! tension a stated constant; no scattering. Every record is `StandIn`.
//!
//! The eleven rows and their oxides:
//!
//! - `basalt`: Alae lava lake's bulk (Peck 1978), the row's own source;
//! - `lava_glass`: `basalt`'s, the glass being the quenched melt;
//! - `basalt_fe`: Apollo 15 basalt 15597 (Lunar Sourcebook, Table A6.1), standing in;
//! - `mars_dust` and `ferric`: `basalt_fe`'s, as their solidus is;
//! - `komatiite`: SA 3091 (Arndt 1976), the row's own sample;
//! - `basalt_reduced`: Mercury's low-Mg lava G2 (Charlier, Grove and Zuber 2013), standing in;
//! - `granite`: the USGS standard granite G-1 (Fairbairn et al. 1951), the row's own sample;
//! - `ultramafic`: McDonough and Sun 1995's pyrolite, standing in for a harzburgite;
//! - `anorthosite`: the row's mode by stoichiometry;
//! - `feldspathic`: the average Apollo 16 soil (Lunar Sourcebook, Table 7.15).
//!
//! The lithologies with **no melt record**, which answer `NotALiquid` (a test holds each):
//!
//! - `ca_al_residue` melts at 2,150 K (Léger et al. 2011), but neither model applies: its `SiO₂` of
//!   0 lies below the GRD calculator's 24 wt% floor, where GRD's B comes out negative (−14,464),
//!   and Lange and Carmichael fitted no silica-free aluminate. Its oxides are known (`CaO` 13,
//!   `Al₂O₃` 87 wt%), so a measured CaO–Al₂O₃ melt source would give it one.
//! - `iron_nickel` melts to a metallic liquid, not a silicate melt: a liquid metal's record (its
//!   density, viscosity and surface tension) is deferred.
//! - `phyllosilicate` dehydrates hundreds of kelvin below its adopted chondritic solidus, before it
//!   melts, and its residue's melt is another lithology's.
//! - The seven chondrites (`chondrite_ci`, `_cm`, `_cv`, `_h`, `_l`, `_ll` and `_e`) are metal,
//!   sulphide and silicate, with water and organics in the carbonaceous ones: their first melt at
//!   the solidus is an Fe–FeS eutectic and a basaltic partial melt, not the bulk, and a
//!   differentiated crust's row (`basalt`, `basalt_fe`) carries the melt.
//! - `cometary`'s organics pyrolyse from 673 K first, and its silicate is chondritic.
//! - `SiC` decomposes peritectically, with no silicate melt; `carbonaceous` decomposes, with no
//!   melt; `MgSO4` and `CaCO3` decompose before they melt at one bar.
//! - `Na2CO3` melts at 1,123 K to a carbonate liquid, not a silicate melt: a molten salt's record
//!   would be its own (a finding for the composition audit).

use super::super::liquid::{HeldOptics, LiquidColumns, LiquidConstitution, Oxide, SilicateMelt};
use super::super::{Basis, Source};
use crate::tables::liquid_optics as bins;

/// A melt row's liquid columns: `melt`'s, with `sources`; every other column `None`, the model's.
#[must_use]
const fn melt_columns(melt: &'static SilicateMelt, sources: &'static [Source]) -> LiquidColumns {
    LiquidColumns {
        optics: None,
        scattering: None,
        surface_tension: None,
        viscosity: None,
        supercooled_viscosity: None,
        constitution: LiquidConstitution::Melt(melt),
        sources,
    }
}

/// R08's basaltic glass (`basaltic-glass.json`, the `lava_glass` row's file), reduced to the bake
/// bins by `tables::liquid_optics`: its room-temperature index and absorption, which every melt
/// holds (decision-r11-palette-liquids §1.8).
const BASALTIC_GLASS: HeldOptics = HeldOptics {
    index: &bins::BASALTIC_GLASS_N,
    absorption_per_m: &bins::BASALTIC_GLASS_A,
};

/// A basaltic melt's surface tension, 0.36 N m⁻¹: the middle of Walker and Mullins 1981's 350–370
/// dyn cm⁻¹ for four lavas, limburgite to andesite, at 1,200–1,500 °C in air, argon, CO₂, H₂ and
/// CO, the composition's effect minor and dσ ÷ dT small and positive (Contrib. Mineral. Petrol. 76,
/// 455, its abstract; the paper closed).
const WALKER_MULLINS_1981_N_PER_M: f64 = 0.36;

/// A granitic melt's surface tension, 0.29 N m⁻¹: the middle of Bagdassarov, Dorfman and Dingwell
/// 2000's 280–300 ± 5 mN m⁻¹ for haplogranite and rhyolite melts at 1,473–1,673 K, with dσ ÷ dT of
/// +0.06 to +0.09 mN m⁻¹ K⁻¹, held (Am. Mineral. 85, 33, its abstract, from its first page,
/// SHA-256 bb730b42…).
const BAGDASSAROV_2000_N_PER_M: f64 = 0.29;

/// The index's and absorption's source, every melt's.
const GLASS_OPTICS: Source = Source {
    covers: "n, a",
    citation: "R08's `basaltic-glass.json` (the `lava_glass` row's file): a basaltic glass at room \
               temperature (Pollack, Toon and Khare 1973, Icarus 19, 372, as the Aerosol \
               Refractive Index Archive holds it, its file's SHA-256 e05d88b2…), reduced to the \
               bake bins by `tables::liquid_optics`: n about 1.568 and a = 4πk ÷ λ about 1.06 × \
               10⁴ m⁻¹ across 545 nm, opaque at a millimetre. Held for the melt, not carried to \
               its density or temperature: the glass's own density is not stated (Pollack et al. \
               unread), so a Lorentz–Lorenz carry would rest on an assumed one; a StandIn \
               (decision-r11-palette-liquids §1.8)",
    basis: Basis::Reduced,
};

/// The density's source, every melt's.
const LANGE_CARMICHAEL_DENSITY: Source = Source {
    covers: "ρ",
    citation: "Lange and Carmichael's partial molar volumes and their dV ÷ dT at 1,673 K, as \
               MELTS's `liq_struct_data.h` carries them, credited there to Lange and Carmichael \
               1990 (Rev. Mineral. 24, ch. 2; it and Lange and Carmichael 1987, GCA 51, 2931, \
               closed; the file's SHA-256 206b298d…), by ideal mixing of SiO2, TiO2 (23.16 cm³ \
               mol⁻¹, decision-p14-t49h-step4-melts §6; DensityX's 28.32 at 1,773 K is another \
               set's), Al2O3, Fe2O3, FeO, MgO, CaO, Na2O and K2O, the rest dropped and the nine \
               renormalised; the compressibility neglected at the surface; Derived, and an \
               extrapolation above their 1,873 K",
    basis: Basis::Fact,
};

/// The viscosity's source for a melt that takes Giordano, Russell and Dingwell 2008's model.
const GRD_VISCOSITY: Source = Source {
    covers: "μ",
    citation: "Giordano, Russell and Dingwell 2008 (EPSL 271, 123; the authors' copy, SHA-256 \
               2b778bab…): log10 μ = −4.55 + B ÷ (T − C), B and C from the oxides in mol% with \
               total iron as FeO, its coefficients to the two decimals of the authors' calculator \
               (grd08.js, SHA-256 44806e60…), as their Table 2 uses them; 0.40 log units rms \
               (Table 3); Derived, and an extrapolation above the calculator's 1,800 °C",
    basis: Basis::Fact,
};

/// The viscosity's source for a melt that takes Giordano and Russell 2007's model.
const GR07_VISCOSITY: Source = Source {
    covers: "μ",
    citation: "Giordano and Russell 2007 (J. Phys.: Condens. Matter 19, 205148, Eqs. 1, 4 and 5 \
               and Table 1's VFT column; from Russell's University of British Columbia page, \
               SHA-256 dfd45a6e…): log10 μ = −4.67 + B ÷ (T − C) on SiO2, Al2O3, MgO and CaO in \
               mol%, FeO(T) and MnO as MgO mole for mole, Na2O, K2O, TiO2, Cr2O3 and P2O5 dropped \
               and the four renormalised (decision-p14-t49h-step4-melts §2.1), by §2.3's rule \
               (Al2O3 at least 15 wt%, Na2O + K2O at most 1 and the dropped oxides at most 3); \
               0.224 log units standard error (Table 1); Derived at every temperature. Giordano, \
               Russell and Dingwell 2008 is refused for this melt: it lies 2.5 log units low on \
               Scarfe et al. 1983's anorthite (Am. Mineral. 68, 1083, Table 1), its alkali-free \
               aluminium outside the calibration (§1); Scarfe's Fulcher fits are not the value, \
               their T_0 too high beyond their measured spans (§2.1)",
    basis: Basis::Fact,
};

/// The surface tension's source for a basaltic melt Walker and Mullins measured the like of.
const WALKER_MULLINS_SURFACE_TENSION: Source = Source {
    covers: "σ",
    citation: "0.36 N m⁻¹ at every temperature, the middle of Walker and Mullins 1981's 350–370 \
               dyn cm⁻¹ for four lavas, limburgite to andesite, at 1,200–1,500 °C (Contrib. \
               Mineral. Petrol. 76, 455, its abstract; the paper closed): a stated constant, \
               StandIn",
    basis: Basis::Reduced,
};

/// The surface tension's source for a melt whose composition's was not measured.
const STAND_IN_SURFACE_TENSION: Source = Source {
    covers: "σ",
    citation: "Walker and Mullins 1981's basaltic 0.36 N m⁻¹ (Contrib. Mineral. Petrol. 76, 455, \
               its abstract: 350–370 dyn cm⁻¹ for four lavas, limburgite to andesite, at \
               1,200–1,500 °C), standing in: no measurement of this composition's melt was read",
    basis: Basis::Adopted,
};

/// The surface tension's source for a granitic melt.
const BAGDASSAROV_SURFACE_TENSION: Source = Source {
    covers: "σ",
    citation: "0.29 N m⁻¹ at every temperature, the middle of Bagdassarov, Dorfman and Dingwell \
               2000's 280–300 ± 5 mN m⁻¹ for haplogranite and rhyolite melts at 1,473–1,673 K \
               (Am. Mineral. 85, 33, its abstract, SHA-256 bb730b42…), its dσ ÷ dT of +0.06 to \
               +0.09 mN m⁻¹ K⁻¹ not carried: a stated constant, StandIn",
    basis: Basis::Reduced,
};

/// The scattering's absence, every melt's.
const NO_SCATTERING: Source = Source {
    covers: "b",
    citation: "absent: a melt's own scattering is not modelled, and it is opaque at a millimetre \
               (decision-r11-palette-liquids §1.8)",
    basis: Basis::Adopted,
};

/// Alae lava lake's bulk composition, weight per cent (Peck 1978, USGS Professional Paper 935-B,
/// p. B3), in its analysis's order; the eleven sum to 100.00.
const ALAE_OXIDES: [(Oxide, f64); 11] = [
    (Oxide::Silica, 50.47),
    (Oxide::Alumina, 13.67),
    (Oxide::Ferric, 1.30),
    (Oxide::Ferrous, 9.80),
    (Oxide::Magnesia, 7.55),
    (Oxide::Lime, 11.11),
    (Oxide::Soda, 2.38),
    (Oxide::Potash, 0.54),
    (Oxide::Titania, 2.74),
    (Oxide::PhosphorusPentoxide, 0.27),
    (Oxide::Manganese, 0.17),
];

/// Alae's melt, `basalt`'s and `lava_glass`'s.
const ALAE_MELT: SilicateMelt = SilicateMelt {
    oxides: &ALAE_OXIDES,
    surface_tension_n_per_m: WALKER_MULLINS_1981_N_PER_M,
    optics: BASALTIC_GLASS,
};

/// Basalt's melt, a Hawaiian tholeiite's.
pub(super) const BASALT: LiquidColumns = melt_columns(
    &ALAE_MELT,
    &[
        Source {
            covers: "oxides",
            citation: "Alae lava lake's uniform bulk composition, weight per cent: SiO2 50.47, \
                       Al2O3 13.67, Fe2O3 1.30, FeO 9.80, MgO 7.55, CaO 11.11, Na2O 2.38, K2O \
                       0.54, TiO2 2.74, P2O5 0.27, MnO 0.17 (Peck 1978, USGS Professional Paper \
                       935-B, p. B3), the row's own source, its grain density Alae's; inside \
                       GRD's calibration",
            basis: Basis::Fact,
        },
        GRD_VISCOSITY,
        LANGE_CARMICHAEL_DENSITY,
        GLASS_OPTICS,
        WALKER_MULLINS_SURFACE_TENSION,
        NO_SCATTERING,
    ],
);

/// Lava glass's melt: the basalt's, which it is quenched from.
pub(super) const LAVA_GLASS: LiquidColumns = melt_columns(
    &ALAE_MELT,
    &[
        Source {
            covers: "oxides",
            citation: "`basalt`'s, Alae's bulk (Peck 1978, USGS Professional Paper 935-B, p. \
                       B3), an assumption: the glass is the quenched melt, and its grain density \
                       Alae pumice glass's (p. B37); the glass's own analysis (Wright and Peck \
                       1978) not read",
            basis: Basis::Adopted,
        },
        GRD_VISCOSITY,
        LANGE_CARMICHAEL_DENSITY,
        GLASS_OPTICS,
        WALKER_MULLINS_SURFACE_TENSION,
        NO_SCATTERING,
    ],
);

/// Apollo 15 pigeonite basalt 15597, weight per cent (Taylor et al. 1991, Lunar Sourcebook, ch. 6,
/// Table A6.1, p. 261, after the Basaltic Volcanism Study Project 1981); the ten sum to 99.66.
const APOLLO_15597_OXIDES: [(Oxide, f64); 10] = [
    (Oxide::Silica, 48.0),
    (Oxide::Titania, 1.8),
    (Oxide::Alumina, 9.4),
    (Oxide::Chromia, 0.48),
    (Oxide::Ferrous, 20.2),
    (Oxide::Manganese, 0.30),
    (Oxide::Magnesia, 8.7),
    (Oxide::Lime, 10.4),
    (Oxide::Soda, 0.32),
    (Oxide::Potash, 0.06),
];

/// 15597's melt, `basalt_fe`'s, `mars_dust`'s and `ferric`'s.
const APOLLO_15597_MELT: SilicateMelt = SilicateMelt {
    oxides: &APOLLO_15597_OXIDES,
    surface_tension_n_per_m: WALKER_MULLINS_1981_N_PER_M,
    optics: BASALTIC_GLASS,
};

/// GRD's calibration, as the iron-rich basalt's melt lies outside it.
const APOLLO_15597_CALIBRATION: &str = "outside GRD's calibration in K2O (0.06 wt%, against \
                                        0.3–9) and FeO(T) (20.2, against 0–12), which it \
                                        extrapolates; Giordano and Russell 2007 agrees within \
                                        0.15 log units at 1,473–1,873 K \
                                        (decision-p14-t49h-step4-melts §7)";

/// An iron-rich basalt's melt.
pub(super) const BASALT_FE: LiquidColumns = melt_columns(
    &APOLLO_15597_MELT,
    &[
        Source {
            covers: "oxides",
            citation: "Apollo 15 pigeonite basalt 15597, weight per cent: SiO2 48.0, TiO2 1.8, \
                       Al2O3 9.4, Cr2O3 0.48, FeO 20.2, MnO 0.30, MgO 8.7, CaO 10.4, Na2O 0.32, \
                       K2O 0.06 (Taylor et al. 1991, Lunar Sourcebook, ch. 6, Table A6.1, p. 261, \
                       after BVSP 1981), a named member of the row's class standing in for it; \
                       Apollo 12's 12064 (TiO2 4.0) and Shergotty (Meyer 2012, Mars Meteorite \
                       Compendium, Table 2, after Lodders 1998) recorded",
            basis: Basis::Fact,
        },
        Source {
            covers: "μ",
            citation: APOLLO_15597_CALIBRATION,
            basis: Basis::Fact,
        },
        GRD_VISCOSITY,
        LANGE_CARMICHAEL_DENSITY,
        GLASS_OPTICS,
        WALKER_MULLINS_SURFACE_TENSION,
        NO_SCATTERING,
    ],
);

/// Mars-type dust's melt: `basalt_fe`'s.
pub(super) const MARS_DUST: LiquidColumns = melt_columns(
    &APOLLO_15597_MELT,
    &[
        Source {
            covers: "oxides",
            citation: "`basalt_fe`'s (Apollo 15 basalt 15597, Lunar Sourcebook, Table A6.1), an \
                       assumption, as its solidus is: the dust is iron-rich and basaltic. Its own \
                       APXS composition (Berger et al. 2016, GRL) was not read, and its SO3 and \
                       Cl lie outside both models",
            basis: Basis::Adopted,
        },
        Source {
            covers: "μ",
            citation: APOLLO_15597_CALIBRATION,
            basis: Basis::Fact,
        },
        GRD_VISCOSITY,
        LANGE_CARMICHAEL_DENSITY,
        GLASS_OPTICS,
        WALKER_MULLINS_SURFACE_TENSION,
        NO_SCATTERING,
    ],
);

/// A ferric rock's melt: `basalt_fe`'s.
pub(super) const FERRIC: LiquidColumns = melt_columns(
    &APOLLO_15597_MELT,
    &[
        Source {
            covers: "oxides",
            citation: "`basalt_fe`'s (Apollo 15 basalt 15597, Lunar Sourcebook, Table A6.1), an \
                       assumption, as its solidus and grain density are: the ferric phases are a \
                       few weight per cent and melt into the basalt, and GRD takes total iron as \
                       FeO",
            basis: Basis::Adopted,
        },
        Source {
            covers: "μ",
            citation: APOLLO_15597_CALIBRATION,
            basis: Basis::Fact,
        },
        GRD_VISCOSITY,
        LANGE_CARMICHAEL_DENSITY,
        GLASS_OPTICS,
        WALKER_MULLINS_SURFACE_TENSION,
        NO_SCATTERING,
    ],
);

/// The spinifex-textured komatiite SA 3091 fused and quenched to a glass, weight per cent, iron as
/// total `FeO` (Arndt 1976, Carnegie Inst. Washington Year Book 75, 555, Table 17, col. 1, p. 556,
/// read from its page image); its stated total 100.1.
const SA_3091_OXIDES: [(Oxide, f64); 9] = [
    (Oxide::Silica, 45.4),
    (Oxide::Titania, 0.39),
    (Oxide::Alumina, 7.5),
    (Oxide::Chromia, 0.40),
    (Oxide::Ferrous, 12.9),
    (Oxide::Manganese, 0.28),
    (Oxide::Magnesia, 25.1),
    (Oxide::Lime, 7.4),
    (Oxide::Soda, 0.73),
];

/// SA 3091's melt.
const SA_3091_MELT: SilicateMelt = SilicateMelt {
    oxides: &SA_3091_OXIDES,
    surface_tension_n_per_m: WALKER_MULLINS_1981_N_PER_M,
    optics: BASALTIC_GLASS,
};

/// Komatiite's melt.
pub(super) const KOMATIITE: LiquidColumns = melt_columns(
    &SA_3091_MELT,
    &[
        Source {
            covers: "oxides",
            citation: "SA 3091, the row's own sample, fused and quenched to a glass and analysed \
                       by microprobe, iron as total FeO, weight per cent: SiO2 45.4, TiO2 0.39, \
                       Al2O3 7.5, Cr2O3 0.40, FeO 12.9, MnO 0.28, MgO 25.1, CaO 7.4, Na2O 0.73, \
                       total 100.1, no K2O or P2O5 reported (Arndt 1976, Carnegie Inst. \
                       Washington Year Book 75, 555, Table 17, col. 1, p. 556)",
            basis: Basis::Fact,
        },
        Source {
            covers: "μ",
            citation: "outside GRD's calibration in K2O (none, against 0.3–9 wt%) and FeO(T) \
                       (12.9, against 0–12), which it extrapolates; Giordano and Russell 2007 \
                       agrees within 0.47 log units at 1,473–1,873 K \
                       (decision-p14-t49h-step4-melts §7)",
            basis: Basis::Fact,
        },
        GRD_VISCOSITY,
        LANGE_CARMICHAEL_DENSITY,
        GLASS_OPTICS,
        STAND_IN_SURFACE_TENSION,
        NO_SCATTERING,
    ],
);

/// Mercury's low-Mg lava G2, weight per cent, the average of MESSENGER XRS footprints 6, 7, 8 and
/// 10 on an oxide basis without sulphur (Charlier, Grove and Zuber 2013, EPSL 363, 50, Table 1);
/// the six sum to 99.98.
const MERCURY_G2_OXIDES: [(Oxide, f64); 6] = [
    (Oxide::Silica, 56.6),
    (Oxide::Titania, 0.59),
    (Oxide::Alumina, 13.2),
    (Oxide::Ferrous, 3.37),
    (Oxide::Magnesia, 19.3),
    (Oxide::Lime, 6.92),
];

/// G2's melt.
const MERCURY_G2_MELT: SilicateMelt = SilicateMelt {
    oxides: &MERCURY_G2_OXIDES,
    surface_tension_n_per_m: WALKER_MULLINS_1981_N_PER_M,
    optics: BASALTIC_GLASS,
};

/// A reduced basalt's melt, Mercury's lid's.
pub(super) const BASALT_REDUCED: LiquidColumns = melt_columns(
    &MERCURY_G2_MELT,
    &[
        Source {
            covers: "oxides",
            citation: "Mercury's low-Mg lava G2, the average of MESSENGER XRS footprints 6, 7, 8 \
                       and 10 on an oxide basis without sulphur, weight per cent: SiO2 56.6, TiO2 \
                       0.59, Al2O3 13.2, FeO 3.37, MgO 19.3, CaO 6.92 (Charlier, Grove and Zuber \
                       2013, EPSL 363, 50, Table 1; its submitted version, ORBi 2268/173794), \
                       the row's solidus source, standing in for the row; its high-Mg G1 \
                       recorded. No Na2O: XRS did not measure sodium",
            basis: Basis::Fact,
        },
        Source {
            covers: "μ",
            citation: "outside GRD's calibration in K2O (none, against 0.3–9 wt%), which it \
                       extrapolates; Giordano and Russell 2007 agrees within 0.22 log units at \
                       1,473–1,873 K (decision-p14-t49h-step4-melts §7), and GRD gives Charlier \
                       et al.'s own 4.0 Pa s at 1,380 °C (their Table 1)",
            basis: Basis::Fact,
        },
        GRD_VISCOSITY,
        LANGE_CARMICHAEL_DENSITY,
        GLASS_OPTICS,
        STAND_IN_SURFACE_TENSION,
        NO_SCATTERING,
    ],
);

/// The USGS standard granite G-1's arithmetic mean, weight per cent (Fairbairn et al. 1951, USGS
/// Bulletin 980, Table 20, p. 41), its H₂O⁺ of 0.37 dropped for a melt at one bar; the eleven sum
/// to 99.57.
const G_1_OXIDES: [(Oxide, f64); 11] = [
    (Oxide::Silica, 72.22),
    (Oxide::Titania, 0.26),
    (Oxide::Alumina, 14.44),
    (Oxide::Ferric, 0.94),
    (Oxide::Ferrous, 1.00),
    (Oxide::Manganese, 0.03),
    (Oxide::Magnesia, 0.39),
    (Oxide::Lime, 1.42),
    (Oxide::Soda, 3.26),
    (Oxide::Potash, 5.51),
    (Oxide::PhosphorusPentoxide, 0.10),
];

/// G-1's melt.
const G_1_MELT: SilicateMelt = SilicateMelt {
    oxides: &G_1_OXIDES,
    surface_tension_n_per_m: BAGDASSAROV_2000_N_PER_M,
    optics: BASALTIC_GLASS,
};

/// Granite's melt.
pub(super) const GRANITE: LiquidColumns = melt_columns(
    &G_1_MELT,
    &[
        Source {
            covers: "oxides",
            citation: "the USGS standard granite G-1's arithmetic mean of the cooperative \
                       analyses, weight per cent: SiO2 72.22, TiO2 0.26, Al2O3 14.44, Fe2O3 0.94, \
                       FeO 1.00, MnO 0.03, MgO 0.39, CaO 1.42, Na2O 3.26, K2O 5.51, P2O5 0.10 \
                       (Fairbairn et al. 1951, USGS Bulletin 980, Table 20, p. 41), the row's own \
                       sample, its grain density G-1's mode's; its H2O+ of 0.37 dropped for a melt \
                       at one bar; inside GRD's calibration",
            basis: Basis::Fact,
        },
        GRD_VISCOSITY,
        LANGE_CARMICHAEL_DENSITY,
        GLASS_OPTICS,
        BAGDASSAROV_SURFACE_TENSION,
        NO_SCATTERING,
    ],
);

/// McDonough and Sun 1995's pyrolite as oxides, weight per cent: its element abundances converted
/// with the registry's atomic weights, all iron as `FeO`, to four decimals; the twelve sum to
/// 100.1463.
const PYROLITE_OXIDES: [(Oxide, f64); 12] = [
    (Oxide::Silica, 44.9259),
    (Oxide::Titania, 0.2011),
    (Oxide::Alumina, 4.4402),
    (Oxide::Chromia, 0.3837),
    (Oxide::Ferrous, 8.0534),
    (Oxide::Manganese, 0.1349),
    (Oxide::Magnesia, 37.8083),
    (Oxide::Lime, 3.5400),
    (Oxide::Nickel, 0.2494),
    (Oxide::Soda, 0.3599),
    (Oxide::Potash, 0.0289),
    (Oxide::PhosphorusPentoxide, 0.0206),
];

/// Pyrolite's melt.
const PYROLITE_MELT: SilicateMelt = SilicateMelt {
    oxides: &PYROLITE_OXIDES,
    surface_tension_n_per_m: WALKER_MULLINS_1981_N_PER_M,
    optics: BASALTIC_GLASS,
};

/// An ultramafic rock's melt, standing in for a harzburgite's.
pub(super) const ULTRAMAFIC: LiquidColumns = melt_columns(
    &PYROLITE_MELT,
    &[
        Source {
            covers: "oxides",
            citation: "McDonough and Sun 1995's pyrolite (Chem. Geol. 120, 223), the peridotite \
                       decision-r11-palette-liquids §1.8 names, from its element abundances as \
                       the `pyrolite` package's PY_McDonoughSun1995.csv holds them (SHA-256 \
                       25846247…: Si 21.0, Mg 22.8, Fe 6.26, Al 2.35, Ca 2.53 wt%; Na 2,670, K \
                       240, Ti 1,205, Cr 2,625, Mn 1,045, P 90, Ni 1,960 ppm), converted to \
                       oxides with the registry's atomic weights, all iron as FeO: a fertile \
                       peridotite standing in for the row's depleted harzburgite, whose own \
                       60/40 Fo90/En90 mode by stoichiometry (MgO 43.6 wt%) is recorded",
            basis: Basis::Reduced,
        },
        Source {
            covers: "μ",
            citation: "outside GRD's calibration in MgO (37.8 wt%, against 0–32) and K2O (0.03, \
                       against 0.3–9), which it extrapolates; for Hui and Zhang 2007's \
                       peridotite Giordano and Russell 2007 agrees within 0.29 log units at \
                       1,473–1,873 K (decision-p14-t49h-step4-melts §7)",
            basis: Basis::Fact,
        },
        GRD_VISCOSITY,
        LANGE_CARMICHAEL_DENSITY,
        GLASS_OPTICS,
        STAND_IN_SURFACE_TENSION,
        NO_SCATTERING,
    ],
);

/// The anorthosite row's mode as oxides, weight per cent: 95 wt% An95 plagioclase with 5 wt%
/// `En60Fs40` low-calcium pyroxene by stoichiometry with the registry's atomic weights, to three
/// decimals; the six sum to 100.001.
const ANORTHOSITE_OXIDES: [(Oxide, f64); 6] = [
    (Oxide::Silica, 44.839),
    (Oxide::Alumina, 34.045),
    (Oxide::Ferrous, 1.272),
    (Oxide::Magnesia, 1.070),
    (Oxide::Lime, 18.244),
    (Oxide::Soda, 0.531),
];

/// The anorthosite's melt.
const ANORTHOSITE_MELT: SilicateMelt = SilicateMelt {
    oxides: &ANORTHOSITE_OXIDES,
    surface_tension_n_per_m: WALKER_MULLINS_1981_N_PER_M,
    optics: BASALTIC_GLASS,
};

/// Anorthosite's melt.
pub(super) const ANORTHOSITE: LiquidColumns = melt_columns(
    &ANORTHOSITE_MELT,
    &[
        Source {
            covers: "oxides",
            citation: "the row's mode, 95 wt% An95 plagioclase with 5 wt% En60Fs40 low-calcium \
                       pyroxene (science-composition-crusts.md §7.1–§7.2; the pyroxene's Fs40 the \
                       middle of §7.1's Fs30–50, from memory there), reduced to oxides by \
                       stoichiometry with the registry's atomic weights: SiO2 44.839, Al2O3 \
                       34.045, FeO 1.272, MgO 1.070, CaO 18.244, Na2O 0.531; Apollo 16's ferroan \
                       anorthosites 60015 and 62236 (Lunar Sourcebook, Table A6.11) recorded",
            basis: Basis::Reduced,
        },
        GR07_VISCOSITY,
        LANGE_CARMICHAEL_DENSITY,
        GLASS_OPTICS,
        STAND_IN_SURFACE_TENSION,
        NO_SCATTERING,
    ],
);

/// The average Apollo 16 soil, weight per cent (McKay et al. 1991, Lunar Sourcebook, ch. 7, Table
/// 7.15, col. 16, p. 346, read from its page image); the eleven sum to 100.71.
const APOLLO_16_SOIL_OXIDES: [(Oxide, f64); 11] = [
    (Oxide::Silica, 45.0),
    (Oxide::Titania, 0.54),
    (Oxide::Alumina, 27.3),
    (Oxide::Chromia, 0.33),
    (Oxide::Ferrous, 5.1),
    (Oxide::Manganese, 0.30),
    (Oxide::Magnesia, 5.7),
    (Oxide::Lime, 15.7),
    (Oxide::Soda, 0.46),
    (Oxide::Potash, 0.17),
    (Oxide::PhosphorusPentoxide, 0.11),
];

/// The Apollo 16 soil's melt.
const APOLLO_16_SOIL_MELT: SilicateMelt = SilicateMelt {
    oxides: &APOLLO_16_SOIL_OXIDES,
    surface_tension_n_per_m: WALKER_MULLINS_1981_N_PER_M,
    optics: BASALTIC_GLASS,
};

/// A feldspathic crust's melt.
pub(super) const FELDSPATHIC: LiquidColumns = melt_columns(
    &APOLLO_16_SOIL_MELT,
    &[
        Source {
            covers: "oxides",
            citation: "the average Apollo 16 soil, weight per cent: SiO2 45.0, TiO2 0.54, Al2O3 \
                       27.3, Cr2O3 0.33, FeO 5.1, MnO 0.30, MgO 5.7, CaO 15.7, Na2O 0.46, K2O \
                       0.17, P2O5 0.11 (McKay et al. 1991, Lunar Sourcebook, ch. 7, Table 7.15, \
                       col. 16, p. 346), the row's own body's",
            basis: Basis::Fact,
        },
        GR07_VISCOSITY,
        LANGE_CARMICHAEL_DENSITY,
        GLASS_OPTICS,
        STAND_IN_SURFACE_TENSION,
        NO_SCATTERING,
    ],
);
