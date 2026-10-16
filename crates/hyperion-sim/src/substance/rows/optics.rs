//! The rows' condensed optics (P14.T49.c): for each row whose solid's optics are modelled, its
//! reference form, the index file its grains' optics are computed from, and the fitted values of
//! `tables::substance_albedo`, with their sources.
//!
//! A row is modelled where all of these hold, in registry order:
//!
//! - it has condensed columns, and rendering plan R08 committed an index file for its solid
//!   (`apps/hyperion/src/renderer/src/view/atmosphere/materials/`, the one source of truth for its
//!   optical constants);
//! - the file's k is at most 0.1 in every band, the lane's threshold for Hapke's equivalent slab,
//!   derived for k ≪ 1 (Emran and Chevrier 2022, arXiv 2204.04192, pp. 3–4, as they read Hapke
//!   1993, who give no number);
//! - and its grains lie inside its reference form's photometry: any grain under fresh snow's, and
//!   under the lunar regolith's a grain whose single-scattering albedo is at most
//!   [`REGOLITH_MAX_SINGLE_SCATTERING_ALBEDO`] in every band.
//!
//! A row's provenance is the stricter of its file's flag and decision-p14-t49c-split's findings:
//! `CH4` and `Ar` (§5.6, the k = 0 stand-ins), whose files say measured, are stand-ins here, their
//! visible k not a measurement of their absorption, until R08 flags the files and the override
//! goes. `NH3` and `CaTiO3` (§5.3) are stand-ins by their files' flags, which R08.T5.i set.
//!
//! An ice whose file gives k = 0 (no visible absorption measured: nitrogen, oxygen, carbon
//! monoxide, hydrogen sulphide, ammonium hydrosulphide, ammonia, argon, and methane, whose source
//! has a gap there) takes w = 1, a lossless grain, and its albedo is the reference form's upper
//! bound.
//!
//! **Not modelled here** (closed set, for the composition audit; P14.T49.i takes laboratory
//! spectra for them, decision-p14-t49c-split §1.3):
//!
//! - every lithology, salt, chondrite and organic without an index file;
//! - the transparent minerals, whose files measure no visible absorption or almost none: `Al2O3`,
//!   `KCl`, `NaCl`, `SiO2`, `S8` (its file a constant-index stand-in without its blue edge) and
//!   `TiO2` (w above [`REGOLITH_MAX_SINGLE_SCATTERING_ALBEDO`] in V and R);
//! - the metals and opaque solids, k above 0.1: `Fe`, `Cr`, `graphite`, `FeS` and `soot`;
//! - `mars_dust`, whose grains, 0.1–2 µm (Moore et al. 1987, USGS Professional Paper 1389,
//!   Table 37, p. 130), are smaller than geometric optics reaches;
//! - the giant planets' hazes, `chromophore` and `ice_giant_haze`, which form no surface.

#[cfg(doc)]
use super::super::optics::REGOLITH_MAX_SINGLE_SCATTERING_ALBEDO;
use super::super::optics::{
    ALBEDO_ROW_LEN, CondensedOptics, FRESH_SNOW, IndexProvenance, LUNAR_REGOLITH, ReferenceForm,
};
use super::super::{Basis, Source, SubstanceKey};
use crate::tables::substance_albedo as table;

/// A source of the values `covers` names.
#[must_use]
const fn source(covers: &'static str, citation: &'static str, basis: Basis) -> Source {
    Source {
        covers,
        citation,
        basis,
    }
}

/// What every row's albedos are: the fit's.
const FIT: Source = source(
    "single-scattering albedo, normal albedo, spherical albedo, directional-hemispherical albedo",
    "hyperion-fit's `substance_albedos` (tables::substance_albedo): Hapke's equivalent slab from \
     the index file's n and k at 440, 550 and 640 nm and the reference grain size (Hapke 2012's \
     surface reflection fits, as Davalos 2026, RMxAA 62, 197, eqs. 7–14, reproduces them), and \
     Hapke's reflectance with isotropic multiple scattering (Hapke, Denevi, Sato, Braden and \
     Robinson 2012, JGR 117, E00H15, eqs. 2–4) and Chandrasekhar's H-function (Kawabata 2016, \
     arXiv 1611.10197, eqs. 2–3) for w and the normal albedo, and the energy albedos by van de \
     Hulst's similarity relation (1974, A&A 35, 209, eq. 14, p. 211)",
    Basis::Fitted,
);

/// Fresh snow's grain size, the frost's.
const FROST_GRAINS: Source = source(
    "grain size",
    "100 µm, twice new snow's representative average grain radius, 50 µm (Wiscombe and Warren \
     1980, J. Atmos. Sci. 37, 2712, p. 2714)",
    Basis::Fact,
);

/// Fresh snow's photometry's source.
const SNOW_PHOTOMETRY: Source = FRESH_SNOW.source();

/// The lunar regolith's photometry's source.
const LUNAR_PHOTOMETRY: Source = LUNAR_REGOLITH.source();

/// A lunar soil's grain size, the regolith's.
const REGOLITH_GRAINS: Source = source(
    "grain size",
    "70 µm, the lunar soils' average median particle size (Carrier, Olhoeft and Mendell 1991, \
     Lunar Sourcebook, ch. 9, p. 478)",
    Basis::Fact,
);

/// The row of `key` in reference form `form`, its grains' optics from `index_file` of
/// `provenance`, with the fitted `values` and the sources `sources`.
#[must_use]
const fn optics(
    key: &str,
    form: ReferenceForm,
    index_file: &'static str,
    provenance: IndexProvenance,
    values: &'static [f64; ALBEDO_ROW_LEN],
    sources: &'static [Source],
) -> CondensedOptics {
    CondensedOptics {
        key: SubstanceKey::new_const(key),
        form,
        index_file,
        provenance,
        values,
        sources,
    }
}

/// The sources of a frost: the fit, fresh snow's photometry and grain size, and the index's paper.
macro_rules! frost_sources {
    ($index:expr) => {{
        const SOURCES: &[Source] = &[
            FIT,
            SNOW_PHOTOMETRY,
            FROST_GRAINS,
            source("index", $index, Basis::Reduced),
        ];
        SOURCES
    }};
}

/// The sources of a regolith: the fit, the lunar regolith's photometry and grain size, and the
/// index's paper.
macro_rules! regolith_sources {
    ($index:expr) => {{
        const SOURCES: &[Source] = &[
            FIT,
            LUNAR_PHOTOMETRY,
            REGOLITH_GRAINS,
            source("index", $index, Basis::Reduced),
        ];
        SOURCES
    }};
}

/// Every modelled row, in registry order.
pub(in crate::substance) static OPTICS: [CondensedOptics; 20] = [
    optics(
        "H2O",
        ReferenceForm::Frost,
        "water-ice.json",
        IndexProvenance::Measured,
        &table::H2O_ALBEDO,
        frost_sources!(
            "water-ice.json: ice Ih at 266 K (Warren and Brandt 2008, JGR 113, D14220), through \
             refractiveindex.info's CC0 copy"
        ),
    ),
    optics(
        "CH4",
        ReferenceForm::Frost,
        "methane-ice.json",
        IndexProvenance::StandIn,
        &table::CH4_ALBEDO,
        frost_sources!(
            "methane-ice.json: phase I at 90 K (Martonchik and Orton 1994, Appl. Opt. 33, 8306), \
             taken here as a stand-in, though its file says measured: its visible k of 0 is a gap \
             in its source, not a measurement of its absorption (the file's reduction), so the \
             grains take w = 1, an upper bound (decision-p14-t49c-split §5.6 and §7's rule for a \
             visible k that is not a measurement)"
        ),
    ),
    optics(
        "NH3",
        ReferenceForm::Frost,
        "ammonia-ice.json",
        IndexProvenance::StandIn,
        &table::NH3_ALBEDO,
        frost_sources!(
            "ammonia-ice.json, a stand-in: crystalline NH3's real index (Martonchik, Orton and \
             Appleby 1984, Appl. Opt. 23, 541) with k = 0, since their Table I gives the visible \
             only at 668 and 433 nm, a linear interpolation in k between its 6,840 and 31,250 \
             cm⁻¹ rows, not a measurement, and no measured visible absorption of the ice was \
             found, so the grains take w = 1, an upper bound (decision-p14-t49c-split §5.3; \
             R08.T5.i flagged the file and drew no k)"
        ),
    ),
    optics(
        "N2",
        ReferenceForm::Frost,
        "nitrogen-ice.json",
        IndexProvenance::StandIn,
        &table::N2_ALBEDO,
        frost_sources!(
            "nitrogen-ice.json, a stand-in: a non-absorbing crystal whose real index is a \
             Lorentz–Lorenz estimate (Peck and Khanna 1966; Schuch and Mills 1970's cell), so \
             the grains take w = 1"
        ),
    ),
    optics(
        "O2",
        ReferenceForm::Frost,
        "oxygen-ice.json",
        IndexProvenance::StandIn,
        &table::O2_ALBEDO,
        frost_sources!(
            "oxygen-ice.json, a stand-in: a non-absorbing crystal whose real index is a \
             Lorentz–Lorenz estimate (Křen 2011; Cox, Samuelsen and Beckurts 1973's cell), so the \
             grains take w = 1"
        ),
    ),
    optics(
        "CO2",
        ReferenceForm::Frost,
        "carbon-dioxide-ice.json",
        IndexProvenance::Measured,
        &table::CO2_ALBEDO,
        frost_sources!(
            "carbon-dioxide-ice.json: n from Warren 1986 (Appl. Opt. 25, 2650, Table I) and k \
             from Hansen 2005's estimated visible absorption (JGR 110, E11003)"
        ),
    ),
    optics(
        "Ar",
        ReferenceForm::Frost,
        "argon-ice.json",
        IndexProvenance::StandIn,
        &table::AR_ALBEDO,
        frost_sources!(
            "argon-ice.json: solid Ar at 80 K (Sinnock and Smith 1969, Phys. Rev. 181, 1297), \
             taken here as a stand-in, though its file says measured: its source gives n only, \
             with k = 0 stated, not measured, so the grains take w = 1, an upper bound \
             (decision-p14-t49c-split §5.6 and §7's rule for a visible k that is not a \
             measurement)"
        ),
    ),
    optics(
        "CO",
        ReferenceForm::Frost,
        "carbon-monoxide-ice.json",
        IndexProvenance::StandIn,
        &table::CO_ALBEDO,
        frost_sources!(
            "carbon-monoxide-ice.json, a stand-in: a non-absorbing crystal whose real index is a \
             Lorentz–Lorenz estimate (Cuthbertson and Cuthbertson 1920; Vegard 1930's cell), so \
             the grains take w = 1"
        ),
    ),
    optics(
        "H2S",
        ReferenceForm::Frost,
        "hydrogen-sulphide-ice.json",
        IndexProvenance::StandIn,
        &table::H2S_ALBEDO,
        frost_sources!(
            "hydrogen-sulphide-ice.json, a stand-in: a non-absorbing crystal whose real index is a \
             Lorentz–Lorenz estimate (Cuthbertson and Cuthbertson 1910; Vegard and \
             Sinding-Oserød 1942's cell), so the grains take w = 1"
        ),
    ),
    optics(
        "NH4SH",
        ReferenceForm::Frost,
        "ammonium-hydrosulphide.json",
        IndexProvenance::StandIn,
        &table::NH4SH_ALBEDO,
        frost_sources!(
            "ammonium-hydrosulphide.json, a stand-in: a non-absorbing particle whose real index is \
             a Lorentz–Lorenz estimate (Cuthbertson and Cuthbertson 1910 and 1914; West 1934's \
             cell), since no visible optical constants of NH4SH are published, so the grains take \
             w = 1"
        ),
    ),
    optics(
        "ZnS",
        ReferenceForm::Regolith,
        "zinc-sulphide.json",
        IndexProvenance::Measured,
        &table::ZNS_ALBEDO,
        regolith_sources!(
            "zinc-sulphide.json: Palik and Addamiano 1985 (Handbook of Optical Constants of \
             Solids), as Kitzmann and Heng 2018 compile it"
        ),
    ),
    optics(
        "Na2S",
        ReferenceForm::Regolith,
        "sodium-sulphide.json",
        IndexProvenance::StandIn,
        &table::NA2S_ALBEDO,
        regolith_sources!(
            "sodium-sulphide.json, a stand-in: Khachai et al. 2009's first-principles index of \
             crystalline Na2S (J. Phys.: Condens. Matter 21, 095404), as Kitzmann and Heng 2018 \
             compile it, since no measured visible optical constants were found. Its visible k, \
             about 1.3 × 10⁻² and flat from 550 to 780 nm (2.1 × 10⁻² at 440 nm), is a computed \
             spectrum's tail, not a colourless solid's absorption, so the row comes out too dark (decision-p14-t49c-split §5.3)"
        ),
    ),
    optics(
        "MnS",
        ReferenceForm::Regolith,
        "manganese-sulphide.json",
        IndexProvenance::Measured,
        &table::MNS_ALBEDO,
        regolith_sources!(
            "manganese-sulphide.json: α-MnS (Huffman and Wild 1967, Phys. Rev. 156, 989), as \
             Kitzmann and Heng 2018 compile it"
        ),
    ),
    optics(
        "MgSiO3",
        ReferenceForm::Regolith,
        "enstatite-glass.json",
        IndexProvenance::Measured,
        &table::MGSIO3_ALBEDO,
        regolith_sources!(
            "enstatite-glass.json: MgSiO3 glass at 295 K (Dorschner et al. 1995, A&A 300, 503), \
             a glass standing for the crystal's grains"
        ),
    ),
    optics(
        "Mg2SiO4",
        ReferenceForm::Regolith,
        "forsterite-amorphous.json",
        IndexProvenance::Measured,
        &table::MG2SIO4_ALBEDO,
        regolith_sources!(
            "forsterite-amorphous.json: amorphous Mg2SiO4 at 295 K (Jäger et al. 2003, A&A 408, \
             193), standing for the crystal's grains"
        ),
    ),
    optics(
        "CaTiO3",
        ReferenceForm::Regolith,
        "calcium-titanate.json",
        IndexProvenance::StandIn,
        &table::CATIO3_ALBEDO,
        regolith_sources!(
            "calcium-titanate.json, a stand-in: Ueda et al. 1998 (J. Phys.: Condens. Matter 10, \
             3669) for the visible, whose k is a Kramers–Kronig value from normal-incidence \
             reflectance, which cannot detect a sub-gap k of 10⁻³, kept as an upper bound on the \
             absorption, so the colourless solid comes out dark, a lower bound on its albedo \
             (decision-p14-t49c-split §5.3; R08.T5.i flagged the file)"
        ),
    ),
    optics(
        "tholin",
        ReferenceForm::Regolith,
        "tholin.json",
        IndexProvenance::Measured,
        &table::THOLIN_ALBEDO,
        regolith_sources!(
            "tholin.json: Titan tholin at room temperature (Khare et al. 1984, Icarus 60, 127), \
             through HITRAN2024's aerosol compilation"
        ),
    ),
    optics(
        "basalt",
        ReferenceForm::Regolith,
        "basalt.json",
        IndexProvenance::Measured,
        &table::BASALT_ALBEDO,
        regolith_sources!(
            "basalt.json: a terrestrial basalt (Pollack, Toon and Khare 1973, Icarus 19, 372), \
             through the Aerosol Refractive Index Archive"
        ),
    ),
    optics(
        "basalt_fe",
        ReferenceForm::Regolith,
        "mars-regolith-simulant.json",
        IndexProvenance::StandIn,
        &table::BASALT_FE_ALBEDO,
        regolith_sources!(
            "mars-regolith-simulant.json, a stand-in: the Mars regolith simulant MGS-1's \
             absorption with its real index fixed at 1.5 (Martikainen et al. 2023, ApJS 268, 47), \
             since no open visible index of an Fe-rich basalt was found. The simulant's bright \
             alteration phases put the row above `basalt`, physically reversed; P14.T49.i \
             replaces it first (decision-p14-t49c-split §5.10)"
        ),
    ),
    optics(
        "lava_glass",
        ReferenceForm::Regolith,
        "basaltic-glass.json",
        IndexProvenance::Measured,
        &table::LAVA_GLASS_ALBEDO,
        regolith_sources!(
            "basaltic-glass.json: a terrestrial basaltic glass (Pollack, Toon and Khare 1973, \
             Icarus 19, 372), through the Aerosol Refractive Index Archive"
        ),
    ),
];
