//! Class photometry: the one scale c on a body's class albedos that makes its scaled classes give
//! its geometric albedo p (plan R10, T10.a; Design note 8; decision-r10-limb §2).
//!
//! # The solve
//!
//! At zero phase a surface element of the lunar-Lambert law seen at emission cosine μ has the
//! radiance factor `A_N` [L + (1 − L) μ] (the law with i = e, so 2 μ₀ ÷ (μ₀ + μ) = 1; R07's
//! `body_brdf`), so a uniform sphere's geometric albedo is ∫₀¹ 2 μ `A_N` [L + (1 − L) μ] dμ =
//! `A_N` [L(0) + ⅔ (1 − L(0))] = `A_N` (2 + L(0)) ÷ 3 (derived here; the sim's
//! `planetary::derive::photometry` uses it; for L = 1 the normal albedo and the geometric albedo
//! "are equal", Buratti et al. 2017, Icarus 287, 207, read as arXiv:1604.06129, p. 10). Averaged
//! over viewing directions, a patterned sphere's p is the plain area average of its elements'
//! (Design note 8; derived here): an element adds its radiance factor times μ dA ÷ π R² to p, and
//! over all directions, each element seen from half of them with μ uniform on [0, 1],
//! ½ ∫₀¹ [L μ + (1 − L) μ²] dμ = L ÷ 4 + (1 − L) ÷ 6, so it adds `A_N` [L + ⅔ (1 − L)] dA ÷ 4π R²:
//! its own p, weighted by its area. The same holds on any convex figure, whose mean projected area
//! is a quarter of its surface (Cauchy's), so the cells' areas on the spheroid weight it. So the
//! scale is the root of
//!
//! Σ aᵢ xᵢ(c) [2 + Lᵢ(xᵢ(c))] ÷ 3 = p,
//!
//! over the materials' area fractions aᵢ ([`class_fractions`]), where xᵢ(c) is c `A_N,i` in V
//! clamped to the material's range ([`scale_range`]) and Lᵢ is its law's L(0) of the scaled albedo
//! ([`PhaseLaw::limb_parameter`]): the particulate rule for the Regolith and Ice laws, 0 for
//! Lambert's, which the consolidated forms take. x L(x) is linear in x on each piece of the
//! particulate rule, so each term is piecewise linear in c, with breaks where c `A_N,i` reaches the
//! range's ends and, for a particulate law, [`LUNAR_LIMB_ONSET_NORMAL_ALBEDO`] (0.60) and
//! [`BRIGHT_FROST_NORMAL_ALBEDO`] (1.35); [`albedo_scale`] evaluates the sum at the sorted breaks
//! and solves exactly on the piece that holds p. The sum never falls as c rises.
//!
//! # One scale, fitted in V
//!
//! The header carries one `albedo_scale` (main's answer to the re-validation's second question): c
//! is fitted in V, and the residuals in B and R are stated ([`AlbedoScale::residual`]). A material
//! keeps its colour: its B and R albedos take the factor its V albedo took, clamp included, each no
//! higher than its law's ceiling, since energy is bounded in every band (decision-r10-limb §4;
//! [`scaled_normal_albedo`]).
//!
//! # Where the ranges do not allow p
//!
//! Where no c gives p, c is the one that comes nearest, and where several do (every material
//! clamped, or a piece of the sum flat at p), the one nearest 1, the entries' own albedos, so that
//! a body is scaled no further than its classes need. c is at least [`ALBEDO_SCALE_FLOOR`], so it
//! is always the positive scale the header carries. The residual says how far p is missed.
//!
//! # The haloes
//!
//! A body whose header carries a maturity is drawn at each point's own maturity, its young craters'
//! fresh ejecta brighter than the background about them (R10.T1.c), so a scale solved at `m_body`
//! alone would draw a young body some 5% brighter than its p (the sampled mean V excess of the
//! testing Moon-like world, 5.2% at 3.5 Gyr and 5.7% at 1 Gyr: plan R10's Risks, "Deviations in
//! T1.c's Rust half, as built"). [`haloed_albedo_scale`] solves over
//! the body's maturity mix instead ([`maturity_mix`](super::maturity_mix);
//! decision-r10-t1c-halo R1): each material of share a is one term at each of the mix's nodes,
//! of share a w, at its exposed albedo, law and range there, what a point of that maturity draws,
//! and the solve above runs unchanged on the terms, exact for albedos linear between the nodes. A
//! material whose albedo does not move with the maturity keeps its one term, so a body of such
//! ground, or one on which no crater leaves a deficit, solves as before to the bit. The excess the
//! haloes add at the solved scale is stated ([`AlbedoScale::halo_excess_v`]). The mix is the
//! haloes' expectation, which leaves out the realised small craters' shot noise about it, the
//! correlation between a crater and the material it lies on, and the coarse haloes' overlaps
//! (labelled).
//!
//! Nothing here draws from a stream, so the module opens no domain tag; the sums run in a fixed
//! order, so the scale is the same to the bit on every target.

use std::collections::BTreeMap;

use super::{
    AlbedoSpan, BRIGHT_FROST_NORMAL_ALBEDO, GroundExposure, GroundMaterial,
    LUNAR_LIMB_ONSET_NORMAL_ALBEDO, MaterialProperties, MaturityMix, PhaseLaw,
    ResolvePropertiesError, exposed_properties, weights_at,
};
use crate::cube::{FaceUv, PATCH_QUADS, PatchKey, st_to_uv};
use crate::field::{FieldView, MaterialPalette, cell_at_index};
use crate::num;
use crate::synth::craters::cell_area;
use crate::synth::{BandLevel, QueryHeightError, SynthCache, Synthesiser};

/// The smallest scale the solve gives: 2⁻²⁰, about 9.5 × 10⁻⁷.
///
/// The sum the solve inverts can sit above p at every positive c, where the materials whose range
/// has a positive low end are already at it and p asks the others to be darker still; its least
/// residual is then only approached as c falls to 0, which the header refuses. Below this floor no
/// scaled albedo of 1.35 or less reaches 1.3 × 10⁻⁶, so the residual at the floor is the least
/// residual to that much. A power of two, exactly representable. A choice, not a measured figure.
pub const ALBEDO_SCALE_FLOOR: f64 = 1.0 / 1_048_576.0;

/// The relative distance within which a material's c `A_N` counts as at its range's end: 10⁻¹²,
/// since a scale that brings a material to an end is a quotient, which rounds.
const AT_END: f64 = 1e-12;

/// The band index of V in a `[B, V, R]` triple, as R09's `PaletteEntry::normal_albedo_bvr` orders
/// it.
const V: usize = 1;

/// The quadtree levels between a patch and its vertex spacing: 6, a patch's 64 quads a side
/// ([`PATCH_QUADS`]), so that a texel of band level n is a cell of quadtree level n + 6 (R10.T2's
/// record, "The texel").
const TEXEL_LEVELS: u32 = PATCH_QUADS.trailing_zeros();

/// The area fractions of a body's ground: each material's share of its surface, over every cell of
/// its coarse field ([`class_fractions`]), the shares summing to 1 to rounding.
///
/// A material is a form with the palette entry it is made of, where the palette names one
/// ([`GroundMaterial`]). The shares are kept in the materials' order, so that two sets of the same
/// shares are one set, however they were gathered.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassFractions {
    shares: Vec<(GroundMaterial, f64)>,
}

/// Why [`ClassFractions::new`] refused its areas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildClassFractionsError {
    /// A material's area is not finite or is negative.
    Area {
        /// The material.
        material: GroundMaterial,
        /// Its area.
        area: f64,
    },
    /// The areas sum to nothing: no material covers any ground.
    Empty,
    /// The areas, each finite, sum past the largest finite value.
    TotalNotFinite,
}

impl std::fmt::Display for BuildClassFractionsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Area { material, area } => {
                write!(
                    f,
                    "the area {area} of {material:?} is not finite and non-negative"
                )
            }
            Self::Empty => f.write_str("the materials' areas sum to nothing"),
            Self::TotalNotFinite => f.write_str("the materials' areas sum past a finite value"),
        }
    }
}

impl std::error::Error for BuildClassFractionsError {}

impl ClassFractions {
    /// The fractions of the materials covering `areas`, each a material and the area it covers in
    /// any unit, a material named more than once taking the sum of its areas in the order given.
    ///
    /// A material of no area is not held.
    ///
    /// # Errors
    ///
    /// [`BuildClassFractionsError::Area`] for an area that is not finite or is negative,
    /// [`BuildClassFractionsError::Empty`] where the areas sum to nothing, and
    /// [`BuildClassFractionsError::TotalNotFinite`] where they sum past a finite value.
    pub fn new(
        areas: impl IntoIterator<Item = (GroundMaterial, f64)>,
    ) -> Result<Self, BuildClassFractionsError> {
        let mut by_material: BTreeMap<GroundMaterial, f64> = BTreeMap::new();
        for (material, area) in areas {
            if !area.is_finite() || area < 0.0 {
                return Err(BuildClassFractionsError::Area { material, area });
            }
            *by_material.entry(material).or_insert(0.0) += area;
        }
        // Summed in the materials' order, so that the shares do not depend on the areas' order,
        // and from +0, since `Sum` starts from −0. Every partial sum is at most the total, so a
        // finite total leaves every material's finite.
        let total = by_material.values().fold(0.0, |sum, &area| sum + area);
        if !total.is_finite() {
            return Err(BuildClassFractionsError::TotalNotFinite);
        }
        if total <= 0.0 {
            return Err(BuildClassFractionsError::Empty);
        }
        let shares = by_material
            .into_iter()
            .filter(|&(_, area)| area > 0.0)
            .map(|(material, area)| (material, area / total))
            .collect();
        Ok(Self { shares })
    }

    /// Each material held with its share, in the materials' order.
    #[must_use]
    pub fn shares(&self) -> &[(GroundMaterial, f64)] {
        &self.shares
    }

    /// The share of `material`: 0 where it is not held.
    #[must_use]
    pub fn share(&self, material: GroundMaterial) -> f64 {
        self.shares
            .binary_search_by(|(m, _)| m.cmp(&material))
            .map_or(0.0, |at| self.shares[at].1)
    }

    /// The share of the ground whose palette names no substance, outside the solve.
    #[must_use]
    pub fn unnamed_share(&self) -> f64 {
        self.shares
            .iter()
            .filter(|(m, _)| m.material().is_none())
            .fold(0.0, |sum, &(_, share)| sum + share)
    }
}

/// The area fractions of the materials over every cell of the field `synth` reads: each cell's
/// class weights at its centre ([`weights_at`]), weighted by the cell's area on the body's figure
/// ([`cell_area`]), in cell-index order.
///
/// The weights are taken at band level L − 6 for a field of level L, at which a texel is the cell
/// (R10.T2's record, "The texel"), so that each gives the expected share of each finest-level
/// material over its cell (Design note 3): every cell counts, surveyed or not, which is why the
/// server, holding the whole field, computes them (Design note 8). A field coarser than level 6
/// takes band level 0, the coarsest, whose texel is a quarter or less of its cell. `cache` is the
/// caller's, and no answer depends on what it holds.
///
/// # Errors
///
/// [`QueryHeightError::NotSurveyed`] naming the first cell read that the view does not hold: never
/// for a [`CoarseField`](crate::field::CoarseField), which holds every cell.
///
/// # Panics
///
/// As [`weights_at`], if a height is not finite (a bug).
pub fn class_fractions<F: FieldView>(
    synth: &Synthesiser<'_, F>,
    cache: &mut SynthCache,
) -> Result<ClassFractions, QueryHeightError> {
    let header = synth.field().header();
    let level = header.level();
    let figure = header.figure();
    let band = BandLevel::new(
        u8::try_from(u32::from(level.get()).saturating_sub(TEXEL_LEVELS))
            .expect("a coarse level less 6 fits a u8"),
    )
    .expect("a coarse level less 6 is a band level");
    let mut areas: BTreeMap<GroundMaterial, f64> = BTreeMap::new();
    for index in 0..level.cell_count() {
        let cell =
            cell_at_index(level.get(), index).expect("every index below the count is a cell");
        let area = cell_area(cell, &figure).value();
        let weights = weights_at(synth, cache, centre(cell), band)?;
        for (material, weight) in weights.iter() {
            *areas.entry(material).or_insert(0.0) +=
                area * f64::from(weight) / f64::from(super::WEIGHT_TOTAL);
        }
    }
    Ok(ClassFractions::new(areas)
        .expect("a field's cells have finite, positive areas and weights summing to the whole"))
}

/// The centre of `cell`, at the middle of its face coordinates' span.
#[must_use]
fn centre(cell: PatchKey) -> FaceUv {
    let side = f64::from(1_u32 << cell.level());
    let middle = |index: u32| st_to_uv((f64::from(index) + 0.5) / side);
    FaceUv {
        face: cell.face(),
        u: middle(cell.i()),
        v: middle(cell.j()),
    }
}

/// The range a material's normal albedo in V is scaled across: its form's physical span, where its
/// table has one ([`FormRow::normal_albedo_v`](super::FormRow::normal_albedo_v)), stretched to hold
/// the material's own albedo, and in any case no higher than its law's ceiling
/// ([`PhaseLaw::normal_albedo_ceiling`]: 1 for Lambert's, 1.35 for the particulate laws;
/// decision-r10-limb §2–§3). Without a span it is 0 to the ceiling.
///
/// The spans are Solar System anchors, the lunar regolith's the nearside's 0.057–0.184 among them,
/// and narrower than a generated body's materials (R10.T1.a's deferred finding, "The spans as a
/// clamp": "T10.a clamps by its own rule"): the palette's entry is the substance's own, resolved
/// from its registry row, so the scale may carry a material across its span but never clamps it
/// away from its own albedo, which c = 1 always keeps, up to the ceiling.
///
/// Public for R10.T10.b's terrain shading and the consoles' readings, which draw and quote a
/// material's albedo under the header's scale as the solve took it.
#[must_use]
pub fn scale_range(material: &MaterialProperties) -> AlbedoSpan {
    let own = material.normal_albedo_bvr()[V];
    let ceiling = material.phase_law().normal_albedo_ceiling();
    match material.form().normal_albedo_v() {
        Some(span) => AlbedoSpan::new(
            num::min(span.low(), own),
            num::min(num::max(span.high(), own), ceiling),
        ),
        None => AlbedoSpan::new(0.0, ceiling),
    }
}

/// The normal albedo in B, V and R of `material` under the scale `scale`: c `A_N` in V clamped to
/// its [`scale_range`], and its B and R times the factor its V took, so that its colour holds, each
/// no higher than its law's ceiling ([`PhaseLaw::normal_albedo_ceiling`]), since energy is bounded
/// in every band (decision-r10-limb §4: "Each row's `A_N` is ≤ 1.35 per band").
///
/// A material black in V is not scaled: its V stays 0 and its B and R are its entry's, under the
/// ceiling. Its L(0) under the scale is its law's of the V returned ([`PhaseLaw::limb_parameter`]).
#[must_use]
pub fn scaled_normal_albedo(material: &MaterialProperties, scale: f64) -> [f64; 3] {
    let range = scale_range(material);
    Term::new(
        1.0,
        material.normal_albedo_bvr(),
        material.phase_law(),
        range,
    )
    .scaled(scale)
}

/// The scale a body's class albedos take ([`albedo_scale`]), with what it gives.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlbedoScale {
    scale: f64,
    geometric_albedo_bvr: [f64; 3],
    target_bvr: [f64; 3],
    clamped_low: f64,
    clamped_high: f64,
    unnamed: f64,
    halo_excess_v: f64,
}

impl AlbedoScale {
    /// The scale c, positive and finite: what `FieldHeader::albedo_scale` carries.
    #[must_use]
    pub const fn scale(&self) -> f64 {
        self.scale
    }

    /// The geometric albedo in B, V and R of the scaled materials, area-weighted over the named
    /// ground.
    #[must_use]
    pub const fn geometric_albedo_bvr(&self) -> [f64; 3] {
        self.geometric_albedo_bvr
    }

    /// The body's geometric albedo in B, V and R, the target.
    #[must_use]
    pub const fn target_bvr(&self) -> [f64; 3] {
        self.target_bvr
    }

    /// The scaled materials' geometric albedo less the body's, in B, V and R: in V, zero to
    /// rounding wherever the ranges allow the body's p.
    #[must_use]
    pub fn residual(&self) -> [f64; 3] {
        [0, 1, 2].map(|band| self.geometric_albedo_bvr[band] - self.target_bvr[band])
    }

    /// [`residual`](Self::residual) over the body's geometric albedo, band by band.
    #[must_use]
    pub fn relative_residual(&self) -> [f64; 3] {
        let residual = self.residual();
        [0, 1, 2].map(|band| residual[band] / self.target_bvr[band])
    }

    /// The share of the named ground whose scaled albedo in V is held up at its range's low end, or
    /// sits at it (within 10⁻¹² of it, relative); a material black in V is never counted.
    #[must_use]
    pub const fn clamped_low_share(&self) -> f64 {
        self.clamped_low
    }

    /// The share of the named ground whose scaled albedo in V is held down at its range's high
    /// end, or sits at it (within 10⁻¹² of it, relative).
    #[must_use]
    pub const fn clamped_high_share(&self) -> f64 {
        self.clamped_high
    }

    /// The share of the whole ground whose palette names no substance, which the solve leaves out:
    /// its p is taken to be the named ground's.
    #[must_use]
    pub const fn unnamed_share(&self) -> f64 {
        self.unnamed
    }

    /// The mean excess in V that the young craters' haloes add to the named ground's geometric
    /// albedo at the solved scale, dimensionless: the model in V over the body's maturity mix,
    /// which the scale is solved on, over the same materials' model at the body's maturity alone,
    /// less 1 ([`haloed_albedo_scale`]; decision-r10-t1c-halo R1). 0 without a mix, from
    /// [`exposed_albedo_scale`] and [`albedo_scale`], and where every material is black in V.
    #[must_use]
    pub const fn halo_excess_v(&self) -> f64 {
        self.halo_excess_v
    }
}

/// Why [`albedo_scale`] gave no scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SolveAlbedoScaleError {
    /// The body's geometric albedo in a band is not finite and positive.
    GeometricAlbedo {
        /// The band: `"B"`, `"V"` or `"R"`.
        band: &'static str,
        /// Its value.
        value: f64,
    },
    /// The palette names no substance for any of the ground: nothing has an albedo to scale.
    NoNamedGround,
    /// A material's entry is past the palette.
    Properties(ResolvePropertiesError),
}

impl std::fmt::Display for SolveAlbedoScaleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GeometricAlbedo { band, value } => write!(
                f,
                "the body's geometric albedo in {band}, {value}, is not finite and positive"
            ),
            Self::NoNamedGround => f.write_str("the palette names no substance for the ground"),
            Self::Properties(_) => f.write_str("a material's properties cannot be resolved"),
        }
    }
}

impl std::error::Error for SolveAlbedoScaleError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Properties(source) => Some(source),
            Self::GeometricAlbedo { .. } | Self::NoNamedGround => None,
        }
    }
}

impl From<ResolvePropertiesError> for SolveAlbedoScaleError {
    fn from(error: ResolvePropertiesError) -> Self {
        Self::Properties(error)
    }
}

/// The scale on the class albedos of a body of palette `palette` whose ground's area fractions are
/// `fractions` that makes the scaled materials' geometric albedo in V the body's,
/// `geometric_albedo_bvr[1]` (plan 14's visual geometric albedo, in B, V and R), with the residual
/// in each band (the `albedo` module documentation).
///
/// Each material's albedo is its entry's times its form's factor, its law the form's or the
/// entry's ([`properties()`](super::properties())): the fresh, uncovered albedos,
/// [`exposed_albedo_scale`] at [`GroundExposure::NONE`]. The ground whose palette names no
/// substance is left out, and the named ground is solved for the body's p. The answer depends on
/// the fractions, the palette and p alone.
///
/// # Errors
///
/// As [`exposed_albedo_scale`].
pub fn albedo_scale(
    fractions: &ClassFractions,
    palette: &MaterialPalette,
    geometric_albedo_bvr: [f64; 3],
) -> Result<AlbedoScale, SolveAlbedoScaleError> {
    exposed_albedo_scale(
        fractions,
        palette,
        GroundExposure::NONE,
        geometric_albedo_bvr,
    )
}

/// The scale on the class albedos of a body of palette `palette` whose ground's area fractions are
/// `fractions`, its ground exposed as `exposure` has it, that makes the scaled materials'
/// geometric albedo in V the body's, `geometric_albedo_bvr[1]`, with the residual in each band
/// (the `albedo` module documentation; R10.T1.c, decision-r10-t1c-maturity R2).
///
/// Each material's albedo is its [`exposed_properties`] at `exposure`: its entry's weathered to the
/// exposure's maturity, times its form's factor, blended with the organic cover where its entry
/// takes one, its law the form's or the entry's. The server solves it at the header's exposure
/// ([`GroundExposure::of_header`]), so that the scale never undoes the weathering: R10 draws c ×
/// the weathered albedo (decision-p14-t51d-weathering §1.5). At [`GroundExposure::NONE`] it is
/// [`albedo_scale`], the fresh solve. The ground whose palette names no substance is left out, and
/// the named ground is solved for the body's p. The answer depends on the fractions, the palette,
/// the exposure and p alone.
///
/// # Errors
///
/// [`SolveAlbedoScaleError::GeometricAlbedo`] for a band of `geometric_albedo_bvr` that is not finite
/// and positive, [`SolveAlbedoScaleError::NoNamedGround`] where no material names an entry, and
/// [`SolveAlbedoScaleError::Properties`] for an entry past `palette`.
///
/// # Panics
///
/// As [`exposed_properties`], for an entry whose values a [`MaterialPalette`] refuses (a bug).
pub fn exposed_albedo_scale(
    fractions: &ClassFractions,
    palette: &MaterialPalette,
    exposure: GroundExposure,
    geometric_albedo_bvr: [f64; 3],
) -> Result<AlbedoScale, SolveAlbedoScaleError> {
    let named = named_share(fractions, geometric_albedo_bvr)?;
    let mut terms = Vec::with_capacity(fractions.shares().len());
    for &(ground, share) in fractions.shares() {
        let Some(material) = ground.material() else {
            continue;
        };
        let p = exposed_properties(material, palette, exposure)?;
        terms.push(Term::of(share / named, &p));
    }
    Ok(solved(
        &terms,
        geometric_albedo_bvr,
        fractions.unnamed_share(),
    ))
}

/// The scale on the class albedos of a body of palette `palette` whose ground's area fractions are
/// `fractions`, its ground exposed as `exposure` has it and its maturity spread over its young
/// craters' haloes as `mix` gives it ([`maturity_mix`](super::maturity_mix)), that makes the
/// scaled materials' geometric albedo in V the body's, `geometric_albedo_bvr[1]`, with the
/// residual in each band and the excess the haloes add (the `albedo` module documentation, "The
/// haloes"; R10.T1.c, decision-r10-t1c-halo R1).
///
/// Each named material of share a becomes one term per node of the mix, of share a w, its albedo,
/// law and range its [`exposed_properties`] at the node's maturity, the cover kept: what the worker
/// draws at a point of that maturity, `scaled_normal_albedo` of the point's exposed properties.
/// A material whose exposed albedo at the mix's least maturity is its albedo at `m_body` to the
/// bit in every band (`Melt`, an entry of `FeO_T` `None` or 0, an uncovered ice) keeps one term, of
/// share a at `exposure`. So a body of such ground, or a mix of one node, solves as
/// [`exposed_albedo_scale`] to the bit, its [`halo_excess_v`](AlbedoScale::halo_excess_v) 0. The
/// solve is [`exposed_albedo_scale`]'s exact piecewise-linear one, unchanged.
///
/// # Errors
///
/// As [`exposed_albedo_scale`].
///
/// # Panics
///
/// If `mix`'s background maturity is not `exposure`'s to the bit, or `exposure` carries none: the
/// mix is of the header whose exposure is solved, so another is a bug. And as
/// [`exposed_properties`].
pub fn haloed_albedo_scale(
    fractions: &ClassFractions,
    palette: &MaterialPalette,
    exposure: GroundExposure,
    mix: &MaturityMix,
    geometric_albedo_bvr: [f64; 3],
) -> Result<AlbedoScale, SolveAlbedoScaleError> {
    assert!(
        exposure
            .maturity()
            .is_some_and(|m| m.total_cmp(&mix.body()).is_eq()),
        "a maturity mix of m_body {} is solved at its own maturity, not at {:?}",
        mix.body(),
        exposure.maturity()
    );
    let named = named_share(fractions, geometric_albedo_bvr)?;
    let least = mix.nodes().last().expect("a mix holds a node").maturity();
    let mut alone = Vec::with_capacity(fractions.shares().len());
    let mut terms = Vec::with_capacity(fractions.shares().len() * mix.nodes().len());
    for &(ground, share) in fractions.shares() {
        let Some(material) = ground.material() else {
            continue;
        };
        let a = share / named;
        let at_body = exposed_properties(material, palette, exposure)?;
        let term = Term::of(a, &at_body);
        alone.push(term);
        let at_least = exposed_properties(material, palette, exposure.with_maturity(least))?;
        // To the bit, as `total_cmp` orders them: the same value of the same sign in every band.
        let unmoved = at_least
            .normal_albedo_bvr()
            .iter()
            .zip(&at_body.normal_albedo_bvr())
            .all(|(least, body)| least.total_cmp(body).is_eq());
        if unmoved {
            terms.push(term);
            continue;
        }
        for node in mix.nodes() {
            let p = exposed_properties(material, palette, exposure.with_maturity(node.maturity()))?;
            terms.push(Term::of(a * node.weight(), &p));
        }
    }
    let solved = solved(&terms, geometric_albedo_bvr, fractions.unnamed_share());
    let without = model_v(&alone, solved.scale);
    let halo_excess_v = if without > 0.0 {
        model_v(&terms, solved.scale) / without - 1.0
    } else {
        0.0
    };
    Ok(AlbedoScale {
        halo_excess_v,
        ..solved
    })
}

/// The named ground's share of `fractions`, once `geometric_albedo_bvr` is checked.
///
/// # Errors
///
/// [`SolveAlbedoScaleError::GeometricAlbedo`] for a band of `geometric_albedo_bvr` that is not
/// finite and positive, and [`SolveAlbedoScaleError::NoNamedGround`] where no material names an
/// entry.
fn named_share(
    fractions: &ClassFractions,
    geometric_albedo_bvr: [f64; 3],
) -> Result<f64, SolveAlbedoScaleError> {
    for (band, &value) in ["B", "V", "R"].into_iter().zip(&geometric_albedo_bvr) {
        if !value.is_finite() || value <= 0.0 {
            return Err(SolveAlbedoScaleError::GeometricAlbedo { band, value });
        }
    }
    let named = fractions
        .shares()
        .iter()
        .filter(|(m, _)| m.material().is_some())
        .fold(0.0, |sum, &(_, share)| sum + share);
    if named <= 0.0 {
        return Err(SolveAlbedoScaleError::NoNamedGround);
    }
    Ok(named)
}

/// The solve of `terms` for the V of `geometric_albedo_bvr`, with what it gives, `unnamed` the
/// share of the ground left out, and no halo excess.
#[must_use]
fn solved(terms: &[Term], geometric_albedo_bvr: [f64; 3], unnamed: f64) -> AlbedoScale {
    let target = geometric_albedo_bvr[V];
    let scale = solve(terms, target);
    let mut clamped_low = 0.0;
    let mut clamped_high = 0.0;
    for term in terms.iter().filter(|term| term.albedo[V] > 0.0) {
        let wanted = scale * term.albedo[V];
        if wanted <= term.range.low() * (1.0 + AT_END) {
            clamped_low += term.share;
        } else if wanted >= term.range.high() * (1.0 - AT_END) {
            clamped_high += term.share;
        }
    }
    AlbedoScale {
        scale,
        geometric_albedo_bvr: model(terms, scale),
        target_bvr: geometric_albedo_bvr,
        clamped_low,
        clamped_high,
        unnamed,
        halo_excess_v: 0.0,
    }
}

/// One material in the solve: its share of the named ground, its normal albedo in B, V and R, its
/// law and the range its V is scaled across.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Term {
    share: f64,
    albedo: [f64; 3],
    law: PhaseLaw,
    range: AlbedoSpan,
}

impl Term {
    #[must_use]
    const fn new(share: f64, albedo: [f64; 3], law: PhaseLaw, range: AlbedoSpan) -> Self {
        Self {
            share,
            albedo,
            law,
            range,
        }
    }

    /// The term of a material of properties `p` and share `share`: its albedo, its law and its
    /// [`scale_range`].
    #[must_use]
    fn of(share: f64, p: &MaterialProperties) -> Self {
        Self::new(share, p.normal_albedo_bvr(), p.phase_law(), scale_range(p))
    }

    /// Whether the material's scaled albedo moves with the scale at `scale`: c `A_N` lies strictly
    /// inside its range.
    #[must_use]
    fn moves(&self, scale: f64) -> bool {
        let wanted = scale * self.albedo[V];
        self.albedo[V] > 0.0 && self.range.low() < wanted && wanted < self.range.high()
    }

    /// The scaled V, c `A_N` clamped to the range.
    #[must_use]
    fn scaled_v(&self, scale: f64) -> f64 {
        num::max(
            self.range.low(),
            num::min(scale * self.albedo[V], self.range.high()),
        )
    }

    /// The scaled albedo in B, V and R ([`scaled_normal_albedo`]).
    #[must_use]
    fn scaled(&self, scale: f64) -> [f64; 3] {
        let v = self.scaled_v(scale);
        let own = self.albedo[V];
        let factor = if own > 0.0 { v / own } else { 1.0 };
        let ceiling = self.law.normal_albedo_ceiling();
        let band = |albedo: f64| num::min(albedo * factor, ceiling);
        [band(self.albedo[0]), v, band(self.albedo[2])]
    }

    /// The material's geometric albedo in V under the scale, x (2 + L(x)) ÷ 3.
    #[must_use]
    fn geometric_v(&self, scale: f64) -> f64 {
        let v = self.scaled_v(scale);
        v * (2.0 + self.law.limb_parameter(v)) / 3.0
    }

    /// The scales at which its geometric albedo in V changes slope: where c `A_N` reaches its
    /// range's ends, and for a particulate law the particulate rule's anchors.
    fn breaks(&self) -> impl Iterator<Item = f64> + '_ {
        let own = self.albedo[V];
        let anchors: &[f64] = match self.law {
            PhaseLaw::Regolith | PhaseLaw::Ice => {
                &[LUNAR_LIMB_ONSET_NORMAL_ALBEDO, BRIGHT_FROST_NORMAL_ALBEDO]
            }
            PhaseLaw::Lambert => &[],
        };
        [self.range.low(), self.range.high()]
            .into_iter()
            .chain(anchors.iter().copied())
            .filter(move |_| own > 0.0)
            .map(move |x| x / own)
    }
}

/// The scaled materials' geometric albedo in V under `scale`, summed in the terms' order from +0.
#[must_use]
fn model_v(terms: &[Term], scale: f64) -> f64 {
    terms
        .iter()
        .fold(0.0, |sum, term| sum + term.share * term.geometric_v(scale))
}

/// The scaled materials' geometric albedo in B, V and R under `scale`, each material's band
/// albedo times its (2 + L) ÷ 3, L its law's of its scaled V.
#[must_use]
fn model(terms: &[Term], scale: f64) -> [f64; 3] {
    let mut sum = [0.0; 3];
    for term in terms {
        let scaled = term.scaled(scale);
        let shape = (2.0 + term.law.limb_parameter(scaled[V])) / 3.0;
        for (band, total) in sum.iter_mut().enumerate() {
            *total += term.share * scaled[band] * shape;
        }
    }
    // V as the solve sums it, so that the stated residual is the solve's own.
    sum[V] = model_v(terms, scale);
    sum
}

/// The scale whose model is nearest `target`, nearest 1 among those that tie, at least
/// [`ALBEDO_SCALE_FLOOR`].
#[must_use]
fn solve(terms: &[Term], target: f64) -> f64 {
    let mut knots: Vec<f64> = core::iter::once(ALBEDO_SCALE_FLOOR)
        .chain(
            terms
                .iter()
                .flat_map(Term::breaks)
                .filter(|&c| c.is_finite() && c > ALBEDO_SCALE_FLOOR),
        )
        .collect();
    knots.sort_by(f64::total_cmp);
    knots.dedup_by(|a, b| a.total_cmp(b).is_eq());
    let values: Vec<f64> = knots.iter().map(|&c| model_v(terms, c)).collect();
    let (low, high) = solutions(terms, &knots, &values, target);
    if 1.0 < low {
        low
    } else if 1.0 > high {
        high
    } else {
        1.0
    }
}

/// The interval of scales, from `knots[0]` up, at which the model of `terms` is nearest `target`,
/// given its `values` at the sorted `knots`, between which it is linear and beyond the last of
/// which it is constant; its upper end infinite where the model is nearest from some knot on.
///
/// A piece between two knots is flat where no material moves on it, every one held at an end of
/// its range, which its middle tells: its ends' values, computed apart, may differ in their last
/// bits there.
#[must_use]
fn solutions(terms: &[Term], knots: &[f64], values: &[f64], target: f64) -> (f64, f64) {
    let last = values.len() - 1;
    let flat = |k: usize| {
        k >= last || {
            let middle = f64::midpoint(knots[k], knots[k + 1]);
            !terms.iter().any(|term| term.moves(middle))
        }
    };
    // The scale at which the flat run of pieces from knot `from` ends: infinite where it runs past
    // the last knot.
    let run_end = |from: usize| {
        (from..=last)
            .find(|&k| !flat(k))
            .map_or(f64::INFINITY, |k| knots[k])
    };
    if target <= values[0] {
        return (knots[0], run_end(0));
    }
    if target >= values[last] {
        let start = (0..last).rev().find(|&k| !flat(k)).map_or(0, |k| k + 1);
        return (knots[start], f64::INFINITY);
    }
    // values[0] < target < values[last]: the first piece that reaches the target holds the root,
    // and it rises.
    let k = (0..last)
        .find(|&k| values[k + 1] >= target)
        .expect("the last knot's value is above the target");
    let (c0, c1, f0, f1) = (knots[k], knots[k + 1], values[k], values[k + 1]);
    let root = num::min(c0 + (target - f0) * (c1 - c0) / (f1 - f0), c1);
    // `f1` is at least the target; where it is not above it, the piece ends on the target, and a
    // flat run from there keeps it.
    if target < f1 {
        (root, root)
    } else {
        (root, run_end(k + 1))
    }
}

#[cfg(test)]
mod tests;
