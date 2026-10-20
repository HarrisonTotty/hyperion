//! The rows' condensed optics (P14.T49.c and P14.T49.i): for each row whose solid's optics are
//! modelled, its reference form, its input, and the fitted values of `tables::substance_albedo`,
//! with their sources.
//!
//! **An index row** is modelled where all of these hold, in registry order:
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
//! **The infrared** (P14.T49.i; decision-p14-t49c-split §1.3): each index row's set beyond 780 nm,
//! from its primaries, each piece of it at one reference temperature, stated in its sources where
//! its source gives one (a set may join pieces of different temperatures):
//!
//! - `H2O`, `CO2`, `CH4`, `N2`, `CO` and `NH3`: their primaries' infrared, each joined to its R08
//!   file at 780 nm, `N2` and `CO` transparent where their sets have no band, `NH3` transparent
//!   below its measured infrared;
//! - `O2`, `Ar`, `H2S` and `NH4SH`: none, no usable infrared primary being found (solid O₂'s are
//!   closed, argon is an atomic solid, H₂S's are closed or give absorbance only, and NH₄SH's,
//!   Howett et al. 2007, is closed). Their data end at 780 nm, and their bins beyond hold its
//!   value, 1: an upper bound, labelled by their stand-in flags and stated in their sources;
//! - the regoliths whose R08 file's own primary reaches the infrared in the same open copy:
//!   `ZnS`, `MgSiO3`, `Mg2SiO4`, `tholin`, `basalt`, `lava_glass` and `mars_dust`. `Na2S` (a
//!   computed index with a flat k), `MnS` (a placeholder k from 1.9 µm, in Kitzmann and Heng's
//!   copy) and `CaTiO3` (two primaries, the visible's a stand-in; Posch et al. 2003's infrared,
//!   Jena's `Per_nk.dat`, reached only through the Wayback Machine with its terms unread, is not
//!   taken) hold their 700–780 nm bin beyond 780 nm, as each row's sources say.
//!
//! **A laboratory row** (P14.T49.i; decision-p14-t49i-rock-mixtures), where no index file serves:
//!
//! - the salts and transparent minerals with a spectrum in the USGS Spectral Library 7 (splib07a;
//!   Kokaly et al. 2017, U.S. public domain), whose files measure no visible absorption or almost
//!   none: `NaCl`, `CaCO3`, `SiO2` and `S8` measured; `KCl` (as halite), `Na2CO3` (as trona),
//!   `MgSO4` (as kieserite), `Al2O3` and `ca_al_residue` (as natural corundum) and `TiO2` (as
//!   natural rutile), labelled stand-ins;
//! - the opaque solids, k above 0.1: `Fe`, `FeS` and `soot` from splib07a's powders, measured;
//!   `iron_nickel` as iron, a stand-in; `Cr` and `graphite` by the opaque slab of R08's file, a
//!   smooth-grain upper bound (splib07a's iron powder sits 15% below the slab of `iron.json`);
//! - the lithologies, chondrites and organics without a whole-rock spectrum: intimate mixtures of
//!   splib07a's minerals at each class's cited pristine mode, weighted by cross-section, the
//!   phases the class's petrography puts below the form's grains at 1 µm (CI, CM, CV and
//!   `cometary`; none for the equilibrated ordinary chondrites, the enstatite chondrites and the
//!   crystalline rocks), each tested against its class's band in the like quantity in
//!   `hyperion-fit`: a miss makes the row a stand-in with its reason, never a refit; a mixture with
//!   no band is a stand-in, an unvalidated mixture; `basalt_fe` is a stand-in, an upper bound;
//!   `carbonaceous` is kerogen, a named analogue;
//! - `mars_dust`: R08's index at its grains' size, 2.8 µm, a stand-in below geometric optics;
//! - `SO2`: Nash, Fanale and Nelson 1980's measured levels, a stand-in.
//!
//! **Not modelled here** (closed set, for the composition audit):
//!
//! - the ices with no index file and no open spectrum: `Kr`, `Xe`, `N2O`, `C2H6` and `CH3OH`
//!   (splib07a's ethane ice at about 76 K, `Ethane_MTG_T~76K_ASDFRa`, is not yet read);
//! - the liquids and solutions, which lie as liquids, not as grains: `H2SO4`, `acid_uv_absorber`,
//!   the brines `brine_nacl`, `brine_mgso4`, `brine_naclo4` and `brine_mgclo4`, `nh3_h2o` and
//!   `ch4_c2h6_n2` (P14.T49.h's liquid optics);
//! - the giant planets' hazes, `chromophore` and `ice_giant_haze`, which form no surface;
//! - `SiC`: no index file and no spectrum in splib07a.

mod lab;

#[cfg(doc)]
use super::super::optics::REGOLITH_MAX_SINGLE_SCATTERING_ALBEDO;
use super::super::optics::{
    ALBEDO_ROW_LEN, CondensedOptics, FRESH_SNOW, Geometry, InfraredIndex, InputProvenance,
    LUNAR_REGOLITH, OpticsInput, ReferenceForm, ReflectanceLevels,
};
use super::super::{Basis, Source, SubstanceKey};
use crate::tables::substance_albedo as table;
use crate::units::Metres;

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
     Hulst's similarity relation (1974, A&A 35, 209, eq. 14, p. 211); the spectral bins, the \
     same slab and relation's spherical albedo averaged over each bin from the index file and \
     the infrared set",
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

/// The row of `key` in reference form `form`, its grains' optics from `input` of `provenance`,
/// with the fitted `values` and the sources `sources`.
#[must_use]
const fn row(
    key: &str,
    form: ReferenceForm,
    input: OpticsInput,
    provenance: InputProvenance,
    values: &'static [f64; ALBEDO_ROW_LEN],
    sources: &'static [Source],
) -> CondensedOptics {
    CondensedOptics {
        key: SubstanceKey::new_const(key),
        form,
        input,
        provenance,
        values,
        sources,
    }
}

/// The row of `key` in reference form `form`, its grains' optics from index file `file` of
/// `provenance` and the infrared set `infrared`, with the fitted `values` and the sources
/// `sources`.
#[must_use]
const fn optics(
    key: &str,
    form: ReferenceForm,
    (file, infrared): (&'static str, Option<InfraredIndex>),
    provenance: InputProvenance,
    values: &'static [f64; ALBEDO_ROW_LEN],
    sources: &'static [Source],
) -> CondensedOptics {
    row(
        key,
        form,
        OpticsInput::Index { file, infrared },
        provenance,
        values,
        sources,
    )
}

/// A source of the bins beyond a row's data: the bin rule's (the module's), `Adopted`.
#[must_use]
const fn held(citation: &'static str) -> Source {
    source("bins beyond the data", citation, Basis::Adopted)
}

/// A source of a stand-in's reason, `Adopted`: the label a reader shows.
#[must_use]
const fn stand_in(citation: &'static str) -> Source {
    source("stand-in", citation, Basis::Adopted)
}

/// The sources of a frost: the fit, fresh snow's photometry and grain size, the index's paper and,
/// where given, the infrared set's.
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
    ($index:expr, $infrared:expr) => {{
        const SOURCES: &[Source] = &[
            FIT,
            SNOW_PHOTOMETRY,
            FROST_GRAINS,
            source("index", $index, Basis::Reduced),
            source("infrared optical constants", $infrared, Basis::Reduced),
        ];
        SOURCES
    }};
    ($index:expr; held: $held:expr) => {{
        const SOURCES: &[Source] = &[
            FIT,
            SNOW_PHOTOMETRY,
            FROST_GRAINS,
            source("index", $index, Basis::Reduced),
            held($held),
        ];
        SOURCES
    }};
}

/// The sources of a regolith: the fit, the lunar regolith's photometry and grain size, the index's
/// paper and, where given, the infrared set's.
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
    ($index:expr, $infrared:expr) => {{
        const SOURCES: &[Source] = &[
            FIT,
            LUNAR_PHOTOMETRY,
            REGOLITH_GRAINS,
            source("index", $index, Basis::Reduced),
            source("infrared optical constants", $infrared, Basis::Reduced),
        ];
        SOURCES
    }};
    ($index:expr; held: $held:expr) => {{
        const SOURCES: &[Source] = &[
            FIT,
            LUNAR_PHOTOMETRY,
            REGOLITH_GRAINS,
            source("index", $index, Basis::Reduced),
            held($held),
        ];
        SOURCES
    }};
}

/// What a laboratory row's albedos are: the fit's.
const FIT_LAB: Source = source(
    "single-scattering albedo, normal albedo, spherical albedo, directional-hemispherical albedo",
    "hyperion-fit's `substance_albedos` (tables::substance_albedo): each of the row's splib07a \
     spectra inverted channel by channel for w by Hapke's model at its own geometry, a Beckman \
     spectrum as the similarity relation's directional-hemispherical albedo at the adopted μ₀ = \
     cos 10° and an \
     ASD spectrum as the reflectance factor with isotropic multiple scattering at i 20°, e 30° and \
     g 35.5°, under the lunar regolith's photometry; each end-member taken to the form's grains \
     through the equivalent slab's internal transmission at its stated n (Hapke 2012's \
     surface-reflection fits, as Davalos 2026 reproduces them) or held as measured; a mixture's w \
     the cross-section weighting Σ (f ÷ D) w ÷ Σ (f ÷ D) (Hapke 2012, its section not read, as \
     decision-p14-t49i-rock-mixtures §2.1 states it); w held at most 0.9995; \
     then the normal albedo and the energy albedos as every row's (Hapke et al. 2012, eqs. 2–4; \
     Kawabata 2016, eqs. 2–3; van de Hulst 1974, eq. 14), and the spectral bins the same \
     relation's spherical albedo averaged over each bin from the spectra",
    Basis::Fitted,
);

/// What an opaque slab's albedos are: the fit's.
const FIT_OPAQUE: Source = source(
    "single-scattering albedo, normal albedo, spherical albedo, directional-hemispherical albedo",
    "hyperion-fit's `substance_albedos` (tables::substance_albedo): w the hemispherically averaged \
     complex Fresnel reflectance of the index file's n and k, S_e = 2 ∫ R(μ) μ dμ, the unpolarised \
     reflectance of a smooth surface (Byrnes 2016, arXiv 1603.02720, eq. 6, the transmitted wave \
     decaying, App. D.2), the grains, of k above 0.1, transmitting nothing at the form's 70 µm; \
     then the normal albedo and the energy albedos as every row's (Hapke et al. 2012, eqs. 2–4; \
     Kawabata 2016, eqs. 2–3; van de Hulst 1974, eq. 14), and the spectral bins the same \
     relation's spherical albedo averaged over each bin from the file",
    Basis::Fitted,
);

/// What measured levels' albedos are: the fit's.
const FIT_LEVELS: Source = source(
    "single-scattering albedo, normal albedo, spherical albedo, directional-hemispherical albedo",
    "hyperion-fit's `substance_albedos` (tables::substance_albedo): the levels, sampled every 1 nm \
     where they vary, inverted for w by the reflectance factor with isotropic multiple scattering \
     (Hapke et al. 2012, eqs. 2–4; Kawabata 2016, eqs. 2–3) at their geometry under fresh snow's \
     photometry, w linear between samples; then the normal albedo and the energy albedos as every \
     row's (van de Hulst 1974, eq. 14), and the spectral bins the same relation's spherical albedo \
     averaged over each bin",
    Basis::Fitted,
);

/// The laboratory library every laboratory row reads.
const SPLIB: Source = source(
    "laboratory spectra",
    "the USGS Spectral Library Version 7, splib07a (Kokaly et al. 2017, USGS Data Series 1035, DOI \
     10.3133/ds1035; data release DOI 10.5066/F7RR1WDJ), U.S. public domain: the spectra each \
     end-member names, committed unmodified as the dataset `usgs_splib07a`, inverted for w, taken \
     to the form's grains or held, and mixed, which changes no value of the library's; USGS has \
     not reviewed these derived values",
    Basis::Raw {
        licence: "U.S. public domain",
    },
);

/// The bins beyond a laboratory row's spectra.
const LAB_HELD: Source = held(
    "its data end with its spectra's, near 3 µm for a Beckman spectrum, at 2.5 µm for an ASD \
     spectrum and where every part's ends for a mixture (its data's reach), and its bins beyond \
     hold the last covered bin's value: the Nicolet's infrared spectra are not spliced on (over \
     2.8–5 µm they would move a row's Bond albedo under a 3,000 K blackbody by −0.03 to +0.04, \
     the lane's prototype found)",
);

/// The reason of a mixture with no band read.
const UNVALIDATED: &str = "an unvalidated mixture: no band of its class's albedo was read \
                           (decision-p14-t49i-rock-mixtures §3), and clean mineral separates bound \
                           an anhydrous rock's albedo from above (`basalt`'s check, in its \
                           sources)";

/// The sources of a laboratory row: the fit, the lunar regolith's photometry and grain size, the
/// library, the bins beyond its spectra, and the row's own.
macro_rules! lab_sources {
    ($($source:expr),+ $(,)?) => {{
        const SOURCES: &[Source] = &[
            FIT_LAB,
            LUNAR_PHOTOMETRY,
            REGOLITH_GRAINS,
            SPLIB,
            LAB_HELD,
            $($source),+
        ];
        SOURCES
    }};
}

/// The label of an opaque slab: an upper bound.
const OPAQUE_UPPER_BOUND: Source = stand_in(
    "a smooth-grain upper bound: the slab's surface term is a smooth facet's reflection, which a \
     rough powder's grains fall below (splib07a's iron powder SA-209309 has w 0.434 at 550 nm, \
     15% below the slab of `iron.json`, 0.509)",
);

/// Halite's clamp, `NaCl`'s and `KCl`'s.
const HALITE_CLAMP: Source = source(
    "single-scattering albedo held",
    "w held at 0.9995, the lunar photometry's energy bound \
     (REGOLITH_MAX_SINGLE_SCATTERING_ALBEDO), where halite's passes it, at 195 of its channels",
    Basis::Adopted,
);

/// `mars_dust`'s reason.
const MARS_DUST_STAND_IN: &str = "grains of 2.8 µm, below geometric optics (a size parameter πD ÷ \
                                  λ of 20 at 440 nm and 2 at 4.4 µm), where the slab is taken \
                                  anyway; and its Bond albedo under a 5,772 K blackbody, 0.36, \
                                  against Mars's bright regions' clear-sky bolometric Lambert \
                                  albedo, 0.27 (Pleskot and Miner 1981, Icarus 45, 179, \
                                  abstract), is brighter, outside the allowance of 1.25 \
                                  (0.216–0.338): a miss, stated, which moves no parameter";

/// `basalt_fe`'s label.
const BASALT_FE_UPPER_BOUND: Source = stand_in(
    "an upper bound, a mixture of clean mineral separates (decision-p14-t49i-rock-mixtures §2.7), \
     within its bounds, above the mature maria's normal albedo 0.09 and at most the HEDs' 0.5 \
     (Beck et al. 2021, Icarus 354, 114066), at 0.410 in V; the same method overstates \
     `basalt`'s whole-rock normal albedo 5.3 times (its sources)",
);

/// `chondrite_ci`'s band.
const CHONDRITE_CI_BAND: Source = source(
    "band",
    "its reflectance factor at i 30°, e 0° and 550 nm, 0.042, against CI's 0.020–0.035 \
     (decision-p14-t49i-rock-mixtures §3), after Amano et al. 2023 (Sci. Adv. 9, eadi3789): the \
     ruling's lower end, 0.020, is Ryugu's global visible reflectance, about 2%, from remote \
     sensing (ONC-T and NIRS3), while Ryugu's returned grains measure about 2 to 4% at v-band at \
     i 30°, e 0° (g 30°), its powder aggregates inside that range and darker than its coarse \
     grains; the upper end is Orgueil's preheated < 155 µm powder, about 3.5% at v-band at the \
     same geometry; inside the allowance of 1.25 (0.016–0.044), at its top: passed",
    Basis::Fitted,
);

/// `chondrite_cm`'s band.
const CHONDRITE_CM_BAND: Source = source(
    "band",
    "its normal albedo at 550 nm, 0.070, against CM's equivalent geometric albedo, 0.066 ± \
     0.019, at zero phase without the coherent backscatter (Beck et al. 2021, Icarus 354, 114066, \
     §6.1; a solar-spectrum mean over 0.4–2.5 µm, 0.85–1.25 times the 550 nm value, §5.3), inside \
     the allowance of 1.25 (0.038–0.106); and its reflectance factor at i 30°, e 0° and 560 nm, \
     0.041, against 0.028–0.140 at 0.56 µm (Cloutis et al. 2011b, Icarus 216, 309, abstract, \
     powders and chips), inside it (0.022–0.175): passed",
    Basis::Fitted,
);

/// `chondrite_cv`'s band.
const CHONDRITE_CV_BAND: Source = source(
    "band",
    "its reflectance factor at i 30°, e 0° and 560 nm, 0.096, against CV's ∼5–13% at 0.56 µm \
     (Cloutis et al. 2012, Icarus 221, 328, abstract, powders and slabs), inside the allowance of \
     1.25 (0.04–0.16): passed",
    Basis::Fitted,
);

/// `chondrite_h`'s band.
const CHONDRITE_H_BAND: Source = source(
    "band",
    "its reflectance factor at i 30°, e 0° and 700 nm, 0.382, against the falls' 0.25–0.39, \
     Chelyabinsk (LL5) and Viñales (L) at 45–90 µm, measured at i 0°, e 30° (Bowen et al. 2023, \
     PSJ 4, 52, p. 3; their lab's geometry, Cantillo et al. 2023, PSJ 4, 177, read as \
     arXiv:2401.10377, §3.2), the same by reciprocity, inside the allowance of 1.25 (0.20–0.49): \
     passed",
    Basis::Fitted,
);

/// `chondrite_l`'s band.
const CHONDRITE_L_BAND: Source = source(
    "band",
    "its reflectance factor at i 30°, e 0° and 700 nm, 0.377, against the falls' 0.25–0.39, \
     Chelyabinsk (LL5) and Viñales (L) at 45–90 µm, measured at i 0°, e 30° (Bowen et al. 2023, \
     PSJ 4, 52, p. 3; their lab's geometry, Cantillo et al. 2023, PSJ 4, 177, read as \
     arXiv:2401.10377, §3.2), the same by reciprocity, inside the allowance of 1.25 (0.20–0.49): \
     passed",
    Basis::Fitted,
);

/// `chondrite_ll`'s band.
const CHONDRITE_LL_BAND: Source = source(
    "band",
    "its reflectance factor at i 30°, e 0° and 700 nm, 0.368, against the falls' 0.25–0.39, \
     Chelyabinsk (LL5) and Viñales (L) at 45–90 µm, measured at i 0°, e 30° (Bowen et al. 2023, \
     PSJ 4, 52, p. 3; their lab's geometry, Cantillo et al. 2023, PSJ 4, 177, read as \
     arXiv:2401.10377, §3.2), the same by reciprocity, inside the allowance of 1.25 (0.20–0.49): \
     passed",
    Basis::Fitted,
);

/// `cometary`'s reason.
const COMETARY_STAND_IN: &str = "an unvalidated mixture: no band of its class is taken \
                                  (decision-p14-t49i-rock-mixtures §3). 67P's nucleus, context \
                                  and not a band, since a body's geometric albedo carries its \
                                  surface's roughness, porosity, coherent backscatter and \
                                  weathering, has a geometric albedo of 6.5 ± 0.2% at 649 nm \
                                  (Fornasier et al. 2015, A&A 583, A30, p. 11) and a Hapke w of \
                                  0.045 (their Table 3), beside this row's normal albedo, 0.051 \
                                  at 640 nm, and its w, 0.117";

/// SO₂ frost's levels (Nash, Fanale and Nelson 1980; the row's sources).
const SO2_LEVELS: ReflectanceLevels = ReflectanceLevels::new(
    &[
        (300.0, 0.035),
        (320.0, 0.035),
        (400.0, 0.775),
        (850.0, 0.775),
    ],
    match Geometry::new(20.0, 60.0, 180.0) {
        Ok(geometry) => geometry,
        Err(_) => panic!("Nash, Fanale and Nelson's geometry is inside its ranges"),
    },
);

/// Mars's dust's grains: 2.8 µm, twice diffuse dust's effective radius (the row's sources).
const MARS_DUST_GRAIN_DIAMETER: Metres = Metres::new(2.8e-6);

/// Every modelled row, in registry order.
pub(in crate::substance) static OPTICS: [CondensedOptics; 55] = [
    optics(
        "H2O",
        ReferenceForm::Frost,
        ("water-ice.json", Some(InfraredIndex::WaterIce)),
        InputProvenance::Measured,
        &table::H2O_ALBEDO,
        frost_sources!(
            "water-ice.json: ice Ih at 266 K (Warren and Brandt 2008, JGR 113, D14220), through \
             refractiveindex.info's CC0 copy",
            "Warren and Brandt 2008 (JGR 113, D14220) at 266 K beyond 780 nm, through \
             refractiveindex.info's CC0 copy, with Grundy and Schmitt 1998's absorption of \
             hexagonal ice at 100 K (JGR 103, 25809) over 0.9623–2.742 µm, k = αλ ÷ 4π, whose k is \
             2.64 times Warren and Brandt's where it begins and 0.561 times where it ends, the \
             steps of the join; the datasets `warren_brandt_2008_h2o` and \
             `grundy_schmitt_1998_h2o`"
        ),
    ),
    optics(
        "CH4",
        ReferenceForm::Frost,
        ("methane-ice.json", Some(InfraredIndex::MethaneIce)),
        InputProvenance::StandIn,
        &table::CH4_ALBEDO,
        frost_sources!(
            "methane-ice.json: phase I at 90 K (Martonchik and Orton 1994, Appl. Opt. 33, 8306), \
             taken here as a stand-in, though its file says measured: its visible k of 0 is a gap \
             in its source, not a measurement of its absorption (the file's reduction), so the \
             grains take w = 1, an upper bound (decision-p14-t49c-split §5.6 and §7's rule for a \
             visible k that is not a measurement)",
            "Grundy, Schmitt and Quirico 2002's absorption of methane ice I at 40 K (Icarus 155, \
             486) over 0.78–4.913 µm, k = αλ ÷ 4π, with Martonchik and Orton 1994's n at 90 K, \
             held past its 2.381 µm row; its k at 780 nm is 3.5 × 10⁻⁸, in a band, where the \
             file's is 0; saturated in 3.2–3.45 µm, where 100 µm grains are opaque at any k; the \
             datasets `grundy_2002_ch4` and `martonchik_orton_1994_ch4`"
        ),
    ),
    optics(
        "NH3",
        ReferenceForm::Frost,
        ("ammonia-ice.json", Some(InfraredIndex::AmmoniaIce)),
        InputProvenance::StandIn,
        &table::NH3_ALBEDO,
        frost_sources!(
            "ammonia-ice.json, a stand-in: crystalline NH3's real index (Martonchik, Orton and \
             Appleby 1984, Appl. Opt. 23, 541) with k = 0, since their Table I gives the visible \
             only at 668 and 433 nm, a linear interpolation in k between its 6,840 and 31,250 \
             cm⁻¹ rows, not a measurement, and no measured visible absorption of the ice was \
             found, so the grains take w = 1, an upper bound (decision-p14-t49c-split §5.3; \
             R08.T5.i flagged the file and drew no k)",
            "Martonchik, Orton and Appleby 1984's measured infrared from 1.462 µm, through \
             optool's copy, with k = 0 over 0.78–1.462 µm, continuing the file's statement, and n \
             from the same table; the dataset `martonchik_1984_nh3`"
        ),
    ),
    optics(
        "N2",
        ReferenceForm::Frost,
        ("nitrogen-ice.json", Some(InfraredIndex::NitrogenIce)),
        InputProvenance::StandIn,
        &table::N2_ALBEDO,
        frost_sources!(
            "nitrogen-ice.json, a stand-in: a non-absorbing crystal whose real index is a \
             Lorentz–Lorenz estimate (Peck and Khanna 1966; Schuch and Mills 1970's cell), so \
             the grains take w = 1",
            "Grundy, Schmitt and Quirico 1993's 2.15 µm band of β-N2 at 41 K (Icarus 105, 254) \
             over 2.0–2.3 µm, k = αλ ÷ 4π, and k = 0 elsewhere to 5 µm, a statement of \
             transparency, not a measurement, with n held at the file's 780 nm value; the dataset \
             `grundy_1993_n2`"
        ),
    ),
    optics(
        "O2",
        ReferenceForm::Frost,
        ("oxygen-ice.json", None),
        InputProvenance::StandIn,
        &table::O2_ALBEDO,
        frost_sources!(
            "oxygen-ice.json, a stand-in: a non-absorbing crystal whose real index is a \
             Lorentz–Lorenz estimate (Křen 2011; Cox, Samuelsen and Beckurts 1973's cell), so the \
             grains take w = 1";
            held: "its data end at 780 nm, and its bins beyond hold its 700–780 nm bin's value, \
                   1, an upper bound: no usable infrared primary of solid O2 was found, the \
                   measurements found being closed"
        ),
    ),
    optics(
        "CO2",
        ReferenceForm::Frost,
        (
            "carbon-dioxide-ice.json",
            Some(InfraredIndex::CarbonDioxideIce),
        ),
        InputProvenance::Measured,
        &table::CO2_ALBEDO,
        frost_sources!(
            "carbon-dioxide-ice.json: n from Warren 1986 (Appl. Opt. 25, 2650, Table I) and k \
             from Hansen 2005's estimated visible absorption (JGR 110, E11003)",
            "Warren 1986 (Appl. Opt. 25, 2650) beyond 780 nm, through optool's copy, a compilation \
             of laboratory measurements \"with emphasis on values for temperatures above 77 K\" \
             (its abstract), not one sample's; its abstract states that \"no measurements of \
             absorption are available\" over 1.0–2.5 µm \"except in the strong narrow absorption \
             lines\", so between the bands there its k is Warren's interpolation, not a \
             measurement; at 780 nm its k is about 2,370 times the file's (Hansen 2005's estimated \
             continuum), taken as an upper bound on the absorption between bands; Hansen 1997 and \
             2005's near infrared could not be fetched; the dataset `warren_1986_co2`"
        ),
    ),
    optics(
        "Ar",
        ReferenceForm::Frost,
        ("argon-ice.json", None),
        InputProvenance::StandIn,
        &table::AR_ALBEDO,
        frost_sources!(
            "argon-ice.json: solid Ar at 80 K (Sinnock and Smith 1969, Phys. Rev. 181, 1297), \
             taken here as a stand-in, though its file says measured: its source gives n only, \
             with k = 0 stated, not measured, so the grains take w = 1, an upper bound \
             (decision-p14-t49c-split §5.6 and §7's rule for a visible k that is not a \
             measurement)";
            held: "its data end at 780 nm, and its bins beyond hold its 700–780 nm bin's value, \
                   1: no infrared primary is taken, argon being an atomic solid with no \
                   vibrational band"
        ),
    ),
    optics(
        "CO",
        ReferenceForm::Frost,
        (
            "carbon-monoxide-ice.json",
            Some(InfraredIndex::CarbonMonoxideIce),
        ),
        InputProvenance::StandIn,
        &table::CO_ALBEDO,
        frost_sources!(
            "carbon-monoxide-ice.json, a stand-in: a non-absorbing crystal whose real index is a \
             Lorentz–Lorenz estimate (Cuthbertson and Cuthbertson 1920; Vegard 1930's cell), so \
             the grains take w = 1",
            "Gerakines, Materese and Hudson 2023's α-CO at 25 K (MNRAS 522, 3145) over 3.33–5 µm, \
             its fundamental band, from NASA's Optical Constants Database, averaged to its 0.5 \
             cm⁻¹ resolution, its n 0.46% below the file's where it begins, at 3,333.5 nm; k = 0 \
             over 0.78–3.33 µm with n held at the file's 780 nm value, the overtones at 1.58 and \
             2.35 µm not drawn (Quirico and Schmitt 1997 being in SSHADE only); its k taken as 0 \
             below its noise floor, 1.02 × 10⁻³, three times its baseline's rms, which discards \
             absorption that matters (at 4.6 µm a k at the floor would give 100 µm grains Θ ≈ 0.80 \
             and w ≈ 0.71), so its 3.5–5 µm bin is an upper bound; the dataset \
             `gerakines_2023_co`"
        ),
    ),
    row(
        "SO2",
        ReferenceForm::Frost,
        OpticsInput::Levels {
            levels: &SO2_LEVELS,
        },
        InputProvenance::StandIn,
        &table::SO2_ALBEDO,
        {
            const SOURCES: &[Source] = &[
                FIT_LEVELS,
                SNOW_PHOTOMETRY,
                source(
                    "grain size",
                    "not stated by Nash, Fanale and Nelson 1980: the levels' w is their measured \
                     frost's, with no rescale to fresh snow's 100 µm",
                    Basis::Adopted,
                ),
                source(
                    "levels",
                    "Nash, Fanale and Nelson 1980 (GRL 7, 665, abstract): SO2 frost grown in \
                     vacuum at 130 K, its bidirectional reflectance at incidence 20° and emission \
                     60° \"very low reflectivity (2 - 5%) at 0.30 µm, a steep absorption edge \
                     rising at 0.32 µm, maximum reflectivity (75 - 80%) at 0.4 µm, and uniformly \
                     high reflectance throughout the visible and near infrared\", over 0.24–0.85 \
                     µm; as a piecewise-linear reflectance factor at the ranges' midpoints, 0.035 \
                     to 320 nm, rising linearly to 0.775 at 400 nm and flat to 850 nm, the \
                     measurement's end; the phase angle taken as 80°, source and detector on \
                     opposite sides of the normal, which the abstract does not state: its Bond \
                     albedo under a 5,772 K blackbody is then 0.80, and at 40°, the two on the \
                     same side, it would be 0.84",
                    Basis::Reduced,
                ),
                held(
                    "its data end at 850 nm, Nash, Fanale and Nelson's last wavelength, and its \
                     bins beyond hold the 780–1,000 nm bin's value: SO2 frost's infrared bands \
                     (its 4.07 µm ν1 + ν3 band among them) are not drawn",
                ),
                stand_in(
                    "levels read from an abstract, at the ranges' midpoints, with the \
                     measurement's azimuth assumed, its grain size unknown and its infrared bands \
                     not drawn",
                ),
            ];
            SOURCES
        },
    ),
    optics(
        "H2S",
        ReferenceForm::Frost,
        ("hydrogen-sulphide-ice.json", None),
        InputProvenance::StandIn,
        &table::H2S_ALBEDO,
        frost_sources!(
            "hydrogen-sulphide-ice.json, a stand-in: a non-absorbing crystal whose real index is a \
             Lorentz–Lorenz estimate (Cuthbertson and Cuthbertson 1910; Vegard and \
             Sinding-Oserød 1942's cell), so the grains take w = 1";
            held: "its data end at 780 nm, and its bins beyond hold its 700–780 nm bin's value, \
                   1, an upper bound: no usable infrared primary of H2S ice was found (Fathe et \
                   al. 2006 is closed, and Zhang et al. 2026 gives absorbance only)"
        ),
    ),
    row(
        "Fe",
        ReferenceForm::Regolith,
        OpticsInput::Spectrum {
            end_member: &lab::IRON,
        },
        InputProvenance::Measured,
        &table::FE_ALBEDO,
        lab_sources!(lab::IRON.source(),),
    ),
    optics(
        "NH4SH",
        ReferenceForm::Frost,
        ("ammonium-hydrosulphide.json", None),
        InputProvenance::StandIn,
        &table::NH4SH_ALBEDO,
        frost_sources!(
            "ammonium-hydrosulphide.json, a stand-in: a non-absorbing particle whose real index is \
             a Lorentz–Lorenz estimate (Cuthbertson and Cuthbertson 1910 and 1914; West 1934's \
             cell), since no visible optical constants of NH4SH are published, so the grains take \
             w = 1";
            held: "its data end at 780 nm, and its bins beyond hold its 700–780 nm bin's value, \
                   1, an upper bound: no usable infrared primary of NH4SH was found (Howett et \
                   al. 2007's is closed)"
        ),
    ),
    row(
        "S8",
        ReferenceForm::Regolith,
        OpticsInput::Spectrum {
            end_member: &lab::SULFUR,
        },
        InputProvenance::Measured,
        &table::S8_ALBEDO,
        lab_sources!(lab::SULFUR.source(),),
    ),
    row(
        "KCl",
        ReferenceForm::Regolith,
        OpticsInput::Spectrum {
            end_member: &lab::HALITE,
        },
        InputProvenance::StandIn,
        &table::KCL_ALBEDO,
        lab_sources!(
            lab::HALITE.source(),
            HALITE_CLAMP,
            stand_in(
                "halite's spectrum, splib07a holding no sylvite: a named analogue, colourless and \
                 of n 1.4903 (the Handbook of Mineralogy's sylvite) against halite's 1.5443 \
                 (decision-p14-t49i-rock-mixtures §4)",
            ),
        ),
    ),
    row(
        "NaCl",
        ReferenceForm::Regolith,
        OpticsInput::Spectrum {
            end_member: &lab::HALITE,
        },
        InputProvenance::Measured,
        &table::NACL_ALBEDO,
        lab_sources!(lab::HALITE.source(), HALITE_CLAMP,),
    ),
    optics(
        "ZnS",
        ReferenceForm::Regolith,
        ("zinc-sulphide.json", Some(InfraredIndex::ZincSulphide)),
        InputProvenance::Measured,
        &table::ZNS_ALBEDO,
        regolith_sources!(
            "zinc-sulphide.json: Palik and Addamiano 1985 (Handbook of Optical Constants of \
             Solids), as Kitzmann and Heng 2018 compile it",
            "Palik and Addamiano 1985 beyond 780 nm, as Kitzmann and Heng 2018 compile it, the \
             file's own source; the dataset `palik_1985_zns`"
        ),
    ),
    optics(
        "Na2S",
        ReferenceForm::Regolith,
        ("sodium-sulphide.json", None),
        InputProvenance::StandIn,
        &table::NA2S_ALBEDO,
        regolith_sources!(
            "sodium-sulphide.json, a stand-in: Khachai et al. 2009's first-principles index of \
             crystalline Na2S (J. Phys.: Condens. Matter 21, 095404), as Kitzmann and Heng 2018 \
             compile it, since no measured visible optical constants were found. Its visible k, \
             about 1.3 × 10⁻² and flat from 550 to 780 nm (2.1 × 10⁻² at 440 nm), is a computed \
             spectrum's tail, not a colourless solid's absorption, so the row comes out too \
             dark (decision-p14-t49c-split §5.3)";
            held: "its data end at 780 nm, and its bins beyond hold its 700–780 nm bin's value: \
                   its file's source, a computed index with a flat k, gives no measured infrared \
                   to join"
        ),
    ),
    optics(
        "MnS",
        ReferenceForm::Regolith,
        ("manganese-sulphide.json", None),
        InputProvenance::Measured,
        &table::MNS_ALBEDO,
        regolith_sources!(
            "manganese-sulphide.json: α-MnS (Huffman and Wild 1967, Phys. Rev. 156, 989), as \
             Kitzmann and Heng 2018 compile it";
            held: "its data end at 780 nm, and its bins beyond hold its 700–780 nm bin's value: \
                   Kitzmann and Heng 2018's copy carries a placeholder k from 1.9 µm, not a \
                   measurement, so no infrared set is joined"
        ),
    ),
    row(
        "Cr",
        ReferenceForm::Regolith,
        OpticsInput::OpaqueSlab {
            file: "chromium.json",
        },
        InputProvenance::StandIn,
        &table::CR_ALBEDO,
        {
            const SOURCES: &[Source] = &[
                FIT_OPAQUE,
                LUNAR_PHOTOMETRY,
                REGOLITH_GRAINS,
                source(
                    "index",
                    "chromium.json: metallic Cr films at room temperature (Johnson and Christy \
                     1974, Phys. Rev. B 9, 5056), through refractiveindex.info's CC0 copy",
                    Basis::Reduced,
                ),
                held(
                    "its data end at 780 nm, and its bins beyond hold its 700–780 nm bin's value: \
                     no infrared set is joined",
                ),
                OPAQUE_UPPER_BOUND,
            ];
            SOURCES
        },
    ),
    row(
        "FeS",
        ReferenceForm::Regolith,
        OpticsInput::Spectrum {
            end_member: &lab::IRON_SULPHIDE,
        },
        InputProvenance::Measured,
        &table::FES_ALBEDO,
        lab_sources!(lab::IRON_SULPHIDE.source(),),
    ),
    optics(
        "MgSiO3",
        ReferenceForm::Regolith,
        ("enstatite-glass.json", Some(InfraredIndex::EnstatiteGlass)),
        InputProvenance::Measured,
        &table::MGSIO3_ALBEDO,
        regolith_sources!(
            "enstatite-glass.json: MgSiO3 glass at 295 K (Dorschner et al. 1995, A&A 300, 503), \
             a glass standing for the crystal's grains",
            "Dorschner et al. 1995 beyond 780 nm at room temperature, through optool's copy, the \
             file's own source; the dataset `dorschner_1995_mgsio3`"
        ),
    ),
    optics(
        "Mg2SiO4",
        ReferenceForm::Regolith,
        (
            "forsterite-amorphous.json",
            Some(InfraredIndex::AmorphousForsterite),
        ),
        InputProvenance::Measured,
        &table::MG2SIO4_ALBEDO,
        regolith_sources!(
            "forsterite-amorphous.json: amorphous Mg2SiO4 at 295 K (Jäger et al. 2003, A&A 408, \
             193), standing for the crystal's grains",
            "Jäger et al. 2003 beyond 780 nm at room temperature, through HITRAN2024's aerosol \
             compilation, the file's own source, its k printed to 10⁻⁴; the dataset \
             `jager_2003_mg2sio4`"
        ),
    ),
    row(
        "SiO2",
        ReferenceForm::Regolith,
        OpticsInput::Spectrum {
            end_member: &lab::QUARTZ,
        },
        InputProvenance::Measured,
        &table::SIO2_ALBEDO,
        lab_sources!(lab::QUARTZ.source(),),
    ),
    row(
        "Al2O3",
        ReferenceForm::Regolith,
        OpticsInput::Spectrum {
            end_member: &lab::CORUNDUM,
        },
        InputProvenance::StandIn,
        &table::AL2O3_ALBEDO,
        lab_sources!(
            lab::CORUNDUM.source(),
            stand_in(
                "natural corundum of 0.5% Fe and 0.01% Cr with minor diaspore, standing for pure \
                 Al2O3: a named analogue (decision-p14-t49i-rock-mixtures §4)",
            ),
        ),
    ),
    optics(
        "CaTiO3",
        ReferenceForm::Regolith,
        ("calcium-titanate.json", None),
        InputProvenance::StandIn,
        &table::CATIO3_ALBEDO,
        regolith_sources!(
            "calcium-titanate.json, a stand-in: Ueda et al. 1998 (J. Phys.: Condens. Matter 10, \
             3669) for the visible, whose k is a Kramers–Kronig value from normal-incidence \
             reflectance, which cannot detect a sub-gap k of 10⁻³, kept as an upper bound on the \
             absorption, so the colourless solid comes out dark, a lower bound on its albedo \
             (decision-p14-t49c-split §5.3; R08.T5.i flagged the file)";
            held: "its data end at 780 nm, and its bins beyond hold its 700–780 nm bin's value: \
                   Posch et al. 2003's infrared (Jena's Per_nk.dat, reached only through the \
                   Wayback Machine, its terms unread) is not taken"
        ),
    ),
    row(
        "TiO2",
        ReferenceForm::Regolith,
        OpticsInput::Spectrum {
            end_member: &lab::RUTILE,
        },
        InputProvenance::StandIn,
        &table::TIO2_ALBEDO,
        lab_sources!(
            lab::RUTILE.source(),
            stand_in(
                "natural reddish-brown rutile of 0.3% Fe, standing for pure TiO2, whose iron \
                 darkens it: a named analogue (decision-p14-t49i-rock-mixtures §4)",
            ),
        ),
    ),
    row(
        "graphite",
        ReferenceForm::Regolith,
        OpticsInput::OpaqueSlab {
            file: "graphite.json",
        },
        InputProvenance::StandIn,
        &table::GRAPHITE_ALBEDO,
        {
            const SOURCES: &[Source] = &[
                FIT_OPAQUE,
                LUNAR_PHOTOMETRY,
                REGOLITH_GRAINS,
                source(
                    "index",
                    "graphite.json: randomly oriented graphite at 20 K, from Draine 2003's \
                     dielectric functions for E ∥ c and E ⊥ c (ApJ 598, 1026), through Draine's \
                     site",
                    Basis::Reduced,
                ),
                held(
                    "its data end at 780 nm, and its bins beyond hold its 700–780 nm bin's value: \
                     no infrared set is joined",
                ),
                OPAQUE_UPPER_BOUND,
            ];
            SOURCES
        },
    ),
    optics(
        "tholin",
        ReferenceForm::Regolith,
        ("tholin.json", Some(InfraredIndex::Tholin)),
        InputProvenance::Measured,
        &table::THOLIN_ALBEDO,
        regolith_sources!(
            "tholin.json: Titan tholin at room temperature (Khare et al. 1984, Icarus 60, 127), \
             through HITRAN2024's aerosol compilation",
            "Khare et al. 1984 beyond 780 nm at room temperature, through HITRAN2024's aerosol \
             compilation, the file's own source; the dataset `khare_1984_tholin`"
        ),
    ),
    row(
        "soot",
        ReferenceForm::Regolith,
        OpticsInput::Spectrum {
            end_member: &lab::CARBON_BLACK,
        },
        InputProvenance::Measured,
        &table::SOOT_ALBEDO,
        lab_sources!(lab::CARBON_BLACK.source(),),
    ),
    row(
        "mars_dust",
        ReferenceForm::Regolith,
        OpticsInput::SizedIndex {
            file: "mars-dust.json",
            infrared: Some(InfraredIndex::MarsDust),
            grain_diameter: MARS_DUST_GRAIN_DIAMETER,
        },
        InputProvenance::StandIn,
        &table::MARS_DUST_ALBEDO,
        {
            const SOURCES: &[Source] = &[
                FIT,
                LUNAR_PHOTOMETRY,
                source(
                    "grain size",
                    "2.8 µm, twice the effective radius of Mars's diffuse dust, 1.4 µm (Wolff et \
                     al. 2009, JGR 114, E00D04, abstract: \"a smaller effective particle radius \
                     more appropriate for diffuse dust conditions (1.4 μm)\"): the radius is \
                     Wolff et al.'s, and the diameter twice it is the lane's choice for Hapke's \
                     D, adopted",
                    Basis::Adopted,
                ),
                source(
                    "index",
                    "mars-dust.json: Mars's dust's index retrieved from CRISM and MER observations \
                     (Wolff et al. 2009, JGR 114, E00D04), as NASA Ames's Mars Climate Modeling \
                     Group distributed it",
                    Basis::Reduced,
                ),
                source(
                    "infrared optical constants",
                    "Wolff et al. 2009 beyond 780 nm, as NASA Ames's Mars Climate Modeling Group \
                     distributed it, the file's own source, retrieved over CRISM's 440–2,920 nm; \
                     the table's rows beyond 2,920 nm lie outside that retrieval, the distributed \
                     table's with no stated primary, and they set the row's 2.8–5 µm bins; n \
                     linear and k linear in ln k, as R08's reduction; the dataset \
                     `wolff_2009_mars_dust`",
                    Basis::Reduced,
                ),
                stand_in(MARS_DUST_STAND_IN),
            ];
            SOURCES
        },
    ),
    row(
        "anorthosite",
        ReferenceForm::Regolith,
        OpticsInput::Mixture {
            parts: lab::ANORTHOSITE,
        },
        InputProvenance::StandIn,
        &table::ANORTHOSITE_ALBEDO,
        lab_sources!(
            lab::ANORTHOSITE_MODE,
            lab::ANORTHITE.source(),
            lab::HYPERSTHENE_C2368.source(),
            stand_in(UNVALIDATED),
        ),
    ),
    row(
        "feldspathic",
        ReferenceForm::Regolith,
        OpticsInput::Mixture {
            parts: lab::FELDSPATHIC,
        },
        InputProvenance::StandIn,
        &table::FELDSPATHIC_ALBEDO,
        lab_sources!(
            lab::FELDSPATHIC_MODE,
            lab::ANORTHITE.source(),
            lab::HYPERSTHENE_C2368.source(),
            lab::OLIVINE_FO66.source(),
            stand_in(UNVALIDATED),
        ),
    ),
    optics(
        "basalt",
        ReferenceForm::Regolith,
        ("basalt.json", Some(InfraredIndex::Basalt)),
        InputProvenance::Measured,
        &table::BASALT_ALBEDO,
        {
            const SOURCES: &[Source] = &[
                FIT,
                LUNAR_PHOTOMETRY,
                REGOLITH_GRAINS,
                source(
                    "index",
                    "basalt.json: a terrestrial basalt (Pollack, Toon and Khare 1973, Icarus 19, \
                     372), through the Aerosol Refractive Index Archive",
                    Basis::Reduced,
                ),
                source(
                    "infrared optical constants",
                    "Pollack, Toon and Khare 1973 beyond 780 nm, through the Aerosol Refractive \
                     Index Archive, the file's own source, n from its rows that give n and k from \
                     its rows that give k; the dataset `pollack_1973_basalt`",
                    Basis::Reduced,
                ),
                source(
                    "method check",
                    "the laboratory mixtures' check (decision-p14-t49i-rock-mixtures §2.6): Peck \
                     1978's average Alae mode (USGS Professional Paper 935-B, p. B3: 3.5 wt% \
                     olivine, 42.1 clinopyroxene, 41.6 plagioclase, 4.4 ilmenite and 1.0 \
                     magnetite, its apatite and glass left out), to vol% at the Handbook of \
                     Mineralogy's densities, as a mixture of splib07a's olivine HS285.2B, augite \
                     NMNH120049, anorthite HS201.3B (for the labradorite), ilmenite HS231.3B and \
                     magnetite HS195.3B at the form's grains, gives a normal albedo 0.493 in V, \
                     5.3 times this whole-rock index's: clean separates bound a \
                     rock's albedo from above, as inclusions, glass and fine groundmass opaques \
                     only darken it. A check, not a calibration: no row is scaled by it",
                    Basis::Fitted,
                ),
            ];
            SOURCES
        },
    ),
    row(
        "basalt_fe",
        ReferenceForm::Regolith,
        OpticsInput::Mixture {
            parts: lab::BASALT_FE,
        },
        InputProvenance::StandIn,
        &table::BASALT_FE_ALBEDO,
        lab_sources!(
            lab::BASALT_FE_MODE,
            lab::AUGITE.source(),
            lab::PIGEONITE.source(),
            lab::OLIVINE_FO66.source(),
            lab::ANORTHITE.source(),
            lab::ILMENITE.source(),
            BASALT_FE_UPPER_BOUND,
        ),
    ),
    row(
        "komatiite",
        ReferenceForm::Regolith,
        OpticsInput::Mixture {
            parts: lab::KOMATIITE,
        },
        InputProvenance::StandIn,
        &table::KOMATIITE_ALBEDO,
        lab_sources!(
            lab::KOMATIITE_MODE,
            lab::OLIVINE_FO89.source(),
            lab::AUGITE.source(),
            stand_in(UNVALIDATED),
        ),
    ),
    row(
        "basalt_reduced",
        ReferenceForm::Regolith,
        OpticsInput::Mixture {
            parts: lab::BASALT_REDUCED,
        },
        InputProvenance::StandIn,
        &table::BASALT_REDUCED_ALBEDO,
        lab_sources!(
            lab::BASALT_REDUCED_MODE,
            lab::ANORTHITE.source(),
            lab::ENSTATITE.source(),
            lab::DIOPSIDE.source(),
            lab::OLIVINE_FO91.source(),
            stand_in(UNVALIDATED),
        ),
    ),
    row(
        "granite",
        ReferenceForm::Regolith,
        OpticsInput::Mixture {
            parts: lab::GRANITE,
        },
        InputProvenance::StandIn,
        &table::GRANITE_ALBEDO,
        lab_sources!(
            lab::GRANITE_MODE,
            lab::QUARTZ.source(),
            lab::MICROCLINE.source(),
            lab::OLIGOCLASE.source(),
            lab::BIOTITE.source(),
            lab::MAGNETITE.source(),
            stand_in(UNVALIDATED),
        ),
    ),
    row(
        "ultramafic",
        ReferenceForm::Regolith,
        OpticsInput::Mixture {
            parts: lab::ULTRAMAFIC,
        },
        InputProvenance::StandIn,
        &table::ULTRAMAFIC_ALBEDO,
        lab_sources!(
            lab::ULTRAMAFIC_MODE,
            lab::OLIVINE_FO89.source(),
            lab::ENSTATITE.source(),
            stand_in(UNVALIDATED),
        ),
    ),
    row(
        "ferric",
        ReferenceForm::Regolith,
        OpticsInput::Mixture { parts: lab::FERRIC },
        InputProvenance::StandIn,
        &table::FERRIC_ALBEDO,
        lab_sources!(
            lab::FERRIC_MODE,
            lab::AUGITE.source(),
            lab::PIGEONITE.source(),
            lab::OLIVINE_FO66.source(),
            lab::ANORTHITE.source(),
            lab::ILMENITE.source(),
            lab::FERRIHYDRITE.source(),
            lab::HEMATITE.source(),
            lab::MAGNETITE.source(),
            stand_in(UNVALIDATED),
        ),
    ),
    row(
        "phyllosilicate",
        ReferenceForm::Regolith,
        OpticsInput::Mixture {
            parts: lab::PHYLLOSILICATE,
        },
        InputProvenance::StandIn,
        &table::PHYLLOSILICATE_ALBEDO,
        lab_sources!(
            lab::PHYLLOSILICATE_MODE,
            lab::LIZARDITE.source(),
            lab::SAPONITE.source(),
            stand_in(UNVALIDATED),
        ),
    ),
    optics(
        "lava_glass",
        ReferenceForm::Regolith,
        ("basaltic-glass.json", Some(InfraredIndex::BasalticGlass)),
        InputProvenance::Measured,
        &table::LAVA_GLASS_ALBEDO,
        regolith_sources!(
            "basaltic-glass.json: a terrestrial basaltic glass (Pollack, Toon and Khare 1973, \
             Icarus 19, 372), through the Aerosol Refractive Index Archive",
            "Pollack, Toon and Khare 1973 beyond 780 nm, through the Aerosol Refractive Index \
             Archive, the file's own source, n from its rows that give n and k from its rows that \
             give k; the dataset `pollack_1973_basaltic_glass`"
        ),
    ),
    row(
        "ca_al_residue",
        ReferenceForm::Regolith,
        OpticsInput::Spectrum {
            end_member: &lab::CORUNDUM,
        },
        InputProvenance::StandIn,
        &table::CA_AL_RESIDUE_ALBEDO,
        lab_sources!(
            lab::CORUNDUM.source(),
            stand_in(
                "natural corundum standing for a Ca–Al refractory residue of hibonite and \
                 corundum, splib07a holding no hibonite: a named analogue \
                 (decision-p14-t49i-rock-mixtures §4)",
            ),
        ),
    ),
    row(
        "iron_nickel",
        ReferenceForm::Regolith,
        OpticsInput::Spectrum {
            end_member: &lab::IRON,
        },
        InputProvenance::StandIn,
        &table::IRON_NICKEL_ALBEDO,
        lab_sources!(
            lab::IRON.source(),
            stand_in(
                "iron powder standing for Fe–Ni metal, splib07a holding no kamacite: a named \
                 analogue (decision-p14-t49i-rock-mixtures §4)",
            ),
        ),
    ),
    row(
        "MgSO4",
        ReferenceForm::Regolith,
        OpticsInput::Spectrum {
            end_member: &lab::KIESERITE,
        },
        InputProvenance::StandIn,
        &table::MGSO4_ALBEDO,
        lab_sources!(
            lab::KIESERITE.source(),
            stand_in(
                "kieserite, the monohydrate MgSO4·H2O, standing for anhydrous MgSO4: a named \
                 analogue (decision-p14-t49i-rock-mixtures §4)",
            ),
        ),
    ),
    row(
        "Na2CO3",
        ReferenceForm::Regolith,
        OpticsInput::Spectrum {
            end_member: &lab::TRONA,
        },
        InputProvenance::StandIn,
        &table::NA2CO3_ALBEDO,
        lab_sources!(
            lab::TRONA.source(),
            stand_in(
                "trona, the hydrated Na3(CO3)(HCO3)·2H2O, standing for Na2CO3: a named analogue \
                 (decision-p14-t49i-rock-mixtures §4)",
            ),
        ),
    ),
    row(
        "CaCO3",
        ReferenceForm::Regolith,
        OpticsInput::Spectrum {
            end_member: &lab::CALCITE,
        },
        InputProvenance::Measured,
        &table::CACO3_ALBEDO,
        lab_sources!(lab::CALCITE.source(),),
    ),
    row(
        "carbonaceous",
        ReferenceForm::Regolith,
        OpticsInput::Spectrum {
            end_member: &lab::KEROGEN,
        },
        InputProvenance::StandIn,
        &table::CARBONACEOUS_ALBEDO,
        lab_sources!(
            lab::KEROGEN.source(),
            stand_in(
                "kerogen, an insoluble coal-tar residue, standing for refractory organics: a named \
                 analogue (decision-p14-t49i-rock-mixtures §4)",
            ),
        ),
    ),
    row(
        "chondrite_ci",
        ReferenceForm::Regolith,
        OpticsInput::Mixture {
            parts: lab::CHONDRITE_CI,
        },
        InputProvenance::Measured,
        &table::CHONDRITE_CI_ALBEDO,
        lab_sources!(
            lab::CHONDRITE_CI_MODE,
            lab::CHONDRITE_CI_FINE,
            lab::SAPONITE.source(),
            lab::LIZARDITE.source(),
            lab::IRON_SULPHIDE.source(),
            lab::MAGNETITE_FINE.source(),
            lab::OLIVINE_FO80.source(),
            lab::KEROGEN.source(),
            CHONDRITE_CI_BAND,
        ),
    ),
    row(
        "chondrite_cm",
        ReferenceForm::Regolith,
        OpticsInput::Mixture {
            parts: lab::CHONDRITE_CM,
        },
        InputProvenance::Measured,
        &table::CHONDRITE_CM_ALBEDO,
        lab_sources!(
            lab::CHONDRITE_CM_MODE,
            lab::CHONDRITE_CM_FINE,
            lab::OLIVINE_FO91.source(),
            lab::OLIVINE_FO80.source(),
            lab::OLIVINE_FO51.source(),
            lab::ENSTATITE.source(),
            lab::LIZARDITE.source(),
            lab::CRONSTEDTITE.source(),
            lab::CALCITE.source(),
            lab::KEROGEN.source(),
            lab::IRON_SULPHIDE.source(),
            lab::MAGNETITE_FINE.source(),
            lab::MAGNETITE.source(),
            CHONDRITE_CM_BAND,
        ),
    ),
    row(
        "chondrite_cv",
        ReferenceForm::Regolith,
        OpticsInput::Mixture {
            parts: lab::CHONDRITE_CV,
        },
        InputProvenance::Measured,
        &table::CHONDRITE_CV_ALBEDO,
        lab_sources!(
            lab::CHONDRITE_CV_MODE,
            lab::CHONDRITE_CV_FINE,
            lab::OLIVINE_FO91.source(),
            lab::OLIVINE_FO80.source(),
            lab::OLIVINE_FO60.source(),
            lab::OLIVINE_FO51.source(),
            lab::OLIVINE_FO29.source(),
            lab::ENSTATITE.source(),
            lab::ANORTHITE.source(),
            lab::IRON_SULPHIDE.source(),
            lab::MAGNETITE_FINE.source(),
            lab::MAGNETITE.source(),
            lab::IRON_FINE.source(),
            lab::IRON.source(),
            CHONDRITE_CV_BAND,
        ),
    ),
    row(
        "chondrite_h",
        ReferenceForm::Regolith,
        OpticsInput::Mixture {
            parts: lab::CHONDRITE_H,
        },
        InputProvenance::Measured,
        &table::CHONDRITE_H_ALBEDO,
        lab_sources!(
            lab::CHONDRITE_H_MODE,
            lab::OLIVINE_FO80.source(),
            lab::HYPERSTHENE_PYX02.source(),
            lab::HYPERSTHENE_C2368.source(),
            lab::DIOPSIDE.source(),
            lab::OLIGOCLASE.source(),
            lab::IRON_SULPHIDE.source(),
            lab::IRON.source(),
            lab::CHROMITE.source(),
            CHONDRITE_H_BAND,
        ),
    ),
    row(
        "chondrite_l",
        ReferenceForm::Regolith,
        OpticsInput::Mixture {
            parts: lab::CHONDRITE_L,
        },
        InputProvenance::Measured,
        &table::CHONDRITE_L_ALBEDO,
        lab_sources!(
            lab::CHONDRITE_L_MODE,
            lab::OLIVINE_FO80.source(),
            lab::OLIVINE_FO66.source(),
            lab::HYPERSTHENE_PYX02.source(),
            lab::HYPERSTHENE_C2368.source(),
            lab::DIOPSIDE.source(),
            lab::OLIGOCLASE.source(),
            lab::IRON_SULPHIDE.source(),
            lab::IRON.source(),
            lab::CHROMITE.source(),
            CHONDRITE_L_BAND,
        ),
    ),
    row(
        "chondrite_ll",
        ReferenceForm::Regolith,
        OpticsInput::Mixture {
            parts: lab::CHONDRITE_LL,
        },
        InputProvenance::Measured,
        &table::CHONDRITE_LL_ALBEDO,
        lab_sources!(
            lab::CHONDRITE_LL_MODE,
            lab::OLIVINE_FO80.source(),
            lab::OLIVINE_FO66.source(),
            lab::HYPERSTHENE_PYX02.source(),
            lab::HYPERSTHENE_C2368.source(),
            lab::DIOPSIDE.source(),
            lab::OLIGOCLASE.source(),
            lab::IRON_SULPHIDE.source(),
            lab::IRON.source(),
            lab::CHROMITE.source(),
            CHONDRITE_LL_BAND,
        ),
    ),
    row(
        "chondrite_e",
        ReferenceForm::Regolith,
        OpticsInput::Mixture {
            parts: lab::CHONDRITE_E,
        },
        InputProvenance::StandIn,
        &table::CHONDRITE_E_ALBEDO,
        lab_sources!(
            lab::CHONDRITE_E_MODE,
            lab::ENSTATITE.source(),
            lab::OLIGOCLASE.source(),
            lab::IRON.source(),
            lab::IRON_SULPHIDE.source(),
            stand_in(UNVALIDATED),
        ),
    ),
    row(
        "cometary",
        ReferenceForm::Regolith,
        OpticsInput::Mixture {
            parts: lab::COMETARY,
        },
        InputProvenance::StandIn,
        &table::COMETARY_ALBEDO,
        lab_sources!(
            lab::COMETARY_MODE,
            lab::COMETARY_FINE,
            lab::KEROGEN.source(),
            lab::OLIVINE_FO89.source(),
            lab::ENSTATITE.source(),
            lab::IRON_SULPHIDE.source(),
            stand_in(COMETARY_STAND_IN),
        ),
    ),
];
