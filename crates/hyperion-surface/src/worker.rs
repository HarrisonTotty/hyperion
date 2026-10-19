//! What a client's height worker holds and answers for real bodies (plan R10, T6.b): a
//! [`PartialField`] a body, built from R09's payload blocks as the render thread posts them, and
//! over it R10's bake ([`bake_terrain`]), readings ([`surveyed_reading`], [`slant_range`]),
//! material ([`material_at`] with [`properties`]) and the photometry the lit terrain draws a
//! bake's palette with ([`WorkerFields::photometry`], R10.T10.b).
//!
//! The client's WebAssembly module wraps [`WorkerFields`] in its `SurfaceFields` export
//! (`src/wasm.rs`) thinly: the decoding, the merge, the survey's change, the level table and each
//! query's synthesis are here, so that the native and wasip1 tests check them as the browser
//! target runs them, and the client's worker test holds the module's answers to this module's on a
//! fixture field (`tests/golden/worker/moon_like.golden`, [`golden`]).
//!
//! # A body's field
//!
//! [`WorkerFields::init`] starts a body's field from a block 0, which carries the field's header
//! (R09's Design note 17), replacing any field of the body held, and [`WorkerFields::chunk`] adds
//! each later block, a later payload's block 0 among them, each through [`decode_block`] and
//! [`PartialField::insert`] (R10's Design note 16). A block is taken whole or refused whole
//! ([`TakeBlockError`]), the field then as it was. [`WorkerFields::release`] drops the body.
//!
//! Each block taken answers with what it changed of the field ([`SurveyChange`]): the cells it
//! surveyed that the field did not hold as surveyed, and the cells it brought, surveyed or margin,
//! that the field did not hold at all. A block can change a baked patch only there, or where the
//! patch was not complete, which is the rebake rule below.
//!
//! # The rebake rule (Design note 16, as built)
//!
//! Cells arrive exact and whole with their margins, so a block changes no value at a vertex the
//! field held before it, surveyed or answered from a margin, and a resolution code alone changes no
//! byte (R10.T6.a). A patch baked before a block is baked again after it exactly where
//!
//! 1. the bake was not [complete](crate::bake::TerrainLayers::is_complete): some vertex took the
//!    reference surface, or has no weights, because the field lacked a cell its reads reach, which
//!    any block may bring, whether or not it changes the mask (main's ruling of 2026-10-14 on
//!    R10.T6.a's question);
//! 2. the block surveys a cell that holds one of the patch's vertices outside the survey, so that
//!    its survey mask changes: the change meets the bake's [`BodyBake::unsurveyed_cells`]; or
//! 3. the block brings a cell, surveyed or margin, that the bake's horizon map's far field was
//!    refused for ([`HorizonMap::refused_cells`](crate::bake::HorizonMap::refused_cells)): a
//!    refused height names the first cell its reads lacked, every read before it held and final,
//!    so it can change only once that cell is held, and a block that brings none of them leaves
//!    the map's words as they are (decision-r10-t8a §3, R10.T8.b). A margin cell answers a far
//!    height with the true ground, so "brings" counts margin as well as surveyed ground.
//!
//! The client's `workers/rebake.ts` (`needsRebake`) applies the rule to the bakes it holds, from
//! each bake's [`unsurveyed_cells`](BodyBake::unsurveyed_cells), its horizon map's refused cells
//! and its completeness, and each block's [`SurveyChange`]. A bake in flight while its body's blocks
//! arrive is taken conservatively there: baked again unless it is complete, its horizon map is
//! [complete](crate::bake::HorizonMap::is_complete), and its mask is whole.
//!
//! # The synthesis
//!
//! Each query builds the body's [`Synthesiser`] over its field afresh (1 to 10 ms, R09's figure),
//! since a synthesiser borrows the field, which the next block changes. The [`SynthCache`] is the
//! body's, from its field's start to its release: [`SynthCache`]'s rule lets one cache serve views
//! whose held cells only grow, as a body's field does. The body's detail seed is each query's
//! argument, the client's from the body's record (R09.T1.b); the field carries none. No answer
//! depends on the order of queries or on what the cache holds.
//!
//! Nothing here draws from a stream, so the module opens no domain tag; the answers are
//! presentation, under no generator version but R09's and R10's functions'.
//!
//! # The disc's class map (plan R10, T10.d)
//!
//! [`WorkerFields::class_map`] answers R07's disc with the body's coarse class weights over the
//! survey: a texel a cell of [`CLASS_MAP_LEVEL`] (64 × 64 a face), each texel touched by the
//! survey with its weights over the body's classes, at most [`CLASS_MAP_CLASSES`], and the share of
//! it surveyed. The render thread packs it into R07's map (`view/terrain/classMap.ts`).

use std::collections::{BTreeMap, BTreeSet};

use hyperion_base::rng::DetailSeed;
use hyperion_base::units::Metres;

use crate::bake::{TerrainBake, TerrainBakeOptions, UNSURVEYED, bake_terrain};
use crate::cube::{FaceUv, MAX_LEVEL, PATCH_QUADS, PatchKey, face_uv_to_xyz, st_to_uv, unit_dir};
use crate::field::{
    BodyRef, FieldView, InsertBlockError, PartialField, ResolutionCode, cell_at_index, cell_index,
};
use crate::geometry::vertex_spacing;
use crate::material::{
    GroundMaterial, Material, MaterialProperties, PhaseLaw, ResolvePropertiesError, WEIGHT_TOTAL,
    material_at, properties, scaled_normal_albedo, weights_at,
};
use crate::num;
use crate::reading::{LineOfSight, SurveyedReading, slant_range, surveyed_reading};
use crate::synth::{BandLevel, LevelBounds, QueryHeightError, SynthCache, Synthesiser};
use crate::wire::{DecodeBlockError, DecodedBlock, decode_block};

/// The values a level's row of a [level table](WorkerFields::level_table) holds: 4.
pub const LEVEL_TABLE_STRIDE: usize = 4;

/// The rows of a [level table](WorkerFields::level_table): one a band level, 0 to 24.
pub const LEVEL_TABLE_ROWS: usize = MAX_LEVEL as usize + 1;

/// Millimetres a metre, the unit of a cell's elevation.
const MM_PER_M: f64 = 1000.0;

/// The fields a client's height worker holds, one [`PartialField`] a body, with each body's cache
/// and level bounds (the [module documentation](self)).
#[derive(Debug, Clone, Default)]
pub struct WorkerFields {
    bodies: BTreeMap<BodyRef, BodyField>,
}

/// One body's field and what the worker keeps with it.
#[derive(Debug, Clone)]
struct BodyField {
    field: PartialField,
    /// The body's level bounds, from the field's header alone, built once at the field's start.
    bounds: LevelBounds,
    /// The lowest and highest elevation of the cells held, millimetres, `None` before any.
    elevation_mm: Option<(i32, i32)>,
    /// The body's synthesis cache, kept from the field's start to its release. It is correct only
    /// because the field never drops a cell ([`SynthCache`]'s rule): a change that evicts cells
    /// must start it afresh with them.
    cache: SynthCache,
}

impl BodyField {
    /// Widens the elevations held by `block`'s cells.
    fn hold_elevations(&mut self, block: &DecodedBlock) {
        for cell in block.cells() {
            let e = cell.elevation_mm;
            self.elevation_mm = Some(
                self.elevation_mm
                    .map_or((e, e), |(lo, hi)| (lo.min(e), hi.max(e))),
            );
        }
    }
}

/// What a block changed of its body's field: the cells it surveyed anew, and those it brought.
///
/// Both are runs of cell indices at the field's level ([`cell_index`]): the cells the block
/// surveyed that the field did not hold as surveyed, where a baked patch's survey mask can change,
/// and the cells it brought, surveyed or margin, that the field did not hold at all, where a horizon
/// map's refused far field can change (the [module documentation](self)'s rebake rule). Both are
/// empty for a block sent again; a margin, or a finer survey of cells held, surveys nothing new,
/// though a margin may bring cells.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct SurveyChange {
    runs: Vec<(u32, u32)>,
    held: Vec<(u32, u32)>,
}

impl SurveyChange {
    /// The runs of the cells surveyed anew, each its first cell index and the index past its last,
    /// sorted, disjoint and with no two adjacent.
    #[must_use]
    pub fn runs(&self) -> &[(u32, u32)] {
        &self.runs
    }

    /// The runs of the cells the field did not hold before the block, surveyed or margin.
    ///
    /// They are laid out as [`runs`](Self::runs) are. Every cell surveyed anew that the field did
    /// not hold as margin is among them.
    #[must_use]
    pub fn held_runs(&self) -> &[(u32, u32)] {
        &self.held
    }

    /// Whether the block surveyed no cell anew; it may still have brought cells as margin
    /// ([`held_runs`](Self::held_runs)), which the rebake rule's third case reads.
    #[must_use]
    pub fn surveys_nothing(&self) -> bool {
        self.runs.is_empty()
    }

    /// Whether the block surveyed the cell of index `index` anew.
    #[must_use]
    pub fn surveys(&self, index: u32) -> bool {
        holds(&self.runs, index)
    }

    /// Whether the block surveyed anew any of `indices`.
    #[must_use]
    pub fn surveys_any(&self, indices: &[u32]) -> bool {
        indices.iter().any(|&index| self.surveys(index))
    }

    /// Whether the block brought the cell of index `index`, surveyed or margin, which the field did
    /// not hold before it.
    #[must_use]
    pub fn brings(&self, index: u32) -> bool {
        holds(&self.held, index)
    }

    /// Whether the block brought any of `indices` that the field did not hold before it.
    #[must_use]
    pub fn brings_any(&self, indices: &[u32]) -> bool {
        indices.iter().any(|&index| self.brings(index))
    }
}

/// Whether `runs`, sorted and disjoint, hold `index`.
#[must_use]
fn holds(runs: &[(u32, u32)], index: u32) -> bool {
    let after = runs.partition_point(|&(start, _)| start <= index);
    after > 0 && index < runs[after - 1].1
}

/// Adds the cell of index `index`, the next in rising order, to `runs`.
fn push_run(runs: &mut Vec<(u32, u32)>, index: u32) {
    match runs.last_mut() {
        Some((_, end)) if *end == index => *end = index + 1,
        _ => runs.push((index, index + 1)),
    }
}

/// Why a worker refused a block. The body's field is then as it was before it.
#[derive(Debug, Clone, PartialEq)]
pub enum TakeBlockError {
    /// The block's bytes are not a block this build reads.
    Decode(DecodeBlockError),
    /// A field was started from a block that is not a block 0, which alone carries the header.
    NoHeader {
        /// The block's index in its payload.
        index: u32,
    },
    /// A later block of a body whose field was not started.
    NotStarted(BodyRef),
    /// The block is of another body than the one it was posted for.
    WrongBody {
        /// The body it was posted for.
        named: BodyRef,
        /// The block's own.
        found: BodyRef,
    },
    /// The field refused to merge it.
    Insert(InsertBlockError),
}

impl std::fmt::Display for TakeBlockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Decode(e) => write!(f, "the block does not decode: {e}"),
            Self::NoHeader { index } => write!(
                f,
                "a field starts from a block 0, which carries its header, not from block {index}"
            ),
            Self::NotStarted(body) => write!(
                f,
                "a block of body {} of system {:#x} arrived before its field was started",
                body.body_index(),
                body.raw_system_id()
            ),
            Self::WrongBody { named, found } => write!(
                f,
                "a block of body {} of system {:#x} was posted for body {} of system {:#x}",
                found.body_index(),
                found.raw_system_id(),
                named.body_index(),
                named.raw_system_id()
            ),
            Self::Insert(e) => write!(f, "the field refused the block: {e}"),
        }
    }
}

impl std::error::Error for TakeBlockError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Decode(e) => Some(e),
            Self::Insert(e) => Some(e),
            Self::NoHeader { .. } | Self::NotStarted(_) | Self::WrongBody { .. } => None,
        }
    }
}

/// Why a worker could not answer a query of a body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QueryBodyError {
    /// No field of the body is held.
    NotStarted(BodyRef),
    /// The query reads a cell the body's field does not hold
    /// ([`QueryHeightError::NotSurveyed`]): one a later block may bring.
    NotSurveyed(PatchKey),
}

impl std::fmt::Display for QueryBodyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotStarted(body) => write!(
                f,
                "no field of body {} of system {:#x} is held",
                body.body_index(),
                body.raw_system_id()
            ),
            Self::NotSurveyed(cell) => write!(f, "{}", QueryHeightError::NotSurveyed(*cell)),
        }
    }
}

impl std::error::Error for QueryBodyError {}

impl From<QueryHeightError> for QueryBodyError {
    fn from(e: QueryHeightError) -> Self {
        match e {
            QueryHeightError::NotSurveyed(cell) => Self::NotSurveyed(cell),
        }
    }
}

/// A material's photometry as the lit terrain draws it (plan R10, T10.b; Design note 8): its normal
/// albedo in B, V and R under the body's albedo scale ([`scaled_normal_albedo`]), its law, and the
/// law's L(0) at the scaled V ([`PhaseLaw::limb_parameter`]), as T10.a's solve assumed them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrawnPhotometry {
    normal_albedo_bvr: [f64; 3],
    phase_law: PhaseLaw,
    limb_parameter: f64,
}

impl DrawnPhotometry {
    /// The normal albedo in the Johnson B, V and R bands under the field header's albedo scale c,
    /// dimensionless: c `A_N`, its V clamped to the material's
    /// [`scale_range`](crate::material::scale_range) and its B and R taking V's factor, each from 0
    /// to its law's ceiling ([`PhaseLaw::normal_albedo_ceiling`]: 1.35 for the particulate laws, 1
    /// for Lambert's).
    #[must_use]
    pub const fn normal_albedo_bvr(&self) -> [f64; 3] {
        self.normal_albedo_bvr
    }

    /// The photometric law: its row's f(α) and L(α) (R07's phase table, R10.T10.b).
    #[must_use]
    pub const fn phase_law(&self) -> PhaseLaw {
        self.phase_law
    }

    /// L(0), the law's Lommel–Seeliger share at opposition at the scaled albedo in V, dimensionless:
    /// 0 for Lambert's law, 0.38 to 1 for the particulate laws ([`limb_parameter`](
    /// crate::material::limb_parameter)), NaN for a NaN albedo.
    #[must_use]
    pub const fn limb_parameter(&self) -> f64 {
        self.limb_parameter
    }
}

/// Why a worker could not give a material's photometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResolvePhotometryError {
    /// No field of the body is held.
    NotStarted(BodyRef),
    /// The material's entry is past the body's palette.
    Properties(ResolvePropertiesError),
}

impl std::fmt::Display for ResolvePhotometryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotStarted(body) => write!(
                f,
                "no field of body {} of system {:#x} is held",
                body.body_index(),
                body.raw_system_id()
            ),
            Self::Properties(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ResolvePhotometryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NotStarted(_) => None,
            Self::Properties(e) => Some(e),
        }
    }
}

/// A patch baked over a body's field, with the cells its survey mask holds unsurveyed.
#[derive(Debug, Clone, PartialEq)]
pub struct BodyBake {
    terrain: TerrainBake,
    unsurveyed_cells: Vec<u32>,
}

impl BodyBake {
    /// R10.T6.a's bake: R05's arrays and R10's layers.
    #[must_use]
    pub const fn terrain(&self) -> &TerrainBake {
        &self.terrain
    }

    /// The [`cell_index`] of each cell of the field's level that holds a vertex whose survey mask
    /// is [`UNSURVEYED`], rising, each once: the cells whose survey would change the mask (the
    /// [module documentation](self)'s rebake rule). Empty for a patch surveyed whole.
    #[must_use]
    pub fn unsurveyed_cells(&self) -> &[u32] {
        &self.unsurveyed_cells
    }

    /// Whether a block that made `change` makes this bake stale, so that the patch is baked again.
    ///
    /// It is the [module documentation](self)'s rebake rule: the bake is not complete, the block
    /// surveys a cell its mask holds unsurveyed, or it brings a cell its horizon map's far field
    /// was refused for. Where it is `false`, the patch bakes the same bytes after the block, its
    /// horizon map's words and refused cells included.
    #[must_use]
    pub fn needs_rebake(&self, change: &SurveyChange) -> bool {
        let layers = self.terrain.layers();
        !layers.is_complete()
            || change.surveys_any(&self.unsurveyed_cells)
            || change.brings_any(layers.horizon().refused_cells())
    }

    /// The two parts, to hand on without a copy.
    #[must_use]
    pub fn into_parts(self) -> (TerrainBake, Vec<u32>) {
        (self.terrain, self.unsurveyed_cells)
    }
}

/// The quadtree level of the disc's class map (plan R10, T10.d): 64 × 64 texels a face, each the
/// cell (face, 6, i, j) of R05's cube sphere, as R07's `classMapTexelOf` finds it.
///
/// A texel of band level n is a cell of quadtree level n + 6 ([`weights_at`]), so a texel here takes
/// its weights at band level 0 ([`CLASS_MAP_BAND`]): inside Design note 8's "level 4 or shallower"
/// (main's provisional ruling on R10.T2's question, a map at quadtree level L reading band level
/// L − 6, so L at most 10), and of Design note 8's "a few hundred kilobytes": 66 × 66 texels a
/// layer with R07's gutter, 24 layers at 16 classes, 418 KB, where a map at L = 10 would take about
/// 100 MB.
pub const CLASS_MAP_LEVEL: u8 = 6;

/// The band level a class-map texel's weights are taken at: [`CLASS_MAP_LEVEL`] − 6.
pub const CLASS_MAP_BAND: u8 = CLASS_MAP_LEVEL - 6;

/// The classes a class map holds at most: R07's `MAX_DISC_CLASSES`, the laws its disc carries.
pub const CLASS_MAP_CLASSES: usize = 16;

/// One class of a [`ClassMap`]: a form of a palette entry, with the photometry it is drawn with.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClassMapClass {
    material: GroundMaterial,
    photometry: Option<DrawnPhotometry>,
}

impl ClassMapClass {
    /// The form and, where the palette names one, the substance.
    #[must_use]
    pub const fn material(&self) -> GroundMaterial {
        self.material
    }

    /// Its photometry ([`WorkerFields::photometry`]), `None` where the palette names no substance:
    /// the disc then shades it with the body's uniform law, as the lit terrain does its law 0.
    #[must_use]
    pub const fn photometry(&self) -> Option<DrawnPhotometry> {
        self.photometry
    }
}

/// One texel of a [`ClassMap`] that the survey touches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClassMapTexel {
    cell: PatchKey,
    surveyed: u32,
    cells: u32,
    weights: [u8; CLASS_MAP_CLASSES],
}

impl ClassMapTexel {
    /// The texel: a cell of [`CLASS_MAP_LEVEL`].
    #[must_use]
    pub const fn cell(&self) -> PatchKey {
        self.cell
    }

    /// The field's cells under the texel that are surveyed, at least 1.
    #[must_use]
    pub const fn surveyed_cells(&self) -> u32 {
        self.surveyed
    }

    /// The field's cells under the texel: 4^(field level − 6), or 1 where the field's level is 6 or
    /// shallower, its one cell holding the texel.
    #[must_use]
    pub const fn cells(&self) -> u32 {
        self.cells
    }

    /// The share of the texel the survey covers, (0, 1]: its field cells surveyed over its cells,
    /// a margin cell counting as unsurveyed; 1 where the field's level is 6 or shallower.
    #[must_use]
    pub fn surveyed_fraction(&self) -> f64 {
        f64::from(self.surveyed) / f64::from(self.cells)
    }

    /// Its weight bytes over the map's classes, in their order, the rest 0: summing to 255
    /// ([`WEIGHT_TOTAL`]), or less where a class past [`CLASS_MAP_CLASSES`] was left out.
    #[must_use]
    pub const fn weights(&self) -> [u8; CLASS_MAP_CLASSES] {
        self.weights
    }
}

/// A body's coarse class map for R07's disc (plan R10, T10.d; Design note 8), from its field.
///
/// Each texel is a cell of [`CLASS_MAP_LEVEL`], and the map holds those the survey touches, in
/// rising [`cell_index`], with:
///
/// - the share of it surveyed ([`ClassMapTexel::surveyed_fraction`]): of the field's cells under
///   it, those surveyed, a margin cell counting as unsurveyed;
/// - its weights over the map's classes: [`weights_at`] at the texel's centre at band level
///   [`CLASS_MAP_BAND`] where the field's cell holding the centre is surveyed, and otherwise the
///   mean of [`weights_at`] at the centres of the field's surveyed cells under it, at that band
///   level, as bytes summing to 255 by the largest remainder (ties to the earlier class).
///
/// The classes are the materials the texels' weights name, in [`GroundMaterial`]'s order: a form
/// of a palette entry, or a form whose palette names no substance. Past [`CLASS_MAP_CLASSES`] the
/// lightest, by their weight summed over the texels times each texel's surveyed share (ties to the
/// later material), are left out, and their weight with them, which the disc gives the body's
/// uniform law.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassMap {
    field_level: u8,
    classes: Vec<ClassMapClass>,
    texels: Vec<ClassMapTexel>,
    complete: bool,
}

impl ClassMap {
    /// The level of the field it was made from, 5 to 8.
    #[must_use]
    pub const fn field_level(&self) -> u8 {
        self.field_level
    }

    /// Its classes, at most [`CLASS_MAP_CLASSES`], in the order the texels' weights take them.
    #[must_use]
    pub fn classes(&self) -> &[ClassMapClass] {
        &self.classes
    }

    /// The texels the survey touches, in rising [`cell_index`].
    #[must_use]
    pub fn texels(&self) -> &[ClassMapTexel] {
        &self.texels
    }

    /// Whether every texel the survey touches was answered: `false` where a texel's weights read a
    /// cell the field does not hold yet (a survey's margin in a later block), which leaves the
    /// texel out until a later block.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.complete
    }
}

/// A texel's surveyed cells and its weight bytes, per material, as means: before the map's
/// classes are chosen.
struct TexelReading {
    cell: PatchKey,
    surveyed: u32,
    cells: u32,
    weights: BTreeMap<GroundMaterial, f64>,
}

/// A class map's classes from its texels' readings ([`ClassMap`]): every material they name, in
/// [`GroundMaterial`]'s order, or past [`CLASS_MAP_CLASSES`] the heaviest, by weight summed over
/// the texels times each one's surveyed share, ties to the earlier material.
#[must_use]
fn kept_classes(readings: &[TexelReading]) -> Vec<GroundMaterial> {
    let mut totals = BTreeMap::<GroundMaterial, f64>::new();
    for reading in readings {
        let share = f64::from(reading.surveyed) / f64::from(reading.cells);
        for (&material, &weight) in &reading.weights {
            *totals.entry(material).or_insert(0.0) += weight * share;
        }
    }
    let mut ranked: Vec<(GroundMaterial, f64)> = totals.into_iter().collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    ranked.truncate(CLASS_MAP_CLASSES);
    let mut kept: Vec<GroundMaterial> = ranked.into_iter().map(|(m, _)| m).collect();
    kept.sort_unstable();
    kept
}

/// The field's cells of `level` under, or holding, the class-map texel `texel`, into `cells`, which
/// it clears first: 4^(`level` − 6) of them in rising (j, i), or the one holding the texel where
/// `level` is 6 or shallower.
fn field_cells_of(texel: PatchKey, level: u8, cells: &mut Vec<PatchKey>) {
    cells.clear();
    if level <= CLASS_MAP_LEVEL {
        let shift = CLASS_MAP_LEVEL - level;
        cells.push(
            PatchKey::new(texel.face(), level, texel.i() >> shift, texel.j() >> shift)
                .expect("a texel's ancestor is a cell of its level"),
        );
    } else {
        let shift = level - CLASS_MAP_LEVEL;
        let side = 1_u32 << shift;
        cells.extend(
            (0..side)
                .flat_map(|dj| (0..side).map(move |di| (di, dj)))
                .map(|(di, dj)| {
                    PatchKey::new(
                        texel.face(),
                        level,
                        (texel.i() << shift) + di,
                        (texel.j() << shift) + dj,
                    )
                    .expect("a texel's descendants are cells of their level")
                }),
        );
    }
}

/// The centre of `cell` in face coordinates.
#[must_use]
fn centre_of(cell: PatchKey) -> FaceUv {
    let side = f64::from(1_u32 << cell.level());
    FaceUv {
        face: cell.face(),
        u: st_to_uv((f64::from(cell.i()) + 0.5) / side),
        v: st_to_uv((f64::from(cell.j()) + 0.5) / side),
    }
}

/// `values`, at most [`CLASS_MAP_CLASSES`], as bytes in their order keeping their rounded sum, at
/// most 255, the bytes past them 0: each rounded down, and the bytes that leaves short given to the
/// largest remainders, ties to the earlier.
///
/// # Panics
///
/// If `values` holds more than [`CLASS_MAP_CLASSES`], which a map's classes never do (a bug).
#[must_use]
fn bytes_by_largest_remainder(values: &[f64]) -> [u8; CLASS_MAP_CLASSES] {
    let n = values.len();
    assert!(
        n <= CLASS_MAP_CLASSES,
        "{n} values for at most {CLASS_MAP_CLASSES} classes"
    );
    let total: f64 = values.iter().sum();
    let mut bytes = [0_u8; CLASS_MAP_CLASSES];
    let mut remainders = [0.0_f64; CLASS_MAP_CLASSES];
    for ((byte, remainder), &value) in bytes.iter_mut().zip(&mut remainders).zip(values) {
        let floor = value.floor();
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "each value lies in [0, 255], so its floor is a whole number a u8 holds"
        )]
        let whole = floor as u8;
        *byte = whole;
        *remainder = value - floor;
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the values' sum lies in [0, 255], so its rounding is a whole number a u32 holds"
    )]
    let target = num::min(total.round(), f64::from(WEIGHT_TOTAL)) as u32;
    let mut order: [usize; CLASS_MAP_CLASSES] = std::array::from_fn(|k| k);
    // The index breaks every tie, so no two compare equal and the unstable sort is the stable one.
    order[..n].sort_unstable_by(|&a, &b| remainders[b].total_cmp(&remainders[a]).then(a.cmp(&b)));
    let held: u32 = bytes.iter().map(|&b| u32::from(b)).sum();
    let mut short = target.saturating_sub(held);
    for &k in &order[..n] {
        if short == 0 {
            break;
        }
        bytes[k] = bytes[k].saturating_add(1);
        short -= 1;
    }
    bytes
}

/// The material at a surveyed point and, where the palette names its substance, its properties.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurveyedMaterial {
    ground: GroundMaterial,
    properties: Option<MaterialProperties>,
}

impl SurveyedMaterial {
    /// The classifier's answer: the form always, and the entry where the palette names one.
    #[must_use]
    pub const fn ground(&self) -> GroundMaterial {
        self.ground
    }

    /// The material's properties, `None` where the palette names no entry
    /// ([`GroundMaterial::material`] is `None`).
    #[must_use]
    pub const fn properties(&self) -> Option<&MaterialProperties> {
        self.properties.as_ref()
    }
}

impl WorkerFields {
    /// No field held.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            bodies: BTreeMap::new(),
        }
    }

    /// The bodies whose fields are held, in [`BodyRef`]'s order.
    pub fn bodies(&self) -> impl Iterator<Item = BodyRef> + '_ {
        self.bodies.keys().copied()
    }

    /// The field of `body`, if one is held.
    #[must_use]
    pub fn field(&self, body: BodyRef) -> Option<&PartialField> {
        self.bodies.get(&body).map(|b| &b.field)
    }

    /// The bytes the fields hold on the heap ([`PartialField::heap_bytes`] summed): the worker's
    /// copy of the bodies' fields, for R12's memory line. The caches and bounds are not counted.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        self.bodies
            .values()
            .map(|b| b.field.heap_bytes())
            .fold(0, usize::saturating_add)
    }

    /// Starts `body`'s field from `bytes`, one block 0, replacing any field of `body` held, and
    /// answers with the cells it surveys and brings ([`SurveyChange`]).
    ///
    /// It builds the body's level bounds from the header ([`LevelBounds::new`], a few to some
    /// tens of milliseconds) and starts its cache afresh.
    ///
    /// # Errors
    ///
    /// [`TakeBlockError`] if the bytes do not decode, are not a block 0, are of another body than
    /// `body`, or are refused by the new field. Any field of `body` held is then kept as it was.
    pub fn init(&mut self, body: BodyRef, bytes: &[u8]) -> Result<SurveyChange, TakeBlockError> {
        let block = decode_block(bytes).map_err(TakeBlockError::Decode)?;
        check_body(body, &block)?;
        let header = block.header().ok_or(TakeBlockError::NoHeader {
            index: block.index(),
        })?;
        let mut field = PartialField::new(header.clone());
        let change = survey_change(&field, &block);
        field.insert(&block).map_err(TakeBlockError::Insert)?;
        let mut held = BodyField {
            bounds: LevelBounds::new(field.header()),
            field,
            elevation_mm: None,
            cache: SynthCache::new(),
        };
        held.hold_elevations(&block);
        self.bodies.insert(body, held);
        Ok(change)
    }

    /// Adds `bytes`, one block, to `body`'s field, and answers with the cells it surveys anew and
    /// brings ([`SurveyChange`]).
    ///
    /// # Errors
    ///
    /// [`TakeBlockError`] if no field of `body` is held, or the bytes do not decode, are of another
    /// body, or are refused by the field ([`PartialField::insert`]). The field is then as it was.
    pub fn chunk(&mut self, body: BodyRef, bytes: &[u8]) -> Result<SurveyChange, TakeBlockError> {
        let held = self
            .bodies
            .get_mut(&body)
            .ok_or(TakeBlockError::NotStarted(body))?;
        let block = decode_block(bytes).map_err(TakeBlockError::Decode)?;
        check_body(body, &block)?;
        // The insert refuses it too, but the change is read first, by the field's level.
        let expected = held.field.header().level();
        if block.level() != expected {
            return Err(TakeBlockError::Insert(InsertBlockError::WrongLevel {
                found: block.level(),
                expected,
            }));
        }
        let change = survey_change(&held.field, &block);
        held.field.insert(&block).map_err(TakeBlockError::Insert)?;
        held.hold_elevations(&block);
        Ok(change)
    }

    /// Drops `body`'s field, its cache and bounds; whether one was held.
    pub fn release(&mut self, body: BodyRef) -> bool {
        self.bodies.remove(&body).is_some()
    }

    /// `body`'s level table, as R05's selection and culling read the test planet's
    /// (`TestPlanet::level_table`): [`LEVEL_TABLE_ROWS`] rows of [`LEVEL_TABLE_STRIDE`], band level
    /// n at `4 n`, each
    ///
    /// - the level bound `ε_n`, metres ([`LevelBounds::bound_m`], R09's [`level_bound_m`]);
    /// - the lowest and highest height a patch of the level can bake, metres above the datum: the
    ///   elevations of the cells held, which R09's approximating spline never leaves, widened by a
    ///   micrometre and a part in 10¹² as R09's patch range is, plus the most the level's and its
    ///   parent's contributions can add ([`LevelBounds::contribution_range_m`]), and stretched to
    ///   the datum, 0, which a vertex outside the survey takes where the field cannot make it
    ///   (R10.T6.a); exactly 0 and 0 while no cell is held;
    /// - the largest vertex spacing of the level on the body's equatorial radius, metres
    ///   ([`vertex_spacing`]).
    ///
    /// It grows as blocks bring cells, so a worker answers it after every block. `None` if no
    /// field of `body` is held.
    ///
    /// [`level_bound_m`]: crate::synth::level_bound_m
    #[must_use]
    pub fn level_table(&self, body: BodyRef) -> Option<Vec<f64>> {
        let held = self.bodies.get(&body)?;
        let a = held.field.header().figure().equatorial_radius_m;
        let base = held.elevation_mm.map(|(lo, hi)| {
            let (lo, hi) = (f64::from(lo) / MM_PER_M, f64::from(hi) / MM_PER_M);
            let slack = |x: f64| 1e-6 + 1e-12 * x.abs();
            (lo - slack(lo), hi + slack(hi))
        });
        let mut table = Vec::with_capacity(LEVEL_TABLE_ROWS * LEVEL_TABLE_STRIDE);
        for level in 0..=MAX_LEVEL {
            let n = band(level);
            let (low, high) = base.map_or((0.0, 0.0), |(base_low, base_high)| {
                let (mut low, mut high) = held.bounds.contribution_range_m(n);
                if let Some(parent) = level.checked_sub(1) {
                    let (l, h) = held.bounds.contribution_range_m(band(parent));
                    low = num::min(low, l);
                    high = num::max(high, h);
                }
                (
                    num::min(base_low + low, 0.0),
                    num::max(base_high + high, 0.0),
                )
            });
            table.extend([
                held.bounds.bound_m(n),
                low,
                high,
                vertex_spacing(a, level).max_m,
            ]);
        }
        Some(table)
    }

    /// Bakes `key` over `body`'s field with the body's detail seed `seed` ([`bake_terrain`], no
    /// hooks), with the cells its survey mask holds unsurveyed.
    ///
    /// # Errors
    ///
    /// [`QueryBodyError::NotStarted`] if no field of `body` is held, and
    /// [`QueryBodyError::NotSurveyed`] where a surveyed vertex reads a cell the field does not hold
    /// yet: a survey whose margin is still to come in a later block.
    ///
    /// # Panics
    ///
    /// As [`bake_terrain`]: if `opts`' skirt margin is negative or not finite.
    pub fn bake(
        &mut self,
        body: BodyRef,
        seed: DetailSeed,
        key: PatchKey,
        opts: &TerrainBakeOptions,
    ) -> Result<BodyBake, QueryBodyError> {
        let BodyField { field, cache, .. } = self.held(body)?;
        let synth = Synthesiser::new(&*field, seed);
        let terrain = bake_terrain(&synth, cache, key, opts, &[])?;
        let coarse = field.header().level().get();
        let quads = u8::try_from(PATCH_QUADS).expect("64 quads fit a u8");
        let mask = terrain.layers().survey_mask();
        let cells: BTreeSet<u32> = (0..=quads)
            .flat_map(|y| (0..=quads).map(move |x| (x, y)))
            .zip(mask)
            .filter(|&(_, &m)| m == UNSURVEYED)
            .map(|((x, y), _)| {
                cell_index(
                    PatchKey::containing(coarse, key.vertex_dir(x, y))
                        .expect("a coarse level is a quadtree level"),
                )
            })
            .collect();
        Ok(BodyBake {
            terrain,
            unsurveyed_cells: cells.into_iter().collect(),
        })
    }

    /// The reading at `at` on `body`, its slope over `slope_baseline` ([`surveyed_reading`]), or
    /// `None` where the ground there has not been surveyed.
    ///
    /// # Errors
    ///
    /// As [`bake`](Self::bake): no field of `body`, or a height the slope reads past the cells
    /// held.
    ///
    /// # Panics
    ///
    /// As [`surveyed_reading`]: if `slope_baseline` is not finite and positive or `at` names no
    /// direction.
    pub fn reading(
        &mut self,
        body: BodyRef,
        seed: DetailSeed,
        at: FaceUv,
        slope_baseline: Metres,
    ) -> Result<Option<SurveyedReading>, QueryBodyError> {
        let BodyField { field, cache, .. } = self.held(body)?;
        let synth = Synthesiser::new(&*field, seed);
        Ok(surveyed_reading(&synth, cache, at, slope_baseline)?)
    }

    /// Where the line of sight from `origin_m` along `direction`, body-fixed, metres, meets
    /// `body`'s ground within `max` metres, as far as the survey can say ([`slant_range`]).
    ///
    /// # Errors
    ///
    /// As [`bake`](Self::bake): no field of `body`, or a height on surveyed ground past the cells
    /// held.
    ///
    /// # Panics
    ///
    /// As [`slant_range`]: if `origin_m` is not finite, `direction` is zero or not finite, or `max`
    /// is not finite and non-negative.
    pub fn slant_range(
        &mut self,
        body: BodyRef,
        seed: DetailSeed,
        origin_m: &[f64; 3],
        direction: &[f64; 3],
        max: Metres,
    ) -> Result<LineOfSight, QueryBodyError> {
        let BodyField { field, cache, .. } = self.held(body)?;
        let synth = Synthesiser::new(&*field, seed);
        Ok(slant_range(&synth, cache, origin_m, direction, max)?)
    }

    /// The material at `at` on `body` ([`material_at`]) with its [`properties`], or `None` where
    /// the field's cell there is not surveyed: a survey's margin, whose ground is known but which
    /// the ship has not measured, or a cell not held (the readout's `NOT SURVEYED`, as
    /// [`reading`](Self::reading)'s).
    ///
    /// # Errors
    ///
    /// As [`bake`](Self::bake): no field of `body`, or a read past the cells held.
    ///
    /// # Panics
    ///
    /// As [`material_at`]: if `at` names no direction. Also if the classifier names an entry past
    /// the body's palette, which its tests rule out (a bug).
    pub fn material(
        &mut self,
        body: BodyRef,
        seed: DetailSeed,
        at: FaceUv,
    ) -> Result<Option<SurveyedMaterial>, QueryBodyError> {
        let BodyField { field, cache, .. } = self.held(body)?;
        let dir = unit_dir(face_uv_to_xyz(at));
        let level = field.header().level().get();
        let cell = PatchKey::containing(level, dir).expect("a coarse level is a quadtree level");
        if !field.is_surveyed(cell) {
            return Ok(None);
        }
        let synth = Synthesiser::new(&*field, seed);
        let ground = material_at(&synth, cache, at)?;
        let properties = ground.material().map(|m| {
            properties(m, field.header().palette())
                .expect("the classifier names only entries of the body's palette")
        });
        Ok(Some(SurveyedMaterial { ground, properties }))
    }

    /// The photometry `material` is drawn with on `body` ([`DrawnPhotometry`]).
    ///
    /// Its properties over the body's palette ([`properties`]), its albedo under the field header's
    /// scale (R10.T10.a's c, 1 where the header holds none: the palette's albedos as resolved, as
    /// [`FieldHeaderParts::albedo_scale`](crate::field::FieldHeaderParts::albedo_scale)'s contract
    /// has it), and its law's L(0) there. It reads the header alone, so it answers wherever the
    /// body's field is held, surveyed or not.
    ///
    /// # Errors
    ///
    /// [`ResolvePhotometryError::NotStarted`] where no field of `body` is held, and
    /// [`ResolvePhotometryError::Properties`] for an entry past its palette.
    pub fn photometry(
        &self,
        body: BodyRef,
        material: Material,
    ) -> Result<DrawnPhotometry, ResolvePhotometryError> {
        let header = self
            .bodies
            .get(&body)
            .ok_or(ResolvePhotometryError::NotStarted(body))?
            .field
            .header();
        let resolved =
            properties(material, header.palette()).map_err(ResolvePhotometryError::Properties)?;
        let normal_albedo_bvr =
            scaled_normal_albedo(&resolved, header.albedo_scale().unwrap_or(1.0));
        let phase_law = resolved.phase_law();
        Ok(DrawnPhotometry {
            normal_albedo_bvr,
            phase_law,
            limb_parameter: phase_law.limb_parameter(normal_albedo_bvr[1]),
        })
    }

    /// `body`'s coarse class map for R07's disc ([`ClassMap`]), with the body's detail seed `seed`.
    ///
    /// Every cell of [`CLASS_MAP_LEVEL`] is a texel, and those under which the field holds a
    /// surveyed cell are answered: 24,576 texels, each surveyed one at one [`weights_at`] or, where
    /// its centre's field cell is not surveyed, one a surveyed field cell under it. A texel whose
    /// weights read a cell the field does not hold yet is left out, and the map is then not
    /// [complete](ClassMap::is_complete).
    ///
    /// # Errors
    ///
    /// [`QueryBodyError::NotStarted`] if no field of `body` is held.
    ///
    /// # Panics
    ///
    /// If the classifier names an entry past the body's palette, which its tests rule out (a bug).
    pub fn class_map(
        &mut self,
        body: BodyRef,
        seed: DetailSeed,
    ) -> Result<ClassMap, QueryBodyError> {
        let BodyField { field, cache, .. } = self.held(body)?;
        let level = field.header().level().get();
        let band = BandLevel::new(CLASS_MAP_BAND).expect("band level 0 is a band level");
        let synth = Synthesiser::new(&*field, seed);
        let texel_count = 6_u32 << (2 * u32::from(CLASS_MAP_LEVEL));
        let mut readings = Vec::new();
        let mut complete = true;
        // A texel's field cells, those surveyed and the points read, at most 16 each (a field of
        // level 8), kept from texel to texel.
        let mut cells = Vec::with_capacity(16);
        let mut surveyed = Vec::with_capacity(16);
        let mut points = Vec::with_capacity(16);
        let count = |n: usize| u32::try_from(n).expect("a texel holds at most 16 field cells");
        for index in 0..texel_count {
            let texel =
                cell_at_index(CLASS_MAP_LEVEL, index).expect("an index below 6 · 4⁶ is a texel");
            field_cells_of(texel, level, &mut cells);
            surveyed.clear();
            surveyed.extend(
                cells
                    .iter()
                    .copied()
                    .filter(|&cell| field.is_surveyed(cell)),
            );
            if surveyed.is_empty() {
                continue;
            }
            let centre = centre_of(texel);
            let centre_cell = PatchKey::containing(level, unit_dir(face_uv_to_xyz(centre)))
                .expect("a coarse level is a quadtree level");
            points.clear();
            if field.is_surveyed(centre_cell) {
                points.push(centre);
            } else {
                points.extend(surveyed.iter().map(|&cell| centre_of(cell)));
            }
            let mut weights = BTreeMap::<GroundMaterial, f64>::new();
            let mut answered = true;
            for &at in &points {
                match weights_at(&synth, cache, at, band) {
                    Ok(at_point) => {
                        for (material, byte) in at_point.iter() {
                            *weights.entry(material).or_insert(0.0) += f64::from(byte);
                        }
                    }
                    Err(QueryHeightError::NotSurveyed(_)) => {
                        // A read past the cells held: the texel waits for a later block.
                        answered = false;
                        break;
                    }
                }
            }
            if !answered {
                complete = false;
                continue;
            }
            for value in weights.values_mut() {
                *value /= f64::from(count(points.len()));
            }
            readings.push(TexelReading {
                cell: texel,
                surveyed: count(surveyed.len()),
                cells: count(cells.len()),
                weights,
            });
        }
        let kept = kept_classes(&readings);
        let texels = readings
            .iter()
            .map(|reading| {
                let mut values = [0.0_f64; CLASS_MAP_CLASSES];
                for (value, material) in values.iter_mut().zip(&kept) {
                    *value = reading.weights.get(material).copied().unwrap_or(0.0);
                }
                ClassMapTexel {
                    cell: reading.cell,
                    surveyed: reading.surveyed,
                    cells: reading.cells,
                    weights: bytes_by_largest_remainder(&values[..kept.len()]),
                }
            })
            .collect();
        let classes = kept
            .into_iter()
            .map(|material| ClassMapClass {
                material,
                photometry: material.material().map(|m| {
                    self.photometry(body, m)
                        .expect("the classifier names only entries of the body's palette")
                }),
            })
            .collect();
        Ok(ClassMap {
            field_level: level,
            classes,
            texels,
            complete,
        })
    }

    /// The body's field and what is kept with it.
    fn held(&mut self, body: BodyRef) -> Result<&mut BodyField, QueryBodyError> {
        self.bodies
            .get_mut(&body)
            .ok_or(QueryBodyError::NotStarted(body))
    }
}

/// Refuses a block of another body than `named`.
fn check_body(named: BodyRef, block: &DecodedBlock) -> Result<(), TakeBlockError> {
    if block.body() == named {
        Ok(())
    } else {
        Err(TakeBlockError::WrongBody {
            named,
            found: block.body(),
        })
    }
}

/// The cells `block` surveys that `field` does not hold as surveyed, and the cells it carries,
/// surveyed or margin, that `field` does not hold. A cell `field` already holds keeps the finer of
/// its codes and a cell it does not takes the block's, so a surveyed code in the block makes the
/// cell surveyed whatever was held ([`PartialField::insert`]).
///
/// # Panics
///
/// If `block` is at another level than `field`, whose cover could index past the field's cells:
/// the callers check the level first.
#[must_use]
fn survey_change(field: &PartialField, block: &DecodedBlock) -> SurveyChange {
    let level = field.header().level();
    let mut change = SurveyChange::default();
    for range in block.cover().ranges() {
        let surveyed = range.code().is_surveyed();
        for index in range.start()..range.end() {
            let cell = cell_at_index(level.get(), index).expect("a cover's cells are the level's");
            let held = field.resolution(cell);
            if held.is_none() {
                push_run(&mut change.held, index);
            }
            if surveyed && !held.is_some_and(ResolutionCode::is_surveyed) {
                push_run(&mut change.runs, index);
            }
        }
    }
    change
}

/// R05's `u8` level as R09's band level.
#[must_use]
fn band(level: u8) -> BandLevel {
    BandLevel::new(level).expect("a level up to MAX_LEVEL is a band level")
}

#[cfg(test)]
mod golden;
#[cfg(test)]
mod tests;
