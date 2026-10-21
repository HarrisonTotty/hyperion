//! What the client's WebAssembly module exports, on `wasm32-unknown-unknown` only.
//!
//! `just gen-surface` builds this crate for the browser target and runs `wasm-bindgen --target
//! web` over it, into the renderer's `generated/surface/`, where a module worker loads it (plan
//! R04, T10.b and T10.c). `#[wasm_bindgen]` writes the exports, which compiles under the
//! workspace's `forbid(unsafe_code)` where a raw `#[unsafe(no_mangle)]` export would not (R04
//! Design notes 12 and 16). The exports are thin: each calls the crate's own function, which the
//! native and wasip1 tests already check.
//!
//! # The bake's layout (plan R05, T5), which the TypeScript side mirrors
//!
//! `bakePatch(face, level, i, j, vertexPath, normals, ridges, skirtM)` bakes the test planet's
//! patch (`face` 0–5 in S2's order, `level` 0–24, cell `i`, `j`; `vertexPath` a `VertexPath`,
//! `BakedOffsets` (0) or `FaceDifferences` (1); `normals` a `NormalScale`, `Mesh` (0) for the
//! mesh's 65 × 65 or `Double` (1) for 129 × 129; `ridges` a `Ridges`, `Off` (0) or `On` (1);
//! `skirtM` an extra skirt margin in metres) and
//! returns a `BakedPatch` whose getters each copy one array out of linear memory into a new typed
//! array the worker owns and may transfer:
//!
//! - `heights()`: `Float32Array` of 65 × 65 × 2, vertex (x, y) at `2 (65 y + x)`: its own height,
//!   then its morph target, metres above the spheroid;
//! - `offsets()`: `Float32Array` of 65 × 65 × 6 for `BakedOffsets`, vertex (x, y) at
//!   `6 (65 y + x)`: q₀ then q₁, metres from the origin, body-fixed; `undefined` for
//!   `FaceDifferences`;
//! - `normals()`: `Float32Array` of N × N × 2, N = 65 or 129, sample (x, y) at `2 (N y + x)`: the
//!   octahedral pair of the unit normal, body-fixed, which the worker packs into a `Float16Array`;
//! - `origin()`: `Float64Array` of 3, the origin, body-fixed metres; `originHeightM`,
//!   `heightRangeM()` (`Float32Array` of 2, lowest then highest), `boundingRadiusM` and
//!   `skirtDepthM`, metres.
//!
//! `testPlanetVersion()` is the crate's `TEST_PLANET_VERSION`, which the client compares with the
//! version its own tests were written against, as it does `generatorVersion()`: a change to the
//! test planet bumps only that, so a stale module would otherwise go unseen.
//!
//! `omittedSigmaM(level, ridges)` is `σ_n`, the RMS height of the octaves the level omits,
//! metres: 0 from the finest level on, smaller with ridges on where a ridged octave is omitted
//! (T6; decisions-r05.md item 6). A statistical figure for `min(ε_n, 4σ_n)`, never a bound.
//!
//! `surfaceHeightM(x, y, z, ridges)` is the height of the drawn finest mesh, metres above the
//! spheroid, at the body-fixed direction (x, y, z), which need not be unit length: R05.T4.c's
//! collision interpolant (`patch::collision::finest_surface_height`), the finest level's vertex
//! heights interpolated on the mesh's (0, 0)–(1, 1) diagonal with the query's (s, t) weights, a
//! query within 10⁻⁶ of a quad of a lattice line snapped onto it. Wherever the morph is 0 this is
//! the drawn surface to the `f32` step of its heights. It throws for a zero or non-finite
//! direction. Each call starts an empty lattice cache, about 3 × 22 octave evaluations.
//!
//! `levelTable(ridges)` returns a `Float64Array` of 25 × 4, level n at `4 n`: the level bound `ε_n`,
//! the lowest and highest height, and the largest vertex spacing, metres (T6).
//!
//! # A body's field (plan R10, T6.b)
//!
//! Real bodies are served beside the test planet by a `SurfaceFields` object the worker makes once
//! (`new SurfaceFields()`), the crate's [`WorkerFields`]: one client's field a body, each named by
//! its system's raw 64-bit ID (a `bigint`) and its index there, the two halves of the wire's
//! `BodyIdHex`, and each query given the body's detail seed, the 64 bits of the wire's
//! `DetailSeedHex` (a `bigint`). Every method throws a `JsError` for an argument out of its range,
//! so that no input traps the module: a body index above 65,535, a face, level or cell that names
//! no patch, a point off its face, a slope baseline outside 1 mm to 10,000 km, a line of sight's
//! origin outside 1 m to 10¹² m from the centre, a direction with no finite length, a reach past
//! 10¹² m, or a skirt margin that is negative.
//!
//! - `fieldInit(system, body, block)` starts the body's field from one block 0 (a `Uint8Array`),
//!   replacing any held, and `fieldChunk(system, body, block)` adds a later block. Each returns a
//!   `FieldBlock`, or throws naming why the block was refused, the field then as it was. A
//!   `FieldBlock`'s `newlySurveyed()` is a `Uint32Array` of runs, each the first cell index and the
//!   index past the last, at the field's level, `fieldLevel`, of the cells the block surveyed
//!   anew; its `newlyHeld()` the runs, laid out alike, of the cells it brought, surveyed or margin,
//!   that the field did not hold ([`SurveyChange::held_runs`], R10.T8.b's rebake rule); and its
//!   `levelTable()` the body's level table in `levelTable(ridges)`'s layout
//!   ([`WorkerFields::level_table`]). `fieldRelease(system, body)` drops it, `true` if one was
//!   held, and `heldBytes` is what every field holds on the heap.
//! - `bake(system, body, seed, face, level, i, j, vertexPath, normals, weights, horizon, skirtM)`
//!   bakes the patch as `bakePatch` does, `weights` a `WeightsLayer` and `horizon` a
//!   `HorizonLayer`, each `Baked` (0) or `Skipped` (1), and returns a `BakedTerrain`, or
//!   `undefined` where the field cannot make it yet: none is held, or a surveyed vertex reads a
//!   cell still to come. A `BakedTerrain` has `BakedPatch`'s getters and R10's layers:
//!   `palette()`, a `Uint8Array` of 8 × 2, each slot its class code then its entry (255 for no
//!   entry; both 255 for an empty slot); `weights()`, a `Uint8Array` of 65 × 65 × 8, vertex (x, y)
//!   at `8 (65 y + x)`, each byte over the palette's slot of its place, empty when skipped;
//!   `surveyMask()`, a `Uint8Array` of 65 × 65, 255 surveyed and 0 not; `boundM` and `sigmaM`,
//!   metres; `complete`; `unsurveyedCells()`, a `Uint32Array` of the cell indices its mask holds
//!   unsurveyed, rising ([`BodyBake::unsurveyed_cells`]); and the horizon map (R10.T8.a's layout,
//!   `bake::horizon`): `horizonNear()`, a `Uint32Array` of 65 × 65 × 4 words, vertex (x, y) at
//!   `4 (65 y + x)`, azimuth 2w in word w's low half and 2w + 1 in its high half as
//!   `pack2x16snorm` packs them, and `horizonFar()`, of 9 × 9 × 4 words, point (a, b) at
//!   `4 (9 b + a)`, both empty when skipped; `horizonComplete`
//!   ([`HorizonMap::is_complete`](crate::bake::HorizonMap::is_complete)); and
//!   `horizonRefusedCells()`, a `Uint32Array` of the cell indices its far field was refused for,
//!   rising ([`HorizonMap::refused_cells`](crate::bake::HorizonMap::refused_cells)).
//! - `reading(system, body, seed, face, u, v, slopeBaselineM)` returns a `Reading`
//!   ([`SurveyedReading`]'s getters, in metres and radians), or `undefined` where the ground is
//!   not surveyed or no field is held: the readout's `NOT SURVEYED`.
//! - `slantRange(system, body, seed, ox, oy, oz, dx, dy, dz, maxM)` returns a `LineOfSightAnswer`,
//!   whose `kind` is a `LineOfSightKind` (`Ground`, `NoGround`, `NotSurveyed`), with the range's
//!   getters on `Ground`. Where no field is held, or the march reads past the cells held, it is
//!   `NotSurveyed`.
//! - `material(system, body, seed, face, u, v)` returns a `GroundAnswer`, the class's code, its
//!   entry and, where the palette names the entry, its properties, or `undefined` where the cell
//!   is not surveyed or no field is held.
//! - `groundReadout(system, body, seed, ox, oy, oz, dx, dy, dz, maxM, slopeBaselineM, padWidthM,
//!   seasonMeanAnomalyRad)` returns a `GroundReadoutAnswer`, a view's whole terrain readout in one
//!   query ([`WorkerFields::ground_readout`], plan R10, T12): its `kind` a `LineOfSightKind`, and
//!   on `Ground` `slantRange`'s getters (`rangeM`, `rangeSigmaM`, `grazing`, `rangeResolved`,
//!   `hitM()`, `atFace`, `atU`, `atV`), `Reading`'s at the foot (`elevationM`, `elevationSigmaM`,
//!   `slopeRmsRad`, `slopeSigmaRad`, `slopeBaselineM`, `resolutionM`, `readingResolved`), the
//!   material's `classCode` and `entry`, and the ground's properties: `bearingCapacityPa`, its
//!   `capacitySlopeRad` (the RMS slope over 2 m), `padFriction`, `frictionOnTemperature` and
//!   `temperatureK`, each `undefined` where not computed. `NotSurveyed` where no field is held, the
//!   march reads past the cells held, or the foot's cell is not surveyed. The line of sight is
//!   checked as `slantRange`'s; the baseline must be one of 2 m, 5 m and 100 m, the pad width
//!   finite and positive, and the season's mean anomaly, radians from periapsis, `undefined` for
//!   none or finite.
//! - `photometry(system, body, palette)` takes a bake's `palette()` (a `Uint8Array` of 8 × 2) and
//!   returns a `Float32Array` of 8 × 5, slot k at `5 k`: the photometry the lit terrain draws its
//!   material with ([`WorkerFields::photometry`], plan R10, T10.b): its normal albedo in B, V and
//!   R under the field's albedo scale, its law's code (R10.T1.a's `PhaseLaw`, 1 to 3) and the
//!   law's L(0); all five 0, law code 0 for none, for an empty slot or one whose palette names no
//!   substance. It returns `undefined` where no field of the body is held, and throws for a
//!   palette of another length, a class code no class has, an entry no palette holds, or one past
//!   the body's palette.
//! - `classMap(system, body, seed)` returns a `ClassMapAnswer`, the body's coarse class map for
//!   R07's disc ([`WorkerFields::class_map`], plan R10, T10.d), or `undefined` where no field of the
//!   body is held: its `level` (6, 64 texels along a face's side) and `fieldLevel`; `classes()`, a
//!   `Uint8Array` of two bytes a class, its class code then its entry (255 for none), at most 16;
//!   `photometry()`, a `Float32Array` of 5 a class as `photometry`'s; `texels()`, a `Uint32Array`
//!   of the cell index at `level` of each texel the survey touches, rising; `fractions()`, a
//!   `Float32Array` of each texel's surveyed share; `weights()`, a `Uint8Array` of each texel's
//!   bytes over the classes, texel t's at `classes · t`; and `complete`, `false` where some texel's
//!   weights read a cell the field does not hold yet.
//! - `normalMap(system, body, seed)` returns a `NormalMapAnswer`, the body's normal map for R07's
//!   disc ([`WorkerFields::normal_map`], plan R10, T10.g), or `undefined` where no field of the
//!   body is held: its `level`, the field's (5 to 8); `cells()`, a `Uint32Array` of the cell index
//!   at `level` of each surveyed cell answered, rising; `pairs()`, a `Float32Array` of two a cell,
//!   cell c's at `2 c`, the octahedral pair of its unit normal, body-fixed, as a bake's `normals()`
//!   holds them; and `complete`, `false` where some cell's height read a cell the field does not
//!   hold yet.

use hyperion_base::rng::DetailSeed;
use hyperion_base::units::{Kelvin, Metres, Pascals, Radians};
use wasm_bindgen::JsError;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::bake::{HorizonLayer, TerrainBakeOptions, TerrainLayers, WeightsLayer};
use crate::cube::{Face, FaceUv, PatchKey};
use crate::field::{BodyRef, FieldView, cell_index};
use crate::geometry::{BAND_LIMIT_M, FINEST_SPACING_M};
use crate::material::{MAX_CLASSES, Material, MaterialClass, MaterialProperties};
use crate::noise::LatticeCache;
use crate::patch::{BakeOptions, NormalScale, PatchBake, VertexPath};
use crate::reading::{LineOfSight, SlantRange, SurveyedReading};
use crate::test_planet::{Ridges, TEST_PLANET};
use crate::worker::{
    BodyBake, CLASS_MAP_LEVEL, ClassMap, DrawnPhotometry, GroundAtSight, GroundProperties,
    GroundReadout, GroundReadoutQuery, NormalMap, QueryBodyError, SeasonAnomaly, SlopeBaseline,
    SurveyChange, SurveyedMaterial, WorkerFields,
};

/// The generator version this module computes surfaces for, the crate's `generator_version`.
///
/// The client compares it with the server's generator version when the module loads, so that a
/// stale build of the module, which would draw terrain the server does not collide with, is
/// reported as a fault of the client's module.
#[wasm_bindgen(js_name = generatorVersion)]
#[must_use]
pub fn generator_version() -> u32 {
    crate::generator_version()
}

/// The terrain's band limit, metres ([`BAND_LIMIT_M`]).
#[wasm_bindgen(js_name = bandLimitM)]
#[must_use]
pub fn band_limit_m() -> f64 {
    BAND_LIMIT_M
}

/// The brainstorm's finest vertex spacing, metres ([`FINEST_SPACING_M`]).
#[wasm_bindgen(js_name = finestSpacingM)]
#[must_use]
pub fn finest_spacing_m() -> f64 {
    FINEST_SPACING_M
}

/// The test planet's version, `TEST_PLANET_VERSION` (see the module documentation).
#[wasm_bindgen(js_name = testPlanetVersion)]
#[must_use]
pub fn test_planet_version() -> u32 {
    crate::TEST_PLANET_VERSION
}

/// The vertex path, as the client names it.
#[wasm_bindgen(js_name = VertexPath)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsVertexPath {
    /// [`VertexPath::BakedOffsets`].
    BakedOffsets = 0,
    /// [`VertexPath::FaceDifferences`].
    FaceDifferences = 1,
}

/// The normals' resolution, as the client names it.
#[wasm_bindgen(js_name = NormalScale)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsNormalScale {
    /// [`NormalScale::Mesh`].
    Mesh = 0,
    /// [`NormalScale::Double`].
    Double = 1,
}

/// Whether the test planet's octaves 8 to 12 are ridged, as the client names it.
#[wasm_bindgen(js_name = Ridges)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsRidges {
    /// [`Ridges::Off`].
    Off = 0,
    /// [`Ridges::On`].
    On = 1,
}

/// The test planet with `ridges`.
fn planet(ridges: JsRidges) -> crate::test_planet::TestPlanet {
    TEST_PLANET.with_ridges(match ridges {
        JsRidges::Off => Ridges::Off,
        JsRidges::On => Ridges::On,
    })
}

/// `σ_n`, the RMS height of the octaves level `level` omits, metres
/// (`TestPlanet::omitted_sigma_m`; see the module documentation).
///
/// # Errors
///
/// A `JsError` if `level` is above 24.
#[wasm_bindgen(js_name = omittedSigmaM)]
pub fn omitted_sigma_m(level: u8, ridges: JsRidges) -> Result<f64, JsError> {
    if level > crate::cube::MAX_LEVEL {
        return Err(JsError::new(&format!("level {level} is above 24")));
    }
    Ok(planet(ridges).omitted_sigma_m(level))
}

/// The height of the drawn finest mesh at the body-fixed direction (`x`, `y`, `z`), metres above
/// the spheroid: `patch::collision::finest_surface_height` over the test planet, the collision
/// interpolant (see the module documentation).
///
/// # Errors
///
/// A `JsError` if the direction is zero or has a component that is not finite.
#[wasm_bindgen(js_name = surfaceHeightM)]
pub fn surface_height_m(x: f64, y: f64, z: f64, ridges: JsRidges) -> Result<f64, JsError> {
    let dir = [x, y, z];
    if !(dir.iter().all(|c| c.is_finite()) && dir.iter().any(|&c| c != 0.0)) {
        return Err(JsError::new(&format!(
            "direction ({x}, {y}, {z}) is not finite and non-zero"
        )));
    }
    let mut cache = LatticeCache::new();
    match crate::patch::collision::finest_surface_height(&planet(ridges), dir, &mut cache) {
        Ok(h) => Ok(h),
        Err(never) => match never {},
    }
}

/// The level table of the test planet (see the module documentation).
#[wasm_bindgen(js_name = levelTable)]
#[must_use]
pub fn level_table(ridges: JsRidges) -> Vec<f64> {
    planet(ridges).level_table()
}

/// A baked patch, whose getters copy its arrays out (see the module documentation).
#[wasm_bindgen]
#[derive(Debug)]
pub struct BakedPatch {
    bake: PatchBake,
}

#[wasm_bindgen]
impl BakedPatch {
    /// Own heights and morph targets, interleaved.
    #[must_use]
    pub fn heights(&self) -> Vec<f32> {
        self.bake.heights.clone()
    }

    /// The baked offsets q₀ and q₁, or none on the `FaceDifferences` path.
    #[must_use]
    pub fn offsets(&self) -> Option<Vec<f32>> {
        self.bake.offsets.clone()
    }

    /// The octahedral normal pairs.
    #[must_use]
    pub fn normals(&self) -> Vec<f32> {
        self.bake.normals.clone()
    }

    /// The origin, body-fixed metres.
    #[must_use]
    pub fn origin(&self) -> Vec<f64> {
        self.bake.origin.to_vec()
    }

    /// The origin's height, metres.
    #[wasm_bindgen(getter, js_name = originHeightM)]
    #[must_use]
    pub fn origin_height_m(&self) -> f64 {
        self.bake.origin_height_m
    }

    /// The lowest and highest baked height, metres.
    #[wasm_bindgen(js_name = heightRangeM)]
    #[must_use]
    pub fn height_range_m(&self) -> Vec<f32> {
        vec![self.bake.height_range_m.0, self.bake.height_range_m.1]
    }

    /// The radius about the origin holding the patch, metres.
    #[wasm_bindgen(getter, js_name = boundingRadiusM)]
    #[must_use]
    pub fn bounding_radius_m(&self) -> f64 {
        self.bake.bounding_radius_m
    }

    /// The skirts' depth, metres.
    #[wasm_bindgen(getter, js_name = skirtDepthM)]
    #[must_use]
    pub fn skirt_depth_m(&self) -> f64 {
        self.bake.skirt_depth_m
    }
}

/// Bakes the test planet's patch (see the module documentation).
///
/// # Errors
///
/// A `JsError` naming the problem if the face, level or cell is out of range, or the skirt margin
/// is negative or not finite.
#[wasm_bindgen(js_name = bakePatch)]
#[expect(
    clippy::too_many_arguments,
    reason = "a flat call is the binding's cheapest shape; the TypeScript side wraps it"
)]
pub fn bake_patch(
    face: u8,
    level: u8,
    i: u32,
    j: u32,
    vertex_path: JsVertexPath,
    normals: JsNormalScale,
    ridges: JsRidges,
    skirt_m: f64,
) -> Result<BakedPatch, JsError> {
    let key = patch_key(u32::from(face), u32::from(level), i, j)?;
    let opts = bake_options(vertex_path, normals, skirt_m)?;
    let mut cache = LatticeCache::new();
    let bake = match crate::patch::bake_patch(&planet(ridges), key, &opts, &mut cache) {
        Ok(bake) => bake,
        Err(never) => match never {},
    };
    Ok(BakedPatch { bake })
}

/// A small integer of the client's, checked into a byte (the JavaScript number reaches the module
/// as a `u32`, so that one past a byte is refused rather than wrapped).
fn byte_of(value: u32, name: &str) -> Result<u8, JsError> {
    u8::try_from(value).map_err(|_| JsError::new(&format!("{name} {value} is above 255")))
}

/// The face of index `face`.
fn face_of(face: u32) -> Result<Face, JsError> {
    Face::try_from(byte_of(face, "face")?).map_err(|e| JsError::new(&e.to_string()))
}

/// The patch key of `face`, `level`, `i` and `j`.
fn patch_key(face: u32, level: u32, i: u32, j: u32) -> Result<PatchKey, JsError> {
    PatchKey::new(face_of(face)?, byte_of(level, "level")?, i, j)
        .map_err(|e| JsError::new(&e.to_string()))
}

/// Body `body` of the system whose raw ID is `system`.
fn body_ref(system: u64, body: u32) -> Result<BodyRef, JsError> {
    let index = u16::try_from(body)
        .map_err(|_| JsError::new(&format!("body index {body} is above 65,535")))?;
    Ok(BodyRef::new(system, index))
}

/// R05's bake options from the client's names.
fn bake_options(
    vertex_path: JsVertexPath,
    normals: JsNormalScale,
    skirt_m: f64,
) -> Result<BakeOptions, JsError> {
    let vertex_path = match vertex_path {
        JsVertexPath::BakedOffsets => VertexPath::BakedOffsets,
        JsVertexPath::FaceDifferences => VertexPath::FaceDifferences,
    };
    let normals = match normals {
        JsNormalScale::Mesh => NormalScale::Mesh,
        JsNormalScale::Double => NormalScale::Double,
    };
    if !(skirt_m.is_finite() && skirt_m >= 0.0) {
        return Err(JsError::new(&format!(
            "skirt margin {skirt_m} is not finite and non-negative"
        )));
    }
    Ok(BakeOptions {
        vertex_path,
        normals,
        skirt_m,
    })
}

/// The point (`face`, `u`, `v`), each coordinate finite and in [−1, 1].
fn face_uv(face: u32, u: f64, v: f64) -> Result<FaceUv, JsError> {
    let face = face_of(face)?;
    let on_face = |x: f64| x.is_finite() && (-1.0..=1.0).contains(&x);
    if !(on_face(u) && on_face(v)) {
        return Err(JsError::new(&format!(
            "({u}, {v}) is not a point of a face, each coordinate in [-1, 1]"
        )));
    }
    Ok(FaceUv { face, u, v })
}

/// Whether the class weights are baked, as the client names it (R10.T6.a's `WeightsLayer`).
#[wasm_bindgen(js_name = WeightsLayer)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsWeightsLayer {
    /// [`WeightsLayer::Baked`].
    Baked = 0,
    /// [`WeightsLayer::Skipped`].
    Skipped = 1,
}

/// Whether the horizon map is baked, as the client names it (R10.T8.a's `HorizonLayer`).
#[wasm_bindgen(js_name = HorizonLayer)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsHorizonLayer {
    /// [`HorizonLayer::Baked`].
    Baked = 0,
    /// [`HorizonLayer::Skipped`].
    Skipped = 1,
}

/// Where a line of sight meets the ground, as the client names it ([`LineOfSight`]).
#[wasm_bindgen(js_name = LineOfSightKind)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsLineOfSightKind {
    /// [`LineOfSight::Ground`].
    Ground = 0,
    /// [`LineOfSight::NoGround`].
    NoGround = 1,
    /// [`LineOfSight::NotSurveyed`].
    NotSurveyed = 2,
}

/// The shortest slope baseline `reading` takes, metres: a millimetre, far above where the
/// slope's b² would underflow, and finer than any baseline a readout states (2 m, 5 m and 100 m,
/// R10's Design note 7).
const MIN_SLOPE_BASELINE_M: f64 = 1e-3;

/// The longest slope baseline `reading` takes, metres: 10,000 km, past any body's radius, far
/// below where the baseline's ends would overflow the datum foot (about 10¹⁵⁴ m).
const MAX_SLOPE_BASELINE_M: f64 = 1e7;

/// The nearest a line of sight's origin may be to the body's centre, metres: a metre, deep under
/// any ground, where the range is zero, and clear of the centre, where the march has no foot.
const MIN_SIGHT_ORIGIN_M: f64 = 1.0;

/// The farthest a line of sight's origin may be from the body's centre, and its longest reach,
/// metres: 10¹² m, about 7 au, beyond any view's hand-over range, and far below where the march's
/// datum foot would overflow (about 10¹⁵⁴ m).
const MAX_SIGHT_M: f64 = 1e12;

/// The Euclidean length of `v`, as [`crate::cube::unit_dir`] takes it: infinite or zero where the
/// squares overflow or underflow.
#[must_use]
fn norm([x, y, z]: [f64; 3]) -> f64 {
    (x * x + y * y + z * z).sqrt()
}

/// A line of sight's reach `max_m`, metres, once its origin, direction and reach are checked.
///
/// # Errors
///
/// A `JsError` if the origin is not between [`MIN_SIGHT_ORIGIN_M`] and [`MAX_SIGHT_M`] from the
/// centre, the direction has no finite, non-zero length, or the reach is not between 0 and
/// [`MAX_SIGHT_M`].
fn sight_of(origin: [f64; 3], direction: [f64; 3], max_m: f64) -> Result<Metres, JsError> {
    let ([ox, oy, oz], [dx, dy, dz]) = (origin, direction);
    let origin_m = norm(origin);
    if !(MIN_SIGHT_ORIGIN_M..=MAX_SIGHT_M).contains(&origin_m) {
        return Err(JsError::new(&format!(
            "origin ({ox}, {oy}, {oz}) m is not between {MIN_SIGHT_ORIGIN_M} m and \
             {MAX_SIGHT_M} m from the centre"
        )));
    }
    let length = norm(direction);
    if !(length.is_finite() && length > 0.0) {
        return Err(JsError::new(&format!(
            "direction ({dx}, {dy}, {dz}) has no finite, non-zero length"
        )));
    }
    if !(0.0..=MAX_SIGHT_M).contains(&max_m) {
        return Err(JsError::new(&format!(
            "reach {max_m} m is not between 0 and {MAX_SIGHT_M} m"
        )));
    }
    Ok(Metres::new(max_m))
}

/// The palette byte of no entry, and of an empty slot's class and entry.
const NONE_BYTE: u8 = 255;

/// The values `photometry` gives a palette slot: `A_N` in B, V and R, the law's code and L(0).
const PHOTOMETRY_STRIDE: usize = 5;

/// The client's fields, one a body ([`WorkerFields`]; see the module documentation).
#[wasm_bindgen(js_name = SurfaceFields)]
#[derive(Debug, Default)]
pub struct JsSurfaceFields {
    fields: WorkerFields,
}

/// What a block taken changed, and the body's level table after it (see the module
/// documentation).
#[wasm_bindgen]
#[derive(Debug)]
pub struct FieldBlock {
    level: u8,
    runs: Vec<u32>,
    held: Vec<u32>,
    table: Vec<f64>,
}

#[wasm_bindgen]
impl FieldBlock {
    /// The runs of cells the block surveyed anew, each its first index and the index past its
    /// last, flat.
    #[wasm_bindgen(js_name = newlySurveyed)]
    #[must_use]
    pub fn newly_surveyed(&self) -> Vec<u32> {
        self.runs.clone()
    }

    /// The runs of cells the block brought, surveyed or margin, that the field did not hold, each
    /// its first index and the index past its last, flat.
    #[wasm_bindgen(js_name = newlyHeld)]
    #[must_use]
    pub fn newly_held(&self) -> Vec<u32> {
        self.held.clone()
    }

    /// The field's level, 5 to 8, which the runs' indices are of.
    #[wasm_bindgen(getter, js_name = fieldLevel)]
    #[must_use]
    pub fn field_level(&self) -> u8 {
        self.level
    }

    /// The body's level table, 25 × 4.
    #[wasm_bindgen(js_name = levelTable)]
    #[must_use]
    pub fn level_table(&self) -> Vec<f64> {
        self.table.clone()
    }
}

/// A patch baked over a body's field: [`BakedPatch`]'s arrays and R10's layers (see the module
/// documentation).
#[wasm_bindgen]
#[derive(Debug)]
pub struct BakedTerrain {
    bake: PatchBake,
    layers: TerrainLayers,
    unsurveyed: Vec<u32>,
}

#[wasm_bindgen]
impl BakedTerrain {
    /// Own heights and morph targets, interleaved.
    #[must_use]
    pub fn heights(&self) -> Vec<f32> {
        self.bake.heights.clone()
    }

    /// The baked offsets q₀ and q₁, or none on the `FaceDifferences` path.
    #[must_use]
    pub fn offsets(&self) -> Option<Vec<f32>> {
        self.bake.offsets.clone()
    }

    /// The octahedral normal pairs.
    #[must_use]
    pub fn normals(&self) -> Vec<f32> {
        self.bake.normals.clone()
    }

    /// The origin, body-fixed metres.
    #[must_use]
    pub fn origin(&self) -> Vec<f64> {
        self.bake.origin.to_vec()
    }

    /// The origin's height, metres.
    #[wasm_bindgen(getter, js_name = originHeightM)]
    #[must_use]
    pub fn origin_height_m(&self) -> f64 {
        self.bake.origin_height_m
    }

    /// The lowest and highest baked height, metres.
    #[wasm_bindgen(js_name = heightRangeM)]
    #[must_use]
    pub fn height_range_m(&self) -> Vec<f32> {
        vec![self.bake.height_range_m.0, self.bake.height_range_m.1]
    }

    /// The radius about the origin holding the patch, metres.
    #[wasm_bindgen(getter, js_name = boundingRadiusM)]
    #[must_use]
    pub fn bounding_radius_m(&self) -> f64 {
        self.bake.bounding_radius_m
    }

    /// The skirts' depth, metres.
    #[wasm_bindgen(getter, js_name = skirtDepthM)]
    #[must_use]
    pub fn skirt_depth_m(&self) -> f64 {
        self.bake.skirt_depth_m
    }

    /// The palette's eight slots, each its class code then its entry.
    #[must_use]
    pub fn palette(&self) -> Vec<u8> {
        self.layers
            .palette()
            .iter()
            .flat_map(|slot| {
                slot.map_or([NONE_BYTE, NONE_BYTE], |m| {
                    [u8::from(m.class()), m.entry().unwrap_or(NONE_BYTE)]
                })
            })
            .collect()
    }

    /// Each vertex's eight weight bytes over the palette, empty when the weights were skipped.
    #[must_use]
    pub fn weights(&self) -> Vec<u8> {
        self.layers.weights().iter().flatten().copied().collect()
    }

    /// Each vertex's survey mask, 255 surveyed and 0 not.
    #[wasm_bindgen(js_name = surveyMask)]
    #[must_use]
    pub fn survey_mask(&self) -> Vec<u8> {
        self.layers.survey_mask().to_vec()
    }

    /// The patch's hard bound, metres.
    #[wasm_bindgen(getter, js_name = boundM)]
    #[must_use]
    pub fn bound_m(&self) -> f64 {
        self.layers.bound().value()
    }

    /// The patch's σ, metres.
    #[wasm_bindgen(getter, js_name = sigmaM)]
    #[must_use]
    pub fn sigma_m(&self) -> f64 {
        self.layers.sigma().value()
    }

    /// Whether every vertex was answered by the field ([`TerrainLayers::is_complete`]).
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn complete(&self) -> bool {
        self.layers.is_complete()
    }

    /// The cell indices the mask holds unsurveyed, rising.
    #[wasm_bindgen(js_name = unsurveyedCells)]
    #[must_use]
    pub fn unsurveyed_cells(&self) -> Vec<u32> {
        self.unsurveyed.clone()
    }

    /// The horizon map's near field, four words a vertex, empty when skipped.
    #[wasm_bindgen(js_name = horizonNear)]
    #[must_use]
    pub fn horizon_near(&self) -> Vec<u32> {
        self.layers.horizon().near_words().to_vec()
    }

    /// The horizon map's far field, four words a point, empty when skipped.
    #[wasm_bindgen(js_name = horizonFar)]
    #[must_use]
    pub fn horizon_far(&self) -> Vec<u32> {
        self.layers.horizon().far_words().to_vec()
    }

    /// Whether the horizon map's far field read only ground the field holds.
    #[wasm_bindgen(getter, js_name = horizonComplete)]
    #[must_use]
    pub fn horizon_complete(&self) -> bool {
        self.layers.horizon().is_complete()
    }

    /// The cell indices the horizon map's far field was refused for, rising.
    #[wasm_bindgen(js_name = horizonRefusedCells)]
    #[must_use]
    pub fn horizon_refused_cells(&self) -> Vec<u32> {
        self.layers.horizon().refused_cells().to_vec()
    }
}

impl From<BodyBake> for BakedTerrain {
    fn from(bake: BodyBake) -> Self {
        let (terrain, unsurveyed) = bake.into_parts();
        let (bake, layers) = terrain.into_parts();
        Self {
            bake,
            layers,
            unsurveyed,
        }
    }
}

/// A reading of the ground ([`SurveyedReading`]).
#[wasm_bindgen]
#[derive(Debug)]
pub struct Reading {
    reading: SurveyedReading,
}

#[wasm_bindgen]
impl Reading {
    /// The elevation above the datum, metres.
    #[wasm_bindgen(getter, js_name = elevationM)]
    #[must_use]
    pub fn elevation_m(&self) -> f64 {
        self.reading.elevation().value()
    }

    /// The elevation's standard uncertainty, metres.
    #[wasm_bindgen(getter, js_name = elevationSigmaM)]
    #[must_use]
    pub fn elevation_sigma_m(&self) -> f64 {
        self.reading.elevation_sigma().value()
    }

    /// The RMS slope over the baseline, radians.
    #[wasm_bindgen(getter, js_name = slopeRmsRad)]
    #[must_use]
    pub fn slope_rms_rad(&self) -> f64 {
        self.reading.slope_rms().value()
    }

    /// The slope's standard uncertainty, radians.
    #[wasm_bindgen(getter, js_name = slopeSigmaRad)]
    #[must_use]
    pub fn slope_sigma_rad(&self) -> f64 {
        self.reading.slope_sigma().value()
    }

    /// The slope's baseline, metres.
    #[wasm_bindgen(getter, js_name = slopeBaselineM)]
    #[must_use]
    pub fn slope_baseline_m(&self) -> f64 {
        self.reading.slope_baseline().value()
    }

    /// The resolution the reading was taken at, metres.
    #[wasm_bindgen(getter, js_name = resolutionM)]
    #[must_use]
    pub fn resolution_m(&self) -> f64 {
        self.reading.resolution().value()
    }

    /// Whether the survey reaches the band limit, so that the uncertainties are zero.
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn resolved(&self) -> bool {
        self.reading.is_resolved()
    }
}

/// Where a line of sight meets the ground ([`LineOfSight`]).
#[wasm_bindgen]
#[derive(Debug)]
pub struct LineOfSightAnswer {
    kind: JsLineOfSightKind,
    ground: Option<SlantRange>,
}

#[wasm_bindgen]
impl LineOfSightAnswer {
    /// Whether it meets surveyed ground, no ground, or ground not surveyed.
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn kind(&self) -> JsLineOfSightKind {
        self.kind
    }

    /// The slant range, metres, on `Ground`.
    #[wasm_bindgen(getter, js_name = rangeM)]
    #[must_use]
    pub fn range_m(&self) -> Option<f64> {
        self.ground.map(|g| g.range().value())
    }

    /// The range's standard uncertainty, metres, on `Ground` where not grazing.
    #[wasm_bindgen(getter, js_name = sigmaM)]
    #[must_use]
    pub fn sigma_m(&self) -> Option<f64> {
        self.ground.and_then(|g| g.sigma()).map(Metres::value)
    }

    /// Whether the line of sight grazes the ground, on `Ground`.
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn grazing(&self) -> bool {
        self.ground.is_some_and(|g| g.is_grazing())
    }

    /// Whether the survey reaches the band limit at the hit, on `Ground`.
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn resolved(&self) -> bool {
        self.ground.is_some_and(|g| g.is_resolved())
    }

    /// The hit, body-fixed metres, on `Ground`.
    #[wasm_bindgen(js_name = hitM)]
    #[must_use]
    pub fn hit_m(&self) -> Option<Vec<f64>> {
        self.ground.map(|g| g.hit_m().to_vec())
    }

    /// The face of the hit's foot, on `Ground`.
    #[wasm_bindgen(getter, js_name = atFace)]
    #[must_use]
    pub fn at_face(&self) -> Option<u8> {
        self.ground.map(|g| g.at().face.index())
    }

    /// The foot's u, its face coordinate in [−1, 1] on `atFace`, on `Ground`.
    #[wasm_bindgen(getter, js_name = atU)]
    #[must_use]
    pub fn at_u(&self) -> Option<f64> {
        self.ground.map(|g| g.at().u)
    }

    /// The foot's v, its face coordinate in [−1, 1] on `atFace`, on `Ground`.
    #[wasm_bindgen(getter, js_name = atV)]
    #[must_use]
    pub fn at_v(&self) -> Option<f64> {
        self.ground.map(|g| g.at().v)
    }
}

impl From<LineOfSight> for LineOfSightAnswer {
    fn from(sight: LineOfSight) -> Self {
        match sight {
            LineOfSight::Ground(g) => Self {
                kind: JsLineOfSightKind::Ground,
                ground: Some(g),
            },
            LineOfSight::NoGround => Self {
                kind: JsLineOfSightKind::NoGround,
                ground: None,
            },
            LineOfSight::NotSurveyed => Self {
                kind: JsLineOfSightKind::NotSurveyed,
                ground: None,
            },
        }
    }
}

/// The material at a surveyed point ([`SurveyedMaterial`]).
#[wasm_bindgen]
#[derive(Debug)]
pub struct GroundAnswer {
    material: SurveyedMaterial,
}

impl GroundAnswer {
    /// The properties, where the palette names the entry.
    #[must_use]
    fn properties(&self) -> Option<&MaterialProperties> {
        self.material.properties()
    }
}

#[wasm_bindgen]
impl GroundAnswer {
    /// The form's class code (R10.T1.a's `MaterialClass`).
    #[wasm_bindgen(getter, js_name = classCode)]
    #[must_use]
    pub fn class_code(&self) -> u8 {
        u8::from(self.material.ground().class())
    }

    /// The palette entry, where the palette names the substance.
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn entry(&self) -> Option<u8> {
        self.material.ground().entry()
    }

    /// The normal albedo in the Johnson B, V and R bands, where the entry is named.
    #[wasm_bindgen(js_name = normalAlbedoBvr)]
    #[must_use]
    pub fn normal_albedo_bvr(&self) -> Option<Vec<f64>> {
        self.properties().map(|p| p.normal_albedo_bvr().to_vec())
    }

    /// The photometric law's code (R10.T1.a's `PhaseLaw`), where the entry is named.
    #[wasm_bindgen(getter, js_name = phaseLaw)]
    #[must_use]
    pub fn phase_law(&self) -> Option<u8> {
        self.properties().map(|p| u8::from(p.phase_law()))
    }

    /// The law's L(0) at the material's albedo in V, where the entry is named.
    #[wasm_bindgen(getter, js_name = limbParameter)]
    #[must_use]
    pub fn limb_parameter(&self) -> Option<f64> {
        self.properties().map(MaterialProperties::limb_parameter)
    }

    /// The substance's grain density, kg/m³, where the entry is named.
    #[wasm_bindgen(getter, js_name = grainDensityKgM3)]
    #[must_use]
    pub fn grain_density_kg_m3(&self) -> Option<f64> {
        self.properties().map(|p| p.grain_density().value())
    }

    /// The substance's transition temperature, K, where the entry is named.
    #[wasm_bindgen(getter, js_name = transitionK)]
    #[must_use]
    pub fn transition_k(&self) -> Option<f64> {
        self.properties().map(|p| p.transition().value())
    }

    /// The substance's mechanics family's code (R09's `MechanicsFamily`), where the entry is
    /// named.
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn mechanics(&self) -> Option<u8> {
        self.properties().map(|p| u8::from(p.mechanics()))
    }
}

/// A view's terrain readout where its line of sight meets the ground ([`GroundReadout`], plan R10,
/// T12): its kind, and on `Ground` the slant range, the reading, the material and the ground's
/// properties, each getter `undefined` (or `false`) off the ground.
#[wasm_bindgen]
#[derive(Debug)]
pub struct GroundReadoutAnswer {
    readout: GroundReadout,
}

impl GroundReadoutAnswer {
    /// The readout's parts, on `Ground`.
    #[must_use]
    fn ground(&self) -> Option<&GroundAtSight> {
        match &self.readout {
            GroundReadout::Ground(ground) => Some(ground),
            GroundReadout::NoGround | GroundReadout::NotSurveyed => None,
        }
    }

    /// The slant range, on `Ground`.
    #[must_use]
    fn sight(&self) -> Option<&SlantRange> {
        self.ground().map(GroundAtSight::sight)
    }

    /// The reading, on `Ground`.
    #[must_use]
    fn reading(&self) -> Option<&SurveyedReading> {
        self.ground().map(GroundAtSight::reading)
    }

    /// The ground's properties, on `Ground`.
    #[must_use]
    fn properties(&self) -> Option<&GroundProperties> {
        self.ground().map(GroundAtSight::properties)
    }
}

#[wasm_bindgen]
impl GroundReadoutAnswer {
    /// Whether the line of sight meets surveyed ground, no ground, or ground not surveyed.
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn kind(&self) -> JsLineOfSightKind {
        match self.readout {
            GroundReadout::Ground(_) => JsLineOfSightKind::Ground,
            GroundReadout::NoGround => JsLineOfSightKind::NoGround,
            GroundReadout::NotSurveyed => JsLineOfSightKind::NotSurveyed,
        }
    }

    /// The slant range, metres, on `Ground`.
    #[wasm_bindgen(getter, js_name = rangeM)]
    #[must_use]
    pub fn range_m(&self) -> Option<f64> {
        self.sight().map(|s| s.range().value())
    }

    /// The range's standard uncertainty, metres, on `Ground` where not grazing.
    #[wasm_bindgen(getter, js_name = rangeSigmaM)]
    #[must_use]
    pub fn range_sigma_m(&self) -> Option<f64> {
        self.sight().and_then(SlantRange::sigma).map(Metres::value)
    }

    /// Whether the line of sight grazes the ground, on `Ground`.
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn grazing(&self) -> bool {
        self.sight().is_some_and(SlantRange::is_grazing)
    }

    /// Whether the survey reaches the band limit at the hit, on `Ground`.
    #[wasm_bindgen(getter, js_name = rangeResolved)]
    #[must_use]
    pub fn range_resolved(&self) -> bool {
        self.sight().is_some_and(SlantRange::is_resolved)
    }

    /// The hit, body-fixed metres, on `Ground`.
    #[wasm_bindgen(js_name = hitM)]
    #[must_use]
    pub fn hit_m(&self) -> Option<Vec<f64>> {
        self.sight().map(|s| s.hit_m().to_vec())
    }

    /// The face of the hit's foot, on `Ground`.
    #[wasm_bindgen(getter, js_name = atFace)]
    #[must_use]
    pub fn at_face(&self) -> Option<u8> {
        self.sight().map(|s| s.at().face.index())
    }

    /// The foot's u, its face coordinate in [−1, 1] on `atFace`, on `Ground`.
    #[wasm_bindgen(getter, js_name = atU)]
    #[must_use]
    pub fn at_u(&self) -> Option<f64> {
        self.sight().map(|s| s.at().u)
    }

    /// The foot's v, its face coordinate in [−1, 1] on `atFace`, on `Ground`.
    #[wasm_bindgen(getter, js_name = atV)]
    #[must_use]
    pub fn at_v(&self) -> Option<f64> {
        self.sight().map(|s| s.at().v)
    }

    /// The elevation above the datum at the foot, metres, on `Ground`.
    #[wasm_bindgen(getter, js_name = elevationM)]
    #[must_use]
    pub fn elevation_m(&self) -> Option<f64> {
        self.reading().map(|r| r.elevation().value())
    }

    /// The elevation's standard uncertainty, metres, on `Ground`.
    #[wasm_bindgen(getter, js_name = elevationSigmaM)]
    #[must_use]
    pub fn elevation_sigma_m(&self) -> Option<f64> {
        self.reading().map(|r| r.elevation_sigma().value())
    }

    /// The RMS slope over the baseline, radians, on `Ground`.
    #[wasm_bindgen(getter, js_name = slopeRmsRad)]
    #[must_use]
    pub fn slope_rms_rad(&self) -> Option<f64> {
        self.reading().map(|r| r.slope_rms().value())
    }

    /// The slope's standard uncertainty, radians, on `Ground`.
    #[wasm_bindgen(getter, js_name = slopeSigmaRad)]
    #[must_use]
    pub fn slope_sigma_rad(&self) -> Option<f64> {
        self.reading().map(|r| r.slope_sigma().value())
    }

    /// The slope's baseline, metres, on `Ground`.
    #[wasm_bindgen(getter, js_name = slopeBaselineM)]
    #[must_use]
    pub fn slope_baseline_m(&self) -> Option<f64> {
        self.reading().map(|r| r.slope_baseline().value())
    }

    /// The resolution the reading was taken at, metres, on `Ground`.
    #[wasm_bindgen(getter, js_name = resolutionM)]
    #[must_use]
    pub fn resolution_m(&self) -> Option<f64> {
        self.reading().map(|r| r.resolution().value())
    }

    /// Whether the survey reaches the band limit at the foot, so that the reading's uncertainties
    /// are zero, on `Ground`.
    #[wasm_bindgen(getter, js_name = readingResolved)]
    #[must_use]
    pub fn reading_resolved(&self) -> bool {
        self.reading().is_some_and(SurveyedReading::is_resolved)
    }

    /// The form's class code (R10.T1.a's `MaterialClass`), on `Ground`.
    #[wasm_bindgen(getter, js_name = classCode)]
    #[must_use]
    pub fn class_code(&self) -> Option<u8> {
        self.ground()
            .map(|g| u8::from(g.material().ground().class()))
    }

    /// The palette entry, on `Ground` where the palette names the substance.
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn entry(&self) -> Option<u8> {
        self.ground().and_then(|g| g.material().ground().entry())
    }

    /// The ultimate bearing capacity, Pa, on `Ground` where it is computed.
    #[wasm_bindgen(getter, js_name = bearingCapacityPa)]
    #[must_use]
    pub fn bearing_capacity_pa(&self) -> Option<f64> {
        self.properties()
            .and_then(GroundProperties::bearing_capacity)
            .map(Pascals::value)
    }

    /// The RMS slope over 2 m the capacity was taken on, radians, on `Ground`.
    #[wasm_bindgen(getter, js_name = capacitySlopeRad)]
    #[must_use]
    pub fn capacity_slope_rad(&self) -> Option<f64> {
        self.properties().map(|p| p.capacity_slope().value())
    }

    /// The pad's interface friction, dimensionless, on `Ground` where it is computed.
    #[wasm_bindgen(getter, js_name = padFriction)]
    #[must_use]
    pub fn pad_friction(&self) -> Option<f64> {
        self.properties().and_then(GroundProperties::friction)
    }

    /// Whether the friction depends on the temperature, on `Ground`.
    #[wasm_bindgen(getter, js_name = frictionOnTemperature)]
    #[must_use]
    pub fn friction_on_temperature(&self) -> bool {
        self.properties()
            .is_some_and(GroundProperties::friction_on_temperature)
    }

    /// The month's mean temperature at the point, K, on `Ground` where it is computed.
    #[wasm_bindgen(getter, js_name = temperatureK)]
    #[must_use]
    pub fn temperature_k(&self) -> Option<f64> {
        self.properties()
            .and_then(GroundProperties::temperature)
            .map(Kelvin::value)
    }
}

impl From<GroundReadout> for GroundReadoutAnswer {
    fn from(readout: GroundReadout) -> Self {
        Self { readout }
    }
}

/// `answer`'s value, `None` where the field cannot make it yet: no field of the body is held, or
/// the answer reads a cell still to come.
#[expect(
    clippy::manual_ok_err,
    reason = "the match names every error, so that a new kind is a compile error, not a silent None"
)]
#[must_use]
fn unless_unsurveyed<T>(answer: Result<T, QueryBodyError>) -> Option<T> {
    match answer {
        Ok(value) => Some(value),
        Err(QueryBodyError::NotStarted(_) | QueryBodyError::NotSurveyed(_)) => None,
    }
}

#[wasm_bindgen(js_class = SurfaceFields)]
impl JsSurfaceFields {
    /// No field held.
    #[wasm_bindgen(constructor)]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts the body's field from `block`, a block 0 (see the module documentation).
    ///
    /// # Errors
    ///
    /// A `JsError` naming why the block was refused ([`WorkerFields::init`]).
    #[wasm_bindgen(js_name = fieldInit)]
    pub fn field_init(
        &mut self,
        system: u64,
        body: u32,
        block: &[u8],
    ) -> Result<FieldBlock, JsError> {
        let body = body_ref(system, body)?;
        let change = self
            .fields
            .init(body, block)
            .map_err(|e| JsError::new(&e.to_string()))?;
        Ok(self.answer(body, &change))
    }

    /// Adds `block` to the body's field (see the module documentation).
    ///
    /// # Errors
    ///
    /// A `JsError` naming why the block was refused ([`WorkerFields::chunk`]).
    #[wasm_bindgen(js_name = fieldChunk)]
    pub fn field_chunk(
        &mut self,
        system: u64,
        body: u32,
        block: &[u8],
    ) -> Result<FieldBlock, JsError> {
        let body = body_ref(system, body)?;
        let change = self
            .fields
            .chunk(body, block)
            .map_err(|e| JsError::new(&e.to_string()))?;
        Ok(self.answer(body, &change))
    }

    /// Drops the body's field; whether one was held.
    #[wasm_bindgen(js_name = fieldRelease)]
    ///
    /// # Errors
    ///
    /// A `JsError` if the body's index is above 65,535.
    pub fn field_release(&mut self, system: u64, body: u32) -> Result<bool, JsError> {
        Ok(self.fields.release(body_ref(system, body)?))
    }

    /// The bytes every field holds on the heap ([`WorkerFields::heap_bytes`]).
    #[wasm_bindgen(getter, js_name = heldBytes)]
    #[must_use]
    pub fn held_bytes(&self) -> usize {
        self.fields.heap_bytes()
    }

    /// Bakes a patch of the body (see the module documentation).
    ///
    /// # Errors
    ///
    /// A `JsError` naming the problem if the face, level or cell is out of range, or the skirt
    /// margin is negative or not finite.
    #[expect(
        clippy::too_many_arguments,
        reason = "a flat call is the binding's cheapest shape; the TypeScript side wraps it"
    )]
    pub fn bake(
        &mut self,
        system: u64,
        body: u32,
        seed: u64,
        face: u32,
        level: u32,
        i: u32,
        j: u32,
        vertex_path: JsVertexPath,
        normals: JsNormalScale,
        weights: JsWeightsLayer,
        horizon: JsHorizonLayer,
        skirt_m: f64,
    ) -> Result<Option<BakedTerrain>, JsError> {
        let key = patch_key(face, level, i, j)?;
        let opts = TerrainBakeOptions {
            patch: bake_options(vertex_path, normals, skirt_m)?,
            weights: match weights {
                JsWeightsLayer::Baked => WeightsLayer::Baked,
                JsWeightsLayer::Skipped => WeightsLayer::Skipped,
            },
            horizon: match horizon {
                JsHorizonLayer::Baked => HorizonLayer::Baked,
                JsHorizonLayer::Skipped => HorizonLayer::Skipped,
            },
        };
        let answer = self
            .fields
            .bake(body_ref(system, body)?, DetailSeed::new(seed), key, &opts);
        Ok(unless_unsurveyed(answer).map(BakedTerrain::from))
    }

    /// The reading at (`face`, `u`, `v`) on the body (see the module documentation).
    ///
    /// # Errors
    ///
    /// A `JsError` if the point is not on a face or the baseline is out of its range.
    #[expect(
        clippy::too_many_arguments,
        reason = "a flat call is the binding's cheapest shape; the TypeScript side wraps it"
    )]
    pub fn reading(
        &mut self,
        system: u64,
        body: u32,
        seed: u64,
        face: u32,
        u: f64,
        v: f64,
        slope_baseline_m: f64,
    ) -> Result<Option<Reading>, JsError> {
        let at = face_uv(face, u, v)?;
        if !(MIN_SLOPE_BASELINE_M..=MAX_SLOPE_BASELINE_M).contains(&slope_baseline_m) {
            return Err(JsError::new(&format!(
                "slope baseline {slope_baseline_m} m is not between {MIN_SLOPE_BASELINE_M} m and \
                 {MAX_SLOPE_BASELINE_M} m"
            )));
        }
        let answer = self.fields.reading(
            body_ref(system, body)?,
            DetailSeed::new(seed),
            at,
            Metres::new(slope_baseline_m),
        );
        Ok(unless_unsurveyed(answer)
            .flatten()
            .map(|reading| Reading { reading }))
    }

    /// Where the line of sight from (`ox`, `oy`, `oz`) along (`dx`, `dy`, `dz`), body-fixed,
    /// metres, meets the body's ground within `max_m` (see the module documentation).
    ///
    /// # Errors
    ///
    /// A `JsError` if the origin is not between [`MIN_SIGHT_ORIGIN_M`] and [`MAX_SIGHT_M`] from
    /// the centre, the direction has no finite, non-zero length, or the reach is not between 0 and
    /// [`MAX_SIGHT_M`].
    #[wasm_bindgen(js_name = slantRange)]
    #[expect(
        clippy::too_many_arguments,
        reason = "a flat call is the binding's cheapest shape; the TypeScript side wraps it"
    )]
    pub fn slant_range(
        &mut self,
        system: u64,
        body: u32,
        seed: u64,
        ox: f64,
        oy: f64,
        oz: f64,
        dx: f64,
        dy: f64,
        dz: f64,
        max_m: f64,
    ) -> Result<LineOfSightAnswer, JsError> {
        let (origin, direction) = ([ox, oy, oz], [dx, dy, dz]);
        let max = sight_of(origin, direction, max_m)?;
        let answer = self.fields.slant_range(
            body_ref(system, body)?,
            DetailSeed::new(seed),
            &origin,
            &direction,
            max,
        );
        Ok(LineOfSightAnswer::from(
            unless_unsurveyed(answer).unwrap_or(LineOfSight::NotSurveyed),
        ))
    }

    /// The material at (`face`, `u`, `v`) on the body (see the module documentation).
    ///
    /// # Errors
    ///
    /// A `JsError` if the point is not on a face.
    pub fn material(
        &mut self,
        system: u64,
        body: u32,
        seed: u64,
        face: u32,
        u: f64,
        v: f64,
    ) -> Result<Option<GroundAnswer>, JsError> {
        let at = face_uv(face, u, v)?;
        let answer = self
            .fields
            .material(body_ref(system, body)?, DetailSeed::new(seed), at);
        Ok(unless_unsurveyed(answer)
            .flatten()
            .map(|material| GroundAnswer { material }))
    }

    /// The terrain readout where the line of sight from (`ox`, `oy`, `oz`) along (`dx`, `dy`,
    /// `dz`), body-fixed, metres, meets the body's ground within `max_m` (see the module
    /// documentation; [`WorkerFields::ground_readout`]).
    ///
    /// # Errors
    ///
    /// A `JsError` for a line of sight `slantRange` refuses, a slope baseline that is none of
    /// 2 m, 5 m and 100 m, a pad width that is not finite and positive, a season's mean anomaly
    /// that is given and not finite (`undefined` is none), or a body index above 65,535.
    #[wasm_bindgen(js_name = groundReadout)]
    #[expect(
        clippy::too_many_arguments,
        reason = "a flat call is the binding's cheapest shape; the TypeScript side wraps it"
    )]
    pub fn ground_readout(
        &mut self,
        system: u64,
        body: u32,
        seed: u64,
        ox: f64,
        oy: f64,
        oz: f64,
        dx: f64,
        dy: f64,
        dz: f64,
        max_m: f64,
        slope_baseline_m: f64,
        pad_width_m: f64,
        season_mean_anomaly_rad: Option<f64>,
    ) -> Result<GroundReadoutAnswer, JsError> {
        let (origin_m, direction) = ([ox, oy, oz], [dx, dy, dz]);
        let max = sight_of(origin_m, direction, max_m)?;
        let slope_baseline = SlopeBaseline::try_from(Metres::new(slope_baseline_m))
            .map_err(|e| JsError::new(&e.to_string()))?;
        if !(pad_width_m.is_finite() && pad_width_m > 0.0) {
            return Err(JsError::new(&format!(
                "pad width {pad_width_m} m is not finite and positive"
            )));
        }
        let season = season_mean_anomaly_rad
            .map(|m| SeasonAnomaly::try_from(Radians::new(m)))
            .transpose()
            .map_err(|e| JsError::new(&e.to_string()))?;
        let query = GroundReadoutQuery {
            origin_m,
            direction,
            max,
            slope_baseline,
            pad_width: Metres::new(pad_width_m),
            season,
        };
        let answer =
            self.fields
                .ground_readout(body_ref(system, body)?, DetailSeed::new(seed), &query);
        Ok(GroundReadoutAnswer::from(
            unless_unsurveyed(answer).unwrap_or(GroundReadout::NotSurveyed),
        ))
    }

    /// The photometry of each slot of a bake's `palette` on the body (see the module
    /// documentation).
    ///
    /// # Errors
    ///
    /// A `JsError` naming the problem if the body index is above 65,535, the palette is not 8 × 2
    /// bytes, or a slot's class code names no class, its entry is one no palette holds, or past the
    /// body's palette.
    pub fn photometry(
        &self,
        system: u64,
        body: u32,
        palette: &[u8],
    ) -> Result<Option<Vec<f32>>, JsError> {
        let body = body_ref(system, body)?;
        if palette.len() != 2 * MAX_CLASSES {
            return Err(JsError::new(&format!(
                "a palette of {} bytes, not {MAX_CLASSES} slots of 2",
                palette.len()
            )));
        }
        // Checked first, so that a palette of empty slots alone answers `undefined` too.
        if self.fields.field(body).is_none() {
            return Ok(None);
        }
        let mut out = vec![0.0_f32; PHOTOMETRY_STRIDE * MAX_CLASSES];
        for (values, &[class, entry]) in out
            .as_chunks_mut::<PHOTOMETRY_STRIDE>()
            .0
            .iter_mut()
            .zip(palette.as_chunks::<2>().0)
        {
            if entry == NONE_BYTE {
                continue;
            }
            let class = MaterialClass::try_from(class)
                .map_err(|_| JsError::new(&format!("class code {class} names no class")))?;
            let material = Material::new(class, entry)
                .ok_or_else(|| JsError::new(&format!("entry {entry} is one no palette holds")))?;
            let drawn = self
                .fields
                .photometry(body, material)
                .map_err(|e| JsError::new(&e.to_string()))?;
            *values = photometry_values(&drawn);
        }
        Ok(Some(out))
    }

    /// The body's class map for R07's disc (see the module documentation), or `undefined` where no
    /// field of the body is held.
    ///
    /// # Errors
    ///
    /// A `JsError` if the body's index is above 65,535.
    #[wasm_bindgen(js_name = classMap)]
    pub fn class_map(
        &mut self,
        system: u64,
        body: u32,
        seed: u64,
    ) -> Result<Option<ClassMapAnswer>, JsError> {
        let answer = self
            .fields
            .class_map(body_ref(system, body)?, DetailSeed::new(seed));
        Ok(unless_unsurveyed(answer).map(|map| ClassMapAnswer { map }))
    }

    /// The body's normal map for R07's disc (see the module documentation), or `undefined` where
    /// no field of the body is held.
    ///
    /// # Errors
    ///
    /// A `JsError` if the body's index is above 65,535.
    #[wasm_bindgen(js_name = normalMap)]
    pub fn normal_map(
        &mut self,
        system: u64,
        body: u32,
        seed: u64,
    ) -> Result<Option<NormalMapAnswer>, JsError> {
        let answer = self
            .fields
            .normal_map(body_ref(system, body)?, DetailSeed::new(seed));
        Ok(unless_unsurveyed(answer).map(|map| NormalMapAnswer { map }))
    }
}

/// A material's photometry as the `photometry` and `classMap` exports give it: `A_N` in B, V and
/// R, the law's code and its L(0), narrowed to `f32`.
#[must_use]
fn photometry_values(drawn: &DrawnPhotometry) -> [f32; PHOTOMETRY_STRIDE] {
    let [b, v, r] = drawn.normal_albedo_bvr();
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the shader reads f32; a normal albedo of at most 1.35 and an L(0) in [0, 1] \
                  narrow far inside its range"
    )]
    let narrowed = [
        b as f32,
        v as f32,
        r as f32,
        f32::from(u8::from(drawn.phase_law())),
        drawn.limb_parameter() as f32,
    ];
    narrowed
}

/// A body's class map for R07's disc ([`ClassMap`]; see the module documentation).
#[wasm_bindgen]
#[derive(Debug)]
pub struct ClassMapAnswer {
    map: ClassMap,
}

#[wasm_bindgen]
impl ClassMapAnswer {
    /// The map's quadtree level, [`CLASS_MAP_LEVEL`]: 2^level texels along a face's side.
    #[wasm_bindgen(getter)]
    #[must_use]
    #[expect(
        clippy::unused_self,
        reason = "a getter of the exported object is a method; the level is the crate's constant"
    )]
    pub fn level(&self) -> u8 {
        CLASS_MAP_LEVEL
    }

    /// The level of the field the map was made from, 5 to 8.
    #[wasm_bindgen(getter, js_name = fieldLevel)]
    #[must_use]
    pub fn field_level(&self) -> u8 {
        self.map.field_level()
    }

    /// Its classes, two bytes each: the form's class code, then the palette entry (255 for none).
    #[must_use]
    pub fn classes(&self) -> Vec<u8> {
        self.map
            .classes()
            .iter()
            .flat_map(|class| {
                let material = class.material();
                [
                    u8::from(material.class()),
                    material.entry().unwrap_or(NONE_BYTE),
                ]
            })
            .collect()
    }

    /// Each class's photometry, [`PHOTOMETRY_STRIDE`] values a class as `photometry`'s; all 0, law
    /// code 0, where the palette names no substance.
    #[must_use]
    pub fn photometry(&self) -> Vec<f32> {
        self.map
            .classes()
            .iter()
            .flat_map(|class| {
                class
                    .photometry()
                    .map_or([0.0; PHOTOMETRY_STRIDE], |drawn| photometry_values(&drawn))
            })
            .collect()
    }

    /// The cell index ([`cell_index`]) at the map's level of each texel the survey touches, rising.
    #[must_use]
    pub fn texels(&self) -> Vec<u32> {
        self.map
            .texels()
            .iter()
            .map(|t| cell_index(t.cell()))
            .collect()
    }

    /// Each texel's surveyed share, (0, 1], in [`texels`](Self::texels)' order.
    #[must_use]
    pub fn fractions(&self) -> Vec<f32> {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a share of at most 16 cells, k ÷ 16, is exact in f32"
        )]
        let narrow = |share: f64| share as f32;
        self.map
            .texels()
            .iter()
            .map(|t| narrow(t.surveyed_fraction()))
            .collect()
    }

    /// Each texel's weight bytes over the classes, texel t's at `classes · t`, in
    /// [`texels`](Self::texels)' order.
    #[must_use]
    pub fn weights(&self) -> Vec<u8> {
        let classes = self.map.classes().len();
        let texels = self.map.texels();
        let mut out = Vec::with_capacity(texels.len() * classes);
        for t in texels {
            out.extend_from_slice(&t.weights()[..classes]);
        }
        out
    }

    /// Whether every texel the survey touches was answered ([`ClassMap::is_complete`]).
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn complete(&self) -> bool {
        self.map.is_complete()
    }
}

/// A body's normal map for R07's disc ([`NormalMap`]; see the module documentation).
#[wasm_bindgen]
#[derive(Debug)]
pub struct NormalMapAnswer {
    map: NormalMap,
}

#[wasm_bindgen]
impl NormalMapAnswer {
    /// The level of the field the map was made from, 5 to 8, whose cells it holds.
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn level(&self) -> u8 {
        self.map.level()
    }

    /// The cell index ([`cell_index`]) at the map's level of each surveyed cell answered, rising.
    #[must_use]
    pub fn cells(&self) -> Vec<u32> {
        self.map.cells().to_vec()
    }

    /// Each cell's octahedral pair, two values a cell in [`cells`](Self::cells)' order: body-fixed,
    /// each in [−1, 1], as a bake's `normals()` holds them.
    #[must_use]
    pub fn pairs(&self) -> Vec<f32> {
        self.map.pairs().as_flattened().to_vec()
    }

    /// Whether every surveyed cell was answered ([`NormalMap::is_complete`]).
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn complete(&self) -> bool {
        self.map.is_complete()
    }
}

impl JsSurfaceFields {
    /// The answer to a block of `body` taken with `change`.
    #[must_use]
    fn answer(&self, body: BodyRef, change: &SurveyChange) -> FieldBlock {
        let field = self
            .fields
            .field(body)
            .expect("a block taken leaves its body's field held");
        let flat = |runs: &[(u32, u32)]| runs.iter().flat_map(|&(a, b)| [a, b]).collect();
        FieldBlock {
            level: field.header().level().get(),
            runs: flat(change.runs()),
            held: flat(change.held_runs()),
            table: self
                .fields
                .level_table(body)
                .expect("a block taken leaves its body's field held"),
        }
    }
}
