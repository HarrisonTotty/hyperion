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
//! Each block taken answers with what it changed of the survey ([`SurveyChange`]): the cells it
//! surveyed that the field did not hold as surveyed. A block can change a baked patch only there,
//! or where the patch was not complete, which is the rebake rule below.
//!
//! # The rebake rule (Design note 16, as built)
//!
//! Cells arrive exact and whole with their margins, so a block changes no value at a vertex the
//! field held before it, surveyed or answered from a margin, and a resolution code alone changes no
//! byte (R10.T6.a). A patch baked before a block is baked again after it exactly where
//!
//! - the block surveys a cell that holds one of the patch's vertices outside the survey, so that
//!   its survey mask changes: the change meets the bake's [`BodyBake::unsurveyed_cells`]; or
//! - the bake was not [complete](crate::bake::TerrainLayers::is_complete): some vertex took the
//!   reference surface, or has no weights, because the field lacked a cell its reads reach, which
//!   any block may bring, whether or not it changes the mask (main's ruling of 2026-10-14 on
//!   R10.T6.a's question).
//!
//! The client's `workers/rebake.ts` (`needsRebake`) applies the rule to the bakes it holds, from
//! each bake's [`unsurveyed_cells`](BodyBake::unsurveyed_cells) and completeness and each block's
//! [`SurveyChange`].
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

use std::collections::{BTreeMap, BTreeSet};

use hyperion_base::rng::DetailSeed;
use hyperion_base::units::Metres;

use crate::bake::{TerrainBake, TerrainBakeOptions, UNSURVEYED, bake_terrain};
use crate::cube::{FaceUv, MAX_LEVEL, PATCH_QUADS, PatchKey, face_uv_to_xyz, unit_dir};
use crate::field::{BodyRef, FieldView, InsertBlockError, PartialField, cell_at_index, cell_index};
use crate::geometry::vertex_spacing;
use crate::material::{
    GroundMaterial, Material, MaterialProperties, PhaseLaw, ResolvePropertiesError, material_at,
    properties, scaled_normal_albedo,
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

/// The cells a block surveyed that its body's field did not hold as surveyed, as runs of cell
/// indices at the field's level ([`cell_index`]): where a baked patch's survey mask can change.
/// Empty for a block that surveys nothing new: a margin, a finer survey of cells held, or a block
/// sent again.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct SurveyChange {
    runs: Vec<(u32, u32)>,
}

impl SurveyChange {
    /// The runs, each its first cell index and the index past its last, sorted, disjoint and with
    /// no two adjacent.
    #[must_use]
    pub fn runs(&self) -> &[(u32, u32)] {
        &self.runs
    }

    /// Whether the block surveyed no cell anew.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.runs.is_empty()
    }

    /// Whether the block surveyed the cell of index `index` anew.
    #[must_use]
    pub fn surveys(&self, index: u32) -> bool {
        let after = self.runs.partition_point(|&(start, _)| start <= index);
        after > 0 && index < self.runs[after - 1].1
    }

    /// Whether the block surveyed anew any of `indices`.
    #[must_use]
    pub fn surveys_any(&self, indices: &[u32]) -> bool {
        indices.iter().any(|&index| self.surveys(index))
    }

    /// Adds the cell of index `index`, the next in rising order.
    fn push(&mut self, index: u32) {
        match self.runs.last_mut() {
            Some((_, end)) if *end == index => *end = index + 1,
            _ => self.runs.push((index, index + 1)),
        }
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

    /// Whether a block that made `change` makes this bake stale, so that the patch is baked again
    /// (the [module documentation](self)'s rebake rule): the bake is not complete, or the block
    /// surveys a cell its mask holds unsurveyed. Where it is `false`, the patch bakes the same
    /// bytes after the block.
    #[must_use]
    pub fn needs_rebake(&self, change: &SurveyChange) -> bool {
        !self.terrain.layers().is_complete() || change.surveys_any(&self.unsurveyed_cells)
    }

    /// The two parts, to hand on without a copy.
    #[must_use]
    pub fn into_parts(self) -> (TerrainBake, Vec<u32>) {
        (self.terrain, self.unsurveyed_cells)
    }
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
    /// answers with the cells it surveys.
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

    /// Adds `bytes`, one block, to `body`'s field, and answers with the cells it surveys anew.
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

/// The cells `block` surveys that `field` does not hold as surveyed. A cell `field` already holds
/// keeps the finer of its codes and a cell it does not takes the block's, so a surveyed code in the
/// block makes the cell surveyed whatever was held ([`PartialField::insert`]).
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
        if !range.code().is_surveyed() {
            continue;
        }
        for index in range.start()..range.end() {
            let cell = cell_at_index(level.get(), index).expect("a cover's cells are the level's");
            if !field.is_surveyed(cell) {
                change.push(index);
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
