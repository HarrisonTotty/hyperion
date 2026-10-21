//! The condensed optics: a solid's normal albedo in B, V and R at its reference grain class, with
//! the photometric law the terrain draws it by, and its spherical albedo in contiguous bins over
//! 0.3–5 µm (plan 14, P14.T49.c and P14.T49.i; decision-composition §1.1, §1.7 and §1.8;
//! decision-r10-limb §2–§4; decision-p14-t49c-split §1.3 and §5.2).
//!
//! # What the albedo is
//!
//! A row's normal albedo `A_N` is the radiance factor of a semi-infinite layer of its grains at
//! i = e = g = 0 (Hapke 2012, §10; rendering plan R10, Design note 8), in Johnson B and V and
//! Cousins R at their effective wavelengths, 440, 550 and 640 nm
//! ([`BVR_EFFECTIVE_WAVELENGTHS_NM`]). It is a value of the substance in its reference form,
//! which R10's material table takes as its entry's albedo and multiplies by each form's factor
//! and the body's albedo scale (R10.T1.a and T10.a): an ice's reference form is fresh snow or fine
//! frost, any other solid's a particulate surface of its fresh grains ([`ReferenceForm`]). Space
//! weathering, a cover or a body's own grain size are modifiers on it (P14.T51.d), not part of it.
//!
//! The albedos are offline fits (`hyperion-fit`'s `substance_albedos`, in
//! `tables::substance_albedo`), each the row's own Hapke solution:
//!
//! - **The single-scattering albedo** w of a grain of diameter D in each band comes from the row's
//!   input ([`OpticsInput`]):
//!   - **an index**: Hapke's equivalent slab, from the row's complex index n + ik at the band's
//!     wavelength, rendering plan R08's committed file for the row's solid
//!     (`apps/hyperion/src/renderer/src/view/atmosphere/materials/`), the one source of truth for
//!     a condensate's optical constants (decision-composition §1.1), read by wavelength on the
//!     file's own grid; at the form's grains, or at a stated finer grain for a dust;
//!   - **an opaque slab** (k above 0.1): the same file's surface reflection alone, the
//!     hemispherically averaged complex Fresnel reflectance, since its grains transmit nothing;
//!   - **a laboratory spectrum** (P14.T49.i): a spectrum of the USGS Spectral Library 7
//!     (splib07a; Kokaly et al. 2017, U.S. public domain) inverted for w by this model at its own
//!     geometry, a Beckman spectrum as the directional-hemispherical albedo and an ASD spectrum as
//!     the reflectance factor, then taken to the form's grains through the slab's internal
//!     transmission at a stated real index, or held where the end-member is opaque or its grain
//!     size is not stated ([`EndMember`], [`GrainScaling`]); w held at most
//!     [`REGOLITH_MAX_SINGLE_SCATTERING_ALBEDO`];
//!   - **a mixture** of such end-members at a cited pristine mode, weighted by cross-section
//!     (volume fraction over diameter: Hapke 2012, its section not read, as
//!     decision-p14-t49i-rock-mixtures §2.1 states it), its silicates at the form's grains and the
//!     phases its class's petrography puts below them at [`FINE_GRAIN_DIAMETER`]
//!     ([`MixturePart`]; decision-p14-t49i-rock-mixtures);
//!   - **measured levels** at a stated geometry, inverted as a spectrum ([`ReflectanceLevels`]).
//! - **The normal albedo** `A_N` is Hapke's reflectance factor at i = e = g = 0 with isotropic
//!   multiple scattering (the IMSA, the model the reference forms' photometry was fitted with),
//!   Chandrasekhar's exact H-function, the reference form's single-particle phase function and its
//!   shadow-hiding opposition effect ([`Photometry`]), and no macroscopic roughness, since R10's
//!   mesh, normals and horizon map draw the relief (R10's Design note 8).
//! - **The directional-hemispherical and spherical albedos**, the light a surface returns, are an
//!   energy-conserving solution's (decision-p14-t49c-split §1.2): van de Hulst's similarity
//!   relation (1974, A&A 35, 209, eq. 14), which puts the grains' w and phase-function asymmetry ξ
//!   into an isotropic medium of albedo w* = w (1 − ξ) ÷ (1 − w ξ), whose plane albedo is exactly 1
//!   − √(1 − w*) H(μ₀; w*), with no opposition term, since shadow hiding moves light in angle and
//!   adds none. The spherical albedo is the relation's own quantity, within 0.004 of the
//!   anisotropic medium's: each lies within 0.002 of his fitted z(s) (his summary, p. 209; §3, Fig.
//!   1c, p. 212), and the fit's tests measure at most +0.0028 against his Tables 1 and 2. He showed
//!   it for Henyey–Greenstein media of g 0 to 0.875 (§3, p. 211); the regolith's backscattering
//!   Legendre function (ξ −0.11) extrapolates his test. The directional-hemispherical albedo is an
//!   approximation (his §4, p. 213): against his exact Henyey–Greenstein medium at a = 0.99, g =
//!   0.75 (Table 2) it is +0.017 at μ₀ 1, −0.0075 at 0.5, −0.040 at 0.1 and −0.060 at grazing
//!   incidence, and it is exact for a lossless grain. The IMSA itself loses up to 11% of a lossless
//!   frost's light and makes more than it receives on a bright regolith, so it gives `A_N` alone.
//! - **What it gives**: w, `A_N`, the directional-hemispherical albedo at four incidences
//!   ([`HEMISPHERICAL_INCIDENCE_COSINES`]) and the spherical albedo, each in B, V and R
//!   ([`CondensedOptics`]), and the spherical albedo in each of the spectral bins (below).
//!   Energy is bounded on the whole solution: the IMSA's own
//!   directional-hemispherical albedo, from the same w, phase function and opposition as `A_N`, is
//!   at most 1 at those incidences in every band, and `A_N` at most 1.35, R10's particulate
//!   ceiling (`hyperion_surface::material::PARTICULATE_NORMAL_ALBEDO_CEILING`; decision-r10-limb
//!   §4), both held by the fit's tests; the tabled albedos, the similarity relation's, are at most
//!   1 by construction.
//!
//! # The spectral bins
//!
//! A row's spherical albedo is also held in [`SPECTRAL_BINS`] contiguous bins over 0.3–5 µm
//! ([`SPECTRAL_BIN_EDGES_NM`]), so that P14.T51.b's Bond albedo weights its host's flux over a
//! whole spectrum: an M dwarf puts most of its light in the near infrared, where ice absorbs
//! (decision-p14-t49c-split §5.2). A bin's value is the mean over the bin, uniform in wavelength,
//! of the spectral spherical albedo by the same similarity relation at the row's reference form and
//! grain size, from the row's data interpolated as each source asks
//! ([`CondensedOptics::spherical_albedo_bins`]):
//!
//! - an index row's: over 380–780 nm, R08's index file, the visible's one source; beyond 780 nm,
//!   its infrared set ([`InfraredIndex`]), read by `hyperion-fit` as datasets with their URLs and
//!   checksums and joined to the file at 780 nm, or none;
//! - a laboratory row's: its spectra's channels inside 0.3–5 µm (a Beckman spectrum's to about 3
//!   µm, an ASD spectrum's over 0.35–2.5 µm, a mixture's where all its parts' are), each inverted
//!   for w; the Nicolet's infrared spectra are not spliced on (splicing them over 2.8–5 µm would
//!   move a row's Bond albedo under a 3,000 K blackbody by −0.03 to +0.04, the lane's prototype
//!   found); measured levels' over their knots;
//! - a bin the row's data cover in part is the mean over the covered part, and a bin they do not
//!   cover takes the nearest covered bin's value. Each row states its data's reach
//!   ([`CondensedOptics::data_reach_nm`]): an index row's from 380 nm, so its first bin is its
//!   380–390 nm part, to 5,000 nm or its infrared set's end, or to 780 nm for a row with none,
//!   whose bins beyond hold the 700–780 nm bin's value.
//!
//! **The first and last bins stand for the light outside 0.3–5 µm**: a Bond albedo over a whole
//! spectrum is to weight the first bin by the flux below 390 nm and the last by the flux above
//! 3.5 µm, as the fit's tests do for a blackbody (P14.T51.b's to apply). The B, V and R bins, 1,
//! 2 and 3 ([`BVR_BINS`]), hold the bands' effective wavelengths; the palette's B, V and R values
//! remain the point values at those wavelengths, not the bins' means.
//!
//! One grain class, the reference form's, serves the infrared too (decision-p14-t49c-split §3): a
//! coarse or old ice's infrared albedo is overstated by it, up to about 0.1 in its Bond albedo,
//! which the first task that carries a grain size or age adds.
//!
//! # The phase row
//!
//! A row's photometric law in the terrain, the code its palette entry's `phase_row` carries, is
//! R10's `PhaseLaw::for_family` of its mechanics family ([`CondensedColumns::phase_law`]): the
//! `Ice` law for water and volatile ices, the Moon's `Regolith` law for the rest, never
//! `Lambert`, which a consolidated form chooses for itself (decision-r10-limb §2). R10's terrain
//! keys its rows by the law and the L(0) the law gives at the scaled albedo
//! (decision-r10-t10b-r09-t2e §1), so the law is all a row names.
//!
//! # What is not modelled yet
//!
//! A row has condensed optics where one of its inputs exists ([`Substance::condensed_optics`]): an
//! index file R08 committed whose absorption the equivalent slab can take (k ≪ 1) and whose grains
//! lie inside the form's photometry, an opaque solid's file, an open laboratory spectrum of the
//! solid or of the minerals of its cited mode, or measured levels. The rows without one are a
//! closed list in `rows/optics.rs`, each with its reason, for the composition audit: the ices
//! without an index file or a spectrum, the liquids, the giant planets' hazes and `SiC`. The
//! thermal columns are P14.T49.j's (`substance::thermal`), and the geomorphic columns are out of
//! 23 (decision-p14-t49c-split §1.3). One grain class per form serves the visible, which grain
//! size barely moves, and the infrared at the reference grain; grain classes are deferred, with
//! their size (§3). A palette entry whose row has none cannot be resolved
//! ([`ResolvePaletteEntryError::OpticsNotModelled`]), and is labelled so.
//!
//! # Generator version
//!
//! Generator version 22 publishes none of it: nothing on the record reads these columns until
//! P14.T54.a wires the palette, which restamps the table `--since 23`
//! (decision-bump-plan-after-22 §1.2). Nothing here draws from a stream.

use hyperion_surface::field::{
    BVR_EFFECTIVE_WAVELENGTHS_NM, MechanicsFamily, PaletteEntry, PaletteRole,
};
use hyperion_surface::material::PhaseLaw;

use super::condensed::{CondensedColumns, CondensedPhase};
use super::{Source, Substance, SubstanceId, SubstanceKey, ids};
use crate::units::{Kelvin, Metres};

use super::rows::optics::OPTICS;

/// The cosines of the incidences at which a row's directional-hemispherical albedo is held: 1, 0.5,
/// 0.2 and 0.05 (decision-r10-limb §4's test).
pub const HEMISPHERICAL_INCIDENCE_COSINES: [f64; 4] = [1.0, 0.5, 0.2, 0.05];

/// The number of a row's values in B, V and R, which lead it: three bands each of w, `A_N` and the
/// spherical albedo, and of the directional-hemispherical albedo at each of
/// [`HEMISPHERICAL_INCIDENCE_COSINES`].
pub const BVR_ROW_LEN: usize = 9 + 3 * HEMISPHERICAL_INCIDENCE_COSINES.len();

/// The edges of the spectral bins, nm: [`SPECTRAL_BINS`] contiguous bins over 0.3–5 µm, the
/// infrared's reach that decision-p14-t49c-split §1.3 rules. Each edge's reason:
///
/// - 300 and 5,000: the ruling's ends. The first bin's value stands for all the light below 390
///   nm, and the last's for all the light above 3.5 µm (the [module documentation](self)); where a
///   row's data begin at 380 nm, as every index row's do, the first bin's value is its mean over
///   380–390 nm alone, which then stands for all the light below 390 nm;
/// - 390, 495, 595 and 700: the B, V and R bins hold their effective wavelengths, 440, 550 and 640
///   nm ([`BVR_EFFECTIVE_WAVELENGTHS_NM`]): 495 and 595 are the midpoints between them, and 390 and
///   700 close B and R 50 and 60 nm from theirs;
/// - 780: the end of rendering plan R08's index files, where each infrared set joins its file;
/// - 1,000, 1,300, 1,700, 2,200, 2,800 and 3,500: the near infrared, cut so that water ice's bands
///   fall inside bins, not on their edges: 1.03 and 1.25 µm in the sixth, 1.5 and 1.65 µm in the
///   seventh, 2.0 µm in the eighth, the 2.5 µm continuum and the 2.7 µm wing in the ninth, the 3.0
///   µm fundamental in the tenth, and the 4.5 µm combination band in the last.
pub const SPECTRAL_BIN_EDGES_NM: [f64; 13] = [
    300.0, 390.0, 495.0, 595.0, 700.0, 780.0, 1_000.0, 1_300.0, 1_700.0, 2_200.0, 2_800.0, 3_500.0,
    5_000.0,
];

/// The number of spectral bins, 12.
pub const SPECTRAL_BINS: usize = SPECTRAL_BIN_EDGES_NM.len() - 1;

/// The bins of B, V and R, which hold 440, 550 and 640 nm: the second, third and fourth.
pub const BVR_BINS: [usize; 3] = [1, 2, 3];

/// The offset in a row of its spectral bins' spherical albedos, after its values in B, V and R.
pub const SPECTRAL_BINS_OFFSET: usize = BVR_ROW_LEN;

/// The offset in a row of its data's reach, nm: its first and last wavelength, after its bins.
pub const DATA_REACH_OFFSET: usize = SPECTRAL_BINS_OFFSET + SPECTRAL_BINS;

/// The number of values a row of `tables::substance_albedo` holds: its [`BVR_ROW_LEN`] values in
/// B, V and R, its [`SPECTRAL_BINS`] bins and its data's reach, 35.
pub const ALBEDO_ROW_LEN: usize = DATA_REACH_OFFSET + 2;

/// The bands' wavelengths, nm: the palette's, 440, 550 and 640 ([`BVR_EFFECTIVE_WAVELENGTHS_NM`]).
pub const BAND_WAVELENGTHS_NM: [f64; 3] = BVR_EFFECTIVE_WAVELENGTHS_NM;

/// What a row's grains' optics are computed from (P14.T49.c and P14.T49.i).
///
/// Not `Eq`, `Hash` or `Ord`: a mixture's volume fractions, an end-member's real index and a
/// level's reflectance are real numbers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OpticsInput {
    /// Optical constants n + ik: rendering plan R08's committed index file over 380–780 nm, the
    /// visible's one source, joined at 780 nm to an infrared set where one is read.
    Index {
        /// The file's name, in `apps/hyperion/src/renderer/src/view/atmosphere/materials/`.
        file: &'static str,
        /// The infrared set beyond 780 nm, or `None`: the row's data then end at 780 nm.
        infrared: Option<InfraredIndex>,
    },
    /// Optical constants n + ik, as [`Index`](Self::Index), of grains of a stated diameter in place
    /// of the reference form's: a dust finer than the form's grains, under the form's photometry
    /// (Mars's dust; P14.T49.i).
    SizedIndex {
        /// The file's name, in `apps/hyperion/src/renderer/src/view/atmosphere/materials/`.
        file: &'static str,
        /// The infrared set beyond 780 nm, or `None`.
        infrared: Option<InfraredIndex>,
        /// The grains' diameter, Hapke's D, which replaces the form's.
        grain_diameter: Metres,
    },
    /// An opaque solid's optical constants (k above 0.1 in the visible): R08's index file, whose
    /// grains scatter only what their first surface reflects, the hemispherically averaged complex
    /// Fresnel reflectance `S_e` (P14.T49.i). Its data end at 780 nm.
    OpaqueSlab {
        /// The file's name, in `apps/hyperion/src/renderer/src/view/atmosphere/materials/`.
        file: &'static str,
    },
    /// One laboratory end-member: a measured spectrum inverted for w at its own geometry and taken
    /// to the reference form's grains ([`EndMember`]; P14.T49.i).
    Spectrum {
        /// The end-member.
        end_member: &'static EndMember,
    },
    /// An intimate mixture of laboratory end-members at a cited mode, weighted by cross-section,
    /// volume fraction over diameter (Hapke 2012, its section not read, as
    /// decision-p14-t49i-rock-mixtures §2.1 states it): w̄ =
    /// Σ (fⱼ ÷ Dⱼ) wⱼ ÷ Σ (fⱼ ÷ Dⱼ), each part at the form's grain or at
    /// [`FINE_GRAIN_DIAMETER`] ([`MixturePart`]; P14.T49.i).
    Mixture {
        /// The parts, each once per grain class.
        parts: &'static [MixturePart],
    },
    /// Measured reflectance levels at a stated geometry, a piecewise-linear spectrum inverted for
    /// w under the reference form's photometry, with no grain size to rescale (SO₂ frost;
    /// [`ReflectanceLevels`]; P14.T49.i).
    Levels {
        /// The levels.
        levels: &'static ReflectanceLevels,
    },
}

/// The diameter of a mixture's fine phases, 1 µm: the smallest grain at which the reference form's
/// equivalent slab and Hapke's extinction efficiency `Q_E` ≈ 1 are still used, a size parameter
/// πD ÷ λ of 7.1, 5.7 and 4.9 in B, V and R (decision-p14-t49i-rock-mixtures §2.3, `Adopted`,
/// never fitted). The petrography it stands beside puts the matrix opaques and organics of the CI,
/// CM and CV chondrites at sub-µm to a few µm (each mixture row's sources). Beyond about 1.5 µm a
/// 1 µm grain is below geometric optics (πD ÷ λ < 2), and the weights stay geometric there: an
/// approximation in the infrared bins of the matrix-rich rows.
pub const FINE_GRAIN_DIAMETER: Metres = Metres::new(1.0e-6);

/// A spectrometer of the USGS Spectroscopy Laboratory, which fixes a spectrum's measured quantity
/// and geometry (Kokaly et al. 2017, USGS Data Series 1035).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Spectrometer {
    /// The Beckman 5270 with its integrating sphere, 0.2–3 µm: inverted as a
    /// directional-hemispherical reflectance at the adopted incidence of
    /// [`BECKMAN_INCIDENCE_DEG`], though DS 1035 describes its powder measurements as biconical
    /// reflectance factors (p. 4; plan 14's deferred findings from P14.T49.i).
    Beckman,
    /// The ASD (Analytical Spectral Devices) spectrometers: a reflectance factor at the geometry of
    /// [`ASD_GEOMETRY`], 0.35–2.5 µm.
    Asd,
}

/// The Beckman's beam's incidence on the sample, degrees: 10, adopted, the lane's prototype's
/// value, at which a spectrum is inverted as the similarity relation's directional-hemispherical
/// albedo (P14.T49.i).
///
/// No source gives it: DS 1035 states no angle for the Beckman (Kokaly et al. 2017, p. 4, which
/// refers the instrument to Clark et al. 1990b, not read here), and says of its powder
/// measurements, "Because the effect of the sample holder walls, these Beckman measurements are
/// described as biconical reflectance factors (BCRF)". Normal incidence in its place moves a
/// spherical albedo by at most 0.0023.
pub const BECKMAN_INCIDENCE_DEG: f64 = 10.0;

/// A measurement's geometry: the incidence and emission angles from the surface's normal and the
/// azimuth between the planes of incidence and emission, degrees.
///
/// Built by [`Geometry::new`], which holds each angle to its range.
#[derive(Debug, Clone, Copy, PartialEq)]
#[expect(
    clippy::struct_field_names,
    reason = "each angle carries its unit, degrees, in its name, as the crate's rule for physical \
              quantities asks, and the names reach hyperion-fit's fingerprint through `Debug`"
)]
pub struct Geometry {
    incidence_deg: f64,
    emission_deg: f64,
    azimuth_deg: f64,
}

/// Why [`Geometry::new`] refused a geometry: the angle outside its range, degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildGeometryError {
    /// The incidence angle is not in [0, 90).
    Incidence(f64),
    /// The emission angle is not in [0, 90).
    Emission(f64),
    /// The azimuth is not in [0, 180].
    Azimuth(f64),
}

impl std::fmt::Display for BuildGeometryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Incidence(deg) => write!(f, "the incidence angle {deg}° is not in [0, 90)"),
            Self::Emission(deg) => write!(f, "the emission angle {deg}° is not in [0, 90)"),
            Self::Azimuth(deg) => write!(f, "the azimuth {deg}° is not in [0, 180]"),
        }
    }
}

impl std::error::Error for BuildGeometryError {}

impl Geometry {
    /// The geometry of incidence `incidence_deg` and emission `emission_deg`, each from the
    /// surface's normal in [0, 90), and azimuth `azimuth_deg` between their planes in [0, 180]:
    /// 180 with the source and the detector on opposite sides of the normal, where the phase angle
    /// is i + e.
    ///
    /// # Errors
    ///
    /// [`BuildGeometryError::Incidence`], [`BuildGeometryError::Emission`] or
    /// [`BuildGeometryError::Azimuth`] for the first angle outside its range, a NaN among them.
    ///
    /// # Examples
    ///
    /// A grazing beam is refused; the USGS laboratory's ASD geometry is not:
    ///
    /// ```
    /// use hyperion_sim::substance::optics::{BuildGeometryError, Geometry};
    ///
    /// assert!(matches!(
    ///     Geometry::new(90.0, 0.0, 0.0),
    ///     Err(BuildGeometryError::Incidence(_))
    /// ));
    /// let asd = Geometry::new(20.0, 30.0, 90.0)?;
    /// assert!((asd.incidence_deg() + asd.emission_deg() - 50.0).abs() < 1e-12);
    /// # Ok::<(), BuildGeometryError>(())
    /// ```
    pub const fn new(
        incidence_deg: f64,
        emission_deg: f64,
        azimuth_deg: f64,
    ) -> Result<Self, BuildGeometryError> {
        if !(incidence_deg >= 0.0 && incidence_deg < 90.0) {
            return Err(BuildGeometryError::Incidence(incidence_deg));
        }
        if !(emission_deg >= 0.0 && emission_deg < 90.0) {
            return Err(BuildGeometryError::Emission(emission_deg));
        }
        if !(azimuth_deg >= 0.0 && azimuth_deg <= 180.0) {
            return Err(BuildGeometryError::Azimuth(azimuth_deg));
        }
        Ok(Self {
            incidence_deg,
            emission_deg,
            azimuth_deg,
        })
    }

    /// The incidence angle i, degrees, in [0, 90).
    #[must_use]
    pub const fn incidence_deg(&self) -> f64 {
        self.incidence_deg
    }

    /// The emission angle e, degrees, in [0, 90).
    #[must_use]
    pub const fn emission_deg(&self) -> f64 {
        self.emission_deg
    }

    /// The azimuth between the planes of incidence and emission, degrees, in [0, 180].
    #[must_use]
    pub const fn azimuth_deg(&self) -> f64 {
        self.azimuth_deg
    }
}

/// The ASD's geometry in the USGS laboratory: incidence 20°, emission 30°, in planes 90° apart, a
/// phase angle of 35.5° (Kokaly et al. 2017, USGS Data Series 1035, p. 7).
pub const ASD_GEOMETRY: Geometry = match Geometry::new(20.0, 30.0, 90.0) {
    Ok(geometry) => geometry,
    Err(_) => panic!("the ASD's geometry is inside its ranges"),
};

/// A spectrum of the USGS Spectral Library version 7, splib07a (Kokaly et al. 2017, USGS Data
/// Series 1035; U.S. public domain), by its file in the release's ASCII data, which
/// `hyperion-fit`'s `usgs_splib07a` dataset commits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LabSpectrum {
    file: &'static str,
    spectrometer: Spectrometer,
}

impl LabSpectrum {
    /// The spectrum in `file` (`splib07a_<record>.txt`), measured by `spectrometer`.
    #[must_use]
    pub const fn new(file: &'static str, spectrometer: Spectrometer) -> Self {
        Self { file, spectrometer }
    }

    /// Its file's name in the release, `splib07a_<record>.txt`.
    #[must_use]
    pub const fn file(&self) -> &'static str {
        self.file
    }

    /// The spectrometer that measured it.
    #[must_use]
    pub const fn spectrometer(&self) -> Spectrometer {
        self.spectrometer
    }
}

/// How an end-member's measured w is taken from its laboratory grains to a row's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GrainScaling {
    /// Through the equivalent slab's internal transmission at a stated real index n and the
    /// laboratory grains' diameter: the measured w gives Θ by the slab's own relation, and the
    /// row's grains take Θ^(D ÷ `D_lab`). A w below the slab's surface term `S_e` at k = 0, which
    /// no Θ gives, is held as measured.
    Slab {
        /// The real index n, dimensionless, at least 1.
        real_index: f64,
        /// The laboratory grains' diameter, `D_lab`.
        lab_grain_diameter: Metres,
    },
    /// Held as measured at every grain size: an opaque phase, which the slab would brighten at a
    /// finer grain against Hunt, Salisbury and Lenhoff 1971's observation that opaque powders
    /// darken as they get finer (as splib07a's magnetite description quotes them), or a phase whose
    /// laboratory grain size is not stated (the end-member's source says which).
    Held,
}

/// A laboratory end-member: one or more splib07a spectra of one material, each inverted for w at
/// its own geometry, their mean where there are several, and how w is taken to a row's grains.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EndMember {
    name: &'static str,
    spectra: &'static [LabSpectrum],
    scaling: GrainScaling,
    source: Source,
}

impl EndMember {
    /// The end-member `name`, of `spectra` (their w averaged), scaled by `scaling`, described by
    /// `source`.
    #[must_use]
    pub const fn new(
        name: &'static str,
        spectra: &'static [LabSpectrum],
        scaling: GrainScaling,
        source: Source,
    ) -> Self {
        Self {
            name,
            spectra,
            scaling,
            source,
        }
    }

    /// Its name: the material and its sample, as splib07a names them.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Its spectra, whose w are averaged: one, or several fractions of one size class.
    #[must_use]
    pub const fn spectra(&self) -> &'static [LabSpectrum] {
        self.spectra
    }

    /// How its w is taken to a row's grains.
    #[must_use]
    pub const fn scaling(&self) -> GrainScaling {
        self.scaling
    }

    /// Its source: the sample's description, its real index and its grain size, with their
    /// basis.
    #[must_use]
    pub const fn source(&self) -> Source {
        self.source
    }
}

/// The grain class of a mixture's part.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GrainClass {
    /// The reference form's grains (70 µm for a regolith).
    Form,
    /// The fine phases' grains, [`FINE_GRAIN_DIAMETER`], at their measured w
    /// (decision-p14-t49i-rock-mixtures §2.2–§2.4).
    Fine,
}

/// A part of an intimate mixture: an end-member, its volume fraction in the mode and its grain
/// class.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MixturePart {
    end_member: &'static EndMember,
    volume_fraction: f64,
    grain: GrainClass,
}

impl MixturePart {
    /// `volume_fraction` of `end_member` at grain class `grain`.
    #[must_use]
    pub const fn new(
        end_member: &'static EndMember,
        volume_fraction: f64,
        grain: GrainClass,
    ) -> Self {
        Self {
            end_member,
            volume_fraction,
            grain,
        }
    }

    /// The end-member.
    #[must_use]
    pub const fn end_member(&self) -> &'static EndMember {
        self.end_member
    }

    /// Its volume fraction in the mode, dimensionless, positive; a row's parts sum to 1 to the
    /// rounding of the stated fractions (the mixing rule normalises them).
    #[must_use]
    pub const fn volume_fraction(&self) -> f64 {
        self.volume_fraction
    }

    /// Its grain class.
    #[must_use]
    pub const fn grain(&self) -> GrainClass {
        self.grain
    }
}

/// Measured reflectance levels: a piecewise-linear spectrum through its knots, the reflectance
/// factor at a stated geometry, held at its end knots' values beyond them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReflectanceLevels {
    knots: &'static [(f64, f64)],
    geometry: Geometry,
}

impl ReflectanceLevels {
    /// The levels of `knots`, (wavelength nm, reflectance factor) of rising wavelength, measured
    /// at `geometry`.
    #[must_use]
    pub const fn new(knots: &'static [(f64, f64)], geometry: Geometry) -> Self {
        Self { knots, geometry }
    }

    /// Its knots: (wavelength nm, reflectance factor), the wavelength rising.
    #[must_use]
    pub const fn knots(&self) -> &'static [(f64, f64)] {
        self.knots
    }

    /// The geometry of the measurement.
    #[must_use]
    pub const fn geometry(&self) -> Geometry {
        self.geometry
    }
}

/// An infrared set: a row's optical constants beyond 780 nm, from its primaries, as
/// `hyperion-fit`'s datasets hold them (P14.T49.i; decision-composition §5). Each piece of a set
/// holds at one reference temperature, which the row's sources state where its source gives one:
/// a set may join pieces of different temperatures (water's 266 K and 100 K, methane's k at 40 K
/// and n at 90 K), and ammonia's states none. Closed, for the composition audit: a row gains one
/// where an open primary of its solid's infrared is found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum InfraredIndex {
    /// Ice Ih: Warren and Brandt 2008 at 266 K, with Grundy and Schmitt 1998's absorption at 100 K
    /// over 0.9623–2.742 µm.
    WaterIce,
    /// CO₂ ice: Warren 1986's compilation of laboratory measurements, its emphasis on temperatures
    /// above 77 K.
    CarbonDioxideIce,
    /// Methane ice I: Grundy, Schmitt and Quirico 2002's absorption at 40 K, with Martonchik and
    /// Orton 1994's real index.
    MethaneIce,
    /// β nitrogen ice: Grundy, Schmitt and Quirico 1993's 2.15 µm band at 41 K, transparent
    /// elsewhere.
    NitrogenIce,
    /// α-CO ice: Gerakines, Materese and Hudson 2023's fundamental band at 25 K, transparent below
    /// it.
    CarbonMonoxideIce,
    /// Ammonia ice: Martonchik, Orton and Appleby 1984's measured infrared from 1.462 µm, at a
    /// temperature its copy does not state.
    AmmoniaIce,
    /// Titan tholin: Khare et al. 1984, at room temperature.
    Tholin,
    /// Amorphous `MgSiO₃`: Dorschner et al. 1995, at room temperature.
    EnstatiteGlass,
    /// Amorphous `Mg₂SiO₄`: Jäger et al. 2003, at room temperature.
    AmorphousForsterite,
    /// A terrestrial basalt: Pollack, Toon and Khare 1973.
    Basalt,
    /// A terrestrial basaltic glass: Pollack, Toon and Khare 1973.
    BasalticGlass,
    /// Cubic `ZnS`: Palik and Addamiano 1985, as Kitzmann and Heng 2018 compile it.
    ZincSulphide,
    /// Mars's dust: the index table Wolff et al. distribute, R08's `mars-dust.json`'s own source.
    MarsDust,
}

impl InfraredIndex {
    /// The `hyperion-fit` datasets it is read from, under `crates/hyperion-fit/data/`.
    #[must_use]
    pub const fn datasets(self) -> &'static [&'static str] {
        match self {
            Self::WaterIce => &["warren_brandt_2008_h2o", "grundy_schmitt_1998_h2o"],
            Self::CarbonDioxideIce => &["warren_1986_co2"],
            Self::MethaneIce => &["grundy_2002_ch4", "martonchik_orton_1994_ch4"],
            Self::NitrogenIce => &["grundy_1993_n2"],
            Self::CarbonMonoxideIce => &["gerakines_2023_co"],
            Self::AmmoniaIce => &["martonchik_1984_nh3"],
            Self::Tholin => &["khare_1984_tholin"],
            Self::EnstatiteGlass => &["dorschner_1995_mgsio3"],
            Self::AmorphousForsterite => &["jager_2003_mg2sio4"],
            Self::Basalt => &["pollack_1973_basalt"],
            Self::BasalticGlass => &["pollack_1973_basaltic_glass"],
            Self::ZincSulphide => &["palik_1985_zns"],
            Self::MarsDust => &["wolff_2009_mars_dust"],
        }
    }
}

/// What a row's albedo is the albedo of: the form of the substance R10 takes its entry's albedo
/// for (R10.T1.a, "the per-form factor"; decision-r10-limb §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ReferenceForm {
    /// Fresh snow or fine frost of the ice, against which R10 takes its forms' factors (glacier,
    /// snow, sea ice): grains of [`FRESH_SNOW_GRAIN_DIAMETER`] under [`FRESH_SNOW`]'s photometry.
    Frost,
    /// A fresh particulate regolith of the solid's grains: grains of the lunar soil's 70 µm, the
    /// grain the thermal columns' silicate regolith takes (`substance::thermal`'s reference
    /// form, the one constant), under [`LUNAR_REGOLITH`]'s photometry.
    Regolith,
}

impl ReferenceForm {
    /// The reference form of a substance of mechanics family `family`: an ice's frost, any other
    /// solid's regolith, the split of R10's `PhaseLaw::for_family`, which a test holds.
    #[must_use]
    pub const fn of_family(family: MechanicsFamily) -> Self {
        match family {
            MechanicsFamily::WaterIce | MechanicsFamily::VolatileIce => Self::Frost,
            MechanicsFamily::Silicate
            | MechanicsFamily::Metal
            | MechanicsFamily::Salt
            | MechanicsFamily::Organic => Self::Regolith,
        }
    }

    /// The diameter of its grains, Hapke's D.
    #[must_use]
    pub const fn grain_diameter(self) -> Metres {
        match self {
            Self::Frost => FRESH_SNOW_GRAIN_DIAMETER,
            Self::Regolith => super::thermal::LUNAR_SOIL_GRAIN_DIAMETER,
        }
    }

    /// Its photometry.
    #[must_use]
    pub const fn photometry(self) -> &'static Photometry {
        match self {
            Self::Frost => &FRESH_SNOW,
            Self::Regolith => &LUNAR_REGOLITH,
        }
    }
}

/// Fresh snow's grain diameter, 100 µm: twice Wiscombe and Warren's representative average grain
/// radius of new snow, 50 µm, inside the 20–100 µm their sources give for it (Wiscombe and Warren
/// 1980, J. Atmos. Sci. 37, 2712, p. 2714, §3b).
pub const FRESH_SNOW_GRAIN_DIAMETER: Metres = Metres::new(100.0e-6);

/// A single-particle phase function p(g), of the phase angle g, normalised to 1 over the sphere.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PhaseFunction {
    /// One Henyey–Greenstein lobe, p(g) = (1 − ξ²) ÷ (1 + 2ξ cos g + ξ²)^{3/2}, ξ the mean cosine
    /// of the scattering angle: forward-scattering for ξ > 0. It is Domingue, Hartman and
    /// Verbiscer 1997's two-parameter double Henyey–Greenstein function with its backscattering
    /// share c₂ = 0 (Icarus 128, 28, p. 30).
    HenyeyGreenstein {
        /// ξ, dimensionless, in (−1, 1).
        asymmetry: f64,
    },
    /// A two-term Legendre polynomial in the phase angle, p(g) = 1 + b cos g + c (3 cos² g − 1) ÷
    /// 2, whose zero-phase value is 1 + b + c: backscattering for b > 0 (Helfenstein and Veverka
    /// 1987, p. 217: "The particle phase function is a Legendre polynomial with first and second
    /// order coefficients b and c").
    Legendre {
        /// b, the first-order coefficient, dimensionless, in [−1, 1] for the polynomial to stay
        /// non-negative with c.
        b: f64,
        /// c, the second-order coefficient, dimensionless; p is non-negative over every phase for
        /// the published pairs (H&V's 0.33 and 0.37 give at least 0.766, at cos g −0.297).
        c: f64,
    },
}

impl PhaseFunction {
    /// Its asymmetry ⟨cos Θ⟩, the mean cosine of the scattering angle Θ = π − g: ξ for the
    /// Henyey–Greenstein lobe, and −b ÷ 3 for the Legendre polynomial, whose first-order term in
    /// cos g is −b cos Θ, ⅓ of whose coefficient is the mean cosine.
    #[must_use]
    pub fn asymmetry(self) -> f64 {
        match self {
            Self::HenyeyGreenstein { asymmetry } => asymmetry,
            Self::Legendre { b, .. } => -b / 3.0,
        }
    }

    /// p(g) at `cos_g`, the cosine of the phase angle.
    #[must_use]
    pub fn at(self, cos_g: f64) -> f64 {
        match self {
            Self::HenyeyGreenstein { asymmetry: xi } => {
                let base = 1.0 + 2.0 * xi * cos_g + xi * xi;
                (1.0 - xi * xi) / (base * base.sqrt())
            }
            Self::Legendre { b, c } => 1.0 + b * cos_g + c * (3.0 * cos_g * cos_g - 1.0) / 2.0,
        }
    }
}

/// A reference form's shadow-hiding opposition effect, B(g) = `B₀` ÷ (1 + tan(g ÷ 2) ÷ h), which
/// multiplies the singly scattered light (Hapke 1986, Icarus 67, 264).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OppositionEffect {
    /// None: `B₀` = 0.
    None,
    /// Hapke's 1986 form, its amplitude `B₀` = S(0) ÷ (w p(0)) following the grain's own w, S(0)
    /// being the singly scattered light reflected by the grains' first surfaces at zero phase
    /// (Helfenstein and Veverka 1987, p. 218: "The total amplitude of the opposition surge, `B₀` =
    /// S(0)/w(1+b+c)"), and at most 1: "Physical considerations restrict the OE amplitudes to the
    /// range … 0 ≤ `B_S0` ≤ 1" (Hapke, Denevi, Sato, Braden and Robinson 2012, JGR 117, E00H15,
    /// ¶21), the first surfaces reflecting no more than the grain scatters
    /// ([`Photometry::opposition_amplitude`]).
    ShadowHiding {
        /// S(0), dimensionless, at least 0: the share of the grain's zero-phase single scattering
        /// reflected by its first surface, w p(0) at most for a physical grain.
        surface_share: f64,
        /// h, the surge's angular width parameter, dimensionless and positive: B(g) falls to half
        /// its amplitude at tan(g ÷ 2) = h.
        width: f64,
    },
}

/// The photometric parameters a reference form's albedo is computed with: its single-particle
/// phase function and its opposition effect, both of a measured surface's Hapke solution, whose
/// own single-scattering albedo is not used (each row takes its grains' own).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Photometry {
    phase_function: PhaseFunction,
    opposition: OppositionEffect,
    source: Source,
}

impl Photometry {
    /// The single-particle phase function.
    #[must_use]
    pub const fn phase_function(&self) -> PhaseFunction {
        self.phase_function
    }

    /// The opposition effect.
    #[must_use]
    pub const fn opposition(&self) -> OppositionEffect {
        self.opposition
    }

    /// Where it comes from.
    #[must_use]
    pub const fn source(&self) -> Source {
        self.source
    }

    /// The amplitude `B₀` of the opposition effect for grains of single-scattering albedo `w`: 0
    /// for none, and for Hapke's 1986 form S(0) ÷ (w p(0)), held at 1 where the grain is too dark
    /// for its first surfaces to reflect S(0) (w p(0) < S(0)), and 0 for a `w` that is not
    /// positive.
    ///
    /// The bound matters only for dark grains: under [`LUNAR_REGOLITH`] S(0) ÷ p(0) is 0.459, so
    /// it holds for w below it. The Moon's fitted amplitudes above 1 carry the coherent
    /// backscatter, which this model leaves out (Hapke et al. 2012, ¶31).
    #[must_use]
    pub fn opposition_amplitude(&self, w: f64) -> f64 {
        match self.opposition {
            OppositionEffect::None => 0.0,
            OppositionEffect::ShadowHiding { surface_share, .. } => {
                if w > 0.0 {
                    let amplitude = surface_share / (w * self.phase_function.at(1.0));
                    if amplitude > 1.0 { 1.0 } else { amplitude }
                } else {
                    0.0
                }
            }
        }
    }
}

/// Fresh snow's photometry: newly fallen snow's Hapke solution with no opposition effect, one
/// forward-scattering Henyey–Greenstein lobe of asymmetry 0.503 (Domingue, Hartman and Verbiscer
/// 1997, Icarus 128, 28, Table III, p. 35: w 0.998, b₂ 0.503, c₂ 0 and θ̄ 0 with `b₀` = 0, fitted
/// with Hapke's 1984 and 1986 equations, p. 30, to Middleton and Mungall 1952's goniophotometry of
/// natural snow, which "show no opposition surge", p. 30).
///
/// Terrestrial snow and frost scatter forward, unlike the icy satellites' regoliths (their
/// abstract, p. 28, and p. 34; Verbiscer and Veverka 1990, Icarus 88, 418, abstract). It is R10's
/// reference form of every ice (R10.T1.a): what an ice's entry `A_N` is, against which the forms'
/// factors are taken; an airless frost's backscattering is its form's law and the body's scale.
pub const FRESH_SNOW: Photometry = Photometry {
    phase_function: PhaseFunction::HenyeyGreenstein { asymmetry: 0.503 },
    opposition: OppositionEffect::None,
    source: Source {
        covers: "phase function, opposition effect",
        citation: "Domingue, Hartman and Verbiscer 1997, Icarus 128, 28, Table III, p. 35 (newly \
                   fallen snow with b₀ = 0: b₂ 0.503, c₂ 0, θ̄ 0; its w 0.998 not used) and p. 30 \
                   (the 2P-HG function; Hapke's 1984 and 1986 equations fitted to Middleton and \
                   Mungall 1952's snow, which shows no opposition surge)",
        basis: super::Basis::Fact,
    },
};

/// The lunar regolith's photometry: the Moon's average terrains' Hapke solution, a Legendre phase
/// function with b 0.33 and c 0.37 and Hapke's 1986 shadow-hiding opposition effect with S(0) 0.78
/// and h 0.06 (Helfenstein and Veverka 1987, "Photometric properties of lunar terrains derived from
/// Hapke's equation", Reports of Planetary Geology and Geophysics Program 1986, NASA, p. 218,
/// Table 1, "Disk-Resolved Average": w 0.25 and θ̄ 20.6°, neither used), fitted to the
/// twenty-five lunar terrains of normal albedo 0.10–0.12 in Shorthill et al.'s catalogue.
///
/// It is the photometry of the particulate forms of R10's `Regolith` law, the Moon's (R10's
/// Design note 8), for a fresh particulate surface of any solid that is not an ice. It was fitted
/// to grains of w 0.12–0.33 (the dark and bright terrains' solutions bracket the average's), and a
/// brighter grain takes it extrapolated in w, up to [`REGOLITH_MAX_SINGLE_SCATTERING_ALBEDO`],
/// an energy bound and not a calibration limit.
pub const LUNAR_REGOLITH: Photometry = Photometry {
    phase_function: PhaseFunction::Legendre { b: 0.33, c: 0.37 },
    opposition: OppositionEffect::ShadowHiding {
        surface_share: 0.78,
        width: 0.06,
    },
    source: Source {
        covers: "phase function, opposition effect",
        citation: "Helfenstein and Veverka 1987, Reports of Planetary Geology and Geophysics \
                   Program 1986 (NASA; NTRS 19870013999), p. 218, Table 1, disk-resolved average \
                   terrains: b 0.33, c 0.37, S(0) 0.78, h 0.06 (w 0.25 and θ̄ 20.6° not used), \
                   and B₀ = S(0)/w(1+b+c), its text",
        basis: super::Basis::Fact,
    },
};

/// The highest single-scattering albedo of a grain under [`LUNAR_REGOLITH`]'s photometry: 0.9995.
///
/// The lunar solution is the Moon's absorbing, backscattering grains', fitted at w 0.12–0.33. With
/// isotropic multiple scattering its directional-hemispherical albedo at normal incidence passes 1
/// between w 0.9995 (0.994) and 0.9998 (1.017), computed by this lane with the task's own
/// quadrature: a grain nearly transparent in a band (corundum, quartz, the halides, rutile in V
/// and R) is outside it, and is not modelled under it. A choice from that computation, an energy
/// bound and not a calibration limit, which no measured figure gives.
pub const REGOLITH_MAX_SINGLE_SCATTERING_ALBEDO: f64 = 0.9995;

/// How directly a row's input was obtained: an index file's `provenance`, or the lane's finding
/// for an input of another kind (decision-p14-t49i-rock-mixtures §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) enum InputProvenance {
    /// Measured or derived from measurements of the substance.
    Measured,
    /// A stand-in: an estimate, a named analogue, an upper bound or an unvalidated mixture.
    StandIn,
}

/// A row's condensed optics: its reference form, what its grains' optics are computed from, and
/// the fitted values of `tables::substance_albedo` (the [module documentation](self)).
#[derive(Debug, Clone, PartialEq)]
pub struct CondensedOptics {
    pub(super) key: SubstanceKey,
    pub(super) form: ReferenceForm,
    pub(super) input: OpticsInput,
    pub(super) provenance: InputProvenance,
    pub(super) values: &'static [f64; ALBEDO_ROW_LEN],
    pub(super) sources: &'static [Source],
}

/// The bands of a `[B, V, R]` triple, as the palette orders them.
const BANDS: usize = 3;

impl CondensedOptics {
    /// The row's key.
    #[must_use]
    pub const fn key(&self) -> SubstanceKey {
        self.key
    }

    /// Its reference form.
    #[must_use]
    pub const fn reference_form(&self) -> ReferenceForm {
        self.form
    }

    /// What its grains' optics are computed from.
    #[must_use]
    pub const fn input(&self) -> OpticsInput {
        self.input
    }

    /// The name of R08's index file its grains' visible optics are computed from, in
    /// `apps/hyperion/src/renderer/src/view/atmosphere/materials/`, or `None` for a row of another
    /// input ([`OpticsInput`]).
    #[must_use]
    pub const fn index_file(&self) -> Option<&'static str> {
        match self.input {
            OpticsInput::Index { file, .. }
            | OpticsInput::SizedIndex { file, .. }
            | OpticsInput::OpaqueSlab { file } => Some(file),
            OpticsInput::Spectrum { .. }
            | OpticsInput::Mixture { .. }
            | OpticsInput::Levels { .. } => None,
        }
    }

    /// The infrared set its optics beyond 780 nm are computed from, or `None` where its data end
    /// at 780 nm or come from no index file ([`data_reach_nm`](Self::data_reach_nm)).
    #[must_use]
    pub const fn infrared_source(&self) -> Option<InfraredIndex> {
        match self.input {
            OpticsInput::Index { infrared, .. } | OpticsInput::SizedIndex { infrared, .. } => {
                infrared
            }
            OpticsInput::OpaqueSlab { .. }
            | OpticsInput::Spectrum { .. }
            | OpticsInput::Mixture { .. }
            | OpticsInput::Levels { .. } => None,
        }
    }

    /// The diameter of its grains, Hapke's D: its input's for a [`OpticsInput::SizedIndex`], else
    /// its reference form's ([`ReferenceForm::grain_diameter`]); a mixture's fine parts take
    /// [`FINE_GRAIN_DIAMETER`].
    #[must_use]
    pub const fn grain_diameter(&self) -> Metres {
        match self.input {
            OpticsInput::SizedIndex { grain_diameter, .. } => grain_diameter,
            OpticsInput::Index { .. }
            | OpticsInput::OpaqueSlab { .. }
            | OpticsInput::Spectrum { .. }
            | OpticsInput::Mixture { .. }
            | OpticsInput::Levels { .. } => self.form.grain_diameter(),
        }
    }

    /// The laboratory spectra its optics are computed from, each once, in the order its input
    /// names them: an end-member's, every part's of a mixture, and none for another input.
    #[must_use]
    pub fn lab_spectra(&self) -> Vec<LabSpectrum> {
        let mut spectra: Vec<LabSpectrum> = Vec::new();
        let mut add = |end_member: &EndMember| {
            for spectrum in end_member.spectra() {
                if !spectra.contains(spectrum) {
                    spectra.push(*spectrum);
                }
            }
        };
        match self.input {
            OpticsInput::Spectrum { end_member } => add(end_member),
            OpticsInput::Mixture { parts } => parts.iter().for_each(|p| add(p.end_member())),
            OpticsInput::Index { .. }
            | OpticsInput::SizedIndex { .. }
            | OpticsInput::OpaqueSlab { .. }
            | OpticsInput::Levels { .. } => {}
        }
        spectra
    }

    /// Whether its input is a stand-in, which a reader labels approximate (its sources give the
    /// reason): an index file's `standIn`, or the lane's finding; a named analogue's spectrum; a
    /// mixture that misses a band its class is tested against, or has none; an upper bound; or
    /// levels from an abstract (decision-p14-t49i-rock-mixtures §4).
    #[must_use]
    pub const fn is_stand_in(&self) -> bool {
        matches!(self.provenance, InputProvenance::StandIn)
    }

    /// The triple of values at `offset` in the row.
    #[must_use]
    fn triple(&self, offset: usize) -> [f64; 3] {
        [
            self.values[offset],
            self.values[offset + 1],
            self.values[offset + 2],
        ]
    }

    /// The single-scattering albedo w of its reference grains in B, V and R: Hapke's equivalent
    /// slab, in [0, 1].
    #[must_use]
    pub fn single_scattering_albedo_bvr(&self) -> [f64; 3] {
        self.triple(0)
    }

    /// The normal albedo `A_N` in B, V and R: the radiance factor at i = e = g = 0, what its
    /// palette entry's `normal_albedo_bvr` holds; finite, at least 0 and at most 1.35.
    #[must_use]
    pub fn normal_albedo_bvr(&self) -> [f64; 3] {
        self.triple(BANDS)
    }

    /// The spherical albedo in B, V and R: the reflected share of the light on a sphere of the
    /// reference form, 2 ∫ `A_h`(μ₀) μ₀ dμ₀, its Bond albedo in each band, by the similarity
    /// relation (the [module documentation](self)): what P14.T51.b weights by the host's light,
    /// never `A_N`; 1 for a lossless grain.
    #[must_use]
    pub fn spherical_albedo_bvr(&self) -> [f64; 3] {
        self.triple(2 * BANDS)
    }

    /// The directional-hemispherical albedo `A_h`(μ₀) in B, V and R at the `incidence`th of
    /// [`HEMISPHERICAL_INCIDENCE_COSINES`]: the share of a collimated beam at incidence cosine μ₀
    /// reflected into the whole upper hemisphere, by the similarity relation; at most 1, and 1 for
    /// a lossless grain. `None` past the fourth.
    #[must_use]
    pub fn directional_hemispherical_albedo_bvr(&self, incidence: usize) -> Option<[f64; 3]> {
        (incidence < HEMISPHERICAL_INCIDENCE_COSINES.len())
            .then(|| self.triple(3 * BANDS + BANDS * incidence))
    }

    /// Its spherical albedo in each spectral bin ([`SPECTRAL_BIN_EDGES_NM`]): the mean over the
    /// bin, uniform in wavelength, of the spectral spherical albedo of the reference form by the
    /// similarity relation, over the part of the bin its data cover, or the nearest covered bin's
    /// value where they cover none of it (the [module documentation](self)); each in [0, 1], and
    /// 1 for a lossless grain. The first and last bins stand for the light below 0.3 µm and above
    /// 5 µm.
    ///
    /// # Examples
    ///
    /// Water frost is bright in the visible and dark in its 3 µm band, where every grain absorbs:
    ///
    /// ```
    /// use hyperion_sim::substance::by_key;
    /// use hyperion_sim::substance::optics::{BVR_BINS, SPECTRAL_BIN_EDGES_NM};
    ///
    /// # fn main() {
    /// #     assert!(example().is_some(), "every value the example reads is there");
    /// # }
    /// # fn example() -> Option<()> {
    /// let frost = by_key("H2O")?.substance().condensed_optics()?;
    /// let bins = frost.spherical_albedo_bins();
    /// assert!(BVR_BINS.iter().all(|&bin| bins[bin] > 0.95));
    /// let band = SPECTRAL_BIN_EDGES_NM.iter().position(|&edge| edge >= 2_800.0)?;
    /// assert!(bins[band] < 0.1, "2.8–3.5 µm: {}", bins[band]);
    /// # Some(())
    /// # }
    /// ```
    #[must_use]
    pub fn spherical_albedo_bins(&self) -> [f64; SPECTRAL_BINS] {
        core::array::from_fn(|bin| self.values[SPECTRAL_BINS_OFFSET + bin])
    }

    /// The reach of its data, nm: the first and last wavelength over which its n and k are given,
    /// measured or stated (the [module documentation](self)), at least 300 and at most 5,000.
    #[must_use]
    pub fn data_reach_nm(&self) -> [f64; 2] {
        [
            self.values[DATA_REACH_OFFSET],
            self.values[DATA_REACH_OFFSET + 1],
        ]
    }

    /// Where its values come from: the index file's paper, its infrared set's where it has one,
    /// the reference form's photometry and grain size, and the fit.
    #[must_use]
    pub const fn sources(&self) -> &'static [Source] {
        self.sources
    }
}

impl CondensedColumns {
    /// The photometric law of the row in R10's terrain, its palette entry's `phase_row` code:
    /// `PhaseLaw::for_family` of its mechanics family, never `Lambert` (the [module
    /// documentation](self)).
    #[must_use]
    pub const fn phase_law(&self) -> PhaseLaw {
        PhaseLaw::for_family(self.mechanics)
    }
}

impl Substance {
    /// Its condensed optics, for a row whose solid's optics are modelled (the [module
    /// documentation](self)); `None` where they are not.
    ///
    /// # Examples
    ///
    /// Fresh water snow is brighter in every band than fresh basalt regolith, and both are within
    /// the energy bound:
    ///
    /// ```
    /// use hyperion_sim::substance::by_key;
    ///
    /// # fn main() {
    /// #     assert!(example().is_some(), "every value the example reads is there");
    /// # }
    /// # fn example() -> Option<()> {
    /// let snow = by_key("H2O")?.substance().condensed_optics()?;
    /// let rock = by_key("basalt")?.substance().condensed_optics()?;
    /// let (snow, rock) = (snow.normal_albedo_bvr(), rock.normal_albedo_bvr());
    /// assert!((0..3).all(|band| snow[band] > rock[band]));
    /// assert!(snow.iter().chain(&rock).all(|a| *a <= 1.35));
    /// // Silicon carbide has neither an index file nor an open spectrum: not modelled yet.
    /// assert!(by_key("SiC")?.substance().condensed_optics().is_none());
    /// # Some(())
    /// # }
    /// ```
    #[must_use]
    pub fn condensed_optics(&self) -> Option<&'static CondensedOptics> {
        OPTICS.iter().find(|optics| optics.key == self.key)
    }
}

/// Every row with condensed optics, in registry order, with its optics.
pub fn rows() -> impl Iterator<Item = (SubstanceId, &'static CondensedOptics)> {
    ids().filter_map(|id| id.substance().condensed_optics().map(|optics| (id, optics)))
}

/// Why [`resolve_palette_entry`] could not give an entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResolvePaletteEntryError {
    /// The row has no condensed columns: it cannot be a solid or a liquid on a surface.
    NotCondensed,
    /// The row's condensed optics are not modelled yet.
    OpticsNotModelled,
    /// The row's solid has no grain density at the temperature asked: a phase whose density was
    /// not read, or above its triple point.
    DensityNotModelled,
    /// A [`PaletteRole::Liquid`] entry needs its liquid's record and Fresnel albedo, P14.T49.h's.
    LiquidNotModelled,
    /// The temperature is not finite and positive.
    Temperature,
}

impl std::fmt::Display for ResolvePaletteEntryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::NotCondensed => "the substance has no condensed columns",
            Self::OpticsNotModelled => "the substance's condensed optics are not modelled",
            Self::DensityNotModelled => {
                "the substance's solid has no grain density at the temperature"
            }
            Self::LiquidNotModelled => "a liquid entry's record is not modelled",
            Self::Temperature => "the temperature is not finite and positive",
        })
    }
}

impl std::error::Error for ResolvePaletteEntryError {}

/// The palette entry of row `id` in `role` on a body where it lies at `temperature`: every value
/// the server resolves from the registry for a solid (decision-composition §1.7; R09's
/// `PaletteEntry`), with no liquid record.
///
/// - `normal_albedo_bvr`: its condensed optics' `A_N` ([`CondensedOptics::normal_albedo_bvr`]);
/// - `phase_row`: its law's code ([`CondensedColumns::phase_law`]);
/// - `grain_density_kg_m3`: its solid's grain density at `temperature`
///   ([`Substance::grain_density`]), an ice's crystal density where it lies;
/// - `transition` and `mechanics`: its condensed columns'.
///
/// A crust entry whose rock the body melts carries its melt's record, which is P14.T49.h's; a
/// caller adds it.
///
/// # Errors
///
/// [`ResolvePaletteEntryError::Temperature`] for a temperature that is not finite and positive,
/// [`ResolvePaletteEntryError::LiquidNotModelled`] for a [`PaletteRole::Liquid`] role,
/// [`ResolvePaletteEntryError::NotCondensed`] for a row with no condensed columns,
/// [`ResolvePaletteEntryError::OpticsNotModelled`] for a row whose optics are not modelled and
/// [`ResolvePaletteEntryError::DensityNotModelled`] where its solid has no density at
/// `temperature`.
///
/// # Examples
///
/// A Mars's polar CO₂ frost:
///
/// ```
/// use hyperion_sim::substance::{by_key, optics::resolve_palette_entry};
/// use hyperion_sim::units::Kelvin;
/// use hyperion_surface::field::PaletteRole;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let co2 = by_key("CO2").ok_or("CO2 is a row")?;
/// let entry = resolve_palette_entry(co2, PaletteRole::Ice, Kelvin::new(148.0))?;
/// assert_eq!(entry.substance.as_str(), "CO2");
/// assert!(entry.normal_albedo_bvr.iter().all(|a| *a > 0.9 && *a < 1.0));
/// assert!(entry.grain_density_kg_m3 > 1_600.0);
/// assert!(entry.liquid.is_none());
/// # Ok(())
/// # }
/// ```
pub fn resolve_palette_entry(
    id: SubstanceId,
    role: PaletteRole,
    temperature: Kelvin,
) -> Result<PaletteEntry, ResolvePaletteEntryError> {
    if !(temperature.value().is_finite() && temperature.value() > 0.0) {
        return Err(ResolvePaletteEntryError::Temperature);
    }
    if role == PaletteRole::Liquid {
        return Err(ResolvePaletteEntryError::LiquidNotModelled);
    }
    let row = id.substance();
    let condensed = row
        .condensed()
        .ok_or(ResolvePaletteEntryError::NotCondensed)?;
    let optics = row
        .condensed_optics()
        .ok_or(ResolvePaletteEntryError::OpticsNotModelled)?;
    let density = row
        .grain_density(CondensedPhase::Solid, temperature)
        .ok_or(ResolvePaletteEntryError::DensityNotModelled)?;
    Ok(PaletteEntry {
        substance: row.key(),
        role,
        normal_albedo_bvr: optics.normal_albedo_bvr(),
        phase_row: u8::from(condensed.phase_law()),
        grain_density_kg_m3: density.value(),
        transition: condensed.transition().temperature(),
        mechanics: condensed.mechanics(),
        liquid: None,
        // Not modelled until P14.T54.b fills it from the registry's composition
        // (decision-p14-t51d-weathering §1.6).
        feo_mass_fraction: None,
    })
}

#[cfg(test)]
mod tests {
    use hyperion_surface::material::PARTICULATE_NORMAL_ALBEDO_CEILING;

    use super::super::{Basis, by_key, ice_ih_density};
    use super::*;

    /// The optics of the row of key `key`.
    fn of(key: &str) -> &'static CondensedOptics {
        by_key(key)
            .and_then(|id| id.substance().condensed_optics())
            .unwrap_or_else(|| panic!("{key} has condensed optics"))
    }

    /// The modelled rows are rows of the registry with condensed columns, in registry order and
    /// each once, each in its family's reference form; each names its own index file once and its
    /// own infrared set, if any, once, or its laboratory spectra, or its levels.
    #[test]
    fn substance_optics_rows_are_condensed_registry_rows_in_order() {
        let mut last = None;
        let mut files = Vec::new();
        let mut sets = Vec::new();
        for optics in &OPTICS {
            let id = by_key(optics.key.as_str()).expect("an optics row is a registry row");
            assert!(last < Some(id), "{} is out of order", optics.key);
            last = Some(id);
            let condensed = id
                .substance()
                .condensed()
                .expect("it has condensed columns");
            assert_eq!(
                optics.reference_form(),
                ReferenceForm::of_family(condensed.mechanics()),
                "{}",
                optics.key
            );
            match (optics.index_file(), optics.input()) {
                (Some(file), _) => {
                    assert!(!files.contains(&file), "{file}");
                    files.push(file);
                }
                (None, OpticsInput::Spectrum { .. } | OpticsInput::Mixture { .. }) => {
                    assert!(!optics.lab_spectra().is_empty(), "{}", optics.key);
                }
                (None, OpticsInput::Levels { levels }) => {
                    assert!(levels.knots().len() >= 2, "{}", optics.key);
                }
                (None, input) => panic!("{}: {input:?} names no file", optics.key),
            }
            if let Some(infrared) = optics.infrared_source() {
                assert!(!sets.contains(&infrared), "{infrared:?}");
                sets.push(infrared);
                assert!(!infrared.datasets().is_empty(), "{infrared:?}");
            }
        }
        assert_eq!(rows().count(), OPTICS.len());
        assert!(rows().all(|(id, optics)| id.substance().key() == optics.key()));
    }

    /// The laboratory inputs are well formed: each spectrum's file is a splib07a file named for its
    /// spectrometer; a scaled end-member's index and grain are physical; a mixture's parts are
    /// positive, sum to 1 within the rounding of their stated fractions, name each end-member once
    /// per grain class, and put only held end-members at the fine grain; every laboratory row is a
    /// regolith; and levels rise in wavelength within [0, 1].
    #[test]
    fn substance_optics_the_laboratory_inputs_are_well_formed() {
        let check_end_member = |e: &EndMember| {
            assert!(!e.spectra().is_empty(), "{}", e.name());
            for spectrum in e.spectra() {
                let file = spectrum.file();
                let txt = std::path::Path::new(file)
                    .extension()
                    .is_some_and(|extension| extension == "txt");
                assert!(file.starts_with("splib07a_") && txt, "{file}");
                let marker = match spectrum.spectrometer() {
                    Spectrometer::Beckman => "_BECK",
                    Spectrometer::Asd => "_ASD",
                };
                assert!(file.contains(marker), "{file} is not a {marker} spectrum");
            }
            if let GrainScaling::Slab {
                real_index,
                lab_grain_diameter,
            } = e.scaling()
            {
                assert!((1.3..=3.0).contains(&real_index), "{}", e.name());
                let um = lab_grain_diameter.value() * 1.0e6;
                assert!((5.0..=500.0).contains(&um), "{}: {um} µm", e.name());
            }
            assert!(!e.source().citation().is_empty(), "{}", e.name());
            assert_eq!(e.source().basis(), Basis::Reduced, "{}", e.name());
        };
        for (_, optics) in rows() {
            let key = optics.key();
            match optics.input() {
                OpticsInput::Spectrum { end_member } => {
                    assert_eq!(optics.reference_form(), ReferenceForm::Regolith, "{key}");
                    check_end_member(end_member);
                }
                OpticsInput::Mixture { parts } => {
                    assert_eq!(optics.reference_form(), ReferenceForm::Regolith, "{key}");
                    assert!(parts.len() >= 2, "{key}");
                    let sum: f64 = parts.iter().map(MixturePart::volume_fraction).sum();
                    assert!(
                        (sum - 1.0).abs() < 2e-3,
                        "{key}: the fractions sum to {sum}"
                    );
                    for (i, part) in parts.iter().enumerate() {
                        assert!(part.volume_fraction() > 0.0, "{key}");
                        check_end_member(part.end_member());
                        assert!(
                            !parts[..i].iter().any(|p| {
                                p.end_member().name() == part.end_member().name()
                                    && p.grain() == part.grain()
                            }),
                            "{key}: {} twice",
                            part.end_member().name()
                        );
                        if part.grain() == GrainClass::Fine {
                            assert_eq!(
                                part.end_member().scaling(),
                                GrainScaling::Held,
                                "{key}: {} is fine but not held",
                                part.end_member().name()
                            );
                        }
                    }
                }
                OpticsInput::Levels { levels } => {
                    let knots = levels.knots();
                    assert!(knots.windows(2).all(|p| p[0].0 < p[1].0), "{key}");
                    assert!(knots.iter().all(|k| (0.0..=1.0).contains(&k.1)), "{key}");
                }
                OpticsInput::SizedIndex { grain_diameter, .. } => {
                    assert!(grain_diameter < optics.reference_form().grain_diameter());
                    assert!(grain_diameter.value() > 0.0);
                }
                OpticsInput::Index { .. } | OpticsInput::OpaqueSlab { .. } => {}
            }
        }
        assert!(
            (FINE_GRAIN_DIAMETER.value() - 1.0e-6).abs() < 1e-18,
            "the ruling's 1 µm"
        );
    }

    /// The phase row is R10's `for_family` law of the row's family for every condensed row, never
    /// Lambert, and the reference forms split the families as the laws do: the frost is the Ice
    /// law's, the regolith the Regolith law's.
    #[test]
    fn substance_optics_the_phase_row_is_the_family_s_law() {
        for id in ids() {
            let Some(condensed) = id.substance().condensed() else {
                continue;
            };
            let law = condensed.phase_law();
            assert_eq!(law, PhaseLaw::for_family(condensed.mechanics()));
            assert_ne!(law, PhaseLaw::Lambert);
            let frost = ReferenceForm::of_family(condensed.mechanics()) == ReferenceForm::Frost;
            assert_eq!(frost, law == PhaseLaw::Ice, "{}", id.substance().key());
        }
    }

    /// Every row's values are inside energy's bounds: each directional-hemispherical albedo at
    /// most 1 in every band, each normal albedo at most R10's particulate ceiling of 1.35
    /// (decision-r10-limb §4), w in [0, 1], and every spectral bin's spherical albedo in [0, 1].
    #[test]
    fn substance_optics_every_row_is_inside_the_energy_bound() {
        for (_, optics) in rows() {
            let key = optics.key();
            for band in 0..3 {
                let w = optics.single_scattering_albedo_bvr()[band];
                assert!((0.0..=1.0).contains(&w), "{key}: w {w}");
                let a_n = optics.normal_albedo_bvr()[band];
                assert!(
                    a_n.is_finite() && a_n > 0.0 && a_n <= PARTICULATE_NORMAL_ALBEDO_CEILING,
                    "{key}: A_N {a_n}"
                );
                let sphere = optics.spherical_albedo_bvr()[band];
                assert!(sphere > 0.0 && sphere <= 1.0, "{key}: {sphere}");
                for incidence in 0..HEMISPHERICAL_INCIDENCE_COSINES.len() {
                    let a_h = optics
                        .directional_hemispherical_albedo_bvr(incidence)
                        .expect("four incidences")[band];
                    assert!(a_h > 0.0 && a_h <= 1.0, "{key}: A_h {a_h}");
                }
            }
            assert!(optics.directional_hemispherical_albedo_bvr(4).is_none());
            for (bin, sphere) in optics.spherical_albedo_bins().into_iter().enumerate() {
                assert!((0.0..=1.0).contains(&sphere), "{key}: bin {bin}: {sphere}");
            }
        }
    }

    /// The bins are contiguous over 0.3–5 µm, rising, each of positive width, with B, V and R's
    /// bins holding 440, 550 and 640 nm and a bin ending at 780 nm, where R08's files end; and the
    /// row's layout is its values in B, V and R, its bins, then its reach.
    #[test]
    fn substance_optics_the_bins_are_contiguous_over_0_3_to_5_um() {
        let edges = SPECTRAL_BIN_EDGES_NM;
        assert!((edges[0] - 300.0).abs() < 1e-12);
        assert!((edges[SPECTRAL_BINS] - 5_000.0).abs() < 1e-12);
        assert!(edges.windows(2).all(|pair| pair[0] < pair[1]));
        for (bin, wavelength) in BVR_BINS.into_iter().zip(BAND_WAVELENGTHS_NM) {
            assert!(
                edges[bin] < wavelength && wavelength < edges[bin + 1],
                "bin {bin}: {} to {} nm, not holding {wavelength}",
                edges[bin],
                edges[bin + 1]
            );
        }
        assert!(edges.iter().any(|edge| (edge - 780.0).abs() < 1e-12));
        assert_eq!(SPECTRAL_BINS, 12);
        assert_eq!(BVR_ROW_LEN, 21);
        assert_eq!(SPECTRAL_BINS_OFFSET, BVR_ROW_LEN);
        assert_eq!(DATA_REACH_OFFSET, SPECTRAL_BINS_OFFSET + SPECTRAL_BINS);
        assert_eq!(ALBEDO_ROW_LEN, 35);
    }

    /// Each row's data reach lie within the bins' 0.3–5 µm and cover the visible's 440–640 nm: an
    /// index row's from R08's 380 nm to 780 nm at least; a row without an infrared set ends at 780
    /// nm and holds its 700–780 nm bin beyond; every bin past the data holds its nearest covered
    /// bin's value; and a lossless grain's bins are 1 to 10⁻⁶ (the rows with k = 0 in the visible
    /// and no infrared set).
    #[test]
    fn substance_optics_the_bins_follow_the_data_s_reach() {
        for (_, optics) in rows() {
            let key = optics.key();
            let [lo, hi] = optics.data_reach_nm();
            assert!(
                (300.0..=440.0).contains(&lo) && (640.0..=5_000.0).contains(&hi),
                "{key}"
            );
            let bins = optics.spherical_albedo_bins();
            if optics.index_file().is_some() {
                assert!((lo - 380.0).abs() < 1e-9, "{key}: from {lo} nm");
                assert!(hi >= 780.0, "{key}: to {hi} nm");
                if optics.infrared_source().is_none() {
                    assert!((hi - 780.0).abs() < 1e-9, "{key}: to {hi} nm");
                    for bin in 5..SPECTRAL_BINS {
                        assert!(bins[bin].total_cmp(&bins[4]).is_eq(), "{key}: bin {bin}");
                    }
                }
            }
            for (bin, pair) in SPECTRAL_BIN_EDGES_NM.windows(2).enumerate() {
                let held = if pair[0] >= hi {
                    SPECTRAL_BIN_EDGES_NM.windows(2).rposition(|p| p[0] < hi)
                } else if pair[1] <= lo {
                    SPECTRAL_BIN_EDGES_NM.windows(2).position(|p| p[1] > lo)
                } else {
                    None
                };
                if let Some(held) = held {
                    assert!(
                        bins[held].total_cmp(&bins[bin]).is_eq(),
                        "{key}: bin {bin} holds its nearest covered bin's value"
                    );
                }
            }
        }
        for key in ["O2", "Ar", "H2S", "NH4SH"] {
            let optics = of(key);
            assert_eq!(optics.infrared_source(), None, "{key}");
            for (bin, sphere) in optics.spherical_albedo_bins().into_iter().enumerate() {
                assert!((sphere - 1.0).abs() < 1e-6, "{key}: bin {bin}: {sphere}");
            }
        }
    }

    /// The mixtures whose mode, or a split or count inside it, no source gives: each mode `Adopted`
    /// and saying it is an assumption (plan 14's T49.i record, "Modes no source gives").
    const ADOPTED_MODES: [&str; 10] = [
        "anorthosite",
        "basalt_fe",
        "komatiite",
        "basalt_reduced",
        "granite",
        "ultramafic",
        "ferric",
        "phyllosilicate",
        "chondrite_ci",
        "cometary",
    ];

    /// Every row names its sources and their basis: the fit, its reference form's photometry and
    /// grain size, and its input's: an index file, by name, and its infrared set where it has one,
    /// once; each end-member's description (`Reduced`), a mixture's mode (`Reduced` where its
    /// source gives it, `Adopted` and saying it is an assumption where none does, the rows of
    /// [`ADOPTED_MODES`]) and its fine grains (`Adopted`) where it has any; levels' measurement;
    /// and a stand-in's reason, where it comes from no index file whose own source gives it.
    #[test]
    fn substance_optics_every_row_names_its_source_and_basis() {
        let mut adopted = Vec::new();
        for (_, optics) in rows() {
            let key = optics.key();
            let sources = optics.sources();
            let covering =
                |what: &'static str| sources.iter().filter(move |s| s.covers().contains(what));
            assert!(covering("normal albedo").all(|s| s.basis() == Basis::Fitted));
            for what in ["normal albedo", "phase function", "grain size"] {
                assert_eq!(covering(what).count(), 1, "{key}: {what}");
            }
            assert_eq!(
                covering("phase function").next().map(Source::citation),
                Some(optics.reference_form().photometry().source().citation())
            );
            assert_eq!(
                covering("infrared").count(),
                usize::from(optics.infrared_source().is_some()),
                "{key}"
            );
            assert!(covering("infrared").all(|s| s.basis() == Basis::Reduced));
            match optics.input() {
                OpticsInput::Index { file, .. }
                | OpticsInput::SizedIndex { file, .. }
                | OpticsInput::OpaqueSlab { file } => {
                    assert_eq!(covering("index").count(), 1, "{key}");
                    assert!(
                        covering("index").all(|s| s.citation().starts_with(file)),
                        "{key}"
                    );
                }
                OpticsInput::Spectrum { end_member } => {
                    assert!(sources.contains(&end_member.source()), "{key}");
                }
                OpticsInput::Mixture { parts } => {
                    for part in parts {
                        assert!(sources.contains(&part.end_member().source()), "{key}");
                    }
                    assert_eq!(covering("mode").count(), 1, "{key}");
                    for mode in covering("mode") {
                        match mode.basis() {
                            Basis::Reduced => {}
                            Basis::Adopted => {
                                assert!(mode.citation().contains("an assumption"), "{key}");
                                adopted.push(key.as_str().to_owned());
                            }
                            basis => panic!("{key}: a mode of basis {basis:?}"),
                        }
                    }
                    let fine = parts.iter().any(|p| p.grain() == GrainClass::Fine);
                    assert_eq!(covering("fine grains").count(), usize::from(fine), "{key}");
                    assert!(covering("fine grains").all(|s| s.basis() == Basis::Adopted));
                }
                OpticsInput::Levels { .. } => {
                    assert_eq!(covering("levels").count(), 1, "{key}");
                }
            }
            if optics.index_file().is_none() {
                assert_eq!(
                    covering("stand-in").count(),
                    usize::from(optics.is_stand_in()),
                    "{key}: a stand-in states its reason"
                );
            }
            assert!(sources.iter().all(|s| !s.citation().is_empty()));
        }
        assert_eq!(adopted, ADOPTED_MODES);
    }

    /// The rows outside the model are labelled by having none (the closed set of
    /// `rows/optics.rs`): the ices with no index file or spectrum, the liquids and solutions, the
    /// giant planets' hazes, `SiC` and every row without condensed columns.
    #[test]
    fn substance_optics_the_rows_not_modelled_have_none() {
        const NOT_MODELLED: [&str; 18] = [
            "Kr",
            "Xe",
            "N2O",
            "C2H6",
            "CH3OH",
            "H2SO4",
            "chromophore",
            "ice_giant_haze",
            "acid_uv_absorber",
            "brine_nacl",
            "brine_mgso4",
            "brine_naclo4",
            "brine_mgclo4",
            "nh3_h2o",
            "ch4_c2h6_n2",
            "SiC",
            "He",
            "H2",
        ];
        for key in NOT_MODELLED {
            let id = by_key(key).unwrap_or_else(|| panic!("{key} is a row"));
            assert!(id.substance().condensed_optics().is_none(), "{key}");
        }
        let condensed = ids()
            .filter(|id| id.substance().condensed().is_some())
            .count();
        let unmodelled_condensed = NOT_MODELLED
            .iter()
            .filter(|key| by_key(key).is_some_and(|id| id.substance().condensed().is_some()))
            .count();
        assert_eq!(
            rows().count() + unmodelled_condensed,
            condensed,
            "every condensed row is modelled or named here"
        );
    }

    /// The Solar System's icy anchors (plan 14's T49.c tests), against each row's normal albedo in
    /// its reference form:
    ///
    /// - **Enceladus**, the cleanest water ice: normal reflectances "0.9 on the leading hemisphere
    ///   to 1.4 on the trailing" (Verbiscer and Veverka 1994, Icarus 110, 155, abstract; Buratti et
    ///   al. 2017, Icarus 287, 207, Table 3). Its convention, stated as the plan asks: these are
    ///   normal reflectances, `A_N`; NASA's Saturnian satellite fact sheet gives its visual
    ///   geometric albedo as 1.0, the 1.08 beside it being its mass in 10²⁰ kg, and its geometric
    ///   albedo at 550 nm at true opposition, the surge included, is 1.375 ± 0.008 (Verbiscer et
    ///   al. 2007, Science 315, 815, read through a secondary; decision-p14-t49c-split §7).
    /// - **Pluto's N₂ plains**, Sputnik Planitia: units centred on normal reflectances of 0.90 and
    ///   0.95 in LORRI's band (pivot 0.607 µm), within Tombaugh Regio's 0.8–1.0 (Buratti et al.
    ///   2017, read as arXiv:1604.06129, pp. 10 and 17–18).
    /// - **Europa**, as a bound: every normal reflectance measured on it, 0.48 on the trailing
    ///   mottled terrain to 0.85 (Buratti and Veverka 1983, Icarus 55, 93, abstract; Buratti et al.
    ///   2017, Table 3, after Buratti and Golombek 1988), lies below pure fresh water frost's, its
    ///   ice being darkened by its non-ice material, which is P14.T51.d's.
    #[test]
    fn substance_optics_matches_the_icy_anchors() {
        let water = of("H2O").normal_albedo_bvr();
        for (band, a_n) in [("B", water[0]), ("V", water[1])] {
            assert!(
                (0.9..=1.4).contains(&a_n),
                "H2O in {band}: {a_n}, outside Enceladus's 0.9–1.4"
            );
        }
        let nitrogen = of("N2").normal_albedo_bvr();
        for (band, a_n) in [("V", nitrogen[1]), ("R", nitrogen[2])] {
            assert!(
                (0.8..=1.0).contains(&a_n),
                "N2 in {band}: {a_n}, outside Pluto's N2 plains' 0.8–1.0"
            );
        }
        assert!(
            water[1] > 0.85,
            "H2O in V: {}, not above Europa's brightest, 0.85",
            water[1]
        );
    }

    /// A palette entry resolves its every solid value from the registry: water ice on a cold world,
    /// carbon dioxide frost, a basalt crust; and refuses what is not modelled.
    #[test]
    fn substance_optics_resolves_palette_entries() {
        let water = by_key("H2O").unwrap();
        let cold = Kelvin::new(150.0);
        let ice = resolve_palette_entry(water, PaletteRole::Ice, cold).unwrap();
        for (resolved, own) in ice
            .normal_albedo_bvr
            .iter()
            .zip(of("H2O").normal_albedo_bvr())
        {
            assert!(resolved.total_cmp(&own).is_eq(), "{resolved} against {own}");
        }
        assert_eq!(ice.phase_row, u8::from(PhaseLaw::Ice));
        assert!((ice.grain_density_kg_m3 - ice_ih_density(cold).value()).abs() < 1e-9);
        assert!((ice.transition.value() - 273.16).abs() < 1e-12);
        assert_eq!(ice.mechanics, MechanicsFamily::WaterIce);
        assert_eq!(ice.liquid, None);
        let basalt = by_key("basalt").unwrap();
        let crust = resolve_palette_entry(basalt, PaletteRole::SecondaryCrust, cold).unwrap();
        assert_eq!(crust.phase_row, u8::from(PhaseLaw::Regolith));
        assert!((crust.grain_density_kg_m3 - 3_010.0).abs() < 1e-9);
        assert_eq!(crust.mechanics, MechanicsFamily::Silicate);
        let error = |key: &str, role, t: f64| {
            resolve_palette_entry(by_key(key).unwrap(), role, Kelvin::new(t)).unwrap_err()
        };
        assert_eq!(
            error("H2O", PaletteRole::Liquid, 280.0),
            ResolvePaletteEntryError::LiquidNotModelled
        );
        assert_eq!(
            error("SiC", PaletteRole::Deposit, 210.0),
            ResolvePaletteEntryError::OpticsNotModelled
        );
        assert_eq!(
            error("He", PaletteRole::Ice, 3.0),
            ResolvePaletteEntryError::NotCondensed
        );
        assert_eq!(
            error("CO2", PaletteRole::Ice, 300.0),
            ResolvePaletteEntryError::DensityNotModelled
        );
        assert_eq!(
            error("CO2", PaletteRole::Ice, f64::NAN),
            ResolvePaletteEntryError::Temperature
        );
    }
}
