//! The property table: each form's rows by the entry's mechanics family, joined to the palette
//! entry's resolved values (plan R10, T1.a, Design notes 2 and 8; decision-composition §1.8).
//!
//! # What the table holds, and what it leaves out
//!
//! A form's rows ([`FormRow`]) are its Mohr–Coulomb pairs by depth with each layer's porosity, its
//! pad friction, its strength as a solid, a factor on its entry's normal albedo, and its physical
//! span of normal albedo, each with its source ([`Citation`]). A row is the form's for one
//! [`MechanicsFamily`]: a regolith of silicate is the lunar soil's, a regolith of water ice is not.
//! A figure without a source is absent (`None`, or an empty shear table), and a console shows it
//! missing, never invented (Design note 2). The modelled rows are few; every other (form, family)
//! pair is [`FormRow::UNMODELLED`], which a reader labels so.
//!
//! The palette entry gives the rest, resolved by the server from plan 14's registry: the normal
//! albedo in B, V and R, the phase law's code, the grain density, the transition temperature and
//! the mechanics family (R09's [`PaletteEntry`]). A layer's bulk density is the entry's grain
//! density times one less the form's porosity, the porosity being how a material lies, never the
//! substance's (science-r09-drainage.md §3).

use hyperion_base::math;
use hyperion_base::units::{Kelvin, KilogramsPerCubicMetre, Metres, Pascals, Radians};

use super::{Material, MaterialClass, PhotometricForm};
use crate::field::{MaterialPalette, MechanicsFamily, PaletteEntry, coded_enum};

/// A Mohr–Coulomb pair: the shear strength τ = c + σ tan φ of a granular material under normal
/// stress σ.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MohrCoulomb {
    cohesion: Pascals,
    friction_angle: Radians,
}

impl MohrCoulomb {
    /// The pair of cohesion `cohesion` (Pa, at least 0) and angle of internal friction
    /// `friction_angle` (radians, between 0 and π/2).
    #[must_use]
    pub(crate) const fn new(cohesion: Pascals, friction_angle: Radians) -> Self {
        Self {
            cohesion,
            friction_angle,
        }
    }

    /// The cohesion c, Pa.
    #[must_use]
    pub const fn cohesion(self) -> Pascals {
        self.cohesion
    }

    /// The angle of internal friction φ, radians.
    #[must_use]
    pub const fn friction_angle(self) -> Radians {
        self.friction_angle
    }
}

/// One layer of a form's shear table: the depth interval below the surface over which its source
/// averages a Mohr–Coulomb pair and a porosity.
///
/// A source's intervals may overlap, as the Lunar Sourcebook's 0–15, 0–30, 30–60 and 0–60 cm do;
/// the layer at a depth is the narrowest that holds it ([`MaterialProperties::shear_layer_at`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShearLayer {
    top: Metres,
    bottom: Metres,
    strength: MohrCoulomb,
    porosity: f64,
}

impl ShearLayer {
    /// The layer from depth `top` to `bottom` (m, `top` < `bottom`) of strength `strength` and
    /// porosity `porosity` (the void share of its volume, 0 to 1).
    #[must_use]
    pub(crate) const fn new(
        top: Metres,
        bottom: Metres,
        strength: MohrCoulomb,
        porosity: f64,
    ) -> Self {
        Self {
            top,
            bottom,
            strength,
            porosity,
        }
    }

    /// The interval's top, m below the surface.
    #[must_use]
    pub const fn top(self) -> Metres {
        self.top
    }

    /// The interval's bottom, m below the surface.
    #[must_use]
    pub const fn bottom(self) -> Metres {
        self.bottom
    }

    /// Its Mohr–Coulomb pair.
    #[must_use]
    pub const fn strength(self) -> MohrCoulomb {
        self.strength
    }

    /// Its porosity n, the void share of its volume, inter- and intragranular together.
    #[must_use]
    pub const fn porosity(self) -> f64 {
        self.porosity
    }

    /// Whether the interval holds `depth`: from its top, inclusive, to its bottom, exclusive.
    #[must_use]
    fn holds(self, depth: Metres) -> bool {
        self.top.value() <= depth.value() && depth.value() < self.bottom.value()
    }

    /// The interval's thickness.
    #[must_use]
    fn thickness(self) -> Metres {
        self.bottom - self.top
    }
}

/// The friction of a landing pad, steel or aluminium, on the ground: the interface friction
/// coefficient tan δ (Design note 2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FrictionModel {
    /// One coefficient at every temperature, as a granular soil's.
    Constant(f64),
    /// Coefficients at two or more temperatures, in increasing temperature, interpolated linearly
    /// between them and unknown outside them ([`interface_friction`]), as an ice's.
    ByTemperature(&'static [(Kelvin, f64)]),
}

/// A form's physical span of normal albedo in V, from its source: the range its occurrences take
/// (Design note 8).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlbedoSpan {
    low: f64,
    high: f64,
}

impl AlbedoSpan {
    /// The span from `low` to `high`, both at least 0, `low` ≤ `high`.
    #[must_use]
    pub(crate) const fn new(low: f64, high: f64) -> Self {
        Self { low, high }
    }

    /// The span's low end.
    #[must_use]
    pub const fn low(self) -> f64 {
        self.low
    }

    /// The span's high end.
    #[must_use]
    pub const fn high(self) -> f64 {
        self.high
    }

    /// Whether `normal_albedo` lies in the span, its ends included.
    #[must_use]
    pub fn contains(self, normal_albedo: f64) -> bool {
        self.low <= normal_albedo && normal_albedo <= self.high
    }
}

/// Which figure of a [`FormRow`] a [`Citation`] supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Figure {
    /// The shear table: its Mohr–Coulomb pairs, depths and porosities.
    Shear,
    /// The pad's interface friction.
    InterfaceFriction,
    /// The strength as a solid.
    CompressiveStrength,
    /// The factor on the entry's normal albedo.
    AlbedoFactor,
    /// The span of normal albedo.
    AlbedoSpan,
}

/// The source of one figure of a [`FormRow`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Citation {
    figure: Figure,
    source: &'static str,
}

impl Citation {
    /// The citation of `figure` to `source`.
    #[must_use]
    pub(crate) const fn new(figure: Figure, source: &'static str) -> Self {
        Self { figure, source }
    }

    /// The figure it supports.
    #[must_use]
    pub const fn figure(self) -> Figure {
        self.figure
    }

    /// The source: author, year, publication and the page, table or figure.
    #[must_use]
    pub const fn source(self) -> &'static str {
        self.source
    }
}

/// A form's rows for one mechanics family: the class's table (Design notes 2 and 8).
#[derive(Debug, Clone, PartialEq)]
pub struct FormRow {
    shear: &'static [ShearLayer],
    interface_friction: Option<FrictionModel>,
    compressive_strength: Option<Pascals>,
    albedo_factor: Option<f64>,
    normal_albedo_v: Option<AlbedoSpan>,
    citations: &'static [Citation],
}

impl FormRow {
    /// The row of a (form, family) pair with no figure: not modelled, which a reader labels so.
    pub const UNMODELLED: Self = Self {
        shear: &[],
        interface_friction: None,
        compressive_strength: None,
        albedo_factor: None,
        normal_albedo_v: None,
        citations: &[],
    };

    /// The Mohr–Coulomb pairs by depth, empty where unknown.
    #[must_use]
    pub const fn shear(&self) -> &'static [ShearLayer] {
        self.shear
    }

    /// The pad's interface friction, `None` where unknown.
    #[must_use]
    pub const fn interface_friction(&self) -> Option<FrictionModel> {
        self.interface_friction
    }

    /// The unconfined compressive strength of a solid form (an ice), Pa; `None` for a granular
    /// form or where unknown.
    #[must_use]
    pub const fn compressive_strength(&self) -> Option<Pascals> {
        self.compressive_strength
    }

    /// The factor on the entry's normal albedo for this form, `None` where the entry's albedo
    /// stands as resolved (Design note 8; main's ruling on the re-validation's first question).
    ///
    /// An entry's albedo is its substance's in its reference form: a silicate's, salt's or
    /// organic's as a particulate surface, and an ice's as fresh, fine-grained snow or frost, as
    /// the server's palette sources resolve it. A form that lies otherwise, as glacier ice or sea
    /// ice of one H₂O entry, multiplies it by this, in all three bands.
    #[must_use]
    pub const fn albedo_factor(&self) -> Option<f64> {
        self.albedo_factor
    }

    /// The form's physical span of normal albedo in V, `None` where no source gives one: the check
    /// on the entries, and the clamp of R10.T10.a's scale (Design note 8).
    #[must_use]
    pub const fn normal_albedo_v(&self) -> Option<AlbedoSpan> {
        self.normal_albedo_v
    }

    /// The sources of its figures.
    #[must_use]
    pub const fn citations(&self) -> &'static [Citation] {
        self.citations
    }

    /// Whether the row holds any figure.
    #[must_use]
    pub const fn is_modelled(&self) -> bool {
        !self.shear.is_empty()
            || self.interface_friction.is_some()
            || self.compressive_strength.is_some()
            || self.albedo_factor.is_some()
            || self.normal_albedo_v.is_some()
    }
}

coded_enum! {
    /// A photometric law of the ground: the code of R09's
    /// [`PaletteEntry::phase_row`](crate::field::PaletteEntry::phase_row).
    ///
    /// A code names the measured V phase curve its f(α) is fitted to and its L(α) (Design note 8).
    /// One byte, append-only; the codes ride the payload and are generated output, under the
    /// table's version. Index anything by a law through `u8::from`, never by its place in
    /// [`ALL`](Self::ALL): code 0 is no law, so the place is one less than the code.
    ///
    /// Every law is McEwen's lunar-Lambert, r = `A_N` f(α) [2 L(α) μ₀ ÷ (μ₀ + μ) + (1 − L(α)) μ₀]
    /// (R07's `body_brdf`). A law's L(α) is L(0) times McEwen's lunar profile
    /// ([`lommel_seeliger_share`](Self::lommel_seeliger_share)), and its L(0) is a measured
    /// property of how the surface is built, never an energy bound's
    /// ([`limb_parameter`](Self::limb_parameter); decision-r10-limb). R10.T10.b tabulates a law's
    /// f(α) against its L(α) in the row's colour channels and L(α) in its alpha (decision-r07-t8b).
    ///
    /// The set is three laws: two for particulate ground, whose L(0) follows its normal albedo by
    /// the particulate rule ([`limb_parameter()`]), and Lambert's, which the form chooses for the
    /// consolidated forms whatever the entry's code ([`PhotometricForm`](super::PhotometricForm),
    /// [`PhaseLawOrigin::Form`]). Energy is bounded on the normal albedo, by the law's own f
    /// ([`normal_albedo_ceiling`](Self::normal_albedo_ceiling)), never through L.
    ///
    /// Code 0 is no law: every palette's entries are 0 until the server's palette sources resolve
    /// them (P14.T49.c's phase column), and a reader then takes [`for_family`](Self::for_family)'s
    /// law and labels it ([`PhaseLawOrigin::FamilyDefault`]). The set is closed, three laws, for
    /// the composition audit: the bright particulate laws' L(α) is the Moon's profile scaled, since
    /// no bright surface's L(α) was read, and a surface whose phase curve no template draws (a
    /// forward-scattering frost, a liquid) is a new code.
    PhaseLaw {
        /// The Moon's: f(α) fitted to the Moon's V curve (R07's `moon` template, Krisciunas and
        /// Schaefer 1991, PASP 103, 1033, eq. 9), for particulate silicate, metal, salt and organic
        /// surfaces.
        Regolith = 1,
        /// An airless ice's: f(α) fitted to R07's `airless-ice` template, the Moon's V curve shape
        /// with Ganymede's phase integral q = 0.80 (Squyres and Veverka 1981, Icarus 46, 137;
        /// provisional there, labelled), for particulate water and volatile ices, frost among them.
        Ice = 2,
        /// Lambert's: f ≡ 1 and L(α) ≡ 0, R07's `lambert` template, a Lambert sphere of phase
        /// integral q = 3/2 (provisional there, labelled: no measured curve), for the consolidated
        /// forms, glacier, snow, sea ice and evaporite of any species or salt. A particulate form
        /// takes it only where its entry's code names it, which [`for_family`](Self::for_family)
        /// never does.
        ///
        /// As the single-scattering albedo tends to 1, multiple scattering dominates and the
        /// zero-phase limb profile becomes Chandrasekhar's H(μ)² shape, and a consolidated medium
        /// has little or no shadow hiding to flatten it: a lunar-Lambert fit to a snow-like
        /// medium's Hapke profile gives L(0) = 0.04–0.05 (decision-r10-limb §1, computed, for an
        /// assumed w ≈ 0.9999 and g ≈ 0.89, unsourced there). Under
        /// this law the directional-hemispherical albedo is `A_N` at every incidence
        /// ([`directional_hemispherical_albedo`] at L = 0), so a measured hemispherical albedo is
        /// the normal albedo with no conversion (decision-r10-limb §3).
        Lambert = 3,
    }
}

/// The code of [`PaletteEntry::phase_row`] that names no law: 0.
pub const UNASSIGNED_PHASE_ROW: u8 = 0;

/// McEwen's lunar L(α) polynomial, 1 + A α + B α² + C α³ in degrees: A, B and C (McEwen 1996,
/// "A precise lunar photometric function", LPSC XXVII, 841, Table 1 on p. 842, the same at 0.56 and
/// 0.76 µm, fitted to Galileo SSI images at 19.5°–101° with L(0) = 1 imposed; the coefficients of
/// USGS ISIS's `LunarLambertMcEwen.cpp`; decision-r07-t8b).
///
/// McEwen prints the law's Lommel–Seeliger term without the factor 2 (p. 841), as Buratti et al.
/// 2017 do; this crate's form is ISIS's and R07's, 2 L μ₀ ÷ (μ₀ + μ), so that r = `A_N` at
/// i = e = α = 0. A share taken from a paper of the other form converts before it is compared:
/// Buratti's f [A μ₀ ÷ (μ₀ + μ) + (1 − A) μ₀] is this form up to scale for L = A ÷ (2 − A), so
/// Buratti's A = 0.70 for Pluto and Charon is an L(0) of 0.54 here (decision-r10-limb §1).
const MCEWEN_LUNAR_L: [f64; 3] = [-0.019, 2.42e-4, -1.46e-6];

impl PhaseLaw {
    /// The law a substance of mechanics family `family` takes when its entry names none.
    ///
    /// The ices' for water and volatile ices, the Moon's for the rest: a particulate law, never
    /// Lambert's, which the form chooses ([`properties()`]). The server's palette sources may fill
    /// the code by the same rule.
    #[must_use]
    pub const fn for_family(family: MechanicsFamily) -> Self {
        match family {
            MechanicsFamily::WaterIce | MechanicsFamily::VolatileIce => Self::Ice,
            MechanicsFamily::Silicate
            | MechanicsFamily::Metal
            | MechanicsFamily::Salt
            | MechanicsFamily::Organic => Self::Regolith,
        }
    }

    /// R07's phase template its f(α) is fitted to, as R07's `PhaseTemplateId`
    /// (`view/appearance/law.ts`).
    #[must_use]
    pub const fn template(self) -> &'static str {
        match self {
            Self::Regolith => "moon",
            Self::Ice => "airless-ice",
            Self::Lambert => "lambert",
        }
    }

    /// L(0) of the law for a material of normal albedo `normal_albedo` in V: 0 for Lambert's,
    /// whatever the albedo, and the particulate rule ([`limb_parameter()`]) for the Regolith and
    /// Ice laws, NaN for a NaN albedo there.
    ///
    /// It takes the albedo as drawn: R10.T10.a's scale c multiplies `A_N`, and L(0) is then this of
    /// c `A_N`.
    #[must_use]
    pub fn limb_parameter(self, normal_albedo: f64) -> f64 {
        match self {
            Self::Regolith | Self::Ice => limb_parameter(normal_albedo),
            Self::Lambert => 0.0,
        }
    }

    /// The highest normal albedo in V the law reflects without breaking energy conservation at any
    /// incidence: the clamp of R10.T10.a's scale (decision-r10-limb §3).
    ///
    /// The bound is on the whole law with its phase factor: for every μ₀ in (0, 1] the
    /// directional-hemispherical albedo `A_h`(μ₀) = (`A_N` ÷ π) ∫ f(α) [2 L(α) ÷ (μ₀ + μ) + 1 −
    /// L(α)] μ dΩ is at most 1, so `A_N` ≤ 1 ÷ sup η(μ₀), η = `A_h` ÷ `A_N` being the row's own.
    /// - Lambert's, 1: with f ≡ 1 and L ≡ 0, `A_h` is `A_N` at every μ₀, exactly.
    /// - Regolith's and Ice's, [`PARTICULATE_NORMAL_ALBEDO_CEILING`] = 1.35, the Enceladus anchor
    ///   of the particulate rule, checked against the rows' real f: under the rule, the Ice row's
    ///   `A_N` × sup η is 1.35 × 0.720 = 0.97 at the ceiling, with sup η at grazing incidence, and
    ///   the Regolith row's 0.72 (computed with R07's construction f = Φ ÷ `Φ_shape`(α; L(α)), f
    ///   clamped at 4 and held past 150°; decision-r10-limb §3). 1.4 would give 1.008.
    ///
    /// A normal albedo above 1 is physical: Enceladus's is 0.9–1.4 and conserves energy, because
    /// its radiance falls away from opposition (Verbiscer and Veverka 1994, Icarus 110, 155,
    /// abstract).
    #[must_use]
    pub const fn normal_albedo_ceiling(self) -> f64 {
        match self {
            Self::Regolith | Self::Ice => PARTICULATE_NORMAL_ALBEDO_CEILING,
            Self::Lambert => 1.0,
        }
    }

    /// L(α), the law's Lommel–Seeliger share at phase `alpha`, for a material whose L(0) is `l0`.
    ///
    /// For the Regolith and Ice laws it is `l0` ([`limb_parameter`](Self::limb_parameter)) times
    /// McEwen's lunar profile, which falls from 1 at 0° to 0 at 103.9° and is held at 0 beyond,
    /// where the fit has no data (McEwen 1996, LPSC XXVII, 841; decision-r07-t8b); both take the
    /// same profile, since no other was read, and give NaN for a NaN phase. Lambert's is 0 at every
    /// phase and for every `l0`.
    #[must_use]
    pub fn lommel_seeliger_share(self, l0: f64, alpha: Radians) -> f64 {
        match self {
            Self::Regolith | Self::Ice => l0 * mcewen_lunar_profile(alpha),
            Self::Lambert => 0.0,
        }
    }
}

/// McEwen's lunar L(α) at phase `alpha`, held at 0 past its root near 103.9° (McEwen 1996); NaN
/// for a NaN phase.
#[must_use]
fn mcewen_lunar_profile(alpha: Radians) -> f64 {
    let a = alpha.value().abs().to_degrees();
    let [c1, c2, c3] = MCEWEN_LUNAR_L;
    let l = 1.0 + a * (c1 + a * (c2 + a * c3));
    // A comparison, not `max`, so that a NaN phase gives NaN, as `limb_parameter` does.
    if l < 0.0 { 0.0 } else { l }
}

/// Where a material's photometric law came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PhaseLawOrigin {
    /// Its palette entry's code.
    Palette,
    /// The family's default ([`PhaseLaw::for_family`]), labelled wherever it is shown.
    ///
    /// The entry's code is [`UNASSIGNED_PHASE_ROW`] or one this build does not know.
    FamilyDefault,
    /// The form's: a consolidated form takes [`PhaseLaw::Lambert`] whatever its entry's code
    /// ([`PhotometricForm::Consolidated`](super::PhotometricForm::Consolidated)).
    Form,
}

/// The directional-hemispherical albedo of the Lommel–Seeliger term at normal incidence, per unit
/// normal albedo, with f taken as 1: 4 (1 − ln 2) ≈ 1.227 (Design note 8;
/// [`directional_hemispherical_albedo`] at μ₀ = 1).
///
/// It is the f ≡ 1 case of the energy bound, which is exact only for a law whose f is 1, the
/// Lambert law, where L = 0 and the ratio does not enter. For a row with a phase factor, mostly
/// below 1 away from opposition, it is not the bound: R10.T10.b computes that with f
/// ([`PhaseLaw::normal_albedo_ceiling`]; decision-r10-limb §3).
pub const LOMMEL_SEELIGER_NORMAL_ALBEDO_RATIO: f64 = 4.0 * (1.0 - core::f64::consts::LN_2);

/// The normal albedo in V up to which a particulate surface has no limb darkening at zero phase,
/// L(0) = 1: 0.60.
///
/// The icy moons of Saturn "closely followed a lunar-like photometric function for geometric
/// albedos of about 0.55–0.60", Buratti et al. 2017 report of Buratti 1984 (Icarus 287, 207, read
/// as arXiv:1604.06129, p. 12; Buratti 1984, Icarus 59, 392); under L = 1 the geometric albedo is
/// `A_N` (p. 10). Below it: McEwen 1996 imposes L(0) = 1 "to match the observation of no
/// limb-darkening at α = 0°" and finds no difference between maria, highlands and bright crater
/// material (LPSC XXVII, 841); Ganymede, at normal reflectance 0.35–0.44, is "not Lambertian"
/// (Squyres and Veverka 1981, Icarus 46, 137, abstract) and lunar-like, A = 1 (Buratti et al.
/// 2017, p. 12); Rhea and Dione "closely follow lunar behavior with almost no limb darkening"
/// (Buratti 1984, abstract). Decision-r10-limb §1 and §2.
pub const LUNAR_LIMB_ONSET_NORMAL_ALBEDO: f64 = 0.60;

/// The normal albedo in V of the brightest particulate frost, the particulate rule's upper anchor:
/// 1.35, Enceladus's.
///
/// It is the normal albedo of Verbiscer and Veverka 1994's Hapke solution for Enceladus (Icarus
/// 110, 155, abstract: w = 0.998 ± 0.001, a Henyey–Greenstein g = −0.399 and B₀ = 0.21 ± 0.07;
/// their θ̄ = 6° is not used), by Hapke 1981 with Hapke 2002's H at α = 0, P(0) = 3.873: 1.351,
/// within the measured normal reflectances, 0.9 on the leading hemisphere to 1.4 on the trailing
/// (the same abstract; Buratti et al. 2017, Table 3), towards their top. Computed by
/// decision-r10-limb (§1, `hapke.py`); with Hapke 1981's approximate H, of the equation they
/// fitted (Hapke 1986), the same parameters give 1.406.
pub const BRIGHT_FROST_NORMAL_ALBEDO: f64 = 1.35;

/// L(0) of the brightest particulate frost, the particulate rule's value from
/// [`BRIGHT_FROST_NORMAL_ALBEDO`] up: 0.38, a derived figure.
///
/// The least-squares lunar-Lambert fit, L + (1 − L) μ weighted by the projected area μ dμ, to the
/// zero-phase disc profile of Verbiscer and Veverka 1994's Hapke solution for Enceladus (the
/// parameters of [`BRIGHT_FROST_NORMAL_ALBEDO`]): 0.378 (decision-r10-limb §1, `hapke.py`).
/// Bright particulate ice is limb-darkened but never Lambert: "Mimas, Tethys, and especially
/// Enceladus show significant limb darkening … No satellite's surface can be described by
/// Lambert's law" (Buratti 1984, abstract). B₀ is loosely constrained, and at B₀ = 1 the fit is
/// 0.515; with Hapke 1981's approximate H it is 0.327.
pub const BRIGHT_FROST_LIMB_PARAMETER: f64 = 0.38;

/// The highest normal albedo in V of a particulate law (Regolith and Ice): 1.35,
/// [`BRIGHT_FROST_NORMAL_ALBEDO`], the clamp of R10.T10.a's scale
/// ([`PhaseLaw::normal_albedo_ceiling`], whose documentation gives the check against the rows' f).
pub const PARTICULATE_NORMAL_ALBEDO_CEILING: f64 = BRIGHT_FROST_NORMAL_ALBEDO;

/// L(0) of a particulate lunar-Lambert law from its normal albedo in V: the particulate rule
/// `L_p` (decision-r10-limb §2).
///
/// L(0) is a measured property of how a surface is built, not an energy bound's. A porous
/// particulate regolith shadow-hides, so its disc is flat at zero phase, L(0) = 1, up to
/// [`LUNAR_LIMB_ONSET_NORMAL_ALBEDO`], 0.60; above it multiple scattering limb-darkens it, and
/// L(0) falls to [`BRIGHT_FROST_LIMB_PARAMETER`], 0.38, at Enceladus's
/// [`BRIGHT_FROST_NORMAL_ALBEDO`], 1.35, and stays there. Between the anchors, with x the normal
/// albedo, A₁ 0.60, `A_E` 1.35 and `L_E` 0.38,
/// `L_p`(x) = [`L_E` `A_E` (x − A₁) + A₁ (`A_E` − x)] ÷ [x (`A_E` − A₁)] = 0.6696 ÷ x − 0.1160,
/// so that x `L_p`(x) is linear in x on each piece. The checks against the observations: Europa's
/// plains at 0.71 give 0.83, lunar-like with a Lambert component (Buratti and Veverka 1983, Icarus
/// 55, 93, abstract), and Enceladus 0.38, limb-darkened but not Lambert. The outliers, where the
/// rule gives more L(0) than measured: Pluto, of mean `A_N` about 0.73, where it gives 0.80
/// against a global fit of 0.54 that undercorrects the brightest regions, at 0.8–1.0, for their
/// limb darkening (Buratti et al. 2017, pp. 10–11 and 17), so that they need less than 0.54, where
/// it gives 0.72–0.55; Charon, at 0.4–0.6 with 0.54 measured (p. 12), where it gives 1; and
/// Europa's mottled terrain, at 0.48–0.60, which "cannot be represented by a lunar-like
/// photometric law" (Buratti and Veverka 1983, abstract), where it gives 1. The consolidated forms
/// take the Lambert law instead ([`PhaseLaw::limb_parameter`]).
///
/// A normal albedo at or below 0 gives 1; a NaN gives NaN.
///
/// It takes the albedo as drawn: R10.T10.a's scale c multiplies `A_N`, and L(0) is then this of
/// c `A_N`, which leaves the geometric albedo c `A_N` [⅔ + ⅓ L(0)] piecewise linear in c, with
/// breaks where c `A_N` crosses 0.60 and 1.35.
#[must_use]
pub fn limb_parameter(normal_albedo: f64) -> f64 {
    const A1: f64 = LUNAR_LIMB_ONSET_NORMAL_ALBEDO;
    const AE: f64 = BRIGHT_FROST_NORMAL_ALBEDO;
    const LE: f64 = BRIGHT_FROST_LIMB_PARAMETER;
    if normal_albedo <= A1 {
        1.0
    } else if normal_albedo < AE {
        (LE * AE * (normal_albedo - A1) + A1 * (AE - normal_albedo)) / (normal_albedo * (AE - A1))
    } else if normal_albedo >= AE {
        LE
    } else {
        // Only a NaN fails all three comparisons.
        normal_albedo
    }
}

/// The lunar-Lambert law's directional-hemispherical albedo at incidence cosine `mu0`, with f
/// taken as 1.
///
/// It is `A_N` [4 L (1 − μ₀ ln(1 + 1 ÷ μ₀)) + (1 − L)], 0 < μ₀ ≤ 1, the Lommel–Seeliger term's
/// integral over the hemisphere and the Lambert term's (Design note 8): 1.227 `A_N` at μ₀ = 1 for
/// L = 1, and towards `A_N` (1 + 3L) at grazing incidence, its Lommel–Seeliger term towards
/// 4 `A_N` L.
///
/// It is the f ≡ 1 case of the energy bound, exact for the Lambert law (L = 0), where it is `A_N`
/// at every μ₀. For a row with a phase factor it is not the bound: f is mostly below 1 away from
/// opposition, and in every case computed it overstates such a row's reflection; R10.T10.b
/// computes the bound with f ([`PhaseLaw::normal_albedo_ceiling`]; decision-r10-limb §3).
#[must_use]
pub fn directional_hemispherical_albedo(normal_albedo: f64, limb: f64, mu0: f64) -> f64 {
    let lommel_seeliger = 4.0 * (1.0 - mu0 * math::ln_1p(1.0 / mu0));
    normal_albedo * (limb * lommel_seeliger + (1.0 - limb))
}

/// What the ground of a material is like: its form's rows joined to its entry's resolved values
/// (plan R10, Provides; decision-composition §1.8).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaterialProperties {
    material: Material,
    form: &'static FormRow,
    normal_albedo_bvr: [f64; 3],
    phase_law: PhaseLaw,
    phase_law_origin: PhaseLawOrigin,
    grain_density: KilogramsPerCubicMetre,
    transition: Kelvin,
    mechanics: MechanicsFamily,
}

impl MaterialProperties {
    /// The material.
    #[must_use]
    pub const fn material(&self) -> Material {
        self.material
    }

    /// The form's rows for the entry's family.
    #[must_use]
    pub const fn form(&self) -> &'static FormRow {
        self.form
    }

    /// The shear table by depth, empty where unknown.
    #[must_use]
    pub const fn shear(&self) -> &'static [ShearLayer] {
        self.form.shear
    }

    /// The layer at `depth` below the surface: the narrowest interval that holds it, or below the
    /// deepest, the deepest (the narrowest of those that reach deepest); `None` for an empty table
    /// or a depth that is negative or not finite.
    #[must_use]
    pub fn shear_layer_at(&self, depth: Metres) -> Option<ShearLayer> {
        if !depth.value().is_finite() || depth.value() < 0.0 {
            return None;
        }
        let narrowest = |a: &&ShearLayer, b: &&ShearLayer| a.thickness().total_cmp(&b.thickness());
        let shear = self.form.shear;
        shear
            .iter()
            .filter(|l| l.holds(depth))
            .min_by(narrowest)
            .or_else(|| {
                let deepest = shear.iter().map(|l| l.bottom).max_by(Metres::total_cmp)?;
                shear
                    .iter()
                    .filter(|l| l.bottom.total_cmp(&deepest).is_eq())
                    .min_by(narrowest)
            })
            .copied()
    }

    /// A layer's bulk density: the entry's grain density times one less the layer's porosity
    /// (science-r09-drainage.md §3).
    #[must_use]
    pub fn bulk_density(&self, layer: ShearLayer) -> KilogramsPerCubicMetre {
        self.grain_density * (1.0 - layer.porosity)
    }

    /// The pad's interface friction model, `None` where unknown; [`interface_friction`] evaluates
    /// it.
    #[must_use]
    pub const fn interface_friction_model(&self) -> Option<FrictionModel> {
        self.form.interface_friction
    }

    /// The unconfined compressive strength of a solid form, Pa, `None` where unknown.
    #[must_use]
    pub const fn compressive_strength(&self) -> Option<Pascals> {
        self.form.compressive_strength
    }

    /// The sources of the form's figures.
    #[must_use]
    pub const fn citations(&self) -> &'static [Citation] {
        self.form.citations
    }

    /// The normal albedo `A_N` in the Johnson B, V and R bands: the entry's times the form's factor
    /// ([`FormRow::albedo_factor`]).
    #[must_use]
    pub const fn normal_albedo_bvr(&self) -> [f64; 3] {
        self.normal_albedo_bvr
    }

    /// L(0) of the material's law at its normal albedo in V ([`PhaseLaw::limb_parameter`]): 0 for
    /// Lambert's, the particulate rule ([`limb_parameter()`]) for the others.
    #[must_use]
    pub fn limb_parameter(&self) -> f64 {
        self.phase_law.limb_parameter(self.normal_albedo_bvr[1])
    }

    /// The photometric law.
    #[must_use]
    pub const fn phase_law(&self) -> PhaseLaw {
        self.phase_law
    }

    /// Where the law came from: the form, the entry's code, or the family's default.
    #[must_use]
    pub const fn phase_law_origin(&self) -> PhaseLawOrigin {
        self.phase_law_origin
    }

    /// The substance's grain density, without pore space.
    #[must_use]
    pub const fn grain_density(&self) -> KilogramsPerCubicMetre {
        self.grain_density
    }

    /// The temperature above which the substance is melt or liquid.
    #[must_use]
    pub const fn transition(&self) -> Kelvin {
        self.transition
    }

    /// The substance's mechanics family.
    #[must_use]
    pub const fn mechanics(&self) -> MechanicsFamily {
        self.mechanics
    }
}

/// Why [`properties`] refused a material.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResolvePropertiesError {
    /// The material's entry is past the palette's.
    EntryNotInPalette {
        /// The entry's index.
        entry: u8,
        /// The palette's length.
        len: usize,
    },
}

impl std::fmt::Display for ResolvePropertiesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EntryNotInPalette { entry, len } => {
                write!(
                    f,
                    "palette entry {entry} is past a palette of {len} entries"
                )
            }
        }
    }
}

impl std::error::Error for ResolvePropertiesError {}

/// The properties of `material` on a body of palette `palette`: its form's rows for its entry's
/// mechanics family ([`form_row`]) with the entry's resolved values, the albedo times the form's
/// factor, and its photometric law.
///
/// The law is the form's or the entry's (decision-r10-limb §2). A consolidated form
/// ([`PhotometricForm::Consolidated`](super::PhotometricForm::Consolidated)) takes
/// [`PhaseLaw::Lambert`], [`PhaseLawOrigin::Form`], whatever the entry's code. A particulate form
/// takes the law the entry's code names ([`PhaseLawOrigin::Palette`]) or, where it names none or
/// one this build does not know, the family's default ([`PhaseLawOrigin::FamilyDefault`]).
///
/// # Errors
///
/// [`ResolvePropertiesError::EntryNotInPalette`] for an entry past the palette.
pub fn properties(
    material: Material,
    palette: &MaterialPalette,
) -> Result<MaterialProperties, ResolvePropertiesError> {
    let entry: &PaletteEntry =
        palette
            .get(material.entry())
            .ok_or(ResolvePropertiesError::EntryNotInPalette {
                entry: material.entry(),
                len: palette.len(),
            })?;
    let form = form_row(material.class(), entry.mechanics);
    let factor = form.albedo_factor.unwrap_or(1.0);
    let (phase_law, phase_law_origin) = match material.class().photometric_form() {
        PhotometricForm::Consolidated => (PhaseLaw::Lambert, PhaseLawOrigin::Form),
        PhotometricForm::Particulate => match PhaseLaw::try_from(entry.phase_row) {
            Ok(law) => (law, PhaseLawOrigin::Palette),
            Err(_) => (
                PhaseLaw::for_family(entry.mechanics),
                PhaseLawOrigin::FamilyDefault,
            ),
        },
    };
    Ok(MaterialProperties {
        material,
        form,
        normal_albedo_bvr: entry.normal_albedo_bvr.map(|a| a * factor),
        phase_law,
        phase_law_origin,
        grain_density: KilogramsPerCubicMetre::new(entry.grain_density_kg_m3),
        transition: entry.transition,
        mechanics: entry.mechanics,
    })
}

/// The pad's interface friction tan δ on the ground of `p` at temperature `temperature`: the
/// form's constant, or between two of its points the linear interpolation; `None` where the form
/// has no model, or outside its points' temperatures (Design note 2).
#[must_use]
pub fn interface_friction(p: &MaterialProperties, temperature: Kelvin) -> Option<f64> {
    match p.form.interface_friction? {
        FrictionModel::Constant(mu) => Some(mu),
        FrictionModel::ByTemperature(points) => {
            let t = temperature.value();
            points.windows(2).find_map(|pair| {
                let [(t0, mu0), (t1, mu1)] = [pair[0], pair[1]];
                let (t0, t1) = (t0.value(), t1.value());
                (t0 <= t && t <= t1).then(|| mu0 + (mu1 - mu0) * (t - t0) / (t1 - t0))
            })
        }
    }
}

/// An angle in degrees, as radians.
#[must_use]
const fn degrees(value: f64) -> Radians {
    Radians::new(value.to_radians())
}

/// A pressure in kilopascals, as pascals.
#[must_use]
const fn kilopascals(value: f64) -> Pascals {
    Pascals::new(value * 1_000.0)
}

/// A length in centimetres, as metres.
#[must_use]
const fn centimetres(value: f64) -> Metres {
    Metres::new(value / 100.0)
}

/// The lunar soil's specific gravity, 3.1, "we recommend a value of 3.1 for general scientific
/// and engineering analyses of lunar soils" (Carrier, Olhoeft and Mendell 1991, Lunar Sourcebook,
/// ch. 9, §9.1.3, p. 481), the grain density with which the Sourcebook turns its bulk densities
/// into porosities, n = 1 − ρ ÷ 3.1 (p. 492).
const LUNAR_SOIL_SPECIFIC_GRAVITY: f64 = 3.1;

/// The porosity of lunar soil of bulk density `bulk_g_cm3`, g cm⁻³, by the Sourcebook's own
/// n = 1 − ρ ÷ (G `ρ_w`), G = 3.1 (p. 492), unrounded, so that an entry of the Sourcebook's grain
/// density returns its bulk density exactly (its Table 9.5 rounds these to 52, 49, 44 and 46%).
#[must_use]
const fn lunar_porosity(bulk_g_cm3: f64) -> f64 {
    1.0 - bulk_g_cm3 / LUNAR_SOIL_SPECIFIC_GRAVITY
}

/// The lunar soil's shear table: the Lunar Sourcebook's recommended typical cohesion and friction
/// angle of the intercrater areas by depth, their averages (Carrier, Olhoeft and Mendell 1991,
/// ch. 9, Table 9.12, p. 510: 0–15 cm c 0.52 kPa (0.44–0.62), φ 42° (41–43); 0–30 cm 0.90 kPa
/// (0.74–1.1), 46° (44–47); 30–60 cm 3.0 kPa (2.4–3.8), 54° (52–55); 0–60 cm 1.6 kPa (1.3–1.9), 49°
/// (48–51)), with the porosities of its best bulk densities by the same intervals (Table 9.4,
/// p. 484, and the text's table on p. 492: 1.50, 1.58, 1.74 and 1.66 ± 0.05 g cm⁻³).
const LUNAR_SOIL_SHEAR: [ShearLayer; 4] = [
    ShearLayer::new(
        centimetres(0.0),
        centimetres(15.0),
        MohrCoulomb::new(kilopascals(0.52), degrees(42.0)),
        lunar_porosity(1.50),
    ),
    ShearLayer::new(
        centimetres(0.0),
        centimetres(30.0),
        MohrCoulomb::new(kilopascals(0.90), degrees(46.0)),
        lunar_porosity(1.58),
    ),
    ShearLayer::new(
        centimetres(30.0),
        centimetres(60.0),
        MohrCoulomb::new(kilopascals(3.0), degrees(54.0)),
        lunar_porosity(1.74),
    ),
    ShearLayer::new(
        centimetres(0.0),
        centimetres(60.0),
        MohrCoulomb::new(kilopascals(1.6), degrees(49.0)),
        lunar_porosity(1.66),
    ),
];

/// The citation of the lunar soil's shear table.
const LUNAR_SOIL_SHEAR_SOURCE: &str = "Carrier, Olhoeft and Mendell 1991, Lunar Sourcebook, ch. 9: Table 9.12, p. 510 (cohesion and friction angle by depth, intercrater areas); Table 9.4, p. 484, and p. 492 (bulk densities by depth; porosity n = 1 − ρ ÷ 3.1); §9.1.3, p. 481 (specific gravity 3.1)";

/// The grain density with which Moore et al. turn the Viking fine material's bulk density into a
/// porosity, 2,600 kg m⁻³: "a porosity of 58 percent if the density of the individual mineral
/// grains was 2,600 kg/m3" (Moore et al. 1987, USGS Professional Paper 1389, p. 121), an assumption
/// there, which `mars_dust`'s grain density takes too (science-r09-drainage.md §3).
const VIKING_GRAIN_DENSITY_KG_M3: f64 = 2_600.0;

/// The deepest the Viking samplers dug, 0.22 m ("Deep holes, as much as 0.22 m deep, were
/// excavated by both landers", Moore et al. 1987, USGS Professional Paper 1389, abstract): the
/// depth their soils' figures reach.
const VIKING_SAMPLED_DEPTH: Metres = Metres::new(0.22);

/// The porosity of a Viking soil of bulk density `bulk_kg_m3`, kg m⁻³, at
/// [`VIKING_GRAIN_DENSITY_KG_M3`], so that an entry of that grain density returns the bulk
/// density of Moore et al.'s Table 37.
#[must_use]
const fn viking_porosity(bulk_kg_m3: f64) -> f64 {
    1.0 - bulk_kg_m3 / VIKING_GRAIN_DENSITY_KG_M3
}

/// Mars's drift, the Viking landers' fine, weakly cohesive material: "an angle of internal
/// friction about 18°, a cohesion ranging from 0.7 to 3.0 kPa, and a bulk density of 1,200 kg/m³"
/// (Moore et al. 1987, USGS Professional Paper 1389, abstract; Table 37, p. 130: φ 14–21° from the
/// sample trenches and 17° from the surface-bearing tests, cohesions from 0.5 to 3.0 kPa by method,
/// 1,200 ± 100 kg m⁻³). The cohesion is the abstract's range's midpoint, 1.85 kPa; the porosity is
/// the bulk density's at the 2,600 kg m⁻³ grain density the report assumes
/// ([`VIKING_GRAIN_DENSITY_KG_M3`]); the depth is the samplers' ([`VIKING_SAMPLED_DEPTH`]).
const MARS_DRIFT_SHEAR: [ShearLayer; 1] = [ShearLayer::new(
    Metres::ZERO,
    VIKING_SAMPLED_DEPTH,
    MohrCoulomb::new(kilopascals(1.85), degrees(18.0)),
    viking_porosity(1_200.0),
)];

/// Viking Lander 2's crusty to cloddy material, the soil of Utopia Planitia: "angles of internal
/// friction were about 35°, and cohesions were from 0.5 to 5.2 kPa" (Moore et al. 1987, USGS
/// Professional Paper 1389, abstract; Table 37, p. 130: φ 39° from the trenches and 32.5° from the
/// surface-bearing test, 1,400 ± 200 kg m⁻³), one of the four soil-like materials Viking measured,
/// the soils of another world. The
/// cohesion is the abstract's range's midpoint, 2.85 kPa; the porosity and depth as
/// [`MARS_DRIFT_SHEAR`]'s. A terrestrial soil's strength varies with its moisture and clay, which
/// are not modelled.
const DRY_SOIL_SHEAR: [ShearLayer; 1] = [ShearLayer::new(
    Metres::ZERO,
    VIKING_SAMPLED_DEPTH,
    MohrCoulomb::new(kilopascals(2.85), degrees(35.0)),
    viking_porosity(1_400.0),
)];

/// The citation of the Viking soils' shear tables.
const VIKING_SHEAR_SOURCE: &str = "Moore, Hutton, Clow and Spitzer 1987, Physical properties of the surface materials at the Viking landing sites on Mars, USGS Professional Paper 1389: abstract (φ and the cohesion's range; the sampled depth, 0.22 m) and Table 37, p. 130 (bulk densities); p. 121 (the assumed grain density, 2,600 kg m⁻³); the cohesion is the abstract's range's midpoint";

/// A temperature in degrees Celsius, as kelvin.
#[must_use]
const fn celsius(value: f64) -> Kelvin {
    Kelvin::new(273.15 + value)
}

/// Stainless steel sliding on freshwater ice by temperature, tan δ: a model ice skate (radius of
/// curvature about 22 m, 1.67 mm wide) at 0.38 mm s⁻¹ under 2.5 N, on ice frozen from
/// demineralised water (Liefferink, Hsia, Weber and Bonn 2021, Phys. Rev. X 11, 011025, Fig. 1,
/// p. 011025-2, its fourteen points read by eye to about ±0.02 in μ and ±2 °C, but the minimum,
/// 0.039 ± 0.003 at −7.7 ± 2.3 °C, from its text on p. 011025-4). Far from the melting point it
/// follows roughly an Arrhenius dependence on temperature, of activation energy 11.5 kJ mol⁻¹, the
/// points 1.2–2.5 times the fitted law, rising steeply below about −70 °C; within a few degrees of
/// melting it rises again, "as the sliders start to plough through the ice" (Fig. 1's caption). It
/// is a slow kinetic friction under a skate's edge: a pad's contact pressure and ploughing differ,
/// unmeasured, and its static friction, and friction at speed, differ too (Kietzig,
/// Hatzikiriakos and Englezos 2010, J. Appl. Phys. 107, 081101, and Bowden and Hughes 1939, Proc.
/// R. Soc. A 172, 280, not read). Unknown below −97 °C, the colder ground of most icy bodies.
const STEEL_ON_ICE: [(Kelvin, f64); 14] = [
    (celsius(-97.0), 0.71),
    (celsius(-89.0), 0.55),
    (celsius(-84.0), 0.41),
    (celsius(-73.0), 0.36),
    (celsius(-67.0), 0.15),
    (celsius(-58.0), 0.10),
    (celsius(-51.0), 0.126),
    (celsius(-44.0), 0.095),
    (celsius(-38.0), 0.09),
    (celsius(-30.0), 0.08),
    (celsius(-21.5), 0.085),
    (celsius(-17.0), 0.07),
    (celsius(-7.7), 0.039),
    (celsius(-1.0), 0.18),
];

/// The citation of [`STEEL_ON_ICE`].
const STEEL_ON_ICE_SOURCE: &str = "Liefferink, Hsia, Weber and Bonn 2021, Phys. Rev. X 11, 011025, Fig. 1, p. 011025-2 (stainless-steel model skate on ice at 0.38 mm/s and 2.5 N, its fourteen points read by eye to about ±0.02 and ±2 °C; a skate edge's contact, not a pad's) and p. 011025-4 (the minimum, 0.039 ± 0.003 at −7.7 ± 2.3 °C); freshwater ice";

/// Fresh snow's normal albedo in the visible, 0.97, the midpoint of "a uniformly high value of
/// 0.96–0.98 across the UV and visible spectrum" (Grenfell, Warren and Mullen 1994, JGR 99 (D9),
/// 18,669, abstract), a spectral hemispherical albedo: the albedo an ice entry's reference form,
/// fresh snow or fine frost, has, against which a form's factor is taken.
///
/// Snow is a consolidated form and takes the Lambert law ([`PhaseLaw::Lambert`]), whose
/// directional-hemispherical albedo is `A_N` at every incidence, so the hemispherical figure is its
/// normal albedo exactly (decision-r10-limb §2). The nadir anisotropy R(0, 0) ÷ `A_h` of about
/// 0.9–1.0 that Design note 8 gives snow (measured on the Antarctic plateau, under its high solar
/// zenith) is the law's error, not a conversion.
const FRESH_SNOW_V: f64 = 0.97;

/// Snow's span in the visible, 0.82–0.98: old melting snow, about 0.82 at 0.5–0.55 µm (Grenfell and
/// Maykut 1977, J. Glaciol. 18, 445, as Warren 1982, Rev. Geophys. 20, 67, Fig. 6, p. 77, shows it;
/// read by eye), to fresh snow, 0.96–0.98 (Grenfell, Warren and Mullen 1994).
const SNOW_V: AlbedoSpan = AlbedoSpan::new(0.82, 0.98);

/// Clean glacier ice's span at 0.55 µm, 0.69–0.89: melting ice of the north-east Greenland
/// ablation zone, about 0.69–0.81 at 0.5–0.55 µm (Bøggild, Brandt, Brown and Warren 2010, J.
/// Glaciol. 56 (195), 101, Fig. 5, p. 105; read by eye), to cold Antarctic blue ice, about 0.89 at
/// 0.55 µm (Warren et al. 2002, JGR 107 (C10), 3167, Fig. 8, p. 31-11; read by eye). Their
/// broadband albedos, 0.5–0.65, are lower: ice absorbs in the near infrared (Design note 8).
const GLACIER_ICE_V: AlbedoSpan = AlbedoSpan::new(0.69, 0.89);

/// Sea ice's span at 0.55 µm, 0.70–0.96: bare cold first-year ice 1.4 m thick, about 0.70, to ice
/// under thick snow in spring, 0.96 below 700 nm (Brandt, Warren, Worby and Grenfell 2005, J.
/// Climate 18, 3606, Figs. 1–2, pp. 3609–3610, read by eye, and Table 3, p. 3615). Perovich et al.
/// 2002 (JGR 107 (C10), 8044) was not read.
const SEA_ICE_V: AlbedoSpan = AlbedoSpan::new(0.70, 0.96);

/// The midpoint of `span`, over [`FRESH_SNOW_V`]: the factor a form of ice takes on its entry's
/// albedo (Design note 8's rule that a class's value is its span's midpoint).
#[must_use]
const fn ice_form_factor(span: AlbedoSpan) -> f64 {
    f64::midpoint(span.low, span.high) / FRESH_SNOW_V
}

/// Volatile frost's span, 0.4–1.0: CO₂ frost and ice, whose observed visible albedos on Mars's
/// caps are about 0.5–0.6 at 0.59 µm and whose modelled hemispherical albedo at 0.5 µm falls from
/// about 0.97 to 0.41 as its grains grow from 5 µm to 2 mm, computed at a solar zenith of 60°
/// (Warren, Wiscombe and Firestone 1990, JGR 95 (B9), 14,717, p. 14,718 and Fig. 5a, p. 14,724),
/// taken as normal albedos as Design note 8 takes snow's (for a particulate frost, whose law has a
/// phase factor, an understatement of its `A_N`: decision-r10-limb's deferred findings), to the
/// N₂- and CH₄-rich plains of a Pluto, normal reflectances of 0.8–1.0 in LORRI's band, pivot
/// 0.607 µm (Buratti et al. 2017, Icarus 287, 207, read as arXiv:1604.06129, pp. 11–12 and 17–18;
/// an underestimate of the brightest regions, the authors say).
const VOLATILE_FROST_V: AlbedoSpan = AlbedoSpan::new(0.4, 1.0);

/// A volatile glacier's span, 0.8–1.0: Sputnik Planitia's N₂ ice sheet, two units centred on 0.90
/// and 0.95 and its brightest areas 1.0, inside Tombaugh Regio's 0.8–1.0, normal reflectances in
/// LORRI's band (Buratti et al. 2017, Icarus 287, 207, read as arXiv:1604.06129, pp. 11–12, 17 and
/// 18).
const VOLATILE_GLACIER_V: AlbedoSpan = AlbedoSpan::new(0.8, 1.0);

/// Steel against fine sandy silt or non-plastic silt, tan δ = 0.20 (11°), the finest of the
/// ultimate friction factors for steel sheet piles against soil (NAVFAC DM-7.02, 1986,
/// Foundations and Earth Structures, Table 1, p. 7.2-63).
const STEEL_ON_SILT: f64 = 0.20;

/// Steel against silty sand, or gravel or sand mixed with silt or clay, tan δ = 0.25 (14°)
/// (NAVFAC DM-7.02, 1986, Table 1, p. 7.2-63).
const STEEL_ON_SILTY_SAND: f64 = 0.25;

/// Steel against clean sand or a silty sand–gravel mixture, tan δ = 0.30 (17°) (NAVFAC DM-7.02,
/// 1986, Table 1, p. 7.2-63).
const STEEL_ON_CLEAN_SAND: f64 = 0.30;

/// The Moon's normal albedos, 0.057–0.184: Dollfus and Bowell's best estimates for 67 lunar
/// regions, "the albedo for these regions ranges from 5.7% to 18.4%" (Dollfus and Bowell 1971, A&A
/// 10, 29, as the Lunar Sourcebook quotes them: Carrier, Olhoeft and Mendell 1991, ch. 9, §9.3,
/// p. 560; the primary not read, and its band not stated there). The nearside's regions, the
/// Moon's alone.
const LUNAR_NORMAL_ALBEDO: AlbedoSpan = AlbedoSpan::new(0.057, 0.184);

/// Regolith of silicate: the lunar soil, its shear table ([`LUNAR_SOIL_SHEAR`]), its pad friction
/// [`STEEL_ON_SILT`], the lower of NAVFAC's two rows the soil spans, since it is "a well-graded (or
/// poorly sorted), silty sand to sandy silt: SW-SM to ML" (Lunar Sourcebook, ch. 9, p. 478), and
/// its span [`LUNAR_NORMAL_ALBEDO`].
static REGOLITH_SILICATE: FormRow = FormRow {
    shear: &LUNAR_SOIL_SHEAR,
    interface_friction: Some(FrictionModel::Constant(STEEL_ON_SILT)),
    compressive_strength: None,
    albedo_factor: None,
    normal_albedo_v: Some(LUNAR_NORMAL_ALBEDO),
    citations: &[
        Citation::new(Figure::Shear, LUNAR_SOIL_SHEAR_SOURCE),
        Citation::new(
            Figure::InterfaceFriction,
            "NAVFAC DM-7.02 (1986), Table 1, p. 7.2-63: steel against fine sandy silt, non-plastic silt, 0.20, the lower of the two rows the lunar soil spans, a silty sand to sandy silt, SW-SM to ML (Carrier, Olhoeft and Mendell 1991, Lunar Sourcebook, ch. 9, p. 478)",
        ),
        Citation::new(
            Figure::AlbedoSpan,
            "Dollfus and Bowell 1971, A&A 10, 29: best estimates of the normal albedo of 67 lunar regions, 5.7% to 18.4%, as the Lunar Sourcebook quotes them (Carrier, Olhoeft and Mendell 1991, ch. 9, §9.3, p. 560; the primary not read, its band not stated there); the nearside's regions, the Moon's alone",
        ),
    ],
};

/// Dust of silicate: Mars's drift, its shear table ([`MARS_DRIFT_SHEAR`]) and its pad friction
/// [`STEEL_ON_SILT`], NAVFAC's finest row, the drift's grains, 0.1–2.0 µm, being finer still. Its
/// albedo span stays unknown: Design note 8's 0.18–0.25 in V has no source read that gives a
/// normal albedo (Mustard and Bell 1994, GRL 21, 353, Fig. 3 and Table 1, p. 355, give about
/// 0.15 at 0.55 µm, and 0.22 near 0.59 µm, for the Olympus–Amazonis bright region, a reflectance at
/// incidence 47° and emission 44° from the telescopic segment; read by eye).
static DUST_SILICATE: FormRow = FormRow {
    shear: &MARS_DRIFT_SHEAR,
    interface_friction: Some(FrictionModel::Constant(STEEL_ON_SILT)),
    compressive_strength: None,
    albedo_factor: None,
    normal_albedo_v: None,
    citations: &[
        Citation::new(Figure::Shear, VIKING_SHEAR_SOURCE),
        Citation::new(
            Figure::InterfaceFriction,
            "NAVFAC DM-7.02 (1986), Table 1, p. 7.2-63: steel against fine sandy silt, non-plastic silt, 0.20, its finest row; the drift is finer still, its grains 0.1–2.0 µm (Moore et al. 1987, USGS Professional Paper 1389, Table 37, p. 130)",
        ),
    ],
};

/// Soil of silicate: Viking Lander 2's crusty to cloddy material ([`DRY_SOIL_SHEAR`]), with the pad
/// friction [`STEEL_ON_SILT`], NAVFAC's finest row, since its grains are as fine as the drift's,
/// 0.1–2.0 µm (Moore et al. 1987, USGS Professional Paper 1389, Table 37, p. 130).
static SOIL_SILICATE: FormRow = FormRow {
    shear: &DRY_SOIL_SHEAR,
    interface_friction: Some(FrictionModel::Constant(STEEL_ON_SILT)),
    compressive_strength: None,
    albedo_factor: None,
    normal_albedo_v: None,
    citations: &[
        Citation::new(Figure::Shear, VIKING_SHEAR_SOURCE),
        Citation::new(
            Figure::InterfaceFriction,
            "NAVFAC DM-7.02 (1986), Table 1, p. 7.2-63: steel against fine sandy silt, non-plastic silt, 0.20, its finest row; the crusty to cloddy material's grains are 0.1–2.0 µm, as the drift's (Moore et al. 1987, USGS Professional Paper 1389, Table 37, p. 130)",
        ),
    ],
};

/// Sand of silicate: clean sand, with the pad friction [`STEEL_ON_CLEAN_SAND`]; its strength
/// stays unknown (no source read).
static SAND_SILICATE: FormRow = FormRow {
    shear: &[],
    interface_friction: Some(FrictionModel::Constant(STEEL_ON_CLEAN_SAND)),
    compressive_strength: None,
    albedo_factor: None,
    normal_albedo_v: None,
    citations: &[Citation::new(
        Figure::InterfaceFriction,
        "NAVFAC DM-7.02 (1986), Table 1, p. 7.2-63: steel against clean sand, silty sand-gravel mixture, 0.30",
    )],
};

/// Sediment of silicate: a channel's or a basin's mixed sands and silts, with the pad friction
/// [`STEEL_ON_SILTY_SAND`]; a clay-rich sediment's is not modelled.
static SEDIMENT_SILICATE: FormRow = FormRow {
    shear: &[],
    interface_friction: Some(FrictionModel::Constant(STEEL_ON_SILTY_SAND)),
    compressive_strength: None,
    albedo_factor: None,
    normal_albedo_v: None,
    citations: &[Citation::new(
        Figure::InterfaceFriction,
        "NAVFAC DM-7.02 (1986), Table 1, p. 7.2-63: steel against silty sand, gravel or sand mixed with silt or clay, 0.25; a clay-rich sediment's is not modelled",
    )],
};

/// Glacier of water ice: the pad friction [`STEEL_ON_ICE`], and its span [`GLACIER_ICE_V`] with
/// the factor it gives ([`ice_form_factor`]).
static GLACIER_WATER_ICE: FormRow = FormRow {
    shear: &[],
    interface_friction: Some(FrictionModel::ByTemperature(&STEEL_ON_ICE)),
    compressive_strength: None,
    albedo_factor: Some(ice_form_factor(GLACIER_ICE_V)),
    normal_albedo_v: Some(GLACIER_ICE_V),
    citations: &[
        Citation::new(Figure::InterfaceFriction, STEEL_ON_ICE_SOURCE),
        Citation::new(
            Figure::AlbedoFactor,
            "the midpoint of the glacier ice span over fresh snow's visible albedo, 0.97, the midpoint of 0.96–0.98 (Grenfell, Warren and Mullen 1994, JGR 99 (D9), 18669, abstract)",
        ),
        Citation::new(
            Figure::AlbedoSpan,
            "Bøggild, Brandt, Brown and Warren 2010, J. Glaciol. 56 (195), 101, Fig. 5, p. 105 (melting ice, about 0.69–0.81 at 0.5–0.55 µm), and Warren et al. 2002, JGR 107 (C10), 3167, Fig. 8, p. 31-11 (cold blue ice, about 0.89 at 0.55 µm); both read by eye",
        ),
    ],
};

/// Snow of water ice: its span [`SNOW_V`] with the factor it gives ([`ice_form_factor`]); its
/// strength and friction stay unknown (Design note 2).
static SNOW_WATER_ICE: FormRow = FormRow {
    shear: &[],
    interface_friction: None,
    compressive_strength: None,
    albedo_factor: Some(ice_form_factor(SNOW_V)),
    normal_albedo_v: Some(SNOW_V),
    citations: &[
        Citation::new(
            Figure::AlbedoFactor,
            "the midpoint of the snow span over fresh snow's visible albedo, 0.97, the midpoint of 0.96–0.98 (Grenfell, Warren and Mullen 1994, JGR 99 (D9), 18669, abstract)",
        ),
        Citation::new(
            Figure::AlbedoSpan,
            "Grenfell and Maykut 1977, J. Glaciol. 18, 445, as Warren 1982, Rev. Geophys. 20, 67, Fig. 6, p. 77 (old melting snow, about 0.82 at 0.5–0.55 µm, read by eye), and Grenfell, Warren and Mullen 1994, JGR 99 (D9), 18669, abstract (fresh snow, 0.96–0.98)",
        ),
    ],
};

/// Sea ice of water ice: the pad friction [`STEEL_ON_ICE`], freshwater ice's, and its span
/// [`SEA_ICE_V`] with the factor it gives ([`ice_form_factor`]).
static SEA_ICE_WATER_ICE: FormRow = FormRow {
    shear: &[],
    interface_friction: Some(FrictionModel::ByTemperature(&STEEL_ON_ICE)),
    compressive_strength: None,
    albedo_factor: Some(ice_form_factor(SEA_ICE_V)),
    normal_albedo_v: Some(SEA_ICE_V),
    citations: &[
        Citation::new(Figure::InterfaceFriction, STEEL_ON_ICE_SOURCE),
        Citation::new(
            Figure::AlbedoFactor,
            "the midpoint of the sea ice span over fresh snow's visible albedo, 0.97, the midpoint of 0.96–0.98 (Grenfell, Warren and Mullen 1994, JGR 99 (D9), 18669, abstract)",
        ),
        Citation::new(
            Figure::AlbedoSpan,
            "Brandt, Warren, Worby and Grenfell 2005, J. Climate 18, 3606, Figs. 1–2, pp. 3609–3610 (bare cold first-year ice, about 0.70 at 0.55 µm, read by eye) and Table 3, p. 3615 (under thick snow in spring, 0.96 below 700 nm); Perovich et al. 2002 not read",
        ),
    ],
};

/// Frost of a volatile ice, CO₂, N₂ or CH₄: its span [`VOLATILE_FROST_V`].
static FROST_VOLATILE_ICE: FormRow = FormRow {
    shear: &[],
    interface_friction: None,
    compressive_strength: None,
    albedo_factor: None,
    normal_albedo_v: Some(VOLATILE_FROST_V),
    citations: &[Citation::new(
        Figure::AlbedoSpan,
        "Warren, Wiscombe and Firestone 1990, JGR 95 (B9), 14717, p. 14718 and Fig. 5a, p. 14724 (CO₂ frost, observed 0.5–0.6 at 0.59 µm, modelled 0.41–0.97 at 0.5 µm by grain size), and Buratti et al. 2017, Icarus 287, 207, as arXiv:1604.06129, pp. 11–12 and 17–18 (Pluto's N₂- and CH₄-rich plains, normal reflectance 0.8–1.0, LORRI's band)",
    )],
};

/// Glacier of a volatile ice, as Sputnik Planitia's: its span [`VOLATILE_GLACIER_V`].
static GLACIER_VOLATILE_ICE: FormRow = FormRow {
    shear: &[],
    interface_friction: None,
    compressive_strength: None,
    albedo_factor: None,
    normal_albedo_v: Some(VOLATILE_GLACIER_V),
    citations: &[Citation::new(
        Figure::AlbedoSpan,
        "Buratti et al. 2017, Icarus 287, 207, as arXiv:1604.06129, pp. 11–12, 17 and 18: Sputnik Planitia's N₂ ice, units centred on 0.90 and 0.95, its brightest areas 1.0, within Tombaugh Regio's 0.8–1.0, normal reflectances in LORRI's band (pivot 0.607 µm)",
    )],
};

/// The rows of `class` for mechanics family `family`: the class's table (Design notes 2 and 8),
/// [`FormRow::UNMODELLED`] where no figure has a source.
///
/// The modelled pairs are a closed set, for the composition audit (plan R10, Risks): regolith,
/// dust, soil, sand and sediment of silicate; glacier, snow and sea ice of water ice; frost and
/// glacier of a volatile ice. Every other pair, every form of metal, salt or organic among them, is
/// not modelled and is labelled so.
#[must_use]
pub fn form_row(class: MaterialClass, family: MechanicsFamily) -> &'static FormRow {
    use MaterialClass as C;
    use MechanicsFamily as F;
    match (class, family) {
        (C::Regolith, F::Silicate) => &REGOLITH_SILICATE,
        (C::Dust, F::Silicate) => &DUST_SILICATE,
        (C::Soil, F::Silicate) => &SOIL_SILICATE,
        (C::Sand, F::Silicate) => &SAND_SILICATE,
        (C::Sediment, F::Silicate) => &SEDIMENT_SILICATE,
        (C::Glacier, F::WaterIce) => &GLACIER_WATER_ICE,
        (C::Snow, F::WaterIce) => &SNOW_WATER_ICE,
        (C::SeaIce, F::WaterIce) => &SEA_ICE_WATER_ICE,
        (C::Frost, F::VolatileIce) => &FROST_VOLATILE_ICE,
        (C::Glacier, F::VolatileIce) => &GLACIER_VOLATILE_ICE,
        (
            C::Bedrock
            | C::Regolith
            | C::Soil
            | C::Sand
            | C::Sediment
            | C::VolcanicRock
            | C::Melt
            | C::Glacier
            | C::Snow
            | C::SeaIce
            | C::Frost
            | C::OrganicSediment
            | C::Seabed
            | C::Evaporite
            | C::Dust,
            F::Silicate | F::Metal | F::WaterIce | F::VolatileIce | F::Salt | F::Organic,
        ) => &FormRow::UNMODELLED,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::PaletteRole;
    use crate::substance_key::SubstanceKey;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    /// Every (form, family) row of the table, with its pair.
    fn every_row() -> Vec<(MaterialClass, MechanicsFamily, &'static FormRow)> {
        MaterialClass::ALL
            .iter()
            .flat_map(|&class| {
                MechanicsFamily::ALL
                    .iter()
                    .map(move |&family| (class, family, form_row(class, family)))
            })
            .collect()
    }

    fn entry(key: &str, mechanics: MechanicsFamily, albedo: f64, phase_row: u8) -> PaletteEntry {
        PaletteEntry {
            substance: SubstanceKey::new(key).unwrap(),
            role: PaletteRole::Ice,
            normal_albedo_bvr: [albedo, albedo, albedo],
            phase_row,
            grain_density_kg_m3: 917.0,
            transition: Kelvin::new(273.15),
            mechanics,
            liquid: None,
        }
    }

    /// The phase-law codes are stable bytes, 0 naming no law (plan R10, T1.a): the payload carries
    /// them in each palette entry.
    #[test]
    fn material_phase_law_codes_are_stable() {
        let table: [(PhaseLaw, u8); 3] = [
            (PhaseLaw::Regolith, 1),
            (PhaseLaw::Ice, 2),
            (PhaseLaw::Lambert, 3),
        ];
        assert_eq!(PhaseLaw::ALL.len(), table.len());
        for (&law, (want, code)) in PhaseLaw::ALL.iter().zip(table) {
            assert_eq!(law, want, "ALL is in code order");
            assert_eq!(u8::from(law), code, "{law:?}'s code");
            assert_eq!(PhaseLaw::try_from(code), Ok(law));
        }
        assert_eq!(UNASSIGNED_PHASE_ROW, 0);
        let none = PhaseLaw::try_from(UNASSIGNED_PHASE_ROW).unwrap_err();
        assert_eq!((none.kind, none.code), ("PhaseLaw", 0));
        let unknown = PhaseLaw::try_from(4).unwrap_err();
        assert_eq!((unknown.kind, unknown.code), ("PhaseLaw", 4));
        assert_eq!(PhaseLaw::Regolith.template(), "moon");
        assert_eq!(PhaseLaw::Ice.template(), "airless-ice");
        assert_eq!(PhaseLaw::Lambert.template(), "lambert");
        for &family in MechanicsFamily::ALL {
            let want = match family {
                MechanicsFamily::WaterIce | MechanicsFamily::VolatileIce => PhaseLaw::Ice,
                MechanicsFamily::Silicate
                | MechanicsFamily::Metal
                | MechanicsFamily::Salt
                | MechanicsFamily::Organic => PhaseLaw::Regolith,
            };
            assert_eq!(PhaseLaw::for_family(family), want, "{family:?}");
        }
    }

    /// Every figure of the table is finite and in its physical range: cohesion at least 0, the
    /// friction angle between 0 and 90°, porosity in [0, 1), the interval's top above its bottom,
    /// friction coefficients and strengths positive, `ByTemperature`'s points two or more and in
    /// increasing temperature, the albedo factor positive and at most 1, and a span from 0 up,
    /// finite; its top's ceiling is its law's (the bound test below).
    #[test]
    fn material_every_property_is_finite_and_in_its_physical_range() {
        for (class, family, row) in every_row() {
            let at = format!("{class:?} of {family:?}");
            for layer in row.shear() {
                let c = layer.strength().cohesion().value();
                let phi = layer.strength().friction_angle().value().to_degrees();
                assert!(c.is_finite() && c >= 0.0, "{at}: cohesion {c}");
                assert!(phi > 0.0 && phi < 90.0, "{at}: friction angle {phi}°");
                let n = layer.porosity();
                assert!((0.0..1.0).contains(&n), "{at}: porosity {n}");
                let (top, bottom) = (layer.top().value(), layer.bottom().value());
                assert!(
                    top >= 0.0 && top < bottom && bottom.is_finite(),
                    "{at}: {layer:?}"
                );
            }
            match row.interface_friction() {
                None => {}
                Some(FrictionModel::Constant(mu)) => {
                    assert!(mu > 0.0 && mu < 2.0, "{at}: tan δ {mu}");
                }
                Some(FrictionModel::ByTemperature(points)) => {
                    assert!(points.len() >= 2, "{at}: {points:?}");
                    for &(t, mu) in points {
                        assert!(t.value() > 0.0 && t.value().is_finite(), "{at}: {t:?}");
                        assert!(mu > 0.0 && mu < 2.0, "{at}: tan δ {mu}");
                    }
                    assert!(
                        points.windows(2).all(|w| w[0].0.value() < w[1].0.value()),
                        "{at}: {points:?} not in increasing temperature"
                    );
                }
            }
            if let Some(s) = row.compressive_strength() {
                assert!(s.value() > 0.0 && s.value().is_finite(), "{at}: {s:?}");
            }
            if let Some(f) = row.albedo_factor() {
                assert!(f > 0.0 && f <= 1.0, "{at}: albedo factor {f}");
            }
            if let Some(span) = row.normal_albedo_v() {
                assert!(
                    0.0 <= span.low() && span.low() <= span.high() && span.high().is_finite(),
                    "{at}: {span:?}"
                );
            }
        }
    }

    /// Every figure a row holds names its source, and no citation names a figure the row lacks.
    #[test]
    fn material_every_row_with_a_value_names_its_source() {
        let mut modelled = 0;
        for (class, family, row) in every_row() {
            let at = format!("{class:?} of {family:?}");
            let has = |figure: Figure| match figure {
                Figure::Shear => !row.shear().is_empty(),
                Figure::InterfaceFriction => row.interface_friction().is_some(),
                Figure::CompressiveStrength => row.compressive_strength().is_some(),
                Figure::AlbedoFactor => row.albedo_factor().is_some(),
                Figure::AlbedoSpan => row.normal_albedo_v().is_some(),
            };
            let cited = |figure: Figure| row.citations().iter().any(|c| c.figure() == figure);
            for figure in [
                Figure::Shear,
                Figure::InterfaceFriction,
                Figure::CompressiveStrength,
                Figure::AlbedoFactor,
                Figure::AlbedoSpan,
            ] {
                assert_eq!(has(figure), cited(figure), "{at}: {figure:?}");
            }
            for c in row.citations() {
                assert!(c.source().len() > 10, "{at}: {c:?}");
            }
            assert_eq!(row.is_modelled(), !row.citations().is_empty(), "{at}");
            modelled += usize::from(row.is_modelled());
        }
        assert!(modelled > 0, "the table models some rows");
    }

    /// Energy is bounded on the normal albedo by the law (decision-r10-limb §3): every consolidated
    /// row's span tops out at Lambert's ceiling of 1, where the law's hemispherical albedo is
    /// `A_N` at every incidence; every particulate row's at the particulate ceiling of 1.35; and
    /// every L(0) the particulate rule gives lies between Enceladus's 0.38 and the Moon's 1.
    #[test]
    fn material_each_law_bounds_its_forms_normal_albedo() {
        assert!((PhaseLaw::Lambert.normal_albedo_ceiling() - 1.0).abs() < 1e-15);
        for law in [PhaseLaw::Regolith, PhaseLaw::Ice] {
            let ceiling = law.normal_albedo_ceiling();
            assert!((ceiling - PARTICULATE_NORMAL_ALBEDO_CEILING).abs() < 1e-15);
            assert!((ceiling - 1.35).abs() < 1e-15, "{law:?}");
        }
        let mut consolidated = 0;
        let mut particulate = 0;
        for (class, family, row) in every_row() {
            let Some(span) = row.normal_albedo_v() else {
                continue;
            };
            let ceiling = match class.photometric_form() {
                PhotometricForm::Consolidated => {
                    consolidated += 1;
                    PhaseLaw::Lambert.normal_albedo_ceiling()
                }
                PhotometricForm::Particulate => {
                    particulate += 1;
                    PARTICULATE_NORMAL_ALBEDO_CEILING
                }
            };
            assert!(
                span.high() <= ceiling,
                "{class:?} of {family:?}: span {span:?} above its law's ceiling {ceiling}"
            );
        }
        assert!(consolidated > 0 && particulate > 0, "both kinds have spans");
        for i in 1..=1_000 {
            let mu0 = f64::from(i) / 1_000.0;
            for a in [0.05, 0.5, 0.79, 1.0] {
                let a_h = directional_hemispherical_albedo(a, 0.0, mu0);
                assert!(
                    (a_h - a).abs() < 1e-15,
                    "Lambert's A_h {a_h} at A_N {a}, μ₀ {mu0}"
                );
            }
        }
        for i in 0..=2_000 {
            let l = limb_parameter(f64::from(i) / 1_000.0);
            assert!(
                (BRIGHT_FROST_LIMB_PARAMETER..=1.0).contains(&l),
                "L(0) {l} at {i}"
            );
        }
        assert!((LOMMEL_SEELIGER_NORMAL_ALBEDO_RATIO - 1.227_411_277_760_219).abs() < 1e-15);
        let ls = directional_hemispherical_albedo(1.0, 1.0, 1.0);
        assert!((ls - LOMMEL_SEELIGER_NORMAL_ALBEDO_RATIO).abs() < 1e-15);
    }

    /// A span holds its ends and nothing outside them, nor a NaN.
    #[test]
    fn material_albedo_span_holds_its_ends() {
        let span = AlbedoSpan::new(0.2, 0.4);
        assert!(span.contains(0.2) && span.contains(0.3) && span.contains(0.4));
        assert!(!span.contains(0.2 - 1e-12) && !span.contains(0.4 + 1e-12));
        assert!(!span.contains(f64::NAN));
        assert!((span.low() - 0.2).abs() < 1e-15 && (span.high() - 0.4).abs() < 1e-15);
    }

    /// The particulate rule (decision-r10-limb §2): L(0) is the Moon's 1 up to a normal albedo of
    /// 0.60, and for a black surface; it falls as 0.6696 ÷ x − 0.1160 to Enceladus's 0.38 at 1.35,
    /// and stays there; a NaN gives NaN.
    #[test]
    fn material_limb_parameter_is_the_particulate_rule() {
        for a in [-1.0, -0.0, 0.0, 0.2, 0.6] {
            assert!((limb_parameter(a) - 1.0).abs() < 1e-15, "{a}");
        }
        for (a, want) in [
            (0.7, 0.840_57),
            (0.8, 0.721_00),
            (0.9, 0.628_00),
            (0.965, 0.577_89),
            (1.0, 0.553_60),
            (1.2, 0.442_00),
        ] {
            let l = limb_parameter(a);
            assert!((l - want).abs() < 1e-5, "L(0) {l} at {a}, not {want}");
            assert!((l - (0.6696 / a - 0.1160)).abs() < 1e-12, "{a}");
        }
        for a in [1.35, 1.4, 2.0, f64::INFINITY] {
            let l = limb_parameter(a);
            assert!((l - 0.38).abs() < 1e-15, "L(0) {l} at {a}");
        }
        assert!(limb_parameter(f64::NAN).is_nan());
        for (edge, outside) in [
            (LUNAR_LIMB_ONSET_NORMAL_ALBEDO, 1.0),
            (BRIGHT_FROST_NORMAL_ALBEDO, BRIGHT_FROST_LIMB_PARAMETER),
        ] {
            for x in [edge.next_down(), edge, edge.next_up()] {
                let l = limb_parameter(x);
                assert!((l - outside).abs() < 1e-12, "{l} at {x}: a step at {edge}");
            }
        }
        let mut last = limb_parameter(0.0);
        for i in 0..=2_000 {
            let l = limb_parameter(f64::from(i) / 1_000.0);
            assert!(l <= last, "L(0) {l} rises above {last} at {i}");
            last = l;
        }
    }

    /// x L(0)(x) is linear in x between the rule's anchors, so that R10.T10.a's solve of the
    /// geometric albedo c `A_N` [⅔ + ⅓ L(0)] stays piecewise linear in the scale c, with its breaks
    /// at 0.60 and 1.35 (decision-r10-limb §2).
    #[test]
    fn material_limb_parameter_keeps_the_geometric_albedo_linear_between_its_anchors() {
        let xl = |x: f64| x * limb_parameter(x);
        let h = 1e-3;
        // From 0.602 to 1.347, so that every point and its neighbours lie inside (0.60, 1.35).
        for i in 2..=747 {
            let x = LUNAR_LIMB_ONSET_NORMAL_ALBEDO + f64::from(i) * h;
            assert!(
                x + h < BRIGHT_FROST_NORMAL_ALBEDO,
                "{x} leaves the middle piece"
            );
            let second = xl(x - h) - 2.0 * xl(x) + xl(x + h);
            assert!(second.abs() < 1e-12, "second difference {second} at {x}");
        }
    }

    /// The Lambert law has no Lommel–Seeliger share at any phase or albedo, draws R07's `lambert`
    /// template, and bounds its normal albedo at 1.
    #[test]
    fn material_lambert_law_has_no_lommel_seeliger_share() {
        let law = PhaseLaw::Lambert;
        assert_eq!(law.template(), "lambert");
        for a in [-1.0, 0.0, 0.1, 0.6, 0.9, 1.0, 1.35, 2.0, f64::NAN] {
            assert!(law.limb_parameter(a).abs() < 1e-15, "L(0) at {a}");
        }
        for deg in [0.0, 30.0, 90.0, 103.9, 150.0, 180.0, -30.0, f64::NAN] {
            for l0 in [0.0, 0.38, 1.0] {
                let l = law.lommel_seeliger_share(l0, Radians::new(deg.to_radians()));
                assert!(l.abs() < 1e-15, "L({deg}°) {l} at L(0) {l0}");
            }
        }
        for a in [0.0, 0.3, 0.6, 0.9, 1.2] {
            for particulate in [PhaseLaw::Regolith, PhaseLaw::Ice] {
                let l = particulate.limb_parameter(a);
                assert!(
                    (l - limb_parameter(a)).abs() < 1e-15,
                    "{particulate:?} at {a}"
                );
            }
        }
    }

    /// L(α) is McEwen's lunar polynomial times L(0): 1 at 0°, 0.6084 at 30° and 0.1859 at 90°,
    /// and 0 from its root at 103.9° (decision-r07-t8b's figures).
    #[test]
    fn material_lommel_seeliger_share_is_mcewen_s_lunar_profile() {
        let at = |law: PhaseLaw, l0: f64, deg: f64| {
            law.lommel_seeliger_share(l0, Radians::new(deg.to_radians()))
        };
        for law in [PhaseLaw::Regolith, PhaseLaw::Ice] {
            assert!((at(law, 1.0, 0.0) - 1.0).abs() < 1e-15);
            assert!((at(law, 1.0, 30.0) - 0.6084).abs() < 1e-4);
            assert!((at(law, 1.0, 90.0) - 0.1859).abs() < 1e-4);
            assert!(at(law, 1.0, 103.8) > 0.0);
            assert!(at(law, 1.0, 104.0).abs() < 1e-15);
            assert!(at(law, 1.0, 180.0).abs() < 1e-15);
            assert!((at(law, 0.25, 30.0) - 0.25 * 0.6084).abs() < 1e-4);
            assert!((at(law, 1.0, -30.0) - at(law, 1.0, 30.0)).abs() < 1e-15);
            assert!(at(law, 1.0, f64::NAN).is_nan());
        }
    }

    /// The properties join the form's rows for the entry's family to the entry's values: the
    /// albedo times the form's factor, and the law: Lambert's for a consolidated form, and for a
    /// particulate one the law the entry's code names or, for 0 or a code this build does not
    /// know, the family's, labelled.
    #[test]
    fn material_properties_join_the_form_to_the_palette_entry() {
        let palette = MaterialPalette::new(vec![
            entry(
                "CO2",
                MechanicsFamily::VolatileIce,
                0.6,
                UNASSIGNED_PHASE_ROW,
            ),
            entry(
                "H2O",
                MechanicsFamily::WaterIce,
                0.965,
                u8::from(PhaseLaw::Ice),
            ),
            entry("N2", MechanicsFamily::VolatileIce, 0.9, 200),
        ])
        .unwrap();
        let of = |class, entry| properties(Material::new(class, entry).unwrap(), &palette).unwrap();
        for class in MaterialClass::ALL.iter().copied() {
            let p = of(class, 1);
            assert_eq!(p.material(), Material::new(class, 1).unwrap());
            assert!(std::ptr::eq(
                p.form(),
                form_row(class, MechanicsFamily::WaterIce)
            ));
            let factor = p.form().albedo_factor().unwrap_or(1.0);
            for band in p.normal_albedo_bvr() {
                assert!((band - 0.965 * factor).abs() < 1e-15, "{class:?}");
            }
            let (law, origin, l0) = match class.photometric_form() {
                PhotometricForm::Consolidated => (PhaseLaw::Lambert, PhaseLawOrigin::Form, 0.0),
                PhotometricForm::Particulate => (
                    PhaseLaw::Ice,
                    PhaseLawOrigin::Palette,
                    limb_parameter(0.965 * factor),
                ),
            };
            assert_eq!(p.phase_law(), law, "{class:?}");
            assert_eq!(p.phase_law_origin(), origin, "{class:?}");
            assert!((p.limb_parameter() - l0).abs() < 1e-15, "{class:?}");
            assert!((p.grain_density().value() - 917.0).abs() < 1e-12);
            assert!((p.transition().value() - 273.15).abs() < 1e-12);
            assert_eq!(p.mechanics(), MechanicsFamily::WaterIce);
            assert_eq!(p.citations(), p.form().citations());
        }
        let unassigned = of(MaterialClass::Frost, 0);
        assert_eq!(unassigned.phase_law(), PhaseLaw::Ice);
        assert_eq!(unassigned.phase_law_origin(), PhaseLawOrigin::FamilyDefault);
        let unknown = of(MaterialClass::Frost, 2);
        assert_eq!(unknown.phase_law(), PhaseLaw::Ice);
        assert_eq!(unknown.phase_law_origin(), PhaseLawOrigin::FamilyDefault);
        let glacier = of(MaterialClass::Glacier, 2);
        assert_eq!(glacier.phase_law(), PhaseLaw::Lambert);
        assert_eq!(glacier.phase_law_origin(), PhaseLawOrigin::Form);
        assert_eq!(
            properties(Material::new(MaterialClass::Snow, 3).unwrap(), &palette),
            Err(ResolvePropertiesError::EntryNotInPalette { entry: 3, len: 3 })
        );
        let empty = MaterialPalette::default();
        let refused = properties(Material::new(MaterialClass::Snow, 0).unwrap(), &empty);
        assert_eq!(
            refused.unwrap_err().to_string(),
            "palette entry 0 is past a palette of 0 entries"
        );
    }

    /// The form chooses the law (decision-r10-limb §2): over every family and every code an entry
    /// can carry (none, each law's, and one this build does not know), a consolidated form takes
    /// Lambert's with origin `Form`, and a particulate form keeps the entry's code, or the
    /// family's default; L(0) is always the chosen law's at the material's `A_N` in V.
    #[test]
    fn material_the_form_chooses_lambert_for_the_consolidated_forms() {
        const KEYS: [&str; 6] = ["fam_a", "fam_b", "fam_c", "fam_d", "fam_e", "fam_f"];
        assert_eq!(MechanicsFamily::ALL.len(), KEYS.len());
        let codes = [
            UNASSIGNED_PHASE_ROW,
            u8::from(PhaseLaw::Regolith),
            u8::from(PhaseLaw::Ice),
            u8::from(PhaseLaw::Lambert),
            200,
        ];
        for code in codes {
            let palette = MaterialPalette::new(
                MechanicsFamily::ALL
                    .iter()
                    .zip(KEYS)
                    .map(|(&family, key)| entry(key, family, 0.9, code))
                    .collect(),
            )
            .unwrap();
            for (index, &family) in (0_u8..).zip(MechanicsFamily::ALL) {
                for &class in MaterialClass::ALL {
                    let material = Material::new(class, index).unwrap();
                    let p = properties(material, &palette).unwrap();
                    let at = format!("{class:?} of {family:?}, code {code}");
                    assert_eq!(p.mechanics(), family, "{at}");
                    let (law, origin) = match class.photometric_form() {
                        PhotometricForm::Consolidated => (PhaseLaw::Lambert, PhaseLawOrigin::Form),
                        PhotometricForm::Particulate => match PhaseLaw::try_from(code) {
                            Ok(law) => (law, PhaseLawOrigin::Palette),
                            Err(_) => (PhaseLaw::for_family(family), PhaseLawOrigin::FamilyDefault),
                        },
                    };
                    assert_eq!((p.phase_law(), p.phase_law_origin()), (law, origin), "{at}");
                    let want = p.phase_law().limb_parameter(p.normal_albedo_bvr()[1]);
                    let got = p.limb_parameter();
                    assert!((got - want).abs() < 1e-15, "{at}: L(0) {got}, not {want}");
                }
            }
        }
    }

    /// The law and L(0) of five materials, written down: frost keeps its entry's Ice, at the
    /// rule's 0.628 for `A_N` 0.9, or its entry's Lambert, at 0; glacier and evaporite take
    /// Lambert's from their form whatever the entry's code; and a silicate regolith of an entry
    /// naming no law takes its family's, at the Moon's 1.
    #[test]
    fn material_law_selection_of_five_materials() {
        let palette = MaterialPalette::new(vec![
            entry(
                "fam_a",
                MechanicsFamily::VolatileIce,
                0.9,
                u8::from(PhaseLaw::Ice),
            ),
            entry(
                "fam_b",
                MechanicsFamily::VolatileIce,
                0.9,
                u8::from(PhaseLaw::Lambert),
            ),
            entry(
                "fam_c",
                MechanicsFamily::Silicate,
                0.1,
                UNASSIGNED_PHASE_ROW,
            ),
        ])
        .unwrap();
        let of = |class, entry| properties(Material::new(class, entry).unwrap(), &palette).unwrap();
        for (class, entry, law, origin, l0) in [
            (
                MaterialClass::Frost,
                0,
                PhaseLaw::Ice,
                PhaseLawOrigin::Palette,
                0.628,
            ),
            (
                MaterialClass::Frost,
                1,
                PhaseLaw::Lambert,
                PhaseLawOrigin::Palette,
                0.0,
            ),
            (
                MaterialClass::Glacier,
                0,
                PhaseLaw::Lambert,
                PhaseLawOrigin::Form,
                0.0,
            ),
            (
                MaterialClass::Evaporite,
                2,
                PhaseLaw::Lambert,
                PhaseLawOrigin::Form,
                0.0,
            ),
            (
                MaterialClass::Regolith,
                2,
                PhaseLaw::Regolith,
                PhaseLawOrigin::FamilyDefault,
                1.0,
            ),
        ] {
            let p = of(class, entry);
            assert_eq!(
                (p.phase_law(), p.phase_law_origin()),
                (law, origin),
                "{class:?}"
            );
            let got = p.limb_parameter();
            assert!((got - l0).abs() < 1e-12, "{class:?}: L(0) {got}, not {l0}");
        }
    }

    /// A pad's friction on ice interpolates between its measured points and is unknown outside
    /// them; a granular form's is one coefficient; a form without a model has none.
    #[test]
    fn material_interface_friction_interpolates_and_is_none_outside_its_points() {
        // In (role, key) order: "H2O" sorts before "basalt".
        let palette = MaterialPalette::new(vec![
            entry(
                "H2O",
                MechanicsFamily::WaterIce,
                0.965,
                UNASSIGNED_PHASE_ROW,
            ),
            entry(
                "basalt",
                MechanicsFamily::Silicate,
                0.1,
                UNASSIGNED_PHASE_ROW,
            ),
        ])
        .unwrap();
        let of = |class, entry| properties(Material::new(class, entry).unwrap(), &palette).unwrap();
        let ice = of(MaterialClass::Glacier, 0);
        let Some(FrictionModel::ByTemperature(points)) = ice.interface_friction_model() else {
            panic!("glacier ice has a friction by temperature");
        };
        for &(t, mu) in points {
            let got = interface_friction(&ice, t).unwrap();
            assert!((got - mu).abs() < 1e-12, "{got} at {t:?}");
        }
        for pair in points.windows(2) {
            let [(t0, mu0), (t1, mu1)] = [pair[0], pair[1]];
            let mid = Kelvin::new(f64::midpoint(t0.value(), t1.value()));
            let got = interface_friction(&ice, mid).unwrap();
            assert!(
                (got - f64::midpoint(mu0, mu1)).abs() < 1e-12,
                "{got} at {mid:?}"
            );
        }
        let (coldest, warmest) = (points[0].0.value(), points[points.len() - 1].0.value());
        assert!(interface_friction(&ice, Kelvin::new(coldest - 0.01)).is_none());
        assert!(interface_friction(&ice, Kelvin::new(warmest + 0.01)).is_none());
        assert!(interface_friction(&ice, Kelvin::new(100.0)).is_none());
        assert!(interface_friction(&ice, Kelvin::new(f64::NAN)).is_none());
        let sand = of(MaterialClass::Sand, 1);
        for t in [50.0, 288.0, 700.0] {
            let mu = interface_friction(&sand, Kelvin::new(t)).unwrap();
            assert!((mu - 0.30).abs() < 1e-15, "{mu} at {t} K");
        }
        assert!(interface_friction(&of(MaterialClass::Snow, 0), Kelvin::new(260.0)).is_none());
        assert!(interface_friction(&of(MaterialClass::Bedrock, 1), Kelvin::new(260.0)).is_none());
    }

    /// The layer at a depth is the narrowest of the Sourcebook's intervals that holds it, and
    /// below the deepest, the deepest.
    #[test]
    fn material_shear_layer_at_a_depth_is_the_narrowest_that_holds_it() {
        let palette = MaterialPalette::new(vec![entry(
            "basalt",
            MechanicsFamily::Silicate,
            0.1,
            UNASSIGNED_PHASE_ROW,
        )])
        .unwrap();
        let regolith = Material::new(MaterialClass::Regolith, 0).unwrap();
        let soil = properties(regolith, &palette).unwrap();
        let interval = |depth: f64| {
            soil.shear_layer_at(Metres::new(depth))
                .map(|l| (l.top().value(), l.bottom().value()))
        };
        let is = |depth: f64, want: (f64, f64)| {
            let got = interval(depth);
            let (top, bottom) = got.unwrap();
            assert!(
                (top - want.0).abs() < 1e-12 && (bottom - want.1).abs() < 1e-12,
                "{got:?} at {depth} m, not {want:?}"
            );
        };
        is(0.0, (0.0, 0.15));
        is(0.1, (0.0, 0.15));
        is(0.15, (0.0, 0.30));
        is(0.2, (0.0, 0.30));
        is(0.3, (0.30, 0.60));
        is(0.45, (0.30, 0.60));
        is(1.0, (0.30, 0.60));
        is(100.0, (0.30, 0.60));
        assert!(interval(-0.01).is_none());
        assert!(interval(f64::NAN).is_none());
        let snow = properties(Material::new(MaterialClass::Snow, 0).unwrap(), &palette).unwrap();
        assert!(snow.shear_layer_at(Metres::new(1.0)).is_none());
    }
}
