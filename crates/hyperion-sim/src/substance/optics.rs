//! The condensed optics: a solid's normal albedo in B, V and R at its reference grain class, with
//! the photometric law the terrain draws it by (plan 14, P14.T49.c; decision-composition §1.1,
//! §1.7 and §1.8; decision-r10-limb §2–§4).
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
//! - **The single-scattering albedo** w of a grain of diameter D in each band is Hapke's
//!   equivalent slab, from the row's complex index n + ik at the band's wavelength. The index is
//!   rendering plan R08's committed file for the row's solid
//!   (`apps/hyperion/src/renderer/src/view/atmosphere/materials/`), the one source of truth for a
//!   condensate's optical constants (decision-composition §1.1), read by wavelength on the file's
//!   own grid.
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
//!   ([`CondensedOptics`]). Energy is bounded on the whole solution: the IMSA's own
//!   directional-hemispherical albedo, from the same w, phase function and opposition as `A_N`, is
//!   at most 1 at those incidences in every band, and `A_N` at most 1.35, R10's particulate
//!   ceiling (`hyperion_surface::material::PARTICULATE_NORMAL_ALBEDO_CEILING`; decision-r10-limb
//!   §4), both held by the fit's tests; the tabled albedos, the similarity relation's, are at most
//!   1 by construction.
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
//! A row has condensed optics where its solid has an index file R08 committed whose absorption the
//! equivalent slab can take (k ≪ 1), and where its reference form's photometry is valid for its
//! grains ([`Substance::condensed_optics`]; the closed list in `rows/optics.rs`, each exclusion
//! with its reason, for the composition audit). The lithologies, salts and chondrites without an
//! index file, the transparent minerals, the metals and opaque solids, and Mars's dust take
//! laboratory spectra in P14.T49.i, which adds the infrared bands; the thermal columns are
//! P14.T49.j's (`substance::thermal`), and the geomorphic columns are out of 23
//! (decision-p14-t49c-split §1.3). One grain class per form serves the visible, which grain size
//! barely moves; grain classes are deferred to the infrared, with their size (§3). A palette entry
//! whose row has none cannot be resolved ([`ResolvePaletteEntryError::OpticsNotModelled`]), and is
//! labelled so.
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

/// The number of values a row of `tables::substance_albedo` holds: three bands each of w, `A_N` and
/// the spherical albedo, and of the directional-hemispherical albedo at each of
/// [`HEMISPHERICAL_INCIDENCE_COSINES`].
pub const ALBEDO_ROW_LEN: usize = 9 + 3 * HEMISPHERICAL_INCIDENCE_COSINES.len();

/// The bands' wavelengths, nm: the palette's, 440, 550 and 640 ([`BVR_EFFECTIVE_WAVELENGTHS_NM`]).
pub const BAND_WAVELENGTHS_NM: [f64; 3] = BVR_EFFECTIVE_WAVELENGTHS_NM;

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

/// How directly an index file's constants were obtained, as its `provenance` gives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) enum IndexProvenance {
    /// Measured or derived from measurements of the substance.
    Measured,
    /// A stand-in: an estimate or a named analogue's index (the file's `standIn`).
    StandIn,
}

/// A row's condensed optics: its reference form, the index file its grains' optics are computed
/// from, and the fitted values of `tables::substance_albedo` (the [module documentation](self)).
#[derive(Debug, Clone, PartialEq)]
pub struct CondensedOptics {
    pub(super) key: SubstanceKey,
    pub(super) form: ReferenceForm,
    pub(super) index_file: &'static str,
    pub(super) provenance: IndexProvenance,
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

    /// The name of R08's index file its grains' optics are computed from, in
    /// `apps/hyperion/src/renderer/src/view/atmosphere/materials/`.
    #[must_use]
    pub const fn index_file(&self) -> &'static str {
        self.index_file
    }

    /// Whether that file is a stand-in (its `standIn`): an estimated or named analogue's index,
    /// which a reader labels approximate.
    #[must_use]
    pub const fn is_stand_in(&self) -> bool {
        matches!(self.provenance, IndexProvenance::StandIn)
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

    /// Where its values come from: the index file's paper, the reference form's photometry and
    /// grain size, and the fit.
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
    /// // Mars's dust is finer than geometric optics reaches: not modelled yet.
    /// assert!(by_key("mars_dust")?.substance().condensed_optics().is_none());
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
    })
}

#[cfg(test)]
mod tests {
    use hyperion_surface::material::PARTICULATE_NORMAL_ALBEDO_CEILING;

    use super::super::thermal::tests::{before_inline_tests, out_of_line_test_modules, rust_files};
    use super::super::{Basis, by_key, ice_ih_density};
    use super::*;

    /// The optics of the row of key `key`.
    fn of(key: &str) -> &'static CondensedOptics {
        by_key(key)
            .and_then(|id| id.substance().condensed_optics())
            .unwrap_or_else(|| panic!("{key} has condensed optics"))
    }

    /// The modelled rows are rows of the registry with condensed columns, in registry order and
    /// each once, each in its family's reference form, and each names its own index file once.
    #[test]
    fn substance_optics_rows_are_condensed_registry_rows_in_order() {
        let mut last = None;
        let mut files = Vec::new();
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
            assert!(!files.contains(&optics.index_file), "{}", optics.index_file);
            files.push(optics.index_file);
        }
        assert_eq!(rows().count(), OPTICS.len());
        assert!(rows().all(|(id, optics)| id.substance().key() == optics.key()));
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
    /// (decision-r10-limb §4), and w in [0, 1].
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
        }
    }

    /// Every row names its sources and their basis: the fit, its reference form's photometry and
    /// grain size, and its index file, by name.
    #[test]
    fn substance_optics_every_row_names_its_source_and_basis() {
        for (_, optics) in rows() {
            let sources = optics.sources();
            let covering =
                |what: &'static str| sources.iter().filter(move |s| s.covers().contains(what));
            assert!(covering("normal albedo").all(|s| s.basis() == Basis::Fitted));
            for what in ["normal albedo", "phase function", "grain size", "index"] {
                assert_eq!(covering(what).count(), 1, "{}: {what}", optics.key());
            }
            assert_eq!(
                covering("phase function").next().map(Source::citation),
                Some(optics.reference_form().photometry().source().citation())
            );
            assert!(
                covering("index").all(|s| s.citation().starts_with(optics.index_file())),
                "{}",
                optics.key()
            );
            assert!(sources.iter().all(|s| !s.citation().is_empty()));
        }
    }

    /// The rows outside the model are labelled by having none (the closed set of
    /// `rows/optics.rs`): the transparent minerals, the opaque solids, Mars's dust, the giant
    /// planets' hazes and every row without an index file.
    #[test]
    fn substance_optics_the_rows_not_modelled_have_none() {
        for key in [
            "Al2O3",
            "KCl",
            "NaCl",
            "SiO2",
            "S8",
            "TiO2",
            "Fe",
            "Cr",
            "graphite",
            "FeS",
            "soot",
            "mars_dust",
            "chromophore",
            "ice_giant_haze",
            "feldspathic",
            "granite",
            "chondrite_ci",
            "SO2",
            "He",
        ] {
            let id = by_key(key).unwrap_or_else(|| panic!("{key} is a row"));
            assert!(id.substance().condensed_optics().is_none(), "{key}");
        }
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

    /// The files outside the substance module whose calls into the held columns sit behind a
    /// named hold, each with it: the tripwire below passes them. A lane that adds a call behind its
    /// hold adds its file here (P14.T51.b adds `planetary/derive/photometry.rs` behind
    /// `SNAPSHOT_VERSION`; decision-p14-t49c-split §8.2).
    const HELD: [(&str, &str); 2] = [
        (
            "planetary/chem/partition.rs",
            "P14.T51.a's partition, which the record calls only behind the p14-snapshot lane's \
             hold until P14.T54.a",
        ),
        (
            "planetary/hooks/crust.rs",
            "P14.T51.c's crust, a pure module the record calls only from P14.T54.a, which reads \
             its lid's first melt, the condensed columns' transition, through `.condensed()`",
        ),
    ];

    /// The files that define what the patterns name: this module, its rows and its table.
    const OWN: [&str; 3] = [
        "substance/optics.rs",
        "substance/rows/optics.rs",
        "tables/substance_albedo.rs",
    ];

    /// The forms by which code outside the substance module would read the held columns, their
    /// optics or the palette's resolution, without whitespace: the unambiguous calls
    /// (decision-p14-t49c-split §8.2, which narrowed them: a partition result's own `.condensed()`
    /// is not the registry's; `.grain_density(` is a method of the registry's alone, and a
    /// `grain_density_kg_m3` field does not match it) and this module's and its table's paths.
    const PATTERNS: [&str; 15] = [
        "condensed_optics(",
        "resolve_palette_entry(",
        "Substance::condensed",
        ".substance().condensed()",
        "CondensedColumns",
        ".grain_density(",
        "Substance::grain_density",
        ".phase_law()",
        "substance::optics",
        "super::optics",
        "optics::rows(",
        "CondensedOptics",
        "ReferenceForm",
        "ResolvePaletteEntryError",
        "substance_albedo::",
    ];

    /// The patterns that are this module's own API, which the rest of the substance module, whose
    /// own code reads the condensed columns, must not name either.
    const OPTICS_PATTERNS: [&str; 10] = [
        "condensed_optics(",
        "resolve_palette_entry(",
        ".phase_law()",
        "substance::optics",
        "super::optics",
        "optics::rows(",
        "CondensedOptics",
        "ReferenceForm",
        "ResolvePaletteEntryError",
        "substance_albedo::",
    ];

    /// The lines of `substance/mod.rs` that declare this module and re-export it.
    const DECLARATIONS: [&str; 2] = ["pub mod optics;", "pub use optics::"];

    /// The `patterns` that the code of the file at `relative` (under the crate's `src/`), of text
    /// `text`, names before its inline test module: its comment lines and, in `substance/mod.rs`,
    /// [`DECLARATIONS`] left out, and the rest read with every whitespace removed, so that a call
    /// rustfmt breaks across lines is found.
    fn named<'p>(relative: &str, text: &str, patterns: &[&'p str]) -> Vec<&'p str> {
        let code: String = before_inline_tests(text)
            .into_iter()
            .map(str::trim_start)
            .filter(|code| {
                let declaration = relative == "substance/mod.rs"
                    && DECLARATIONS.iter().any(|d| code.starts_with(d));
                !(code.starts_with("//") || declaration)
            })
            .flat_map(|code| code.chars().filter(|c| !c.is_whitespace()))
            .collect();
        patterns
            .iter()
            .copied()
            .filter(|pattern| code.contains(pattern))
            .collect()
    }

    /// The scan finds a call that rustfmt breaks across lines, reads past an out-of-line test
    /// module's declaration and a test-only item, and leaves out comments and the inline test
    /// module.
    #[test]
    fn substance_optics_the_tripwire_s_scan_reads_the_code_alone() {
        let text = "#[cfg(test)]\nmod checks;\n#[cfg(test)]\nfn helper() {}\n\
                    // id.substance().condensed()\nfn melt(id: SubstanceId) -> Kelvin {\n    \
                    id.substance()\n        .condensed()\n        .unwrap()\n}\n\
                    #[cfg(test)]\nmod tests {\n    fn f() { condensed_optics(); }\n}\n";
        assert_eq!(
            named("planetary/x.rs", text, &PATTERNS),
            [".substance().condensed()"]
        );
        let qualified = "let d = row\n    .grain_density(crate::substance::CondensedPhase::Solid, t);\n\
                         let g = GrainForm { grain_density_kg_m3: 3_010.0 };\n";
        assert_eq!(
            named("planetary/y.rs", qualified, &PATTERNS),
            [".grain_density("]
        );
        assert_eq!(
            named(
                "substance/mod.rs",
                "pub use optics::{CondensedOptics};\n",
                &PATTERNS
            ),
            Vec::<&str>::new()
        );
        assert_eq!(
            named(
                "substance/mix.rs",
                "use super::optics::rows;\n",
                &OPTICS_PATTERNS
            ),
            ["super::optics"]
        );
    }

    /// The held columns stay off the record until P14.T54.a wires them (decision-bump-plan-after-22
    /// §1.2; plan 14's Risks, P14.T49.b part 2: "no caller of `Substance::condensed` or
    /// `grain_density` is reachable from `record::Surface` before T54.a"). Every file of the
    /// crate's `src/` is read, in sorted order, by `substance::thermal`'s tripwire's walk, but this
    /// module's own ([`OWN`]), [`HELD`]'s, each inline test module and every file an out-of-line
    /// test module declares ([`named`]): outside the substance module no code names
    /// [`PATTERNS`], and inside it none names [`OPTICS_PATTERNS`]. **P14.T54.a deletes this
    /// tripwire** when it wires them.
    #[test]
    fn substance_optics_nothing_on_the_record_reads_the_held_columns_yet() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        rust_files(&src, &src, &mut files);
        assert!(files.len() > 100, "the walk found the crate's sources");
        let read = |path: &std::path::Path| {
            std::fs::read_to_string(path)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
        };
        for (held, hold) in HELD {
            assert!(!hold.is_empty(), "{held} names its hold");
            assert!(
                files.iter().any(|(relative, _)| relative == held),
                "{held} is a source file"
            );
        }
        let mut tests = Vec::new();
        for (relative, path) in &files {
            tests.extend(out_of_line_test_modules(relative, &read(path)));
        }
        let mut found = Vec::new();
        for (relative, path) in &files {
            let test_file = tests
                .iter()
                .any(|t| relative == t || (t.ends_with('/') && relative.starts_with(t.as_str())));
            if test_file
                || OWN.contains(&relative.as_str())
                || HELD.iter().any(|(held, _)| held == relative)
            {
                continue;
            }
            let patterns: &[&str] = if relative.starts_with("substance/") {
                &OPTICS_PATTERNS
            } else {
                &PATTERNS
            };
            for pattern in named(relative, &read(path), patterns) {
                found.push(format!("{relative} names {pattern}"));
            }
        }
        assert!(found.is_empty(), "{found:#?}");
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
            error("mars_dust", PaletteRole::Deposit, 210.0),
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
