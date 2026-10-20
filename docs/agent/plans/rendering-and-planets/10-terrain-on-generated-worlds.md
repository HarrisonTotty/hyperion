# Plan R10: Terrain on Generated Worlds

- **Milestone:** Rendering milestone RM5 (with R09).
- **Depends on:**
  [R05 Terrain geometry and the descent spike](05-terrain-geometry-and-descent-spike.md) and
  [R09 The surface generator](09-surface-generator.md). For the photorealistic style's shading it
  reads [R07](07-lit-bodies-styles-and-main-screen.md) (`body_brdf`, the horizon and eclipse terms
  `sphere_irradiance` and `eclipse_visible`, and the lit regimes), and [R08](08-atmospheres.md)'s
  aerial perspective where the body has an atmosphere.
  [R11](11-surface-detail-clouds-oceans-rings-stills.md) builds on this plan's hooks. Through them
  it reads R01–R04 and galaxy plans [12](../galaxy-generation/12-retarded-observation-alerts.md) and
  [14](../galaxy-generation/14-planetary-systems.md).
- **Brainstorm sections covered** (by heading, in
  [the rendering brainstorm](../../brainstorming/rendering-and-planets.md)): step 8, "Terrain", of
  "Suggested order of attack"; "The line between truth and decoration" for the material class and
  its physical properties; "Materials, and what a surface looks like" for the class baked beside the
  height and blended, never re-decided, by the shader; "Level-of-detail consistency, and why
  collision agrees"; "Knowledge, and the surface seed" for the readouts and the survey coverage
  readout; "Two styles of one renderer" and "Two deployments, one scene" for the wireframe's
  depth-only terrain at 4 px with contours; "Performance budget" for the horizon-map shadows, the
  height-texture cache and the terrain rows of the ladder; the items of "Testing" named "Collision
  agrees with what is drawn" and "The descent" as far as real terrain adds to them.

## Goal

When this plan is done, a generated world that the ship has surveyed is drawn from the server's
coarse field and the shared height function, on R05's quadtree, in both render styles. The client
fetches the surveyed coarse-field chunks R09 sends, holds them per body, and hands them to its
height workers, which bake each patch's heights, normals, material class weights, survey mask and
horizon map. The material class is authoritative: `hyperion-surface` decides it at every point, with
the albedo, friction and bearing capacity that landing and the consoles read, and the shader only
blends what the worker baked. Every class is lit by one photometric law, lunar-Lambert, with class
albedos scaled on the server so that the body's disc and its terrain agree. Collision reads one
function, the piecewise-linear ground through the finest level's vertices, and a test shows the
finest mesh agrees with it to the height texture's `f32` step, about 2 mm. Every coarser level stays
inside R09's stated bound on generated worlds, and that bound, not R05's provisional ratio, selects
the levels. Unsurveyed ground stays undrawn as terrain: the body's reference surface stands there,
and the survey edge is marked. Readouts of elevation, slope and slant range quote the ground only as
well as the ship has measured it, `~2140 m ± 180 m` until a close survey resolves the point. The
view carries the survey's coverage and resolution as a readout. The wireframe draws the same terrain
once, depth only, at a 4 px tolerance, with contours from the interpolated height. Terrain shadows
come from horizon maps everywhere, combined with cascades near the camera on the high setting. The
patch cache is sized per setting from measured selections.

## Scope and non-goals

In scope:

- `hyperion-surface`: the material classes, their property table, the bearing capacity and the
  authoritative classifier; the class weights coarser levels bake; the class-albedo scale; the
  ground wrapper collision reads; the surveyed readings with their uncertainty; the horizon-map
  bake.
- R05's bake and worker moved onto R09's height function and extended: class weights, the survey
  mask, horizon maps, the per-body level bounds handed to selection, and the hooks R11 adds to.
- The client's coarse-field store: requesting R09's `surface_field` for the bodies that need it,
  assembling the chunks, keeping the coverage and its resolution, posting to each worker once and
  per chunk, and releasing on departure.
- The photorealistic style's terrain: class shading through R07's `body_brdf`, the survey mask and
  edge, the coarse class map R07's disc samples, and the hand-over from disc to terrain.
- The wireframe style's terrain: depth-only patches at 4 px, contours evaluated in the same pass,
  the interval stated, the grounded-body rule in this style.
- Terrain shadows: horizon maps on both settings, soft by the star's disc, and on the high setting
  cascaded shadow maps near the camera combined with them.
- The view's survey coverage readout, the survey edge, and the readouts of the ground on the view's
  line of sight (`ELEVATION`, `SLOPE`, `SLANT RANGE`, `MATERIAL`, `RESOLUTION`) with coverage- and
  resolution-gated uncertainty.
- The patch cache's sizes, formats and slot layout per setting, and the resident-memory check.
- Draft UX guide entries these need, for the owner's sign-off.

Non-goals:

- The coarse pass, the per-query height function, `level_bound_m`, `unresolved_rms`, the chunk
  format, the coverage record and the detail seed: R09's. Knowledge gating on the server: R09's, on
  P12.T7's store.
- The quadtree, patch selection by τ, morphing, culling, the patch cache's eviction rule, the worker
  pool and streaming priority: R05's. This plan feeds R05's selection its real bounds and sizes its
  cache; it does not rebuild either.
- GPU decoration, textures of any kind below the band limit, triplanar projection and repetition
  hiding, scatter and authoritative rocks, clouds and cloud shadows, oceans and sea glint: R11's,
  through this plan's hooks (Design note 9).
- Aerial perspective and the atmosphere over terrain: R08's. This plan exposes the terrain pass to
  it.
- A flight model, contact dynamics, landing gear and the sensors that survey on the way down. The
  ground function is what they will call; nothing here calls it but tests. Landing hazard margins
  (3σ and the like) are theirs, multiplying the σ this plan states.
- A server request for readouts on consoles that draw no terrain. No such console exists yet; the
  pure function is the one a later request will wrap (Design note 7).
- The consolidated performance runs: R12's. This plan records its own benchmarks.
- Vegetation and any biosphere class: a later biosphere owes it. Lifeless ground keeps bare classes.

## Provides

Signatures are sketches, named to be grepped. Rust paths are under `hyperion_surface` unless a crate
is named. R09's types (`FieldView`, `PartialField`, `CoarseField`, `Synthesiser`, `SynthCache`,
`BandLevel`, `DetailSeed`, `ResolutionCode`), R05's (`PatchKey`, `Face`, `FaceUv`, `bake_patch`,
`BakeOptions`, `PatchBake`, `finest_surface_height`, `NormalScale`), R09's `height::HeightSample`
(moved there by R09.T4, re-exported by `test_planet`) and R11's `scatter::ExtraLayer` are theirs.
Positions and directions in the surface crate are body-fixed `[f64; 3]` in metres, as R05's are,
since the crate depends on base alone (R11 aliases the type `BodyFixed`); the client holds them as
R05's `BodyFixedVec3` (`view/terrain/planet.ts`, an alias of `geometry/vec3.ts`'s `Vec3`) and, at
R02's boundary, as a `ViewPosition` of kind `body_fixed`. `BodyFixedPosition` and
`BodyFixedVector` are the sim's Rust types (`coords/body_fixed.rs`), not the client's (re-validated
at `c6d77cac`).

### `material`

```rust
/// Authoritative ground classes. Discriminants are wire- and golden-stable `u8`s; a new class
/// appends, and any change to the set, the table or the classifier is a generator-version change.
/// As built by T1.a (Risks, "Deviations in T1.a, as built"): `WaterIce` is `Glacier` and
/// `VolatileFrost` is `Frost`, the forms of the cell's ice entry whatever its species; the codes
/// are unchanged. A `coded_enum!` (`ALL`, `u8::from`, `TryFrom<u8>`).
pub enum MaterialClass {
    Bedrock = 0, Regolith = 1, Soil = 2, Sand = 3, Sediment = 4, VolcanicRock = 5, Melt = 6,
    Glacier = 7, Snow = 8, SeaIce = 9, Frost = 10, OrganicSediment = 11, Seabed = 12,
    Evaporite = 13, Dust = 14,
}
/// The form and the substance: `entry` is the index of R09's header palette entry the ground is
/// made of (`field::MaterialPalette`, R09.T2's follow-up B). `FrostSpecies` is gone: the ice
/// entry names the species (decision-composition §1.8, folded in at the re-validation).
pub struct Material { /* class: MaterialClass, entry: u8 */ }   // new(class, entry) -> Option
/// The codes of R09's `PaletteEntry::phase_row`: 0 no law (`UNASSIGNED_PHASE_ROW`), 1 `Regolith`
/// (R07's `moon` template), 2 `Ice` (`airless-ice`), 3 `Lambert` (`lambert`: f ≡ 1, L ≡ 0), which
/// the form chooses for the consolidated forms (decision-r10-limb); L(α) = L(0) × McEwen 1996's
/// profile. `limb_parameter(a)` is 0 for Lambert and the particulate rule for the others;
/// `normal_albedo_ceiling()` is 1 for Lambert and 1.35 for the others, T10.a's clamp.
pub enum PhaseLaw { Regolith = 1, Ice = 2, Lambert = 3 }
// for_family, template, lommel_seeliger_share, limb_parameter, normal_albedo_ceiling
/// How a form's ground scatters: `Glacier`, `Snow`, `SeaIce` and `Evaporite` are consolidated
/// (the Lambert law), every other form particulate (`MaterialClass::photometric_form`).
pub enum PhotometricForm { Consolidated, Particulate }

pub struct MohrCoulomb { /* cohesion: Pascals, friction_angle: Radians */ }
pub struct ShearLayer { /* top: Metres, bottom: Metres, strength: MohrCoulomb, porosity: f64 */ }
pub enum FrictionModel { Constant(f64), ByTemperature(&'static [(Kelvin, f64)]) }  // tan δ, pad
/// A form's rows for one mechanics family, each figure with its `Citation`.
pub struct FormRow {
    /* shear: &'static [ShearLayer] (by depth, empty where unknown), interface_friction:
       Option<FrictionModel>, compressive_strength: Option<Pascals> (ice), albedo_factor:
       Option<f64> (on the entry's A_N), normal_albedo_v: Option<AlbedoSpan>,
       citations: &'static [Citation] */
}
pub fn form_row(class: MaterialClass, family: MechanicsFamily) -> &'static FormRow;
pub struct MaterialProperties {
    /* the form's `FormRow`, and the entry's resolved values (R09's `PaletteEntry`):
       normal_albedo_bvr: [f64; 3] (A_N in B, V and R, times the form's factor), phase_law:
       PhaseLaw with its PhaseLawOrigin (Palette, FamilyDefault or Form), grain_density, transition:
       Kelvin, mechanics: MechanicsFamily; each layer's bulk density from the grain density */
}
pub fn properties(material: Material, palette: &MaterialPalette)
    -> Result<MaterialProperties, ResolvePropertiesError>;
/// L(0) of a particulate lunar-Lambert law from the normal albedo: the particulate rule L_p, 1 up
/// to 0.60, 0.6696 ÷ x − 0.1160 to 0.38 at 1.35, 0.38 beyond (Design note 8; decision-r10-limb).
pub fn limb_parameter(normal_albedo: f64) -> f64;
// LUNAR_LIMB_ONSET_NORMAL_ALBEDO 0.60, BRIGHT_FROST_NORMAL_ALBEDO 1.35,
// BRIGHT_FROST_LIMB_PARAMETER 0.38, PARTICULATE_NORMAL_ALBEDO_CEILING 1.35
/// The f ≡ 1 case of the energy bound, exact for the Lambert law.
pub fn directional_hemispherical_albedo(normal_albedo: f64, limb: f64, mu0: f64) -> f64;
/// Ultimate bearing capacity under a footing (Design note 2); `None` where the table has no data.
pub fn bearing_capacity(p: &MaterialProperties, gravity: MetresPerSecondSquared,
    footing_width: Metres) -> Option<Pascals>;
pub fn ultimate_bearing_capacity(strength: MohrCoulomb, bulk_density: KilogramsPerCubicMetre,
    gravity: MetresPerSecondSquared, width: Metres) -> Pascals;
pub fn interface_friction(p: &MaterialProperties, temperature: Kelvin) -> Option<f64>;

/// The authoritative class at a point, from the finest level's slope, altitude, R09's climate and
/// surface-state classes, the ice field, and the cell's substance byte and the header's palette.
/// As built by T1.b (Risks, "Deviations in T1.b, as built"): a `GroundMaterial`, the form always
/// and the entry where the palette names one (signoff-9 §17.5's `MATERIAL REGOLITH · —`).
pub fn material_at<F: FieldView>(synth: &Synthesiser<'_, F>, cache: &mut SynthCache,
    at: FaceUv) -> Result<GroundMaterial, QueryHeightError>;
pub struct GroundMaterial { /* class: MaterialClass, entry: Option<u8> */ }  // material() -> Option<Material>
pub const MASSIVE_ICE_SHARE: f64;          // ½: glacier (or airless regolith of ice) or frost (T1.b)
pub const THIN_ATMOSPHERE_PRESSURE: Pascals;  // 30 kPa, the sim's, for `Dust` (T1.b)
pub const CLIFF_SLOPE: Radians;            // 40°, Design note 1
pub const TRANSITION_HALF_WIDTH: Radians;  // 5°, the visual blend band either side
pub const SNOW_SHED_SLOPE: Radians;        // 60°, Schaerer 1977, p. 286 (T1.a, as built)
/// Presentation weights at a point over up to eight materials, each a (form, entry) pair (Design
/// note 3); a patch's palette is T6.a's `TerrainLayers`, gathered from its vertices'. As built by
/// T2 (Risks, "Deviations in T2, as built"): each slot a `GroundMaterial`, the form always and the
/// entry where the palette names one, as `material_at` answers; slots in falling weight, ties by
/// discriminant, the first the largest (at the finest level, strictly, the authoritative class).
/// A texel of band level n is a cell of quadtree level n + 6.
pub struct ClassWeights { /* palette: [Option<GroundMaterial>; 8], weights: [u8; 8] summing to 255 */ }
// palette, weights, iter, weight(material), largest, len, is_empty; MAX_CLASSES 8,
// WEIGHT_TOTAL 255, SLOPE_LAG 5 cm (the omitted structure function's small-lag slope)
pub fn weights_at<F: FieldView>(synth: &Synthesiser<'_, F>, cache: &mut SynthCache,
    at: FaceUv, level: BandLevel) -> Result<ClassWeights, QueryHeightError>;
/// The per-body scale on class albedos that makes the body's geometric albedo p (Design note 8).
pub struct AlbedoScale { /* scale: f64, residual: f64 */ }
pub fn albedo_scale(fractions: &ClassFractions, palette: &MaterialPalette,
    geometric_albedo_bvr: [f64; 3]) -> Result<AlbedoScale, SolveAlbedoScaleError>;
/// The solve on the albedos a ground's exposure gives (R10.T1.c); `albedo_scale` is it at
/// `GroundExposure::NONE`, and the server solves it at `GroundExposure::of_header`.
pub fn exposed_albedo_scale(fractions: &ClassFractions, palette: &MaterialPalette,
    exposure: GroundExposure, geometric_albedo_bvr: [f64; 3])
    -> Result<AlbedoScale, SolveAlbedoScaleError>;
/// What a ground is exposed to: a maturity and an organic cover, each in [0, 1] or absent.
pub struct GroundExposure { /* maturity: Option<f64>, organic_cover: Option<f64> */ }
// NONE, of_header, with_maturity, with_organic_cover, maturity, organic_cover
/// `properties` with the albedo weathered to the exposure's maturity (`Melt` fresh), times the
/// form's factor, and blended with the organic cover where the entry takes one.
pub fn exposed_properties(material: Material, palette: &MaterialPalette,
    exposure: GroundExposure) -> Result<MaterialProperties, ResolvePropertiesError>;
/// Whether an entry takes the cover: an `Ice` formula holding carbon, CO₂ excepted, or `N2` beside one.
pub fn takes_organic_cover(palette: &MaterialPalette, entry: &PaletteEntry) -> bool;
/// The maturity at a point, m_local: m_body less the largest young-crater deficit there.
pub fn maturity_at<F: FieldView>(synth: &Synthesiser<'_, F>, cache: &mut SynthCache,
    at: FaceUv) -> Result<Option<f64>, QueryHeightError>;
/// One crater's maturity at fresh share s: m_body − s max(0, m_body − maturity_after(t_c, k)).
pub fn crater_maturity(body_maturity: f64, crater_age: Gigayears, weathering_rate: f64,
    fresh_share: f64) -> f64;
/// The fresh share's parts: f_ex below the mature layer d_m, and the halo p(x).
pub fn excavated_share(surface_age: Gigayears, crater_age: Gigayears, diameter: Metres) -> f64;
pub fn reworked_depth(exposure: Gigayears) -> Metres;
pub fn halo_share(distance: f64, outer: f64) -> f64;
// HALO_LENGTH_RIM_RADII 3, HALO_EXTENT_RIM_RADII 10, EXCAVATION_DEPTH_RATIO 0.084,
// REWORKING_DEPTH_AT_ONE_MYR 0.022 m, REWORKING_EXPONENT 0.45
```

### `ground` and `reading`

```rust
/// The authoritative ground: R05's `finest_surface_height` over R09's synthesis, with the
/// triangle it came from. Collision reads this and nothing else. As built R05's interpolant
/// returns the height alone (its lattice and split are private), so T3 adds the triangle's query
/// beside it in `patch::collision` (as built: the crate's `mesh_point` and `finest_surface_point`,
/// returning `MeshPoint`). The ground is at p's foot on the datum, along its normal (T3's
/// deviations).
pub fn ground_at<F: FieldView>(synth: &Synthesiser<'_, F>, cache: &mut SynthCache,
    p: &[f64; 3]) -> Result<GroundPoint, QueryHeightError>;            // body-fixed, metres
pub struct GroundPoint { /* height: Metres (above R07's reference spheroid, along its normal),
    normal: [f64; 3] (body-fixed, unit),
    triangle: FinestTriangle,
    as built also dir: [f64; 3] (p's foot) and point_height: Metres (p's own); getters */ }
/// As built, a type alias of R05's `patch::collision::MeshTriangle`, with getters and `vertices()`.
pub struct FinestTriangle { /* key: PatchKey (finest level), x: u8, y: u8, upper: bool */ }

/// A field view that knows each held cell's survey code (as built, T5): a survey's code, `NONE`
/// for a margin cell, `None` for a cell not held. Implemented for `PartialField`.
pub trait SurveyView: FieldView { fn resolution(&self, cell: PatchKey) -> Option<ResolutionCode>; }
/// What a readout may quote at a point, given the survey resolution the ship holds there; private
/// fields with getters (`is_resolved()` for `resolved`).
pub struct SurveyedReading {
    /* elevation: Metres, elevation_sigma: Metres (R09's level-keyed `omitted_rms` at the
       reading's level), slope_rms: Radians, slope_sigma: Radians, slope_baseline: Metres,
       resolution: Metres (the cell code's, for T12's RESOLUTION, as ruled),
       resolved: bool (sigma is zero: the survey reaches the band limit) */
}
pub fn surveyed_reading<F: SurveyView>(synth: &Synthesiser<'_, F>, cache: &mut SynthCache,
    at: FaceUv, slope_baseline: Metres)
    -> Result<Option<SurveyedReading>, QueryHeightError>;          // None: unsurveyed
/// Where a line of sight meets the ground, as far as the survey can say: `NO GROUND` and
/// `NOT SURVEYED` apart (as ruled, `decision-r10-t5-closed-forms.md`).
pub enum LineOfSight { Ground(SlantRange), NoGround, NotSurveyed }
pub struct SlantRange { /* range: Metres, sigma: Option<Metres> (None when grazing),
    resolved: bool, hit_m: [f64; 3] (body-fixed), at: FaceUv (the hit's foot); getters,
    `is_grazing()` (sigma is None) and `is_resolved()` */ }
pub fn slant_range<F: SurveyView>(synth: &Synthesiser<'_, F>, cache: &mut SynthCache,
    origin_m: &[f64; 3], direction: &[f64; 3], max: Metres)
    -> Result<LineOfSight, QueryHeightError>;
pub const SLOPE_BASELINES: [Metres; 3];   // 2 m (default), 5 m, 100 m, Design note 7
pub const GRAZING_FACTOR: f64;             // 2: grazing where sin γ_s < 2√v, Design note 7
```

As built (T5's deviations, ruled in `decision-r10-t5-closed-forms.md`): both read the survey from
the view through `SurveyView`, so `surveyed_reading` takes no `survey`; the σ and the slope's v are
R09's level-keyed closed forms, which T5 added to R09's `Synthesiser` (`omitted_variance`,
`omitted_structure_function`, `omitted_rms`, R09's Provides).

### `bake`

```rust
/// What this plan adds to R05's `PatchBake`. The palette is the union of the patch's vertices'
/// `ClassWeights` materials, `GroundMaterial`s as T2 built them (R10.T2's record).
pub struct TerrainLayers {
    /* palette: [Option<GroundMaterial>; 8], weights: Vec<[u8; 8]> (65 × 65),
       survey_mask: Vec<u8> (65 × 65: 255 surveyed, 0 not), horizon: HorizonMap,
       bound: Metres, sigma: Metres, extra: Vec<scatter::ExtraLayer> (R11's, through `BakeHook`) */
}
pub struct HorizonMap { /* azimuths: 8, near: 65 × 65 × 8, far: 9 × 9 × 8, elevations as
    snorm16 of e ÷ 90°, two to a u32, Design note 10 */ }
pub fn encode_horizon(elevation: Radians) -> i16;   // WGSL `pack2x16snorm`'s rounding, integer only
pub fn decode_horizon(code: i16) -> Radians;
/// R11 adds per-patch layers (the rock list) through this, in the same bake and cache. R11's
/// `RockBakeHook` owns its own `SynthCache`, so the hook takes nothing more. T6.a declares
/// `scatter::ExtraLayer` as an empty `#[non_exhaustive]` enum, which R11 owns and gives its
/// `Rocks(Vec<Rock>)` variant.
pub trait BakeHook { fn bake(&self, key: PatchKey, heights: &[f32], out: &mut Vec<ExtraLayer>); }
/// The face-wide far-field lattice: its points keyed by position alone, the same bits at
/// every level; its values a level's (Design note 10; main's ruling for R10.T8.a, 2026-10-15).
pub fn far_field_point(face: Face, level: u8, i: u32, j: u32) -> FaceUv;
/// As built by T6.a (Risks, "Deviations in T6.a, as built"): the bake over a client's field, R05's
/// arrays and this plan's layers; `WeightsLayer::{Baked, Skipped}`; `TerrainLayers`' fields
/// private, with getters and `is_complete()` (no vertex took the reference surface); the mask's
/// `SURVEYED` (255) and `UNSURVEYED` (0); `HorizonMap` empty until T8.a. As built by T8.a (Risks,
/// "Deviations in T8.a, as built"): `bake/horizon.rs` (`pub mod horizon`, its items re-exported by
/// `bake`); `TerrainBakeOptions` gains `horizon: HorizonLayer::{Baked, Skipped}`; the azimuths are
/// the patch grid's `AZIMUTH_STEPS` at k × 45° of `grid_azimuth(face, at, towards)`; `HorizonMap`'s
/// `near_words`, `far_words` (4 words a texel, `pack2x16snorm`'s halves), `near(x, y, k)`,
/// `far(a, b, k)`, `elevation_at(x, y, azimuth)` (the shader's lookup at a vertex) and
/// `is_complete()` (the far field read only ground the view holds); `NO_HORIZON`, `AZIMUTHS`,
/// `FAR_SIDE`, `FAR_STRIDE`, `WORDS_PER_TEXEL`, `ZENITH_TOLERANCE`.
pub struct TerrainBakeOptions { pub patch: BakeOptions, pub weights: WeightsLayer }
pub struct TerrainBake { /* patch: PatchBake, layers: TerrainLayers */ } // patch, layers, into_parts
pub fn bake_terrain<F: SurveyView>(synth: &Synthesiser<'_, F>, cache: &mut SynthCache,
    key: PatchKey, opts: &TerrainBakeOptions, hooks: &[&dyn BakeHook])
    -> Result<TerrainBake, QueryHeightError>;
```

The worker's WebAssembly entry points, on R05's binding, gain `field_init`, `field_chunk`,
`field_release` and a `bake` that takes R05's `BakeOptions` with this plan's settings, and answer
`field_init` with the body's `LevelBounds` (below). They replace R05's single `postField(bytes)`
with these per-body calls. For the readouts they also gain `reading`, `slant_range` and `material`,
the wasm faces of `surveyed_reading`, `slant_range` and `material_at` with `properties` (T6.b).
As built, R05's module (`src/wasm.rs`) serves the test planet alone: `bakePatch`, `levelTable`,
`surfaceHeightM` and `omittedSigmaM`, each taking `Ridges`, which the spike keeps (R09 Design note
18); the per-body exports go beside them, under the same camel-case `js_name` rule. _As built by
T6.b (Risks, "Deviations in T6.b, as built"): the per-body exports are the methods of one
`SurfaceFields` object a worker makes, over the crate's new `worker::WorkerFields` (a
`PartialField`, its level bounds and a `SynthCache` a body): `fieldInit`, `fieldChunk` and
`fieldRelease`, each block answered with the cells it surveyed anew and the body's level table in
`levelTable`'s layout; `bake`, with R10's layers and the cells the mask holds unsurveyed;
`reading`, `slantRange` and `material`; and `heldBytes`. A body is its system's raw ID and its
index, and each query is given the body's detail seed, which the field does not carry._

### Client (`apps/hyperion/src/renderer/src/view/terrain/`)

- `coarseFieldStore.ts`: `CoarseFieldStore` with `want(body, reason)`, `release(body)`,
  `coverage(body): CoverageView`, `onRevision`; it drives R09's `surface_field` and posts to R05's
  `HeightWorkerPool`.
- `coverage.ts`: `CoverageView` (`resolutionAt(cell)`, `surveyedFraction()`, `bestResolution()`),
  `surveyEdges(face, level)` for the edge mark, and the GPU coverage mask per face. _As built
  (Risks, "Deviations in T10.c, as built"): `surveyEdges(coverage, face)`, the level the
  coverage's own; the mask is `CoverageMask` (`update`, `survey`, `restore`, `release`), and the
  mark `surveyEdge.ts`' `surveyEdgeMark` and `surveyEdgeNote`._
- `handover.ts` (as built, R10.T10.e; Risks, "Deviations in T10.e, as built"): `handoverRangeM`,
  the decision (`handsOver`, and `handOverRegimes` over a frame's discs), `handoverRangeRatio`,
  `handoverReliefM`, `limbDistanceM`, `pixelAngleRad`, `handoverTauPx`, and `storeReliefSource`
  over a `CoarseFieldStore` as a `HandoverReliefSource`; with R07's `photoreal/renderer.ts`
  `FrameTerrain`, the frame's terrain input. R10.T16 ("The live terrain path") consumes them.
- `levelBounds.ts`: `LevelBounds { hard: Float64Array; sigma: Float64Array }` per body, and
  `selectionBound(bounds, level, rule)`. As built, R05's selection reads its bound from the body's
  level table (`PlanetGeometry.levels`, four values a level: bound, lowest and highest height,
  spacing; `planet.ts`'s `levelBoundM`), and `select.ts`'s `selectionErrorM` adds the chord
  sagitta before `screenSpaceErrorPx`; so `selectionBound` fills that table's bound column.
- `classMap.ts`: the per-body coarse class-weights map R07's disc samples.
- `shaders/terrainLit.wgsl` (the file R07 names) with `terrainMaterial.ts`: palette and weights to
  `body_brdf`'s per-class law, the survey mask, skirts at the survey edge, and the hooks R11 fills:
  `terrain_decoration`; `terrain_shadow_factor(p) -> f32`, the direct sun's lit fraction from this
  plan's terrain shadow, into which R11's cloud shadows multiply;
  `terrain_ring_shadow(p, sun) -> vec3f`, 1 by default, which R11 fills for a ringed body with its
  `ring_shadow_on_body(p, sun) -> vec3f` per channel, the type R07's call uses, and which also
  multiplies the direct sun only; and
  `terrain_sky_factor(p) -> vec3f`, 1 by default, a factor on the ground's diffuse sky irradiance
  (R08's `atmosphere_sky_irradiance` where the body has an atmosphere, zero otherwise), into which
  R11 puts a cloud deck's diffuse transmittance (Design note 11). The direct sun on terrain is R08's
  `atmosphere_sun_transmittance` times the three shadow terms.
- `shaders/terrainWire.wgsl` with `terrainWire.ts`: the depth-only pass and its contours;
  `contourInterval(aglM, previous)`, `contourPhase(patchOriginHeightM, intervalM)`.
- `shadows/horizon.wgsl`, `shadows/cascades.ts`: the two shadow paths and their combination.
- `readouts.ts`: `formatWithUncertainty`, `formatElevationReading`, `formatSlopeReading`,
  `formatSlantRange`, and the `SurveyLine` and `TerrainReadout` components in the view's label
  block.
- `cacheBudget.ts`: `bytesPerPatch(options)` and `patchCountModel(camera, tau, viewport)`, the
  pure count model of Design note 15. The budgets themselves are R05's `TerrainSettings.cacheBytes`
  in `view/quality/qualitySetting.ts`'s `SETTINGS` (400,000,000 B high, 64 MiB low, as built),
  the one list of R05's Design note 26, not a second `PATCH_CACHE_BYTES`.

### Test helpers

Two helper modules, one on each side of R04's crate boundary (`hyperion-surface` depends on base
alone and never on the sim, by R04's entry in `.claude/rules/rust-dev.md`):

- `crates/hyperion-surface/tests/common/worlds.rs`: `synthetic_worlds()`, R09's
  `testing::synthetic_field` worlds, fast enough for the goldens on all three targets; and
  `flat_field(height)`, `cliff_field(height, strike)` built with R09's `FieldBuilder` for the
  horizon and classifier tests. As built, `testing` is a feature that the surface crate's own
  integration tests do not see, so their commands pass `--features testing` (the browser recipes
  already do). `FieldBuilder::elevation` sets one value a coarse cell, which R09's approximating
  spline blends over about two cells (R09's "Deviations in T4, as built"), so a `cliff_field`
  steeper than 40° needs a body small enough for two cells to span less than the cliff's run.
- `crates/hyperion-sim/tests/common/surface_worlds.rs`: `sample_worlds()`, R09's reference worlds
  (`planetary::surface::reference::{earth_like, mars_like, moon_like, ceres_like}`) plus an icy
  moon and a locked world built with `CoarseInputs::builder()`, each run through the coarse pass
  once per test binary. The sim depends on the surface crate, so these tests call this plan's
  functions directly. Tests on the reference Earth and Mars are slow, as R09.T16's are; the Moon,
  Ceres and the two built worlds are fast. They run natively; the cross-target goldens use
  `synthetic_worlds()`. As built, `reference::*` is behind the sim's `testing` feature
  (`coarse_pass(seed, &inputs)`), so `cargo test -p hyperion-sim` needs `--features testing`; the
  file is R09.T16's to create, and until R09.T11–T15 fill the coarse pass's steps the reference
  worlds come out as T10's no-op pass leaves them, a smooth dry sphere with no fine relief (R09's
  "Deviations in T10, as built"). The order of tasks says what waits on it.

## Consumes

Names are the owning plans' as they stand on disk; the owner is authoritative, and where a name has
changed by the time this plan runs only the call sites here change. Re-validated at `c6d77cac`
against R05, R07, R08 and R09 as built (Risks, "Re-validated at `c6d77cac`"). R09.T8 has landed
(`c280f604`): `height_at`, `bake_patch`, the level bounds and the closed forms are built, under the
names R09's "Deviations in T8, as built" records. R09.T19.b has landed (`7392c064`): the
`surface_field` answer, its deltas since `have_revision`, and `surface_revisions` on the scene
topic are built, under the names in R09's "Deviations in T19.b, as built" (updated 2026-10-15).
R09.T9 has landed (`070da8ee`), and its goldens pin R09's heights and closed forms on all three
targets at header 21 (below).

- **R01:** the engine adapter, its WGSL-only material path, its adapter feature report
  (`GpuCapabilities` in `view/engine/platform.ts`, with `depthClipControl`, commented as for R10's
  cascades, and `timestampQuery`) and the headless SwiftShader smoke harness, with its no-f16 and
  no-subgroup runs; `MemoryCategory` (`view/engine/memory.ts`), which has neither
  `coarse-field-gpu` nor `shadows` yet (T7 and T9.a add them).
- **R02:** the view's camera, per-patch origin convention and rotation-only view matrix; the
  wireframe style's line tokens, casing and label block (`displays/view/viewRun.ts`'s
  `labelStatements(run, terrain)` and `labelLines`, drawn by `ViewLabelBlock.tsx`); the DOM list
  (`ViewMarkList.tsx`, a windowed `listbox` of `viewRun.ts`'s `MarkRow`); the graticule; the nine
  drafted guide items, of which this plan extends items 1, 3, 5 and 7. Body-fixed vectors reach the
  view as a `ViewPosition` of kind `body_fixed` (`view/coords/position.ts`); the client has no
  `BodyFixedPosition` or `BodyFixedVector`, which are the sim's Rust types.
- **R03:** the binary frames the chunks ride, their backpressure, and the scene topic carrying R09's
  `surface_revisions`: R09.T19.b's optional list of `SurfaceRevisionDto { body, revision }` on the
  scene notification, one entry per surveyed body with its latest revision, merged by body keeping
  the larger (updated 2026-10-15). As built, `packages/protocol/src/requests.ts`'s
  `RequestClient.requestBulk`, taking `(body, manifestOf, options)`, hands over whole answers
  only, each `partial_response` with its chunks through `options.onPartial` and the last through
  `outcome`, so forwarding a block as its frames arrive
  (Design note 16) takes a per-chunk option there, which T7 adds; `MAX_BULK_CHUNKS` (257) and
  `MAX_BULK_PAYLOAD_BYTES` (64 MiB) are in `packages/protocol/src/bulk.ts`. The client's scene
  model is `lib/scene/useScene.ts` and `sceneWire.ts`'s `applySceneNotification`; the fake socket
  is `renderer/src/test/FakeWebSocket.ts` (with `binaryFrames.ts`'s `binaryFrame`).
- **R04:** the `hyperion-surface` crate with its `clippy.toml` and relaxed-SIMD `compile_error!`;
  the base crate's `math`, with R04.T4.c's `math::j0` for Design note 7, `rng` and `units`; both
  wasm targets' fast goldens (`just test-wasm-fast`, which `just ci` runs); the testkit's
  `include_str!` arm (`golden!`, `check_embedded`), `GoldenWriter`, `f32_digest` and `f64_digest`,
  and `order::assert_order_independent(keys, f)`; and R04 Design note 12's conventions for every
  test in `hyperion-surface`: the `wasm_bindgen_test as test` import, a `native_only` module for a
  test that cannot run on `wasm32-unknown-unknown` (one that reads files), and every
  `should_panic` test stating `expected` in the crate's own `tests/panics.rs`.
- **R05:** `cube` and `geometry` (`Face`, `FaceUv`, `PatchKey`, `finest_level`, `MAX_LEVEL`,
  `FINEST_SPACING_M`); `patch::{HeightSource, bake_patch, BakeOptions, PatchBake}`,
  `patch::{VertexPath, NormalScale, finest_surface_height}`, generic over `HeightSource` (R05.T4.a,
  T4.c), which this plan implements for R09's `Synthesiser` (Design note 4); the level bound's role
  in selection (its Design note 15) and `screenSpaceErrorPx`; `selectPatches`, `morphHold`,
  `GroundContact` and the grounded-body rule; `PatchCache` and `resolveDrawSet`; `HeightWorkerPool`,
  whose single `postField(bytes)` this plan replaces with per-body `field_init`, `field_chunk` and
  `field_release` (T7 on the render thread, T6.b in the worker); `normals` packed as `rg16float`
  (its Design note 5) and `PatchCache` built on a `SlotLayout` of this plan's (its Design note 10);
  `terrainPass.ts`; `terrainAnnunciation`; the scripted descent, `descentProfile.ts`, `metrics.ts`
  and the results files under `docs/measurements/descent-spike/`. As built:
  - `HeightSource` (`patch.rs`) is `type Cache`, an `Error` bounded by
    `std::error::Error + Send + Sync + 'static`, `figure()`, `height(cache, dir, level: u8)`,
    `level_bound_m(level: u8)` (documented as "the level's hard error bound"),
    `height_range_m(key: PatchKey)` and a default `prepare_cache(cache, key)`; only `TestPlanet`
    and a test fixture implement it.
  - `PatchBake` also carries `origin_height_m` and `skirt_depth_m` (`level_bound_m` plus the `f32`
    step plus `skirt_m`), and `height_range_m: (f32, f32)`.
  - `collision::finest_surface_height` and `collision::mesh_height(source, dir, level, cache)`
    return the height alone; no query returns the triangle (T3 adds one).
  - Selection reads the bound from `PlanetGeometry.levels` (`planet.ts`, `LEVEL_TABLE_STRIDE` 4:
    bound, lowest and highest height, spacing; `levelBoundM`, `levelHeightRangeM`), and
    `selectionErrorM` adds the chord sagitta; the wasm module's `levelTable(ridges)` fills it for
    the test planet.
  - `slotLayout.ts`: `slotLayout(fields, budgetBytes)`, `terrainSlotFields(terrain)`,
    `terrainSlotLayout(terrain)`, `MIN_SLOTS` (6); `SlotFieldName` is `heights`, `offsets`,
    `normals` and `horizon-map` (0 B until sized), with R10's class weights and survey mask still
    to add. Normals are stored with the atlas's one-texel gutter, 17,956 B a patch at the mesh's
    resolution and 68,644 B doubled (R05's "Deviations in T8, as built").
  - The pool (`workers/pool.ts`) takes `{ workers, createWorker, bake: BakeSettings }`;
    `BakeSettings` and `TerrainPassOptions` carry the test planet's `ridges`; `postField` sends one
    whole buffer, which the worker (`heightBake.ts`'s `answerRequest`, `height.worker.ts`) holds in
    JavaScript; the spike calls it (`view/spike/spikeRun.ts`). The scripted fake worker is in
    `pool.test.ts`.
  - The settings: `view/quality/qualitySetting.ts`'s `TerrainSettings` (`tauPx`, `renderHeightPx`,
    `normals`, `vertexPath`, `cacheBytes`); high is τ 1, double normals, `BakedOffsets`,
    400,000,000 B; low is τ 2 at 720 px, mesh normals, `FaceDifferences`, 64 MiB.
  - `gpu/vertexEmulation.ts` (`faceDifferencePositionF32`, `faceDifferenceMorphF32`) forms the
    vertices in `Math.fround`, for T15.
- **R07:** in `shaders/litBody.wgsl`,
  `struct LunarLambert { a: vec3f, l: f32, s: vec3f, table_row: u32 }` (`table_row`, not `template`, a
  WGSL reserved word: the row of `phase_factor_table` holding the law's f, R07.T4.c as built),
  `body_brdf(law: LunarLambert, mu0, mu, alpha) -> vec3f` (R07.T4.c),
  `sphere_irradiance(h, phi, horizon) -> f32` with its local-horizon argument (R07.T6.a, T6.c) and
  `eclipse_visible`; `DiscSurface` and its `class-map` case
  `{ weights: TextureHandle; laws: ReadonlyArray<PhotometricLaw>; elsewhere: PhotometricLaw }`,
  the type in `appearance/bodyAppearance.ts` and its builders in `bodies/discSurface.ts`, read by
  `bodyDisc.wgsl` (R07.T8.b), as built: the map's layout (a 2D array of `rgba8unorm`,
  N × N texels a face of R05's cube sphere under `uvToSt`, class k in channel k mod 4 of layer f +
  6⌊k ÷ 4⌋, at most 16 classes, `MAX_DISC_CLASSES`), `packClassMap`, `classMapSurface` and
  `classMapTextureSpec`, and
  `LitBodyInput`'s optional `surface` and `rotation`, which the view's `litBodiesOf` passes (T10.d);
  `BodyFigure`, the reference spheroid every height here is
  measured from, along its normal (R07's Design note 19); `BodyAppearance`, `LitRegime`
  (`"point" | "disc" | "mesh"`) and `lawFor(p: Rgb, q: Rgb, template: PhaseTemplateId)`; the star's
  angular radius per view. R07 creates those signatures with default inputs; this plan's T8.b (the
  horizon from the horizon map), T10.b (a `LunarLambert` per class, their `body_brdf` summed per
  texel by weight) and T10.d (the class map) supply the inputs through them unchanged. As built:
  - `phase_factor_table` is an `rgba32float` texture (`bodyDisc.wgsl` binding 1, `DISC_TEXTURES` in
    `bodies/draw.ts`), one row a law of 361 texels from 0° to 180°, f in r, g and b and alpha unused
    (written 0), filled by `appearance/litBodyProbe.ts`'s `packPhaseFactorRows`; `PhotometricLaw`
    (`appearance/law.ts`) has `lommelSeeligerShare` and no curve. The L(α) channel is T10.b's.
  - `sphere_irradiance` clamps `horizon` at 0, and `litBody.wgsl` takes WGSL's built-in
    trigonometry, which stays on main's deferred list (`decision-r08-f32.md` §9).
  - `classMapTextureSpec` files the map under `MemoryCategory` `"other"`; `litBodiesOf`
    (`displays/view/photorealFrame.ts`) sets `rotation` where known and never `surface`, and
    `discSurfaceOf(appearance)` is always uniform. `PhotorealInputs` is built in
    `displays/view/viewFrameDrawer.ts`, which `ViewDisplay.tsx` makes; `photorealFrame` passes
    `depthWriters: []`, "R10's terrain and lit craft add their footprints".
  - The `mesh` regime is `bodies/smoothMesh.ts` with `shaders/smoothMesh.wgsl` and
    `SMOOTH_MESH_MATERIAL` (`bodies/draw.ts`), which already include R05's
    `terrain/shaders/patchVertex.wgsl`; `discEclipseVisible` is in `view/lighting/discEclipse.ts`;
    `pointFlux` and `oblateAlbedoScale(share, cOverA)` in `bodies/draw.ts` and `bodies/oblate.ts`.
- **R08:** aerial perspective applied to the terrain pass's output; R08.T9.b's
  `surfaceLighting.wgsl` with
  `atmosphere_sun_transmittance(altitude_m, mu_sun, latitude_rad, sun_azimuth_rad, curve) -> vec3f`,
  `atmosphere_sky_irradiance(altitude_m, mu_sun, latitude_rad, curve) -> vec3f` and, for
  planetshine, `atmosphere_source_transmittance`, which takes the source's angular radius before
  its `curve`, the terrain's direct and diffuse light where the body has
  an atmosphere, taking the texel's geodetic height and latitude, the sun's azimuth from local north
  (R08 Design note 17) and `curve : u32`, the light's absorber-curve slot (0 for a planetshine
  source; R08's Risks, "`atmosphere_sun_transmittance`'s fifth argument"); and the sun's refracted
  apparent elevation where it draws one (Design note 10).
  Every argument is a height above the datum (the patch's baked height, R05 Design note 5) and a
  cosine against the datum normal, never a radius or |P| − R. This plan's own WGSL follows R08
  Design note 18's rule for angles, through R08's `view/shaders/preciseTrig.wgsl`
  (`decision-r08-f32.md`). As built:
  - R08.T9.b is not built: the two names are R07's stubs in `litBody.wgsl`, four and three
    arguments, returning 1 and 0, which R08.T9.b replaces.
  - R08.T9.a's aerial perspective (`5264a413`): on the low setting (`AERIAL_PERSPECTIVE_SCOPE`
    `"terrain"`, `view/atmosphere/settings.ts`) lit terrain is drawn into an `AtmosphereScene`
    (`{ colour, depth, nearM, exposureScale }`) and
    `HillaireAtmosphere.drawFrame(view, suns, scene)` composites it; on the high (`"scene"`) the
    composite covers every opaque pass drawn
    before it. `aerialPerspective(view)` (null on low) and `aerialRead.wgsl`'s
    `aerialPerspectiveAt` serve only passes the composite does not cover.
  - `preciseTrig.wgsl` has `preciseAtan2(y, x)` for y ≥ 0 (a result in [0, π]), `preciseAtan`,
    `preciseSinCos` and `preciseCos`, and no `asin` or `acos`.
  - Refraction is not drawn: until it is, this plan reads the geometric elevation (R08's Risks).
  - R08.T18's thermal emission, which T10.b calls on terrain (decision-composition), is not built.
- **R09:** `field`'s `FieldView`, `PartialField`, `CoarseField`, `FieldHeader`, `SynthesisCell`,
  `ClimateCell`, `CoarseCrater`, `Cover` and `SYNTHESIS_MARGIN_CELLS`; `wire::decode_payload`;
  `PartialField::{is_surveyed, resolution}`, its `insert(&wire::DecodedBlock)`, and
  `ResolutionCode`; `FieldView::craters_reaching`, an iterator; `synth`'s `Synthesiser`,
  `SynthCache`, `BandLevel` and `level_bound_m`; `unresolved_variance`, `structure_function` and
  `unresolved_rms`, each taking `field: &impl FieldView` and a cell and returning `Option<_>`
  (`None` for a cell the view does not hold), and the level-keyed `Synthesiser::omitted_variance`,
  `omitted_structure_function` and `omitted_rms` (added by R10.T5), what a band level leaves out; `height::HeightSample`, with R05's `BAND_LIMIT_M` and
  `FINEST_SPACING_M`; `FieldHeader.albedo_scale: Option<f64>`, which this plan fills (T10.a);
  `SurfaceClass`; `DetailSeed`; the `surface_field` request with `SurfaceFieldDto` and `CoverDto`,
  whose ranges carry each cell's `ResolutionCode`; `surface_revisions` on the scene topic;
  `SurfaceService` for the albedo scale's call site; `testing::{synthetic_field, FieldBuilder}` and
  the reference worlds (`hyperion_sim::planetary::surface::reference`, in the sim's tests only);
  heights, elevations and sea level along the normal of R07's reference spheroid, with a and c in
  the header, and block 0 of every payload carrying the whole `FieldHeader` (its Design note 17).
  As built (R09's "Deviations … as built" records):
  - _Built._ `PartialField` implements `FieldView`; its `resolution` is `Some(ResolutionCode::NONE)`
    on a margin cell (held, not surveyed) and `None` on a cell not held, and
    `ResolutionCode::resolution()` gives metres (`finer()` orders codes, not `Ord`). The header is
    read through getters (`figure()`, `sea_level()`, `realised_relief()`, `albedo_scale()`,
    `palette()`, `crust_entry(crust)`, `main_liquid()`, `surface_age()`), and
    `CoarseField::with_albedo_scale(Option<f64>)` sets the scale without rebuilding the field. The
    palette is `MaterialPalette` of at most 15 `PaletteEntry`s, whose members are `substance`,
    `role`, `normal_albedo_bvr`, `phase_row`, `grain_density_kg_m3`, `transition` and `mechanics`
    (`PaletteRole`, `MechanicsFamily`),
    every `phase_row` 0 until this plan assigns its codes; `SynthesisCell::substances` packs the
    cell's ice and liquid entries (`ice_entry()`, `liquid_entry()`; the nibble names the species,
    the share its presence). A worker decodes one block at a time with `wire::decode_block`; a
    block's first 10 bytes are its frame (`BLOCK_MAGIC` `HYSF`, the format, its length as a `u32`
    at offset 6, `BLOCK_HEADER_BYTES` 33), so the render thread splits blocks without decoding.
    `Synthesiser::{new, height_at, field, seed, cell_at}`, `SynthCache::new`, `BandLevel::new` (a
    `Result`), `QueryHeightError::NotSurveyed(PatchKey)` alone, `unresolved_variance` with
    `UnresolvedVariance`'s getters `relief()`, `channels()`, `craters()` and `total()`, and the
    per-contribution structure functions `synth::channels::channel_structure_function` and
    `synth::craters::structure_function` (the relief has only its unit tables, `UnitStructure`);
    the slope R09 computes is a body-fixed gradient whose tangential part is the surface gradient,
    so a slope reader projects it first, as `patch::normals::surface_normal` does (R09's
    "Deviations in T4, as built"). `coarse_pass(seed, &inputs)`. On the wire (R09.T19.a):
    `SurfaceFieldRequest { universe, body, have_revision: Option<u32> }` (`null`, not absent),
    `SurfaceFieldDto` tagged by `state`, its `ready` arm holding `header` (a `SurfaceHeaderDto`),
    `revision`, `coverage` (a `CoverDto`) and `bulk`, or `not_modelled { section }`,
    `CoverRangeDto { start, end, resolution }`,
    and `SurfaceHeaderDto`'s scalars with `albedo_scale` (`null` until T10.a) and the palette as
    `PaletteEntryDto { substance, role }` alone (the optical and mechanical values ride block 0).
    The server's call site for the albedo scale is `surface/service.rs`'s `compute`, after
    `coarse_pass`, where the body's record (and so its photometry) is in hand (R09's "Deviations in
    T17, as built").
  - _Landed with R09.T8_ (`c280f604`; R09's "Deviations in T8, as built"; updated 2026-10-15):
    Provides' `synth/mod.rs` is `synth.rs`, with two public modules, `synth::levels` (`LevelSet`,
    `LevelBounds` with `new`, `set`, `bound_m` and `contribution_range_m`, the free
    `level_bound_m`, `shortest_wavelength` and `CRATER_BOUND_TAIL`) and `synth::bake`
    (`PatchHeights` and `PARENT_VERTICES`). `Synthesiser` is `Clone`, not `Copy`, and takes 1–10 ms
    to build. It gains `level_bound_m`, `bake_patch` (which gives `PatchHeights`, the source's
    view, not R05's `PatchBake`), `height_range_m`, `unresolved_variance`, `structure_function` and
    `unresolved_rms`, beside the free `structure_function` and `unresolved_rms`. `height_at` sums
    the base elevation, the octaves, the incision, the small craters and the coarse rims. There is
    no `QueryHeightError::NonFinite`: every part asserts its heights finite. `Synthesiser`
    implements R05's `HeightSource` since R10.T3 (Risks, "Deviations in T3, as built"). Main has
    accepted three as-built changes for T8 (Risks, "Re-validated at `c6d77cac`"): the bound is hard
    in its octaves and channels and a 1 − 10⁻⁹ per-point Chernoff quantile in its crater and rim
    terms; the structure function holds within 5% on belt cells only, channelled plains running
    15–37% high; and the bench is provisional at 14 µs a point and 77–82 ms a patch at L + 4.
  - _Landed with R09.T9_ (`070da8ee`; R09's "Deviations in T9, as built"): the golden heights and
    closed forms in `tests/golden/height/`, pinned on native, `wasm32-wasip1` and
    `wasm32-unknown-unknown` at header 21, which this plan's material and ground goldens follow.
  - _Landed with R09.T19.b_ (`7392c064`; R09's "Deviations in T19.b, as built"; updated
    2026-10-15): `surface_field` answers `ready { header, revision, coverage, bulk }` after the
    payload's binary frames, or `not_modelled { section }` with no bulk. With `have_revision` n,
    the payload is the delta since revision n; block 0 always carries the header, and `coverage`
    holds the delta's own cells alone. A `have_revision` above the body's revision is
    `bad_request` naming `have_revision`. The scene topic carries `surface_revisions`, a list of
    `SurfaceRevisionDto { body, revision }` merged by body, keeping the larger revision. Under
    `RecordInputs`, every generated body still answers `not_modelled`.
  - _Not built, and not R09.T8–T9's:_ the coarse pass's steps (R09.T11–T15) and R09.T16's
    `crates/hyperion-sim/tests/common/surface_worlds.rs`; until they land the reference worlds are
    T10's no-op pass, a smooth dry sphere with no fine relief.
- **R11:** nothing consumed; it fills this plan's `BakeHook` and shader hooks (`terrain_decoration`,
  `terrain_shadow_factor` with its cloud shadows, `terrain_ring_shadow` with R11's
  `ring_shadow_on_body`, and `terrain_sky_factor` with a cloud deck's diffuse transmittance).
- **Galaxy plan 14:** radius and reference surface, ocean fraction, ice fraction, `SurfaceState`
  with `SurfaceMaterial`, surface age, tectonic regime and volcanism level, `body_fixed_at`
  (P14.T14.c), and the rotation section on the wire (P14.T46.f), which the scene body carries as
  `rotation` once R07.T2.b reads it; without it a class map cannot be oriented and the disc shades
  with `elsewhere`, and the visual geometric albedo p that R07 asks of it. As built:
  `frames::body_fixed_at(frame, t) -> FrameRotation`; the rotation on the wire is
  `BodyRotationDto` on `BodySummaryDto.rotation` (built, read by R07.T2.b); the record's
  photometry has `geometric_albedo()` and `phase_integral()` (`Bands`), the wire's
  `BodyPhotometryDto` the albedo and no q; and the record's `Surface` is still an empty enum, so
  this plan reads the surface's figures through R09's field header, never the record.
- **Galaxy plan 12:** nothing directly; R09 gates coverage on P12.T7's store.

## Design notes

1. **The material classifier is `hyperion-surface` code and runs where the height runs** (researched
   2026-09-29). The brainstorm makes the class authoritative, "from slope, altitude, the coarse
   climate field and plan 14's ice fraction, from the same inputs as the height", and R09's Design
   note 18 leaves it to this plan over `FieldView` and `HeightSample`'s gradient. It sits beside the
   synthesis, under the same determinism discipline, and goldens pin it on native and both wasm
   targets. Its order of decision is fixed: `Seabed` below R09's water surface; the coarse pass's
   ice (`SeaIce` over the sea, `WaterIce` or `VolatileFrost` on land, of the cell's ice entry: the
   guide reads them `ICE`, a glacier or ice cap, and `FROST`, so T1 tells them apart by form
   whatever the species, and a nitrogen glacier is `ICE · N2`; signoff-9), since glaciers and ice
   caps stand in cliffs of their own; `Bedrock` above `CLIFF_SLOPE`; `Snow`, giving way to `Bedrock`
   above `SNOW_SHED_SLOPE`; `Melt` where the surface state is a magma ocean or a young volcanic
   province; `Evaporite` in arid basins whose base level the coarse pass filled (playas, among the
   brightest land surfaces); `Sediment` along channels and in the other filled basins; then by
   regime and R09's `SurfaceClass`: `Regolith` on an airless world, `Dust` on an arid thin-aired
   world where R09's wind field deposits, `Sand` and `Soil` by aridity, `OrganicSediment` for a
   Titan-like haze regime, `VolcanicRock` in older provinces. Loose material cannot rest above its
   static angle of repose, so steeper ground is exposed rock: `CLIFF_SLOPE` is 40° with a 5°
   transition band either side, covering the 35–45° of the literature. Kleinhans et al. 2011 (JGR
   116, E11004) measured about 40° for angular grains at 0.1, 0.38 and 1 g, with the static angle
   rising about 5° at low gravity, while Atwood-Stone and McEwen 2013 (GRL 40, 2929) found Martian
   dune lee faces at 30–34°, as on Earth; since the two disagree on gravity, the threshold does not
   scale with it. The Lunar Sourcebook (§9.1.10) gives dumped lunar soil nearly 40° and compacted
   soil about 45°. `SNOW_SHED_SLOPE`, about 50–60°, is from memory; T1.a finds a source or removes
   the rule and records the gap. Nothing is drawn from a stream, so the classifier needs no domain
   tag. _Folded in at the re-validation (decision-composition §1.8; R09's "Cold traps" ruling): each
   rule names its substance through R09's palette, frost or glacier of the cell's ice entry,
   `Seabed` of the floor's entry (the crust's, the bulk's or a `Deposit`'s), never its liquid's
   (signoff-9), `Melt` where R09's cell temperature exceeds the lithology's transition, and
   `Regolith`, `Sand` and `Dust` of the crust's entry or of a `Deposit` or `Cover` entry; and a
   cell's cold-trapped share, which is exposed ice and not the cold-trap area, lies on its shadowed
   ground (crater floors and pole-facing slopes), not evenly over the cell._
2. **Physical properties are data with sources, and bearing capacity is a function** (researched
   2026-09-29). Ultimate bearing capacity is not a constant of the ground: it grows with footing
   width and gravity, q_ult = c N_c ξ_c + ½ ρ g B N_γq ξ_γq (Lunar Sourcebook §9.1.9, after
   Durgunoglu and Mitchell 1975). The Sourcebook gives about 6,000 kPa for a 1 m footing on the
   Moon (§9.1.9, p. 517) and, in Fig. 9.36 (p. 518), a band of 3,000–11,000 kPa at the Apollo 11
   lunar module's footpads, nearly 1 m across, which applied only about 5 kPa (a factor of safety
   of 600–2,200). Those figures hold only for the soil a 1 m footing's failure zone reaches, one to
   two widths down: the near-surface pair (Table 9.12, 0–15 cm: c ≈ 0.52 kPa, φ ≈ 42°; Table 9.4:
   ρ ≈ 1.50 g/cm³) gives about 210 kPa (Vesić factors N_c ≈ 94, N_γ ≈ 156, square-footing shape
   factors about 1.9 and 0.6), while the 30–60 cm pair (c ≈ 3.0 kPa, φ ≈ 54°, ρ ≈ 1.74 g/cm³) gives
   about 5,400 kPa, and its range (c 2.4–3.8 kPa, φ 52–55°) about 3,000–8,000 kPa (researched
   2026-09-29 from Chapter 9 of the Sourcebook, read directly; high confidence in the arithmetic
   and the quotations, medium that the band was built from the deeper pair, which the book does
   not derive). So the table holds Mohr–Coulomb pairs and bulk densities by depth, and
   `bearing_capacity(props, g, width)` takes the layer at the failure zone's depth, about one
   footing width. The figures to settle source by source in T1.a: lunar regolith by Table 9.12's
   depth intervals (0–15, 0–30, 30–60 and 0–60 cm) with Table 9.4's densities; Martian soils from
   Viking (Moore et al. 1987, USGS PP 1389): drift φ ≈ 18°, c ≈ 1.6 kPa; crusty to cloddy φ ≈ 35°, c
   ≈ 1.1 kPa; blocky φ ≈ 31°, c ≈ 5.1 kPa; ice as a strength, 5–25 MPa unconfined at 10⁻³ s⁻¹ and
   rising as the temperature falls (Schulson and Duval 2009), with no Mohr–Coulomb term. Snow varies
   by orders of magnitude with density and stays missing. Pad friction is the interface friction tan
   δ against steel or aluminium: for granular classes from NAVFAC DM-7.02 Table 1 (0.20–0.40 from
   fine sandy silt to clean gravel) and Potyondy 1961's δ/φ ≈ 0.5–0.8, a lower bound once a pad has
   sunk in and passive earth pressure adds to it; for ice and frost a function of temperature, from
   about 0.01–0.05 near 0 °C at speed to 0.3–0.5 at −100 °C and below (Bowden and Hughes 1939;
   Kietzig, Hatzikiriakos and Englezos 2010), stored as two or more points and `None` outside them.
   Metal on bare rock has no primary source found (Byerlee's 0.6–0.85 is rock on rock) and stays
   `None` unless T1.a finds one. A value without a source is left out and the console shows it
   missing, never invented. Most of these figures are from memory in the research and are checked
   against the primaries named before they become code.
3. **Coarser levels bake weights, the finest level bakes the class.** A coarse patch that evaluated
   the class from its own band-limited slope would lose every cliff, since smoothing flattens them,
   and a cliffed range would change colour as the camera closed. So `weights_at` at level n gives
   the expected fraction of each finest-level class over the texel's footprint: for the slope rule,
   the probability that the finest slope exceeds `CLIFF_SLOPE`, from the resolved slope at level n
   and the variance of the slope in the bands finer than n, from R09's per-cell level-keyed closed
   forms (`Synthesiser::omitted_variance` and `omitted_structure_function`, per contribution, what
   level n leaves out of the finest level's ground; added by R10.T5,
   `decision-r10-t5-closed-forms.md`); for the rules on altitude,
   the same with the height variance. At the finest level the weights are the class, blended
   linearly across `TRANSITION_HALF_WIDTH` either side of each threshold for the image only, and
   their largest weight is always the authoritative class, ties broken by discriminant. A test holds
   the weights at every coarser level to within ten percentage points of the fractions counted by
   brute force from the finest level. This is the plan's reading of the brainstorm's rule that the
   shader never re-decides the material: the worker decides everything, and the shader only sums, by
   the weights it was handed, the radiances its classes reflect, each through its own law
   (Design note 8).
4. **Collision reads one function.** The brainstorm defines the authoritative ground as "the
   piecewise-linear surface through the finest level's vertices, on the same triangle diagonal as
   the mesh". R05 builds it, `finest_surface_height`, for its test planet, with the patch bake and
   the bound's role in selection. This plan keeps R05's code and names, points them at R09's
   `Synthesiser`, and wraps the interpolant as `ground_at` (the name R11 consumes), returning the
   triangle and its normal as well as the height. Across a cube-face edge the point belongs to the
   face R05's `xyz_to_face_uv` gives, ties broken as its Design note 2 says. The chord's sagitta at
   0.5 m spacing is L² ÷ 8R, about 5 nm at an Earth's radius, so interpolating in face coordinates
   or in space differs far below the 2 mm target. What this plan adds is goldens on generated
   worlds, the face-edge and corner cases on real fields, and the end-to-end agreement with the mesh
   (T15). R09's Design note 18 also gives this plan the retirement of R05's provisional height
   function from the client path; the spike keeps it for its own reproducibility.
5. **The stated bound is R09's hard bound; selection may take a statistical one** (researched
   2026-09-29). R09 provides `level_bound_m(header, level)`, a hard bound summed over the omitted
   contributions, and R05's Design note 15 makes a hard bound the selection contract with an
   explicit calibration factor as its fallback if it proves loose. Real terrain is where looseness
   shows. For summed octaves of gain g the hard tail is about B a_(n+1) ÷ (1 − g) and its RMS about
   σ_b a_(n+1) ÷ √(1 − g²), so with noise normalised to B ≈ 1 and σ_b ≈ 0.2–0.3 the observed 99.9th
   percentile over the hard bound is expected at about 0.3–0.5, lower with craters, whose bound is
   the largest omitted crater's depth that few points see. Patch count scales as the inverse square
   of that ratio, four to eleven times a perfect bound. The reference renderers bound it the same
   way: chunked level of detail (Ulrich 2002) stores a hard maximum vertical error per chunk and
   refines while ρ = δ K ÷ D > τ; CDLOD (Strugar 2009) and Proland are distance-based; CesiumJS's
   geometric error for heightmap terrain is a horizontal-spacing heuristic,
   `getEstimatedLevelZeroGeometricErrorForAHeightmap`, with `maximumScreenSpaceError` defaulting to
   2 px (verified in `TerrainProvider.js` and `Globe.js`). So the hard bound stays the stated and
   tested bound, which is what a sensor's stated error means. The candidate calibration, for the
   owner's ruling with T4.a's numbers, is that selection takes the smaller of the hard bound and 4σ
   of the omitted bands, σ from R09's level-keyed closed forms (`Synthesiser::omitted_rms` at the
   level, added by R10.T5). Over a patch's 4,225 vertices, taken as
   independent Gaussian deviations, the 1 − 1 ÷ 4,225 quantile of |x| is Φ⁻¹(1 − 1 ÷ 8,450) ≈
   3.68σ and the expected maximum of |x| about 3.81σ (median 3.77σ), so 4σ is a thin margin: one
   patch in four or so has a vertex past it, P(max |x| > 4σ) = 1 − (2Φ(4) − 1)^4,225 ≈ 0.23, and
   a bound exceeded in one patch in a thousand would need about 5.2σ (researched 2026-09-29; the
   quantiles computed, high confidence; real deviations are correlated and skewed, which T4.a's
   recorded maxima test). It is
   chunked level of detail's "maximum error within the chunk" taken statistically, it saves two to
   three times the patches, and what it lets through is a sub-τ pop that morphing hides; collision,
   which reads the finest level, and the readouts, which state 1σ, are untouched. Until the ruling,
   selection uses the hard bound. A per-patch bound, each bake computing its children's bounds from
   the amplitudes over their footprints as chunked level of detail stores δ per chunk, is adopted
   only if T4.a shows body-wide and per-patch bounds differing by more than 1.5 times. _As accepted
   by main for R09.T8, and as built (`c280f604`; R09's "Deviations in T8, as built"; updated
   2026-10-15): `level_bound_m` is hard in its octaves and channels, and its crater and rim terms
   are a 1 − 10⁻⁹ per-point Chernoff quantile, so "the hard bound" here reads as that stated bound;
   the finding is in Risks ("Re-validated at `c6d77cac`")._
6. **Knowledge gates coverage for the image, and coverage and resolution for every number**
   (researched 2026-09-29). The client holds exact cells wherever it holds any, so the image of
   surveyed ground is the ground at whatever detail the camera resolves; the survey's resolution
   never changes a drawn height. What the resolution changes is what a readout may quote.
   `surveyed_reading` evaluates the height at the band level matching the survey resolution of the
   point's cell and states as its uncertainty R09's `omitted_rms` at that level, one standard
   deviation of every contribution the level leaves out of the finest level's ground, each at the
   level's own cut, not `unresolved_rms` at `λ_n` (R10.T5, `decision-r10-t5-closed-forms.md`). It is 1σ and not an interval such as LE90 (1.6449σ, a
   Gaussian figure), because the unresolved heights are not Gaussian (craters and channels are
   skewed and heavy-tailed), because 1σ is the only figure the closed forms give, and because
   current practice states RMS: HiRISE's expected vertical precision (Kirk et al. 2008), MOLA and
   LOLA shot precision, and the ASPRS positional accuracy standards' second edition (2023). LE90
   survives mainly in military and legacy terrestrial products (DTED; SRTM's 16 m absolute
   specification). The value is rounded to its uncertainty by the Particle Data Group's rule: take
   the uncertainty's three leading digits; 100–354 keep two significant figures, 355–949 one,
   950–999 round up to 1000 and keep two; the value takes the same decimal place. So
   `~2140 m ± 180 m` and `~2100 m ± 500 m`. Neither number goes finer than the quantity's own
   precision; where the rule would, the uncertainty is rounded up to one step (the guide's Estimated
   exception, signoff-9). When the survey reaches the band limit the uncertainty is zero, the value
   is the ground itself at the ship-wide elevation precision of 0.1 m, and both the `~` and the
   `± …` go. Unsurveyed ground has no reading: `NOT SURVEYED` stands once in place of the readings
   (signoff-9).
7. **Slope and slant range state their basis** (researched 2026-09-29). The slope is the
   adirectional slope, the gradient magnitude from central differences over a baseline b along the
   local east and north axes; a two-point profile slope is smaller by about √2 on isotropic terrain
   (Shepard et al. 2001, as Beyer, McEwen and Kirk 2003, JGR 108, 8085, ¶25, cite it; corrected
   2026-10-14), and the adirectional one is what tilts a lander. Each unresolved band
   of variance σ_k² at wavenumber k adds per axis v = Σ 2 σ_k² (1 − J₀(k b)) ÷ b², the structure
   function at lag b (Shepard et al. 2001), which tends to 2σ_k² ÷ b² for bands much finer than b
   and to the band's gradient variance for bands much coarser; craters and channels take R09's
   structure function at lag b, not only a variance: the level-keyed one, of what the reading's
   level leaves out (`Synthesiser::omitted_structure_function`, R10.T5). The slope magnitude is then Rician
   about the resolved slope |g₀|, biased upward when |g₀| is below about 2√v, so the readout quotes
   the RMS slope √(|g₀|² + 2v) with the Rice standard deviation as its uncertainty; quoting |g₀|
   alone would understate slopes from orbit, the unsafe direction. The baseline is shown with the
   value: 2 m by default, the band limit, and the scale a lander's tilt is judged over
   (signoff-10), with 5 m and 100 m selectable. The first two are the lander-stability baselines of
   InSight's site selection, slopes at 2–5 m from HiRISE DEMs against a 15° limit (Golombek et al.
   2014, LPSC 45, 1499, verified), and the third stands near the same limit's 84 m scale for the
   landing radar (Golombek et al. 2017, Space Sci. Rev. 211, 5, §2.7); the 2014 abstract also reads
   slopes at about 300 m from MOLA (corrected 2026-10-14 to `SLOPE_BASELINES`' citations in
   `reading.rs`; "Deviations in T5, as built").
   The slant range to the ground on the view's line of sight is a march of the ray against the
   surface at the survey's resolution. Linearising the hit of p(t) = o + t d on z = h + δ gives dt ÷
   dδ = cos θ_s ÷ sin γ_s, with θ_s the resolved surface's slope and γ_s the grazing angle to the
   local surface plane, so σ_R = σ_h cos θ_s ÷ sin γ_s. The linearisation fails once unresolved
   relief can occlude the hit, so when sin γ_s < 2√v at b equal to the survey's cell size the
   readout drops its σ and says `GRAZING`: the true range is then biased short and its error
   one-sided. Consoles that draw no terrain would need a server request for the same numbers; none
   exists yet, and when one does it wraps these functions unchanged.
8. **One photometric law for every class, and the disc and terrain share it** (researched
   2026-09-29). Every class takes McEwen's lunar-Lambert law, r(i, e, α) = A_N f(α) [2 L(α) μ₀ ÷
   (μ₀ + μ) + (1 − L(α)) μ₀], with f(0) = 1, μ₀ = cos i, μ = cos e and A_N the normal albedo; L = 1
   is Lommel–Seeliger and L = 0 Lambert. For the Moon L(α) = 1 − 0.019 α + 2.42 × 10⁻⁴ α² − 1.46 ×
   10⁻⁶ α³ in degrees (McEwen 1996, LPSC XXVII, 841, Table 1, the same at 0.56 and 0.76 µm, fitted
   to Galileo SSI images at 19.5°–101° with L(0) = 1 imposed; the coefficients are USGS ISIS's
   `LunarLambertMcEwen.cpp`; McEwen 1991, Icarus 92, 298, is the earlier study of L(α) from Hapke's
   model). It reaches 0 at 103.9° and L is held at 0 beyond, where the fit has no data. McEwen 1996
   finds no difference in limb darkening between maria, highlands and bright Copernican craters, so
   across regolith albedos L(α) does not follow A_N, and only the brightest surfaces are
   limb-darkened at zero phase (Buratti 1984, Icarus 59, 392; decision-phase-curves; Buratti and
   Veverka 1983, Icarus 55, 93, propose an albedo-dependent form, which McEwen 1996 rejects for the
   Moon). The dark and moderate classes take the Moon's L(α); the bright ices' and evaporites' L(α)
   is settled in T1.a against the primaries and the A_h ≤ 1 bound below. Hapke's full model is not
   used per texel: its roughness term counts relief the mesh, normals and horizon map already
   resolve, and what it adds lies below the band limit. It is the law R07 adopts (its
   Design note 5), so R07's `body_brdf(law: LunarLambert, mu0, mu, alpha)` carries it and this plan
   calls it once per class, with that class's `LunarLambert` (T10.b). Each law's L(α) is tabulated
   beside its f, in the alpha channel of its `phase_factor_table` row, and f against that L(α):
   Φ_shape(α; L) = [L(α) Φ_LS(α) + ⅔(1 − L(α)) Φ_Lam(α)] ÷ [L(0) + ⅔(1 − L(0))], so p is unchanged,
   and inside each template's range Φ and q are too and only the limb profile moves; past it the
   held f draws L(α)'s own tail, which lowers q by at most 6.8 × 10⁻⁴ (R10.T10.b's record);
   `body_brdf` reads L from the row and `LunarLambert.l`
   is L(0) (decision-r07-t8b). A texel reflects its classes' `body_brdf` radiances summed by weight,
   each law through its own row, as R07's disc sums its class map's, not one law built from weighted
   parameters, which differs from the sum wherever classes of different A_N and L blend (1.7× at
   μ₀ = 0.5, μ = 0.1 for an even blend of regolith at L = 1 and snow at L = 0). At zero phase a
   uniform body's geometric albedo is p = A_N [L(0) + ⅔ (1 − L(0))], and averaged over viewing
   directions a patterned sphere's p is the plain area average of each element's, so the scale c on
   the class albedos that makes the body's p is the root of Σ a_i c A_N,i [L_i + ⅔ (1 − L_i)] = p
   over the classes' area fractions a_i. Clamping to the classes' ranges makes it piecewise linear
   in c, solved exactly between sorted breakpoints. The server holds the whole field, so
   `albedo_scale` runs there, on every cell, and c ships with the field in R09's
   `FieldHeader.albedo_scale`: the terrain's brightness then never shifts as the survey grows, and
   one scalar reveals nothing the disc's brightness does not. Matching p at zero phase is not enough
   for a hand-over that changes geometry only, since the laws differ at other phases and the pattern
   differs everywhere (an Iapetus, a Mars albedo map). So R07's disc samples a per-body coarse
   class-weights map, through the `class-map` case of its `DiscSurface` (R07.T8.b; filled by T10.d),
   T2's weights at level 4 or shallower, a few hundred kilobytes, through the same law, and disc and
   terrain agree at every phase by construction. The map covers surveyed cells only; unsurveyed
   texels take R07's uniform `lawFor(p, q, template)`, so the disc shows no pattern the ship has not
   seen. Plan 14's phase integral q then becomes a result of the classes, reported with its residual
   as the albedo's is. A_N is the normal albedo, the radiance factor at i = e = α = 0 (Hapke 2012,
   §10), in V; for a Lambert surface it equals the hemispherical albedo, and the law gives r = A_N
   there. Terrestrial snow and ice albedos are broadband (0.3–2.5 µm) hemispherical figures, which
   understate the visible because ice absorbs in the near infrared, so they are converted before
   use: the spectral albedo near 0.55 µm, at normal incidence, times the nadir anisotropy R(0, 0) ÷
   A_h (about 0.9–1.0 for snow, low confidence), and since bright classes have L near 0, A_N ≈ A_h
   after that (researched 2026-09-29: Warren 1982; Wiscombe and Warren 1980; Grenfell et al. 1994;
   Warren et al. 1998; Hudson et al. 2006). Each class's A_N and L together must reflect no more
   than they receive: the Lommel–Seeliger term's directional-hemispherical albedo is 4 A_N [1 − μ₀
   ln(1 + 1 ÷ μ₀)], 1.23 A_N at μ₀ = 1, so a bright A_N with a sizeable L breaks A_h ≤ 1, and T1.a
   tests it. Starting parameters per class, all settled in T1.a against the primaries (most from
   memory in the research): regolith A_N 0.07–0.20 with the lunar L(α); basaltic rock 0.05–0.15,
   felsic 0.2–0.35; dry soil and sediment 0.10–0.30, and wet ground about 0.5–0.65 times its dry
   albedo, the ratio rising with the dry albedo, by total internal reflection in the water film
   (Ångström 1925; Lekner and Dorf 1988, Appl. Opt. 27, 1278; Twomey et al. 1986), though the
   classifier has no wet state until R09's climate gives one; desert sand 0.25–0.45, Martian dune
   sand 0.10–0.15; Martian bright dust 0.18–0.25 in V (its broadband 0.25–0.32 is redder: Kieffer et
   al. 1977; Christensen et al. 2001; Mustard and Bell 1994; Bell et al. 2000); snow 0.95–0.98 fresh
   and 0.85–0.93 aged in the visible (Wiscombe and Warren 1980; Grenfell et al. 1994), against
   Warren 1982's broadband 0.80–0.95 and 0.5–0.7; clean glacier ice about 0.5–0.65 visible (Cuffey
   and Paterson 2010, Table 5.2, gives 0.34–0.51 broadband); sea ice about 0.75–0.85 bare and
   0.93–0.97 snow-covered in the visible (Perovich et al. 2002's broadband 0.5–0.7 and 0.80–0.87);
   nitrogen and methane frost 0.8–1.0 (Buratti et al. 2017, already normal reflectances in LORRI's
   band); carbon dioxide frost 0.4–0.8; organic sediment 0.05–0.2; evaporite 0.5–0.8. A range is the
   class's physical span; the class's value before scaling is its midpoint, which the scale c
   multiplies and the range then clamps. `Melt` is emissive, which is R07's or R11's, and `Seabed`
   lies under the ocean pass. _Folded in at the re-validation (decision-composition §1.8 and §1.9):
   a texel's A_N in B, V and R and its phase row are its material's palette entry's, which the
   server resolves (R09's `PaletteEntry::normal_albedo_bvr` and `phase_row`, every row 0 until T1.a
   assigns the codes), in place of the per-class figures above, which stay the classes' physical
   spans and T1.a's check on the entries; the ground's thermal emission, `Melt`'s and any cell's
   above R08.T18's visibility cut, is R08.T18's shared function, called by T10.b. How a form
   changes its entry's A_N (fresh snow and glacier ice of one H₂O entry) was a question in Risks,
   answered by a per-form factor in T1.a's table ("Deviations in T1.a, as built")._
   _Folded in from decision-r10-limb (2026-10-13; built under T1.a): "bright classes have L near
   0" becomes two kinds of form, and L(0) is a measured property of how the surface is built, not
   an energy bound's. The consolidated forms (`Glacier`, `Snow`, `SeaIce` and `Evaporite`, of any
   species or salt) take the Lambert law, `PhaseLaw::Lambert` (R07's `lambert` template: f ≡ 1,
   L ≡ 0, q = 3/2), which the form chooses whatever the entry's code; their measured spectral
   hemispherical albedos are their A_N with no conversion, since the law's A_h is A_N at every
   incidence, and the nadir anisotropy above is the law's error. The particulate forms (every
   other form, `Frost` of any species included) keep the entry's lunar-Lambert row, with L(0) =
   L_p(c A_N): 1 up to A_N 0.60 (Buratti 1984, as Buratti et al. 2017, p. 12, reports it), falling
   as 0.6696 ÷ x − 0.1160 to 0.38 at Enceladus's 1.35 (Verbiscer and Veverka 1994's Hapke
   parameters), and 0.38 beyond. The A_h ≤ 1 bound above is the f ≡ 1 case, exact only for the
   Lambert law. The bound is on the whole law with its phase factor: for every μ₀ in (0, 1],
   A_h(μ₀) = (A_N ÷ π) ∫ f(α) [2 L(α) ÷ (μ₀ + μ) + 1 − L(α)] μ dΩ ≤ 1. Energy is enforced on A_N,
   never through L, by each law's ceiling (`PhaseLaw::normal_albedo_ceiling`): 1 for Lambert's,
   exact, and 1.35 for the particulate rows, against which the Ice row's A_N × sup η is 0.97 under
   the rule. A normal albedo above 1 is physical (Enceladus's is 0.9–1.4). See "Deviations in
   T1.a, as built"._
9. **No textures in this plan.** The Materials section lists triplanar projection of rock and detail
   textures and two-scale repetition hiding. Anything a texture adds below the band limit is detail
   the simulation did not compute, which the brainstorm defines as decoration, and R11 owns it,
   zero-mean about this plan's class albedo, through the `terrain_decoration` hook of
   `terrainLit.wgsl`. This plan's lit terrain is shaded from baked data alone: the height, the
   normals at twice the mesh resolution on the high setting (R05's `NormalScale`), the class weights
   and their photometric parameters. It is the undecorated ground R11's decoration switch turns off
   to.
10. **Horizon maps are sun-independent, linear 16-bit, and their far field is a shared lattice**
    (researched 2026-09-29). A horizon map stores, per texel and azimuth, the horizon's elevation
    (Max 1988; Sloan and Cohen 2000), and the test compares the sun's elevation in that azimuth
    against it; nothing depends on the light, so a patch never rebakes for the sun or the body's
    rotation (the brainstorm's "needs rebaking only as the sun moves" is reported in the notes).
    Eight azimuths, interpolated linearly in azimuth; sixteen would double the memory, and the
    by-hand sunset decides whether narrow distant peaks need them. The encoding is the elevation
    itself, linear, as a signed normalised 16-bit value s = e ÷ 90° over [−90°, 90°], two azimuths
    to a `u32` word through WGSL's core `pack2x16snorm` and `unpack2x16snorm`, four words (16 bytes)
    a texel in the slot's storage buffer (Design note 15), and filtered bilinearly by hand from four
    reads (researched 2026-09-29, on IEEE 754 binary16 and the WGSL and WebGPU specifications). Its
    step is 180° ÷ 65,534, 9.9″, and its worst round-trip error 4.95″ at every elevation, signed for
    horizons below zero on peaks. The alternatives fail: eight bits of sin(e) step 0.45° against the
    Sun's 0.53° penumbra at 1 au, so sunsets would band; and any half-float encoding keeps a
    relative precision of 2⁻¹¹, so sin(e) in `rgba16float` steps 0.032° at 30°, 0.056° at 60° and
    0.16° at 80°, and passes half the Sun's radius above about 78°. `rgba16snorm` would filter in
    hardware but needs the optional `texture-formats-tier1`. The rule stated with the encoding
    bounds the image, not the relief: the lit fraction changes fastest at mid-transit, dF/de = 2 ÷
    (πρ), so a quantisation error δ_q moves it by at most 2 δ_q ÷ (πρ), 0.33% for the Sun at 1 au
    (under 1/255). A star is drawn with a soft penumbra only when ρ is at least 20″, about two steps
    (at 32″, a Sun at 30 au, the error is at most 10%, over a band a few centimetres wide); smaller
    stars take a hard step. The terrain's own sampling and the eight azimuths' interpolation err far
    more in rough ground; the rule governs banding only. The near field is marched from the patch's
    own 65 × 65 heights, about 2.2 M compares, an estimated 2–6 ms in WebAssembly (provisional; T8.a
    measures it), or by Timonen and Westerholm 2010's convex-hull line sweep, O(n) per direction, if
    the march breaks the budget. The far field is not rays through the height function, which at 81
    points × 8 azimuths × about 45 steps is some 29,000 evaluations, an estimated 60–120 ms: each
    far sample is the bilinear interpolation of the level-m vertex heights, m chosen so the vertex
    spacing is about the step length times the growth factor less one, memoised by (face, level, i,
    j) in the worker's `SynthCache`. The far-field points lie on a face-wide lattice at a patch
    edge's spacing ÷ 8, defined by position alone and not by the patch's level, so points on a
    shared edge are the same point, and between patches of one level on one face the same value, so
    that the far field opens no seam there. Across a level change the finer map differs by its
    receiver's height and its finer near ground, as the near field does, and the shader blends a
    patch's map to its parent's by the morph factor, as the geometry is blended (T8.b). Across a
    face edge the azimuths turn with each face's grid, and the two sides differ by the eight
    azimuths' interpolation (main's ruling for R10.T8.a, 2026-10-15). Steps grow by 1.15–1.25
    from one texel beyond the patch edge, the curvature drop d² ÷ 2R is included, so an obstacle at
    d stands at arctan((h − h₀ − d² ÷ 2R) ÷ d), and the march stops at √(2 R Δh), Δh the body's
    realised relief range: about 505 km for an Earth with 20 km of relief. The shader takes the
    larger of near and far. The lit fraction is the share of the star's disc of angular radius ρ
    above the horizon, 1 − (arccos x − x √(1 − x²)) ÷ π with x = (e_sun − e_horizon) ÷ ρ clamped to
    [−1, 1] (checked: 0, ½ and 1 at x = −1, 0, 1), without limb darkening, which moves the
    half-light point by under 0.1ρ. On terrain R07's horizon term (`sphere_irradiance`, its Design
    note 6) takes its local horizon per disc sample from this map, through the `horizon` argument of
    R07's `sphere_irradiance(h, phi, horizon)` (R07.T6.a, T6.c; 0 on the smooth figure), carried
    into the element's frame as e − (e★ − (π ÷ 2 − φ)), so that R07's cut, whose visible share is
    this lit fraction, lies at x = (e★ − e) ÷ ρ (decision-r10-t8b §1), with R07's oracle as its test
    oracle; T8.b fills it. Where R08 draws the sun at its refracted elevation (on an Earth about
    0.57° at the horizon, more than the Sun's disc, and 0.16° at 5°; Bennett 1982), the shadow test
    uses the same apparent elevation, or shadows and the drawn sunset disagree by more than a solar
    diameter (as built R08 draws no refraction, so the test takes the geometric elevation until it
    does, R08's Risks). Max, Sloan and Cohen, Stewart 1998, Timonen and Westerholm and Bennett are
    from memory in the research; T8.a's bench and the by-hand sunset settle what they support.
11. **Terrain shadows are this plan's on both settings, and combine by minimum** (researched
    2026-09-29). The brainstorm's ladder gives the high setting cascaded shadows "with cloud shadows
    on terrain" and the low one the horizon map, and its budget's third rule builds the low setting
    beside the high. No plan in the set names the cascades, and terrain self-shadowing is this
    plan's feature, so the reading closest to step 8 is that both are built here; R11's cloud
    shadows multiply into the result through `terrain_shadow_factor` and a ringed body's ring shadow
    through `terrain_ring_shadow`, both on the direct sun only; the diffuse light a cloud deck
    passes reaches the ground as `terrain_sky_factor` on R08's `atmosphere_sky_irradiance`, so an
    overcast sky does not draw a sharp half-bright sun (R11's Design note on cloud shadows). The
    horizon map is applied everywhere on both settings; on the high one the lit fraction is
    min(horizon, cascade) where a cascade exists: the horizon's share m is R07's visible share at
    the cut it used, `disc_visible_fraction(h, φ + max(η, 0))`, and the cascade's c enters
    `terrain_shadow_factor` as min(1, c ÷ m), 1 where m = 0 (decision-r10-t8b §2), so cascades add
    what the horizon map cannot, casters that are not terrain (craft, R11's rocks and scatter) and
    sub-texel detail near the camera, and there is no seam at the last cascade, which only fades to
    lit over its last 10%. Cascades switch off when cascade 1's texel is coarser than the horizon
    map's, as in orbit. Splits follow Zhang et al. 2006's practical scheme, Cᵢ = λ n (f ÷ n)^(i ÷
    N) + (1 − λ)(n + (f − n) i ÷ N) with N = 4 and λ ≈ 0.75, over a shadowed range f = min(view far,
    max(2 km, 20 × height above ground)), continuous in that height. Each cascade is fitted to a
    bounding sphere of its frustum slice, whose radius does not change as the camera turns, snapped
    to whole texels (Valient 2008), with every light matrix built camera-relative in `f64` and
    narrowed to `f32`, since snapping at planetary coordinates would jitter. An orthographic light
    projection has linear depth, so reversed-Z buys nothing there: the maps are `depth32float`,
    whose worst step over a 50 km range is about 3 mm, and keep the project's greater-equal
    convention for uniformity only, which the code says so that no one tunes bias expecting
    precision. Casters in front of the near plane are pancaked through `primitive.unclippedDepth`
    where the adapter offers `depth-clip-control` (to be confirmed on the UHD 620 through R01's
    feature report), and otherwise the near plane is pulled towards the sun by Δh ÷ sin(e_sun),
    capped. Acne is held by slope-scaled bias (`depthBias`, `depthBiasSlopeScale`, `depthBiasClamp`)
    and a normal offset of 1–1.5 texels of the cascade's world-space size; filtering is 2 × 2
    hardware PCF through `textureSampleCompare` with a few Poisson taps, since the Sun's penumbra at
    10 m from an occluder is about 9 cm and PCSS is unneeded. Four 2,048² `depth32float` layers are
    64 MiB, the brainstorm's figure. Zhang et al.'s formula, Valient's snapping and the bias recipes
    are from memory in the research and are checked against the sources in T9.a.
12. **Unsurveyed ground is the reference surface, and its edge is marked.** A patch's vertices whose
    coarse cell is not surveyed (R09's margin cells are held but not surveyed) carry a survey mask
    of zero, and the terrain shader discards fragments whose interpolated mask is below one half.
    Beneath them R07's `mesh`-regime body, the reference spheroid of its `BodyFigure`, is drawn with
    its own fragments discarded where the GPU's coverage mask says surveyed, so no terrain is
    claimed there and no reference surface shows through a surveyed valley. Terrain heights are
    measured along that spheroid's normal (R07's Design note 19; R09's Design note 17), so the two
    surfaces share one datum and the skirt spans only the terrain's own relief at the edge. Where
    terrain and reference surface meet at different heights, a skirt along the survey edge joins
    them, drawn in the reference surface's appearance, so the edge reads as an edge rather than as a
    cliff in the ground. The edge itself is symbology: a mark over the image, cased under guide item
    3, in both styles. The coverage mask is the brainstorm's "coarse field, GPU copy", one byte per
    coarse cell per face, at most 384 KiB at level 8.
13. **The disc hands over to terrain by the body's own relief.** R07 draws a body in its `disc`
    regime until R09's `realised_relief` subtends τ pixels; nearer, the `mesh` regime hands its
    geometry to R05's quadtree, from the root faces down, with τ the style's own: 1 px on the high
    setting, 2 px on the low and 4 px in the wireframe. At an Earth's 10 km of relief either side of
    the datum, over 1080p across 60° (5.45 × 10⁻⁴ rad a pixel), that is about 1.8 × 10⁷ m at 1 px
    and 4.6 × 10⁶ m in the wireframe. The first lies near the brainstorm's 2 × 10⁷ m (Two styles of
    one renderer), but that figure is a different one, where a 35 km coarse cell spans 4 px, and
    agrees only by coincidence. The resolved slopes and shadows at that level are negligible, which
    is why the same smooth law holds on both sides of it (Design note 8). A body with no survey
    never hands over.
14. **The wireframe's contours are chosen as a scale is** (researched 2026-09-29). The wireframe
    draws its terrain once, depth only, at 4 px, and writes colour only where the interpolated
    height crosses a contour level. The interval is one deterministic rule of pose alone, so every
    client with the same camera states the same one: the 1-2-5 step nearest, in log terms, to a
    twentieth of the camera's height above the surveyed ground at its nadir (above the datum where
    the nadir is unsurveyed), never below 1 m. The nadir, not the view's centre, so the interval
    does not jump as the camera tilts towards the sky. A twentieth keeps steep ground legible: at a
    30° look-down the view's centre is at about twice the height, where a 1:1 slope's contours come
    about 20 px apart, and a fiftieth would crowd them to 8 px. It steps up when a twentieth of the
    height passes the geometric midpoint to the next step by 20%, and down 20% below it. Every fifth
    contour is an index contour, as on USGS maps; sea level on an ocean world is the coastline,
    drawn at R09's header `sea_level` whether or not it is a multiple of the interval. Widths, since
    colour alone may not carry meaning: minor 1 px and index 1.5 px in `--text-muted`, coastline 2
    px in `--text`; never dashed, since the guide reserves dashes for predictions; no contour labels
    in the image, since the readout's `ELEVATION` and the stated datum carry the values. The line is
    antialiased at constant width from u = (h_local + φ) ÷ CI: dist_px = |fract(u − 0.5) − 0.5| ÷
    length((dpdxFine(u), dpdyFine(u))), coverage = clamp(w ÷ 2 + 0.5 − dist_px, 0, 1), the
    gradient's length rather than `fwidth`, whose L1 norm varies widths by up to √2 with direction
    (after Golus 2021). The derivatives are taken in uniform control flow, before the survey mask's
    `discard`. A family is drawn in full at 6 px of spacing or more and fades out as its spacing
    closes to 3 px (CSS pixels), minor contours first, and is not scored for contrast in its fade;
    the guide says that surveyed ground without contours lies within one interval or is too steep
    for them, so that an absent line is not read as flat ground; this is the grazing case, where
    spacing collapses faster than any interval can follow. The height varying is patch-local, h −
    h_patch, with a per-patch phase φ = h_patch mod CI formed in `f64` on the CPU, because the `f32`
    height above the reference spheroid steps 2 mm at ±20 km and would quantise the derivative near
    the camera. The label block states `CONTOUR INTERVAL 200 m · INDEX 1 km`, the USGS margin form,
    with the `DATUM` on a line of its own in both styles (T12), and guide item 5's substitute for a
    scale bar, scaling to km with hysteresis. Unsurveyed ground has no contours; R02's graticule is
    drawn over the terrain's height where terrain is surveyed. The datum is the body's rotational
    spheroid, R07's reference body, from which every height in the patch is measured along its
    normal (R07's Design note 19, researched there with high confidence; R09's Design note 17
    measures its elevations and sea level the same way), stated as
    `DATUM REFERENCE SPHEROID 6378.1 km × 6356.8 km` with the equatorial and polar radii (a sphere,
    `DATUM REFERENCE SPHERE 6371.0 km`, until plan 14 sends a flattening); the sea level, a height
    above the spheroid, is stated beside it on an ocean world. The h ÷ 20 figure has no published
    precedent for perspective views and is settled by T11's check on steep and gentle terrain; the
    index and margin conventions are from memory in the research.
15. **The cache is sized from a count model and then measured** (researched 2026-09-29). A pure
    count model (a quadtree on an Earth-sized cube sphere, finest level 19 at R05's 17.7 m mean
    patch edge; a patch accepted at a distance of k times its level's edge, k = 5 at 1 px 1080p and
    60°, 1.67 at the low setting's 2 px at 720p, 1.25 for the wireframe's 4 px; exact horizon
    culling; frustum 60° × 34°) gives, all-round over a full turn and in the frustum:

    | Camera height | High, 1 px 1080p | Low, 2 px 720p | Wireframe, 4 px 1080p |
    | ------------- | ---------------- | -------------- | --------------------- |
    | 100 m         | 1,468 / 246–268  | 332 / 80–88    | 284 / 76              |
    | 1 km          | 1,216 / 200–210  | 260 / 64–68    | 212 / 52–56           |
    | 10 km         | 936 / 144–164    | 208 / 50–54    | 148 / 38–42           |
    | 100 km        | 604 / 92–116     | 152 / 38–42    | 104 / 26–30           |
    | 400 km        | 424 / 78–92      | 116 / 30–32    | 68 / 18–20            |

    A full level ring holds about 180 patches at k = 5. Quadtree granularity floors a ring at about
    36 patches once k falls below about 2, so the low setting selects about a quarter of the high
    one's patches, not a ninth, and the wireframe about a fifth, not a sixteenth (the brainstorm's
    figures, which hold for demand only while k exceeds about 3). Relief, the 2:1 neighbour rule and
    the forced region under grounded bodies are not modelled and add perhaps 20–40%. A patch uploads
    R05's heights and parent heights, 33.8 kB (65 × 65 × two `f32`, read from the slot's storage
    buffer, so `float32-filterable` is not needed); normals, 16.9 kB at mesh resolution or 66.6 kB
    at twice it (`rg16float` octahedral, core and filterable, angular error about 0.05°); class
    weights, 33.8 kB (two `rgba8unorm`); the horizon map, 67.6 kB (four `u32` words a texel, Design
    note 10); and the survey mask, 4.2 kB (one byte a vertex, four to a `u32`): about 156 kB on the
    low setting and 206 kB on the high. R05's `BakedOffsets` vertex path adds about 101 kB a patch
    while it stays R05's default. The sizing rule is max(1.3 × the all-round peak, 3 × the frustum
    peak), summed over the views that stream, plus the forced region. On the low setting that is
    about 430 slots, which 64 MiB holds at 156 kB with nothing to spare, and not with
    `BakedOffsets`; on the high setting 3 × the frustum peak is about 800 slots, 165 MB, but the
    rule's other term governs: 1.3 × the all-round peak at 100 m is about 1,900 slots, 390 MB. So
    the high setting's budget is about 400 MB, not the brainstorm's 128–256 MB, whose upper end
    holds only the frustum term; it sits inside the discrete ceiling of 2–3 GB and raises the
    brainstorm's GPU total of 0.6–1 GB by about 0.15–0.25 GB. For context only (the budgets stand,
    decided 2026-09-30 by a delegated decision, hardware item 5): the development machine's RTX
    3080 has 10 GiB of VRAM, shared with the local LLM, and the UHD 620 shares its laptop's system
    memory. The frustum term alone would fit 256
    MiB; T14 measures whether the all-round peak ever becomes resident, and adopts the frustum rule
    on the high setting only if it never does, recording which. `writeTexture` has no 256-byte row
    rule (only encoder copies do), so 520-byte rows upload as they are, but tiled layouts on Intel
    may pad a 65 × 65 texture to 1.8–2.2 times its size, so T14 measures driver-reported memory.
    Hence the layout: the cache is preallocated as fixed slots from the byte budget; heights, parent
    heights, the horizon map and the mask, all read by index, never sampled (updated 2026-10-16),
    live in storage buffers addressed by slot index, tight and untiled (the default
    `maxStorageBufferBindingSize` is 128 MiB); each storage binding is visible only to the stages
    that read it, so WebGPU's default eight
    a stage, never raised (Metal's adapters report 10 under Dawn), counts per stage: the lit
    material's vertex stage reads the heights, slot records, instances, contacts and, on
    `baked-offsets`, the offsets, and its fragment stage the class layers and the lights; the
    horizon map and the survey mask are each their own slot field (main's ruling for R10.T10.b,
    2026-10-15); normals and class weights, which want filtering, live in 2D arrays (at most the
    adapter's `maxTextureArrayLayers`, 256 by default) or atlases with a one-texel gutter; and each
    style's terrain is one instanced draw over R05's shared 65 × 65 index buffer with a slot index
    per instance, which also keeps the CPU's per-draw cost down. R05 already packs its octahedral
    normals as core, filterable `rg16float` (its Design note 5; the `rg16snorm` alternative would
    need `texture-formats-tier1`) and already builds `PatchCache` on this layout's fixed slots (its
    Design note 10); this plan supplies the `SlotLayout` and the budget, and the eviction rule stays
    R05's. _As built (R05's "Deviations in T8, as built" and T18): normals are stored with the
    atlas's one-texel gutter, 17,956 B at mesh resolution and 68,644 B at twice it, so a low slot
    is about 157 kB (160 kB with gutters on the class weights too) and 64 MiB holds 420–426 slots,
    under the rule's 430; and R05.T18 kept `BakedOffsets` on the high setting, so a high slot is
    about 310 kB and 400,000,000 B holds about 1,290 slots, against the rule's 1,900. T14 decides,
    as Risks' "The low setting's cache is at its edge" already has it for the low setting._

16. **The client fetches what the camera can resolve, and nothing more.** A body's field is wanted
    once it is the camera's local body or within the hand-over range of Design note 13 of any view's
    camera, and released when every view has left twice that range. The client sends R09's
    `surface_field` with `have_revision`, and a rise in the body's revision in `surface_revisions`
    on the scene topic sends it again for the new cells only. A chunk here is one of R09's
    self-contained payload blocks (at most `MAX_BLOCK_BYTES`, 1 MiB): the render thread reassembles
    R03's 256 KiB binary frames and, at each block boundary, forwards the block, without waiting for
    the whole payload as R09's Design note 17 allows. Blocks are decoded in the workers, each
    holding a `PartialField` per body: block 0 of every payload, delta or whole, carries the whole
    `FieldHeader` (R09's Design note 17), so `field_init` posts block 0 and `field_chunk` each block
    after it, and no block waits for the terminal `SurfaceFieldDto`. Cells arrive exact and whole
    with their margins, so a new chunk never changes a baked height, and only patches whose survey
    mask it changes are rebaked. Patches waiting on a chunk of a surveyed region count toward R05's
    `TERRAIN: STREAMING`; unsurveyed ground does not, because it is not coming. _As built: R03's
    `requestBulk` hands over whole answers (`onPartial`, then `outcome`), so T7 adds a per-chunk
    option there to forward a block at its boundary; a block's length is the `u32` at offset 6 of
    its 10-byte frame, so the render thread splits blocks without decoding them, and each worker
    decodes one with `wire::decode_block` (R09's "Deviations in T3, as built")._ _As built by
    T6.b, on main's ruling of 2026-10-14 (T6.a's question): a vertex outside the survey carries
    the true ground wherever the field can make it, a margin cell's being the server's to the bit,
    and the reference surface only where it cannot, the bake then incomplete. So a new block never
    changes a value at a vertex the field held before it, surveyed or margin, and a patch is
    rebaked on a block that surveys a cell holding one of its vertices outside the survey (its
    mask changes), or on any block while its bake is incomplete, and on no other block. Each
    worker answers a block with the cells it surveyed anew, and each bake carries its completeness
    and the cells its mask holds unsurveyed (Risks, "Deviations in T6.b, as built")._ _From T8.b
    (decision-r10-t8a §3): also on a block that brings a cell, surveyed or margin, that the patch's
    horizon map's far field was refused for. Each bake carries those cells, and each block's change
    the cells it brought that the field did not hold._

## Tasks

T1–T5 are `hyperion-surface` code and run in order after R09's synthesis lands; T6 moves R05's bake
onto it. T7 (the client's field store) needs only R09's chunks and R05's workers and can run beside
T1–T5, though its worker tests need T6.b's entry points. T8 (horizon maps) needs T6.a. T9 (cascades)
is independent of T8 but lands after it, so the high setting has its horizon map to combine with.
T10 (lit terrain) needs T2, T6 and T7, and its shadow term from T8 and T9; T10.f needs T10.d. T11
(the wireframe) needs T6 and T7. T12 (readouts) needs T5, T6.b and T7. T13 (guide drafts) is docs
only, can be written at any time, and lands before T11 and T12, which are built to the drafts; it
keeps its number because the roadmap cites `R10.T13`. T14 (cache sizes) follows T10 and T11. T15
closes. There is no interface-reconciliation task: this plan is re-validated against R05 and R09 as
built when its turn comes.

As re-validated at `c6d77cac` (Risks), with the critical path's order (R09's synthesis, then this
plan, then R09's server and coarse pass):

- **Startable now.** T13 (docs). T7, against R09.T19.a's types, R03's `FakeWebSocket` and the pool's
  scripted fake worker: T7 builds the render thread's half of the per-body calls (the pool's
  methods in place of `postField` and their messages in `workers/messages.ts`), and the height
  worker holds the blocks in JavaScript, as it holds R05's field today, until T6.b's module takes
  them; its live path waits on R09.T19.b. T1.a, whose tables read nothing of the synthesis (R09's
  palette types are built); its albedo rows took main's answer (Risks).
- **After R09.T8, and R09.T9 for goldens.** T1.b, T2 and T3, then T5 and T6.a; T6.b after T6.a, T5
  and T1.b. Each is built on `synthetic_worlds()` and `FieldBuilder` fields.
- **After R09.T16.** The coarse pass follows this plan, so until R09.T11–T16 land the reference
  worlds are a smooth dry sphere and `tests/common/surface_worlds.rs` does not exist. T4.a, which
  measures the real coarse pass's output, waits on R09.T16 whole; and the `sample_worlds()` cases
  of T1.b, T2, T5 and T10.a (their `crates/hyperion-sim/tests/` binaries) follow R09.T16 under the
  same task IDs, after each task's `synthetic_worlds()` part lands (a follow-up, as R09's T5 had).
  T3's `surface_ground_worlds` asserts finiteness, which the no-op worlds already exercise (as
  built, on the reference worlds through `coarse_pass`, moving onto `sample_worlds()` when it lands).
  T5's `surface_reading_worlds` holds its pooled case on `synthetic_worlds()`'s four bodies (as
  built), and its `sample_worlds()` case follows R09.T16.
- **T4.b** builds `levelBounds.ts` and the selection rule after T6.b, on a fixture from
  `synthetic_worlds()`, and records the owner's counts after T4.a's `level-bounds.json`.
- **T6.a's bench** waits on R09's follow-up after T8, the fused interpolant pass that main has
  queued before this plan's quiet bake timing; meanwhile its figure is recorded provisional.
- **New T1.c** (decision-composition) follows T1.b and T1.d, as a Rust half now and a client half
  after T16 (decision-r10-t1c-maturity).
- **T10.a** follows T1.b; its server test of `surface_field`'s answer waits on R09.T19.b. **T10.b**
  draws the atmosphere through R07's stubs until R08.T9.b lands, and the ground's thermal emission
  waits on R08.T18.

Rust files are under `crates/hyperion-surface/src/` and client files under
`apps/hyperion/src/renderer/src/view/terrain/` unless a path says otherwise. Every test in
`hyperion-surface` follows R04's Design note 12: the `wasm_bindgen_test as test` import, a
`native_only` module for any test that reads files, and `should_panic` tests, each stating
`expected`, in the crate's `tests/panics.rs`. Golden files end in `.golden` and are read through the
testkit's embedded arm. Every GPU check is by hand and recorded in the task's plan entry with the
machine, setting and date, as the brainstorm's Testing section requires; no golden images. Every
timing, bench or by hand, is taken on a quiet machine, with no other test suite or lane running, and
recorded with the load average; a figure taken under shared load is marked provisional and
re-measured quiet before any decision rests on it. The timings this plan quotes from its research
(2–6 ms for the near-field horizon march, 60–120 ms for ray-marched far fields, 3–6.5 ms for the
station wireframe) are reasoned estimates, not measurements, and are provisional until the tasks
that own them measure them.

### R10.T1 Material classes

- **R10.T1.a The class set and its properties.** `MaterialClass` (with `Evaporite` and `Dust`),
  `Material { class, entry }`, `MohrCoulomb`, `FrictionModel`, `MaterialProperties`,
  `properties(material, &palette)`, `limb_parameter`, `bearing_capacity`, `interface_friction`
  (Design notes 2 and 8; decision-composition §1.8: `FrostSpecies` is gone, and `properties` joins
  the class's table to the entry's resolved values in R09's `PaletteEntry`, by its `mechanics`
  family where the class's rows are per family). It also assigns the codes of R09's
  `PaletteEntry::phase_row`, 0 in every field until then, each naming a law's phase row and its
  L(α) (Design note 8), which T10.b builds and the server's palette sources fill; the codes are
  generated output, under the table's version. Each figure is
  re-checked in its primary (the Lunar Sourcebook's Table 9.12 page, USGS PP 1389 Table 7, NAVFAC
  DM-7.02 Table 1, Kietzig et al. 2010's Fig. 2, the albedo sources of Design note 8) and cited in
  the doc comment; a figure without a source leaves its field `None`. `CLIFF_SLOPE` 40° and
  `TRANSITION_HALF_WIDTH` 5° with Kleinhans et al. 2011 and Atwood-Stone and McEwen 2013;
  `SNOW_SHED_SLOPE` with a source or removed, recorded either way. Files:
  `material/{mod,class,properties,bearing}.rs`. Tests: discriminants are stable (a table test on the
  `u8`s); every property is finite and in its physical range; every row with a value names its
  source; `bearing_capacity` for a square 1 m footing on lunar regolith at 1.62 m/s², which takes
  the 30–60 cm layer (c = 3.0 kPa, φ = 54°, ρ = 1,740 kg/m³), lies within the Sourcebook's
  3,000–11,000 kPa and within ±50% of its 6,000 kPa, and the same formula on the 0–15 cm pair (c =
  0.52 kPa, φ = 42°, ρ = 1,500 kg/m³) gives 150–300 kPa (Design note 2); with the factor formulas
  the code cites, both values are pinned to 1%, since the Sourcebook's band is too wide to catch a
  factor error; it grows with width and gravity; every class's A_N and L(0) keep the law's
  directional-hemispherical albedo at μ₀ = 1 at most 1 (Design note 8); `interface_friction` for ice
  interpolates between its points and is `None` outside them. Acceptance:
  `cargo test -p hyperion-surface material`.
- **R10.T1.b The classifier.** `material_at` with the decision order of Design note 1, over R09's
  `Synthesiser` and the finest level's gradient, projected into the tangent plane first (R09's
  "Deviations in T4, as built"), reading the cell's substance byte (`SynthesisCell::ice_entry`,
  `liquid_entry`) and the header's palette (`crust_entry`, the `Deposit` and `Cover` roles) as
  Design note 1's folded-in rules have it, with the cold-trapped share on the cell's shadowed
  ground. Files: `material/classify.rs`, both helper modules of Test helpers, `material/golden.rs`,
  a `cfg(test)` module of the crate as `ground/golden.rs` is, writing its goldens under
  `crates/hyperion-surface/tests/golden/material/` (a thousand points on each of
  `synthetic_worlds()`; corrected 2026-10-14; R09.T9's pattern), and
  `crates/hyperion-sim/tests/surface_material_worlds.rs`.
  Tests: a `cliff_field` is `Bedrock` above 40° and not below; on `sample_worlds()`, a warm pole
  has no ice cap and a locked world's ice lies where R09's field puts it (the brainstorm's two
  examples under Materials); `Seabed` exactly below the water surface; an arid filled basin is
  `Evaporite`; the Pluto-like and lava-world `FieldBuilder` worlds of decision-composition take
  their frost and `Melt`; the order of rules is total; the goldens match on native,
  `wasm32-wasip1` and `wasm32-unknown-unknown` through R04's checks. The `sample_worlds()` cases
  follow R09.T16 (the order of tasks). Bumps `GENERATOR_VERSION` (see Generator version).
  Acceptance: `cargo test -p hyperion-surface material` and
  `cargo test -p hyperion-surface --lib material::golden` (the goldens alone; corrected 2026-10-14;
  R09.T9's pattern), `cargo test -p hyperion-sim --features testing --test surface_material_worlds`,
  and `just test-wasm-fast` runs the goldens on both wasm targets (the orchestrator's `just ci`
  runs all three).
- **R10.T1.d The space-weathering law** (decision-p14-t51d-weathering §1, 2026-10-16; lane
  r10-weathering, after R09.T2.f; built early for P14.T54.b, as R09.T7.d was for P14.T51.f).
  - **The law.** Hapke 2001's npFe⁰ absorption, α = (36π z ÷ λ) φ ρ_h ÷ ρ_Fe, added to the
    equivalent slab's internal transmission:
    - Θ = Θ₀ exp(−τ), τ = m f (M_Fe ÷ M_FeO)(ρ_h ÷ ρ_Fe)(36π z(λ) ÷ λ) Λ;
    - applied to the entry's fresh w, which the reference form's `A_N`(w) inverts;
    - given as ratios of the form's `A_N`, `A_S` and `A_h`;
    - the identity at τ = 0 or for an opaque grain.
  - **The fit.** `hyperion-fit`'s `space_weathering` fits Λ by least squares, each residual
    weighted by its anchor's 10–90% spread, to the Moon's mature highland and mare normal albedos
    in B, V and R (Dollfus and Bowell 1971, Table 4, at 600 nm, taken to the bands by the paper's
    own whole-Moon spectral law, Eq. 11, Ā ∝ λ^0.83, one colour for both terrains; the task
    computes each band's centre and 10–90% bounds from the 600 nm values;
    decision-r10-t1d-anchor-colour) from the fresh `feldspathic` and `basalt_fe` rows at m = 1. Each
    row is taken at its anchor soil's FeO_T, 0.055 and 0.16 (Morris 1980, Table 1: the mature
    Apollo 16 soils and the Apollo 11, 12 and 17 mare-plain soils), the manifest's explicit
    parameters (decision-p14-t51d-weathering, addendum 2026-10-19). It reports the implied y_sat
    against Morris 1980's Eq. 4 at T51.d's X = 75. It emits `tables/space_weathering.rs`, stamped
    `since-generator-version: 22`, the version in force, since nothing at 22 reads it, and P14.T54.b
    restamps it `--since 24` when it wires the law (main's ruling, 2026-10-19: `hyperion-fit check`
    refuses a table ahead of `GENERATOR_VERSION`, as P14.T49.c's `substance_albedo` was held).
  - **The scale.** `field_albedo_scale` solves c on the albedos weathered at the header's
    maturity, and R10 draws clamp(c × W(…)).
  - _Files:_ `material/weathering.rs`, `tables/space_weathering.rs`, `tables/mod.rs`,
    `crates/hyperion-fit/src/tasks/space_weathering.rs` with its manifest, and
    `crates/hyperion-server/src/surface/service.rs`.
  - _Tests:_
    - the identity at m = 0, f = 0 and for `Lambert`;
    - monotone non-increasing in m and f;
    - B darkens relatively more than R;
    - never above the fresh value, and `A_h` at most 1;
    - the highlands inside their spread in B, V and R and the maria inside in B and at the lower
      edge in V (at least the P10 bound less σ_V ÷ 8, computed from the anchors) with one Λ; the
      maria's R and colour stated as a known miss (the form's dark end and the stand-in fresh
      rows); y_sat within a factor 3 of Morris's Eq. 4; and the fresh/mature contrast of each
      terrain stated against fresh craters (Dollfus and Bowell 1971, Table 4);
    - the golden on native and both wasm targets.
  - _Accept:_ `cargo test -p hyperion-surface material::weathering`;
    `cargo test -p hyperion-fit space_weathering`; `cargo run -p hyperion-fit -- check`;
    `just test-wasm-fast`.
- **R10.T1.c Maturity and cover.** New at the re-validation (decision-composition §1.8 and
  §7.5); ruled 2026-10-20 (decision-r10-t1c-maturity). Each material's `A_N` and colour come from
  R10.T1.d's law at the ground's maturity, blended with the organic cover, by one shared function
  in `material/maturity.rs`. The server's solve, the worker's photometry and class map (at
  m_body) and its point query (at m_local) all call it, so main-thread readers take the worker's
  values, never a TypeScript copy. `properties` stays the fresh join; `Melt` stays fresh.
  - _The maturity at a point:_ m_local = m_body − max over craters of s × max(0, m_body −
    `maturity_after`(t_c, k)), from the header's `maturity` and `weathering_rate` and each
    crater's age t_c (`SmallCrater::age`, `CoarseCrater::age`). A crater's fresh share is
    s = g f_ex p(x):
    - p(x) is 1 inside the rim and (e^(−(x − 1) ÷ 3) − e^(−3)) ÷ (1 − e^(−3)) out to 10 rim
      radii (Grier et al. 2000's radially averaged OMAT), shifted to 0 at the reach (1 + a) R for
      a coarse crater, which the field carries no farther;
    - f_ex = (1 − q)², q = d_m ÷ 0.084 D, d_m = 0.022 m ((t_s − t_c) ÷ 1 Myr)^0.45: the share of
      a paraboloid excavation cavity below the mature layer (Morris 1978; Melosh 1989), so that a
      crater under about 10 m on old ground exposes nothing fresh;
    - g is the crater's kept relief, as the synthesis computes it: the gardening that wears a
      small crater mixes its ejecta away (Grier et al. 2000: small craters "age" faster).

  - _The cover:_ (1 − c) A + c A_cover, with c the header's `organic_cover` and A_cover the first
    organic `Cover` entry's `A_N`. It applies on every form of an `Ice` entry whose formula holds
    carbon, CO₂ excepted, and of `N2` beside one (P14.T51.d's `organic_ground` on the palette's
    keys).
  - _The split:_
    - this Rust half now, landing before P14.T54.b (if it does not, T54.b leaves the maturity and
      the rate absent);
    - a second Rust follow-up, in which the solve takes each material's albedo averaged over the
      body's young-crater haloes, so that the drawn mean stays p (generator-versioned: at 24 if
      ready, else the next bump);
    - a client half after R10.T16 and with or after that follow-up, which bakes a per-texel
      maturity with the same function and blends between Rust-computed albedos
      (`bake/layers.rs`, `worker.rs`, `wasm.rs`, `heightBake.ts`, `terrainMaterial.ts`,
      `terrainLit.wgsl`, `classMap.ts`, `bodies/draw.ts`), with its check by hand on the RTX 3080.
      It is presentation, so it owes no bump.

  - _Files (the Rust half):_ `material/{maturity,mod,albedo,properties,classify,golden}.rs`,
    `synth.rs` (an accessor), `worker.rs`, `testing.rs`, `testing/worlds.rs`,
    `tests/golden/material/maturity.golden`, `tests/panics.rs` and
    `crates/hyperion-server/src/surface/service.rs`.
  - _Tests:_
    - the fresh/mature contrast is stated, not asserted, against Dollfus and Bowell 1971's Tycho,
      Proclus and Kepler at each crater's own age. It is a known miss (D2) until T49.i's whole-soil
      fresh rows, and is asserted within the measured range after their refit. Its invariants and
      as-built values are pinned;
    - one crater's profile; k scaling; f_ex and g; overlaps;
    - the immature shares, with a guard, and Grier et al. 2000's count printed;
    - the identity, to the bit, with no maturity and no cover;
    - `Melt` fresh;
    - zero cover gives the substance's own albedo;
    - the solve on the exposed albedos;
    - the golden on native and both wasm targets.

  - _Accept:_ `cargo test -p hyperion-surface material::maturity`;
    `cargo test -p hyperion-surface --lib material::golden`;
    `cargo test -p hyperion-surface --test panics maturity`;
    `cargo test -p hyperion-server albedo_scale`; `just test-wasm-fast`.

### R10.T2 Class weights for coarser levels

`ClassWeights`, `weights_at` (Design note 3), with the slope-exceedance probability from R09's
level-keyed closed forms at level n, `Synthesiser::omitted_variance` and
`omitted_structure_function` (what the level leaves out of the finest level's ground; added by
R10.T5, `decision-r10-t5-closed-forms.md`), and the palette of up to eight classes a patch keeps. As built, `unresolved_variance` and the channels' and craters' own structure functions
exist, and the whole `structure_function` is R09.T8's (landed, `c280f604`: the free function and
`Synthesiser::structure_function`; updated 2026-10-15); the closed forms are held once per body,
since R09's crater octaves cost about a millisecond to build and the per-cell forms rebuild them
(R09's "Deviations in T7.b, as built"). Files: `material/weights.rs`,
`crates/hyperion-sim/tests/surface_weights_worlds.rs`. Tests:
at the finest level the largest weight is `material_at`'s class at every sampled point, ties
included; weights sum to 255; on `sample_worlds()` the weights at levels n − 1 to n − 6 from the
finest, and at the class map's level (level 4, Design note 8), agree with brute-force class
fractions over each texel's footprint (sixty-four finest points a texel at n − 1 to n − 6, 4,096
sampled points a texel at the class map's level) to within ten percentage points in the mean over
a thousand texels a level (slow); a patch never needs more than eight classes on `sample_worlds()`,
and if one does the test names it. The same agreement is first held on `synthetic_worlds()`, whose
full belts put about a third of their area above 40° at 2 m and plain styles none (R09's
"Deviations in T5's second follow-up"); the `sample_worlds()` binary follows R09.T16. Acceptance:
`cargo test -p hyperion-surface material::weights`,
and the slow case under `just test-slow -E 'binary(surface_weights_worlds)'`.

### R10.T3 The authoritative ground on real terrain

R05's `finest_surface_height`, pointed at R09's `Synthesiser` through R05's `HeightSource` trait
(R05.T4.a, T4.c), which this task implements for it (R09's `BandLevel` from the `u8` level,
`SynthCache` as the cache, `QueryHeightError` as the error), and wrapped as `ground_at` with
`GroundPoint` and `FinestTriangle` (Design note 4), on R05's diagonal and `xyz_to_face_uv`. The
trait's `level_bound_m` and `height_range_m(key)` forward R09.T8's `level_bound_m(header, level)`
and `Synthesiser::height_range_m(patch)`, and `figure()` the header's (R09.T8 has landed,
`c280f604`, and as built the trait's `level_bound_m` reads the bounds the synthesiser holds, bit
for bit the free `level_bound_m(header, level)`: Risks, "Deviations in T3, as built"; updated
2026-10-15). R05's interpolant returns the height alone, so this task adds beside it, in
`patch::collision`, the query that also gives the triangle, with `finest_surface_height` its
height. Files:
`ground.rs`, R05's `patch` module, `src/ground/golden.rs` (an in-crate `cfg(test)` module) with
`tests/golden/ground/<world>.golden` (corrected 2026-10-15). Tests: at
a vertex `ground_at` equals R09's `height_at` there bit for bit; on an analytic plane field it is
exact; points on a shared edge get the same height from either triangle; points either side of a
cube-face edge and at a cube corner agree to 1 µm; the normal is the triangle's; the chord sagitta
of Design note 4 (L² ÷ 8R at 0.5 m on 6,371 km, about 5 nm) keeps face-coordinate and spatial
interpolants within 1 µm; `ground_at` is finite at every sampled point of `synthetic_worlds()` and
of `sample_worlds()` (the latter in `crates/hyperion-sim/tests/surface_ground_worlds.rs`), and
asserts finiteness before it returns (the brainstorm's NaN hazard). The goldens, on
`synthetic_worlds()`, are asserted on native and both wasm targets. No bump: the ground reads
heights that R09's goldens already pin (R09.T9's, landed in `070da8ee`, on all three targets at
header 21). Acceptance:
`cargo test -p hyperion-surface --lib ground`, which runs the goldens too, an in-crate `cfg(test)`
module (`src/ground/golden.rs`; corrected 2026-10-14; "Deviations in T3, as built"), and
`cargo test -p hyperion-sim --features testing --test surface_ground_worlds`.

### R10.T4 The level bound on generated worlds

- **R10.T4.a The bound measured.** R09's `level_bound_m` on `sample_worlds()`, the real coarse
  pass's output rather than R09's synthetic fields, with σ per level from R09's level-keyed closed
  forms, `Synthesiser::omitted_rms` at each level (Design note 5; added by R10.T5). Files: `crates/hyperion-sim/tests/level_bound_worlds.rs`, which also writes each world's
  per-level hard bound and σ as a fixture for T4.b, `fixtures/level-bounds.json` under the client's
  `view/terrain/`. Tests: for ten thousand positions a world and every pair (n, n +
  k), |h_n − h_(n+k)| stays under `level_bound_m(n)` (slow). Recorded per world and level in this
  task's plan entry: the observed 99.9th percentile over the bound (expected 0.3–0.5), the
  per-patch maximum over 4,225 vertices over the bound, and the spread between the body-wide and a
  per-patch bound. _Re-validated at `c6d77cac`: this task waits on R09.T16 (the reference worlds
  through the whole coarse pass, and `tests/common/surface_worlds.rs`) and on R09.T8, whose bound as
  accepted is hard in its octaves and channels and a 1 − 10⁻⁹ per-point quantile in its crater and
  rim terms, so the test holds that stated bound and records which term each worst case came
  from._ Acceptance: `just test-slow -E 'binary(level_bound_worlds)'` passes, and the
  figures are recorded.
- **R10.T4.b Selection by the real bound.** The worker answers `field_init` with the body's
  `LevelBounds` (hard and σ per level, σ R09's level-keyed `Synthesiser::omitted_rms`, added by
  R10.T5); `levelBounds.ts` holds them per body and `selectionBound`
  gives R05's `screenSpaceErrorPx` its ε_n under the rule in force (the hard bound until the owner's
  ruling), for each style's τ: 1 px high and 2 px low (the brainstorm's lean; CesiumJS's default
  `maximumScreenSpaceError` is 2 px, though its error is a spacing heuristic), and 4 px wireframe.
  As built, selection reads the bound column of the body's level table (`PlanetGeometry.levels`,
  stride 4), and its error is that bound plus R05's chord sagitta (`selectionErrorM`), so
  `selectionBound` fills that column and `field_init`'s answer carries the rest of the table, the
  lowest and highest height by level that culling reads before a patch is baked (lean: the union of
  the six root patches' `height_range_m`, R09.T8's). The test planet keeps its own `levelTable`.
  _As built by T6.b (Risks, "Deviations in T6.b, as built"): every block taken, not `field_init`
  alone, is answered with the body's level table in `levelTable`'s layout, its heights a row each
  from the held cells' elevations and the level's own contributions rather than the root patches'
  union; there is no σ column, which T4.b makes; the pool keeps the latest table
  (`fieldLevels(body)`)._
  Files: `levelBounds.ts`, R05's `planet.ts` and `select.ts`. Tests (Vitest, pure): a level is
  selected exactly where its selection error subtends at most τ, under either rule, taken as a
  parameter; R05's
  history-independence test passes with real bounds; the grounded-body rule still yields the finest
  level. Recorded in this task's plan entry, from T4.a's `level-bounds.json` fixture through
  `selectPatches` over R05's scripted descent (`descentProfile.ts`): the patches selected at τ = 1
  px per world under the hard bound and under min(hard, 4σ), so the owner rules with numbers.
  Acceptance: `pnpm --filter hyperion exec vitest run src/renderer/src/view/terrain` and the counts
  recorded.

### R10.T5 Surveyed readings

`SurveyedReading`, `surveyed_reading`, `SlantRange`, `slant_range`, `SLOPE_BASELINES` (Design notes
6 and 7), over R09's level-keyed `omitted_rms` at the reading's level, its
`omitted_structure_function` at lag b (both added by this task, as ruled in
`decision-r10-t5-closed-forms.md`, since R09's `unresolved_rms` at `λ_n` counts less than a level
leaves out) and the coverage resolution per cell (`PartialField::resolution`), with J₀ through
base's `math::j0` (R04.T4.c), inside R09's crater form. As built: the closed forms are held once
per body by the `Synthesiser`, as T2's are; `PartialField::resolution` is `Some(ResolutionCode::NONE)`
on a margin cell, so "unsurveyed" is `!is_surveyed(cell)`, and `ResolutionCode::resolution()`
gives the metres; the resolved slope is the gradient's tangential part (R09's "Deviations in T4,
as built"). The pooled case on `sample_worlds()` follows R09.T16; it is first held on
`synthetic_worlds()`. Files: `reading.rs`, `crates/hyperion-sim/tests/surface_reading_worlds.rs`.
Tests: at a band-limit survey the elevation is `ground_at`'s, the uncertainty zero and `resolved`
set; at an orbital
survey, pooled over about 200 cells × 100 points a world on `sample_worlds()`, the RMS of
`ground_at` less the quoted value is within 10% of the pooled predicted σ, and the per-cell z-scores
have unit variance within 15% (slow); per band, the slope uncertainty tends to √2 σ_k ÷ b for b
much larger than the band's wavelength and to the band's gradient σ for b much smaller (Design note
7's limits); the RMS slope is never
below |g₀|; an unsurveyed cell gives `None`; the slant range to a plane is exact and its σ follows
cos θ_s ÷ sin γ_s; a ray under the `GRAZING` threshold has no σ; a ray that leaves the surveyed
region before it hits gives `LineOfSight::NotSurveyed`, and one that misses the body's ground
gives `LineOfSight::NoGround` (as ruled, `decision-r10-t5-closed-forms.md`; the σ is R09's
level-keyed `omitted_rms`, the slope's v its `omitted_structure_function`). Acceptance: `cargo test -p hyperion-surface reading`, and the
pooled case under `just test-slow -E 'binary(surface_reading_worlds)'`.

### R10.T6 The bake on real terrain

- **R10.T6.a The Rust bake.** R05's `bake_patch`, pointed through T3's `HeightSource` implementation
  at R09's `Synthesiser` and its `Synthesiser::bake_patch` (heights, parent band and gradients) and
  `SynthCache`, and extended with `TerrainLayers`: the class palette and weights (T2), the survey
  mask from the `PartialField`'s `is_surveyed`, the patch's hard bound and σ, the horizon map once
  T8 lands, and R11's `BakeHook`, with `scatter::ExtraLayer` declared as an empty
  `#[non_exhaustive]` enum for R11 to own and extend; normals at R05's `NormalScale`, which R05
  already packs as `rg16float` (its Design note 5). Files: `bake/{mod,layers}.rs`, R05's `patch`
  module. Tests: the bake is the same bytes whatever order a patch and its neighbours are baked in
  (`assert_order_independent`); heights equal R09's `height_at` narrowed to `f32`; the survey mask
  is 0 exactly on vertices of unsurveyed or margin cells; inserting a block that adds cells changes
  no baked height of a patch already baked (Design note 16); a change of a cell's `ResolutionCode`
  alone leaves every bake byte-identical (the image is the same at every survey resolution that
  covers it, Design note 6). Bench `bake/patch_65`: the brainstorm's 40 ms a patch on two cores
  (about 10 µs a point with its gradient, Performance budget), recorded with and without the class
  weights on the development machine (a Ryzen 7 3700X) and, by the owner, on the UHD 620 laptop's
  i7-8665U, whose cores the brainstorm's figure budgets. The 40 ms stays the laptop's figure: the
  desktop records its own and fails only if over it (decided 2026-09-30, Risks). _Re-validated at
  `c6d77cac`: `Synthesiser::bake_patch` is R09.T8's (landed, `c280f604`; it gives `PatchHeights`;
  updated 2026-10-15), whose own bench is provisional at 77–82 ms a patch and 14 µs a point at
  L + 4, so this bench's quiet figure
  follows R09's fused-interpolant follow-up, which main queued before it; meanwhile the figure is
  recorded provisional. The bench is a `[[bench]]` of its own (`benches/bake.rs`, on the synthetic
  worlds, so `required-features = ["testing"]` as R09's `synth` bench has it)._ Acceptance:
  `cargo test -p hyperion-surface bake`, and the bench's figure in this task's entry
  (`just bench -- bake/patch`, on a quiet machine, under the heavy lock, or recorded provisional).
- **R10.T6.b The worker binding.** The worker's entry points on R05's binding:
  `field_init(body, header)`, `field_chunk(body, block)` (one R09 payload block, Design note 16),
  `field_release(body)`, holding one `PartialField` per body; `bake`, taking R05's `BakeOptions`
  with this plan's settings; and, for T12's readouts, `reading(body, at, slope_baseline)`,
  `slant_range(body, origin, direction, max)` and `material(body, at)` (the class with its
  `MaterialProperties`), each answering `NotSurveyed` where the field has no cell. `field_init`
  answers with the body's `LevelBounds`. R05's provisional height function leaves the client path
  (R09's Design note 18); `hyperion_surface::height`, where R09.T4 moved `HeightSample`, stays, as
  does `LatticeCache` in `noise`, which R09.T4 did not move (it already lived there). As built,
  R05's module serves the test planet (`bakePatch`, `levelTable`, `surfaceHeightM`,
  `omittedSigmaM`, each with `Ridges`), which the spike keeps: the per-body exports go beside them,
  `BakeSettings` and `TerrainPassOptions` name the height source (the test planet with its
  `ridges`, or a body) in place of `ridges` alone, and `heightBake.ts`'s `HeightModule` gains the
  new exports. T7 has built the render thread's half (the pool's per-body calls and their
  messages), so this task builds the worker's: `answerRequest` takes the blocks into the module,
  each through `wire::decode_block` and `PartialField::insert`, and answers `field_init` with the
  level table. Files: R05's binding, R05's `view/terrain/workers/*.ts`. Tests: in the worker test,
  init, two blocks and a release leave no field held; a block posted before its `field_init` is
  refused; each reading entry point equals the native function on a fixture field.
  Acceptance: `just gen-surface`, then
  `pnpm --filter hyperion exec vitest run src/renderer/src/view/terrain/workers`.

### R10.T7 The client's coarse-field store

`CoarseFieldStore` and `CoverageView` (Design note 16): wanting and releasing bodies from the views'
cameras with hysteresis, R09's `surface_field` with `have_revision`, re-requests on
`surface_revisions`, forwarding chunks to every worker, the coverage and its resolution per cell,
the GPU coverage mask per face (Design note 12), allocated under the `MemoryCategory`
`coarse-field-gpu`, which this task adds to R01's union under R12's name (R12 Design note 6), and
the link's loss keeping what was received and
marking the coverage stale. As built (R09.T19.a; Consumes): the request is
`SurfaceFieldRequest { universe, body, have_revision }` (`null` for none) and the answer
`SurfaceFieldDto`'s `ready` arm (`header`, `revision`, `coverage`, `bulk`) or `not_modelled`;
`requestBulk` gains a per-chunk option beside `onPartial`, through which the store splits blocks
at each block's length (offset 6 of its frame) and posts each as it completes; `CoverDto`'s ranges
carry codes, which a TypeScript twin of `ResolutionCode::resolution` turns into metres, pinned to
R09's `tests/golden/wire/codes.golden` `resolution_m` rows. This task builds the render thread's
half of the per-body calls: `HeightWorkerPool`'s calls for `field_init`, `field_chunk` and
`field_release` in place of `postField` (the pool keeping a body's blocks for a replacement worker,
as it keeps R05's field now) and their messages in `messages.ts`, keeping the spike's call working
(`view/spike/spikeRun.ts`, whose 15 MB `syntheticField()` stands in for a field's memory and is no
payload; T7 records how); until T6.b the height worker holds the blocks in JavaScript, as it
holds R05's field today. R09.T19.b has landed (`7392c064`), so the server answers
`surface_field` with the payload and its deltas, pushes `surface_revisions` as
`SurfaceRevisionDto`s, and answers generated bodies `not_modelled` under `RecordInputs`; the tests
drive these through the fake socket in those shapes (Risks, "Deviations in T7, as built", its
"With R09.T19.b" item; updated 2026-10-15).
Files: `coarseFieldStore.ts`, `coverage.ts`; R05's `workers/{pool,messages,heightBake}.ts` and
`view/spike/spikeRun.ts`; R01's `view/engine/memory.ts`; R03's
`packages/protocol/src/requests.ts`. Tests (Vitest with R03's `FakeWebSocket` and
`pool.test.ts`'s scripted worker): a body within range is requested once and its chunks reach
every worker in arrival order; a revision rise fetches only new cells; leaving and returning
inside the hysteresis band fetches nothing; every worker gets `field_init` once and every chunk;
releasing a body frees it in every worker; the resident total of worker copies is reported, for
T14; the per-chunk option sees every chunk in order before the outcome settles. Acceptance:
`pnpm --filter hyperion exec vitest run src/renderer/src/view/terrain` and
`pnpm --filter @hyperion/protocol test`.

### R10.T8 Horizon-map shadows

- **R10.T8.a The bake.** `HorizonMap`, `far_field_point` and the bake of Design note 10 in
  `bake/horizon.rs`, called by T6's bake on both settings. Tests: a `flat_field` on a sphere has a
  horizon at the curvature drop's angle and never above zero; a `cliff_field` of height H at
  distance d gives arctan((H − d² ÷ 2R) ÷ d) within one quantisation step; over at least 10⁶
  elevations spanning [−90°, 90°], ±90°, 0 and the neighbours of code boundaries included,
  `decode_horizon(encode_horizon(e))` is within 90° ÷ 32,767 ÷ 2 (4.95″) of e, the decode is
  monotonic in the code, and `encode_horizon` equals WGSL's `pack2x16snorm` rounding on the same
  inputs (a fixture the TypeScript twin of T8.b also reads); for the Sun at 1 au (ρ = 959.6″) the
  lit fraction from the decoded horizon is within 1/255 of the exact one at every e; azimuth
  interpolation is continuous; the far field is bit-identical at shared lattice points of adjacent
  patches of one level on one face, and the lattice's points and the march's step distances are the
  same bits at every level that holds them (decision-r10-t8a §1); the far field is independent of
  the cache's state, with bakes in any order; the march stops at √(2 R Δh). Bench `bake/horizon`: at
  most a quarter of the patch's 40 ms, recorded; over it, the near field moves to the line sweep.
  Presentation-only: no golden across targets, though the bake is deterministic. The far field's
  level-m heights cost R09.T8's per-point figure (provisional at 14 µs, against the 2–4 µs Design
  note 10's estimates assumed), which the bench records. Acceptance:
  `cargo test -p hyperion-surface bake::horizon` and the bench's figure recorded
  (`just bench -- bake/horizon`, in T6.a's `benches/bake.rs`).
- **R10.T8.b The shadow lookup.** `shadows/horizon.wgsl`: the sun's apparent direction from
  `body_fixed_at`, its elevation against the datum's normal ν and its grid azimuth in the patch's
  face's grid frame (`grid_azimuth`, T8.a; R08's lookups keep their azimuth from local north), the
  azimuth through `preciseAtan2`, R07's star and R08's refraction where drawn; the compare against
  the interpolated horizon, carried into the element's frame as e − (e★ − (π ÷ 2 − φ)), e★ the
  sun's elevation above the datum and φ its angle from the terrain's normal, and passed as the
  `horizon` argument of R07's `sphere_irradiance(h, phi, horizon)` (the signature and its default
  of 0 on the smooth figure are R07.T6.a's and T6.c's, used unchanged), whose visible share is the
  soft lit fraction of Design note 10, applied once (decision-r10-t8b §1–2). The horizon is
  blended from the patch's map to its parent's by the vertex's morph factor k, the parent's slot
  and quadrant in the instance record's spare words, the parent read where k > 0 from the same
  `horizon-map` buffer, and the own map where no parent's map is resident (decision-r10-t8a §1).
  The worker bakes the map for the lit style and hands it over, with its refused cells; a patch is
  baked again on a block that brings one of them, surveyed or margin, and each block's change
  carries the cells it brought that the field did not hold (decision-r10-t8a §3; Design note 16).
  The horizon map binds as the `horizon-map` slot field's storage buffer, fragment only
  (decision-r10-t10b-r09-t2e §5). The sun's elevation is π ÷ 2 less its zenith angle
  `preciseAtan2(|s × ν|, s·ν)`, since as built `preciseAtan2(y, x)` takes y ≥ 0 and returns
  [0, π] (R08 Design note 18); R07's function keeps WGSL's built-ins, a deferred correction to R07
  (decision-r10-t8b §2). R08 draws no refraction yet, so the sun's direction is the geometric one.
  R07's clamp of a negative horizon then takes the element's own plane, exactly, where that hides
  more. WGSL's `asin` inherits atan2's 4,096 ULP, about 50″ at 30°, so no elevation here is formed
  by it. Files: `shadows/horizon.wgsl`, `shadows/litFraction.ts`, and the call in
  `terrainLit.wgsl`; R07's `litBody.wgsl` is unchanged. In the crate, `bake/horizon.rs`,
  `worker.rs` and `wasm.rs`; in the client, `workers/{messages,heightBake,rebake,pool}.ts`,
  `slotLayout.ts`, `gpu/uniforms.ts` (with the terrain pass's instance packing) and
  `shaders/{terrainPatch,terrainLit}.wgsl`. Tests: with a horizon of 0 the terrain's
  `sphere_irradiance` equals R07's smooth-figure value bit for bit, and with the map's horizon in
  the element's frame it equals R07.T6.a's oracle to 10⁻⁶ where the disc is wholly up or down, and
  an integral over the disc above the datum's horizon and the element's plane within 1.5 × 10⁻³ of
  face-on over slopes to 45°, every bearing and horizons to 60°; the lit-fraction formula as a pure
  TypeScript twin against hand values (0 and 1 beyond ±ρ, one half at e_sun = e_horizon); the
  blend's twin, `gridAzimuth` against the crate's fixture, and the rebake rule in the crate and on
  the module (decision-r10-t8a §1–3); R01's smoke harness compiles and runs the pass on
  SwiftShader with and without f16, every texel finite. By hand, recorded: a sunset over a
  generated range on the low setting, on the development machine and, by the owner, on the UHD
  620, whose GPU time, with the blend, is the one held against the budget's 0.3–0.5 ms
  (Performance budget, Shadows row); whether eight azimuths show missing peaks; and three places
  to look: a coarse–fine edge, with the blend, a same-level patch edge (Risks, "Edges between far
  points") and a face edge near a cube corner (decision-r10-t8a §1). Acceptance:
  `pnpm --filter hyperion exec vitest run src/renderer/src/view/terrain/shadows` and the smoke run.

### R10.T9 Cascaded terrain shadows on the high setting

- **R10.T9.a Cascades.** `shadows/cascades.ts` (Design note 11): four 2,048² `depth32float` layers
  (the brainstorm's 64 MB), allocated under the `MemoryCategory` `shadows`, added to R01's union
  under R12's name, PSSM splits with λ ≈ 0.75 over the shadowed range f(h), bounding-sphere
  fitting, whole-texel snapping, light matrices camera-relative in `f64`, `unclippedDepth` where
  `depth-clip-control` is offered and the pulled-back near plane otherwise, slope-scaled bias and
  normal offset; the sources of Design note 11 re-checked. Tests (pure): the splits are monotonic
  and continuous in the height above ground; snapping moves a cascade by whole texels only; the
  fitted radius does not change as the camera turns; cascades are off where cascade 1's texel is
  coarser than the horizon map's. Acceptance:
  `pnpm --filter hyperion exec vitest run src/renderer/src/view/terrain/shadows`.
- **R10.T9.b The high setting's shadow term.** The horizon map everywhere, cascades by `min`, each
  cascade fading to lit over its last 10%, 2 × 2 PCF with Poisson taps, exposed to R11 through
  `terrain_shadow_factor`. By hand, recorded: the discrete target (the development machine's
  RTX 3080) at 1080p against the budget's 1.5–3 ms, and no double-dark band at a cascade's edge in a
  sunset scene; R01's feature report on whether the UHD 620 offers `depth-clip-control`. Smoke run
  on SwiftShader. Acceptance: the smoke run passes and the figures are recorded.

### R10.T10 The lit style on generated terrain

- **R10.T10.a Class photometry.** `albedo_scale` in `material/albedo.rs` (Design note 8): the exact
  piecewise-linear solve of Σ a_i c A_N,i [L_i + ⅔ (1 − L_i)] = p over the whole field, with the
  residual. _The limb ruling (decision-r10-limb §2, folded in): the solve uses each class law's
  L(0) of c A_N, L_i = `PhaseLaw::limb_parameter(c A_N,i)`, so p_i = c A_N,i [⅔ + ⅓ L_i]. For a
  particulate class it is linear in c between breaks where c A_N,i crosses 0.60 and 1.35
  (`LUNAR_LIMB_ONSET_NORMAL_ALBEDO`, `BRIGHT_FROST_NORMAL_ALBEDO`), since x L_p(x) is linear
  there; a consolidated class, under the Lambert law, has p_i = ⅔ c A_N,i. Each c A_N,i is clamped
  to its form's span, where one exists, and to its law's `normal_albedo_ceiling` (1 for Lambert's,
  1.35 for the particulate laws). The as-built rule's breaks at 0.815 and 1 are retired._ It is
  called by R09's `SurfaceService` once the field is built, with plan 14's visual
  geometric albedo p (the photometry section R07.T1 asks of plan 14), so that c ships in
  `FieldHeader.albedo_scale`. Filling the header changes the payload, which R09's Design note 17
  puts under `GENERATOR_VERSION`; as built it bumps nothing, since the payload's layout freezes only
  at the first send and no generated body is served before it (main's ruling for T10.a, 2026-10-14:
  Generator version, and Risks, "Deviations in T10.a, as built"; updated 2026-10-15). As built
  (R09's "Deviations in T17, as built"): the call site is `surface/service.rs`'s `compute`, after
  `coarse_pass`, where the body's record is in hand, its p the photometry's `geometric_albedo()`
  (by band); the tests that compare the served field with `coarse_pass` bit for bit
  (`tests/surface_service.rs` and two in `surface::service`) apply the same `with_albedo_scale`.
  Per band or one c with its residual per band (decision-composition §1.8) was asked in Risks,
  since the header holds one `Option<f64>`; main answered one c, fitted in V, with the B and R
  residuals stated and recorded. Files:
  `material/albedo.rs`, R09's `hyperion_server::surface` service,
  `crates/hyperion-sim/tests/surface_albedo_worlds.rs`, a server test. Tests: on each of
  `sample_worlds()` the area-weighted p of the scaled classes equals the body's p to 1% where the
  ranges allow, and the residual is stated where they do not; the clamped solve equals a bisection
  reference; no scaled class's c A_N exceeds its law's `normal_albedo_ceiling` (the limb ruling);
  the solve depends on the class fractions only; the server's `surface_field` answer
  carries `albedo_scale` as `Some(c)` equal to `albedo_scale(field, p)`, and `None` while plan 14's
  p is `not_modelled` (R09.T19.b has landed, `7392c064`, so the `surface_field` answer and its
  block 0 are tested over the socket in `crates/hyperion-server/tests/surface_knowledge.rs`: Risks,
  "Deviations in T10.a, as built"; updated 2026-10-15; the `sample_worlds()` binary follows
  R09.T16). Acceptance: `cargo test -p hyperion-surface material::albedo`,
  `cargo test -p hyperion-sim --features testing --test surface_albedo_worlds` and
  `cargo test -p hyperion-server surface`.
- **R10.T10.b The terrain shader.** `terrainMaterial.ts` and `shaders/terrainLit.wgsl`: the palette
  and weights to one lunar-Lambert law per class (A_N × c, L(0), and f(α) and L(α) from the class's
  row) through `body_brdf`, summed by weight, the normals at the setting's resolution, the shadow
  term from T8 or T9, the hooks `terrain_decoration`, `terrain_shadow_factor`, `terrain_ring_shadow`
  and `terrain_sky_factor` for R11 (each defaulting to no effect), the direct sun through R08's
  `atmosphere_sun_transmittance` and the sky through its `atmosphere_sky_irradiance`, and R08's
  aerial perspective on the output. Each call passes the texel's geodetic height above
  `BodyFigure`'s spheroid, its geodetic latitude (from the spheroid normal against the pole) and,
  for the transmittance, each sun's azimuth from local north, as R08 Design note 17's signatures
  require. As built (Consumes, R08): each call also passes the light's `curve` slot (0 for a
  planetshine source), and planetshine's direct term goes through
  `atmosphere_source_transmittance`; until R08.T9.b lands the two names are R07's four- and
  three-argument stubs in `litBody.wgsl`, which this task calls as R07's discs do. The aerial
  perspective comes from drawing the lit terrain into the `AtmosphereScene` that
  `HillaireAtmosphere.drawFrame` composites on the low setting (scope `"terrain"`), and before that
  composite on the high (`"scene"`). The terrain's footprint joins `photorealFrame`'s
  `depthWriters`. The ground's thermal emission, `Melt`'s and any texel's above R08.T18's
  visibility cut, goes through R08.T18's shared function once it lands (decision-composition).
  Each class of the patch's palette has its own
  `struct LunarLambert { a, l, s, table_row }` (a = A_N × c, l = L(0), the class's s and row),
  passed to R07's `body_brdf(law: LunarLambert, mu0, mu, alpha)`, whose signature R07.T4.c creates
  and this task uses unchanged. _The limb ruling (decision-r10-limb, folded in): `LunarLambert.l`
  is `PhaseLaw::limb_parameter(c A_N)` of the class's law, and the class laws are three, Regolith,
  Ice and Lambert (f ≡ 1, alpha 0, from R07's `lambert` template, provisional and labelled as R07
  has it), with a row for each distinct (law, L(0)), the law fixing its template and s: f is
  tabulated against L(α) = L(0) × McEwen's profile, so a particulate class at L(0) < 1 (a bright
  frost) takes a row of its own, and the Lambert law one row whatever its classes' albedos; row 0 is
  the body's uniform law (main's ruling for R10.T10.b, 2026-10-15); a consolidated class's row is
  Lambert's whatever its entry's code._ The
  texel reflects the classes' `body_brdf` radiances summed by its
  weights, skipping zero weights, not one `LunarLambert` built from weighted parameters, which
  differs from the sum by the covariance of A_N and L: 1.7× too bright at μ₀ = 0.5, μ = 0.1 for an
  even blend of regolith and snow (Design note 8). One `phase_factor_table` row per class law (its
  template, s and L(α)): f in r, g and b, L(α) in a; `body_brdf` reads L(α) from the row and
  `LunarLambert.l` carries L(0), so no row is built per texel and no f is taken from a neighbouring
  L (decision-r07-t8b). This task builds the channel in R07's files: `PhotometricLaw` gains an
  optional `lommelSeeligerCurve` (absent: L constant at `lommelSeeligerShare`, which is L(0)) in
  `appearance/law.ts`; Φ_shape with L(α) in `shapes.ts`, `phase.ts` (`lawFor`'s q) and `brdf.ts`;
  the spheroid integral at L(α) and `oblateAlbedoScale` at L(0) in `bodies/oblate.ts` and
  `bodies/draw.ts`; `shaders/litBody.wgsl` and `shaders/bodyDisc.wgsl`. It moves R07's L = 1
  templates (`moon`, `mercury`, `airless-ice`, `snowball`, `magma`) to McEwen 1996's L(α), which
  changes no p, and no q or point flux inside each template's range (past it the held f draws L(α)'s
  own tail: Risks, "Deviations in T10.b, as built"). Files: `terrainMaterial.ts`,
  `shaders/terrainLit.wgsl`; R07's `appearance/law.ts`, `shapes.ts`, `phase.ts`, `brdf.ts` and
  `templates.ts`, `bodies/oblate.ts` and
  `bodies/draw.ts`, `shaders/litBody.wgsl` and `shaders/bodyDisc.wgsl`, with
  `appearance/litBodyProbe.ts`'s `packPhaseFactorRows`, which writes the rows' alpha (0 as built);
  `displays/view/photorealFrame.ts` for the footprint. Tests: a pure test that a
  texel of two classes in weights w and 1 − w reflects w r₁ + (1 − w) r₂ to 10⁻⁶, r₁ and r₂ being
  the classes' own `body_brdf` radiances in R07.T4.c's twin, that for an even blend of regolith
  (A_N 0.12, L 1) and snow (A_N 0.9, L 0) sharing one f(α), at μ₀ = 0.5, μ = 0.1 it is that sum and
  not the 1.7× brighter law of the mixed parameters (unchanged by the limb ruling: snow is at L =
  0, and the shared f is the test's own construction), that a zero weight adds nothing, and that
  the largest weight's class is the one a readout names; the energy bound with f (the limb
  ruling, §3), in R07.T4.c's twin: for each row, with L(0) from the rule at A_N on a grid up to
  its law's `normal_albedo_ceiling`, A_h(μ₀) computed with the row's f and L(α) over μ₀ ∈ {1, 0.5,
  0.2, 0.1, 0.05, 0.02, 0.01, 0.005} is at most 1; for the Ice row at L(0) 0.38, sup η = 0.720 ±
  0.01 and η(1) = 0.636 ± 0.01 (η = A_h ÷ A_N); for the Lambert row, η ≡ 1 to 10⁻⁶; a one-class
  palette's texel equals the per-body
  law of that class in R07.T4.c's twin; the smoke harness renders a generated world's patch set on
  SwiftShader with every texel finite; a law without a curve draws R07's disc and twin unchanged
  (the row's alpha is its L); L(α) is 1 at 0°, 0.6084 at 30° and 0.1859 at 90°, and 0 from 103.9°;
  under it, the Moon template's p equals the constant-L law's exactly, its point flux to 10⁻⁴ at
  30°–120°, and its q falls by the held tail's 1.80 × 10⁻⁴, pinned to 5 × 10⁻⁶ and within 2 × 10⁻⁴
  (main's ruling for R10.T10.b, 2026-10-15), and its
  CPU-rasterised disc flux equals its point's to 1% at 30°, 90° and 120°. Acceptance:
  `pnpm --filter hyperion test` (the client suite: the Files reach R07's `appearance/`, `bodies/`
  and `shaders/`, which the view's `lighting`, `photoreal`, `post` and `scenes` tests also read) and
  `just test-render` (the WGSL).
- **R10.T10.c The survey edge.** The survey mask's discard, the skirts at the survey edge, and the
  reference surface drawn beneath with the inverse mask from the GPU coverage mask (Design note 12),
  in `terrainLit.wgsl` and the `mesh`-regime body's material; the edge mark over the image from
  `surveyEdges`, the guide's `SURVEY EDGE`. It first builds the per-stage storage visibility of
  Design note 15, unless R10.T8.b has built it: `StorageBufferSpec` gains an optional `stages`
  (`"vertex"`, `"fragment"` or both, both by default), `resourceLayoutEntries` writes each entry's
  own visibility, and the lit material declares `heights`, `slots`, `instances`, `contacts` and, on
  `baked-offsets`, `offsets` vertex only, and `class_layers` and `terrain_lights` fragment only; the
  mask is its own `survey-mask` slot field (65 × 65 bytes, four to a `u32`), bound to the vertex
  stage if read per vertex and interpolated, or to the fragment stage if bilinear by hand; if the
  split cannot be built, the mask joins R05's `heights` field, read as `u32` words through
  `bitcast<f32>`, never the reverse (main's ruling for R10.T10.b, 2026-10-15). Files:
  `shaders/terrainLit.wgsl`, `coverage.ts`, R07's `mesh`-regime material (`bodies/smoothMesh.ts`,
  `shaders/smoothMesh.wgsl`, `SMOOTH_MESH_MATERIAL` in `bodies/draw.ts`), `view/engine/types.ts`
  (`StorageBufferSpec.stages`), `view/engine/webgpu/materials.ts` (`resourceLayoutEntries`),
  `view/engine/platform.ts` (`requiredLimits`' doc: no binding-count limit is raised),
  `slotLayout.ts` (the `survey-mask` field, 4,228 B a slot) and `gpu/resources.ts`. Tests:
  `surveyEdges` returns exactly the boundary between surveyed and unsurveyed cells on a fixture
  coverage, margin cells counted unsurveyed; the smoke harness renders a half-surveyed patch set
  with no fragment where both terrain and the reference surface write; in `gpu/resources.test.ts`
  (the terrain's, not `view/engine/webgpu/resources.test.ts`) and `terrainPass.test.ts`, every
  terrain material, on both vertex paths and in lit and spike shading (and T11's wire once it
  lands), counts at most 8 storage buffers per stage by its declared visibility, and each binding's
  declared stages match the entry points that use it (a static check over the composed WGSL; the
  smoke harness's pipeline creation on SwiftShader validates it too). By
  hand, recorded: the skirt heights seen at the edge on a flattened generated world, which stay
  within the terrain's local relief. Acceptance:
  `pnpm --filter hyperion exec vitest run src/renderer/src/view/terrain src/renderer/src/view/engine`
  (the `view/engine/` files it changes) and the smoke run. _As built (Risks, "Deviations in T10.c,
  as built")._
- **R10.T10.d The class map on the disc.**
  - **The map.** `classMap.ts` builds the per-body coarse class-weights map over surveyed cells
    that R07's disc samples (Design note 8), under `coarse-field-gpu` with the coverage mask. It
    goes through R07.T8.b's construction as built. `classMap.ts` supplies each texel's weights
    to `packClassMap` and `classMapSurface` (R07's `bodies/discSurface.ts`):
    - `weights_at` at the cell's centre, byte ÷ 255;
    - `null` where the cell is unsurveyed;
    - a partly surveyed cell's weights times its surveyed fraction.

    `laws` are one per class, from Design note 8's parameters and T10.a's scale (at most 16,
    `MAX_DISC_CLASSES`). `elsewhere` is R07's uniform `lawFor(p, q, template)`. `classMap.ts`
    owns the texture: it remakes it after a device loss and releases it with the map. Folded in
    at the re-validation (decision-composition §1.8): the map's classes are the body's (form,
    entry) pairs, at most 16, which retires R07's "a body carrying all three `FrostSpecies` would
    need 17". As built, `classMapTextureSpec` files the map under `"other"`; this task gives it
    `coarse-field-gpu`.

  - **The reconstruction.** The disc and `rasteriseDisc` read the map by a survey-masked bilinear
    reconstruction, which replaces T8.b's nearest read (decision-r07-t8b):
    - A sample in an unsurveyed texel takes `elsewhere` alone.
    - A sample in a surveyed texel interpolates the full share vector (the classes and
      `elsewhere`'s 1 − Σw) over the four nearest texel centres. It counts surveyed texels only
      and renormalises.
    - Each face carries a one-texel gutter from its neighbours (N + 2 texels a side, the gutter
      corners unsurveyed), so the reconstruction is continuous across face edges.

    No surveyed pattern shows past the survey's cells, and the cells show no blocks.

  - **The view.** `litBodiesOf` (`displays/view/photorealFrame.ts`) passes each body's `surface`
    from the class-map store and its `rotation` from the scene body's `rotation` (body-fixed to
    the body frame, whose axes are the galactic ones), taken at the body's drawn time (R07.T10.a).
    `viewFrameDrawer.ts`, which `ViewDisplay.tsx` makes and which builds `PhotorealInputs`, passes
    the store. As built, `litBodiesOf` never sets `surface`, and `discSurfaceOf(appearance)`
    (`appearance/bodyAppearance.ts`, where `DiscSurface` is declared) is always uniform.
  - **Until the rotation arrives.** The scene body's rotation comes from P14.T46.f's rotation
    section, which R07.T2.b reads. Until then `rotation` is absent, and a live class map shades
    with `elsewhere`, as R07 built it. _As built, both are built (`BodyRotationDto` on
    `BodySummaryDto.rotation`), so a body has a rotation wherever its record's section is `ok`._
  - **Files.** `classMap.ts`; R07's `bodies/discSurface.ts`, `bodies/discShading.ts` and
    `shaders/bodyDisc.wgsl`; `displays/view/photorealFrame.ts`, `displays/view/viewFrameDrawer.ts`
    and `displays/view/ViewDisplay.tsx`.
  - **Tests:**
    - The class map holds no texel for an unsurveyed cell.
    - R07's disc under a class map of one uniform class equals its `lawFor` disc to 10⁻⁴.
    - At a surveyed texel's centre the shares are its own. A sample in an unsurveyed texel
      carries no surveyed weight.
    - A linear ramp of weights across a face edge stays linear to 10⁻⁶.
    - The half-surveyed two-class map's disc flux equals the area-weighted fluxes to 1%.
    - `litBodiesOf` passes a rotation where the scene body has one and none where it is `null`,
      and such a body's disc shades with `elsewhere`.
    - The smoke harness renders a disc with a half-surveyed class map, every texel finite, and
      equal to `rasteriseDisc` within R07.T8.a's tolerance.
  - **Acceptance.** `pnpm --filter hyperion exec vitest run src/renderer/src/view/terrain
src/renderer/src/view/bodies src/renderer/src/displays/view` and `just test-render`.
  - _As built (Risks, "Deviations in T10.d, as built")._
- **R10.T10.e The disc hands over.** R07's `disc` regime gives way to the quadtree at the range of
  Design note 13, per view and style. Tests: the handover range as a pure function (about
  1.8 × 10⁷ m for 10 km of relief at 1080p and 1 px, and 4.6 × 10⁶ m at the wireframe's 4 px), and a
  body with no survey never hands over; per class law, the disc's and the terrain's I/F at the same
  (μ₀, μ) agree to 10⁻⁶ at 30° and 90° of phase, one row read by both. By hand, recorded: at phase
  angles of about 30° and 90°, the body's mean brightness across the handover changes by less than
  1/3 stop and, averaged over each class-map texel, by less than 1/3 stop on a fully surveyed
  generated world; the residuals of T10.a against plan 14's p and q. Acceptance:
  `pnpm --filter hyperion exec vitest run src/renderer/src/view/terrain` and the recorded check.
  _As built (Risks, "Deviations in T10.e, as built"): the hand-over is `view/terrain/handover.ts`,
  R07's regime `terrain` and an optional terrain input to the frame, measured to the body's limb;
  the terrain's class laws take the disc's figure scale; the live pipeline that would feed the
  input is not built (asked of main), so the check by hand ran scripted on the Moon-like fixture
  and stays pending on screen._
- **R10.T10.f The point under a class map.**
  - **What it does.** R07's point regime (`pointFlux` in `bodies/draw.ts`) integrates the body's
    class map wherever its `surface` is a class map and its `rotation` is known, so that the flux
    stays continuous at the 3 px switch (R07 Design note 5, T8.a's 1%; decision-r07-t8b).
    Elsewhere it keeps the photometry's law, which equals `elsewhere`.
  - **The integral.** It uses the disc's own law arithmetic in `f64`, summed over lit, visible
    surface elements in body-fixed axes: each element's shares, as T10.d reconstructs them, times
    each law's I/F, μ and the element's area, on the spheroid where the figure is one. Each light
    takes its eclipse term from the centre, as now. _R07.T10.b: the point now takes each light's
    `discEclipseVisible`, the eclipse averaged over its disc, not the centre's term, so that it
    meets the disc at 3 px through an eclipse (R07's "Deviations in T10.b, as built")._ Texels are
    sub-sampled as the bound needs.
  - **Caching.** The result may be cached on the sun's and the camera's body-fixed directions
    while it stays within 0.3% of a fresh integral.
  - **Files.** R07's `bodies/draw.ts` and `bodies/discSurface.ts`.
  - **Tests**, on a synthetic Iapetus (a 70° dark cap about the apex at A 0.04, the rest 0.55):
    - Facing the leading side, the trailing side and a pole, at 0°, 60° and 120° of phase, the
      point's flux equals the CPU rasteriser's summed disc flux at the switch to 1%.
    - It equals an `f64` brute-force integral over 2 × 10⁵ Fibonacci directions to 0.3%.
    - A one-class map equal to the uniform law gives the uniform point to 10⁻⁶.
    - A class map without a rotation gives the photometry's point exactly.
    - A cached value is within 0.3% of a fresh one.
  - **Acceptance.** `pnpm --filter hyperion exec vitest run src/renderer/src/view/bodies` and
    `just test-render`.
  - _As built (Risks, "Deviations in T10.f, as built")._

### R10.T11 The wireframe's terrain

Depth-only terrain in the wireframe style at τ = 4 px from the same bounds and bakes (heights and
mask only, the mask from the same `survey-mask` slot field as T10.c's), as one instanced draw, with
the contours of Design note 14 evaluated in the same pass,
the interval and index stated in the label block (`CONTOUR INTERVAL 200 m · INDEX 1 km`; the `DATUM`
line is T12's, in both styles) through R02's label API (as built `displays/view/viewRun.ts`'s
`labelStatements`, drawn by `ViewLabelBlock.tsx`), analytic line antialiasing and no MSAA, and the
patches under every grounded body in view at the finest level with morph held at zero, as in the lit
style (Performance budget, the second rule). Unsurveyed ground has no contours, and the survey edge
is drawn. Files: `terrainWire.ts`, `shaders/terrainWire.wgsl`. Tests (pure): `contourInterval` walks
the 1-2-5 sequence from the nadir's height above ground with ±20% hysteresis and never falls below 1
m; `contourPhase` with the patch-local height equals the `f64` contour levels to 1 mm at ±20 km;
each family is drawn in full at 6 px of spacing or more and fades to nothing at 3 px, in CSS pixels
times the stroke scale, minor before index; the index contour is 1.5 px; the wireframe's selection
is within a factor of two of `patchCountModel`'s ratio to the lit style, about a fifth; the
grounded-body rule holds in this style. Smoke run on SwiftShader. By hand, recorded: a station
wireframe at 1080p on the UHD 620, by the owner, during the low fast pass of the scripted descent
with the height workers busy, by `timestamp-query` with developer features, p50, p95 and p99 GPU
time per pass and the GPU clock from `intel_gpu_top`, with the patch and triangle counts, against
the brainstorm's 60 fps and its 4–9 ms for the whole view (Two deployments, one scene; the
research's provisional estimate is 3–6.5 ms with analytic antialiasing); and legibility on steep and
gentle terrain, including that the graticule is told from the contours, on a small body too. If it
is not, the graticule is left out over surveyed terrain, the finding is recorded, and the guide's
graticule sentence (the contours' bullet under "Views") is changed to match in the same commit
(signoff-9). Acceptance: `pnpm --filter hyperion exec vitest run src/renderer/src/view/terrain`, the
smoke run, and the figures recorded.

### R10.T12 Coverage and terrain readouts on the view

`SurveyLine` in the view's label block: the survey's coverage of the body and its finest resolution
(`SURVEY COVERAGE 12.4 % · FINEST 2.0 m`, after the star chart's census line), or
`SURVEY COVERAGE: PENDING`, `SURVEY COVERAGE NONE` or `TERRAIN: NOT YET MODELLED`, each value stale
with its `S` when the link is lost. The resolution at the line of sight (the brainstorm's "states
the survey's resolution beneath its centre") stands in the readout as `RESOLUTION`. The `DATUM`
line, with its `SEA LEVEL` on a world with a sea, is stated once in both styles. `TerrainReadout`,
under `GROUND AT LINE OF SIGHT`: `ELEVATION`, `SLOPE` with its `· BASELINE 2 m` part and
`SLANT RANGE` (`SLANT RANGE FROM CAMERA` whenever the camera is off the hull) from
`surveyed_reading` and `slant_range`, run in a worker at the guide's 4 Hz, in `output` elements on
their `--surface-0` plates, with `~` and `± …` until resolved, rounded by Design note 6's rule,
`NOT SURVEYED` once in place of the readings over unsurveyed ground, `NO GROUND` where the line of
sight misses the body's ground, `GRAZING` where it applies, the material as its form and its
substance (`MATERIAL REGOLITH · Fe-RICH BASALT`; the seabed's substance the floor's; the em dash for
a substance not computed), and `RESOLUTION`. The material's properties: `ULTIMATE BEARING CAPACITY`
with its `· PAD WIDTH 1.0 m`, from `bearing_capacity` at the body's surface gravity, with the slope
argument the later T1 follow-up gives it before this task builds the reading; and `PAD FRICTION`,
from `interface_friction`, with its `· AT` temperature where the friction depends on it. That
temperature is the climate field's mean for the month the view's time falls in, carried to the
point's elevation by the header's lapse rate. Also the line-of-sight mark, the tripod, with its
give-way and its list note `GROUND AT LINE OF SIGHT`. Both are built to the guide as signed off in
`signoff-10.md` (this task's drafts, below). The DOM list notes the survey edge while it is in view,
as `SURVEY EDGE` with its nearest range, a note that cannot be selected. The task also renames the
descent spike's `SITE HEIGHT` to `SITE ELEVATION`, in `ELEVATION`'s form, and makes the guide's row
edit in the same commit (signoff-9). Build to the guide's rows as signed off on 2026-10-13
(`signoff-9.md`) and 2026-10-14 (`signoff-10.md`), and make that file's guide edit (h), the
capacity's slope clause, in the commit that builds the reading. As built, the label block is
`viewRun.ts`'s `labelStatements` and `ViewLabelBlock.tsx`, and the DOM list `ViewMarkList.tsx` over
`viewRun.ts`'s `MarkRow`; the material is named by its form and its entry's substance through
P14.T49.f's `substanceName` (`lib/substances.ts`; the roadmap's Conventions), never by a registry
key. Files:

- `readouts.ts`, `SurveyLine.tsx`, `TerrainReadout.tsx` and their tests;
- `displays/view/viewRun.ts` and `ViewMarkList.tsx`;
- `view/wireframe/symbology.ts` and `view/wireframe/drawList.ts`, where the tripod is built and
  drawn first of the marks, beneath the reticles, and their tests;
- the survey edge's ticks (T10.c's `surveyEdges` and T11's edge in the draw list), which yield to
  the tripod, built by whichever of T10.c, T11 and T12 lands last, with its test;
- `lib/format.ts` (`formatBearingCapacity`, beside `formatPressure`) and its test;
- R05's binding, where T6.b's `material` face does not already answer the two properties and the
  temperature from the body's gravity, the pad width, the slope and the month;
- `view/spike/DescentSpike.tsx` and its test.

Tests (Vitest and Testing Library): the formats of T13's drafts for each state, `~2140 m ± 180 m`
among them; the PDG rounding at its breakpoints (354, 355, 949, 950), and the round-up to one step
where it would go finer than the quantity's precision (0.12 m → `± 0.2 m`); no value is shown finer
than its uncertainty; the field keeps its fixed width, and the value's unit its place, from
Estimated to resolved and stale; `SITE ELEVATION` in `ELEVATION`'s form; the two properties' lines
resolved, Estimated, stale and Missing, with and without `· AT`; `formatBearingCapacity`'s two
figures and its switch at 1000 kPa with hysteresis; each field's width and its unit's place from
Estimated to resolved and stale; `PAD FRICTION — · AT 102 K` on glacier ice of H2O at 102 K and
`PAD FRICTION —` on snow; the tripod's geometry (three ticks at 120°, one from below,
0.1875–0.4375 rem, moved out by the outline's shift); its leaving within 0.5 rem of a body's symbol,
a craft's ticks or the flight path marker, and its return only beyond 0.75 rem; the survey edge's
ticks absent within 0.5 rem of its ticks; no tripod at `NO GROUND` or without a readout; its note
first in the list, unselectable, with its accessible name; stale on link loss; keyboard reach of the
readout's target through the DOM list; the console-ux skill's lint, contrast and glyph checks pass,
with no `σ` anywhere (B612 has none); at a finest-level bake of a synthetic world whose patch holds
at least three classes, at every vertex, `dominantSlot` of the baked weights (`terrainMaterial.ts`,
T10.b) names the palette slot whose (form, entry) is the worker's `material` answer at that vertex,
ties broken as `dominantSlot` breaks them (main's ruling for R10.T10.b, 2026-10-15); between
vertices the texel blends and no claim is made. By hand, recorded: in both styles at a device-pixel
ratio of 1, at the 80% and 100% interface scales, the tripod's open centre, the tripod over the
survey edge, and the tripod beside the flight path marker at 0.5 rem; and at 1280×720 the label
block's longest lines, `ULTIMATE BEARING CAPACITY ~0.95 MPa S · PAD WIDTH 1.0 m` and
`PAD FRICTION ~0.08 S · AT 243 K S`. If the centre closes at either scale, the ticks start 0.25 rem
out, keeping their length, and the guide's figures change in the same commit (signoff-10).
Acceptance:
`pnpm --filter hyperion exec vitest run src/renderer/src/view/terrain` and the console-ux checks.

#### The drafts, for the owner

_Drafted by lane 11 (2026-10-13); signed off 2026-10-14 under the owner's delegation
(`signoff-10.md`): both drafts amended, and the guide edited as that file directs._ They are the two
drafts the task's sentence on the material's properties and the mark owes. Each gives its wording
for the guide, where it goes, and the guide rows it extends, by section heading and line in
`docs/frontend/ux-guidelines.md` as it stands at `fe12a05e`, with R10.T13's rows in it (signoff-9).
Each lists its questions with this draft's lean, and ends in its own marker. The guide is not edited
here: once the owner signs a draft off, it is applied as the owner directs, and the task builds to
it. Every string proposed passes the console-ux skill's glyph and lint scripts ("Checks", below).
Over a Moon-like regolith plain under a 410 m survey, with the default pad, the readout would read:

```text
GROUND AT LINE OF SIGHT
ELEVATION                 ~2140 m ± 180 m
SLOPE                     ~12° ± 4° · BASELINE 2 m
SLANT RANGE               ~41.2 km ± 0.3 km
MATERIAL                  REGOLITH · UNLISTED MATERIAL
ULTIMATE BEARING CAPACITY ~5.4 MPa · PAD WIDTH 1.0 m
PAD FRICTION              ~0.20
RESOLUTION                410 m
```

The substance is `feldspathic`'s, which `substanceName` does not name yet (R10.T13's Draft 5); the
capacity is lunar regolith's at the Moon's 1.62 m/s², and the friction NAVFAC's for silt (Draft 1).

- **Draft 1. The material's properties: `BEARING CAPACITY` and `PAD FRICTION`.**
  - _The quantities, as built_ (R10.T1.a; `crates/hyperion-surface/src/material/`, `bearing.rs` and
    `properties.rs`).
    - `bearing_capacity(properties, gravity, footing_width)` is the ultimate bearing capacity, Pa,
      under a square or circular footing of that width resting on level ground: Terzaghi's equation
      with Vesić's factors and shape factors, from the layer of the form's shear table at the depth
      of one footing width, at the body's gravity (Design note 2). Its test pins it on lunar
      regolith at 1.62 m/s² under a 1 m footing: 5,368.6 kPa from the 30–60 cm layer. The top
      15 cm's pair gives 206.5 kPa under the same footing, so a narrow pad, whose failure zone stays
      near the surface, bears far less: about 100 kPa under 0.1 m.
    - `interface_friction(properties, temperature)` is the friction coefficient tan δ of a metal
      pad on the ground, with no unit. On a soil it is a constant, NAVFAC DM-7.02 (1986) Table 1's
      factor for steel: 0.20 on regolith, soil and dust, 0.25 on sediment and 0.30 on sand. On
      water ice (a glacier, sea ice) it follows the temperature between Liefferink et al. 2021's
      points, from 176 K to 272 K (−97 °C to −1 °C, read by eye from their Fig. 1, as
      `properties.rs` records), 0.039 at its least, near 265 K, and 0.71 at 176 K. Outside them it
      is `None`, as it is on the colder ground of most icy bodies.
    - Either is `None` where the form's row has no source for it (`form_row`). The capacity is
      `None` on every form but regolith, dust and soil of silicate, the three with a shear table.
      The friction is `None` on every form but those three, sand and sediment of silicate, and
      glacier and sea ice of water ice: so on snow, a frost, bedrock, volcanic rock, melt, an
      evaporite, a seabed, an organic sediment, a regolith of ice, and every form of a metal, a
      salt or an organic (`FormRow::UNMODELLED`).
  - _Wording,_ sub-bullets of the terrain readout's bullet in "Views" (lines 700–708), whose list
    of readings becomes `ELEVATION`, `SLOPE`, `SLANT RANGE`, `MATERIAL`, `BEARING CAPACITY`,
    `PAD FRICTION` and `RESOLUTION`: "After `MATERIAL` stand two of the ground's properties, from
    its material and the body: what it will bear before it fails in shear, and how a landing pad
    grips it.
    - `BEARING CAPACITY` is the pressure under which the ground fails beneath a pad: its ultimate
      bearing capacity, with no factor of safety, at the body's surface gravity, on level ground.
      It is not a safe load: a pad sinks into the ground at far lower pressures, which are not
      modelled.
      It is on the console's pressure ladder, `Pa`, `kPa` and `MPa`, switching at 1000 with
      hysteresis, to two significant figures, and the pad it is taken under follows a middle dot:
      `BEARING CAPACITY 5.4 MPa · PAD WIDTH 1.0 m`. The pad is the own ship's; until the ship has
      pads it is 1.0 m.
    - `PAD FRICTION` is the coefficient of friction of a metal pad on the ground, a ratio with no
      unit, to two decimal places: `PAD FRICTION 0.20`. Where it depends on the ground's
      temperature, as an ice's does, the temperature it is taken at follows a middle dot, in whole
      kelvin: `PAD FRICTION 0.08 · AT 243 K`. That is the ground's temperature at the point for the
      month, from the survey's climate field; the day's cycle is not modelled.
    - Each is Estimated, its `~` without an uncertainty, since none is computed, until the survey
      resolves the point, as `ELEVATION` is: `BEARING CAPACITY ~5.4 MPa · PAD WIDTH 1.0 m`. On loss
      of the link each value the server's data gives goes stale with its `S`, the temperature among
      them, while the pad's width, a constant and not a live value, does not:
      `PAD FRICTION ~0.08 S · AT 243 K S`. Each value keeps a fixed field, sized for its longest
      Estimated reading with its `S`: `~990 kPa S`, `~0.71 S` and `272 K S`.
    - Where no source gives the figure for the ground's form, or the temperature lies outside its
      source's, the value is Missing, the em dash, since it is not computed: `BEARING CAPACITY —`,
      `PAD FRICTION —`. A part follows it only where the gap depends on it, as the friction of an
      ice colder than its source does on the temperature: `PAD FRICTION — · AT 102 K`.
    - Neither is ever `0` for a gap, and neither stands apart from the readings: `NOT SURVEYED` or
      `NO GROUND` stands once in place of them all."
  - _Rows_ (kind `Label`).
    - `BEARING CAPACITY`: in a view's terrain readout, the pressure under which the ground at the
      point fails beneath a pad of the stated `PAD WIDTH`: its ultimate bearing capacity, with no
      factor of safety, at the body's surface gravity, on level ground, since a slope's lowering of
      it is not modelled. Not a safe load: a pad's sinking, at far lower pressures, is not
      modelled either. Two significant figures in `Pa`, `kPa` and `MPa`, switching at 1000 with
      hysteresis (`BEARING CAPACITY 5.4 MPa · PAD WIDTH 1.0 m`); Estimated until the survey
      resolves the point; the em dash where no source gives it for the ground's form. Spelled out,
      and never `BEARING` alone, which the guide's prose keeps for a direction.
    - `PAD WIDTH`: after `BEARING CAPACITY` alone, the width of the pad it is taken under, a square
      pad's side or a round pad's diameter, to two significant figures: the own ship's pads', or
      1.0 m until the ship has pads. A constant, never stale.
    - `AT`: after `PAD FRICTION` alone, the ground's temperature the friction is taken at, the
      month's at the point from the survey's climate field, in whole kelvin (`· AT 243 K`).
    - `PAD FRICTION`: in a view's terrain readout, the coefficient of friction of a metal landing
      pad on the ground at the point (its sources measured steel): the ratio of the force that
      slides the pad to the load on it, with no unit, to two decimal places (`PAD FRICTION 0.20`).
      After a middle dot, `AT` and the ground's temperature, where the friction depends on it
      (`· AT 243 K`). Estimated until the survey resolves the point; the em dash where no source
      gives it for the ground's form or its temperature. Spelled out, never `μ` or `tan δ`, which
      neither face holds.
  - _The lines by state._
    - Resolved: `BEARING CAPACITY 5.4 MPa · PAD WIDTH 1.0 m`, `PAD FRICTION 0.20` and, on glacier
      ice of H2O at 243 K, `PAD FRICTION 0.08 · AT 243 K`.
    - Estimated: `BEARING CAPACITY ~5.4 MPa · PAD WIDTH 1.0 m`, `PAD FRICTION ~0.20`.
    - Stale: `BEARING CAPACITY ~5.4 MPa S · PAD WIDTH 1.0 m`, `PAD FRICTION ~0.08 S · AT 243 K S`.
    - Not computed: `BEARING CAPACITY —` on sand or ice; `PAD FRICTION —` on snow;
      `PAD FRICTION — · AT 102 K` on glacier ice of H2O at 102 K, as on a Europa.
    - Not surveyed, no ground: neither line, the readout's one status standing for every reading.
  - _The figures behind the examples_, from the formula, computed apart from the code in Python's
    `math` (2026-10-13), each checked again by the lane's science check: lunar regolith at
    1.62 m/s², 5,370 kPa under a 1.0 m pad (`5.4 MPa`) and 104 kPa under 0.1 m (`100 kPa`); Mars's
    drift at 3.71 m/s², of bulk density 1,200 kg/m³, 39 kPa under 1.0 m, and Viking Lander 2's
    crusty to cloddy soil, of 1,400 kg/m³, 301 kPa (`300 kPa`), both from the top 0.22 m, the depth
    Viking sampled, taken deeper, as `shear_layer_at` takes its deepest layer below it; glacier ice
    of H2O at 243 K, 0.080, between Liefferink et al.'s points at −38 °C and −30 °C.
  - _Sources for the wording._ "Ultimate" and "with no factor of safety" describe what `bearing.rs`
    computes: the pressure at which the ground fails in shear, nothing divided out. The Lunar
    Sourcebook keeps the two apart: the allowable bearing capacity, limited by settlement, "is
    significantly less than the ultimate bearing capacity" (Carrier, Olhoeft and Mendell 1991,
    §9.1.9, pp. 517–519, as the lane's science check read it). The 1.0 m pad is the footing the
    Sourcebook gives its lunar figure for, about 6,000 kPa (p. 517), and about the width of the
    Apollo 11 lunar module's footpads, nearly 1 m across, under which it reads 3,000–11,000 kPa
    (p. 518), both as `bearing.rs` gives them. The temperatures are the guide's kelvin (lines
    234–239): the source's −97 °C and −1 °C, read by eye to about 2 °C, are 176 K and 272 K. A
    ratio with no unit is shown bare, as `ECC` is (line 938; `BodyRecordReadings.tsx`).
  - _Goes in_ "Graphs, schematics and spatial displays", "Views": the terrain readout's bullet
    (lines 700–708) takes the two readings into its list and the wording as its sub-bullets; the
    rows go after the material form row (line 1050).
  - _Extends:_
    - "Views", lines 700–708 (the readout, its order, `NOT SURVEYED`, `NO GROUND` and staleness);
    - "Voice and nomenclature": lines 822–823 (one name per thing; an abbreviation only from the
      list), 858–877 (the material reading these follow), 878 (a reading of parts joined by `·`)
      and 879–880 (real engineering terms: geotechnics' ultimate bearing capacity and interface
      friction);
    - "Typography", lines 134–141 (no Greek letter; a quantity named in words, so never `μ` or
      `tan δ`);
    - "Numbers, units and time": lines 188–193 (a unit with every value, which a ratio has not, as
      `ECC`, line 938), 218–219 (one unit and precision for a quantity; the precision the operator
      can act on), 234–239 (kelvin, a source's degrees Celsius converted), 240–241 (scaled units
      with hysteresis) and 253 (a fixed field width with its status marks);
    - "Data states": the Stale, Missing and Estimated rows (lines 359, 360 and 362), lines 366–367
      (a lost link makes every live value stale) and 414–415 (a single value not computed, inside a
      section that is shown, is the em dash);
    - "Principles", item 5 (lines 26–28: never a number the simulation does not have, never a
      hidden estimate or gap);
    - the rows `MATERIAL` (line 1049), the material forms (line 1050), `SLOPE` and `BASELINE`
      (lines 1061–1062: a condition after a middle dot, the console's own setting never stale),
      `NOT SURVEYED` (line 1066) and `NO GROUND` (line 1067).
  - _Amends_ the `BASELINE` row (line 1062), which reads "`2 m` by default (the band limit and a
    pad's scale)": with `PAD WIDTH 1.0 m` in the same readout, "pad" would name two sizes. Its
    aside becomes "(the band limit, and the scale a lander's tilt is judged over)", after Design
    note 7's InSight ladder.
  - _Excepts_ nothing. The Estimated value's rounding (lines 220–233) needs a `±`, and these have
    none.
  - _Questions for the sign-off_, each with its lean.
    - _Estimated under a coarse survey._ The material is quoted at coverage alone (signoff-9),
      since it is a name; these are numbers, and Design note 6 gates every number by coverage and
      resolution. Lean: `~` until the point is resolved, then the plain value. Resolved, it is not
      an estimate of the simulation's quantity but the very figure landing reads (the brainstorm's
      properties "that landing and the consoles read"), though its table's figures are generic for
      the form. The alternatives are the material's state, never `~`, and `~` always, which would
      mark that genericness (the Estimated row's "derived", line 362).
    - _A figure not computed._ Lean: the em dash, as the guide's lines 414–415 have a single value
      the generator does not compute, and as signoff-9 set a substance not computed
      (`MATERIAL REGOLITH · —`). The alternative, `NOT YET MODELLED` in the value's place, is the
      guide's form for a section, and would stand on most icy ground.
    - _Precision._ Lean: two significant figures for the capacity, since its source holds it only
      to within a factor of about two (the Sourcebook's 3,000–11,000 kPa about its 6,000), on the
      console's one pressure ladder (`lib/format.ts`'s `formatPressure`), whose three figures are
      a surface pressure's; the friction to two decimal places, as NAVFAC states it.
    - _"Ultimate" in the label._ Lean: `BEARING CAPACITY`, the row saying it is the ultimate, with
      no factor of safety. The alternative is `ULTIMATE BEARING CAPACITY`, three words.
    - _The pad._ The own ship's hull is not defined (`TEST HULL`, line 1034), so it has no pads.
      Lean: a fixed 1.0 m, stated on the line, with no selector. The alternative is a `PAD WIDTH`
      selector, as `BASELINE` has one, which would meet the steps of Risks' deferred finding.
    - _The temperature._ Lean: the climate field's month at the point (R09's `ClimateCell`, taken
      to the point's elevation), stated only where the friction depends on it. Stated on every
      friction, it would suggest that a soil's constant varies with it.
    - _Slope._ The capacity is a level footing's on level ground; on a slope it is lower (Vesić's
      ground-inclination factors, (1 − tan β)² on the self-weight term for a slope β below φ, in
      EM 1110-1-1905's Table 4-6, p. 4-9, the table `bearing.rs` takes its shape factors from, as
      the lane's science check read it; about 0.62 at 12°), which T1.a's function does not take.
      Lean: say so in the row, and leave T1.a as built. _Ruled 2026-10-14 by main, on the issue:
      the lean. The row
      says so as drafted, and T1.a stays as built. Vesić's ground-inclination factors, with the
      footing-base tilt factors that go with them, are a deferred finding for a later T1
      follow-up, which adds the slope argument and its test before this task builds the readout
      (Risks)._
    - _The friction's kind._ NAVFAC's are unfactored friction factors at the onset of sliding, and
      Liefferink et al.'s a slow kinetic friction under a model skate's blade (`properties.rs`);
      neither was measured under a pad. Lean: one label, its row naming no kind and saying that
      its sources measured steel, since the two kinds cannot be told apart on one line without a
      word the console would have to add for this reading alone.
  - _Signed off (signoff-10): amended._

- **Draft 2. The line-of-sight mark.**
  - _The marks it must not resemble._ The guide names the ship-wide symbol set, ten closed
    outlines (lines 763–782), among them the feature's Greek cross (lines 784–790); the selection's
    bracket reticle, four corners of a square in `--accent`, and the destination's four chevrons
    in `--target`, each pointing at the mark (lines 557–567 and 613–616, rows 1069 and 1155); a
    craft's four open cardinal ticks outside its mark (`TICKS TARGET`, row 1068); the survey edge's
    ticked line (lines 715–718); a feature's circle at its true radius (lines 786–787); an apparent
    position's tick and dashed line (lines 527–528); the off-scale arrowhead (lines 358 and 511);
    the drawn unit glyphs, `☉` a circle with a dot, `⊕` a circle with a cross, and `⌘` (lines
    160–167); and on a spatial display the axis triad's three arrows and the range rings (lines
    568–572). The client draws three more the guide does not name: the view's flight path marker,
    a circle 0.375 rem in radius with two level wings and an upright fin in `--text`
    (`view/wireframe/symbology.ts`, R02.T12.c), and on the galaxy map the cursor, an upright cross
    with a gap at its point in `--accent`, and the chart centre, a small diagonal cross in `--text`
    (`displays/galaxy/GalaxyMapView.tsx`). The reticles, a craft's ticks and the crosses have
    four-fold symmetry, every symbol is closed, and the crosses and the flight path marker are what
    a centre mark would most likely copy.
  - _Where the point falls._ The view's projection is centred on its line of sight
    (`view/camera/projection.ts`), so the point is always the picture's centre, where
    `PREVIOUS TARGET` and `NEXT TARGET` also set an aimed target (line 1089). So the mark shares its
    place with an aimed target's reticles and, whenever the view looks along the ship's velocity,
    as on a straight-in descent, with the flight path marker, which stands where the velocity
    projects.
  - _Wording,_ a bullet of "Views" after the terrain readout's (line 708): "Where the view's line of
    sight meets the ground its terrain readout reads, the view marks the point, in both styles,
    with a tripod: three short straight ticks pointing at the point, one from directly below it and
    two from above, each 60° from the upward vertical, the point itself left open. No other mark
    is three open ticks about an open point: the reticles, a craft's ticks and the crosses are
    four-fold; the triangles and the hexagon, which share its symmetry, are closed outlines; and a
    spatial display's axis triad meets at its origin, with arrowheads and labels. So the tripod is
    never read as a reticle, a craft or a body.
    - _Size and stroke._ Its ticks reach 0.375 rem from the point, the radius of an unresolved
      contact's symbol, so that it stands inside the selection's bracket about any body as a
      symbol does, and start 0.125 rem from it, each 0.25 rem long, as a craft's tick is. They are
      drawn at the `1.5px` outline of a mark sized in `rem`, moved out as a symbol's line is so
      that the centre stays open (see "Layout"), with their `--surface-0` casing in both styles.
    - _Colour._ `--text`, as the flight path marker and a craft's ticks are: it marks a reading,
      and is neither available nor selected (`--accent`), nor commanded (`--target`), nor an alert.
    - _Place._ It stands where the line of sight falls, at the picture's centre, and is never
      moved, since a moved mark would mark ground the readout does not read; so it never nears the
      view's edge. Where the view's chrome covers the centre, a label block's plate or an
      instrument's slot over the `PRIMARY` view, it is covered with the picture. No mark's label is
      placed over it: a label that would cover it stands on another side of its mark, or is not
      shown, as one the chrome would cover.
    - _Beside other marks._ It is drawn over the ground's own lines, the contours, the graticule
      and the survey edge, whose line its casing cuts, so that its ticks stay whole and are never
      read as the edge's, which hang from the line on one side alone. It is drawn beneath the
      reticles: the selection's bracket and the destination's chevrons about an aimed body stand
      round it. Where a body's symbol, a craft's ticks or the flight path marker come within
      0.25 rem of its ticks, it is left out, since that mark then stands where the line of sight
      is, and the two together would read as one new symbol, a circle or a triangle with spokes.
    - _When it is drawn._ It is drawn with the readout, over surveyed ground and over ground the
      ship has not surveyed alike, where the readout reads `NOT SURVEYED` of the point it marks. It
      is unchanged under `GRAZING`: its place is exact, and what is uncertain is how far along the
      line of sight the ground lies, which the readout states. At `NO GROUND` it is not drawn, there
      being no point, nor where the view states no terrain readout. It comes and goes with the
      readout's own updates, as a cut, so that the mark and the readout never disagree.
    - _While stale._ Unchanged: it marks the picture's centre, the point the readout reads from the
      same pose, whatever that pose's age, and staleness is carried by the readings' `S` and
      `POINTING`'s, never by a mark's colour.
    - _In the list._ While the readout has a point, drawn or left out for another mark, the
      view's list notes it as `GROUND AT LINE OF SIGHT`, the first of its notes, before
      `SURVEY EDGE` and `INTEGRATED STARLIGHT`. Its accessible name gives its place, "Ground at
      line of sight, at the view's centre". It is a note outside the list's options: not a
      contact, it cannot be selected, and it carries no range, since the readout's `SLANT RANGE` is
      the point's."
  - _Rows._
    - `TRIPOD GROUND AT LINE OF SIGHT`, kind `Legend`, after `CHEVRONS DESTINATION` (line 1069):
      "A view's mark where its line of sight meets the ground its terrain readout reads: three open
      ticks pointing at the point, one from below and two from above, 120° apart. Four ticks mean
      another craft (`TICKS TARGET`), and corner brackets the selection (`BRACKET SELECTED`)."
    - `GROUND AT LINE OF SIGHT` (line 1059) gains: "Also the view's list note of the point's mark
      (`TRIPOD GROUND AT LINE OF SIGHT`), which cannot be selected."
  - _Contrast._ `contrast.py text surface-0` gives 13.57:1, above the 6:1 a stroke that carries
    meaning needs, and the view's black ground is darker than `--surface-0`. At the floor of 2
    device px a view's stroke wholly covers its brightest pixel (lines 75–76), so the pair holds
    as scored.
  - _Goes in_ "Graphs, schematics and spatial displays", "Views", after the terrain readout's
    bullet (line 708) and before the survey edge's (line 709); the rows as above.
  - _Extends:_
    - "Colour": the token table (lines 45–59), lines 62–65 (6:1), 66–82 (a stroke as drawn,
      against its casing over an image), 83 (never by colour alone: the shape and the list's note
      carry it) and 106–110 (every mark over the image cased);
    - "Layout", lines 301–309 (widths times the larger of the device-pixel ratio and 2; a mark
      sized in `rem` at no less than 2 device px; an open mark keeps its hole);
    - "Graphs, schematics and spatial displays": lines 520–523 (a small fixed symbol set, shape for
      type), 557–567 (the bracket and the chevrons, told apart by shape) and 576–577 (the canvas
      paired with a DOM list);
    - "Views": lines 608–616 (what a view keeps of the symbol set and reticles), 625–626 (its DOM
      list), 660–674 (both styles; casings; a label never over its mark's reticle, never shown in
      part; changes as cuts), 676–680 and the `INTEGRATED STARLIGHT` row (line 1079: a list note
      that is not a contact), 700–708 (the readout's point, moved by the camera, never by a
      pointer) and 709–725 (the survey edge's mark and its note);
    - "Motion and sound", lines 800–802 (readouts at about 4 Hz) and 805–812 (a view never stops;
      cuts);
    - the rows `GROUND AT LINE OF SIGHT` (line 1059), `SURVEY EDGE` (line 1055), `TICKS TARGET`
      (line 1068), `CHEVRONS DESTINATION` (line 1069), `PREVIOUS TARGET` and `NEXT TARGET` (line
      1089), and `BRACKET SELECTED` (line 1155).
  - _Questions for the sign-off_, each with its lean.
    - _The tripod's way up._ Lean: one tick below and two above, with no strong reason between the
      two: either way up its ticks point along one triangle's vertex directions (the planet's
      here, the neutron star's inverted), both closed outlines, and where the flight path marker or
      a contact comes close, it gives way either way up. Inverted is the alternative.
    - _Over unsurveyed ground._ Lean: drawn, since `NOT SURVEYED` is the readout's reading of that
      point, and the operator steers by the mark onto surveyed ground. Left out, the mark would
      come to mean "surveyed".
    - _A contact on the point._ Lean: left out within 0.25 rem of it, its note standing. Drawn
      beneath, a craft's lower tick would continue the tripod's, and a body's outline would close
      round it as a new symbol.
    - _The flight path marker on the point._ Lean: left out within 0.25 rem of it, as for a
      contact, since the tripod's ticks, reaching 0.375 rem, would end on the inside of the
      marker's circle, 0.375 rem in radius, and the two would read as a circle with three spokes;
      the note stands, and on a straight-in approach the marker itself then shows where the line of
      sight meets the ground. The alternatives are to draw the tripod beneath it, or to set its
      ticks outside the marker's circle, which would make it the largest point mark on the view.
    - _The survey edge through the point._ The tripod's ticks are the edge's length and colour
      (0.25 rem, `--text`). Lean: the tripod over the edge, its casing cutting the edge's line, as
      drafted. The alternative is ticks of another length, so that the two differ by length too.
    - _The note's range._ Lean: none, since the readout's `SLANT RANGE` is the point's, from the
      camera, and a second range, from the own ship as the list's are, would give two figures for
      one point. The alternative is `SURVEY EDGE`'s, measured as the list's ranges are.
    - _Labels clear of it._ Lean: as drafted, which extends the label rule (lines 669–674) from a
      mark's own reticle to this mark.
    - _Its name._ Lean: `TRIPOD`, a shape's word as `TICKS`, `CHEVRONS` and `BRACKET` are, joined
      to the readout's heading so that the point keeps one name.
  - _Signed off (signoff-10): amended._

- **Followed from the sign-off** (`signoff-10.md`):
  - this task's files, tests and by-hand checks, now in its text above;
  - the `ULTIMATE BEARING CAPACITY` row's level-ground clause, replaced by signoff-10's (h) in the
    commit that builds the reading on the T1 follow-up's slope argument;
  - the roadmap README's "Awaiting the owner" (signoff-10, (p6));
  - and a guide row for the flight path marker, which the client draws and the guide does not name
    (Risks, "Deferred findings (2026-10-13 rule) (R10.T12's drafts)").

- **Checks** (2026-10-13, this worktree).
  - `glyphs.py "· — ~ ± °" '\u00a0'`: every character outside ASCII the drafts put on screen,
    with the tilde and the no-break space of the `±` row (exit 0):

    ```text
    char  code     B612   Mono   name
    ·     U+00B7   yes    yes    MIDDLE DOT
    —     U+2014   yes    yes    EM DASH
    ~     U+007E   yes    yes    TILDE
    ±     U+00B1   yes    yes    PLUS-MINUS SIGN
    °     U+00B0   yes    yes    DEGREE SIGN
          U+00A0   yes    yes    NO-BREAK SPACE
    ```

  - `glyphs.py "μ δ"`: the friction's symbols, which the drafts name in words instead (exit 1):

    ```text
    char  code     B612   Mono   name
    μ     U+03BC   no     no     GREEK SMALL LETTER MU
    δ     U+03B4   no     no     GREEK SMALL LETTER DELTA
    ```

  - `ux_lint.py` on a scratch `.ts` file, outside the worktree and not committed, holding every
    string the drafts propose as a string literal (exit 0), after a control string in a second
    file there (`"Please wait, loading the PAD FRICTION"`) drew its `[voice]` error, so the file
    was read:

    ```text
    0 error(s), 0 check(s) in 1 file(s) scanned. Heuristic: confirm each line against the guide.
    ```

  - `contrast.py text surface-0` (exit 0):

    ```text
    text on surface-0: 13.57:1 (minimum 6:1) PASS
    ```

  - _Reviews._ The `ux-reviewer` agent found nothing must-fix, and five should-fix points and six
    nits, all applied: the three-fold claim, the flight path marker's give-way, the staleness
    reason, the `~` question's third alternative, `BASELINE`'s pad, the row's ladder, the pad's
    staleness, `AT`'s row, the field widths, the survey edge, and two citations. The
    `science-checker` agent found nothing must-fix, and six should-fix points and five nits, all
    applied: "what it will bear", the slope factors' table, the forms without figures, Viking's
    depth, the friction's kinds, the flight path marker's place, and the figures' rounding. It
    could not reach NAVFAC DM-7.02 or Vesić's primaries, which stand as `properties.rs` and
    `bearing.rs` cite them.

### R10.T13 Guide drafts for the owner

Drafts, in one pass, of what this plan adds to R02's nine items: `±` as one standard deviation of an
Estimated value, written `~2140 m ± 180 m` with the unit on both numbers and a space either side of
`±` (NIST SP 811 §7.7, verified; `±` is in B612 and B612 Mono, `σ`, `≈` and `Δ` are not, checked
with the console-ux skill's glyph script), as a nomenclature entry; an exception to "the same
quantity uses the same unit and precision everywhere" for an Estimated value rounded to its
uncertainty, its field keeping its fixed width; the readout names `ELEVATION` (spelled out, since
`ELV` means a spatial display camera's elevation, an angle, and the brainstorm's `ELEV` would be a
second abbreviation of the word), `SLOPE` with its `· 2 m BASELINE` part, `SLANT RANGE` (`RANGE` is
taken by the chart's drive range), `DATUM`, `GRAZING` and `NOT SURVEYED`, after ICAO's distinction
of elevation, altitude and height; the survey line's wording; the material reading's wording (its
form and its substance, named by P14.T49.f's `substanceName`); the survey edge as a mark in both
styles; the contour widths and tokens, `CONTOUR INTERVAL`, `INDEX`, and the statement that minor
contours are omitted where closer than 4 px, as guide item 5's substitute for a scale bar. The
client is built to the drafts, as the galaxy plans' slice was. Files: a draft section in this task's
entry, not the guide. Acceptance: the drafts are written and **the owner signs off**, after which
the guide edit is made as the owner directs.

#### The drafts, for the owner

_Drafted by lane 5 (2026-10-13); signed off 2026-10-13 under the owner's delegation
(`signoff-9.md`): draft 6 adopted, drafts 1–5 and 7 amended, and the guide edited as that file
directs._ Each draft gives its wording for the guide, where it goes, and the guide rows it extends
or excepts, by section heading and line in `docs/frontend/ux-guidelines.md` as it stands at
`be8f8a6f`. Each ends in its own marker. The guide is not edited here: once the owner signs a draft
off, it is applied as the owner directs, and T11 and T12 are built to it. The drafts extend R02's
item 1 (Views), item 3 (outlines for symbology), item 5 (scale without a scale bar) and item 7
(limited detail). They also fill item 2's honesty rule, whose "survey coverage" Draft 4 supplies.
Every string proposed here passes the console-ux skill's glyph and lint scripts ("Checks", below).
Over a surveyed plain, a label block would show them as below, the substance as `substanceName`
names it today (Draft 5), and the contour line in the wireframe style alone:

```text
SURVEY COVERAGE  12.4 % · FINEST 2.0 m
DATUM            REFERENCE SPHERE 6371.0 km · SEA LEVEL -2.4 m
CONTOUR INTERVAL 200 m · INDEX 1 km
GROUND AT LINE OF SIGHT
ELEVATION        ~2140 m ± 180 m
SLOPE            ~12° ± 4° · BASELINE 2 m
SLANT RANGE      ~41.2 km ± 0.3 km
MATERIAL         SOIL · UNLISTED MATERIAL
RESOLUTION       410 m
```

- **Draft 1. `±`, one standard deviation of an Estimated value** (a nomenclature row).
  - _Row._ `±`, kind `Notation`: "After an Estimated value, its uncertainty: one standard
    deviation of it, the unit written on both numbers and a space either side of the sign,
    `~2140 m ± 180 m`. The figure after `±` is a standard deviation and not a confidence interval;
    it is never to be read as a 68 % interval, a tolerance or a limit, and never as LE90 or a 95 %
    figure. It stands only with the `~`, and both go when the value is resolved. The spaces in the
    reading are no-break, U+00A0, so that it never breaks across a line. Both numbers are rounded
    as the Estimated value's exception gives. `σ` is never shown, nor `≈` or `Δ`, since neither
    B612 face has them; `±`, from Latin-1, is in both."
  - _Sources._ The form is NIST SP 811 (2008) §7.7, "Clarity in writing values of quantities":
    "63.2 m ± 0.1 m or (63.2 ± 0.1) m but not: 63.2 ± 0.1 m or 63.2 m ± 0.1". SP 811 does not say
    what the figure after `±` means. The meaning, and the caveat the row states, are the GUM's
    (JCGM 100:2008 §7.2.2, item 4): "where the number following the symbol ± is the numerical value
    of (the combined standard uncertainty) uc and not a confidence interval". Its NOTE adds that
    "The ± format should be avoided whenever possible", since it "may be confused with expanded
    uncertainty". The console keeps `±` because B612 has no `σ`. The GUM's concise form,
    `2140(18) m`, would set parentheses, which the guide keeps for a symbol's subscript and a
    formula's group (lines 141, 213–216, 752–754). The nomenclature list defines the figure once,
    as §7.2.2 allows ("if uc is defined elsewhere in the document"). The figure is a 1σ RMS, not
    LE90, by Design note 6; and since the unresolved heights are not Gaussian, a 68 % reading would
    be wrong too. The uncertainty is a quantity of its own, not the far end of a span (line
    227–229), so it carries its own unit.
  - _Glyphs._ `glyphs.py "± σ ≈ Δ"`, run 2026-10-13 in this worktree (exit 1, for the three
    missing):

    ```text
    char  code     B612   Mono   name
    ±     U+00B1   yes    yes    PLUS-MINUS SIGN
    σ     U+03C3   no     no     GREEK SMALL LETTER SIGMA
    ≈     U+2248   no     no     ALMOST EQUAL TO
    Δ     U+0394   no     no     GREEK CAPITAL LETTER DELTA
    ```

  - _Goes in_ the nomenclature list (from line 776), before the readout names of Draft 3.
  - _Extends_ "Data states", the Estimated row (line 347, "Prefixed with `~`"), which gains its
    uncertainty; "Numbers, units and time", line 187–188 (a unit on every value, with a space) and
    line 227–229 (a span takes one unit; an uncertainty is no span); "Typography", lines 127–137
    (`±` among Latin-1's signs; Greek letters, `≈` and `∆` among what neither face holds).
  - _Signed off (signoff-9): amended._

- **Draft 2. The Estimated value rounded to its uncertainty** (an exception).
  - _Wording,_ after "The same quantity uses the same unit and precision everywhere on the ship":
    "The one exception is an Estimated value stated with its uncertainty (`±`). It is rounded to
    the uncertainty, not to the quantity's precision, so that it shows no digit the measurement
    does not hold. The uncertainty's three leading digits decide: 100 to 354 keep two significant
    figures, 355 to 949 keep one, and 950 to 999 round up to 1000 and keep two. The value is
    rounded to the same decimal place: `~2140 m ± 180 m`, `~2100 m ± 500 m`, `~2100 m ± 1000 m`.
    Neither number is shown finer than the quantity's own precision. An uncertainty that would
    round below one step of it is shown as one step: `~2143.7 m ± 0.1 m`, at an elevation's 0.1 m.
    The field keeps its fixed width, sized for the longest Estimated form with its `~`, sign,
    digits, `±` and uncertainty. The reading stands right-aligned in it, so that the value does not
    move as the survey resolves it. Once resolved, the value takes the quantity's own precision,
    and the `~` and the `± …` go."
  - _Source._ The rule is the Particle Data Group's (Review of Particle Physics 2024, Introduction,
    §5.3 "Rounding"; Design note 6): "if the three highest order digits of the error lie between
    100 and 354, we round to two significant digits. If they lie between 355 and 949, we round to
    one significant digit. Finally, if they lie between 950 and 999, we round up to 1000 and keep
    two significant digits. In all cases, the central value is given with a precision that matches
    that of the error." The floor of one step is this draft's own.
  - _Goes in_ "Numbers, units and time", as the bullet after line 217–218.
  - _Excepts_ "Numbers, units and time", line 217–218 ("The same quantity uses the same unit and
    precision everywhere on the ship"). _Keeps_ line 238 ("A field has a fixed width sized for its
    longest possible value and its status marks") and "Typography", line 183 (numbers
    right-aligned).
  - _Signed off (signoff-9): amended._

- **Draft 3. The readout names.**
  - _The vertical distances._ The names follow ICAO's distinction of three vertical distances. It
    is ICAO Doc 4444, PANS-ATM, 16th edition (2016), Chapter 1, "Definitions", which carries all
    three; Annex 2, 10th edition (2005), Chapter 1, defines altitude and height but not elevation.
    The definitions are as follows.
    - An altitude is "The vertical distance of a level, a point or an object considered as a
      point, measured from mean sea level (MSL)."
    - An elevation is "The vertical distance of a point or a level, on or affixed to the surface of
      the earth, measured from mean sea level."
    - A height is "The vertical distance of a level, a point or an object considered as a point,
      measured from a specified datum."

    A body has a stated `DATUM` and often no sea. So the ground's distance above the datum is its
    `ELEVATION`: ICAO's elevation, with the datum in place of mean sea level. Planetary cartography
    measures the same way. The IAU's working group uses a reference surface "for measuring
    geometric height (elevation)" (Archinal et al. 2018, Celest. Mech. Dyn. Astron. 130:22, §7).
    LRO LOLA's gridded topography reads "HEIGHT, i.e. elevation in meters" above a reference sphere
    (`LDEM_4.LBL`). A craft's or camera's distance above the ground, or above the datum, is a
    height (`HEIGHT ABOVE SITE`, line 976). No reading here is an altitude, which ICAO keeps for
    mean sea level.

  - _The point._ A view's terrain readout reads the ground where the view's line of sight first
    meets it. It reads no pointer cursor, since nothing may depend on hover. The camera's own keys
    and drag move the point, and `PREVIOUS TARGET` and `NEXT TARGET` aim it. On the label block,
    the readings stand under the heading `GROUND AT LINE OF SIGHT`, in the order `ELEVATION`,
    `SLOPE`, `SLANT RANGE`, `MATERIAL` (Draft 5) and `RESOLUTION` (Draft 4), at the guide's 4 Hz.
  - _Rows_ (kind `Label`, but `GRAZING`, `NOT SURVEYED` and `NO GROUND`, which are kind `Status`):
    - `GROUND AT LINE OF SIGHT`: the heading of a view's terrain readout. It reads the ground where
      the view's line of sight first meets it. Stated while the view's `FRAME` is a body's and the
      body has a surface.
    - `ELEVATION`: the ground's height above the body's `DATUM`, along its normal, at that point.
      It is in metres at every value, to 0.1 m, with `-` below the datum and no `+` above it, as a
      map's elevations are (`2143.7 m`, `-410.0 m`). It is Estimated, with its uncertainty, until a
      close survey resolves the point (`~2140 m ± 180 m`). Spelled out: `ELV` is a spatial display
      camera's elevation, an angle, and `ELEV` would be a second abbreviation of one word. Never
      `ALTITUDE` or `HEIGHT`, which ICAO gives to other distances; `HEIGHT` is also the `GALACTIC`
      frame's +z reading.
    - `SLOPE`: the ground's slope at that point over a stated baseline. It is the adirectional
      slope that tilts a lander: the gradient's magnitude from central differences along local
      east and north (Design note 7). In whole degrees, with its baseline after a middle dot:
      `SLOPE 7° · 2 m BASELINE`. While the point is not resolved, it is the RMS slope, Estimated,
      with its uncertainty: `SLOPE ~12° ± 4° · 2 m BASELINE`.
    - `BASELINE`: the horizontal distance a `SLOPE` is taken over: `2 m` by default (the band limit
      and a pad's scale), `5 m` or `100 m` (Design note 7). Also the readout's selector of it, a
      display control.
    - `SLANT RANGE`: the distance from the camera, along its line of sight, to the ground it first
      meets. It is in `m` below 1 km, then in `km` and `Mm` as a view's ranges are, to three
      figures, with hysteresis (`SLANT RANGE 412 m`). It is Estimated, with its uncertainty, while
      the survey has not resolved the ground there (`~41.2 km ± 0.3 km`), or carries `GRAZING` in
      the uncertainty's place. It is measured from the camera, so whenever the camera is off the
      hull it reads `SLANT RANGE FROM CAMERA`. Never `RANGE`: the chart's curve labels take it for
      the drive range, and a view's list heads its own column with it. Nor `DIST`.
    - `GRAZING`: after a `SLANT RANGE` and a middle dot, where the line of sight meets the ground so
      obliquely that relief finer than the survey could stand in front of the point. That is where
      the grazing angle's sine is under twice the unresolved slope's standard deviation over the
      survey's cell (Design note 7). The range is Estimated and stands without an uncertainty,
      since the true range may be shorter by more than one: `SLANT RANGE ~41.2 km · GRAZING`.
      Never on a resolved range.
    - `DATUM`: the surface every terrain height on a view is measured from, along its normal. It is
      the body's rotational spheroid, with its equatorial and polar radii to 0.1 km and the unit on
      both (`DATUM REFERENCE SPHEROID 6378.1 km × 6356.8 km`). Until the body's flattening is sent
      it is a sphere, with its radius (`DATUM REFERENCE SPHERE 6371.0 km`). It is stated once, on
      the label block, in either style, while the readout stands. On a world with a sea it is
      followed, after a middle dot, by its `SEA LEVEL`, a height above the datum in `ELEVATION`'s
      form (`· SEA LEVEL -2.4 m`).
    - `NOT SURVEYED`: where the ground at the point has not been surveyed, `NOT SURVEYED` stands
      once in place of the readout's readings, under its heading. It is steady, in `--text`, as
      `NOT RESOLVED` stands in place of a section the granted detail withholds. Never "unknown" or
      "no data". The view draws no terrain and no contours there, and the body's reference surface
      stands in its place (Draft 6).
    - `NO GROUND`: where the line of sight meets no ground within the readout's reach, `NO GROUND`
      stands once in place of the readings, as `NO LIGHT` does for a lightless object.
  - _Amends_ "Data states", "Limited detail is an annunciation" (lines 375–377). Its "since the
    view's readouts come from the server" becomes "since the view's readouts come from the server's
    data and the height function, not from pixels", as the brainstorm's item 7 has it: these
    readouts are computed in the client's worker from the server's field.
  - _Goes in_ the nomenclature list, after `FROM CAMERA` (line 935).
  - _Extends:_
    - the `ELV` row (line 784), which these leave as it is;
    - `DRIVE RANGE` (line 796) and `FROM CAMERA` (line 935, "where there is no own ship to measure
      from"). The second gains a slant range, always measured from the camera;
    - `NO BODIES`, `NO SYSTEMS IN MAP`, `NO LIGHT` (line 1012), and `GALACTIC` (line 1017,
      whose `HEIGHT` these do not reuse);
    - "Data states": the Missing row (line 345), the Estimated row (line 347), the section states
      (lines 379–399) and staleness (lines 351–352);
    - "Numbers, units and time": lines 225–226 (units scaled with hysteresis), 230–236 (`-` always,
      `+` only where direction matters), 237 (digits grouped from five) and 239 (angles in degrees);
    - "Controls and commanding": lines 477–480 (the keyboard; nothing on hover alone);
    - "Motion and sound": lines 707–709 (readouts at about 4 Hz);
    - "Voice and nomenclature": line 729–730 (an abbreviation only from the list) and line 765 (a
      reading of parts joined by `·`).
  - _Leaves_ the descent spike's `SITE HEIGHT` (line 977) as R05 has it, although by this
    distinction it is an elevation (a question below).
  - _Signed off (signoff-9): amended._

- **Draft 4. The survey line.**
  - _Wording,_ a bullet of "Views": "While a view's `FRAME` is a body's and the body has a
    surface, its label block carries the ship's survey of that body, as the star chart carries its
    census line: `SURVEY COVERAGE 12.4 % · FINEST 2.0 m`.
    - `COVERAGE` is the share of the body's surface surveyed. It is in per cent to three
      significant figures, in E notation below 0.001 % (`2.00E-5 %`). From 99.95 % until the whole
      body is surveyed it is `ABOVE 99.9 %`, and only then `100 %`. Where no cell is surveyed it is
      `NONE`, with no `FINEST` part, since none is a value: `SURVEY COVERAGE NONE`.
    - `FINEST` is the finest resolution surveyed anywhere on the body. It is a length to two
      significant figures, in `m` below 1 km and in `km` above, with hysteresis.
    - Until the body's survey first arrives the line reads `SURVEY COVERAGE: PENDING`. Where this
      generator version computes no surface for the body, it reads
      `SURVEY COVERAGE: NOT YET MODELLED`.
    - On loss of the link its values go stale, in `--text-muted` with one trailing `S`.
    - `DECORATION ON`, where it stands, follows it.
    - The survey's resolution at the line of sight sets every reading's uncertainty, so it stands
      with those readings, in `FINEST`'s form, as `RESOLUTION 410 m` under
      `GROUND AT LINE OF SIGHT`."
  - _Rows._
    - `SURVEY COVERAGE`, kind `Label`: the share of a body's surface the ship has surveyed, on a
      view's label block, with its `FINEST` after a middle dot.
    - `FINEST`, kind `Label`: after `SURVEY COVERAGE` alone, the finest survey resolution held
      anywhere on the body. A literal word, where "best" would be a judgement.
    - `RESOLUTION`, kind `Label`: in a view's terrain readout, the survey's resolution at the
      point it reads, or `NOT SURVEYED`.
  - _Goes in_ "Graphs, schematics and spatial displays", "Views", after the star limit's bullet
    (lines 611–627); the rows after `DECORATION ON` (line 934).
  - _Extends_ "Colour", "The rendered image is data" (lines 92–104). Its `DECORATION ON` stands
    "beside its survey coverage", which this line is. It also extends:
    - the `DECORATION ON` row (line 934);
    - "Views", lines 611–627 (the label block's notes on what has not arrived);
    - "Data states": staleness (lines 344 and 351–352), and none as a value (line 395, with the
      `NONE` of lines 831 and 888);
    - the `STARS: PENDING` row (line 922) and `NOT YET MODELLED` (line 815);
    - "Numbers, units and time", lines 208–211 (ladders with hysteresis; E notation off the
      ladder);
    - "Voice and nomenclature", line 755–758 (a fraction named in words, a space before `%`).
  - _Signed off (signoff-9): amended._

- **Draft 5. The material reading.**
  - _Wording,_ a bullet of "Voice and nomenclature", after the phase bullet (lines 750–754): "The
    ground's material at a point is one reading of two parts, joined by a middle dot: first its
    form, then its substance (`MATERIAL REGOLITH · Fe-RICH BASALT`, `MATERIAL SNOW · H2O`,
    `MATERIAL FROST · CO2`).
    - The form is how the substance lies, a word from a fixed list: `BEDROCK`, `REGOLITH`, `SOIL`,
      `SAND`, `SEDIMENT`, `VOLCANIC ROCK`, `MELT`, `ICE`, `SNOW`, `SEA ICE`, `FROST`,
      `ORGANIC SEDIMENT`, `SEABED`, `EVAPORITE` and `DUST`. It is the form the view draws most of at
      that point, which is also the authoritative one there.
    - The form is the ground's class, not a substance or a phase, and keeps its word, as a
      structure layer does. A form word holds no common name of a formula: `ICE · H2O`, never
      `WATER ICE`. `MELT` is ground above its lithology's solidus, which petrology calls melt; it is
      not `liquid`, a word that would claim it is above its liquidus.
    - The substance is the ground's palette entry's. It is named only by the console's list of
      substance names, in a label's case. A formula keeps its case (`H2O`, `CO2`, `Na2CO3`), a name
      in words is upper-cased (`MARS-TYPE DUST`), and a substance the list does not name reads
      `UNLISTED MATERIAL`, never its key.
    - Where the body's composition is not modelled (its palette is empty) the reading is
      `MATERIAL NOT YET MODELLED`.
    - Prose and other readings still name a molecular ice as `CO2 ice`, by its phase. The form's
      reading is the ground's, and the two are not rivals."
  - _Real outputs_ of `substanceName(key, "upper")` today (`lib/substances.ts`, P14.T49.f, run
    2026-10-13):
    - formula keys name themselves: `H2O`, `CO2` and `Na2CO3`;
    - name keys take their names: `mars_dust` gives `MARS-TYPE DUST` and `tholin` gives `THOLIN`;
    - the lithology keys the reference and generated palettes use today give `UNLISTED MATERIAL`:
      `basalt`, `basalt_fe`, `granite`, `feldspathic` and `phyllosilicate`. Their names land with
      plan 14's T49.b rows, and plan 14 drafts `basalt_fe` as `Fe-rich basalt` (`Fe-RICH BASALT`)
      by signoff-5.

    So today a basaltic plain reads `MATERIAL REGOLITH · UNLISTED MATERIAL`.

  - _Rows._ `MATERIAL`, kind `Label`: the ground's form and substance at the point a view's terrain
    readout reads, joined by a middle dot. Beside it, one row, kind `Material form`, lists the
    fifteen form words with their meanings, the classes of this plan's `MaterialClass` (Design
    note 1):
    - `BEDROCK`, rock exposed, as on ground steeper than loose material can rest on;
    - `REGOLITH`, an airless body's loose, impact-broken layer;
    - `SOIL` and `SAND`, loose ground by aridity;
    - `SEDIMENT`, the fill of channels and basins;
    - `VOLCANIC ROCK`, an older volcanic province's;
    - `MELT`, ground above its lithology's solidus;
    - `ICE`, a glacier's or an ice cap's;
    - `SNOW`;
    - `SEA ICE`, ice over a sea;
    - `FROST`, a frost of the ice entry's species;
    - `ORGANIC SEDIMENT`, the fallout of an organic haze;
    - `SEABED`, the ground under a sea;
    - `EVAPORITE`, the salts of a dried basin;
    - `DUST`, the fines that wind lays.
  - _Goes in_ "Voice and nomenclature", after lines 750–754; the rows after `UNLISTED MATERIAL`
    (line 933).
  - _Extends:_
    - "Voice and nomenclature": lines 731–745 (a substance named by the console's list,
      `UNLISTED MATERIAL` where it names none, never a key); 746–749 (no common name for a formula,
      inside another name too); 750–754 (a phase is a word beside the name, `CO2 ice`); 762–764 (a
      structure layer keeps its word); 765 (a reading of parts joined by `·`);
    - "Typography", lines 143–145 (a formula keeps its case in an upper-case label);
    - the `UNLISTED MATERIAL` row (line 933), and a ring's `COMPOSITION` (`POROUS ICE`, `ROCK`;
      line 883), the precedent for a form word.
  - _Signed off (signoff-9): amended._

- **Draft 6. The survey edge, a mark in both styles.**
  - _Wording,_ a bullet of "Views": "Where a body's surveyed terrain meets ground the ship has not
    surveyed, the view marks the survey's edge, in both styles. Unsurveyed ground then never looks
    surveyed, and the step between the two is never read as a cliff.
    - Unsurveyed ground shows the body's reference surface, the `DATUM`, with no terrain, contours
      or reading claimed there. A skirt in the reference surface's appearance joins the two where
      their heights differ.
    - The edge is drawn along the surveyed terrain's border, as the chart's query edge marks where
      fetched data ends: a solid `--text` line `1px` wide with short ticks pointing into the
      unsurveyed side. Each tick is `0.25rem` long, one every `1rem` of the drawn line. The line is
      never dashed.
    - It is symbology. Over the photorealistic image it carries its `--surface-0` casing, as every
      mark does, and in the wireframe it is cased as the wireframe's lines are. Nearer ground in
      front of it hides it, as a limb hides a graticule.
    - While any of it is in view, the view's list notes it as `SURVEY EDGE`. The note gives the
      range to the edge's nearest point in the picture, measured and labelled as the list's ranges
      are (from the own ship, else `FROM CAMERA`), to three figures. It is a note outside the
      list's options, like `INTEGRATED STARLIGHT`: not a contact, and it cannot be selected."
  - _Row._ `SURVEY EDGE`, kind `Label`: the mark where a body's surveyed terrain ends, in both
    styles. It is a ticked `--text` line, its ticks towards the unsurveyed side, and the view's list
    notes it with its nearest range (`SURVEY EDGE 4.21 km`).
  - _Goes in_ "Graphs, schematics and spatial displays", "Views", after the bullet on what a view
    keeps (lines 597–610); the row after `SURVEY COVERAGE`.
  - _Extends:_
    - "Colour", "Outlines for symbology, and only for symbology" (lines 105–110, R02's item 3: a
      mark over the image is cased);
    - "Graphs, schematics and spatial displays", lines 552–554 ("The edge of the fetched data is
      drawn, so that unfetched space never looks empty"), as a principle. A view is not a spatial
      display (lines 593–596), so the mark is taken over from plan 05's D13 query edge ("a solid
      `--text` hairline with short outward ticks every 10°, the look of a chart boundary"), its
      ticks spaced along the drawn line instead of by angle;
    - line 503 (dashes are predictions);
    - "Views": lines 597–610 (what a view keeps), lines 644–658 (both styles, the black ground, the
      casings), lines 660–664 and the `INTEGRATED STARLIGHT` row (line 947), a list note that is
      not a contact;
    - lines 666–669 (every list entry carries a range) and 671–672 (the list names every kind in
      words).
  - _Signed off (signoff-9): adopted._

- **Draft 7. Contours, their interval and index.**
  - _Wording,_ a bullet of "Views", after the paragraph on both styles (lines 644–658): "In the
    wireframe style a view draws surveyed terrain as its contours, lines of equal `ELEVATION` above
    the `DATUM`. It draws nothing else of the ground, which is depth only and hides every line and
    star behind it.
    - _The lines._ Minor contours are solid `--text-muted` lines `1px` wide. Every fifth is an index
      contour, solid `--text-muted` `1.5px`, a step heavier, as a graticule's equator is. On a world
      with a sea, the coastline lies at the sea level, whether or not that falls on the interval,
      and is solid `--text` `2px`. So each kind differs from the others by width, not by colour
      alone.
    - _How they are drawn._ Each is drawn at its width as "Layout" gives it, constant across the
      picture. None is ever dashed, since dashes are predictions, or labelled in the picture, since
      the readout's `ELEVATION` and the `DATUM` carry the values. A contour in `--text-muted` is not
      stale, as an orbit is not. On the wireframe's black ground a contour is the ground's own line,
      and uncased; drawn over the photorealistic image, it is cased as any mark is.
    - _The interval._ The label block states the interval and the index once, in the form of a
      map's margin: `CONTOUR INTERVAL 200 m · INDEX 1 km`, each in `m`, and from 1 km in `km`, with
      hysteresis. The interval is a step of 1, 2 and 5 times a power of ten, never below 1 m. It is
      set by the camera's height above the ground beneath it, so that every view in one pose states
      the same one.
    - _Where contours are left out._ Where a family's lines would stand closer than `4px` (in CSS
      pixels, scaled as line widths are), the family is left out. It fades out between `6px` and
      `3px` of spacing, the minor contours first and then the index. The coastline is always drawn.
      A contour is scored as drawn in full; it is not scored in its fade, as a stroke is not scored
      near an open end. So surveyed ground without contours is steeper than the interval can show
      there, never flat, and ground with no contours at all is not surveyed.
    - _Scale._ With the field of view, the interval is the wireframe's statement of scale in place
      of the 1-2-5 scale bar. The field of view gives the picture's angular scale, and the interval
      its vertical one.
    - _The graticule._ The graticule is still drawn over surveyed terrain, and is told from the
      contours by its geometry: it crosses contours, which never cross one another."
  - _Rows._
    - `CONTOUR INTERVAL`, kind `Label`: on a wireframe view's label block, the height between
      adjacent contours, a 1-2-5 step (`CONTOUR INTERVAL 200 m`), with its `INDEX` after a middle
      dot.
    - `INDEX`, kind `Label`: after `CONTOUR INTERVAL` alone, the height between index contours,
      five intervals (`INDEX 1 km`). Not a belt's `SIZE INDEX`.
  - _Contrast._ `contrast.py text-muted surface-0` gives 7.22:1 and `contrast.py text surface-0`
    13.57:1, each above the 6:1 a stroke that carries meaning needs. The view's ground is black,
    darker than `--surface-0`, so either pairing is higher there.
  - _Goes in_ "Views", after lines 644–658; the rows after `FOV` (line 907).
  - _Extends:_
    - "Views": lines 644–647 ("its marks, graticules and contours included, follow this guide's
      ordinary rules for colour, stroke and contrast"), and lines 666–669 ("A view cannot carry the
      1-2-5 scale bar ... In its place the view states its horizontal field of view", R02's item 5),
      to which the interval is added;
    - the `FOV` row (line 907, "a view's statement of scale in place of a scale bar");
    - "Colour", lines 62–82 (a stroke meets its ratio as drawn; not scored near an open end);
    - "Graphs, schematics and spatial displays": line 486 (lines told apart by style first), lines
      503 and 512 (dashes are predictions), and lines 571–576 (a selected orbit wider, since colour
      alone must not carry it; `--text-muted` is not stale);
    - "Layout", lines 286–294 (widths times the larger of the device-pixel ratio and 2).
  - _Excepts_ nothing. It departs from Design note 14 in three places, each a question below: the
    index contour's width, the fade's scoring, and the datum on a line of its own (Draft 3).
  - _Signed off (signoff-9): amended._

- **Questions for the sign-off**, each with this draft's lean.
  - _Unsurveyed readings._ Design note 6, and the life-and-planet-content brainstorm in two places,
    write the em dash with `NOT SURVEYED`. Draft 3 sets `NOT SURVEYED` once in place of the
    readings, as `NOT RESOLVED` stands for a section, since the em dash means "not computed". Lean:
    Draft 3's form, with Design note 6 and those brainstorm sentences brought in line.
  - _The index contour._ Design note 14 makes it `2px` `--text-muted` beside a `2px` `--text`
    coastline, so the two would differ by colour alone (lines 573–574). Lean: index `1.5px`, as
    drafted. The alternative is a `3px` coastline.
  - _The contours' fade._ A fading contour is drawn below its 6:1. Lean: exempt it in its fade, as
    drafted, since T11 fades it. The alternative is a cut at `4px`.
  - _The graticule over contours._ Minor contours and the graticule's lines are both `1px`
    `--text-muted`. Lean: keep the graticule, told apart by geometry as drafted, and add to T11's
    recorded legibility check that the graticule is told from the contours, on a small body too.
    The fallback is leaving the graticule out over surveyed terrain.
  - _The datum's line._ Design note 14 states `CONTOUR INTERVAL 200 m · INDEX 1 km · DATUM …` as
    one line. T12 wants the datum stated once, in both styles. Lean: `DATUM` on its own line, as
    the USGS margin puts its datum under its interval.
  - _The datum's radii._ Design note 14 writes `6378.1 × 6356.8 km`, which NIST SP 811 §7.7 rules
    out ("51 mm × 51 mm × 25 mm but not: 51 × 51 × 25 mm"). Lean: `6378.1 km × 6356.8 km`, as
    drafted.
  - _The resolution at the line of sight._ T12 puts it on the survey line. Lean: in the readout
    group as `RESOLUTION`, beside the uncertainties it sets, since a bare figure on the survey line
    would read as a distance.
  - _A slant range off the hull._ It is measured from the camera. Lean: `SLANT RANGE FROM CAMERA`
    whenever the camera is off the hull, widening `FROM CAMERA`'s meaning from "no own ship".
  - _A mark at the line of sight._ The view draws none today, so the readout's point is unmarked.
    Lean: T12 draws one, of a shape no other mark uses (not the selection's bracket, nor the
    galaxy cursor's or chart centre's cross), drafted with T12 for the owner.
  - _The material under a coarse survey._ The class comes from the finest level, so it can name
    `BEDROCK` on a cliff the survey has not resolved. Lean: name the class drawn, as Verification
    has the readout do, gating it by coverage but not by resolution, since it is not a number.
  - _`SEABED`'s substance._ Design note 1 puts `Seabed` "under its liquid entry". If that entry is
    the material's, the reading says `SEABED · H2O`, a floor made of H2O. Lean: the seabed's entry
    is the floor's own, the crust's or a deposit's, and the sea is named by its coastline and its
    `SEA LEVEL`.
  - _The material's properties._ T12 shows "its properties as a reading", whose names are not
    drafted here: bearing capacity and pad friction, with their footing width and temperature.
    Lean: T12 drafts them for the owner before it builds them.
  - _Precision._ `ELEVATION` 0.1 m (Design note 6); `SLOPE` whole degrees; `SLANT RANGE` three
    figures; resolutions two. Lean: as drafted.
  - _`ELEVATION`'s sign._ The issue's form, `~2140 m ± 180 m`, sets no `+`, while the spike's
    `SITE HEIGHT` (line 977) is signed. Lean: no `+`, as maps write it.
  - _`SITE HEIGHT`._ By ICAO's distinction it is an elevation. Lean: rename it `SITE ELEVATION` in
    `ELEVATION`'s form when the spike next changes (R05's row; `DescentSpike.tsx`). It is not this
    task's to change.
  - _NIST and the guide's spans._ The same §7.7 also rules out "0 – 5 V", the form of the guide's
    spans (`66.0 – 137 Mm`, line 227–229). The drafts do not touch the span rule. Lean: leave it,
    a separate question for the owner.

- **If signed off, these follow** (other tasks' text, not changed here): Design note 6 (the
  unsurveyed form); Design note 14 (the index width, the fade's scoring, the datum's line and its
  radii's units); T11 (its fade test's thresholds in CSS pixels, its legibility check);
  T12 (`RESOLUTION` in the readout group, `SLANT RANGE FROM CAMERA`, the point's mark, the
  properties' names); the life-and-planet-content brainstorm's sentences on `NOT SURVEYED`; R05's
  `SITE HEIGHT`; and the roadmap README's "Still awaiting" entry for R10.T13.

- **Checks** (2026-10-13, this worktree).
  - `glyphs.py "± · × ° — ~" '\u00a0'`: every character outside ASCII that the drafts put on
    screen, with the tilde and the no-break space (exit 0):

    ```text
    char  code     B612   Mono   name
    ±     U+00B1   yes    yes    PLUS-MINUS SIGN
    ·     U+00B7   yes    yes    MIDDLE DOT
    ×     U+00D7   yes    yes    MULTIPLICATION SIGN
    °     U+00B0   yes    yes    DEGREE SIGN
    —     U+2014   yes    yes    EM DASH
    ~     U+007E   yes    yes    TILDE
          U+00A0   yes    yes    NO-BREAK SPACE
    ```

  - `ux_lint.py` on a scratch `.ts` file, outside the worktree and not committed, that holds every
    string the drafts propose as a string literal (exit 0):

    ```text
    0 error(s), 0 check(s) in 1 file(s) scanned. Heuristic: confirm each line against the guide.
    ```

### R10.T14 Cache sizes

- **R10.T14.a The count model and the slot layout.** `patchCountModel` as a pure function of camera,
  τ and viewport (Design note 15, after the research model's rule); `bytesPerPatch`;
  the budget per setting, provisionally 64 MiB low and about 400 MB high (Design note 15), in R05's
  `TerrainSettings.cacheBytes` (`view/quality/qualitySetting.ts`, the one list), as fixed slots in
  the layout of Design note 15 (storage buffers for unfiltered data, arrays or atlases for filtered
  data), handed to R05's `PatchCache` as its `SlotLayout`. As built, `slotLayout.ts`'s
  `SlotFieldName` holds `horizon-map` at 0 B and no class-weights or survey-mask field: each is
  added by the task that first uploads it (T8.b, T10.b, T10.c), and this task sizes them. Files:
  `cacheBudget.ts`, R05's `cache.ts`, `slotLayout.ts` and `view/quality/qualitySetting.ts`. Tests
  (pure): `bytesPerPatch` matches Design note 15's formats
  for each setting and vertex path; `patchCountModel` reproduces the table of Design note 15 within
  10%; the cache never evicts a patch under a grounded body at the chosen size during a replay of
  the descent's selections. Acceptance:
  `pnpm --filter hyperion exec vitest run src/renderer/src/view/terrain`.
- **R10.T14.b The measured sizes.** Measure the all-round and frustum peaks per view on R05's
  scripted descent over a generated world, on both settings and in both styles, with a second view
  open; set each setting's `cacheBytes` from max(1.3 × all-round peak, 3 × frustum peak) summed
  over the streaming views plus the forced region, or the frustum rule on the high setting if the
  all-round peak is never resident (Design note 15), recording which, whether R05 chose
  `BakedOffsets` (as built R05.T18 keeps it on the high setting), and whether 64 MiB and about
  400 MB hold, or the finding; and record the resident
  total (the cache, the coverage mask, the class map, the cascades, each worker's field copy)
  against the ceilings of 1 GB low and 2–3 GB discrete, from driver-reported GPU memory as well as
  computed bytes. By hand, recorded, on a quiet machine: the peaks and totals on the development
  machine's RTX 3080 and, by the owner, on the UHD 620. Acceptance: the figures recorded and the
  constants updated, with `pnpm --filter hyperion exec vitest run src/renderer/src/view/terrain`
  passing.

### R10.T15 Verification pass

The collision-agrees test end to end: a TypeScript test that forms the finest patch's vertices with
R05's vertex formation in `Math.fround` (`gpu/vertexEmulation.ts`), from a bake of a real patch
committed as a fixture, and
interpolates on R05's diagonal, agrees with `ground_at` from the same fixture to 2 mm plus the
formation's stated error (the height texture's `f32` step at ±20 km is 2⁻⁹ m, 1.95 mm: the
brainstorm's "about 2 mm", under The geometry); R05's descent run on a generated world at both
settings with its metrics harness and the horizon map on (T8.b), recording the map's share of the
workers' bake time and `bake/horizon`'s WebAssembly figure (main's ruling for R10.T8.a, 2026-10-15),
patches a second sustained against the brainstorm's demand of (200 · v + 290 · |ḣ|) ÷ h, the level
bound per level from T4.a, and pops or cracks seen; every golden of this plan asserted on the three
targets. Acceptance: the lane's `just smart-ci` and `just test-wasm-fast`, this plan's slow binaries
by name (`just test-slow -E 'binary(…)'`) and `just bench -- bake/` complete, and the descent's
figures are recorded; `just ci` and the slow pass are the orchestrator's.

## Verification

- **Collision agrees with what is drawn:** the finest mesh and `ground_at` to 2 mm (T3, T15).
- **Level-of-detail consistency:** every coarser level inside R09's bound on generated worlds, both
  ratios recorded (T4.a); selection by that bound, history-independent (T4.b).
- **The ground drawn and the ground quoted agree:** the largest weight is the authoritative class at
  the finest level; coarse weights within ten points of brute force (T2); the readout names the
  class the shader draws most of (T10.b).
- **Photometry:** the scaled classes reproduce the body's p (T10.a); the handover changes brightness
  by under 1/3 stop at two phases (T10.e); a patterned body's point equals its disc at the 3 px
  switch to 1% (T10.f).
- **Knowledge:** unsurveyed ground has no terrain, no contours, no class-map texel and no reading; a
  readout's uncertainty matches the pooled RMS of the unsurveyed bands (T5, T12); the image is the
  same at every survey resolution that covers it (T6.a, T10.c, T10.d).
- **Determinism:** material and ground goldens on native and both wasm targets (T1.b, T3); bakes
  order-independent, the horizon far field bit-identical at shared points of one level on one face,
  and blended to the parent's map across a level change (T6.a, T8.a, T8.b; main's ruling for
  R10.T8.a, 2026-10-15).
- **Benches:** `bake/patch_65` (40 ms), `bake/horizon` (a quarter of it), taken on a quiet machine
  and recorded with the load average; a run under shared load is provisional and repeated.
- **By hand, recorded:** on the development machine (RTX 3080), the cascades (1.5–3 ms), the
  disc-to-terrain handover, the descent on a generated world and the cache peaks; by the owner on
  the UHD 620, the horizon-map pass (0.3–0.5 ms), the station wireframe at 60 fps at 1080p under
  load and the cache peaks.

## Generator version

T1.b bumps `GENERATOR_VERSION` once: the material class is new authoritative output, read by landing
and the consoles, and its goldens pin it. The class discriminants, the property table, the
classifier's thresholds and its order of rules are generator-versioned from then on; a new class
appends a discriminant. So are T1.a's codes for R09's `PaletteEntry::phase_row`, which the payload
carries (the re-validation; the layout's freeze at R09.T19's first send is R09's Generator
version). `ground_at` reads only heights R09's goldens pin, so T3 adds goldens without
a bump. T10.a bumps it a second time: the albedo scale ships in R09's payload header, whose format
and contents R09's Design note 17 puts under `GENERATOR_VERSION`, and R09's payload and coarse
goldens re-bless in the same commit; from then on the solve and the class albedo ranges are
generator-versioned too. _As built (main's ruling for T10.a, 2026-10-14): no bump. R09.T19.b's
ruling freezes the payload's layout only at the first payload served for a generated body, or a
client's first persisting of one, and no generated body is served before P14.T54.a, so T10.a
fills the header at 22. No golden of R09's moves (the server sets the scale after `coarse_pass`,
and the testing worlds' payloads keep `None`); T10.a's own goldens, `tests/golden/albedo/`, carry
22, and the solve and the ranges are generator-versioned from them (Risks, "Deviations in T10.a,
as built")._ The weights, the class map, the bounds' use in selection, the readings and
the horizon maps are presentation and move nothing generated; a change to them is a client change.
No domain tag is added: nothing here draws from a stream. The plan adds no protocol message of its
own; it reads R09's `surface_field` (with `CoverDto`'s per-range `ResolutionCode`) and R03's frames,
and fills R09's `FieldHeader.albedo_scale`, which R09 already reserves.

## Risks and open points

- **The local body's terrain sits ωτ ahead of its lighting (from R07.T10.a, ruled 2026-10-05).**
  R07 lights the ship's local body at its retarded time, T − τ, as the brainstorm draws every
  time-varying state, while its geometry, and so this plan's terrain, is drawn at the present.
  Once rotation is drawn, the terrain sits ωτ ahead of its lighting: about 2,100 km at a Jupiter's
  equator from its Hill sphere's edge (τ 168 s), about 38 km at τ 3 s. This plan decides whether
  to draw rotation-dependent state (the class map's orientation, shadows on terrain) at the
  retarded time too.
- **The selection bound's calibration is an owner ruling.** If T4.a's 99.9th-percentile ratios fall
  at the expected 0.3–0.5, the hard bound costs four to eleven times the patches of a perfect bound;
  selection by min(hard, 4σ) recovers two to three times. The owner rules with T4.a's recorded
  counts; until then selection uses the hard bound.
- **The low setting's cache is at its edge.** With 16-bit horizon maps a low-setting patch is
  about 156 kB, and the sizing rule's 430 slots fill 64 MiB exactly before relief and balancing. T14
  decides between a larger low cache (inside the 1 GB ceiling), a half-resolution near-field horizon
  (33 × 33, 17 kB) and the frustum rule, and records which. R05 choosing `BakedOffsets` would break
  the 64 MiB outright.
- **Coarse weights assume Gaussian slopes.** The slope-exceedance probability of Design note 3
  treats the unresolved slope as Gaussian; ridged and cratered terrain is not. The ten-point test is
  the guard; if it fails on a class of world, the fallback is counting a sparse sample of finest
  points, at a cost T6's bench states.
- **The survey edge's skirt** is a visible artefact by design. If players read it as geology despite
  the mark, the alternative is fading the terrain to transparency over a cell, which draws no
  invented ground either.
- **Eight azimuths** may miss a narrow distant peak at sunset; the by-hand sunset in T8.b decides
  whether sixteen are needed, at twice the horizon memory, and whether a face edge near a cube
  corner, where the two faces' azimuths differ most, shows a step (main's ruling for R10.T8.a,
  2026-10-15).
- **Figures from memory.** Most per-class albedos, friction and strength figures, and several shadow
  and contour conventions, were recalled rather than read by the research; each task that turns one
  into code checks it against the primary named in its design note first.

- **Asks of other plans, which the owners may not yet carry** (listed in the notes): of R05, nothing
  now: its interpolant, bake and bound take their height source as R05's `HeightSource` trait
  (R05.T4.a, T4.c), which T3 and T6.a implement for R09's `Synthesiser`, and its Design note 4
  carries this plan's per-patch figures (Design note 15); of R07, which lands first, `body_brdf`
  with per-texel lunar-Lambert parameters, its disc sampling this plan's class map, and
  `sphere_irradiance` taking a local horizon, are hooks R07 creates with defaults (R07.T4.c, T6.a,
  T6.c, T8.b) and this plan fills (T8.b, T10.b, T10.d); it also extends R07's files with an L(α)
  channel in the phase table (T10.b), a masked bilinear class map (T10.d) and the point's class-map
  integral (T10.f), and wires `surface` and `rotation` through R07's `litBodiesOf` (T10.d)
  (decision-r07-t8b), and a disc's pattern waits on plan 14's rotation (P14.T46.f, read by
  R07.T2.b); of R08, the sun's refracted elevation for the shadow test; of R11, that its cloud
  shadows enter through `terrain_shadow_factor`, its ring shadows through `terrain_ring_shadow` and
  its diffuse cloud light through `terrain_sky_factor` (R11's proposed names, adopted). R09's closed
  forms, per-cell resolution, header field and spheroid datum (its Design note 17), R05's
  `rg16float` normals and slot layout, and R04's `math::j0` are already carried and are listed under
  Consumes. Of R09's palette sources and P14.T49.c, from T1.a: an `Ice` entry's A_N is its
  substance's as fresh snow or fine frost (the forms' factors are taken against it), and a
  `phase_row` is filled by `PhaseLaw::for_family` until a substance's own law is known.
- **Shadows on the high setting** are taken here by the reading of Design note 11. If the roadmap
  gives them to R07 or R11 instead, T9 moves whole, and T8's horizon map stays here.
- **`Dust` is placed by a simple rule** (arid, thin-aired, where R09's wind deposits); if the coarse
  field cannot tell deposition from erosion well enough, `Dust` falls back into `Soil` with its
  lower bearing figures, and the class stays reserved.
- **Classes plan 14 cannot yet reach.** `OrganicSediment` (a Titan-like haze regime) and
  `VolatileFrost` of methane need species plan 14 does not produce: it gives only H₂O, CO₂, N₂ and
  Ar, and its Titan has no methane (R08's Design notes 13 and 16). They stay reachable only through
  R09's surface-state classes, and on no generated world until P14.T24.f's carbon speciation
  (R08.T1's draft) lands; T1.b's tests build them on a `FieldBuilder` field and record that no
  sample world has them. _P14.T24.f landed on 2026-10-10 (`4cb28c0b`, `324fbe34`); what now keeps
  them off generated worlds is R09's coarse pass (T11–T16) and plan 14's surface section
  (P14.T48.e), which `for_body` reads (the re-validation of `c6d77cac`)._
- **Vegetation** is absent until a biosphere exists; a living world's land is drawn bare, which is
  wrong for it and says nothing false about what the generator computed.
- **The hardware decisions, decided 2026-09-30 by a delegated decision**, as they fall on this
  plan:
  - _CPU budgets (item 3):_ the 40 ms patch bake (`bake/patch_65`, and the horizon's quarter of
    it) stays the UHD 620 laptop's figure. The development machine records its own and fails only
    if it is over the laptop's budget.
  - _Memory (item 5):_ the ceilings are unchanged: about 400 MB for the high setting's height
    cache (Design note 15) inside the 2–3 GB discrete ceiling. Only the context changes: the RTX
    3080's 10 GiB is shared with the local LLM.
- **Class-map discs and R07's cell pass** (R07.T8.d, 2026-10-06). R07's `disc cells` pass binds
  one class map a dispatch, so a class-map disc under 32 px sums its own cells in its draws, the
  serial chain T8.d removes from uniform discs (about 2.0 M cycles under 4 px). T10.d is to bring
  such discs into the pass: the frame's maps as one array texture with each record's layer in its
  record, or one dispatch a map. Open, for the orchestrator (R07's "Deviations in T8.d, as built").
- **Composition, for the re-validation (decision-composition, 2026-10-09).** _Folded in at the
  re-validation of `c6d77cac`: Provides' `material`, Design notes 1 and 8, T1.a, T1.b, the new
  T1.c, T10.a, T10.b and T10.d; two questions it leaves are in that record._
  - `MaterialClass` stays the form, and `Material` becomes `{ class, entry }`, the entry being
    R09's palette index. `FrostSpecies` goes.
  - `properties(material, &palette)` takes the class's table with the entry's resolved A_N in B,
    V and R, phase row, grain density (the form's porosity by depth giving each layer's bulk),
    transition and mechanics family (science-r09-drainage.md §3).
  - The classifier reads the cell's substance byte and the header: frost or glacier of the ice
    entry, `Seabed` under the liquid entry, `Melt` above the crust's transition by R09's cell
    temperature, and regolith, sand and dust of the crust, `Deposit` or `Cover` entry.
  - R07's class map indexes the body's (form, entry) pairs, at most 16.
  - R10.T10.a solves the scale per band, or states one c with the residual per band.
  - R10.T10.b adds the ground's thermal emission through R08.T18's function.
  - New **R10.T1.c, maturity and cover**: regolith's A_N and colour modulated by exposure (crater
    and surface ages) and by plan 14's T51.d cover, with tests that a fresh crater's ejecta is
    brighter than mature regolith by the Moon's measured contrast, and that zero cover gives the
    substance's own albedo.
  - The Pluto-like and lava-world `FieldBuilder` worlds join T1.b's tests.
- **Re-validated at `c6d77cac`** (2026-10-13, lane r10-revalidate, the `revalidate-plan` skill),
  against R05, R07, R08's T9.a (`5264a413`) and R09 as built: R09.T1.a, T2 with its follow-ups A–D,
  T3, T4, T5 with its two follow-ups, T6.a, T6.b, T7.a–f, T10.a–d, T17, T18 and T19.a, with their
  "as built" records (unchanged at `888c6f8b`, whose two commits touch none of it). No brainstorm
  drift on this plan's topics since the plans were last brought in line with it
  (`git diff 3138e393 -- docs/agent/brainstorming/rendering-and-planets.md`: the wet
  epoch's years, the base spline's name, the crater map's and Venus's taper's figures, R08's
  adiabat and channels, and the sky's counts, none of them this plan's). What changed here:
  - _Names, paths and signatures_ (Provides, Consumes, the tasks). R09's palette entry's members
    (`substance`, `normal_albedo_bvr`, `phase_row`, `grain_density_kg_m3`, `transition`,
    `mechanics`), the header's getters, `decode_block` and the block's frame, `PartialField`'s
    margin code, `UnresolvedVariance`'s getters and the per-contribution structure functions,
    `coarse_pass(seed, &inputs)`, and R09.T19.a's wire types; R05's `HeightSource` as built, the
    triangle query T3 adds, selection's level table and chord sagitta, `slotLayout.ts`'s fields,
    the pool, worker and spike, and the settings' `cacheBytes` (no second `PATCH_CACHE_BYTES`);
    R07's `DiscSurface` file, `phase_factor_table`'s channels, `viewFrameDrawer.ts`, the mesh
    regime's files and `depthWriters`; R08's `curve` argument and `atmosphere_source_transmittance`,
    the aerial perspective's two paths, `preciseAtan2`'s range (T8.b's elevation now takes the
    zenith angle) and the geometric sun; R03's `requestBulk`, `FakeWebSocket` and the scene model;
    R02's label block and DOM list, and the client's body-fixed types; plan 14's rotation and
    photometry on the wire.
  - _Folded in, as their rulings direct:_ decision-composition §1.8 (`Material { class, entry }`,
    no `FrostSpecies`, `properties(material, &palette)`, the classifier's palette rules, the class
    map's (form, entry) pairs, the new T1.c, T10.a's bands and T10.b's thermal emission); R09's
    cold-trap rule for the classifier (Design note 1, T1.b); T1.a's codes for R09's `phase_row`
    (R09's "Deviations in T2", follow-up B); and the slope's projection into the tangent plane
    (R09's "Deviations in T4, as built").
  - _Commands._ The surface crate's integration tests and the sim's take `--features testing`;
    the wasm goldens run under `just test-wasm-fast`; T6.a's and T8.a's benches are named
    (`just bench -- bake/patch`, `bake/horizon`, in a new `benches/bake.rs`); T6.b runs
    `just gen-surface` first; T7 adds `pnpm --filter @hyperion/protocol test`; T15's acceptance
    names the lane's checks, `just ci` and the slow pass being the orchestrator's.
  - _Order._ The Tasks' opening paragraph now gives the order as built: T7, T13 and T1.a startable;
    T1.b, T2, T3, T5 and T6 after R09.T8 (and T9 for goldens); T4.a whole, and the
    `sample_worlds()` cases of T1.b, T2, T5 and T10.a, after R09.T16, since the coarse pass follows
    this plan on the critical path; T7 building the pool's half of the per-body calls and T6.b the
    worker's.
  - _Provisional until R09.T8/T9/T19.b land_ (Consumes, R09): the full `height_at`,
    `Synthesiser::bake_patch` and `height_range_m`, `level_bound_m`, the whole `structure_function`
    and `unresolved_rms`, `QueryHeightError::NonFinite`; the golden heights; and the `surface_field`
    payload, its revisions and `surface_revisions`. Their names are R09's Provides, not guessed.
  - _Pending re-validation:_ none beyond those names. T1.c's cover term waits on P14.T51.d.
- **Questions for "main" from the re-validation** (each touches a contract, the payload R09.T19.b
  is about to send for the first time, so each is cheapest ruled before that send):
  - _A form's albedo on one entry._ Snow and glacier ice are two forms of one H₂O entry, and
    `VolatileFrost` a frost grain of its ice's, but R09's `PaletteEntry` carries one A_N a band, at
    the one grain class the server resolves (decision-composition: "A_N in B, V and R at grain
    classes"). Either the entry carries A_N per grain class (a payload member), or the class table
    holds a per-form factor on the entry's A_N. Lean: the per-form factor, in T1.a's table under its
    generator version, from Design note 8's spans; no wire change.
  - _The albedo scale by band._ decision-composition §1.8 lets T10.a solve per band or keep one c
    with the residual per band stated; the header holds one `albedo_scale: Option<f64>`, and per
    band would be three. Lean: one c, fitted in V, with the B and R residuals stated; no wire
    change.
  - _The `sample_worlds()` cases as follow-ups._ T1.b, T2, T5 and T10.a land their synthetic-world
    parts first and their reference-world binaries after R09.T16, under the same task IDs, as R09's
    T5 had follow-ups. Lean: so; the alternative is new subtask letters.
  - _Answered by "main", 2026-10-13._ The first: a per-form factor in T1.a's class table, no wire
    change (built: "Deviations in T1.a, as built"). The second: one `albedo_scale`, fitted in V,
    with the B and R residuals stated and recorded (T10.a). The third: the `sample_worlds()` tests
    land later as commits under the same task IDs, not new subtask letters.
- **Deferred findings (2026-10-13 rule)** (the re-validation of `c6d77cac`; each leaves this plan's
  algorithm and contracts as they are, for "main"'s list).
  - _The bound's wording._ Main's acceptance for R09.T8 makes `level_bound_m` a 1 − 10⁻⁹ per-point
    quantile in its crater and rim terms, while R05's `HeightSource::level_bound_m` is documented
    as "the level's hard error bound", and T3 implements that trait over it. Lean: T3 rewords the
    trait's doc comment to the stated bound in its own commit (R05's file; no code moves). Design
    note 5, T4.a and T4.b carry a pointer here.
  - _Channelled plains' structure function._ R09.T8's closed form runs 15–37% high there, so T5's
    slope uncertainty and T2's cliff exceedance overstate channelled plains: the safe direction for
    a readout, but T2's ten-point check may fail on channelled cells, and the test names them. The
    fix is R09's channel follow-up.
  - _The bake's budget._ R09.T8's provisional 77–82 ms a patch at L + 4 (14 µs a point) already
    exceeds T6.a's 40 ms before the class weights; T6.a's quiet figure follows R09's
    fused-interpolant follow-up. T8.a's far field pays the same per-point cost.
  - _The cache's slots._ With R05's as-built normal gutter, a low slot is about 157–160 kB and
    64 MiB holds 420–426 slots against Design note 15's 430; with `BakedOffsets` kept on the high
    setting (R05.T18), a high slot is about 310 kB and 400 MB holds about 1,290 against 1,900.
    T14 decides, as planned.
  - _R03's client API._ `requestBulk` has no per-chunk delivery, so T7 adds an option beside
    `onPartial` in `@hyperion/protocol`: additive, and the first R10 change in R03's package.
  - _A peak's extended-source horizon._ R07's `sphere_irradiance` clamps a negative horizon at 0,
    so a peak's planetshine and wide-source term is the flat ground's; the sun's own lit fraction
    (Design note 10) keeps the signed horizon. Small; R07's file. _For the sun, settled by T8.b
    (decision-r10-t8b §1): in the element's frame the clamp takes the element's own plane exactly
    where it hides more, and a slope's signed horizon is carried by the conversion. Planetshine is
    not shadowed by the map (Deferred findings (R10.T8.b))._
- **Deviations in T7, as built** (2026-10-13, lane r10-store).
  - _Files._ The new `view/terrain/coarseFieldStore.ts` and `coverage.ts`, with their tests; R05's
    `workers/{pool,messages,heightBake,height.worker}.ts` and their tests, with a new
    `heightBake.test.ts`; `view/spike/spikeScene.ts` and `spikeRun.ts`; R01's
    `view/engine/memory.ts`; R03's `packages/protocol/src/{requests,index}.ts` and `bulk.test.ts`.
    `pool.test.ts`'s scripted worker moved to `src/renderer/src/test/scriptedHeightWorker.ts`, so
    that the store's tests share it; it answers field messages through the worker's own
    `HeldFields`. The store's test renders R03's link hook over `FakeWebSocket`, so it joins
    `vitest.config.mts`'s `DOM_TESTS`.
  - _The per-chunk option._ `BulkRequestOptions.onChunk(chunk: BulkChunk)`, with `BulkChunk`
    (`payload`, `index`, `count`) exported. It sees each chunk the request keeps, in order, before
    its answer; a chunk that breaks the order fails the request unseen. The client still collects
    the chunks and checks them against the manifest, so `outcome.chunks` is unchanged. Additive: no
    other caller changes.
  - _The pool's per-body calls._ `fieldInit(body, block)`, `fieldChunk(body, block)` and
    `fieldRelease(body)` replace `postField`, with `onFieldRefused`, `fieldResidentBytes` (the
    workers' copies, each as its worker last reported it) and `fieldBodies`.
    - The messages are `field-init`, `field-chunk` and `field-release`, kebab-case as the file's
      kinds are. Each is answered `field-held`, with the worker's `heldBytes`, or `field-refused`,
      with its `body`, `message` and `heldBytes`.
    - Each block goes to every worker at once. R05 posted its one 15 MB field to one worker at a
      time, each acknowledged before the next, and held bakes until a worker had it. Now a
      transient copy is one block, at most 1 MiB, a worker. A worker takes its messages in the
      order sent, so a bake posted after a block is made with it, and the bake gate goes.
    - A second `fieldInit` of a started body throws: release it first. A refusal is reported once
      for each start of a body's field. A field's start re-bakes the bakes in flight, as
      `postField` did.
    - For T6.b: `field_init`'s answer is to carry the body's level bounds. T6.b adds them to
      `field-held` for a `field-init`, or gives that answer a kind of its own.
  - _The worker until T6.b._ `heightBake.ts`'s `HeldFields` keeps each body's blocks in JavaScript,
    and `answerRequest(module, request, fields)` takes it in place of `holdField`. A block of a body
    with no `field-init` before it is refused. T6.b moves the blocks into the module's
    `PartialField` a body and keeps the replies.
  - _The spike._ `syntheticField()` is now `syntheticFieldBlocks()`: the same 15 MB in 15 pieces of
    at most `MAX_BLOCK_BYTES`. `postSyntheticField(pool)` posts them as the test planet's
    (`SPIKE_PLANET`) `field_init` and `field_chunk`s, so each worker still holds 15 MB. The pieces
    are no payload blocks, and the worker holds them undecoded. **For T6.b:** its worker decodes
    every block, so it must either hold a test-planet body's blocks undecoded (the test planet reads
    no field) or move the spike to a real payload of `synthetic_worlds()`'s, sized to 15 MB.
  - _The store's API._
    - Provides' `want(body, reason)` and `release(body)`, and `coverage(body)`, which is
      `CoverageView | null`: `null` until a `ready` answer is held, and after the field is
      dropped.
    - `onRevision(cb)` is a listener for each new coverage, returning the unsubscribe, as
      `onBaked` is. A listener may release the body; one that throws is reported on the console
      and changes nothing else. The scene's input is `noteSurfaceRevisions(revisions)`, which takes
      `SceneNotificationDto.surface_revisions` as it is.
    - Also: `follow(candidates)` over the pure `fieldDemand`, `setLinkUp(up)`, `header(body)`,
      `status(body)` (`pending`, `notModelled`, `failure`), `wanted` and `dispose()`.
    - `FieldWantReason` is `local-body` or `in-range`. A `FieldCandidate` is one view's view of a
      body: whether it is that view's local body, and its `rangeRatio`, the camera's distance over
      the body's hand-over range in that view's style (Design note 13's τ is the style's, so each
      view has its own range). `fieldDemand` folds a body's views, so a body is wanted once some
      view is within its own range, and released once every view has left twice its own. The
      caller works the range out (the question below).
  - _Asking again._
    - When the link returns, every body whose field is kept is asked again with the revision held,
      since a rise made while the link was down is not replayed. Asked at the latest revision, the
      server answers block 0 alone, which every worker gets as a `field_chunk`.
    - The revisions announced for a body are forgotten when its field is dropped or the link goes
      down, so that a server of another history (a refused `have_revision`) is not asked again
      and again for a revision it has not reached.
    - The server's busy answers (`queue_full`, `too_many_requests`, and `cancelled` from its side)
      are asked again after `FIELD_RETRY_DELAY_MS`, 2 s, doubling to `FIELD_RETRY_MAX_DELAY_MS`,
      32 s: the link's own reconnection delay to start, a choice and not a measured figure. Every
      other refusal is held until the body is wanted afresh.
    - A first payload that fails (a refused frame, a manifest it disagrees with, a level outside
      5 to 8, a cover that is not a run of cells) drops its blocks from the workers too; a delta
      that fails leaves the field held as it was.
    - An `onChunk` that throws fails its request as `internal` (R03's package), so that the
      socket's listener never sees it.
  - _The coverage._ `CoverageView` is immutable for each revision. Its readings carry their unit in
    their names, by `.claude/rules/typescript-dev.md`: Provides' `resolutionAt` and
    `bestResolution` are `resolutionAtM` and `bestResolutionM`, and the code's twin is
    `resolutionOfCodeM`.
    - `resolutionAtM(cell)` gives metres or `null`, for a cell at the field's level or a patch
      deeper (read at its coarse ancestor).
    - `surveyedFraction()` is by solid angle from the centre, the gnomonic rectangle's closed form
      for each cell, so the cube's small corner cells weigh what they cover. It is exactly 1 only
      when every cell is surveyed, for T13's draft 4 (`100 %` only then).
    - Also `bestResolutionM()`, `maskTexels()`, `withStale` and `merged`.
    - `resolutionOfCodeM` is pinned to `codes.golden`'s 256 `resolution_m` rows, bit for bit. It
      divides by 100, as the Rust does.
    - `fieldCellIndex` and `fieldCellAt` are the client's twins of `cell_index` and
      `cell_at_index`.
    - Margin cells count as unsurveyed everywhere. `surveyEdges(face, level)` is T10.c's, by its
      tests, and is not built here.
  - _The GPU mask._ `CoverageMask` and `coverageMaskTextureSpec`. The mask is an `r8unorm` 2D array
    of six 2ᴸ × 2ᴸ layers, one a face, 255 where a cell is surveyed and 0 elsewhere, under
    `coarse-field-gpu`: 384 KiB at level 8. It is filterable, which the cell's code (`r8uint`) would
    not be. No shader binds it yet (T10.c).
  - _With R09.T19.b._ T19.b landed while this task ran (`7392c064`, merged before the commit).
    - The store takes `SurfaceRevisionDto` for the revisions.
    - As T19.b's record directs, a `bad_request` naming `have_revision` drops what is held, here
      and in the workers, and asks for the field whole.
    - Its deltas carry only cells new to the client or surveyed again more finely, so merging by
      the finer code, as R09's `Cover` folds, is the same as the delta replacing the client's
      copies.
  - _Not wired yet._ No view makes a store yet. T7 builds it and tests it over the fake socket and
    the scripted workers. Its first consumer (T10.c, T11 or T12) makes it, with these inputs:
    - the terrain pass's pool;
    - the scene's `surface_revisions`, passed to `noteSurfaceRevisions`. R03's scene model
      (`sceneWire.ts`'s `applySceneNotification`, `useScene.ts`) does not keep them today, so the
      consumer adds that path in R03's files;
    - the link's status, passed to `setLinkUp`;
    - the views' cameras, passed to `follow`.
  - _Acceptance._ The task's two commands do not reach `view/spike/`, which this task changed, so
    its tests ran by name as well: `pnpm --filter hyperion exec vitest run src/renderer/src/view/spike`
    (20 files, 288 tests, all passing).
  - _Tests_ (`coarseFieldStore.test.ts`, `coverage.test.ts`, `pool.test.ts`, `heightBake.test.ts`,
    `spikeScene.test.ts`, and the protocol's `bulk.test.ts`): all of the task's list, and also:
    - a link lost mid-delta keeps the posted block and marks the coverage stale; the return asks
      with the revision held;
    - a body wanted while the link is down is asked once it returns;
    - a revision announced while a request was in flight asks again after it settles;
    - `not_modelled` holds its section and posts nothing;
    - a refusal is held until the body is wanted afresh;
    - a refused `have_revision` asks again whole;
    - a malformed frame, and a payload that ends inside a block, give the payload up;
    - a worker's refusal drops the field;
    - the frame constants are pinned to `wire.rs`'s source;
    - each view is judged by its own style's range;
    - busy answers are asked again after 2 s, then 4 s, up to 32 s, and not once released;
    - a listener may release the body, and one that throws leaves the rest as they were;
    - a level outside 5 to 8, a level that moved, and a cover that is not a run are refused;
    - an `onChunk` that throws fails its request as `internal`.
- **Question for "main" (R10.T7)**, touching Design note 16's want rule, not a contract.
  - _The hand-over range of a body with no header._ Design note 16's want range is the relief's
    hand-over range (Design note 13). The relief arrives only in the field's header, and plan 14's
    surface section is not on the wire, so a body not yet held has no range: an approached moon,
    say, that is no view's local body. The store takes each view's range ratio from its caller, so
    no contract moves, and T7 is built either way. Lean: until a relief bound reaches the wire,
    the first consumer wants such a body only as a local body, and states it; the alternative is a
    stated stand-in relief from the body's radius until its first header.
- **Deferred findings (2026-10-13 rule) (R10.T7)**, each leaving the algorithm and the contracts
  as they are.
  - _R12's Consumes names `postField`._ R12 (line 305) reads R05's `postField`, which T7 removed.
    Its re-validation reads `fieldInit`, `fieldChunk`, `fieldRelease` and `fieldResidentBytes`,
    the workers' copies for Design note 6's line.
  - _The render thread's second copy._ `requestBulk` keeps a payload's frames until its answer while
    the pool keeps its blocks, so the render thread holds a payload twice meanwhile, about 24 MB for
    an Earth. An `onChunk` caller that drops the chunks from the collection, counting them for the
    manifest, would halve it. Additive, in R03's package.
  - _The pool's own copy._ As R05's did, the pool keeps each body's blocks on the render thread for
    a worker that replaces a failed one. That is about 12 MB for an Earth's first whole payload,
    and it grows until release: every delta, with its block 0, and every re-ask's block 0 is kept,
    re-surveyed cells included. `fieldResidentBytes` counts the workers' copies alone. Lean: T14
    states the kept total beside them, and T6.b may have a replacement worker fetch the field
    whole instead. _Left deferred by T6.b_ ("Deviations in T6.b, as built").
  - _Coverage lags the workers._ A payload's blocks reach the workers before its answer brings the
    coverage. A payload cut off by the link leaves blocks in the workers that the coverage never
    names, until the next answer covers them again. T10.c, which bakes the survey mask, should take
    it from the store's coverage, or from a cover T6.b reports, and say which. _Settled by T6.b for
    the terrain's own mask, which each bake carries; the reference surface's GPU mask is asked of
    main_ ("Deviations in T6.b, as built"; "Questions for "main" (R10.T6.b)").
  - _The fraction on a spheroid._ `surveyedFraction` is by solid angle from the centre; on a
    spheroid the share of its area differs by about its flattening (Earth's 1 ÷ 298). Small.
  - _The re-bake on a field's start._ Starting any body's field re-bakes every bake in flight, as
    `postField` did. Once bakes name their body (T6.b), the epoch belongs to the body. _Settled by
    T6.b: per-body epochs_ ("Deviations in T6.b, as built").
- **Deviations in T3, as built** (2026-10-13; lane r10-ground, on R09.T8 at `c280f604`, merged to
  `59af0b6b` before the commit).
  - _A point, not a direction._ `ground_at` takes the body-fixed point p in metres, and the ground
    under it is the one along the datum's normal through it: p's foot, the unit direction d with
    p = M·d + h·ν(d), which `ground::datum_foot` (crate-private) finds by Newton's method in p's
    meridian plane, with no trigonometric function, from the datum's own direction M⁻¹p ÷ |M⁻¹p|.
    It is tested on WGS 84, a sphere and a body flattened by 0.2, from −0.3 c to +a; the science
    check ran the same iteration on 10⁵ points of each, at most five steps, worst 3 × 10⁻¹⁶ rad.
    - Passing p to R05's interpolant as a direction would put the query at p's geocentric
      direction, some f sin 2φ ÷ 2 of arc away (10.7 km of an Earth's ground at 45°), and M⁻¹p
      alone is off by about |h| f sin 2φ ÷ a (about 34 m at 10 km and 45°): the hazard the
      client's `datumDirection` records.
    - The foot is unique on p's side of the axis (the datum's nearest point) outside the
      spheroid's evolute, which lies within (a² − c²) ÷ c of the centre (42.8 km on WGS 84). The
      second normal through p, from across the axis, is never the one returned.
    - `ground_at` also panics for a point beyond about 10¹⁵⁴ m, where ρ² overflows.
  - _`GroundPoint`_ has private fields and getters. Beside `height()`, `normal()` and `triangle()`
    it gains `dir()`, the foot, and `point_height()`, p's own height along the same normal. With
    them a caller has the ground point, `figure.surface_point(dir, height)`, and p's clearance.
    `FinestTriangle` is a type alias of R05's new `patch::collision::MeshTriangle` (`key`, `x`, `y`,
    `upper` and `vertices()`; `Copy`, `Eq`, `Ord` and `Hash`), because the triangle query serves
    every level.
  - _The triangle query is the crate's._ `patch::collision::mesh_point` and `finest_surface_point`
    return `MeshPoint` (`height_m`, `triangle` and `normal`), all `pub(crate)`: `ground_at` is
    their one public face, and no later plan names them. They sit beside R05's `mesh_height` and
    `finest_surface_height` and share their location (the snap, the split and the vertex reads)
    and their interpolation (`Quad::interpolate`, R05's exact operations). So the height is theirs
    bit for bit (tested), and `collision.golden` is unchanged.
    - At a vertex, `mesh_height` still reads that vertex alone, while `mesh_point` reads its
      triangle's three. So at a vertex whose neighbours read a cell the view lacks, `mesh_point`
      can refuse where `mesh_height` answers.
    - The normal is the flat triangle's, through its vertices' surface points. It points outward
      because every face's (u, v, outward) is right-handed. Its form is pinned output now, as is
      the foot's: a later precision change to either is a bump.
    - A query belongs to the quad whose lattice cell holds it (the face's far edge to the last
      quad), and to the lower half where fa ≥ fb, R05's split. So a vertex has one triangle, and
      that triangle holds it.
  - _`HeightSource` for `Synthesiser`_, in `ground.rs`:
    - `Cache` is `SynthCache` and `Error` is `QueryHeightError`; `figure()` is the header's.
    - `height` is `height_at` at `BandLevel::new(level)`, and panics above 24.
    - `level_bound_m` reads the bounds the synthesiser holds, bit for bit the free
      `level_bound_m(header, level)` (tested). `height_range_m` is `Synthesiser::height_range_m`.
    - `prepare_cache` keeps R05's default, nothing. R09's lattice cover is `synth::bake`'s private
      `cover`, which T6.a's bake reaches, and no value depends on it.
    - R09.T8 built no `QueryHeightError::NonFinite` (R09's "Deviations in T8, as built"), so
      `NotSurveyed` is the ground's only error. Every part asserts its values finite, and
      `ground_at` asserts the normal and the foot again before it returns.
  - _The goldens are in the library_ (main, 2026-10-13). `tests/ground_golden.rs` became
    `src/ground/golden.rs` under `cfg(test)`, with six goldens at
    `tests/golden/ground/<world>.golden`, header 21.
    - `ci`'s wasip1 selection builds the crate without `testing`, so an integration test that needs
      the synthetic worlds would not run there. R09's wire, crater and month goldens live in the
      library for that reason, and main has R09.T9's heights do the same (`synth::height_golden`).
    - They sit in a subdirectory because `cube_golden.rs` holds every flat golden to
      `TEST_PLANET_VERSION`, and a `native_only` test holds `ground/` to `GENERATOR_VERSION`.
    - Each cratered world pins 28 points: 16 at random, two in each of `random_cells`' three seam
      patches (a cube corner's and two on face edges), one under each of up to three coarse
      craters' rims, a finest vertex, a cube corner and a pole, at −500 m to 5 km. The flat world
      pins 25 and the one-crater world 26.
    - Until R09.T9 lands they are the only goldens over `height_at` on the synthetic worlds, so the
      bump that re-blesses R09's goldens re-blesses these too.
  - _No shared world helper._ There is no `tests/common/worlds.rs` or `synthetic_worlds()`: with
    the goldens in the library, the tests take `testing::SyntheticWorld::ALL` through R09.T8's
    crate-private `synth::test_support` (`world`, `SEED`, `random_cells`, `rim_cells`, `Held`),
    and the analytic fields go through the crate-private `ground::ground_over`, `ground_at` over
    any `HeightSource`.
  - _The sim's binary._ `crates/hyperion-sim/tests/surface_ground_worlds.rs`
    (`#![cfg(feature = "testing")]`) runs the four reference worlds through `coarse_pass` itself,
    once each per binary, in 2 s. It asserts finiteness, an outward unit normal and the vertex's
    bit equality. When R09.T16's `sample_worlds()` lands, that list takes this one's place, a small
    follow-up under T3's ID.
  - _Acceptance_, as built:
    - `cargo test -p hyperion-surface --lib ground` (the goldens alone: `--lib ground::golden`);
    - `cargo test -p hyperion-surface --lib patch::collision`;
    - `cargo test -p hyperion-surface --features testing --test panics ground`;
    - `cargo test -p hyperion-surface --test collision_golden` (R05's, unchanged);
    - `cargo test -p hyperion-sim --features testing --test surface_ground_worlds`;
    - the wasm targets under `just test-wasm-fast`.
  - _Tests_ (`ground::tests`, `patch::collision::tests`, `tests/panics.rs`'s `ground`):
    - at a vertex the ground is `height_at` there, bit for bit, at three altitudes;
    - a plane linear in the finest lattice's (s, t), the interpolant's own form, is held within
      10⁻⁹ m, not bit for bit: it is evaluated at the vertices' directions, whose lattice
      coordinates come back up to about 10⁻⁸ of a quad off whole, 5 × 10⁻¹⁰ m at its 0.05 m a
      quad. A level ground, the datum shell at a height, is held bit for bit;
    - either triangle gives a shared edge (diagonal, right and top edges) the same height: within
      10⁻⁹ m on the synthetic worlds, and within 10⁻¹¹ m on R05's relief fixture, across a
      patch's edge too. Either side of the diagonal through `ground_at` agrees within 1 µm;
    - either side of every cube-face edge, and at every cube corner, the ground agrees within 1 µm;
    - the normal is the triangle's: unit, perpendicular to both edges and outward;
    - the face-coordinate and spatial interpolants agree within 1 µm;
    - the drawn mesh, its vertex heights narrowed to `f32`, agrees within half the `f32` step;
    - the ground is the same bits whatever the cache holds and in whatever order points are asked
      (`assert_order_independent`, and each point from an empty cache);
    - the ground is finite at every sampled point, and a `Held` view refuses cells it lacks;
    - the `HeightSource` methods forward R09's values;
    - the foot inverts `surface_point`;
    - three `should_panic` tests: the centre, a NaN point and level 25.
  - _The agreement figures_ come from the `#[ignore]` measurement
    `ground::tests::the_agreement_figures`, 400 points a world, dev profile:
    - the drawn `f32` mesh against the ground, at most:

      | World            | Gap      | Share of the `f32` step |
      | ---------------- | -------- | ----------------------- |
      | Earth-like       | 0.205 mm | 0.397                   |
      | Mars-like        | 0.110 mm | 0.395                   |
      | Moon-like        | 0.120 mm | 0.391                   |
      | Ceres-like       | 0.229 mm | 0.384                   |
      | Flat, one-crater | 0        | 0                       |

      The `f32` step is |h| 2⁻²³ of the patch's largest height. Every share is inside the half step
      of rounding to nearest; the plan's "about 2 mm at ±20 km" is the full step.

    - the face-coordinate interpolant against the spatial one: 15.3 nm Earth-like, 13.9 nm
      Mars-like, 26.1 nm Moon-like, 21.2 nm Ceres-like, 18.0 nm flat and 17.3 nm one-crater.
      - Most of it is Design note 4's sagitta, D² ÷ 8R on each world's finest diagonal D: about
        5 nm on an Earth, 9 nm on a Mars, 18 nm at the Moon's radius and about 20 nm on Ceres.
        Design note 4's 5 nm is an Earth's figure.
      - On the flat world, with no heights, the gap is the sagitta alone.
      - The rest is weighted by slope: the rounding of the lattice coordinates and the warp's
        curvature.
    - either side of face edges and at corners: 14.2 nm Earth-like, 12.3 nm Mars-like, 22.5 nm
      Moon-like, 4.6 nm Ceres-like, 0 flat and 0.09 nm one-crater. Most of it is the nudge across
      the edge leaking into the other face's edge coordinate.
  - _Generator version._ Untouched (21): the goldens pin only what R09's heights already fix.
  - _R05's trait doc_ is reworded in its own commit (Deferred findings, "The bound's wording").
- **Deferred findings (2026-10-13 rule)** (R10.T3; recorded, not ruled).
  - _Skirts on a quantile._ R05's skirt depth adds `level_bound_m`, which is now a 1 − 10⁻⁹
    per-point quantile in its crater terms, so a vertex may, at that rate, step past its skirt.
    The selection and skirt contracts are unchanged.
  - _The cost of a contact._ `ground_at` is three `height_at` at the finest level, about 80 µs
    natively by R09.T8's provisional 26 µs a point at level 19. The fused-interpolant follow-up
    that main queued is the lever; a server collision cache of R09's policy shares the vertices
    between contacts.
  - _A vertex's reads._ At a vertex `mesh_point` reads three vertices where `mesh_height` reads
    one, so near a survey margin's edge the ground can refuse where the height answers (above).
- **Deviations in T1.a, as built** (2026-10-13, lane r10-classes). Files:
  `crates/hyperion-surface/src/material/{mod,class,properties,bearing}.rs`, `lib.rs`'s
  `pub mod material`, and the crate's `clippy.toml`, which gains `McEwen` among its doc idents.
  Nothing generated reads the module yet, so no golden moves and `GENERATOR_VERSION` stays 21
  (T1.b bumps it).
  - _Names._ `WaterIce` (7) is `Glacier` and `VolatileFrost` (10) is `Frost`: the forms of the
    cell's ice entry whatever its species (decision-composition §1.8, "frost or glacier of the
    cell's ice entry"); the codes are the sketch's. `Material::new` returns `Option` (an entry of 15
    or more is none), and `properties` returns `Result` (`ResolvePropertiesError::EntryNotInPalette`
    for an entry past the palette). The sketch's `sources` are a `Citation` per figure
    (`Figure::{Shear, InterfaceFriction, CompressiveStrength, AlbedoFactor, AlbedoSpan}`), so the
    "names its source" test checks each figure. The class's table is public as
    `form_row(class, family) -> &'static FormRow`, for T10.a and the consoles. Also public:
    `ShearLayer` (the sketch's tuple, with a bottom), `AlbedoSpan`, `PhaseLaw`,
    `UNASSIGNED_PHASE_ROW`, `BearingFactors`, `ultimate_bearing_capacity`,
    `directional_hemispherical_albedo` and `LOMMEL_SEELIGER_NORMAL_ALBEDO_RATIO`; from the limb
    ruling's follow-up, `PhotometricForm` with `MaterialClass::photometric_form`,
    `PhaseLaw::limb_parameter` and `normal_albedo_ceiling`, `PhaseLawOrigin::Form`, and the
    constants `LUNAR_LIMB_ONSET_NORMAL_ALBEDO`, `BRIGHT_FROST_NORMAL_ALBEDO`,
    `BRIGHT_FROST_LIMB_PARAMETER` and `PARTICULATE_NORMAL_ALBEDO_CEILING`, public for T1.b's
    table golden (an integration test, `tests/golden/material/`), T10.a's breaks and the consoles.
  - _The phase-row codes._ `PhaseLaw`, one byte: 0 names no law (`UNASSIGNED_PHASE_ROW`, which
    every palette writes today), 1 is `Regolith` (f fitted to R07's `moon` template), 2 is `Ice`
    (R07's `airless-ice`) and 3 is `Lambert` (R07's `lambert`, f ≡ 1 and L(α) ≡ 0; appended by the
    limb ruling's follow-up). `Regolith` and `Ice` take L(α) = L(0) × McEwen 1996's lunar profile,
    held at 0 past 103.9°, with L(0) = `PhaseLaw::limb_parameter(A_N in V)`, the particulate rule.
    A consolidated form (`Glacier`, `Snow`, `SeaIce`, `Evaporite`) takes `Lambert` whatever its
    entry's code, and `phase_law_origin()` is `Form`. A particulate form takes its entry's code; an
    entry of 0, or of a code this build does not know, takes `PhaseLaw::for_family(mechanics)`:
    `Ice` for water and volatile ices, `Regolith` for the rest, never `Lambert`, and
    `phase_law_origin()` is then `FamilyDefault`, for the label. An entry whose code names
    `Lambert` gives a particulate form Lambert's, with origin `Palette`: this follows the ruling's
    "a particulate form keeps the entry's code" literally and was not ruled (asked of "main"; its
    lean: palette sources fill codes by `for_family`'s rule, which never names `Lambert`). The
    server's palette sources (R09's synthetic and reference worlds, P14.T49.c) still write 0;
    filling them is theirs, by the same rule.
  - _`limb_parameter`._ As first built, the largest L(0) for which the law, with f ≡ 1, reflects
    no more than it receives at μ₀ = 1: 1 up to A_N 0.815, then (1 ÷ A_N − 1) ÷ 0.227, and 0 from
    an A_N of 1. The limb ruling rejected it (below), and the follow-up commit makes it the
    particulate rule L_p of decision-r10-limb §2, under the same name: 1 for x ≤ 0.60, [L_E A_E (x −
    A₁) + A₁ (A_E − x)] ÷ [x (A_E − A₁)] = 0.6696 ÷ x − 0.1160 between, and L_E = 0.38 from A_E =
    1.35; NaN gives NaN. A₁ = 0.60 is Buratti 1984 as Buratti et al. 2017 (p. 12) quotes it,
    A_E = 1.35 Enceladus's A_N from Verbiscer and Veverka 1994's Hapke parameters, and L_E = 0.38
    the zero-phase lunar-Lambert fit to those parameters, a derived figure (decision-r10-limb's
    `hapke.py`). `PhaseLaw::limb_parameter` is 0 for Lambert's and L_p for the others, and
    `MaterialProperties::limb_parameter()` is the law's at the material's A_N in V.
  - _The per-form factor (main's answer to the first question)._ An entry's A_N is its substance's
    in its reference form: a particulate surface, or for an ice, fresh snow or fine frost (0.97 in
    the visible, Grenfell, Warren and Mullen 1994). This is a convention for the server's palette
    sources; the synthetic H₂O entry's 0.965 follows it. A form's factor is its span's midpoint over
    that reference, the same in B, V and R: for water ice, glacier 0.814, snow 0.928 and sea ice
    0.856. Every other pair takes the entry's A_N unchanged.
  - _Bearing._ The formula is Terzaghi's with Vesić's factors and shape factors, q_ult = c N_c s_c +
    ½ ρ g B N_γ s_γ, read through EM 1110-1-1905 (Eq. 4-1, Table 4-6). Design note 2's "½ ρ g B
    N_γq ξ_γq" mixes two conventions. The Sourcebook prints its form with no ½, because Durgunoglu
    and Mitchell's N_γq is ½ N_γ for a surface footing (their 1973 eqs. 4.28a and 4.30, read by the
    science check). The layer at depth B is the narrowest of the Sourcebook's overlapping intervals
    that holds B; past the deepest it is the deepest, so a 1 m footing takes 30–60 cm. The
    porosities are n = 1 − ρ ÷ 3.1, unrounded, from Table 9.4's bulk densities, so an entry of grain
    density 3,100 kg m⁻³ returns the Sourcebook's ρ exactly. The pins are 5,368.6 and 206.5 kPa.
  - _The table, source by source_ (each figure's doc comment carries its location). The modelled
    rows, each a (form, family) pair:
    - Regolith of silicate: shear from the Sourcebook's Tables 9.12 and 9.4 (pp. 510 and 484);
      pad friction 0.20 (NAVFAC DM-7.02 Table 1, p. 7.2-63); V span 0.057–0.184 (Dollfus and
      Bowell 1971, via the Sourcebook, p. 560).
    - Dust of silicate: φ 18° and c 1.85 kPa, the midpoint of 0.7–3.0, with its porosity from
      1,200 kg m⁻³ at the assumed 2,600 (USGS PP 1389's abstract, Table 37 on p. 130, and p. 121);
      friction 0.20. Its span stays `None`: no source read gives Design note 8's 0.18–0.25 as a
      normal albedo (Mustard and Bell 1994's Fig. 3 gives about 0.15 at 0.55 µm, a reflectance at
      oblique geometry).
    - Soil of silicate: Viking Lander 2's crusty to cloddy material, φ 35°, c 2.85 kPa, porosity
      from 1,400 kg m⁻³; friction 0.20, its grains as fine as the drift's.
    - Sand of silicate: friction 0.30. Sediment of silicate: friction 0.25.
    - Glacier and sea ice of water ice: steel on ice by temperature, fourteen points from −97 to
      −1 °C under a skate's edge (Liefferink et al. 2021, PRX 11, 011025, Fig. 1). Their spans:
      glacier 0.69–0.89 (Bøggild et al. 2010; Warren et al. 2002), sea ice 0.70–0.96 (Brandt et
      al. 2005).
    - Snow of water ice: span 0.82–0.98 (Grenfell and Maykut 1977, via Warren 1982; Grenfell et
      al. 1994).
    - Frost of a volatile ice: span 0.4–1.0 (Warren, Wiscombe and Firestone 1990; Buratti et al.
      2017). Glacier of a volatile ice: span 0.8–1.0 (Buratti et al. 2017).

    Every other pair is `FormRow::UNMODELLED` and is labelled so: bedrock, volcanic rock, melt,
    seabed, evaporite and organic sediment, and every pair of metal, salt or organic. Ice's
    strength is `None` (Schulson and Duval 2009 not read), as are snow's (as planned), sand's
    shear (no source read) and metal on rock (as planned).

  - _Design note 8's spans, corrected in the code._ Glacier ice's 0.5–0.65 is broadband, and its
    visible is 0.69–0.89. Aged snow reaches 0.82, not 0.85. Bare sea ice is 0.70, not 0.75–0.85.
    The Moon's regolith is 0.057–0.184 (the Sourcebook's p. 560), not 0.07–0.20. The note's text
    is left as written.
  - _Spans not carried._ Design note 8's starting spans with no primary read stay `None`:
    bedrock (basaltic 0.05–0.15, felsic 0.2–0.35), dry soil and sediment 0.10–0.30, desert sand
    0.25–0.45, Martian dune sand 0.10–0.15, Martian bright dust 0.18–0.25, organic sediment
    0.05–0.2 and evaporite 0.5–0.8. T10.a's clamp has no range for them.
  - _Pad friction on ice, against Design note 2._ Kietzig et al. 2010's Fig. 2 was not read. The
    points are Liefferink et al. 2021's Fig. 1, a slow kinetic friction of steel: 0.71 at −97 °C,
    where Design note 2 has 0.3–0.5 at −100 °C, and 0.039 at −7.7 °C rising to 0.18 at −1 °C as
    the slider ploughs, where it has 0.01–0.05 near 0 °C at speed. Frost of any ice, and snow, have
    no friction: no source for them was read. Frost of water ice, a cold trap's exposed ice among
    it, has no row, and takes its entry's albedo unchanged.
  - _Glacier or frost._ With the species gone from the class, Design note 1's "by the condensable
    plan 14 names" no longer chooses between `Glacier` and `Frost`. T1.b's classifier owes the rule
    that does (massive ice against a thin condensate, by the ice field's share or thickness), and
    reads Design note 1's `WaterIce` and `VolatileFrost` as `Glacier` and `Frost`. The limb ruling
    (§2, "What T1.b owes") makes the split photometric too: on an airless world exposed ice is
    gardened into a particulate regolith (Ganymede, Rhea, Dione: lunar-like, "not Lambertian"), so
    the rule must not label a gardened airless ice surface `Glacier`. Its lean: on an airless
    regime, the ice field's land ice is `Frost` where thin and `Regolith` of the ice entry where
    massive; `Glacier` is for ice under an atmosphere (Earth's ice sheets, Mars's residual cap,
    Pluto's N₂ glacier). T1.b records the rule it chooses.
  - _The bright forms' L(0), ruled_ (decision-r10-limb, 2026-10-13; built in the follow-up commit
    `feat(surface): R10.T1.a The bright forms' Lambert law and the particulate limb rule`). As
    first built, `limb_parameter` gave each form the largest L(0) the f ≡ 1 bound allowed (glacier
    ice of A_N 0.79 took 1, sea ice 0.83 took 0.90, snow 0.90 took 0.49), and the lane leaned to
    keeping it. The ruling rejects it: L(0) is a measured property of how the surface is built,
    and the rule contradicted the measurements at both ends (Europa, Mimas and Tethys are
    limb-darkened at A_N 0.7–0.8; Enceladus, at 0.9–1.4, is "not Lambert"; snow's radiative
    transfer gives L(0) about 0).
    - The consolidated forms, `Glacier`, `Snow`, `SeaIce` and `Evaporite` of any species or salt,
      take the new `PhaseLaw::Lambert` (L ≡ 0 and f ≡ 1), chosen by the form
      (`PhotometricForm::Consolidated`, `PhaseLawOrigin::Form`). Their A_h is A_N at every μ₀, so
      the measured spans are A_N with no conversion and the per-form factors (glacier 0.814, snow
      0.928, sea ice 0.856) stand.
    - The particulate forms, every other form and `Frost` of any species, keep the entry's
      lunar-Lambert row with L(0) = L_p(c A_N), above. A CO₂ or N₂ frost at 0.7 takes 0.84, and an
      H₂O frost at the reference 0.965 takes 0.578; Europa's plains at 0.71 give 0.83 and
      Enceladus 0.38. Pluto, Charon and Europa's mottled terrain are outliers, where the rule gives
      more L(0) than measured (the science check; deferred, below).
    - Energy is enforced on A_N by the law's own f, never through L (`normal_albedo_ceiling`): 1
      for Lambert's, exact; 1.35 for the particulate rows, where the Ice row's A_N × sup η under
      the rule is 1.35 × 0.720 = 0.97 and the Regolith row's 0.72 (decision-r10-limb §3, computed
      with R07's construction of f). A_N above 1 is physical (Enceladus).
    - The "23% too much" does not exist: it was an artefact of f ≡ 1. Under the Ice row's real f,
      the as-built glacier ice reflected 0.58 at μ₀ = 1 against a measured 0.79. Alternative 2
      (A_N = A_h ÷ [1 + 0.227 L]) converts through a law no row has, and is rejected.
    - _Correction:_ Buratti's form, f [A μ₀ ÷ (μ₀ + μ) + (1 − A) μ₀], is this crate's up to scale
      for L = A ÷ (2 − A), so Pluto's and Charon's A = 0.70 is L(0) = 0.54 here, not "about 0.58"
      (`MCEWEN_LUNAR_L`'s comment is corrected).
  - _The bound test, ruled._ T1.a's "every class's A_N and L(0) keep the law's
    directional-hemispherical albedo at μ₀ = 1 at most 1" was the f ≡ 1 bound. The follow-up
    replaces it: every consolidated row's span top is at most 1; `directional_hemispherical_albedo`
    at L = 0 is A_N on a μ₀ grid; every particulate row's span top is at most 1.35; every L(0) the
    rule gives lies in [0.38, 1]. The physical-range test's span top is now only finite, not at
    most 1, since A_N above 1 is physical; its ceiling is the bound test's, by the form's law.
    `directional_hemispherical_albedo` and `LOMMEL_SEELIGER_NORMAL_ALBEDO_RATIO` stay, documented
    as the f ≡ 1 case, exact for Lambert's.
  - _L(0) on the scaled albedo._ T10.a's scale c multiplies A_N, and L(0) is the class law's
    `limb_parameter` of c A_N. That leaves p = c A_N [⅔ + ⅓ L(0)] piecewise linear in c, with
    breaks where c A_N crosses 0.60 and 1.35 for a particulate class (the as-built 0.815 and 1 are
    retired), and p = ⅔ c A_N for a consolidated one.
  - _Slopes._ `CLIFF_SLOPE` is 40° and `TRANSITION_HALF_WIDTH` 5°, as planned. Kleinhans et al.
    2011's 40° is their time-averaged angle (§40), not the static one (§39 and Fig. 9), and the doc
    comment now says so. `SNOW_SHED_SLOPE` is kept at 60° with a source: "Terrain steeper than 60°
    is usually avalanche-free … It sloughs off as soon as it is deposited" (Schaerer 1977, Can.
    Geotech. J. 14, 281, p. 286).
  - _Citations corrected._ USGS PP 1389's Martian soils are in Table 37 (p. 130); the task text's
    Table 7 is the report's Earth simulants. Warren, Wiscombe and Firestone 1990 is JGR 95 (B9).
  - _Closed sets, for the composition audit._
    - `MaterialClass`: fifteen forms.
    - `PhaseLaw`: three laws (`Lambert` appended by the limb ruling's follow-up), and
      `PhotometricForm`'s two kinds, which split the fifteen forms.
    - The modelled (form, family) rows: ten of ninety.
    - The rows are keyed by R09's six `MechanicsFamily`s, so substances of one family share a
      form's figures. A CO₂ frost and an N₂ frost share 0.4–1.0; basalt, granite and
      phyllosilicate share the silicate rows.
- **Deferred findings (T1.a, 2026-10-13 rule)** (each leaves the algorithm and the contracts as
  they are, for "main"'s list).
  - _Static or time-averaged repose._ Kleinhans et al.'s static angles are 41–55°, above the
    band's 45° top, while `CLIFF_SLOPE` keeps 40°, the angle loose slopes relax to.
  - _The spans as a clamp._ Design note 8's spans are Solar System anchors: the lunar regolith's is
    the nearside's 0.057–0.184. They are too narrow for T10.a's clamp on a generated body, whose
    silicate regolith may be as bright as a Vesta's (p about 0.4, from memory). The synthetic
    feldspathic crust's 0.20 already lies above the lunar span. T10.a clamps by its own rule.
  - _The A_h bound at f ≡ 1_ (ruled by decision-r10-limb §3–§4, and built). It was stricter than
    a backscattering surface's, and an entry's A_N above 1 (an Enceladus) broke it for every L.
    The bound is now each law's `normal_albedo_ceiling`, with f: 1 for Lambert's, 1.35 for the
    particulate rows. P14.T49.c's test of "R10 Design note 8's bound" is replaced, in plan 14 by
    main: each row's directional-hemispherical albedo from its own Hapke solution at μ₀ ∈ {1, 0.5,
    0.2, 0.05} is at most 1 per band, and each row's A_N is at most 1.35 per band, so the
    Enceladus anchor passes, its 1.4 terrain clamped to 1.35 (4%, recorded). Pluto's A = 0.70 is
    L(0) 0.54, not 0.58 (above).
  - _Vesić's N_γ at 54°._ The closed form may lie outside the range it was fitted over
    (unverified), so the 1 m pin checks the formula, not N_γ.
  - _Ice's strength and friction._ Schulson and Duval 2009, Bowden and Hughes 1939 and Kietzig et
    al. 2010 were not read. Liefferink et al.'s slow (0.38 mm s⁻¹) kinetic friction stands, and is
    unknown below −97 °C, where most icy bodies' ground lies.
  - _Mars dust in V._ No source read gives a normal albedo in V, so the span stays `None`.
  - _Viking cohesions._ Moore and Jakosky 1989 (Design note 2's 1.6, 1.1 and 5.1 kPa) was not
    read; the abstract ranges' midpoints stand.
  - _A grey factor._ Glacier and sea ice absorb more in R than in B, so a per-band factor would
    follow P14.T49.c's spectra.
  - _One shed slope._ `SNOW_SHED_SLOPE` is terrestrial snow's.
  - _A golden of the table, for T1.b._ No golden pins the table's figures bit for bit, and T1.b's
    planned goldens pin only (class, entry). T1.b adds one through `GoldenWriter` at the version it
    bumps to: every modelled row, the slopes, the factors, `limb_parameter` and L(α) on a grid,
    `BearingFactors` at the table's angles, the two lunar capacities, the friction at its knots and
    `for_family`, and from the limb ruling each form's `photometric_form` and each law's
    `limb_parameter` and `normal_albedo_ceiling`. It goes in a subdirectory
    (`tests/golden/material/`), since `golden_diff.py`
    takes the files directly under `tests/golden/` for the test planet's, under
    `TEST_PLANET_VERSION`; T1.b's `golden/material_*.golden` move there too.
  - _`PhaseLaw`'s place is not its code._ `coded_enum!` gives its variants no discriminants, so
    `PhaseLaw::Regolith as u8` would be 0, the unassigned code. T10.b indexes rows by `u8::from`
    alone, with a test that row k is code k. R09's `wire/codes.golden`, which lists "every code a
    payload carries", could add the phase codes (R09's file).
- **Deferred findings (the limb ruling, 2026-10-13 rule)** (decision-r10-limb's list; each leaves
  the ruled algorithm and contracts as they are, for "main"'s list).
  - _Snow's forward peak._ At a high solar zenith snow's reflectance factor rises well above 1
    forward (Warren et al. 1998; Hudson et al. 2006, not re-read); no lunar-Lambert law has a
    forward lobe, so Lambert's is right at small incidence and too dim forward of a low sun.
  - _Charon, Pluto and Europa's mottled terrain._ Charon's L(0) is 0.54 at A_N 0.4–0.6 (Buratti
    et al. 2017, p. 12), where L_p gives 1; neither L_p nor the form models compaction or regolith
    structure (Europa's compact regolith). The science check adds two outliers of the same sign:
    Pluto, of mean A_N about 0.73, takes 0.80 against its global 0.54, and its brightest regions,
    at 0.8–1.0, which that fit undercorrects for limb darkening and so need less than 0.54, take
    0.72–0.55 (pp. 10–11 and 17); Europa's mottled terrain at 0.48–0.60, "not lunar-like"
    (Buratti and Veverka 1983), takes 1. The ruling's "0.85–1.0 give 0.67–0.55" check is so
    corrected in the code's documentation.
  - _Snow's computation inputs._ The ruling's snow-like medium (w ≈ 0.9999, g ≈ 0.89) has no cited
    source, in the ruling or the code; Wiscombe and Warren 1980 (J. Atmos. Sci. 37, 2712) is the
    science check's likely source for g, not read.
  - _A particulate form under an entry naming Lambert_ (asked of "main"). It takes Lambert's
    (L ≡ 0, ceiling 1), as the ruling's letter has it, against §2's table for particulate forms.
    Lean: as built, with palette sources never writing code 3; the alternative maps code 3 on a
    particulate form to `for_family`.
  - _Dark consolidated ground._ Hapke without shadow hiding gives L(0) 0.3–0.8 below A_N 0.5; the
    consolidated forms take 0 at every albedo. No row is darker than 0.69 in V, but a dusty-ice
    entry could be.
  - _Bright rock._ Bedrock and volcanic rock above A_N 0.6 (marble, anorthosite) take L_p; low
    confidence.
  - _Evaporite's Lambert law._ No BRDF primary was read; salt flats' near-Lambertian behaviour as
    calibration sites is from memory, with §1's physics argument.
  - _The particulate rows' f._ Both are the Moon's curve shape (q 0.63 for Regolith, 0.80 for
    Ice), where Europa's q is 1.01 and Enceladus's 0.92 (Verbiscer and Veverka 1994), so bright
    frost is drawn darker than measured away from opposition: plan 14's per-body s, a later code.
  - _A_E's sensitivity._ L_E = 0.38 rises to 0.52 at B₀ = 1, within Verbiscer and Veverka's loose
    constraint, and the ceiling would then fall to about 1.2. Verbiscer and Veverka fitted Hapke
    1986's equation: with its approximate H (Hapke 1981's) the same parameters give A_N 1.406 and
    L(0) 0.327, against `hapke.py`'s 1.351 and 0.378 with Hapke 2002's H (the science check);
    their θ̄ = 6° enters neither.
  - _Hemispherical figures as A_N for frost._ Warren et al. 1990's CO₂ albedos are hemispherical
    at a solar zenith of 60°; under a particulate law with f, A_N = A_h ÷ η(0.5) is 1.2–1.5 times
    higher. The spans are only checks: P14.T49.c computes A_N from Hapke directly.
  - _L(α) for bright particulate forms._ The Moon's profile scaled by L(0), as built; McEwen
    1991's insensitivity is shown for typical surfaces, not for w → 1.
- **Deferred findings (2026-10-13 rule) (R10.T12's drafts)** (lane 11; recorded, not ruled, but
  for the slope, ruled on the issue), each leaving the algorithm and the contracts as they are now.
  - _The flight path marker has no guide row._ R02.T12.c's marker, a circle 0.375 rem in radius
    with 0.5 rem wings and a 0.3125 rem fin in `--text` (`view/wireframe/symbology.ts`), is drawn
    on every view with an own ship whose velocity points into the picture, but no row or bullet of
    the guide names it, nor a legend entry as `TICKS TARGET` has. T12's Draft 2 counts it among
    the marks the tripod must not resemble, and gives way to it. Lean: a `Legend` row at the next
    guide edit, R02's.
  - _The capacity steps at a layer's edge._ On lunar regolith at 1.62 m/s² the capacity is
    353 kPa under a 0.29 m pad and 4,202 kPa under 0.30 m, and 109 kPa under 0.14 m and 318 kPa
    under 0.15 m, since the layer at one width's depth passes there from one of the Sourcebook's
    intervals to the next (`shear_layer_at`; computed from the formula, and checked again by the
    lane's science check). A pad near those widths reads one side or the other. Draft 1's fixed
    1.0 m pad meets neither; a `PAD WIDTH` selector would (its question on the pad).
  - _A fifteenth friction point._ The lane's science check, reading Liefferink et al. 2021's Fig. 1
    by eye, saw a fifteenth point, partly hidden, near −2 °C at about 0.15, which `STEEL_ON_ICE`'s
    fourteen leave out; it would move the interpolation by less than 0.01. Unverified here; for
    main's list, beside the later T1 follow-up.
  - _No slope in the capacity._ `bearing_capacity` is a level footing's on level ground, so on a
    slope it overstates what the ground bears. Ruled 2026-10-14 by main, on the issue: T1.a stays
    as built, and Draft 1's row says so. Vesić's ground-inclination factors, and the footing-base
    tilt factors that go with them, go to a later T1 follow-up, which adds the slope argument and
    its test before T12 builds the readout.
  - _Regolith on small bodies_ (signoff-10). Every regolith of silicate takes the lunar table,
    whose 3.0 kPa cohesion at 30–60 cm is the Moon's soil compacted under lunar gravity. So on a
    small airless body the capacity stays cohesion-held: 3.71 MPa under 1.0 m at 0.01 m/s²,
    against 5.37 MPa at the Moon's 1.62. The cohesion of asteroid regolith is thought to be far
    lower (not read here). For the later T1 follow-up's list; the readout's contract is
    unchanged.
  - _Loose soils' failure mode_ (signoff-10). Terzaghi's equation with Vesić's factors is
    general shear failure. For a loose, compressible soil such as Mars's drift (φ 18°), Vesić
    gives local or punching shear, which bears less (reduced strength parameters, or the rigidity
    index). Not read here. For the same list.
- **Deviations in T1.b, as built** (2026-10-13, lane r10-classify, on R10.T1.a at `eef1fabb` with
  its limb follow-up `42ae6db3`, landed as `fe12a05e`, and R09.T9 at `070da8ee`).
  - _Files._ `crates/hyperion-surface/src/material/classify.rs` with `classify/tests.rs`, and
    `material/golden.rs`; `material/mod.rs`'s exports; `lib.rs`'s summary line. Three of R09's
    files gain crate-private items, additively: `synth/channels/incision.rs`
    (`Channels::on_channel_floor`), `synth/channels.rs` (`Channels::id` made `pub(crate)`) and
    `synth.rs` (`SynthCache` holds the classifier's cuts). `tests/panics.rs` gains a `material`
    module, and `crates/hyperion-sim/tests/surface_material_worlds.rs` is new.
  - _The goldens are in the library_ (main, 2026-10-13), as R10.T3's and R09.T9's are: there is no
    `tests/material_golden.rs` and no `tests/common/worlds.rs`. `ci`'s wasip1 selection builds
    the crate without `testing`, which the synthetic worlds need. `material::golden` writes seven
    files at header 21 in `tests/golden/material/`, a subdirectory, since `golden_diff.py` takes
    the files directly under `tests/golden/` for the test planet's (T1.a's finding):
    - `<world>.golden`, one a synthetic world, each a thousand points: the cube's corners, up to
      32 trunks' heads, up to 40 points in cells with ice and 40 in the shallowest cells under
      water, and random points, each its (face, u, v), its class and its entry or `-`;
    - `table.golden`, T1.a's deferred table golden, pinned after the limb ruling's follow-up
      (main's instruction): the constants, each class's code and photometric form, each phase
      law's code, template and ceiling and the family defaults, `limb_parameter` and each law's
      L(0) on a grid, L(α) on a grid, a few directional-hemispherical albedos, every (form, family)
      row with its layers' bearing factors and its friction at and between its knots, and the two
      lunar capacities.

    The tests use the synthetic worlds through R09.T8's `synth::test_support`, and fields built for
    each test with R09's `FieldBuilder`.

  - _Acceptance_, as built:
    - `cargo test -p hyperion-surface --lib material`, the goldens alone through
      `--lib material::golden`;
    - `cargo test -p hyperion-surface --features testing --test panics material`;
    - `cargo test -p hyperion-sim --features testing --test surface_material_worlds`;
    - the wasm targets under `just test-wasm-fast`.
  - _The signature._ `material_at` returns `Result<GroundMaterial, QueryHeightError>`.
    `GroundMaterial` holds the form always, and the entry where the palette names the deciding
    rule's substance: `class()`, `entry()`, an `Option<u8>`, and `material()`, an
    `Option<Material>`, T1.a's pair, which `properties` reads. On a body with an empty palette every
    point is so, the guide's `MATERIAL REGOLITH · —` (signoff-9 §17.5, which replaced T13's draft
    `MATERIAL NOT YET MODELLED`). No stand-in entry is ever named. T1.a's `Material` is unchanged.
  - _The order of rules_, as built (`Rule::ALL`; the first whose condition holds decides):

    | Rule             | Holds where                                             | Material                                                      |
    | ---------------- | ------------------------------------------------------- | ------------------------------------------------------------- |
    | Sea ice          | below the cell's water surface, in the cell's ice       | `SeaIce` of the ice entry                                     |
    | Seabed           | below the cell's water surface                          | `Seabed` of the crust's, the `Bulk`'s or a `Deposit`'s entry  |
    | Land ice         | in the cell's ice                                       | `Frost`, `Glacier`, or airless `Regolith`, of the ice entry   |
    | Melt             | annual mean above the crust entry's transition          | `Melt` of the crust's entry                                   |
    | Snow             | frozen all year where it snows, up to `SNOW_SHED_SLOPE` | `Snow` of the snow's entry                                    |
    | Cliff            | slope above `CLIFF_SLOPE`                               | `Bedrock` of the crust's entry                                |
    | Evaporite        | an arid closed basin, a salt in the palette             | `Evaporite` of the salt's entry                               |
    | Sediment         | a closed basin, or a channel's bed                      | `Sediment` of the crust's entry                               |
    | Regolith         | an airless body                                         | `Regolith` of the crust's, a `Deposit`'s or a `Cover`'s entry |
    | Organic sediment | an organic `Cover`, not arid                            | `OrganicSediment` of the cover's entry                        |
    | Dust             | arid and thin-aired                                     | `Dust` of a `Deposit`'s, a `Cover`'s or the crust's entry     |
    | Volcanic rock    | a province's crust                                      | `VolcanicRock` of the province's entry                        |
    | Sand             | arid                                                    | `Sand`, as dust's                                             |
    | Soil             | everywhere else                                         | `Soil` of the crust's entry                                   |

    Three moves from Design note 1's order, each to keep a rule reachable, which
    `material_the_order_of_rules_is_total` checks on every combination of the conditions:
    - Sea ice comes before the seabed: the ice of a cell under water lies over its points below
      the water surface.
    - Melt and snow come before the cliff. Melt above its transition holds no slope. And
      `SNOW_SHED_SLOPE` (60°) is steeper than `CLIFF_SLOPE` (40°), so after the cliff snow would end
      at 40° and its shed slope could never decide.
    - Organic sediment, dust and volcanic rock come before sand and soil, which between them hold
      on all ground.

  - _The entry chains._ The seabed's is the crust's, then the body's `Bulk`, then its first
    `Deposit`. The regolith's is the crust's, then the first `Deposit` not of the salt family, then
    a `Cover`, then any `Deposit`. Sand's and dust's take such a `Deposit` first (the role's
    "source of a body's sand and dust"), then a `Cover`, then the crust, then any `Deposit`: a salt
    deposit is an evaporite's source. The evaporite's is the first salt-family `Deposit`, else one
    in a crust's or a `Cover`'s role; without a salt an arid basin is sediment.
  - _What the cell's substance byte gives._ Its ice entry names the forms of ice. Its liquid entry
    is not read: whether a point is under water comes from the cell's water surface, and snow's
    entry is the cell's ice entry, else the `Ice` entry of the header's main liquid's substance.
  - _The Lambert code is a form's._ Main's ruling on T1.a's limb follow-up: an entry whose
    `phase_row` is 3 gives a particulate form Lambert's law, as built, so a palette source writes
    only 0 to 2. The classifier builds no palette; the synthetic worlds' test checks that none of
    their palettes writes 3.
  - _Melt_ is the folded-in rule alone, the point's annual mean temperature above its crust
    entry's transition. The field carries no province's age, so "a young volcanic province" has no
    rule. On an airless body a province is its entry's regolith (the guide's
    `REGOLITH · Fe-RICH BASALT`); volcanic rock is a province under an atmosphere.
  - _Glacier or frost_ (T1.a's owed rule; decision-r10-limb.md §2, "What T1.b owes"):
    - A cell's ice is massive where its share is at least `MASSIVE_ICE_SHARE`, a half, and thin
      below. That is the ice field's share, since the field carries no thickness, and the half is a
      choice, not a measured figure.
    - Thin ice is `Frost`, on any body: a seasonal frost, or a cold trap's exposed ice.
    - Massive ice is `Glacier` under an atmosphere: Earth's ice sheets, Mars's residual caps,
      Pluto's nitrogen glacier (signoff-9's `ICE · N2`).
    - On an airless body, massive ice is `Regolith` of the ice entry: the ruling's lean, gardened
      ice being particulate, lunar-like and not Lambertian. Main's relay named it `Frost`; both
      are particulate (T1.a's `PhotometricForm`), and neither is ever `Glacier` there (tested).
    - A body is airless where its header's surface pressure is zero, as the header's own field
      defines one. So Pluto's 1 Pa counts as air.
  - _Where a partial share lies_ (R09's "Cold traps" ruling). The ice lies on the cell's
    shadowed ground: its hollows and pole-facing slopes.
    - A point's index is z = t ÷ σ_t + d ÷ σ_d, of the ground finer than the coarse cells. t is the
      fall towards the nearer rotation pole along the surface, per metre, and d the depth below the
      coarse interpolant. Each is over its standard deviation on the cell's grid; the equal weights
      are a choice.
    - The cell's cut is the 1 − s quantile of z over a 16 × 16 grid of the cell, by Hyndman and
      Fan's definition 7. A point is ice above it, so the ice covers its share to the grid's
      sampling. The smallest share, 1 ÷ 255, lies within the grid's 256 points.
    - The caller's `SynthCache` keeps each cut by its cell, level and ice code, for the field the
      channels' memo is kept for (`NetworkId`: the body, the seed and the header's marks), emptied
      at 16,384 cells. A cut is a pure function of those, so no answer depends on it (tested across
      fields and seeds). A first query of a partial cell costs the grid's 256 heights, several
      milliseconds natively at the Moon's finest level (provisional: not timed on a quiet machine).
    - A Gaussian quantile of z, the first build, was rejected: craters and steep walls give z heavy
      tails, and on a cratered airless pole the ice covered 0.03 of a cell against its 0.2.
    - A cell where neither term varies is ice where its share is above a half. Sea ice is placed
      so too, since a sea's ice does not follow its floor.
  - _Snow_ is perennial snow alone. It lies where the point's warmest month, at its height by the
    lapse rate, is below the snow entry's transition, and the cell precipitates. The class takes no
    month, so seasonal snow and frost wait (deferred, below).
  - _Arid_ is a mean precipitation of zero, or below Köppen–Geiger's arid bound: MAP < 10 ×
    (2 T, 2 T + 28 or 2 T + 14, as 70% falls in winter, in summer, or neither), T in °C (Peel,
    Finlayson and McMahon 2007, Table 1), in millimetres of water a Julian year at the point's
    annual mean. Peel's summer is the warmer of October to March and April to September; a
    generated calendar has neither, so it is the year's six warmest months. The bound is negative
    below −7 °C (−14 °C summer-heavy, 0 °C winter-heavy), so colder ground is arid only where
    nothing falls. Mars-like is arid everywhere.
  - _A closed basin_ is a dry cell whose flow is `Terminal`, with a drainage area and a neighbour
    flowing into it. The inflow test keeps out the flat cells the builder's routing leaves
    terminal. The whole cell is the basin's floor.
  - _A channel's bed_ is where the point lies within half a segment's bankfull width of one of its
    chords, at a level the incision evaluates there. `Channels::on_channel_floor` reads what
    `incision_at` reads of the network. Away from a chord's ends the chord sags below the surface
    (R09.T6.a's note), which narrows a floor narrower than the sag.
  - _Dust._ R09's climate record carries winds but no deposition. So every arid, thin-aired loose
    ground is dust: the Risks' simple rule. Thin-aired is below `THIN_ATMOSPHERE_PRESSURE`, 30 kPa,
    the sim's photometric threshold, reused with no aeolian source and repeated here because this
    crate cannot see the sim; the sim's binary checks the two are one.
  - _An organic cover_ is a palette's `Cover` entry of the organic family.
  - _The slope_ is R05's `surface_normal` against the datum's normal, at R05's finest level, as
    `ground_at` reads it. That is the gradient's tangential part (R09's "Deviations in T4, as
    built"), scaled by ρ ÷ (ρ + h). On the small cliff body of the tests the scale is ⅔ at 1 km,
    which the test's own measurement of the slope, through surface points, confirms. The slope
    tests leave out the 0.5° either side of `CLIFF_SLOPE` and `SNOW_SHED_SLOPE`, where that
    measurement and the classifier's reading may round apart.
  - _R09's `SurfaceClass` is not read_: until R09.T15 assigns its codes every cell is unclassified.
  - _The `sample_worlds()` cases_ (a warm pole without an ice cap, a locked world's ice where R09's
    field puts it) follow R09.T16 as a later commit under T1.b's ID (main's ruling).
    `surface_material_worlds.rs` meanwhile runs the four reference worlds through `coarse_pass`.
    Their no-op pass leaves a smooth dry sphere with no ice, where every entry is the palette's and
    the airless Moon's and Ceres's ground is their crusts' regolith.
  - _Built fields_ (illustrative values, each noted):
    - the cliff field, a 2 km airless body with a 1 km step;
    - a cold snowy cliff, and a warm stepped field with a lapse rate;
    - a closed basin at 300 K, dry or wet, with a halite deposit or none;
    - decision-composition's Pluto-like world: a nitrogen glacier in a basin, methane frost on 0.3
      of the mid-northern cells, a tholin cover and 1 Pa of air, and the same world airless;
    - its locked lava world: basalt under 2,474 K cos^¼ θ on the day side (CoRoT-7 b's);
    - a Titan-like world, an organic cover under 1.47 bar, with methane rain or none;
    - a cratered airless pole with 0.2, 0.6 and 0.8 of water ice;
    - a frozen sea, and seasonal climates for Köppen–Geiger's offsets.

    So `Frost`, `OrganicSediment` and frost of methane are reached on built fields; no sample world
    reaches them (Risks, "Classes plan 14 cannot yet reach").

  - _Tests_ (`material::classify::tests`, `material::golden`, `tests/panics.rs`'s `material`, the
    sim's binary):
    - the order of rules is total, every rule reachable, every rule's form its own;
    - a missing entry keeps the form, and airless ice is never `Glacier`;
    - each rule's entry chain;
    - the cut's quantile;
    - on the synthetic worlds every entry is in the palette, and the answers are the same bits
      from a fresh, a shared and a reversed cache;
    - order and cache independence (`assert_order_independent`) on partial ice and channels' beds,
      a cache kept across fields and seeds, and a partial-ice cell answered within its margin;
    - a cell not held is not surveyed;
    - `Seabed` exactly below the water surface, of the floor's entry and never the liquid's;
    - a channel's bed is `Sediment`, and a point beside it is off it;
    - a `cliff_field` is `Bedrock` above 40° and not below;
    - snow lies to 60° and is bedrock above it, and lies only above the snow line;
    - an arid filled basin is `Evaporite`, a wet one or one without a salt `Sediment`;
    - Köppen–Geiger's bound at 20 °C and its seasonal offsets;
    - the Pluto-like world's `Glacier · N2` and `Frost · CH4` on about its share, and its airless
      twin's regolith of nitrogen;
    - the lava world's `Melt` on the day side and regolith on the night side;
    - the Titan-like world's `OrganicSediment` where it rains, and sand where it does not;
    - the cold-trapped share on its share of the cell, on its most shadowed ground, lower and more
      pole-facing than the bare ground;
    - sea ice by the majority;
    - a face-edge point alike from either face;
    - the goldens, and their header;
    - one `should_panic`: a (u, v) that is not finite;
    - the reference worlds, and the thin-air threshold the sim's.
  - _Generator version._ Not bumped (main, 2026-10-13). R09's first bump rides plan 14's held 21 →
    22 batch (P14.T48.e), as R09.T9's did, and that batch re-blesses these goldens. The class
    codes, the rules, their order and thresholds, the per-cell cut and the property table are
    generator output from then on.
  - _Closed sets, for the composition audit._ The fourteen rules. The three regimes, airless, thin
    and thick, cut by two pressures. The entry chains of each rule.
- **Deferred findings (T1.b, 2026-10-13 rule)** (each leaves the algorithm and the contracts as
  they are, for "main"'s list).
  - _No month._ The class is static. Seasonal snow and frost (Mars's CO₂ frost, a winter's snow)
    wait on a month argument and P14.T55.e's frost by month.
  - _Köppen–Geiger is water's and Earth's._ It is fitted to Earth's station climates, and says
    nothing of gravity, insolation or air density. A methane world's aridity has no bound but
    "nothing falls", so a Titan-like world that precipitates is never arid, and its dunes wait on
    R09.T15's classes.
  - _Snow without ablation._ Any snowfall under a summer below freezing gives perennial snow; polar
    deserts such as the Dry Valleys stay bare by sublimation and wind. An accumulation against
    ablation test would separate them (the science check; qualitative).
  - _Sea ice by the majority_ loses area: a 0.4 concentration shows none and 0.6 shows full cover,
    up to half a cell over a marginal ice zone. A seeded field's quantile, independent of the
    floor, would keep the share.
  - _Melt at the solidus._ The transition is a lithology's solidus, just above which the rock is
    mostly crystal and holds a slope, so "melt holds no slope" is true only well above it.
  - _A locked world's cold side._ The tilt is towards the rotation pole. On a locked world the
    cold side is away from the substellar point, which the header does not carry.
  - _Dust and the wind._ No deposition term, so all arid thin-aired loose ground is dust, and sand
    never appears on a Mars. A deposition or wind-stress field from R09 would place both.
  - _Airless is zero pressure._ The sim's `AIRLESS_PRESSURE` is 100 Pa, below which a body keeps
    its partial pressures, and R11 takes 100 Pa as airless (its line 439), so the two plans place
    rocks and ice under different rules. R09's `for_body` (P14.T48.e) should write 0 for an
    `Airless` state, or such a body's ice reads as under air (`Glacier`); 100 Pa here would make
    Pluto's nitrogen glacier a regolith.
  - _The massive share's half_ is a choice. A cell under half covered is `Frost`, so a small
    mountain glacier reads `FROST` where the guide's `ICE` is a glacier's. A thickness, or plan
    14's caps and frosts apart, would decide it.
  - _A margin cell's cut_ reads its grid's heights, which may reach cells a view lacks where the
    point's own height answers, so such a point is refused.
  - _The coastline follows cells._ A dry cell's ground below the sea level is dry land, and a sea
    cell's ground above its surface is land.
  - _A basin's whole cell_ is its floor, so a playa is as wide as its terminal cell.
  - _Ice on cliffs._ Ice precedes the cliff rule (Design note 1), so a thin frost or an airless
    ice regolith lies above 40° too.
  - _The channel floor repeats the groove's geometry._ `Groove::floor_holds` repeats `Groove::at`'s
    nearest point on a chord, and `on_floor` the incision's level range. A change to either should
    move both; a shared helper in R09's file would keep them one.
- **Deviations in T1.d, as built** (2026-10-19; lane r10-weathering, after R09.T2.f at `7d279126`;
  decision-p14-t51d-weathering §1, its addendum of 2026-10-19 "Λ's fit inputs", and
  decision-r10-t1d-anchor-colour). Pure at 22: nothing on the record reads the law, and
  `field_albedo_scale` weathers only a header that carries a maturity, which none does before
  P14.T54.b, so no golden but the new `weathering.golden` moves and `GENERATOR_VERSION` stays 22.
  - _Files beyond the list:_ `material/weathering/tests.rs`, `material/{mod,golden}.rs` (the golden
    and `MATERIAL_GOLDENS` 8), `tests/panics.rs`'s `weathering` module (eight panics),
    `tests/golden/material/weathering.golden`; in `hyperion-fit`, the task's `tests.rs`,
    `tests/space_weathering.rs` (the reproduction and the Moon's tests), `src/task.rs` (the
    registry, 39 tasks, and its surface list), `src/tasks/mod.rs`, `tables.lock`, and the datasets
    `johnson_christy_1974_fe` and `ordal_1988_fe` (refractiveindex.info's CC0 YAML files,
    unmodified, at the database's commit c5c2f188), with NOTICE's iron entry and two
    `.prettierignore` lines so that no formatter rewrites the hashed files.
  - _Public items_ (`hyperion_surface::material`): §1.4's `npfe_optical_depth`,
    `weathered_normal_albedo`, `weathered_spherical_albedo` and `weathered_entry_bvr`, with
    `weathered_hemispherical_albedos(fresh: [f64; 4], …)` at the table's four incidence cosines
    (1, 0.5, 0.2, 0.05, the sim's), each inverting its own quantity's column; and the law's
    constants `REGOLITH_HOST_INDEX` 1.7, `ICE_HOST_INDEX` 1.31 (Warren and Brandt 2008 through
    R08's `water-ice.json`), `IRON_DENSITY_KG_M3` 7,874, `IRON_MOLAR_MASS_G_PER_MOL` 55.845 and
    `FEO_MOLAR_MASS_G_PER_MOL` 71.844, which the fit reads. Out-of-range inputs panic, as
    `material/maturity.rs`'s do (the palette and the header validate first). The maturity and
    FeO_T stay bare `f64`, as the ruling's signatures name them (the Rust review's newtype
    suggestion, not taken).
  - _The table_ (`tables/space_weathering.rs`, task `space_weathering` revision 0, fast): 129
    single-scattering albedos uniform in √(1 − w); each form's `A_N`, `A_S` and `A_h` at the four
    incidences by `substance_albedos`' own `Medium` and `energy_albedos`, refused unless strictly
    increasing in w and at most 1; Hapke 2012's slab at k = 0 (S_e 0.1165, S_i 0.6943 at n_h 1.7),
    held to `single_scattering_albedo` to 10⁻¹²; ⟨D⟩ = ⅔[n² − (n² − 1)^{3/2} ÷ n] D, 63.54 µm for
    the regolith's 70 µm grains and 83.57 µm for the frost's 100 µm. The inversion's error is
    within 10⁻⁴ of w, and 10⁻³ in the node interval holding the regolith form's B₀ knee at
    w 0.4588 (5.5 × 10⁻⁴ measured); a node at the knee would cut it, not taken.
  - _Iron:_ Johnson and Christy 1974 to 1,937 nm, then Ordal et al. 1988 times the join factor
    z_JC ÷ z_Ordal at 1,937 nm, 1.1820 at n_h 1.7 and 1.1626 at 1.31, so z is continuous (a plain
    switch steps it by 18% and 16%); n and ln k linear in λ between rows. The grid runs 300–5,000
    nm every 10 nm, 471 nodes, through P14.T49.i's last infrared bin (decision-r10-t1d-anchor-colour
    §4.1; the ruling's 4,000 nm would overstate z by ×1.29 at 4.5 µm and ×1.62 at 5 µm), and the
    end node's z is held beyond it. 36π z ÷ λ is 4.09, 2.62 and 2.27 × 10⁷ m⁻¹ in B, V and R.
  - _The frost's Λ_ is derived, Λ × ⟨D⟩_frost ÷ ⟨D⟩_regolith, the yield being one constant and Λ
    its product with the form's mean path (§1.1, step 4); not ruled, small and reversible.
  - _The stamp:_ since 22, the version in force, as P14.T49.c's `substance_albedo` was held:
    `hyperion-fit check` refuses a table ahead of `GENERATOR_VERSION` (main's ruling, 2026-10-19).
    P14.T54.b restamps it `--since 24` when it wires the law. The emitter refuses a changed body at
    an unchanged since: a refit before 24 deletes and re-emits the table, as T49.i's held refit did.
  - _The fit_ (addendum; decision-r10-t1d-anchor-colour): weighted least squares at m = 1 over B,
    V and R of both terrains, σ = (P90 − P10) ÷ 2.563, a 100-step golden section in ln Λ over
    10⁻⁸–10⁻⁴ m; the fresh `feldspathic` and `basalt_fe` rows (P14.T49.i) at the anchor soils'
    FeO_T 0.055 and 0.16 (Morris 1980, Table 1) and the rows' grain densities; the anchors
    Dollfus and Bowell 1971's Table 4 at 600 nm (highland 0.114, 0.096–0.142; mare 0.072,
    0.062–0.082) taken to 440, 550 and 640 nm by their Eq. 11's slope, (λ ÷ 600 nm)^0.83, computed
    with no rounding. One colour for both terrains: the terrains' mean B/V differ by < 1% at zero
    phase (Gehrels et al. 1964, Table XIII), and the maria's own ±3.5% spread is not modelled. The
    decision record compared these colours with laboratory LSCC/RELAB soil spectra; RELAB's terms
    are non-commercial, so nothing from them is committed.
  - _The residuals, as fitted_ (on P14.T49.i's rows at origin's 0e9adf71): Λ 2.1301 µm, the
    ruling's expected 2.131. The highlands come out 0.0876/0.1226/0.1368 against
    0.0881/0.1061/0.1203 (z −0.04/+0.99/+0.87), all inside, their own best Λ 2.434 µm. The maria
    come out 0.0519/0.0570/0.0602 against 0.0557/0.0670/0.0760 (z −0.62/−1.37/−1.91), their own
    best Λ 1.425 µm:
    - B is inside;
    - V is 0.05703, below its P10 edge 0.05768, at +0.00026 over the floor P10_V − σ_V ÷ 8 =
      0.05677;
    - R is the pinned miss.

    The model's mare B/R is 0.863 against the anchors' 0.733. y_sat = Λ ÷ 63.54 µm = 0.0335 per
    FeO-iron (I_s/FeO about 81 by Eq. 4), inside the window 0.0103–0.093 about Morris's 3.2 × 10⁻⁴
    per FeO (4.117 × 10⁻⁴ per FeO-iron) × 75 = 0.0309, beside the mature soils' medians, mare
    0.029 and highland 0.040. The fresh/mature contrasts are 7.00/5.44/5.10 (highland) and
    7.07/7.19/7.06 (mare) in B/V/R, against fresh craters over the medians at 600 nm, Tycho
    1.34–1.48, Proclus 1.58–1.68 and Kepler 1.68 (D2). The frost's Λ is 2.802 µm, and each row's
    `A_N` inverts to its own w within 2.7 × 10⁻⁵. The generated Moon at the registry's 0.05 and
    0.20 (decision-r10-t1d-anchor-colour §3, stated for P14.T54.b): highlands 0.094/0.132/0.147,
    maria 0.051/0.054/0.056, grey, and a highland-to-mare V of 2.45 against the measured 1.58.

  - _The tests_ (`tests/space_weathering.rs`): the highlands inside in B, V and R; the maria
    inside in B and at least the computed floor in V; Λ within 2.0–2.4 µm; y_sat in its window;
    pinned as known misses, with their causes (the form's dark end, D1; the stand-in fresh rows,
    D2): the mare R's z within −2.3 to −1.5 and the model's mare B/R within 0.85–0.90 (about 0.86
    against 0.733). A broken pin means a refit, never a wider bound.
  - _The scale._ `field_albedo_scale` rebuilds the palette with `weathered_entry_bvr` at the
    header's maturity and solves c on it; without a maturity the path is unchanged, bit for bit.
  - _The golden_ holds τ, `A_N`, `A_S` and the four `A_h` on a grid of fresh albedo (0.05, 0.2,
    0.667, 1.2), f (0, 0.05, 0.2), m (0, 0.5, 1), λ (250, 440, 550, 640, 2,200 and 5,200 nm, the
    grid's two outsides among them) and law, a line whose seven outputs are the fresh value's bits
    reading `same`, and `weathered_entry_bvr` on every `testing` world's entries at m 0.5 and 1.
  - _Acceptance, as built:_ the plan's commands, and three that its filters miss:
    - `cargo test -p hyperion-surface --lib material::golden`, the native golden;
    - `cargo test -p hyperion-surface --test panics weathering`;
    - `cargo test -p hyperion-server albedo_scale`.
  - _The ruling's findings_, recorded and deferred (each a value change, none a contract's):
    - D1, the regolith form's dark end: the in-situ maria map near the slab's floor, so one Λ
      misses the mare R and greys the maria; to the form's next revision, then a refit;
    - D2, the stand-in fresh rows (clean separates): the fresh/mature contrasts are 5–8× against
      the measured 1.3–2.4, so R10.T1.c's young ejecta will be about 2× too bright; to T49.i's
      whole-soil follow-up and R10.T1.c's contrast test, which states it;
    - D3, lateral mixing: mare soils carry highland material (16.5 wt% against 18–22), not
      modelled, so the generated maria are pure lid;
    - D4, phase reddening: resolved for the anchors by Eq. 11's zero-phase colour;
    - D5, each spot's own FeO_T (Lunar Prospector, Clementine) at the next refit, which removes
      Λ's ≈ 1 ÷ f_h systematic: f_h 0.045, 0.05, 0.055, 0.06 and 0.065 give 2.75, 2.43, 2.19,
      2.01 and 1.86 µm on the addendum's anchors, and the mare's 0.14–0.18 moves Λ by at most
      0.03 µm;
    - D6, the anchors' quantity: Dollfus and Bowell's scale is built from catalogues at phase
      1.5–12° (Table 7), never mixed with coherent-backscatter zero-phase values;
    - D7, licence: nothing from RELAB is committed;
    - D8, surface-correlated npFe⁰: rims make τ per grain nearly independent of D, for T49.i's
      grain classes and the infrared;
    - F1, LROC WAC per-terrain colour (public domain) with D5 at the next refit; F5, the solar
      colour (a modern (B − V)☉ takes Λ about 2.131 → 2.114 µm); F6, the maria's TiO₂ colour
      spread, the fresh rows'.
  - _Also deferred:_ Cahill et al. 2019's iron constants (Icarus 317, 229; CC BY 4.0, measured for
    npFe⁰ work, not fetched: the repository sits behind a bot wall) could replace Johnson and
    Christy with Ordal; Morris's largest Fe⁰_A (33 nm) sits at the Rayleigh regime's edge at
    440 nm, and bulk constants ignore size-limited damping, both spectral-shape values; Hapke 2001,
    Lucey and Riner 2011 and Hapke 2012's slab fits stay _from memory_.
  - _Not taken from the reviews:_ sharing `star_colour/columns.rs`'s refractiveindex.info reader
    (another task's file; the task carries its own small reader), and a modifiers-tripwire pattern
    for the law (another lane's test, for main).
- **Deviations in T1.c's Rust half, as built** (2026-10-20; issue lane 50, on
  `origin/rendering-and-planets` at `921a0ba8`; decision-r10-t1c-maturity). Pure at 22: no header
  or palette outside the `testing` builder carries a maturity, a rate or a cover, so every exposure
  the server and the worker take is `GroundExposure::NONE`'s and every output is as it was; the
  golden-diff script shows only `tests/golden/material/maturity.golden` new, and
  `GENERATOR_VERSION` stays 22.
  - _Files beyond the list:_ `material/albedo/tests.rs` (the solve's identity on every world),
    `material/classify/tests.rs` (`pluto_like` moved out), `worker/tests.rs` (the identity on
    every world, and the weathered photometry and point query), `material/weathering.rs`
    (`check_unit` made `pub(super)`, the exposure's and the one-crater formula's check) and
    `synth/test_support.rs` (`along_chord`, the crater transect the tests and the golden share).
  - _Public items_ (`hyperion_surface::material`): `GroundExposure` (`NONE`, `of_header`,
    `with_maturity`, `with_organic_cover`, `maturity`, `organic_cover`; no constructor of two bare
    `Option`s, so the two fractions cannot be swapped), `exposed_properties`, `takes_organic_cover`,
    `maturity_at`, `crater_maturity` (the one-crater formula), `excavated_share`,
    `reworked_depth`, `halo_share`, `exposed_albedo_scale`, and the constants
    `HALO_LENGTH_RIM_RADII` 3, `HALO_EXTENT_RIM_RADII` 10, `EXCAVATION_DEPTH_RATIO` 0.084,
    `REWORKING_DEPTH_AT_ONE_MYR` 0.022 m and `REWORKING_EXPONENT` 0.45; in `worker`,
    `SurveyedMaterial::maturity`. Crate-private: `MaterialProperties::with_normal_albedo_bvr`,
    `classify::organic_cover_entry` (the classifier's own rule, which `Entries` now calls) and
    `SynthCache::craters_mut`; and, for the tests, `testing`'s `moon_like_builder`,
    `moon_like_crater_params`, `one_crater_builder`, `one_crater_centre` and `pluto_like`.
  - _Deviations:_
    - `albedo_scale` keeps its three arguments, the fresh solve: the sim's
      `tests/surface_albedo_worlds.rs` and the server's `tests/surface_service.rs` call it, outside
      the task's files. The exposure goes to a new `exposed_albedo_scale`, of which `albedo_scale`
      is the `GroundExposure::NONE` case; the server calls it at `GroundExposure::of_header`.
    - `maturity.rs` keeps its own copy of `synth/craters.rs`'s private `neighbourhood`, that file
      being another lane's; one shared helper is for whichever lane next edits both.
    - The sim's tripwire `modifiers_nothing_on_the_record_reads_the_modifiers_yet` (until
      P14.T54.b deletes it) reads every file of the surface crate but `material/maturity.rs`, less
      its inline test module and comment lines, for the time law's names and the module's path,
      and allows them on `material/mod.rs`'s one re-export line alone. So the module's tests stay
      inline in `maturity.rs`, the maturity golden is written there and only counted in
      `material/golden.rs` (`MATERIAL_GOLDENS` 9), and the re-export is one line under
      `#[rustfmt::skip]`. `just test-wasm-fast`'s wasip1 run of the sim caught the first layout.
    - The search draws no cell that provably lies beyond the octave's largest halo (`is_beyond`:
      a cell's corners' angles from its centre, or S2's quadratic `kMaxEdge` bound to skip them,
      with a relative margin of 10⁻⁹ and an absolute one of 10⁻¹² radians), and no octave that
      draws no crater (`can_expose`); neither moves a value, which the halo search's test holds
      against a direct search of a wide neighbourhood at the octave's own level. Without the
      pass-by a lone query on the weathered Moon-like world took about 44 ms (provisional, at a
      1-minute load of about 4), most of it drawing some 400 craters an octave for about 8 within
      reach.
    - The solve's per-form rule replaces T1.d's: `weathered_palette` is gone, and where a header
      carries a maturity `Melt` stays fresh and a carbon ice takes the cover, where T1.d's solve
      weathered every entry; T1.d's error-and-`None` path goes with it, the solve reading the
      header's validated palette. Neither part is carried at 22, so the change reaches the
      generator-versioned `albedo_scale` with P14.T54.b's 24, whose goldens carry it.
    - The cover's choices: `A_cover` is the cover entry's `A_N` weathered to the same maturity
      (the identity at its `FeO_T` of `None` or 0), with no form factor, while the material's own
      albedo takes its form's factor before the blend; `Melt` keeps both fresh but still takes the
      cover where its entry does (`maturity_and_cover_weather_both_then_blend`).
    - The moved `pluto_like`'s tholin is a dark red 0.06/0.09/0.14, illustrative, in place of
      0.5 in every band, so that the cover's blend can be told apart, and its basin's `arc` is
      `testing`'s, atan2 of the cross and dot products, where the classifier's tests had acos of
      the clamped dot: a cell site within about 10⁻¹⁶ radians of the basin's edge could fall the
      other way. The classifier reads no albedo, no golden pins the world, and its tests pass as
      they did.
    - `GroundExposure`'s fractions accept −0, as the law's inputs do; a −0 cover is no cover.
    - `maturity.golden` pins `maturity_after` directly, on a grid of exposures and rates, which
      answers plan 14's "Not pinned across targets" for it (P14.T51.d's record); `exposed_fraction`
      stays P14.T54.b's.
  - _The halo, printed_ (the one-crater test): p(2), p(4), p(6) and p(8) are 0.702, 0.335, 0.146
    and 0.050 against North Ray's 0.60–0.62, 0.25–0.30, 0.14–0.15 and 0.085–0.095 (the ruling's
    ranges) and South Ray's 0.68, 0.24, 0.13 and 0.08 (the science check's reading of Fig. 3).
  - _The point query's time_ (the measurement test `maturity_at_s_time_per_point`, native at the
    dev profile's opt-level 2 on the 8-thread laptop, its 1-minute load 2.4–3.7 from another
    lane's data reductions, so provisional): a lone point from a fresh cache 5.4, 3.6 and 3.1 ms on
    the 4.4, 3.5 and 1 Gyr worlds, and a vertex of a level-14 patch through one cache 224, 214 and
    229 µs.
  - _Acceptance, as built:_ the plan's commands (the golden itself runs under
    `material::maturity`, as `maturity_matches_its_golden`, and `material::golden` counts it), and
    those its filters miss, each `cargo test -p hyperion-surface` with:
    - `--lib worker::tests`;
    - `--lib material::albedo`;
    - `--lib material::classify`;
    - the slow `--lib material::maturity -- --ignored maturity_immature`.

    And the sim's `cargo test -p hyperion-sim modifiers_nothing_on_the_record`, whose tripwire
    reads this task's files (above).

  - _The pins against the replica_ (the contrast at m_local 0 against 1, then at Tycho's 109 Myr,
    m 0.283, in B/V/R), each within 0.25% of the ruling's replica and pinned to 1% of the code:
    highland 6.999/5.442/5.104 and 2.456/2.143/2.045; mare 7.065/7.189/7.059 and
    1.677/2.116/2.226. At 810 Myr the V contrast is below 1.10 for both rows.
  - _The known miss, D2, printed_ (600 nm by V and R linearly; Dollfus and Bowell 1971's ranges
    over T1.d's medians): Tycho (highland, 109 Myr) V 2.143, R 2.045, 600 nm 2.089 against
    1.34–1.48, 1.41–1.56 times too bright; Kepler (mare) at 50 Myr 3.374 at 600 nm and at 100 Myr
    2.289, against 1.68, 2.01 and 1.36 times; the model reaches Proclus's 1.58–1.68 at 210–244 Myr
    (highland), Tycho's 1.34–1.48 at 283–362 Myr and Kepler's 1.68 at 173 Myr, beside the m 0
    limits (highland V 5.44, R 5.10; mare V 7.19, R 7.06).
  - _The measured shares_ (the slow test, 10⁴ points uniform in area, at maturity 1 and k = 1),
    the share of the ground whose deficit m_body − m_local is above 0.05, 0.2 and 0.5:
    - the Moon-like world at 4.4 Gyr: 4.0%, 0.71% and 0.13%, against R4's 0.3% above 0.2;
    - its 3.5 Gyr variant (N(>1 km) 4.8 × 10⁻³): 53.9%, 12.5% and 1.64%, against R4's 16% and
      2.1%; the guard, under 10% above 0.5, holds;
    - its 1 Gyr variant (N(>1 km) 8.4 × 10⁻⁴): 56.1%, 14.4% and 2.15%.
  - _Grier's check_, in closed form from the production and the chronology at the present flux
    (400 bins of D from 1 to 20 km, each bin's share of N(>1 km) times the chronology's count over
    the age below which its rim deficit, g f_ex (1 − m(t)), is above the threshold): 4.1 × 10⁻⁴
    km⁻² above 0.2 and 1.75 × 10⁻⁴ above 0.5 on all three surfaces, the ruling's 4.2 × 10⁻⁴ and
    1.7 × 10⁻⁴, against Grier et al. 2000's 1.26 × 10⁻⁴ with "some discernable degree of
    immaturity".
  - _The mean V excess_ of the sampled ground's exposed albedo at m_local over its albedo at
    m_body (R5b): 0.44% at 4.4 Gyr, 5.2% at 3.5 Gyr and 5.7% at 1 Gyr, against R5b's upper bounds
    of 0.4% and 18–20%.
- **Deferred findings (2026-10-13 rule) (R10.T1.c's Rust half)** (recorded, not ruled but where
  stated; each leaves the algorithm and the contracts as built):
  1. D2: Tycho is 1.4–1.6 times too bright at its own age, from the stand-in fresh rows; the
     comparison is stated, and asserted within the measured range after T49.i's whole-soil rows.
  2. Rays are not modelled, nor a coarse crater's halo beyond its reach.
  3. Steep slopes' freshness from about 10°, mass wasting's, waits for D2's fix.
  4. The mature layer is by surface age alone, the Moon's reworking, not by the body's own flux.
  5. Seasonal ices take the cover: `organic_ground` drops them, which the palette cannot tell.
  6. Several craters take the largest deficit, not exact compositing.
  7. The cover's mix keeps the material's one law, where Design note 8 would sum each law's
     radiance.
  8. Young craters on covered ice keep the cover.
  9. New craters' roughness splotches are not modelled.
  10. One halo serves every size and body (Mercury's ejecta reach about 2 R).
  11. The OMAT excess stands in for the fresh share.
  12. Kepler's age is a weak, morphological upper bound, and Proclus has none.
  13. The haloes in the solve: ruled for a second Rust follow-up before or with the client half;
      the mean V excess measured here (above) sizes it.
  14. λ = 3.0 R is the ruling's value, but the science check's fits of the model to North Ray's
      four quoted points give 2.34 R (linear), 2.45 R (shifted to 0 at 10 R, the code's form) and
      2.72 R (log space), and to North Ray's and South Ray's whole profiles 2.3–2.8 R; only a
      log-space fit with a free rim amplitude, which puts the rim at 0.75–0.80, gives about 3.1.
      Kept; a change moves only `maturity.golden`, the point maturity being presentation.
  15. The ruling's Fig. 3 ranges are North Ray's alone; South Ray's are 0.68, 0.24, 0.13 and 0.08
      at 2, 4, 6 and 8 R.
  16. Tycho's 1.34 is region D3's, a 65″ (about 121 km) average wider than the crater; B42's
      spot alone gives 1.48.
  17. Morris 1978's reworking depth is fitted over about 1–500 Myr and is extrapolated here to
      4.4 Gyr (d_m 0.87–0.96 m on old ground).
  18. Robinson et al. 2015's quote and Lemelin et al. 2020 were not read (the science check could
      reach neither text); Lemelin et al.'s subject is impact melts, so its support for the slopes'
      freshness is unconfirmed.
  19. The point query's cost for the client half: a vertex costs about 220 µs through a warm
      cache, about 0.9 s a 65 × 65 patch, so a per-texel bake through `maturity_at` alone is far
      over the brainstorm's 40 ms a patch; the client half wants a per-patch gather of the craters
      whose haloes reach the patch, evaluated per texel, or a coarser maturity grid.
- **Deviations in T5, as built** (2026-10-13; lane r10-readings, begun on R09.T8 and R10.T3 at
  `627e9b35` and finished on R09.T9 and R10.T1.a at `4474e781`; the findings and the interface
  ruled in `decision-r10-t5-closed-forms.md`).
  - _Files._ `reading.rs` with `reading/tests.rs`; `lib.rs`'s `pub mod reading` and its doc;
    `tests/panics.rs`'s `reading` module; `crates/hyperion-sim/tests/surface_reading_worlds.rs`;
    and, as ruled, R09's `synth.rs`, `synth/levels.rs` and `synth/relief.rs` (the level-keyed
    closed forms, below) with R09's plan text.
  - _The survey comes from the view._ `surveyed_reading(synth, cache, at, slope_baseline)` takes no
    `survey: Metres`. It and `slant_range`, which must read the codes along the line of sight
    anyway, read each cell's code through a new public trait, `reading::SurveyView: FieldView`
    (`resolution(cell) -> Option<ResolutionCode>`: a survey's code, `NONE` for a margin cell,
    `None` for a cell not held), implemented for `PartialField`. A `CoarseField` has no survey; a
    server request (Design note 7's) would wrap the field and the ship's cover in a view.
  - _The level._ A cell of code c is read at the deepest band level, to the band limit's, whose
    `λ_n` codes no finer than c, compared in the code's 9% steps, so that a pass at 2 m, which codes
    "2.15 m or finer", reaches the band limit. The band limit's level, the shallowest whose `λ_n` is
    2 m, is resolved: the elevation is `ground_at` at the datum point of `at`, bit for bit, and both
    uncertainties are zero.
  - _The uncertainty is what the level leaves out_ (Finding A, ruled).
    - R09's level sets evaluate a contribution by its own scale, while its closed forms place each
      at wavelengths of their own. A structural octave of spacing s carries the degrees
      N(m − 1) ≤ l < N(m), wavelengths 8/3 s down to 4/3 s, so the octaves a level evaluates
      (s ≥ `λ_n`) leave out wavelengths up to 4/3–8/3 `λ_n`.
    - So R09's `unresolved_rms(cell, λ_n)` understates what the level leaves out, 2.6–3.3 times in
      the relief's variance on the synthetic worlds; the relief's realised mean square left out
      matches the closed form at 1.6–2 `λ_n`. Read so, the Moon-like world's pooled RMS was
      2.57 σ.
    - So the σ, and the slope's v, are R09's closed forms by contribution, each from its own cut
      for the level. As ruled, they are R09's public level-keyed forms, added by this task to
      `Synthesiser`, which T2 and T4 read too: `omitted_variance(cell, level)` (per contribution),
      `omitted_structure_function(cell, level, lag)` and `omitted_rms(cell, level)`, each `None`
      where the view does not hold the cell. The cuts are crate-private, `synth::levels::LevelCuts`
      beside `LevelSet::of`: the relief from the first degree of the first octave the level does
      not evaluate, N(last) (N(L) where it evaluates none), through a degree-keyed entry
      (`relief::relief_variance_from`, `relief_structure_from`) under the wavelength-keyed forms,
      whose bits are unchanged (`height_golden`'s `closed_forms.golden` passes); the channels below
      the last evaluated level's mean cell width (each level is counted from its width to twice
      it); the small craters below the last evaluated octave's smallest diameter; and the coarse
      rims at `λ_n`. Each cut is at least `λ_n`, so σ is at least R09's RMS at `λ_n` (tested).
    - The lane's first cut for the relief, the wavelength 2πR ÷ (N(last) + ½), dropped degree
      N(last), the first the level leaves out (about 10⁻⁴ of the next octave); the degree-keyed
      entry is exact.
    - R09's wavelength-keyed forms stay as they are; their docs (`unresolved_variance`,
      `structure_function`, `unresolved_rms`, free and methods, `UnresolvedVariance` and the
      `levels` module) now say what λ means for each contribution and point at the level-keyed
      forms.
  - _The slope_ is the central difference of the level's heights over b along local east and north,
    between the feet of the points b ÷ 2 either way in the tangent plane at the ground point, not
    the point gradient: over a baseline long against `λ_n` the point gradient keeps slopes the
    baseline averages out.
    - Its short-baseline limit is the gradient's tangential part (R09's "Deviations in T4, as
      built"), per metre at the ground along each principal direction (tested to 10⁻⁴ at 1 cm).
    - v = D(b) ÷ b² of what the level leaves out. The reading quotes atan √(|g₀|² + 2v), with the
      Rice standard deviation of |g₀ + δ| taken through the arctangent's derivative there.
    - The Rice spread is Kummer's series of `L_½`(−x) below x = 40 and its asymptotic series above
      (Abramowitz and Stegun 13.1.27 and 13.5.1). It is tested against Rayleigh's exactly, the
      asymptote, and a sample of 2 × 10⁵ slopes at ν ÷ σ from 0 to 12, within 1%.
  - _The resolution read._ As ruled, `SurveyedReading::resolution()` is the cell code's
    resolution the reading was taken at (`ResolutionCode::resolution()`), so that T12's
    `RESOLUTION` and its ± come from one read (tested).
  - _The line of sight_ (accepted as ruled). `slant_range` returns `LineOfSight {
Ground(SlantRange), NoGround, NotSurveyed }` in place of `Option`, so that T12 can tell
    `NO GROUND` from `NOT SURVEYED`. `SlantRange` has getters, and gains `is_resolved()` and `at()`,
    the hit's foot as `FaceUv`, for the reading under it (the geocentric direction of the hit is
    T3's hazard). The hit is `hit_m()` and the origin `origin_m`, their unit in their names;
    `is_grazing()` is `sigma()` being `None`, so the two cannot disagree.
    - `NoGround` also stands where the line has gone the reach `max` it was given. signoff-9's
      `NO GROUND` is the line passing the body alone, so T12 gives a reach past the body's far
      side.
    - A `SurveyView` that codes a cell its `FieldView` does not hold has its readings refused as
      `QueryHeightError::NotSurveyed`, not a panic.
    - Above a ceiling it takes Newton's step to it, which never overshoots on a convex datum, and
      stops once the line rises. The ceiling is the highest cell elevation the view holds, plus the
      header's greatest relief, plus the most any level's contributions add (`LevelBounds`'
      `contribution_range_m`), plus 1 m. A view holding no cell has no ceiling, so every line of
      sight is `NotSurveyed`.
    - Below it, over a cell not held, the line is `NotSurveyed`. Over a margin cell it is
      `NotSurveyed` where it meets the margin's band-limit ground or that ground cannot be
      synthesised.
    - Over surveyed ground it steps by the clearance ÷ 3 (safe for slopes to 70.5°), at least
      `λ_n` ÷ 4 and at most half a coarse cell, and bisects the first step under the ground to
      1 µm. From under the ground the range is zero.
    - σ_R = σ_h cos θ_s ÷ sin γ_s, from R05's `surface_normal` at the hit. On an analytic plane it
      is tested against the geometry to 10⁻⁹, and against the range's own change with the
      ground's height to 10⁻⁴. `GRAZING_FACTOR` (2, public) is the threshold sin γ_s < 2√v at
      b = `λ_n`, never on a resolved range: Design note 7's "survey's cell size" is taken as the
      reading level's shortest wavelength, within one 9% code step of the survey's resolution.
  - _Names._ The readout's (`ELEVATION`, `SLOPE … · BASELINE 2 m`, `SLANT RANGE`, `GRAZING`,
    `NOT SURVEYED`, `NO GROUND`) are T13's as signed off (signoff-9), in the doc comments.
  - _The pooled case_ is held on `synthetic_worlds()`'s four bodies through a client's
    `PartialField`: 200 cells × 100 points a world, at the final tree (556 s in the slow-test
    profile, one thread, load average 3.1, 8.5 and 11.8 at its start; provisional). The run before
    the relief's one-degree fix gave the same figures to the third decimal.

    | World      | Survey | RMS ÷ σ | var z  |
    | ---------- | ------ | ------- | ------ |
    | Earth-like | 100 m  | 0.9835  | 0.9409 |
    | Earth-like | 1 km   | 0.9870  | 0.9736 |
    | Mars-like  | 100 m  | 1.0262  | 1.0293 |
    | Mars-like  | 1 km   | 1.0655  | 1.1071 |
    | Moon-like  | 100 m  | 1.4300  | 2.4732 |
    | Moon-like  | 1 km   | 1.0808  | 1.1672 |
    | Ceres-like | 100 m  | 1.1107  | 1.2334 |
    | Ceres-like | 1 km   | 1.1280  | 1.2692 |

    The Earth- and Mars-like worlds hold the plan's 10% and 15%. The Moon- and Ceres-like worlds,
    whose craters dominate, are held to the known gap as ruled (Finding B, below): RMS ÷ σ in
    [0.90, 1.6] and var z in [0.85, 3.0], at the plan's tolerance from below, so that a σ quoted
    too high still fails. The bounds are in one place (`CRATER_GAP`, `GAP_RMS`, `GAP_Z`), and a
    commit under T5's ID removes them after R09.T7.h, so that every world is held at the plan's.

  - _After R09.T7.h_ (2026-10-14; lane r09-crater-sharpening; main's ruling of that day). With the
    kept craters' sharpening in R09's closed forms, the same case gives, by name in 577 s:

    | World      | Survey | RMS ÷ σ | var z  |
    | ---------- | ------ | ------- | ------ |
    | Earth-like | 100 m  | 0.9831  | 0.9318 |
    | Earth-like | 1 km   | 0.9869  | 0.9697 |
    | Mars-like  | 100 m  | 0.9941  | 0.9620 |
    | Mars-like  | 1 km   | 1.0244  | 1.0230 |
    | Moon-like  | 100 m  | 0.9883  | 0.9748 |
    | Moon-like  | 1 km   | 0.9828  | 0.9653 |
    | Ceres-like | 100 m  | 0.9942  | 0.9882 |
    | Ceres-like | 1 km   | 1.0858  | 1.1762 |

    Every case is held at the plan's 10% in RMS ÷ σ, and all but one at its 15% in var z. The
    exception, as main ruled, is the Ceres-like world at the 1 km survey alone: its var z is held in
    [0.85, 1.25], 1.176 measured with headroom for a change of seed, at the plan's tolerance from
    below. `CRATER_GAP` is now that one case (a world and a survey's index) and `GAP_Z` its bound;
    `GAP_RMS` is gone. The cause is the ruling's heavy-tailed crater sampling: an octave's E[g²] is
    carried by its youngest few tenths of a per cent of craters, so 200 × 100 points sample a few
    dozen. The closed form agrees with R09.T7.h's 10⁴-point measurement within sampling there (kept
    sharpening 15.0 ± 3.5 against 15.4 m², octaves left out 116 ± 26 against 108 m²). Deferred,
    below.

  - _Acceptance_, as built:
    - `cargo test -p hyperion-surface --lib reading` (21 tests, and the `#[ignore]` measurement
      `reading::tests::the_ground_a_level_leaves_out_by_contribution`);
    - `cargo test -p hyperion-surface --test panics reading` (9);
    - `cargo test -p hyperion-surface --lib height_golden` and `synth`, unchanged by the
      degree-keyed entry;
    - the pooled case under `just test-slow -E 'binary(surface_reading_worlds)'`;
    - the wasm targets through `just test-wasm-fast`.
  - _Tests_ (`reading::tests`):
    - the band-limit survey's elevation is `ground_at`'s bit for bit, with no uncertainty, at 2 m
      and 0.5 m; a coarser survey's level, σ and resolution;
    - the level-keyed forms by contribution against their decomposition (each relief octave the
      level does not evaluate, whole, as (a `g_m`^b)² times its band's variance; the channel levels
      after the last; each crater octave the level does not keep, whole; the rims at `λ_n`), zero
      from the band limit's level, and the whole variance at level 0, which evaluates none; and
      non-increasing as the level deepens, the RMS their root, and the structure function
      saturating at twice the variance;
    - margin and unheld cells have no reading through a client's field, and a client's field reads
      as the whole field;
    - the slope's short-baseline limit; the unresolved slope's long and short limits on every body,
      and per band on one J₀ band; the RMS slope never below |g₀|;
    - readings independent of the cache and of their order;
    - the Rice spread's limits, its two series' agreement and a sample;
    - the slant range to an analytic plane, exact to 2 µm; its σ's linearisation; the grazing
      threshold from both sides, and never on a resolved range;
    - lines of sight that leave the survey, meet margin ground, come down from far above unheld
      ground, look away, fall short, or start under the ground;
    - a line of sight on every synthetic world meets the ground where the surface is, and nothing
      before it;
    - a small pooled sample on the Mars-like world (24 cells × 24 points, at 25% and 40%);
    - a line of sight through a client's field ranges as over the whole field, is `NotSurveyed` once
      it rises out of the survey, and `NotSurveyed` for a client holding no cell; the level-keyed
      forms answer `None` for a cell the client does not hold;
    - nine `should_panic` tests in `tests/panics.rs`'s `reading`, four of them the level-keyed
      forms' (a cell of another level, and a lag that is not finite).
  - _Generator version._ Untouched (21): the readings are presentation, and no golden pins them.
- **Ruled for T5** (`decision-r10-t5-closed-forms.md`, 2026-10-13).
  - _Finding A, R09's level sets against its closed forms_ (above): real. The per-contribution
    sum is the contract, as R09's level-keyed forms; R09's wavelength-keyed forms stay, their docs
    corrected. Aligning the level sets with the forms would move every height golden, and stays
    octave-quantised.
  - _Finding B, the small craters' sharpening_: real. R09's small craters' `unresolved_variance`
    and `structure_function` leave out the sharpening of the craters a level keeps, from its band
    (½ `λ_n`) to the band limit's, which the coarse rims' closed form includes. On the Moon-like
    world at 100 m it is 0.79 m² of realised mean square, against 0.014 m² for the octaves the
    level leaves out (`the_ground_a_level_leaves_out_by_contribution`), so the quoted σ is low
    there, the unsafe side. It goes in R09's closed forms as R09.T7.h (Campbell's theorem on the
    difference profile), which re-blesses `closed_forms.golden` at 21 with no bump; T5 lands now
    with the known gap above.
  - _The interface deviations_: both accepted, with `SurveyedReading::resolution()` added.
- **Deferred findings (2026-10-13 rule)** (R10.T5; recorded, not ruled).
  - _R05's normal on a spheroid._ `patch::normals::surface_normal` scales the tangential gradient
    by the section radius along it, exact only along a principal direction. Off one it is off by
    up to h ÷ ρ × (N − M) ÷ N, 7.5 × 10⁻⁴ of the slope on the Ceres-like world a few kilometres
    up. The reading's slope uses central differences and is exact.
  - _The ceiling's walk._ `slant_range` reads every cell of the field's level once a call (393,216
    at level 8) for its ceiling. A ceiling held with the body's field (T6.b's worker) would spare
    it.
  - _The reading quotes less than the survey resolved_ for the relief: a level whose `λ_n` codes
    no finer than the survey resolves the octaves only to 4/3–8/3 `λ_n`. It is the safe side; a
    level chosen by the cutoffs instead would quote about one level deeper.
  - _The craters' realisations_ are sparse against the per-cell closed forms, an ensemble's mean:
    on the cratered worlds the z-scores' variance runs above the RMS ratio's square.
  - _The march's Lipschitz factor_ is a choice: ground steeper than 70.5° and thinner than
    `λ_n` ÷ 4 can be stepped past at a grazing angle.
  - _The slope's σ_ is taken through the arctangent's derivative at the RMS slope, a first-order
    figure where both are steep and wide.
  - _Design note 7's citations_ (the science check). Its √2 between the adirectional and the
    profile slope is credited to Kreslavsky and Head 2000, which does not appear to state it; it is
    Shepard et al. 2001's, as Beyer, McEwen and Kirk 2003 (JGR 108, 8085, ¶25) cite it, and holds
    for RMS slopes (about 1.75 for medians). Its 100 m baseline is not in Golombek et al. 2014,
    which gives 2–5 m and about 300 m; Golombek et al. 2017 (Space Sci. Rev. 211, 5, §2.7) gives
    the landing radar's 84 m. `reading.rs` cites both as read; docs-batch-17 (`2955006e`,
    2026-10-14) corrected the design note's text to match (updated 2026-10-15). Shepard et al.
    2001 itself was not read.
  - _The level-keyed forms have no golden yet_ (the determinism review). The ruling gives their
    labels in `closed_forms.golden` to R09.T7.h, so nothing compares them across targets before
    it, though they are R09's pinned wavelength-keyed forms at other arguments. Lean: T7.h adds
    the labels at today's values in a first commit, so that its crater term then shows as changed
    values it explains.
  - _The crater terms' pooled checks are heavy-tailed_ (the ruling's). Realised crater mean
    squares on a few hundred points differ from the closed forms by 3–9 times, and from each other
    by as much: an octave's E[g²] is carried by its youngest few tenths of a per cent of craters,
    of which a 200 × 100 pooled check samples a few dozen. If T7.h's acceptance misses for that
    reason, main rules on a larger sample or a check weighted by crater count. It did, once: the
    Ceres-like world at 1 km, var z 1.176 (2026-10-14). Main ruled that case alone held in
    [0.85, 1.25] (`CRATER_GAP`, above). A check weighted by crater count, or a larger cratered
    sample, would retire it: for R10.T15's verification pass, not before.
  - _The whole octaves' J₀ structure function_ (the ruling's) places each octave at √(D₀ D₁). Its
    small-lag slope, σ² k² b² ÷ 2, is not the craters' slope variance, ρ E[g² ∫ |∇f|² dA], so
    slopes at 2 m from octaves left out may be off. T7.h leaves that form as it is.
  - _The rims' crater-wide share_ (R09.T8's deferred item, the ruling's figures): realised against
    closed per sample of cells, 0.457 against 0.0063 m² on the Mars-like world at 1 km, 0.072
    against 0.86 m² on the Moon-like world at 1 km, and 0.048 against 6.45 m² on the Ceres-like
    world at 1 km.
- **Deviations in T2, as built** (2026-10-14; lane r10-weights, on R10.T5 at `1a56ee17`, with
  R10.T1.b at `7e1537eb`, and merged with R09.T7.h at `742aa3e6` before its figures and goldens
  were taken).
  - _Files._ `crates/hyperion-surface/src/material/weights.rs` with `weights/tests.rs` and
    `weights/golden.rs`; `material/mod.rs`'s exports, `TRANSITION_HALF_WIDTH`'s doc and a
    compile-time check of the slope thresholds; `tests/panics.rs`'s `weights` module; eight goldens
    in `tests/golden/weights/`. T1.b's `material/classify.rs` is refactored, its answers unchanged
    (below). No `crates/hyperion-sim/tests/surface_weights_worlds.rs` yet: its `sample_worlds()`
    cases follow R09.T16 as a later commit under T2's ID (main's ruling), and the synthetic
    worlds' agreement lives in the library, as T1.b's and T3's goldens do.
  - _The signature._ `ClassWeights` holds `[Option<GroundMaterial>; 8]`, not Provides'
    `Option<Material>`: a slot is the form always and the entry where the palette names one, as
    T1.b's `material_at` answers, so a body whose palette names no substance still has its forms
    weighted (signoff-9 §17.5's `MATERIAL REGOLITH · —`), and `None` marks an empty slot. Getters:
    `palette`, `weights`, `iter`, `weight(material)`, `largest`, `len` and `is_empty`. Public
    constants: `MAX_CLASSES` (8), `WEIGHT_TOTAL` (255) and `SLOPE_LAG` (5 cm).
  - _A point's palette, not a patch's._ `ClassWeights` is one point's up to eight materials, in
    falling weight, ties in rising class code and then entry, so the first is the largest weight
    with ties broken by discriminant, and a material's slot differs between neighbouring vertices.
    The palette of up to eight a patch keeps, with each vertex's bytes over it, is T6.a's
    `TerrainLayers`, gathered from its vertices' materials; Provides' `TerrainLayers.palette` is
    now `[Option<GroundMaterial>; 8]` for the same reason as `ClassWeights`'. A patch needing more
    than eight is not handled at run time: the test names it. A material whose weight rounds to no
    byte is not held.
  - _The finest level_ (`finest_level`, or deeper): `material_at`'s class, blended linearly over
    `TRANSITION_HALF_WIDTH` either side of both slope thresholds, `CLIFF_SLOPE` and
    `SNOW_SHED_SLOPE` (their bands, 35–45° and 55–65°, do not meet, which `material/mod.rs` now
    checks at compile time), for the image only: at a slope θ within a band about θ₀ the steeper
    ground's material takes (θ − θ₀ + 5°) ÷ 10° of the weight. The rules on the height (the water
    surface, the snow line, the melt and the arid bound) are not blended: the plan names a width
    for the slopes alone. Design note 3's "ties broken by discriminant" orders the slots at
    coarser levels; at the finest level the authoritative class is kept first whatever its share,
    wins the largest remainder's ties, and then takes bytes from its largest rival until its byte
    is strictly the largest, so no reader's tie-break can choose another (an even blend at 40°
    gives it 128 to 127). `TRANSITION_HALF_WIDTH`'s doc names both thresholds; the snow's band is
    presentation alone (the science check).
  - _A coarser level n_ (Design note 3, as `weights_at`'s documentation gives it). The finest
    ground is the level's plus what the level leaves out:
    - the height, the level's plus a normal deviate of R09's level-keyed `omitted_variance`
      (R10.T5's, as ruled, with R09.T7.h's sharpening of the craters a level keeps; the two
      closed forms kept in `SynthCache`'s material part by cell and level, `classify::kept_forms`,
      since T7.h they cost 50 to 150 µs a call), asked at each band between the heights
      where a rule's condition
      changes (`Site::height_breaks`: the cell's water surface, and where the temperature by the
      lapse rate crosses the crust entry's transition, the snow entry's and Köppen–Geiger's arid
      bound), at the mean where it lies in the band, so that with no variance it is the level's
      own height (a test scans 24 km of heights on 30-odd cells a body and finds every change of
      material at a listed break);
    - the slope, the level's tangential gradient plus an isotropic normal deviate whose variance
      along each tangent axis is `omitted_structure_function` at `SLOPE_LAG` over the lag
      squared, the small-lag slope (tested against half the lag within 1%); the probability that
      the finest slope's tangent, the gradient scaled by ρ ÷ (ρ + h) as R05's `surface_normal`
      takes it, exceeds tan 40° and tan 60° is the Rice distribution's tail, Marcum's Q₁ (Rice
      1945, §3.10, eq. 3.10-11), integrated in closed form along one axis and by panels of
      eight-point Gauss–Legendre along the other (`exceedance`; tested within 10⁻⁶ against the
      Rayleigh tail and an independent polar sum, and pinned bit for bit in `kernels.golden`);
    - a partial ice share's place, the point's shadowed-ground index at level n against the cell's
      cut at the finest level, plus the index the level leaves out, taken from the cell's own 256
      grid points, the finest index less the level's at each (`classify::ice_share_at`, kept in
      `SynthCache`'s cuts beside the cut, by cell, level and ice code). As first built it was a
      normal deviate of the closed forms' variance through the cut's scales, which put too much of
      a small share's ice far from the cut, the index being heavy-tailed (T1.b's record): on the
      cold-trapped pole at 0.2, 0.27 of a level-4 texel's weight against 0.18 counted, and 0.34
      against 0.20 at level 2. The empirical form is the cell's share where the level resolves
      nothing below the cells, and the point's own answer where it leaves nothing out;
    - the three independent, each combination a case of the classifier's rules (`Site::facts`)
      weighted by its probability, the weights the cases' materials summed. Where the level leaves
      nothing out (from the band limit's level, which is coarser than the finest on the Ceres-like
      world) the weights are the class at the point, all of the weight (tested);
    - the cell's facts are the texel centre's, and a channel's bed is one of the channel levels
      level n evaluates, at the centre (deferred findings, below).
  - _At most eight a point._ The likeliest eight are kept and the rest of the weight spread over
    them in proportion, then made bytes summing to 255 by the largest remainder. No point of the
    goldens holds more than four materials, and no patch of the tests more than three.
  - _The texel._ A texel of band level n is one vertex spacing of the level's lattice about a
    vertex, a cell of quadtree level n + 6, which is what R05's bake reads at a patch's 65 × 65
    vertices. So R07's class map at quadtree level L (its "N = 2^L" texels a face) takes each
    texel's weights at band level L − 6, at the cell's centre, as T10.d's "`weights_at` at the
    cell's centre" has it once the level is so read; the plan's "level 4" for the class map is band
    level 4, a map at L = 10. Design note 8's "a few hundred kilobytes" is a map at L = 6, band
    level 0. This is the lane's reading, asked of "main" (below); the agreement at level 4 is over
    level 4's texels, 1/1,024 of a face.
  - _The classifier, split._ T1.b's `Facts::at` becomes `Point::at` over a cell's `Site` (the
    climate's reads, the regime, the basin, the entries and the sea-level temperatures) and
    `Site::facts(h, steepness, in_ice, channel_floor)`, with `Steepness` (loose, cliff, shed),
    `IcePlacement`, `finest_band`, `cos_slope`, `on_channel_floor`, `ice_share_at` and `kept_forms`, and
    `koppen_offset_mm` split out of `is_arid`, all crate-private. Every comparison is the one it
    was, so `material_at`'s goldens pass unchanged (the reviews traced each). The cell's
    neighbours are now read before the ice cut's grid and the channels, so in a view lacking
    several cells the first one named may differ; the error is the same kind.
  - _The agreement_, as the plan's test measures it: at levels n − 1 to n − 6 from the finest, 64
    points a texel (an 8 × 8 grid of its steps' centres), and at the class map's level 4, 4,096
    (64 × 64), against `material_at`; the measure is a texel's total variation, ½ Σ |weight ÷
    255 − fraction| over the materials of either, the share of the texel given to the wrong
    material, and every texel's bytes are checked to sum to 255. Half the texels are at random
    points of the cells the weights find hardest (belts on dry land without ice, partial ice, sea
    under 300 m deep, the channels' trunks, the coarse craters' rims) and half at random over the
    sphere. A failure names the level's five worst texels and their cells. The fast tests take 24
    texels a level and 2 at level 4 a world; the slow ones
    (`material_weights_agree_with_the_classes_counted_over_a_thousand_texels_*`,
    `#[ignore = "slow: …"]`, by name) a thousand of each. The slow figures (slow-test profile, 3
    threads, load 5–12, on R09.T7.h's closed forms), mean total variation over a level's thousand
    texels, the worst
    texel's, and the mean over its mixed texels, those with more than one material in the weights
    or the points, with their count:

    | World      | Level      | Mean   | Worst  | Mixed: mean (count) |
    | ---------- | ---------- | ------ | ------ | ------------------- |
    | Earth-like | 18 (n − 1) | 0.0022 | 0.4688 | 0.1875 (12)         |
    | Earth-like | 17         | 0.0070 | 0.9294 | 0.1617 (43)         |
    | Earth-like | 16         | 0.0078 | 0.7647 | 0.1214 (64)         |
    | Earth-like | 15         | 0.0118 | 0.7453 | 0.1293 (91)         |
    | Earth-like | 14         | 0.0145 | 0.5347 | 0.1284 (113)        |
    | Earth-like | 13 (n − 6) | 0.0108 | 0.5104 | 0.0907 (119)        |
    | Earth-like | 4          | 0.0351 | 0.7871 | 0.0700 (501)        |
    | Moon-like  | 16 (n − 1) | 0.0002 | 0.2188 | 0.2188 (1)          |
    | Moon-like  | 15         | 0.0014 | 0.8000 | 0.2831 (5)          |
    | Moon-like  | 14         | 0.0023 | 0.9451 | 0.1757 (13)         |
    | Moon-like  | 13         | 0.0023 | 0.7969 | 0.2252 (10)         |
    | Moon-like  | 12         | 0.0016 | 0.3928 | 0.0893 (18)         |
    | Moon-like  | 11 (n − 6) | 0.0008 | 0.2344 | 0.0363 (23)         |
    | Moon-like  | 4          | 0.0062 | 0.6914 | 0.0412 (151)        |
    | Ceres-like | 15 (n − 1) | 0      | 0      | (0)                 |
    | Ceres-like | 14, 13     | 0      | 0      | (0)                 |
    | Ceres-like | 12         | 0.0003 | 0.2118 | 0.0865 (3)          |
    | Ceres-like | 11         | 0.0000 | 0.0039 | 0.0039 (1)          |
    | Ceres-like | 10 (n − 6) | 0      | 0      | (0)                 |
    | Ceres-like | 4          | 0.0000 | 0.0100 | 0.0014 (12)         |
    | Mars-like  | 17 to 12   | 0      | 0      | (0)                 |
    | Mars-like  | 4          | 0.0034 | 0.0090 | 0.0038 (14)         |

    The Earth-, Moon- and Ceres-like cases passed in about 40 minutes, three at once; before
    R09.T7.h the Earth-like figures were the same and the Moon- and Ceres-like ones within a few
    ten-thousandths in the mean. The Mars-like case, in the first run, did not finish in its 3 h
    timeout: its finest points cost about
    2 ms or more each away from a cache, of which its class-map level has 4.1 × 10⁶ (not profiled;
    the screened small craters' draws, about 80 µs a crater by R09's "Deviations in T7.b", are
    the likely cost). Its row is the dev profile's measure of 200 texels a level and 16 at level
    4, chosen by the same rule, before R09.T7.h; the thousand-texel run waits on main's ruling on
    its cost
    (deferred findings, "The slow cases' cost"), the world being dust but for its trunks' beds.

    The cold-trapped pole (T1.b's field, partial water ice poleward of 85°, the synthetic worlds
    having none on land), 16 texels a level of 64 points in its polar cells, and 24 of 256 points
    at level 4, nearly all mixed, held to the plan's ten points in the total variation at
    n − 1 to n − 6 and, at every level, in the ice's mean weight against its mean counted share
    (at level 4, where nothing below the cells is resolved, a texel's own count scatters about the
    cell's share). At a share of 0.2: total variation 0.020, 0.042 and 0.041 at levels 16, 14 and
    11, and the ice's weight 0.31, 0.21, 0.24 and 0.23 against 0.29, 0.24, 0.23 and 0.25 counted
    there and at level 4 (whose total variation is 0.11). At 0.6: 0.048, 0.087 and 0.044, and
    0.69, 0.58, 0.50 and 0.69 against 0.74, 0.57, 0.53 and 0.66 (level 4's 0.13).

  - _Acceptance_, as built:
    - `cargo test -p hyperion-surface --lib material::weights` (the goldens alone:
      `--lib material::weights::golden`);
    - `cargo test -p hyperion-surface --features testing --test panics weights` (1);
    - the four slow agreement cases by name, `cargo test -p hyperion-surface --features testing
--profile slow-test --lib material::weights::tests::material_weights_agree_with_the_classes_counted_over_a_thousand_texels
-- --ignored`;
    - the wasm targets under `just test-wasm-fast`;
    - the plan's `just test-slow -E 'binary(surface_weights_worlds)'` waits for the
      `sample_worlds()` binary. The four slow cases also run in the slow pass's native and
      wasip1 holds (`--run-ignored only`), at the times above natively.
  - _Tests_ (`material::weights::tests`, `material::weights::golden`, `tests/panics.rs`'s
    `weights`):
    - the exceedance: the Rayleigh tail at a zero mean, an independent polar sum over 70 cases
      (means 0.05–2, variances 10⁻⁴–1, both thresholds), monotone in the threshold and the mean;
    - the blend's shares at nine slopes; the authoritative class's largest byte, ties included,
      and against a larger rival; eight kept of eleven, summing to 255, a share of no byte dropped;
      the height cases summing to one, each strictly inside a band of its own;
    - every change of a material with the height at a listed break; the steepness cases summing to
      one and tending to the level's own steepness;
    - at the finest level and one deeper, on every synthetic world at 400 points (two in each of up
      to 50 hard cells, then random), the largest weight is `material_at`'s class, strictly, and
      the weights sum to 255;
    - from the band limit's level, the weights are the class;
    - order and cache independence (`assert_order_independent`, a shared and a fresh cache), and
      on the cold-trapped pole with `material_at` interleaved, which keeps the same cut;
    - a cell not held is not surveyed, at three levels;
    - the slope lag is short enough on every body;
    - the agreement, fast and slow, on the four bodies and on the cold-trapped pole;
    - a patch's 65 × 65 vertices need no more than eight materials, at four levels a body;
    - the goldens and their header; one `should_panic` (a (u, v) that is not finite).
  - _The goldens_ (main's ruling: header 21, a subdirectory of their own), eight:
    - `weights/<world>.golden`, one a synthetic world: 250 points (up to 30 each on dry-land
      belts, in partial ice, in the shallowest sea and under coarse craters' rims, then random),
      each at the finest level, n − 1, n − 3, n − 6, n − 10 and level 4, each slot its class code,
      entry and byte;
    - `weights/cold_trap.golden`: 40 points of the cold-trapped pole at 0.2, at the same levels,
      the only golden whose weights read the ice's cut;
    - `weights/kernels.golden`: `exceedance` over 216 cases (the reach's early return and the
      panels' bound among them), the normal distribution, the height cases and the blend, bit for
      bit, since the bytes show a float's drift only where it crosses a byte.

    The weights are presentation, but the client's WebAssembly workers bake them, so the goldens
    hold the targets to one blend. A `native_only` test holds `weights/` to `GENERATOR_VERSION`.
    A change to the weights alone is a client change (Generator version, above): it re-blesses
    `weights/` at the version in force with no bump, the commit saying so, an exception
    `golden_diff.py` does not know (asked of "main", below). Any later change to R09's closed
    forms or the classifier before P14.T48.e re-blesses `weights/` too; R09.T7.h landed before
    them, so they are taken with its sharpening.

  - _The cost_ (provisional: the dev profile at `opt-level = 2`, one thread, the Earth-like world,
    a level's patch's vertices in order from one cache, load about 5, with R09.T7.h and the forms
    kept by cell and level): `weights_at` 26 µs a vertex at the finest level, 21–22 µs at n − 1
    to level 12 and 13 µs at level 4, against the level's warm `height_at` 20, 15 to 9 and
    0.5 µs. A partial-ice cell adds its grid's 512 heights once a level. Most of the
    rest is the per-cell site, asked afresh at each vertex (deferred findings). Without the kept
    forms each coarse vertex would pay R09.T7.h's 50–150 µs twice. T6.a's bench states it with
    and without the weights.
  - _Generator version._ Untouched (21): the weights are presentation, and the classifier's
    answers do not move.
  - _Closed sets, for the composition audit._ None new: the weights read the classifier's rules
    and the palette's entries as T1.b built them.
- **Questions for "main" (R10.T2)** (each with the lane's lean, built to it).
  - _What a class-map texel is._ Lean: as above, a texel of band level n is a cell of quadtree
    level n + 6, so T10.d calls `weights_at` at a map cell's centre at band level L − 6, and
    Design note 8's "level 4 or shallower" bounds the map at L = 10. The alternative, a texel at
    level 4 being the quadtree cell (f, 4, i, j), needs a footprint argument or T10.d averaging a
    cell's vertices, since one centre sample cannot stand for a 600 km cell.
  - _The weights goldens and `golden_diff.py`._ Lean: record the exception beside `wire/`'s in
    the sim-determinism skill (a weights-only change re-blesses `tests/golden/weights/` at the
    version in force with no bump), and add `weights/` to the re-bless list of any later change to
    R09's closed forms or the classifier before P14.T48.e.
- **Deferred findings (2026-10-13 rule)** (R10.T2; recorded, not ruled; each leaves the algorithm
  and the contracts as they are).
  - _The synthetic worlds barely mix._ Away from the Earth-like world's belts, snow lines and
    coasts every texel is one material: the Mars-like world is dust but for its trunks' beds, the
    Moon- and Ceres-like worlds regolith but for a few bedrock points under coarse rims, and none
    has a partial ice share on land (the cold-trapped pole stands in for it). So the agreement
    there is near zero by construction, and `sample_worlds()` (R09.T16), whose coarse pass places
    ice, frost and provinces, is the real test.
  - _A mixed texel's own error_ is up to 28% of it in the mean, 7–19% on the Earth-like world
    (the table's last figures): the weights are the expectation given the level's ground at the
    texel's centre, while the omitted ground's longer wavelengths, up to about four of the texel's
    widths, are coherent over it, so a texel's realised fraction scatters about that expectation.
    The mean over all texels is within the plan's ten points at every level.
  - _The slow cases' cost._ The Mars-like case ran past its 3 h timeout natively (above), and the
    slow pass runs the four natively and on wasip1 (`--run-ignored only`), where it would be
    slower still. Lean, for "main": hold the class-map level's thousand texels on the Earth-,
    Moon- and Ceres-like worlds and take a hundred on the Mars-like one, or split the case out of
    the pass's holds; the plan's count stands until main rules.
  - _The texel's own spread._ The level's ground is taken as its centre's over the whole
    footprint. Adding |∇h|² s² ÷ 12 (a uniform square of side s) to the height's variance, and a
    curvature term to the slope's, would carry the footprint's own spread across a threshold, most
    of a straddling texel's error at n − 1, where the level leaves almost nothing out.
  - _A ridged octave's local slope._ R09 weights each ridged octave by the ridge of the octave
    above it, the last the level keeps, so the first omitted octave's local slope variance swings
    from 0 to about 1.9 times its cell mean with the kept octave at the point; the weights use the
    cell mean (the science check). Another face of "Coarse weights assume Gaussian slopes".
  - _The ice index reads the height and the slope_, which the weights take as independent of it:
    in a partial cell the ice lies in hollows, where the height is low, so the snow line's and the
    ice's cases are correlated. A joint distribution of the height and the index over the cell's
    grid would carry it.
  - _Channel beds of finer channel levels_ are not counted at a coarse level: its texels are wider
    than those beds, and a texel's centre on one would weigh the whole texel. Their expected share
    is the levels' Σ ρ_k × 2h_k (groove length a unit area times bankfull width), which R09's
    channel closed form already computes per level (`incision::level_terms`' density and half
    width, crate-private in R09's file); an accessor there would let T2 add it as a probability.
  - _A class-map texel takes its centre's cell._ At level 4 a texel is a quarter of an Earth's
    coarse cell across, so one straddling a coast, a crust's edge or a climate's step takes one
    cell's facts. A texel's corners' cells, weighted by area, would carry it.
  - _The per-cell site is asked at every vertex._ A bake's 4,225 vertices lie in a few cells; the
    closed forms are kept by cell and level, but the site (the climate's months, the basin's
    neighbours, the breaks) is built afresh at each, which a memo beside the forms would spare.
    R09.T7.h's own lever, tabulating the kept octaves' sharpening by band level, would make the
    forms' first call in a cell cheap too.
  - _The image changes blend at the band limit's level._ Where a level coarser than the finest
    leaves nothing out (the Ceres-like world's level 15), its weights are the unblended class,
    while the finest level blends 35–45° and 55–65°, so those slopes sharpen as the level of
    detail coarsens by one. For T10's by-hand check.
  - _Gaussian slopes_ (the plan's Risks, "Coarse weights assume Gaussian slopes"): the agreement
    holds on the synthetic worlds' belts; ridged and cratered ground on `sample_worlds()` is the
    remaining check.
- **Deviations in T6.a, as built** (2026-10-14; lane r10-bake, begun on R10.T2 at `e20efe8e` and
  fast-forwarded to `0dc9a640`, after P14.T48.e's bump to 22, before its goldens and the gate).
  - _Files._ `crates/hyperion-surface/src/bake/{mod,layers,tests,golden}.rs` and `src/scatter.rs`,
    with `lib.rs`'s `pub mod bake`, `pub mod scatter` and summary line; two goldens in
    `tests/golden/terrain_bake/`; `benches/bake.rs` and its `[[bench]]` in the crate's manifest
    (`required-features = ["testing"]`); `tests/panics.rs`'s `bake` module. Beside them,
    additively:
    - R05's `patch.rs`: `HeightSource::patch_heights` (below), the crate-private
      `bake_with_heights`, `patch_heights_of` and `vertices_of`, and `bake_vertices` now for the
      tests alone; and `patch/normals.rs`, whose normals read the vertices' samples.
    - T3's `ground.rs`: the synthesiser's `prepare_cache` and `patch_heights`.
    - R09's `synth/bake.rs`: the crate-private `PatchHeights::from_queries`, `VertexQuery`,
      `heights` and `cover_patch`. `Synthesiser::bake_patch` is their sum, its values and order
      unchanged (`height_golden`'s `bakes` passes).
    - T2's `material/weights.rs`: the crate-private `weights_with_sample` and `kept_omitted_rms`,
      and `ClassWeights::from_slots` for the tests. `material/classify.rs` gains
      `Point::with_sample`. `weights_at` now makes its height first and hands it on, but deeper
      than the finest level asks only the finest's, as before; its answers are unchanged (T2's
      goldens pass).
  - _The signature._ `bake::bake_terrain(synth, cache, key, opts, hooks)` returns
    `Result<TerrainBake, QueryHeightError>`, over any `F: SurveyView` (T5's trait, a client's
    `PartialField`).
    - `TerrainBake` holds R05's `PatchBake` and `TerrainLayers`, with `patch()`, `layers()` and
      `into_parts()`.
    - `TerrainBakeOptions` is R05's `BakeOptions` and `WeightsLayer::{Baked, Skipped}`. `Skipped`
      leaves the palette empty and makes no weights: it serves T11's depth-only wireframe and the
      bench's "without".
    - `hooks: &[&dyn BakeHook]` is a slice, each hook appending to `extra` in turn, since the Rust
      rules bar a bare `Option` argument. `BakeHook` is Provides' to the letter.
    - `TerrainLayers` has private fields with getters: `palette`, `weights`, `survey_mask`,
      `horizon`, `bound`, `sigma`, `extra`, and `is_complete` (below). The mask's values are the
      public `SURVEYED` (255) and `UNSURVEYED` (0).
  - _R05's bake through the height source._ R05's `HeightSource` gains
    `patch_heights(cache, key) -> PatchHeights`, R09's type. Its default asks `height` at each
    vertex in R09's order, the patch's level first. T3's implementation for `Synthesiser` overrides
    it with R09's bake less its cover, and `prepare_cache` now covers the lattice, as T3's record
    asked. R05's `bake_patch` asks for it once after `prepare_cache`, and panics on another
    patch's heights (a source's bug; the trait being open, an override could hand any).
    - The normals read the vertices' own samples: all of them for mesh normals, and the even
      samples of `NormalScale::Double`, which are the vertices bit for bit (`sample_dir`'s
      contract). A mesh-normal bake asks its source 5,314 times where it asked 9,540.
    - R05's test planet takes the default, and its `bake.golden`, `collision.golden` and every R05
      test are unchanged. R05's `bake_patch` over the whole field, through T3's source, equals
      `bake_terrain` over a client's field surveyed there, byte for byte (tested).
  - _The survey and the reference surface_ (Design note 12, a choice the plan leaves open).
    - A vertex's mask is `SURVEYED` exactly where `SurveyView::resolution` of the field's level
      cell holding it (`PatchKey::containing`, the synthesis's own cell rule) is a surveyed code.
    - A vertex outside the survey, a margin cell's or a cell the view does not hold, is discarded by
      the shader. The triangles from it to surveyed vertices are drawn up to the mask's half, so it
      carries the ground wherever the view can make it: its height, gradient, morph target and
      weights, the margin's cells being the server's to the bit. A vertex in the survey's first
      margin ring nearly always can.
    - Where the view cannot make a query, a vertex outside the survey takes the reference surface:
      the datum, height 0 with a zero gradient, so the datum's normal, and no weights. The bake is
      then not `is_complete()`.
    - Each query takes it on its own. So such a vertex can be partly answered: its own height
      real and its morph target the datum, or the reverse, or both heights real and the weights
      refused. Its morph target is then what the parent patch bakes at that vertex, real ground or
      the datum, so a child at morph one lies on its parent's mesh across the survey's edge too, as
      R05's morph requires (tested). Making both the datum where either is refused would break
      that.
    - The alternative, the reference surface at every unsurveyed vertex, would draw the visible
      half of each edge quad sloping halfway down to the datum in the terrain's appearance: the
      "cliff in the ground" Design note 12's skirt is there to avoid. T10.c, which draws that skirt,
      confirms the choice (lean, below).
    - A surveyed vertex the view cannot make is a `NotSurveyed` error: its view lacks the margin
      a survey brings.
  - _Design note 16, as built._ A block of new cells changes no value at a vertex surveyed before
    it (its heights, normals and weights), nor at a vertex outside the survey that was real ground,
    while the patch's palette stays within eight (tested). A patch whose survey mask it leaves as it
    was, and which was complete, bakes the same bytes. An incomplete patch can change while its mask
    does not, once a block brings a cell its reference vertices' reads reach. So T6.b rebakes a
    patch on a block that changes its mask, or on any block while it is incomplete: Design note
    16's rebake rule, widened (the question below). A worker's cache filled before the block bakes
    as a fresh one after it (tested), by `SynthCache`'s rule that a cache serves views whose held
    cells only grow, which `bake_terrain`'s documentation repeats.
  - _The palette_ is the union of the vertices' materials, in rising class code and then entry
    (`GroundMaterial`'s order). It includes those of answered vertices outside the survey, whose
    weights the edge quads' visible halves interpolate.
    - T2's record left a patch needing more than eight "not handled at run time". It is handled now,
      deterministically: the eight of largest summed bytes over the patch are kept, ties to the
      lower material.
    - A vertex holding a dropped material has the bytes of those kept made whole again in integers,
      each share floored and the rest given by the largest remainder, ties to the lower slot. A
      vertex all of whose materials are dropped takes the patch's heaviest material whole, so every
      vertex with weights sums to 255 and only one without (outside the survey) has none.
    - No patch of the tests passes eight. T2's test still names any that would. A block that
      pushes a patch's union past eight re-makes the bytes at its surveyed vertices too.
  - _The weights from the bake's samples._ Each vertex's weights are `weights_with_sample` at its
    direction, given the height the bake made there. That is `weights_at`'s answer at the vertex's
    (face, u, v) bit for bit (tested), without its second height. The finest level hands the sample
    to `Point::with_sample`.
  - _The bound and σ._ The bound is R09's `level_bound_m` at the patch's level: the body-wide
    level bound, the per-patch bound of Design note 5 not being adopted. σ is the largest of R09's
    `omitted_rms` at the level over the cells of the patch's surveyed vertices, 0 where none is
    surveyed. It comes from the closed forms T2's weights keep by cell and level, and its root
    is `omitted_rms` bit for bit (tested). T4.b's body-wide σ is its own.
  - _The horizon map's slot._ `HorizonMap` is declared in `bake/layers.rs`: near and far words,
    both empty (`is_empty`, `near_words`, `far_words`), as main ruled. T8.a fills it, and may move
    it to its `bake/horizon.rs`. `encode_horizon`, `decode_horizon` and `far_field_point` are
    T8.a's.
  - _`scatter::ExtraLayer`_ is an empty `#[non_exhaustive]` enum in `src/scatter.rs`. It derives
    only `Debug`, `Clone` and `PartialEq`, which R11's rock list of `f64` positions can keep.
  - _The goldens_ (the determinism review's ask; the task names none), two, at header 22, in
    `tests/golden/terrain_bake/`, a subdirectory of their own, written in the library under
    `cfg(test)` (`bake::golden`) as T3's and T2's are, and compared on `wasm32-wasip1` and
    `wasm32-unknown-unknown`:
    - `moon_like.golden`: the edge patch over the 2 × 2 survey (incomplete), a patch inside it at
      the high setting, and a patch on a coarse rim surveyed whole;
    - `earth_like.golden`: patches at L + 3 on a coarse rim and at the finest level, surveyed
      whole.

    Each pins its arrays' digests (heights, offsets, normals, weights, mask), its scalars' bits and
    its palette. The heights, gradients and weights under them are R09's and T2's goldens'; these
    pin what the bake makes of them on every target, the mask's cell-edge vertices and the
    reference surface among it. The bake is presentation, so a change to it alone re-blesses
    `terrain_bake/` at the version in force with no bump, as `weights/` is (T2's record), and a
    change to R09's heights or closed forms or to the classifier re-blesses it with theirs. A
    `native_only` test holds the directory to `GENERATOR_VERSION`. Generator version untouched
    (22).

  - _Acceptance_, as built:
    - `cargo test -p hyperion-surface bake`: 14 tests of `bake::tests` and 2 of `bake::golden`,
      and R05's and R09's bake tests the filter also selects, 22 in all (6 s natively), and
      `tests/panics.rs`'s new `bake` module (2);
    - the wasm targets through smart-ci's wasip1 and browser suites (`just test-wasm-fast` in
      `ci`);
    - the bench, `bake/patch_65`, run as
      `just _locked cargo bench -p hyperion-surface --features testing --bench bake -- bake/patch`
      after its build outside the lock. `just bench -- bake/patch` measures the same (the
      workspace's features turn `testing` on), but builds every bench of the workspace first.
  - _Tests_ (`bake::tests`, `bake::golden`, `tests/panics.rs`'s `bake`):
    - on every synthetic world, at the coarse level less one, plus two and the finest, a cube
      corner's cell among them: every vertex surveyed and the bake complete; the bake byte for byte
      R05's over the whole field through T3's source; each vertex's own and even morph heights R09's
      `height_at` narrowed to `f32`;
    - the survey mask, on a level-2 patch over a 2 × 2 survey of the Moon-like world: `SURVEYED`
      exactly on surveyed cells' vertices, against `is_surveyed` and, off the cells' edges, the
      lattice's integers. Of its 4,225 vertices 64 are surveyed, 2,240 in the margin (1,232 of them
      answered, the true ground to the bit) and 1,921 not held (the reference surface, no weights).
      The bake is incomplete;
    - a resolution code alone changes no byte: four codings, 2 m, 50 m, 1 km and a mix, on an edge
      patch with mesh normals and an inside patch with the high setting's;
    - a block of the 2 × 2 cells beside the survey changes nothing at a vertex surveyed before,
      nor at an unsurveyed vertex that was real ground; the inside patch, complete, bakes the same
      bytes; and a cache kept across the block bakes as a fresh one, at both settings;
    - order and cache independence (`assert_order_independent`, and each against a fresh cache)
      over a patch on a coarse rim, two neighbours, its parent and two children; and over the
      survey's edge, among them three incomplete bakes;
    - a child at morph one lies on its parent across the survey's edge: its morph targets at even
      vertices are its parent's heights, bit for bit;
    - each vertex's weights are `weights_at`'s at its (face, u, v), the palette their union in
      order, and the bytes sum to 255, on the Earth- and Moon-like worlds at three levels;
    - the bound and σ, with and without the weights, on edge, inside and unheld patches;
    - the weights skipped leave the rest as baked;
    - each hook is handed the key and the heights in turn, and hooks change no byte;
    - a patch wholly outside the survey, on a held and an empty view, is the reference surface
      with the datum's normals at both resolutions;
    - the palette's union and order, past eight, its ties, the largest remainder and the heaviest
      material for a vertex of dropped ones;
    - the goldens, and their header;
    - two `should_panic`: a negative skirt margin, refused before any height is asked, and a
      source whose patch heights are another patch's.
  - _The bench_ (on the development machine, a Ryzen 7 3700X; provisional, as main ruled, until
    R09's fused interpolant lands: the release profile, one thread, under the heavy lock with
    `HEAVY_CPU_QUOTA=1200%`, schedutil; load 3.3, 10.1 and 10.6 at the first run's start, and 9.4,
    8.1 and 9.4 at the breakdown's), each patch warm from a first bake unless named cold:

    | Patch                     | Weights  | No weights | Cold, weights | High setting | Heights, client | Heights, whole field |
    | ------------------------- | -------- | ---------- | ------------- | ------------ | --------------- | -------------------- |
    | Earth-like, L + 4 (12)    | 161.8 ms | 139.2 ms   | 165.7 ms      | 476.4 ms     | 136.0 ms        | 104.1 ms             |
    | Earth-like, 19            | 168.1 ms | 158.4 ms   |               |              | 157.6 ms        | 125.3 ms             |
    | Moon-like rim, L + 4 (10) | 60.2 ms  | 40.9 ms    |               |              | 40.0 ms         | 36.6 ms              |
    - The 40 ms is the UHD 620 laptop's budget, on two of its cores. The desktop's figure is over
      it on every patch, with the weights and without, so by the 2026-09-30 rule (Risks, "The
      hardware decisions") the desktop fails it. It stands provisional, as main ruled, until R09's
      fused interpolant lands. R09's heights are almost all of it: R05's arrays add 1–3 ms.
    - The weights add 23 ms at L + 4 (5.4 µs a vertex), 10 ms at 19 and 19 ms on the Moon-like
      rim, against T2's 21–26 µs a vertex from a point query: the bake's sample and the forms kept
      by cell spare most of it.
    - The high setting's double normals ask 12,416 more points: 3 times the low setting's bake.
    - The owner's figures on the i7-8665U stay pending by hand.
- **Questions for "main" (R10.T6.a)** (built to the lean).
  - _The height at an unsurveyed vertex._ It touches Design note 16's rebake rule, which T6.b
    builds, and reads T6.a's block test per vertex. Lean, built: the true ground where the view can
    make it, else the reference surface with `is_complete()` false (above), so that T10.c's skirt
    hangs from the drawn ground's own edge. Then Design note 16's "only patches whose survey mask
    it changes are rebaked" becomes "and every incomplete patch on any block", and "a new chunk
    never changes a baked height" holds at every vertex surveyed before, or answered before. The
    alternative is the reference surface at every unsurveyed vertex, whose bake no block changes
    without changing its mask, but whose edge quads draw a slope of terrain halfway down to the
    datum. Either way the ground is drawn up to half a quad past the mask's edge: real margin
    ground under the lean, an invented slope under the alternative, which T10.c may address.
    Once ruled, Design note 16 takes a pointer here. _Answered by "main", 2026-10-14:_ the lean,
    as built. T6.b builds the widened rebake rule (Design note 16's as-built note; "Deviations in
    T6.b, as built").
- **Deferred findings (2026-10-13 rule)** (R10.T6.a; recorded, not ruled; each leaves the algorithm
  and the contracts as they are).
  - _The bake's budget._ 162 ms with the weights and 139 ms without, on the Earth-like L + 4
    patch, against the laptop's 40 ms, of which R09's heights are 136 ms. R09's fused interpolant
    pass (queued) is the lever, before this bench's quiet figure.
  - _A client's field costs a quarter more a height._ R09's bake of the same patch reads 31% longer
    over the client's `PartialField` than over the whole `CoarseField` at the Earth-like L + 4, 26%
    at 19 and 9% on the Moon-like rim: the channels read cells most. Not profiled. The
    `PartialField`'s slots are `Option<SynthesisCell>`, and each read checks the level and the
    slot (R09's `field/partial.rs`); a dense array with a held bitmap is a lever.
  - _The high setting's normals_ triple the bake (476 ms): R09's bake makes the vertices' heights,
    and the double grid's 12,416 other samples are asked one by one. R09 baking the double grid
    with the cover, or double normals only near the camera, are levers.
  - _Coarse patches' closed forms._ A patch below the coarse level less six has each vertex in a
    coarse cell of its own, so its weights and σ compute the level-keyed forms afresh at every
    vertex (50–150 µs, R09.T7.h): some 0.2–0.6 s a patch on a cratered world, not measured.
    R09.T7.h's table by band level is the lever.
  - _The weights' per-vertex site_ (T2's finding) stands. The bake spares the height and the kept
    forms, and the site is still built at every vertex.
  - _Unsurveyed vertices in the palette._ An edge patch's palette holds the materials of its
    answered unsurveyed vertices, so a material none of its drawn ground shows can take a slot.
- **Deviations in T1.a's slope follow-up, as built** (2026-10-14, lane 12, on R10.T1.b at
  `ba371487` and merged with R10.T5 at `1a56ee17`, with the brief's amendments of 2026-10-14: the
  sign-off of T12's drafts, and the ruling on the geometry). Files: `material/bearing.rs` and `material/mod.rs`. The level figures
  T1.b's table golden pins are unchanged, so no golden moves and `GENERATOR_VERSION` stays 21.
  - _The factors: the flush pad in its base's frame_ (ruled 2026-10-14 on the issue, reading F).
    The brief named Vesić's ground-inclination factors and the base-tilt factors that go with
    them. The capacity instead takes the base frame's inclination and soil-inertia factors.
    - A pad rests flush on the slope under the craft's vertical weight. Turned through β, it is a
      surface footing on level ground whose load leans at β to the base's normal (H = V tan β),
      and whose soil's weight leans by the same β (ρ g cos β normal to the base).
    - That is Paolucci and Pecker's pseudo-static problem with `k_h` = tan β on both, "the soil
      inertia f_x = k_h f_y and the horizontal component of the seismic action H = k_h V" (1997,
      Soils and Foundations 37(3), 95–105, Fig. 11, p. 101). On a static slope it is exact.
    - `q_ult` = c `N_c` `s_c` `i_c` + ½ ρ (g cos β) B `N_γ` `s_γ` `i_γ` `z_γ`, with:
      - x = tan β q ÷ (q + c cot φ);
      - Vesić's `i_q` = (1 − x)^m and `i_γ` = (1 − x)^(m+1), m = 1.5 at B = L (Vesić 1975,
        Eqs. 3.14a, 3.17a and 3.18a, pp. 130–131);
      - `i_c` = `i_q` − (1 − `i_q`) ÷ (`N_c` tan φ) (his Eq. 3.15, p. 130);
      - Paolucci and Pecker's `z_γ` = (1 − tan β ÷ tan φ)^0.35 (their Eq. 10, p. 99);
      - `z_c` = 1, since the cohesion term is the weightless soil's.
    - No ground-inclination or base-tilt factor enters. `N_c`, `N_γ`, `s_c` and `s_γ` are the
      module's, unchanged.
  - _The adhesion._ Vesić's Eqs. 3.14a and 3.17a print the soil's c in the denominator, but his
    Example 3.3(b) applies them with the adhesion `c_a` = 0 (p. 131), as EM 1110-1-1905 writes
    `c_a` (Table 4-6). Here `c_a` = c, as the ruling directs.
  - _The solve._ x needs the pressure, since V is the capacity itself.
    - The capacity is the root of q = `q_ult`(x(q)), found by bisection on [0, the level capacity]
      with a fixed 64 halvings, `f64::midpoint` at each. The right side falls as q rises, so the
      root is unique, and the fixed count makes it the same on every target.
    - With c = 0, x = tan β and there is no solve. 1 − x is held at 0.
    - `N_c` tan φ is taken as `N_q` − 1, equal in algebra and one rounding fewer.
    - Without the c cot φ term the drift's figure at 10° would be 25.88 kPa, 7% low. A test pins
      27.84 kPa.
    - A test pins the solve's bits on seven cases, taken natively, so that a divergence between
      the native and WebAssembly targets fails rather than passing at 0.1% (the determinism
      audit).
  - _The API._
    - `bearing_capacity_on_slope(p, gravity, footing_width, slope: Radians)` and
      `ultimate_bearing_capacity_on_slope(strength, bulk_density, gravity, width, slope) ->
Option<Pascals>` sit beside the level `bearing_capacity` and `ultimate_bearing_capacity`.
      Those keep their signatures and figures, and share the private `level_terms` with T1.a's
      arithmetic in its order.
    - The brief allowed a new function or a changed signature. This one was chosen because
      T1.b's `material/golden.rs`, landed meanwhile and outside this lane's scope, calls the level
      forms.
    - A slope of 0, of either sign, takes the level path with no solve, whatever φ. A test holds
      both the level functions and the slope ones to the formula as it stood at `89c27081`, by
      the testkit's `assert_same_bits`.
    - The first commit's `SlopeFactors` (Vesić's ground-slope factors) is dropped. The factors are
      private: `inclination_factors`, `soil_inertia_factor` and the solve, `flush_pad`.
    - T12 builds its reading on `bearing_capacity_on_slope`.
  - _The range._ The capacity holds for 0 ≤ β < φ.
    - That is Paolucci and Pecker's `k_h` < tan φ, and the β < φ of Hansen's and Vesić's slope
      factors (Vesić's Eq. 3.25). Within it, a pad as rough as the ground does not slide (Vesić's
      Eq. 3.12, whose φ is the soil–footing friction angle).
    - Vesić's α < 45° and ω < 45° (Eq. 3.24) belong to his tilt and slope factors, which are not
      used, so there is no 45° cut on a cohesive soil.
    - The shape factors multiply the inclination factors, as in Vesić's Eq. 3.13 and his Example
      3.3. EM's note under its Table 4-6, which would not combine eccentricity and inclination
      factors, is not followed.

    The capacity is `None`, never 0:
    - at β ≥ φ with β > 0, outside every source's range;
    - on a cohesionless soil at tan β ≥ 1, tested on the angle at 45°, since `math::tan` puts
      tan 45° a rounding below 1;
    - wherever else the pressure is not positive and finite, level ground included: a cohesionless
      soil with no weight term (zero gravity, density or width), or an overflow. The level
      figure's bits are kept wherever it is positive and finite, while the level functions
      themselves are unchanged;
    - for a negative or NaN slope;
    - for a bulk density, gravity or width that is negative or not finite;
    - where the form has no shear table.

  - _The slope's basis_ (the amendments; `bearing_capacity_on_slope`'s doc comment, "The slope's
    basis", is the contract, and signoff-10's (h) wording applies as written).
    - The slope is the ground's adirectional slope over a fixed 2 m baseline, by Design note 7's
      central differences, never the slope at the `BASELINE` the operator selects.
    - The capacity's dominant term is the load's inclination to the base, which the ground under
      the 1.0 m reference pad sets. 2 m is the finest scale the terrain resolves, about two pad
      widths; a wider basis would smooth away the pad's seat.
    - Where the point is unresolved, the caller passes the RMS slope at 2 m. The capacity falls
      with β, so an upper estimate errs low.
    - It is a uniform slope's: a crest's or a toe's nearness is not modelled.
  - _The sources, read._
    - Paolucci and Pecker 1997 (Eqs. 10 and 13, Figs. 9, 11 and 12, pp. 99–102), in the scan
      J-STAGE publishes.
    - Vesić 1975 (Foundation Engineering Handbook, ch. 3, §§3.6–3.7, pp. 130–132: Eqs. 3.12,
      3.14a, 3.15, 3.17a, 3.18a and 3.19–3.25, Examples 3.3, 3.5 and 3.6), in a scanned copy.
    - Brinch Hansen 1970 (Danish Geotechnical Institute Bulletin 28, §7, pp. 7–8, Figs. 8 and 9,
      Eqs. 11–16), in the scan GEO publishes.
    - EM 1110-1-1905 (Tables 4-5 and 4-6, pp. 4-8 and 4-9; §4-5c, pp. 4-27 to 4-29), the Corps'
      own file as the Internet Archive holds it, since the Corps' site refused the download.
    - Vesić 1973, read by the lane's research agent, has no slope or tilt factors.
  - _The figures_ (the ruling's, reproduced apart from the code in Python's `math`, and pinned to
    0.1%):
    - On lunar regolith at 1.62 m/s² under a 1 m pad, level ground gives 5,368.547 kPa (the
      records' 5,368.6) and the top 15 cm's pair 206.512 kPa.
    - At 10° the pad bears 3,730.03 kPa, 69% of level: x 0.176224, `i_c` 0.747323, `i_γ`
      0.615918, `z_γ` 0.953151 and cos β 0.984808, a cohesion term of 2,766.32 kPa and a weight
      term of 963.71 kPa.
    - At 20° it bears 2,331.04 kPa. The top 15 cm's pair under 1 m at 10° gives 133.285 kPa.
    - At 3.71 m/s² under 1 m at 10°, Viking Lander 2's soil bears 209.916 kPa and the drift
      27.8416 kPa (bulk densities 1,400 and 1,200 kg/m³).
    - On a cohesionless φ 30° soil, q(β) ÷ q(0) = cos β (1 − tan β)^2.5 (1 − tan β ÷ tan φ)^0.35:
      0.53376 at 10° and 0.21398 at 20°.
    - At zero gravity, the lunar 30–60 cm layer at 10° gives 2,766.50 kPa = c `N_c` `s_c`
      (3,701.64 kPa) × `i_c` (0.747372), at x 0.176188. That flush identity tells F from the
      ground-slope and base-tilt readings.
  - _The limit as β → φ._
    - A cohesionless capacity falls to 0. Below 45° it goes as (φ − β)^0.35, and is below 10⁻³ of
      level at φ − 10⁻⁹ rad on the φ 30° soil. On a soil of φ above 45° it goes as (1 − tan β)^2.5
      toward 45°.
    - A cohesive one falls to its inclined cohesion term alone, c `N_c` `s_c` `i_c`. At φ − 10⁻⁹
      rad under 1 m that is 0.488117
      of level on the drift and 0.144005 on Lander 2's soil (3.71 m/s²), and 0.0290892 on the
      lunar 0–15 cm pair and 0.00100339 on the 30–60 cm layer (1.62 m/s²).
    - At φ, just above it and at 89° the capacity is `None`. A cohesionless φ 50° soil has none
      at 45° and 46°, and a positive one at 44.9°.
  - _Tests._
    - Level ground, bit for bit.
    - The ruled values above.
    - Vesić's inclined-load Example 3.3(b): `ζ_qi` 0.66, `ζ_ci` 0.62 and `ζ_γi` 0.52 at P ÷ Q 0.21
      and m 1.75. This is the one worked example of the inclination factors; the flush pad has
      none in print.
    - Never increasing, and positive, on a 20,000-step grid to just under φ on every layer of every
      shear table at its body's gravity, and on the cohesionless soil.
    - The limits, and `None` outside the inputs, on bedrock and for a cohesionless pad of no
      width.
    - The solve's bits on seven cases, on every target.

    No test panics, so `tests/panics.rs` is unchanged.
- **The geometry question (T1.a's slope follow-up), ruled 2026-10-14.** The lane asked which
  reading the capacity should take, with its lean, G: Vesić's ground-slope factors for a level
  base cut into the slope.
  - The first commit built G. The ruling chose F, the flush pad in its base's frame (above), and
    the second commit builds F.
  - G leaves out the shear the pad's weight puts on its base. It overstates on frictional ground,
    by 27% at 10° on a φ 30° soil, and does not fall to the limit at φ.
  - A, the ground-slope factors with the base-tilt factors at −β and the inclined load, fails the
    flush identity in weightless soil, where `g_c` `b_c` must be 1.
  - B, the first build of all (the base-tilt factor at +β, after EM's example), stays dropped.
  - On the lunar 1 m pad at 10°, F gives 0.695 of level, against G's 0.678 and A's 0.74.
- **Deferred findings (2026-10-13 rule) (T1.a's slope follow-up)** (each leaves the algorithm and
  the contracts as built, for main's list; 1–8 are the ruling's).
  1. _The pad's joint._ A pad loaded through a joint at height ζ above its base carries an
     eccentricity e = ζ tan β downslope. That is the hull's to apply: Paolucci and Pecker's
     (1 − 2ζ tan β ÷ B)^1.8, or B′ = B − 2e.
  2. _Sliding first._ The inclination factors assume the pad holds. On regolith the pad friction
     is 0.20 (about 11°), so the pad slides first. Whether the capacity yields to the em dash past
     `PAD FRICTION` is R10.T12's question.
  3. _Cohesive slopes past φ_ can stand, but no source in hand gives their capacity, so the em
     dash stands.
  4. _`z_γ`'s fit._ It was fitted in plane strain at φ 30°–40°, and is used here at 18°–54° on a
     square pad, unverified.
  5. _Fig. 9a's `N_c` reduction_ is not adopted. Read off the figure, it is about 5% on the
     cohesion term at 10°, so about 3–4% of the lunar 1 m pad's capacity there.
  6. _The spread between factor families._ Paolucci and Pecker's strip Eq. (13) lies about 19%
     below the square form at 10°, and Hansen's `i_γ` is 0.52 against Vesić's 0.62. Both lie inside
     the level capacity's own uncertainty.
  7. _The basis_ wants revisiting once the hull's pads are wider than about 1 m.
  8. _A static load only._ Touchdown's lateral load is not included.
  9. _The slope's own stability._ Vesić advises a slope-stability analysis besides for slopes
     steeper than φ/2 (1975, p. 132): the slope failing as a whole, which a footing's capacity
     does not cover. Nothing models it. The uniform-slope line in `bearing_capacity_on_slope`'s
     contract covers it only in part.
  10. _Draft 1's other level-ground text._ Its "quantities, as built" bullet still gives
      `bearing_capacity(properties, gravity, footing_width)` on level ground, and its wording
      bullet says "on level ground". Its example readout's `~5.4 MPa` sits under `SLOPE ~12°`,
      where the capacity is 3.4 MPa. The amendment leaves T12's text to T12's own commit.
  11. _The spread across the slope's uncertainty._ Across the example's 12° ± 4° the 1 m pad's
      capacity runs from 4.0 MPa to 2.9 MPa, while Draft 1 gives the figure a `~` with no `±`.
  12. _Design note 7's InSight wording._ It says "lander-stability slopes at 2–5 m … against a
      15° limit". Golombek et al. 2014 set the limit "because the seismometer cannot level itself
      on steeper slopes" (p. 1), and do not say stability (`bearing_capacity_on_slope`'s doc
      quotes the abstract). The design note is the owner's text.
  13. _Goldens._ T1.b's table golden (`tests/golden/material/table.golden`) pins `BearingFactors`
      and the two level lunar capacities. It could take the slope capacities too, but that file is
      outside this lane, so a unit test pins seven of their bits instead. A golden file of them,
      through `GoldenWriter`, is owed once T12 first shows the figure or a payload carries it.
  14. _signoff-10's two findings for this follow-up's list are still open._ They are "Regolith on
      small bodies" and "Loose soils' failure mode", from R10.T12's drafts' findings above. Both
      change the level formula beyond the slope this brief asked for, so they stay on main's list.
- **Deviations in T6.b, as built** (2026-10-14; lane r10-worker, on R10.T6.a at `373a0158`, merged
  to `5f02f0cb` before the gate).
  - _Files._ The crate's new `src/worker.rs`, with `worker/tests.rs`, `worker/golden.rs` and one
    golden, `tests/golden/worker/moon_like.golden` (header 22); `src/wasm.rs`'s per-body exports;
    `lib.rs`'s `pub mod worker` and summary line; and R09's `field/partial.rs`, which gains
    `PartialField::heap_bytes`, additively, as `CoarseField::heap_bytes` is. On the client, R05's
    `workers/{heightBake,height.worker,messages,pool}.ts` with their tests, and the new
    `workers/rebake.ts`, `rebake.test.ts` and `fieldsWasm.test.ts`; `terrainPass.ts` (its
    `source`); one call each in the spike's `spikeScene.ts`, `spikeRun.ts` and
    `surfaceQuery.worker.ts`; the smoke harness's `heightWorker.ts` and `terrain.ts` (the source);
    the shared fake `test/scriptedHeightWorker.ts`; and the new members in `cache.test.ts`,
    `terrainPass.test.ts`, `coarseFieldStore.test.ts` and `spikeScene.test.ts`.
  - _One object's methods._ The task's free functions are the methods of a `SurfaceFields` object
    (a `wasm-bindgen` class) that a worker makes once its module has loaded, so the module keeps no
    global state. Each is thin over the crate's target-independent `worker::WorkerFields`, which
    holds a body's `PartialField` with its `LevelBounds` (built once from the header), the cells'
    elevation range and its `SynthCache`, so that the native and wasip1 tests check what the
    browser runs, as `src/wasm.rs`'s rule has it. `field_init(body, header)`'s header is block 0's.
  - _A body and its seed._ A body is its system's raw 64-bit ID (a `bigint`) and its index, the
    halves of the wire's `BodyIdHex`, which the worker takes apart with `@hyperion/protocol`'s
    `parseBodyId` and `hexToU64`; the module refuses a block of another body than the one named.
    The payload carries no detail seed (R09's `FieldHeader` has none; the client reads it from the
    body's hooks section, `BodyHooksDto.detail_seed`), so each bake, reading, line of sight and
    material is given it: a bake from `BakeSettings.source`.
  - _The synthesiser each query._ A `Synthesiser` borrows its field, which the next block changes,
    and the crate forbids `unsafe`, so each query builds the body's afresh (R09's 1 to 10 ms, not
    timed here; its own level bounds among it), while the cache is kept from the field's start to
    its release, and the bounds kept with the field serve the level table alone (deferred, below).
    A test holds every kind of query to the same answers in any order from the body's one cache,
    and a cache warmed under the first survey to a fresh one's after the second's blocks
    (`assert_order_independent`).
  - _`field_init`'s answer, the level table._ Every block taken, the first and each later one, is
    answered with the body's level table in the test planet's `levelTable` layout, since it grows
    as cells arrive (T7's record asked it for an init). Level n's row is
    - its bound, `LevelBounds::bound_m`, R09's `level_bound_m` bit for bit;
    - its lowest and highest height: the held cells' elevations, widened by a micrometre and a part
      in 10¹² as R09's patch range is, plus the most the level's and its parent's contributions can
      add (`contribution_range_m`), and stretched to the datum, 0, which an unsurveyed vertex the
      field cannot make takes; exactly 0 and 0 while no cell is held;
    - and its largest vertex spacing on the equatorial radius, `vertex_spacing`.

    Every surveyed patch's R09 `height_range_m` lies in its level's row, and the table only widens
    (tested). The pool keeps the latest table of each body (`fieldLevels(body)`) for T4.b.
    - _No σ column_, which T4.b's client `LevelBounds` has: R09's `LevelBounds` holds none, and a
      body-wide σ a level would be the largest `omitted_rms` over the held surveyed cells, a closed
      form per cell and level (50 to 150 µs each, R09.T7.h), too dear to remake at every block over
      a level-8 survey. T4.b makes it, and chooses its rule.
    - _Not T4.b's lean for the heights_, the union of the six root patches' `height_range_m`: a root
      patch takes level 0's contributions alone, where a finer level's reach more, and any
      elevation a cell can encode (±2,147 km) over a face with no held cell. So each row takes the
      held cells' elevations with its own level's and its parent's contributions, as R09's patch
      range does. T4.b's text takes a pointer here.

  - _The messages._ `field-init` gains `form`, `payload` or `stand-in`. `field-held` gains `body`
    and `block`, a payload block's `FieldBlockAnswer` (`fieldLevel`, `newlySurveyed` as flat runs
    of cell indices, and `levels`), `null` for a release or a stand-in. A new reply,
    `bake-unsurveyed`. `BakedPatch` gains `layers`, `null` for the test planet: R10's
    `TerrainLayers` as the worker hands them over (`palette`, 8 slots of class code and entry, 255
    for none; `weights`, 65 × 65 × 8, empty when skipped; `surveyMask`, 65 × 65; `boundM`,
    `sigmaM`, `complete` and `unsurveyedCells`), each buffer transferred. `BakeSettings` and
    `TerrainPassOptions` name the source, `HeightSourceSetting`: the test planet with its `ridges`,
    or a body with its `detailSeed` and `weights` (`baked` or `skipped`, the wireframe's).
  - _The rebake rule, as built_ (main's ruling of 2026-10-14; Design note 16's as-built note). A
    patch is baked again after a block where its bake is not complete, or where the block surveys a
    cell holding one of its vertices outside the survey, and after no other block. A vertex's mask
    is its field-level cell's (`PatchKey::containing`), and a cell's code only gets finer, so the
    mask changes exactly there. The worker answers each block with its `SurveyChange` (the cells of
    its surveyed ranges the field did not hold as surveyed), and each bake carries its
    `unsurveyed_cells` (the cells of its unsurveyed vertices, rising). The rule is
    `BodyBake::needs_rebake` in the crate and `rebake.ts`'s `needsRebake` on the client.
    - In the crate's test, six patches across the two surveys are baked after the first payload,
      the second payload's blocks are taken, and every patch the rule passes over bakes the same
      bytes after it. A patch surveyed whole is never taken, the patches beside the second survey
      are taken with changed masks, and the patch beyond every held cell (incomplete) is taken on
      every block. `fieldsWasm.test.ts` holds on the module that every patch the client's rule
      passes over bakes the same bytes, and that it takes some and passes over others.
    - _In flight_, the pool takes the rule conservatively: a bake made while blocks of its body were
      being posted is baked again unless it is complete with no unsurveyed cell, since the worker
      answers it before the block behind it, so the block's change may not be known yet.
    - _Delivered_, the pool reports each block's change once, from the first worker to answer it
      (`onFieldChange`, a `FieldChange` with the body and the block's place), and the holder of the
      bakes applies `needsRebake`. No view makes a body's terrain pass yet, so the eviction and the
      asking again are the first consumer's (T10.c or T11).
  - _Per-body bake epochs_ (T7's deferred item, settled). A bake in flight records its source
    body's field, as kept then, and the blocks posted. Only that body's field starting anew, or its
    release, makes it stale; another body's field never does, and nothing does the test planet's,
    which reads none. A body's bakes are not sent before its field starts.
  - _Bakes that wait_ (new). Between a payload's blocks, which split it in cell-index order, a
    survey's margin may be still to come, and a surveyed vertex that reads a cell not held is
    T6.a's `NotSurveyed`. The worker answers it, and a body with no field held, `bake-unsurveyed`;
    the pool holds the request until the body's next block, or asks again at once if a block came
    while it was in flight. Each frame's demand leaves a waiting request waiting (re-scored), and a
    release of the field forgets them, the next demand held until a field starts. Nothing reads
    `waitingCount` yet: R05's `TERRAIN: STREAMING` counts these patches among those it finds
    missing, and the first consumer tells them apart (Design note 16's "patches waiting on a chunk
    of a surveyed region"). `blocksInFlight` counts the blocks the running workers have still to
    answer; a replaced worker's are forgotten, the replacement being posted them afresh.
  - _The spike's stand-in_ (T7's first option). The spike posts its synthetic pieces as a
    `stand-in`, which the worker holds undecoded in JavaScript (`StandInFields`, T7's `HeldFields`
    renamed); every payload's block goes to the module. Each worker still holds the spike's 15 MB.
  - _`heldBytes`_ is the module's fields' heap, `PartialField::heap_bytes`, plus the stand-ins'
    bytes. The caches are not counted. A field keeps a slot for every cell of its level, so it is
    about 13 MB at level 8 whatever it holds (0.7 MB at the fixture's level 6).
  - _`material` answers only surveyed ground._ It answers nothing where the point's cell is not
    surveyed, a margin cell or one not held, as `reading` does, though the classifier could answer
    over a margin: a readout states what the ship measured. Its answer is the class code, the
    entry and, where the palette names the entry, the properties: A_N in B, V and R, the phase
    law's code, L(0), the grain density, the transition and the mechanics family's code. The
    bearing capacity and friction are T12's (with the slope, the gravity and the month).
  - _No answer yet._ `reading`, `material` and `bake` answer nothing, and `slantRange`
    `NotSurveyed`, where no field is held, and also where a surveyed point's reads reach a cell
    still to come (a margin in a later block): T12's readout shows `NOT SURVEYED` there until the
    block arrives (asked of main, below).
  - _Arguments in range._ Every argument is checked in `src/wasm.rs`, so that no input traps the
    module: a body index above 65,535, or a face or level past a byte (each taken as a `u32` and
    checked, not wrapped), a face, level or cell that names no patch, (u, v) outside [−1, 1], a
    slope baseline outside 1 mm to 10,000 km, a line of sight's origin outside 1 m to 10¹² m from
    the centre, a direction whose squared length underflows or overflows (as `unit_dir` would
    refuse), a reach outside 0 to 10¹² m, and a negative skirt margin. The bounds are wide of any
    use and far inside the 10¹⁵⁴ m where the datum foot's squares overflow (tested on the module).
    A block of the field's body at another level is refused before its change is read
    (`InsertBlockError::WrongLevel`, tested), since its cells could index past the field's.
  - _The readings' worker messages_ are T12's; T6.b gives `WorkerFields` its `reading`,
    `slantRange` and `material`, which copy the module's answers into plain objects.
  - _The coverage lag_ (T7's deferred item), settled for the terrain's own mask: each bake carries
    the mask of the field it was made over, so T10.c's discard reads the bake's mask. Design note
    12's reference surface is discarded by the GPU coverage mask, the store's, which lags the
    workers by up to a payload and, after a payload the link cut off, names none of its blocks
    until the next answer; there both surfaces would draw. T10.c builds that mask from the
    workers' cells (the pool's `onFieldChange`, whose `newlySurveyed` runs are each block's), or
    states the lag (asked of main, below). _The pool's own copy_ (T7's) is not touched and stays
    deferred.
  - _The module_ grows from 118 KB to 715 KB, now linking R09's synthesis and R10's bake, readings
    and classifier.
  - _Acceptance_, as built: `just gen-surface`, then
    `pnpm --filter hyperion exec vitest run src/renderer/src/view/terrain/workers`; and
    `cargo test -p hyperion-surface --lib worker` (20 tests), the wasm targets through smart-ci's
    wasip1 and browser suites, and `src/renderer/src/view/terrain` and `view/spike` by name.
  - _Tests_ (`worker::tests`, `worker::golden`, `fieldsWasm.test.ts`, `heightBake.test.ts`,
    `rebake.test.ts`, `pool.test.ts`):
    - the worker takes every block into the field a client builds from the decoded payloads, and
      reports each surveyed cell once, by the block that carries it; a block sent again surveys
      nothing;
    - a block before its field is refused, as are a start from a later block, a block of another
      body, a block of the body at another level, a truncated block and a block 0 of another
      header, each leaving the field as it was; a second start replaces the field; init, blocks
      and a release leave nothing held;
    - the level table bounds every surveyed patch of its level, holds the datum and only widens;
      it is the datum alone before any cell;
    - each bake is `bake_terrain` over the same field, and its unsurveyed cells its mask's; a
      survey's bake waits for its margin; the rebake rule (above);
    - each reading, line of sight and material is the free function's over the same field, in
      either order from the kept cache; every kind of query, interleaved, is the same in any order
      from the body's one cache, and after the second survey's blocks a cache warmed under the
      first answers as a fresh one; no field gives `NotStarted`;
    - _the entry points' agreement with native_: `fieldsWasm.test.ts` posts the golden's blocks
      through the worker's answers and holds every block's change and the level tables, six bakes'
      digests, scalars, palettes, masks and weights, eight readings, three lines of sight (ground,
      not surveyed, no ground) and four materials to the golden, bit for bit; and refuses every
      argument out of its range without trapping;
    - the pool: per-body epochs, bakes held before the field and after its release, waiting,
      waking, re-scoring and dropping, the conservative rule in flight, each change reported once
      and not from a replacement or a released start, the latest level table, the blocks still
      to answer, the stand-in's form to a replacement too.
  - _Generator version._ Untouched (22). The worker's golden is presentation, as T6.a's are: a
    change to the bake, readings or classifier's use alone re-blesses `worker/` at the version in
    force, and a change to R09's heights or closed forms, or to the classifier, with theirs.
- **Questions for "main" (R10.T6.b)** (neither touches a contract; built to the lean).
  - _A surveyed point whose margin is still arriving._ While a payload streams, a reading or
    material at a surveyed point whose reads reach a margin cell in a later block answers nothing,
    which T12 shows as `NOT SURVEYED`; signoff-9's and signoff-10's rows have no streaming state.
    Lean: as built, since it lasts a block; the alternative is a pending state in T12's readout.
  - _The reference surface's GPU mask._ Should Design note 12's GPU coverage mask follow the
    workers (each block's `newlySurveyed` through the pool's `onFieldChange`) rather than the
    store's answers, so that the terrain's mask and the reference surface's agree while a payload
    streams and after one the link cut off? Lean: yes, in T10.c, which builds the mask; T6.b
    provides the change.
- **Deferred findings (2026-10-13 rule) (R10.T6.b)** (each leaves the algorithm and the contracts as
  built).
  - _The synthesiser each query._ Each bake, reading and material rebuilds the body's synthesiser,
    1 to 10 ms by R09's figure (not timed). A synthesiser made from prepared parts over another
    borrow of the field (R09's `synth.rs`) would spare it.
  - _The level table's heights are loose._ On the Moon-like fixture R09's `contribution_range_m`
    is about ±200 km at every level (the craters' 1 − 10⁻⁹ quantile), and the bound at level 0 is
    296 km, so the table's heights, as R09's `height_range_m`, bound culling loosely.
  - _The body's cache grows._ The `SynthCache`'s channel part is never pruned (R09's policy), and
    the worker keeps it until the body is released; dropping it as the camera moves on is the first
    consumer's to decide.
  - _A field at rest is the level's size._ A level-8 field costs each worker about 13 MB whatever
    it holds, three workers about 40 MB besides the pool's copy; T6.a's dense-array lever applies.
  - _The rule in flight is conservative._ An edge patch in flight while a block is posted is baked
    again even where the block misses it; waiting for the block's answer would make it exact.
  - _The datum in the table._ Stretching the heights to 0 widens a body whose held ground lies all
    above or below the datum; T4.b may drop it once T10.c's skirts at the survey edge are drawn.
  - _The ceiling's walk_ (T5's finding) stands: `slant_range` still reads every held cell a call,
    which the worker could keep with the field.
  - _A field that shrinks needs a fresh cache._ The body's cache is correct only because a
    `PartialField` never drops a cell (`SynthCache`'s rule). Should a later change evict cells as
    the camera moves on, it must start the cache afresh with them, or a cache filled under the
    larger view could bake real ground where the smaller view's fresh bake takes the reference
    surface (the determinism review).
- **Deviations in T10.a, as built** (2026-10-14; lane r10-photometry, on R10.T6.b at `bfdcc837`).
  - _Files._ `crates/hyperion-surface/src/material/albedo.rs` with `albedo/tests.rs`,
    `material/mod.rs`'s exports, and two goldens in `tests/golden/albedo/`; the server's
    `surface/service.rs` (`compute` and the public `field_albedo_scale`) and `surface/mod.rs`'s
    documentation; `tests/surface_service.rs`, `tests/surface_knowledge.rs` and
    `tests/common/surface.rs`'s `served_field`; `crates/hyperion-sim/tests/surface_albedo_worlds.rs`.
    Documentation only: `FieldHeaderParts::albedo_scale` and its getter (`field/header.rs`),
    `CoarseField::with_albedo_scale` (`field.rs`), the protocol's `SurfaceHeaderDto::albedo_scale`
    with its regenerated binding, and R09's Design note 18, payload text and Generator version.
  - _The signature._ Provides' `albedo_scale(field: &CoarseField, geometric_albedo: f64) ->
AlbedoScale` is two functions, since the class fractions need the synthesis under the body's
    detail seed, which the field does not hold:
    - `class_fractions(synth, cache) -> Result<ClassFractions, QueryHeightError>`;
    - `albedo_scale(&ClassFractions, &MaterialPalette, geometric_albedo_bvr: [f64; 3]) ->
Result<AlbedoScale, SolveAlbedoScaleError>`, taking p in B, V and R for the stated residuals.
      Its refusals: a band of p not finite and positive (`GeometricAlbedo`), ground with no named
      substance at all (`NoNamedGround`), and an entry past the palette (`Properties`).
    - `AlbedoScale`: `scale()`, `geometric_albedo_bvr()`, `target_bvr()`, `residual()` and
      `relative_residual()` per band, `clamped_low_share()`, `clamped_high_share()` and
      `unnamed_share()`. `ClassFractions::new` (areas finite and non-negative, their total finite
      and positive: `BuildClassFractionsError`), `shares()`, `share(material)` and
      `unnamed_share()`.
    - Also public, for T10.b's shading and the consoles' readings, which must draw what the solve
      assumed: `scale_range`, `scaled_normal_albedo` and `ALBEDO_SCALE_FLOOR`.
  - _The fractions, over every cell._ Each coarse cell's T2 weights at its centre at band level
    L − 6, at which a texel is the cell (T2's record, "The texel"), band level 0 for a field
    coarser than level 6 (a texel a quarter of the cell or less, at its centre), each weighted by
    `cell_area` on the header's figure and summed in cell-index order from +0. Ground whose
    palette names no substance is held and reported, and left out of the solve: the named ground
    is solved for the body's p.
  - _The solve._ Σ aᵢ xᵢ(c) [2 + Lᵢ(xᵢ)] ÷ 3 = p in V, Lᵢ the class law's
    `limb_parameter(xᵢ)`, evaluated at the sorted breaks (each range's ends, and 0.60 and 1.35 for a
    particulate law) and solved exactly on the piece that holds p. The module documentation derives
    the uniform sphere's p = `A_N` (2 + L) ÷ 3 and the orientation-averaged p of a patterned body as
    the area average of its elements' (the science check confirmed both, the second by Monte Carlo,
    and noted it holds on any convex figure); the sim's McEwen 1991 citation for the first was not
    seen in the paper, so it is cited as derived, with Buratti et al. 2017, p. 10, for L = 1.
  - _The clamp, T10.a's own rule_ (T1.a's deferred finding "The spans as a clamp": "T10.a clamps by
    its own rule"). A material's range is its form's span stretched to hold its entry's own V
    albedo, capped at its law's `normal_albedo_ceiling`; with no span, 0 to the ceiling. The
    ruling's "clamped to its form's span" is so read because the spans are Solar System anchors
    (the lunar regolith's 0.057–0.184; the synthetic feldspathic crust's 0.20 is above it) and the
    entry is the substance's own from its registry row: c = 1 always draws the entry's albedo, up
    to the ceiling.
  - _One c, fitted in V_ (main's answer to the re-validation's second question). A material's B
    and R take the factor its V took, so its colour holds, each capped at its law's ceiling, since
    the energy bound is per band (decision-r10-limb §4, from the science and plan reviews). The B
    and R residuals are stated (`residual()`, `worlds.golden`; table below).
  - _Ties, misses and the floor._ Where no c gives p, c comes nearest; where several do (every
    material clamped, or a flat piece at p) it is the one nearest 1, the entries' own albedos. A
    piece is flat where no material moves at its middle. c is at least `ALBEDO_SCALE_FLOOR`, 2⁻²⁰,
    a choice: where spanned materials already sit at their floors above p, the least residual is
    only approached as c → 0, which the header refuses. A material within 10⁻¹² of a range end,
    relative, counts as clamped there.
  - _The call site and the gate_ (main's rulings, 2026-10-14). `service::compute` sets
    `field_albedo_scale(&field, record.photometry(), detail)` on the field after `coarse_pass`,
    the detail seed being the record's hooks section's (the derivation `detail_seed` where it is
    not given). It is `Some(c)` only where the photometry is `Ok`, the field header's surface
    pressure is below 30 kPa (`THIN_ATMOSPHERE_PRESSURE`, the photometry's own split, the surface
    crate's copy tied to the sim's by `surface_material_worlds`), and the photometry's law has a
    ground term, `lunar_lambert_share() > 0` (Mercury, Moon, airless ice, Mars, thin magma ocean,
    snowball). `None`, documented as the contract on `FieldHeaderParts::albedo_scale` and the
    protocol's DTO, means no scale, the palette's albedos as resolved: where p is not modelled,
    where the air hides the ground (30 kPa and up: the cloudy disc's p, whose reflectance
    R08.T16.b computes from the medium over the ground), where the law is a cloud deck's at any
    pressure (Venus's, which a runaway greenhouse takes at 1 kPa, Earth's and the giants'; the
    science check's finding, ruled by main), and, beyond the rulings, where the palette names no
    substance. A solve that fails otherwise is a bug, logged at `error`, and gives `None`.
  - _Generator version_ (main's ruling). No bump, against the task text: R09.T19.b's ruling freezes
    the payload's layout only at the first payload served for a generated body, or a client's first
    persisting of one, and none is served before P14.T54.a (`CoarseInputs::for_body` cannot give a
    generated body's inputs yet). No existing golden moved (`golden_diff.py`, run by the
    determinism review: 22 → 22, two new goldens, nothing pinned changed): the server sets the
    scale after `coarse_pass`, whose output keeps `None`, as the testing worlds' payloads in
    `wire/` do. The new goldens carry 22: `albedo/kernels.golden`, 64 random cases' scale, p in B,
    V and R and clamped shares, bit for bit on native, `wasm32-wasip1` and the browser target; and
    `albedo/worlds.golden`, each synthetic world's fractions and its solve at its analogue's p with
    the residuals, natively and on `wasm32-wasip1` (its fields cost too much for the browser
    target's lib binary). The solve and the ranges are generator-versioned from them.
  - _Results_ (V fitted; B and R residuals relative; p is the template the photometry chooses for
    the world's state, its analogue's own):

    | World                  | Level | Ground                                                                                            | p (B, V, R)                   | c                 | B, R residual | Clamped                 |
    | ---------------------- | ----- | ------------------------------------------------------------------------------------------------- | ----------------------------- | ----------------- | ------------- | ----------------------- |
    | Moon-like (synthetic)  | 6     | regolith 94.5% feldspathic, 5.5% mare basalt                                                      | Mercury's 0.105, 0.142, 0.172 | 0.730             | +35%, −17%    | none                    |
    | Ceres-like (synthetic) | 5     | regolith of phyllosilicate                                                                        | Mercury's                     | 1.511             | +35%, −17%    | none                    |
    | Mars-like (synthetic)  | 7     | dust 98.3%, H₂O and CO₂ glacier 0.77% each, sediment 0.17%                                        | Mars's 0.088, 0.170, 0.288    | 0.772             | +93%, −41%    | 1.5% low (the glaciers) |
    | Earth-like (synthetic) | 8     | seabed 63.6%, soil 13.6%, sand 10.5%, glacier 6.5%, sea ice 5.3%, bedrock, snow and sediment 0.5% | Earth's 0.263, 0.215, 0.210   | 1.056, not served | −18%, +2%     | none                    |
    | Mars (reference)       | 7     | dust of `mars_dust`                                                                               | Mars's                        | 0.791             | +93%, −41%    | none                    |
    | Moon (reference)       | 6     | regolith 99.6% feldspathic, bedrock 0.4%                                                          | Mercury's                     | 0.710             | +35%, −17%    | none                    |
    | Ceres (reference)      | 5     | regolith of phyllosilicate                                                                        | Mercury's                     | 1.511             | +35%, −17%    | none                    |
    | Earth (reference)      | 8     | sand of basalt and granite (the no-op pass)                                                       | Earth's                       | 1.258, not served | −18%, +2%     | none                    |

    Every V residual is zero to rounding (at most 2 × 10⁻¹⁶ relative); the flat fields' palettes are
    empty, so they have no scale. The B and R residuals are the grey palettes' (every entry one
    figure in all three bands): one c cannot give a body's colour, and plan 14's registry palettes
    will carry colour (P14.T49.c).

  - _Tests._ `material::albedo` (on every target but the browser's for the fields): the solve meets
    p to 10⁻¹² where the ranges allow and states the range end's residual otherwise (400 random
    cases, a quarter of each random sample on the browser target by `test_sizes::per_target`, of
    up to 15 entries of any family, albedo to 1.6, colour 0.6–1.4 of V, and every code, and up to 8
    materials of any form, one in ten unnamed); it is a 200-step bisection of an
    independently written clamped sum, in p to 10⁻¹² and in c to 10⁻⁹ where the sum rises; no
    scaled albedo passes its law's ceiling in any band, at any scale from the floor to 10⁶, for
    every form, family and code; the solve depends on the fractions alone (areas reordered or
    doubled give the same bits); one material's closed forms under L = 1, between the anchors and
    under Lambert's, clamped high and low; the scale nearest 1 where the ranges leave it free, a
    black body's 1 and the floor; the refusals; the fractions' rules; `scale_range`; every form of
    every family under every code is solved; the kernels' golden; and on the synthetic worlds, the
    fractions within three points of total variation of the classifier counted at random cells'
    centres (1,000 on the Moon- and Ceres-like worlds, 400 on the Mars-like one, whose finest
    points cost about 20 ms each; a model-agreement tolerance, not a statistical one at
    α = 10⁻³: the determinism review's note), a
    used cache giving a fresh one's fractions bit for bit, and the worlds' golden with the airless
    and thin-aired worlds meeting p to 1%. The sim's `surface_albedo_worlds` runs on R09's
    reference Mars, Moon and Ceres (`sample_worlds()` follows R09.T16 under this ID): p to 1%, or
    every material that could move it at its range's end, and no band over its ceiling; and the
    reference Earth thick-aired under Earth's template. The server: `surface::service`'s gate on
    both sides of 30 kPa, a runaway greenhouse at 1 kPa, a thick and a thin snowball, the sections
    not `Ok` and an unnamed palette; the bit-for-bit tests applying `served_field`'s scale (one in
    `surface::service`, `tests/surface_service.rs` with the scale equal to `albedo_scale` of the
    field's fractions); and the `surface_field` answer and block 0 carrying the field's scale over
    the socket (`tests/surface_knowledge.rs`, since R09.T19.b has landed).
  - _Acceptance, as built._ The plan's three commands, and
    `cargo test -p hyperion-server --test surface_knowledge`, whose two tests' names the
    `surface` filter does not select.
  - _Cost_ (provisional: the dev profile at the surface crate's `opt-level = 2`, one thread, this
    machine's Ryzen 7 3700X under other lanes' load, 1-minute load 2 to 19): about 20 µs a cell
    from a fresh cache, 0.12 s for a level-5 field, 0.5 s at level 6, 2.4 s at level 7 and 8.1 s
    at level 8, on the pool with the pass; nothing for a body the gate refuses.
  - _Closed sets, for the composition audit._ None new: the solve reads T1.a's forms, laws and
    rows and T2's weights.
- **Deferred findings (2026-10-13 rule) (R10.T10.a)** (recorded, not ruled but where stated; each
  leaves the algorithm and the contracts as built).
  - _The seabed counts at its floor's albedo_ (ruled deferred by main). The under-water share
    enters the solve at its floor entry's lunar-Lambert albedo, about 0.1–0.2, where the disc shows
    water, whose zero-phase p is about R(0) ÷ 4 ≈ 0.005 (Fresnel R(0) = 0.020 at n = 1.333; a
    specular sphere; the science check's derivation), so a thin-aired world with seas has its land
    darkened to compensate. Until R11's ocean model gives the water's p; the lever is to solve the
    land for p − a_water p_water.
  - _Melt has no span_ (ruled deferred). Lava's and quenched glass's albedo is at most about 0.1
    (Essack, Seager and Pajusalu 2020, ApJ 898, 160, abstract, via the science check, not read
    here), but Melt's range is 0 to 1.35, so the solve can brighten it without bound; and the Magma
    template's p_V, 0.142 × 0.10 ÷ 0.11 = 0.129, is already above that limit (for plan 14). A T1.a
    table figure; Melt's particulate law for a liquid is a model choice for the same follow-up.
  - _Span-less forms go to the ends_ (ruled deferred). Bedrock, volcanic rock, soil, sand,
    sediment, seabed, melt and the others with no span range from 0 to the ceiling, so where the
    spanned forms pin, the floor or a large c drives them toward black or past fresh snow (a test
    shows soil at 10⁻⁷). T1.a's "Spans not carried" are the fix; a physical floor in `scale_range`
    would change the algorithm.
  - _A thick snowball is refused_ (ruled recorded, for plan 14). The snowball template's law is the
    ground's (L = 1) at any pressure, but from 30 kPa the gate sends `None`. Whether a thick-aired
    snowball, or a Titan whose haze hides its ground, keeps a ground template is plan 14's.
  - _An airless rock's p is Mercury's whatever its rock._ Plan 14 draws every airless rock with
    Mercury's p scaled by the state's Bond albedo, not by its palette's albedo, so the reference
    Ceres's phyllosilicate, 0.094 measured (its own row's Ciarniello et al. 2017), is scaled 1.51×
    to 0.142, and the Moon-like worlds' to 0.142 against the sim's own Moon row's 0.12. For plan 14
    (a p from the surface's materials, P14.T49.c's palette).
  - _One c in V against the colour._ On grey palettes the B and R residuals reach +93% and −41%
    (the Mars-like worlds), about a stop in B, so surveyed class-map texels would differ in colour
    from `lawFor`'s unsurveyed ones by that much (Design note 8's hand-over); T10.e's check, if
    read per channel, may trip. Main's ruling (one c, fitted in V) stands; per-band scales would be
    three header fields, a wire change. Registry palettes with colour will shrink the gap; for
    T10.e's lane and, if it persists, the owner (the brainstorm's per-channel albedo).
  - _q's residual._ Design note 8's "q then becomes a result of the classes, reported with its
    residual" is not computed here; T10.e, which draws the classes through each law's row, reports
    it beside T10.a's p.
  - _The solve's cost._ About 8 s of pool time for a thin-aired level-8 field beside its pass
    (provisional above); the lever is the fractions' per-cell site, which T2's deferred finding
    already names, or the pool's workers.
- **Deviations in T10.b, as built** (2026-10-15; lane r10-shader, on R10.T10.a at `87adcd38`, merged
  to `2955006e`).
  - _Files._ Beside the task's: R05's `shaders/terrain.wgsl` now composes a new shared
    `shaders/terrainPatch.wgsl` (the slot, instance and contact records, their bindings, and the
    vertex and normal arithmetic, R05's operations in their order), which the lit material composes
    too; the vertex paths' headers say so. `local_frame` and `local_azimuth` move from
    `bodyDisc.wgsl` to `litBody.wgsl`, which both materials compose. `slotLayout.ts`,
    `gpu/resources.ts`, `gpu/material.ts` (`PATH_SOURCES` exported), `terrainPass.ts`,
    `workers/{messages,heightBake}.ts`, the catalogue, `lighting/{discEclipse,planetshine}.ts`,
    `appearance/fromWire.ts`, and `src/wasm.rs` and `src/worker.rs` (with one light test) in the
    surface crate; the smoke harness's new `smoke/litTerrain.ts` in `page.ts`'s part 6.
  - _The L(α) channel_ (decision-r07-t8b). `PhotometricLaw.lommelSeeligerCurve?:
"mcewen-1996"` (`LommelSeeligerCurve`), `mcewenLunarProfile`, `limbShareAt(law, α)` and
    `MCEWEN_LUNAR_L` (the surface crate's three) in `law.ts`; `PhaseFactorTable.alpha`, L(α) at each
    sample's own phase (L(0) at every sample without a curve), read by `limbShareFromTable` and by
    the shaders' new `phase_row` (`litBody.wgsl`; `phase_factor` reads its rgb). `shapePhase(l0, α,
shareAtPhase = l0)`; `PhaseTemplate.lommelSeeligerCurve` on `moon`, `mercury`, `airless-ice`,
    `snowball` and `magma`; `withTemplateCurve(law)` (`phase.ts`), which `lawFor` and
    `modelledPhotometry` apply, so a body from the wire on those templates takes the curve.
    `brdfFromTable` takes L(α) from the table (its law argument is `Pick<PhotometricLaw, "a">`), as
    the disc's twin (`surfaceReflectance`) and its shader (`surface_reflectance`, `lit_disc_term(row.a,
…)`) do. `bodyReflection` integrates the spheroid at L(α) and scales A at L(0);
    `discEclipseVisible` takes an `EclipseShare`, L or L(α) as a function of its phase;
    `phaseMaximum` and `packPhaseFactorRows` (alpha written) follow. The probe's per-texel case now
    takes its A from a normal albedo and its L(α) from its row (`syntheticShare` goes: the row
    carries R10's L(α)).
  - _McEwen's root is 103.948°_, which the task's and decision's 103.9° rounds (L(103.9°) =
    7.7 × 10⁻⁴); the test holds 0 from 103.95°.
  - _q under the curve._ Past a template's range f is held, and under L(α) = 0 there the held f
    draws Lambert's tail, not Lommel–Seeliger's: q falls by 1.80 × 10⁻⁴ (the Moon), 3.73 × 10⁻⁴
    (airless ice), 6.84 × 10⁻⁴ (a snowball) and 1.4 × 10⁻⁶ (Mercury, magma), relative; p is
    unchanged and the point's flux holds to 10⁻⁴ at 30°–120°. The task's "q … to 10⁻⁴" is held at
    2 × 10⁻⁴ (`phase.test.ts`), recorded here; the decision's own figure was 2 × 10⁻⁴. So `lawFor`,
    solving s under the curve, meets the generator's constant-L q at an s lower by 1.38 × 10⁻⁴ (the
    Moon), 2.58 × 10⁻⁴, 4.32 × 10⁻⁴ and 9 × 10⁻⁷: `templatesGolden.test.ts` recovers the other seven
    templates' exponents to 6 places as before, and pins the five curved templates' shifts with the
    golden's q met to 10⁻⁶. Past 150° the Moon's point dims under the curve, 0.838× at 155°, 0.673×
    (−0.43 mag) at 160° and 0.339× at 170°, toward Allen's table (7.5 mag at 160°, `templates.ts`;
    the constant-L tail gives 6.80, the curve 7.23). _Ruled 2026-10-15 (main's ruling for
    R10.T10.b): accepted; the shift is the held tail's alone, computed independently._
  - _The class laws and their rows_ (`terrainMaterial.ts`). The worker resolves each palette slot's
    photometry (`TerrainLayers.photometry`, 8 × 5 `f32`: c `A_N` in B, V and R, the law's code and
    L(0); `SurfaceFields.photometry(system, body, palette)` over `WorkerFields::photometry`, the
    header's `albedo_scale`, 1 where it holds none, through `scaled_normal_albedo`), transferred with
    the other layers. `CLASS_LAW_TEMPLATES` maps codes 1–3 to `moon` at s = 1, `airless-ice` at
    `AIRLESS_ICE_EXPONENT` (the generator's `S_AIRLESS_ICE`, 0.8215365, Ganymede's q of 0.80 at
    constant L) and `lambert` at s = 1. `TerrainPhaseRows` keys a row by (law, L(0)), row 0 the
    body's uniform law, kept across a restore so a slot's rows hold; the task's "three rows" are
    read as three kinds of row, since a bright frost's L(0) needs a row of its own and no f is
    taken from a neighbouring L (decision-r07-t8b); the row-k-is-code-k test this replaces is the
    keying's. `PHASE_LAW` names the codes.
  - _One slot field, `class-layers`_ (34,056 B a slot): its class weights (33,800 B, two `u32` a
    vertex) then its class laws (256 B, two `vec4f` a palette slot: A in display order, R V B as
    r g b, as `modelledPhotometry` maps them; L(0); the row, −1 for none), one storage buffer, not
    Design note 15's filterable `rgba8unorm` array: the weights are interpolated by hand
    (bilinearly between the quad's four vertices, no gutter needed), and with one buffer for both
    the `baked-offsets` lit material binds seven storage buffers, one below WebGPU's default eight
    a stage, left for T8.b's horizon map; T10.c's survey mask must then join a buffer or take a
    texture. _Ruled 2026-10-15 (main's ruling for R10.T10.b): neither; the bindings take per-stage
    visibility (T10.c), and the mask and the horizon map keep their own storage fields._
    `TerrainShading` (`"spike" | "lit"`) adds the field to
    `terrainSlotFields(terrain, shading)` for a body baked with its weights alone, so R05's test
    planet, the spike and their slot counts are unchanged (T14 sizes). The bindings: R05's 0–5,
    then 6 `class_layers`, 7 the phase table, 8 `terrain_lights`, 9–15 R08's tables.
  - _Law 0 takes what the weights leave._ A texel's weights are each slot's interpolated bilinearly
    between its quad's four vertices; the slots with a law keep theirs, and law 0, the body's uniform
    law (R07's `elsewhere`, A scaled for the figure as the disc's), takes the rest: an unnamed
    substance's slot (law code 0) and the vertices with no weights, outside the survey, as R07's
    disc's `surface_shares` gives `elsewhere` what its map leaves. So unsurveyed ground shows what
    the disc shows, rather than reflecting nothing, and T10.c's discard then draws the reference
    surface there, which shades with the same law.
  - _Not `body_brdf` itself._ Each law's I/F is its row's `phase_row` under `lit_disc_term(row.a,
h, μ₀, μ, ρ)`, R07's disc term under an extended source, as `bodyDisc.wgsl`'s
    `surface_reflectance` takes it; it equals `body_brdf` wherever the whole source is up and μ₀ +
    μ exceeds ρ, so the disc and the terrain read one row one way (T10.e). Each class's
    `LunarLambert` carries A, L(0) and its row; its `s` is (1, 1, 1), which no shader reads, the
    rows being built on the CPU.
  - _R08.T9.b had landed_ (`db3bdc92`), so the task's "R07's four- and three-argument stubs"
    branch does not apply: the shader calls `surfaceLighting.wgsl`'s three lookups with the
    `curve` slot and binds R08's seven tables; a body without air takes `createAirlessTables`'
    one-texel tables, which read 1 and 0.
  - _The lighting is R07's disc's_ (`bodyDisc.wgsl`'s `shade`), body-fixed: up to two stars, two
    occluders and two planetshine sources in a `terrain_lights` buffer (`TerrainLighting` of R07's
    `DiscLight`, `DiscOccluder` and `DiscSecondary`, relative to the centre ÷ a,
    `packTerrainLighting`, 25 rows), the horizon term on the terrain's normal at a local horizon of
    0 (T8's horizon map gives it), the eclipse term on the direct sun and the sky, R08's lookups at
    the texel's mixed baked height, the datum normal's latitude and the sources' cosines against the
    datum normal, their azimuths from north, slot j's curve for star j and 0 for planetshine. The
    texel's place, for the eclipse and planetshine geometry, is the camera's body-fixed place (the
    `cameraBodyFixedM` uniform, w 1 ÷ a) plus the vertex's offset from it, in `f32`.
  - _The sky on a slope_ (the science check). R08's irradiance is a horizontal surface's, and its
    record ("A tilted surface", R08.T9.b) leaves Bruneton's (1 + n · up) ÷ 2 to the caller, so the
    texel's sky light takes (1 + n · ν) ÷ 2 for the terrain's normal n (Bruneton 2017's
    `GetSunAndSkyIrradiance`), beside R11's `terrain_sky_factor`, which stays its cloud hook at 1.
    Light from the facing slopes, (1 − n · ν) ÷ 2 of the ground's, is not drawn.
  - _Angles through `preciseAtan2`._ The shader's own angles (the horizon term's zenith angle, the
    phase, the sources' angular radii and the eclipse's separation) are `preciseTrig.wgsl`'s
    `preciseAtan2` of a cross and a dot product, or of r and √(d² − r²), never WGSL's `acos`,
    `asin` or `atan2`, as this plan's Consumes takes R08's Design note 18 rule; R07's library
    functions it calls (`sphere_irradiance`, `eclipse_visible`) keep theirs (decision-r08-f32 §9's
    deferred item).
  - _The hooks._ `terrain_decoration(p, normal) -> TerrainDecoration { albedo, normal }` (a factor on
    the texel's reflectance and its shading normal; this task's signature, R11.T2.b fills it),
    `terrain_shadow_factor(p) -> f32` (1: T8 and T9 are not built, so there is no terrain shadow),
    `terrain_ring_shadow(p, sun) -> vec3f` and `terrain_sky_factor(p) -> vec3f`, each with no effect;
    the shadow and ring hooks multiply the direct sun alone (Design note 11), the sky hook the sky's
    irradiance; `p` is the texel from the centre ÷ a, body-fixed.
  - _The view._ No view draws a body's terrain yet (T10.c, T11), so `TerrainPass` takes the lit
    material for a body source with baked weights (`terrainShadingOf`) and its lights from a new
    optional `TerrainFrameInput.lighting` (with its atmosphere's tables; a body without air's
    one-texel tables otherwise, made per device), and `photorealFrame` takes a new optional
    `PhotorealInputs.terrainFootprints` into `depthWriters`, none by default. The aerial
    perspective is the `AtmosphereScene` composite's, nothing in the shader, but no caller draws a
    lit pass into an `AtmosphereScene` yet (the spike draws the test planet's shading): the first
    view to draw a body's terrain (T10.c, T11) wires it and tests it.
  - _The readout's class._ `dominantSlot(weights)` is the texel weights' largest, the lowest slot
    on a tie, the class a readout names; T2's
    `material_weights_at_the_finest_level_the_largest_is_the_class` holds the finest-level
    weights' largest to the classifier's class, and the tie to the worker's `material` answer,
    which T12's readout shows, is T12's (asked of main). _Ruled 2026-10-15 (main's ruling for
    R10.T10.b): T12's._
  - _The energy bound's grid_ starts at A_N 0.60: below it L(0) is 1 and A_h = A_N η rises with
    A_N, so 0.60 stands for it.
  - _Not built, as planned._ The ground's thermal emission waits on R08.T18 (decision-composition).
    The terrain's figure scale: law 0's A takes `oblateAlbedoScale` as the disc's does, the class
    laws none (T10.a solves c on the sphere's p).
  - _The meter._ A texel lit directly by a star writes `METER_CLASS.litBody`, one no star lights
    `unlitBody`, and with no star `other`, as R07's disc's pure pixels do (R05's material wrote
    `litBody` always).
  - _Tests_ (`terrainMaterial.test.ts`, `law.test.ts`, `phase.test.ts`, `draw.test.ts`,
    `templatesGolden.test.ts`, `litBodyProbe.test.ts`, `slotLayout.test.ts`, `resources.test.ts`,
    `terrainPass.test.ts`, `photorealFrame.test.ts`, `fieldsWasm.test.ts`, `worker::tests`): as the
    task lists them, the energy bound by an 8-point Gauss–Legendre quadrature in μ over 41
    geometric pieces and 180 midpoints in azimuth with each row's own f and L(α) (worst A_h 0.97,
    the Ice row at its ceiling; η(1) 0.636 and sup η 0.720 at L(0) 0.38; Lambert's η 1 to 10⁻⁶); the
    disc's near-field oracle in `draw.test.ts` takes L(α) at each element's phase; and the smoke
    harness's `R10.T10.b the lit terrain` (part 6) bakes a 3 × 3 block of level-7 patches of the
    Moon-like fixture's first survey in a height worker and draws it by both vertex paths under a
    star and a planetshine source: every texel finite, at least half the frame terrain, and lit.
    Beside the task's: an eclipse of the curved Moon on its disc and its point alike at 60°, the
    pass binding the larger phase table once the rows outgrow it, the lit layout's slot field, its
    refusal of a bake without layers, and the photometry export against the native materials and
    its refusals on the module.
  - _Acceptance, as built._ `just gen-surface` first (the worker's `photometry` export), then
    `pnpm --filter hyperion test` and `just test-render`, and `cargo test -p hyperion-surface --lib
worker` (the photometry's Rust test), which the task's two commands do not reach.
  - _Generator version._ Untouched (22). No golden moved; the photometry export presents T1.a's and
    T10.a's versioned values, and the rest is presentation.
- **Questions for "main" (R10.T10.b)** (none touches a contract; each built to the lean).
  - _"Three rows"._ The folded-in limb ruling says three rows; with L(α) = L(0) × McEwen's profile a
    bright frost needs its own, so rows are keyed by (law, L(0)). Lean: as built, and the task's
    text corrected to "three laws, a row per (law, L(0))". _Ruled 2026-10-15 (main's ruling for
    R10.T10.b): as built; the text corrected._
  - _q to 2 × 10⁻⁴._ The task's 10⁻⁴ on the Moon's q misses by the held tail past 150°. Lean: as
    built (the decision's own figure); the alternative, holding Φ rather than f past a template's
    range, changes R07's rule. _Ruled 2026-10-15: as built._
  - _The brainstorm's wording._ "Materials, and what a surface looks like" says the shader "blends
    the materials' photometric parameters"; Design note 8 and decision-r07-t8b rule that it sums
    their radiances (the parameters' blend is 1.7× too bright). Lean: a brainstorm edit to
    "radiances", for the owner. _Ruled 2026-10-15: drafted for the owner, sign-off queue item 23._
    _Signed off 2026-10-16 (`signoff-11.md`, item 23): applied to the brainstorm and the README, and
    Design note 3 corrected with them._
  - _The readout's class at the vertex._ Lean: T12's test, at a finest-level bake, that the
    largest weight's palette pair is the worker's `material` answer. _Ruled 2026-10-15: T12's._
- **Deferred findings (2026-10-13 rule) (R10.T10.b)** (recorded, not ruled; each leaves the algorithm
  and the contracts as built).
  - _Sky light on particulate ground is A, not its Bond albedo._ The diffuse sky's reflected share
    is taken as the mean A (R07's disc's rule, which the terrain matches); for a lunar-Lambert law
    the hemispherical response to diffuse light is about q k A, 0.63 A for the Regolith row at
    L(0) 1 (the science check's computation), so the sky light on particulate ground is about 1.6×
    too bright under an atmosphere, exactly right on the Lambert rows. A change goes to R07's disc
    and here together.
  - _McEwen's printed form._ McEwen 1996 prints the lunar-Lambert law without the factor 2 on its
    Lommel–Seeliger term; the coefficients are applied in the 2L form of McEwen 1991 and of
    McEwen's ISIS `LunarLambertMcEwen.cpp`, as Design note 8 and the surface crate have it (stated
    in `law.ts`); decision-r07-t8b's verification row should say so.
  - _The generator's q is the constant-L law's._ Plan 14's `photometry.rs` states q at a constant L
    while the client draws the five airless templates under McEwen's L(α), so a modelled body's q on
    the client is up to 6.8 × 10⁻⁴ (relative) below the stated one; inside every stated tolerance.
    The lever is the generator taking the same L(α) (plan 14's).
  - _Back-facing shading normals draw black._ Where a baked normal faces away from the camera on a
    facet the camera sees (grazing views of steep relief), the texel reflects nothing, as
    `body_brdf`'s μ ≤ 0 rule has it; the Lommel–Seeliger term is 2 A f L there, not 0, so the edge
    is a step. T10.e's look will say whether a floor on μ is wanted.
  - _The class laws carry no figure scale._ Law 0's A is scaled for an oblate disc, the classes' are
    not (T10.a's c is solved on the sphere's p), a difference of order f; T10.d draws both and may
    align them. _Aligned by R10.T10.e: each class's A takes the disc's factor of its L(0) ("Deviations
    in T10.e, as built")._
  - _Bilinear weights across the quad's diagonal._ The mesh splits each quad on its diagonal while
    the weights are read bilinearly over its four vertices: a different interpolant from the
    triangles', harmless for smooth fields; T14's measurements may say otherwise at class edges.
- **Deviations in T8.a, as built** (2026-10-15; lane r10-horizon, on `rendering-and-planets` at
  `298f51ca`).
  - _Files._ `crates/hyperion-surface/src/bake/horizon.rs` with `horizon/tests.rs` (18 tests) and
    one golden, `tests/golden/horizon/encoding.golden` (header 22, the fixture T8.b's TypeScript
    twin reads); `bake/mod.rs` (`pub mod horizon`, its items re-exported, `TerrainBakeOptions`'
    new field, the call); `bake/layers.rs` (`HorizonMap` moved out, two doc comments); R09's
    `synth.rs` (`SynthCache` gains the far field's memo, additively, as it holds T1.b's ice cuts);
    T6.b's `worker.rs` (the rebake rule counts the map, below); `wasm.rs` (the field set to
    `Skipped`, below); `tests/panics.rs`'s `bake::horizon` (5); `benches/bake.rs` (the
    `bake/horizon` group); the option literals and the hook test's map assertion in
    `bake/tests.rs`, and in `worker/tests.rs` a test of the rule; Provides' `bake` as-built note.
  - _The azimuths are the patch grid's._ Azimuth k looks along the grid step `AZIMUTH_STEPS[k]`,
    (1, 0), (1, 1), (0, 1) and on round, counterclockwise from +x seen from outside; a direction's
    grid azimuth, `grid_azimuth(face, at, towards)`, is atan2(b, a) of its components on S2's grid
    tangents ∂p/∂x ∝ (U − p (p · U)) du/ds and ∂p/∂y ∝ (V − p (p · V)) dv/dt, so azimuth k is at
    k × 45° exactly, whatever the grid's skew. The task names no frame; T8.b's "each patch's local
    frame" is this one. The near field then reads only the patch's vertices along its grid lines,
    with no interpolation, and sweeps each line once. On the ground the eight are 45° apart at a
    face's centre, within 12° of it over 97% of a face (15° at worst), and 30° to 60° apart at a
    cube corner, where the grid's lines meet at 120° (the science check). They turn across a face's
    edge: a frame the same on both sides of every face edge has singular points (east and north,
    at the poles), and the grid's was taken so that the near field reads its own lines.
  - _The near field is the line sweep, from the start._ Timonen and Westerholm 2010's convex-hull
    sweep, O(n) a line, rather than the march with the sweep as its fallback, since the grid's
    azimuths make the lines the patch's own and the sweep costs no more to write: each grid line is
    walked from its far end back, the hull of the vertices passed kept in the plane through the
    body's centre and the line's ends (a row or a column of S2's grid is a great circle in it; a
    diagonal, bent a little off it by S2's warp, and a spheroid's heights along ν are projected
    onto it). It equals brute force over the line's vertices to one code on the Moon- and
    Earth-like worlds' rough ground, under 50 of each world's 33,800 codes a step off (tested). The
    near field's ground is the drawn mesh: its edges along a row or a column join its vertices, so
    a line's highest point seen from a vertex is one of them. Design note 10's 2–6 ms for the march
    is not measured; `bake/horizon` measures the sweep.
  - _Exact elevations._ Each horizon is taken from the two points' body-fixed positions on R07's
    spheroid and the datum's normal ν, atan2 of the rise along ν and the run across it, so the
    curvature drop is exact rather than Design note 10's d² ÷ 2R. The cliff test holds the plan's
    arctan((H − d² ÷ 2R) ÷ d) to two half-steps (9.9″) at H ÷ R = 1.6 × 10⁻⁵, where the two
    differ by at most (H ÷ R) ÷ 2, 1.6″ (the science check), and the exact form to the code.
  - _The far march._ From each lattice point, at θₖ = θ₀ 2^(k ÷ 4), θ₀ one vertex spacing of the
    patch's level, growing by 2^¼ ≈ 1.19 (the note's 1.15–1.25), four steps to each exact doubling;
    the sample's level is the patch's own for the first twelve steps (to 2^(11/4) θ₀, 6.7 θ₀) and
    one coarser every four steps after, its vertex spacing 0.79–1.32 of the step's length from the
    thirteenth step (tested to 0.6–1.7); the samples bilinear between that level's four vertex
    heights about them, in (s, t); the point standing at its own drawn height. The patch's own
    vertex heights, and its parent level's at its even vertices, are read from the bake, the same
    bits `height_at` gives there. Design note 10 starts the march one texel beyond the patch's
    edge; it starts one vertex spacing from each point instead, so that the far field is a function
    of the point (below), and an edge point looking out reads the ground beyond the edge from the
    first vertex spacing.
  - _The reach_ is √(2 Δh ÷ c), Design note 10's √(2 R Δh) as an angle with R the datum's polar
    radius c, where an oblate datum drops least below its tangent plane (the science check), Δh the
    header's `realised_relief`, and π ÷ 2 at most for a small rough body (tested). On the plan's
    Earth, a sphere of 6,371 km with 20 km of relief, 504.8 km.
  - _The far field is a level's, not the same across levels._ The task asks it bit-identical at
    shared points "across levels"; as built it is so between adjacent patches of one level on a
    face (tested: the shared edges and corner of four patches), not across levels nor a face's
    edge. A level's march starts at its own vertex spacing, reads its own ground near, and stands
    on its own height; a march the same at every level would have to start at the finest level's
    spacing and read every level's ground about every point, tens of thousands of heights a patch
    at the 5 to 8 µs a height measured below. A march from the patch's edge, as the note puts it, would differ between
    the two patches that share a point. The lattice's points are the same bits at every level
    (`far_field_point` at (i, j) and level L is (2i, 2j) at L + 1, tested), and so are the march's
    step distances (a level's are its parent's four steps on, to the bit, tested), and, from the
    parent's ninth step on, the levels they read. A seam at a level boundary is of the near fields' own
    kind, fine ground against coarse.
  - _`far_field_point`_ names the lattice as (face, u, v). The bake takes each point's direction
    by `PatchKey::vertex_dir`'s canonical rule (within 10⁻¹⁵ rad of the lattice's on the face,
    tested), so that a point on a face edge is the same bits from either face, and its tangents
    from the exact s and t.
  - _The far field's memo._ The vertex heights the bake does not hold are kept in the worker's
    `SynthCache` by face, level and lattice vertex, for one field at a time (R09's `NetworkId`), in
    a `BTreeMap` that starts afresh at 131,072 heights (about 4 MB a body and worker, which
    `heldBytes` does not count; for T14 and R12's memory line), answered heights only; a refused
    one is asked again by the next bake. A bake whose heights were all answered adds the band of
    its own that its neighbours' marches read, its vertices within eight of its edges at its level
    and its parent's (2,600 or so), so that a neighbour baked after it asks some 750 to 950 heights
    where a cold bake asks 4,300 to 5,200 (below). A bake keeps a direct-mapped 4,096 cells' four
    heights of its own in front of the map, 1–15% off the march (the doc comment's figures).
  - _The survey and the rebake rule._ A far sample whose vertex the view cannot make takes the
    reference surface, height 0, as the bake's own heights do, and the map then says so:
    `HorizonMap::is_complete()` is false where the far field read the reference surface or the
    bake's own heights stood at it. `TerrainLayers::is_complete()` keeps its meaning (the heights,
    normals, weights and σ). T6.b's `BodyBake::needs_rebake` takes an incomplete map as an
    incomplete bake, baked again on any block (the determinism review: a patch surveyed whole
    whose far field reaches past the survey would otherwise keep a map the next block changes);
    the module's rule says so, and `worker_rebake_rule_takes_an_incomplete_horizon_as_an_incomplete_bake`
    holds it. T6.a's block test compares the bytes only where the map was complete too.
  - _`HorizonLayer`._ `TerrainBakeOptions` gains `horizon: HorizonLayer::{Baked, Skipped}`, beside
    `WeightsLayer`, so that the wireframe's depth-only bake, and a worker whose client reads no
    map, skip it; the bake makes it on both settings when asked. `bake/patch_65`'s cases skip it,
    so that T6.a's figures stay comparable; T6.a's tests bake it, so their order, cache and block
    tests cover it; T6.b's skip it, as the module does, beside the new rule test.
  - _The worker skips it._ `src/wasm.rs`'s `bake` passes `HorizonLayer::Skipped`: no client reads
    the map before T8.b, which turns it on for the lit style with its transfer.
  - _The layout_ (decision-r10-t10b-r09-t2e §5, a storage buffer): four `u32` words a texel, vertex
    by vertex, row by row, azimuth 2w in word w's low half and 2w + 1 in its high half
    (`pack2x16snorm`'s order); the near field's 65 × 65 (67,600 B) then the far field's 9 × 9
    (1,296 B), point (a, b) at vertex (8a, 8b): an `array<vec4u>`, 68,896 B a slot in `horizon-map`,
    1,296 B over Design note 15's and the brainstorm's 67.6 kB, which counted the near field
    alone. T14.a's `bytesPerPatch` takes 68,896.
  - _The encoding._ `encode_horizon` is ⌊½ + 32,767 × clamp(e ÷ 90°, −1, 1)⌋ in real arithmetic:
    the product exactly as two `f64`s (`math::two_product`), the floor decided on their sum, so that
    for any input it is WGSL's formula, tested against an integer oracle over the `f64`s and
    `f32`s either side of every code boundary and 10⁵ more. The plan's "integer only" is read as
    that exactness. `decode_horizon` is `unpack2x16snorm`'s max(c ÷ 32,767, −1) × 90°.
  - _Added to Provides:_ `HorizonLayer`, `AZIMUTHS`, `AZIMUTH_STEPS`, `FAR_SIDE`, `FAR_STRIDE`,
    `WORDS_PER_TEXEL`, `NO_HORIZON` (−32,767, −90°: no ground ahead in the patch, or no sample in
    the reach), `grid_azimuth` with `ZENITH_TOLERANCE` (10⁻¹²: no azimuth where a direction's part
    normal to the point's is below that share of it), and `HorizonMap`'s `near`, `far`,
    `elevation_at` and `is_complete`; `elevation_at(x, y, azimuth)` is the shader's lookup at a
    vertex (each field linear in azimuth, the far bilinear between its points, the larger), T8.b's
    oracle.
  - _The test worlds._ The cliff test feeds a sharp step through `near_codes` as synthetic
    `PatchHeights`, since `FieldBuilder`'s spline spreads a cliff over two coarse cells, wider than
    a patch; `flat_field` is the test module's own, `tests/common/worlds.rs` not existing (as
    T1.b's `cliff_field` is the classifier's). A `FieldBuilder` basin's wall tests the far field to
    a bracket.
  - _Tests_ (`bake::horizon::tests`, 18; `tests/panics.rs`'s `bake::horizon`, 5): the round trip
    within 4.95″ over 10⁶ elevations and both sides of every code boundary, reaching the half step;
    the decode rising with the code; the rounding exact against an integer oracle; the fixture; the
    Sun's lit fraction at 1 au moved by at most 2 δ ÷ (π ρ), 0.33%, under 1/255; a flat field's near
    horizon −θ ÷ 2 to the next vertex and never above zero, its far field empty (no relief, no
    reach); a cliff; the sweep against brute force; the march's steps and reach; a basin's wall of
    10 km at 150 km seen by the far field within the bracket a wall of H can give; the shared
    points' bits; order and cache independence (`assert_order_independent`, warm and fresh, over a
    patch, its neighbour, parent and child and the survey's edge patch); a cache warmed under a
    smaller view baking as a cold one under the grown view; the survey's completeness; the layer
    skipped; the grid azimuth k × 45° along the grid's steps on every face, and none at the zenith;
    the lookup continuous in azimuth round the turn.
  - _Acceptance, as built:_ `cargo test -p hyperion-surface --features testing bake::horizon` (18
    and 5); the bench below; the wasm targets through smart-ci.
  - _The bench_, `bake/horizon` (2026-10-15 at 18:10–18:19, on the development machine, a Ryzen 7
    3700X: the bench profile, one thread, under the heavy lock with `HEAVY_CPU_QUOTA=1200%`,
    schedutil; load 10.3, 8.6 and 11.5 at the start and 22.3, 22.0 and 17.1 at the end, so
    provisional). Each patch at the low setting with the weights, as `bake/patch_65`'s are; the
    map's cost is the bake with it less the bake without it:

    | Patch                     | Without | Warm    | Beside 8 baked | Cold, without | Cold     |
    | ------------------------- | ------- | ------- | -------------- | ------------- | -------- |
    | Earth-like, L + 4 (12)    | 59.2 ms | 67.5 ms | 70.1 ms        | 65.1 ms       | 109.6 ms |
    | Earth-like, 19            | 63.9 ms | 80.1 ms | 77.5 ms        | 68.7 ms       | 138.3 ms |
    | Moon-like rim, L + 4 (10) | 51.7 ms | 59.4 ms | 59.8 ms        | 53.1 ms       | 90.4 ms  |
    - The map: warm 8.4, 16.1 and 7.7 ms; beside its eight neighbours baked, at most 10.9, 13.5 and
      8.0 ms (the bake's own heights there partly cold too); cold 44.5, 69.6 and 37.3 ms. Against
      the plan's quarter of the patch's 40 ms, 10 ms: the L + 4 patches baked beside their
      neighbours, as a worker bakes a descent, are about at it, the finest level's 1.4 to 1.6
      times it, and a cold patch four to seven times it.
    - The lane's own timing test (release without LTO, the least of five, load 8 to 18, not
      committed) splits it: the sweep 1.9–2.0 ms against Design note 10's 2–6 ms estimate of the
      march; the far march warm 5.1 ms at L + 4 and 9.2 at the finest level (55 and 83 steps of 648
      rays; the Earth-like world's relief 13.3 km and reach 411 km); and a cold bake asking 4,275
      to 5,181 heights, 4.8 to 7.9 µs a point (the cold less the warm time over them), against
      R09.T8's provisional 14 µs and Design note 10's 2–4 µs.

  - _Generator version._ Untouched (22). The map is presentation: no golden pins it; the
    encoding's fixture carries 22 and pins only `pack2x16snorm`'s rounding.
- **Questions for "main" (R10.T8.a)** (none touches a contract of the server's; each built to the
  lean).
  - _The far field across levels and face edges._ The task's test asks bit-identical bits across
    levels, and Design note 10 says no seam can open; as built they are a level's and a face's
    (above). Lean: as built, with the task's text and the plan's Verification line narrowed to
    "adjacent patches of a level on a face". _Ruled 2026-10-15 (main's ruling for R10.T8.a): the
    lean, and T8.b blends to the parent's map by the morph factor._
  - _The azimuth frame._ The patch grid's, turning across face edges (above). Lean: as built; the
    alternative, east and north, is singular at the poles, where faces 2 and 5 are centred. _Ruled
    2026-10-15: the lean; T8.b uses `grid_azimuth`'s construction on the patch's face, through
    `preciseAtan2`._
  - _The map's rebake rule._ As built, an incomplete map is baked again on any block, which on a
    survey smaller than the far field's reach is nearly every patch near it. Lean: T8.b, when it
    turns the map on, carries the cells the far field refused with each bake, as T6.b's
    `unsurveyed_cells` are, and narrows the rule to a block that brings one. _Ruled 2026-10-15: the
    lean, with "brings" counting margin cells; T8.b builds it._
- **Deferred findings (2026-10-13 rule) (R10.T8.a)** (recorded, not ruled; each leaves the
  algorithm and the contracts as built).
  - _The bake's budget._ A cold map, 37–70 ms (above), is four to seven times the quarter of the
    patch's 40 ms; one baked beside its neighbours is about at it at L + 4 and 1.4 times it at the
    finest level. Levers: the march's step count (83 at
    an Earth's finest level), the early stop below, sharing coarse samples between the 81 points,
    and the far field of a coarser patch's map. The quiet figures and the WebAssembly ones (the
    worker runs the bake as WebAssembly) are pending; the owner's i7-8665U figures stay pending by
    hand. _Ruled deferred 2026-10-15 (main's ruling for R10.T8.a): T15's descent with the map on
    decides; the early stop is the first lever._
  - _The far memo starts afresh wholesale._ At 131,072 heights (`FAR_HEIGHTS_HELD`) `FarHeights`
    clears everything. A complete bake adds about 2,880 band heights (2,016 at its level and 864 at
    its parent's) plus the 750–950 it asks, so in a long descent each body's memo restarts after
    about 35 warm bakes, or about 17 cold ones, and the bakes after a reset run partly cold, which
    the bench's warm column does not see. The lever is a reset that keeps the coarser levels, the
    most reused, and evicts the finest first, or a larger cap; any policy is safe for determinism,
    since the map is cache-independent (tested). T15's descent figure includes the effect (main's
    ruling for R10.T8.a, 2026-10-15).
  - _Edges between far points._ An edge vertex between two far points has the far field
    interpolated from theirs, eight vertex spacings apart, so ground just beyond the edge narrower
    than that shades the neighbour's vertex there at its own resolution and this patch's only
    approximately: a seam of fine detail along patch edges at low sun, of the near fields' own
    kind. The lever is a near field that reads a margin of the neighbours' heights. After T8.b's
    blend this is the residual seam at every patch edge; T8.b's by-hand sunset looks for it.
  - _The reach and negative horizons._ Seen from a peak, ground just past the reach can stand
    higher than anything within it (−2.3° against −4.5° on the plan's Earth, the science check),
    for a sun below the peak's horizontal plane. The lever, which also stops sooner in valleys: stop
    where (h_max − h₀) ÷ d − d ÷ 2R falls below the best elevation found.
  - _The reach's small-angle form._ The reach's square root is the small-angle form of
    arccos(1 − Δh ÷ (R + h_max)); a few percent short on a small rough body.
  - _The Sun's radius in the task._ The task's ρ = 959.6″ is the older standard 959.63″ (Allen
    1976); IAU 2015 B3's nominal radius at 1 au gives 959.23″ (the Astronomical Almanac's), which
    the test takes from the crate's constants; the bound moves by 0.0001%.
  - _Limb darkening_ moves no half-light point (by symmetry), but steepens the mid-transit slope
    by about 9% for a linear law with u = 0.6, raising the 0.33% bound to 0.36%, still under
    1/255; T8.b recomputes it if it darkens the limb (the science check).
  - _Datum shadows._ Far ground the view does not hold stands at the reference surface, as it is
    drawn, so a deep surveyed valley at a survey's edge is shaded by the datum until the next block.
- **Deviations in T8.b, as built** (2026-10-16; lane r10-shadow, on `rendering-and-planets` at
  `8c052086`; decision-r10-t8a §1–3 and decision-r10-t10b-r09-t2e §5 built as ruled, except where
  stated).
  - _Files._ In the crate: `bake/horizon.rs` (`HorizonMap::refused_cells`), `worker.rs`
    (`SurveyChange::held_runs`, `brings`, `brings_any`; `BodyBake::needs_rebake` rules 1–3),
    `wasm.rs` (`bake`'s `horizon` argument, `BakedTerrain`'s map getters, `FieldBlock.newlyHeld`),
    `worker/golden.rs` and `worker/tests.rs`, `bake/horizon/tests.rs`, and the goldens
    `horizon/grid_azimuth.golden`, `horizon/blend.golden` and `worker/moon_like.golden`
    (re-blessed at 22). In the client: `view/engine/types.ts` (`StorageBufferSpec.stages`,
    `ShaderStageName`), `view/engine/webgpu/materials.ts` (`stageVisibility`,
    `storageBuffersPerStage`, `resourceLayoutEntries`), `view/engine/platform.ts` (the
    `requiredLimits` rule); in `view/terrain/shadows/`, `horizon.wgsl`, `gridAzimuth.ts`,
    `horizonLookup.ts` and `litFraction.ts` with their tests; `shaders/{terrainPatch,terrainLit}.wgsl`,
    `terrainMaterial.ts`, `gpu/{material,uniforms,resources}.ts`, `slotLayout.ts`, `terrainPass.ts`,
    `workers/{messages,heightBake,rebake,pool}.ts`; R07's `view/shaders/smoothMesh.wgsl` (its
    instance struct follows the record's new words, unread); the smoke harness's `litTerrain.ts`;
    and `src/test/wgslStages.ts`. `cascades.ts` is T9's. R07's `litBody.wgsl` is unchanged: the
    terrain's call is in `terrainLit.wgsl` (T10.b's file), not at "the call sites in R07's
    `shaders/litBody.wgsl`" the task names.
  - _The storage visibility split is built here_ (decision-r10-t10b-r09-t2e §5; T10.c has not
    landed). `StorageBufferSpec` gains `stages`, both by default; `resourceLayoutEntries` writes each
    storage entry's own visibility and refuses one visible to no stage. Both terrain materials
    declare their stages, R05's spike shading too (its five are the vertex stage's, its fragment
    reading the normals texture alone), so that "declared stages match the entry points" holds for
    every terrain material: the lit material's vertex stage reads 5 (4 on `face-differences`) and
    its fragment stage 3, the class layers, the lights and the horizon map. The stage match is a
    static walk of the composed WGSL's call graphs from `vertexMain` and `fragmentMain`
    (`src/test/wgslStages.ts`), since the machine has no naga and no `compose.py`; Tint validates the
    layout against the shaders in `just test-render`. The per-stage counts and the match are in
    `terrainPass.test.ts` over all four terrain materials; `resources.test.ts` holds the horizon
    field's buffer and writes.
  - _The horizon enters R07's horizon term in the element's frame._ The ruling passes the blended
    horizon e to `sphere_irradiance(h, phi, horizon)`. R07's `horizon` is an elevation above the
    element's own tangent plane towards the star (its oracle's cone about the normal), and the map's
    is against the datum's horizontal, so passed raw it would light a slope facing the Sun under a
    Sun below the horizon by up to the slope (tested: a 20° slope, the Sun 1° below a 5° horizon,
    lit at 0.41 of face-on by the raw horizon). `terrain_horizon` passes
    e − (e★ − (π ÷ 2 − φ)), e★ the star's elevation above the datum and φ its angle from the
    terrain's normal: the disc is then cut exactly where its centre stands at e above the datum,
    and by the element's own plane where that hides more (R07 takes a negative horizon as the
    plane). On the smooth figure (n = ν) it is e itself, and with e = 0 the term is R07's smooth
    value bit for bit (tested on the twin). The conversion is one function, `element_horizon`
    (`elementHorizonRad` in the twin), so that a ruling can change it in one place. The science
    check confirmed it: the cut stands at x = (e★ − e) ÷ ρ for any slope, since a small disc's
    visible share depends on the cut's distance from its centre alone, and an independent integral
    over the disc above the datum's horizon and the slope's plane agrees to 7.3 × 10⁻⁴ of cos φ on a
    slope tilted across the Sun's bearing (tested to 10⁻³); over horizons of 0.5° to 60°, slopes to
    45° facing towards, across and away from the Sun, and the Sun within ±1.5ρ of the horizon, the
    term is held within 1.5 × 10⁻³ of face-on of that integral, taken exact in azimuth ring by ring
    (decision-r10-t8b §5 L1; the ruling measured 1.34 × 10⁻³ at worst).
  - _The lit fraction is R07's._ Design note 10's lit fraction is R07's `disc_visible_fraction`
    inside `sphere_irradiance`, which is the same formula with x measured from the cut, so the
    shader takes no second factor (one would darken the half-light point to a quarter);
    `terrain_shadow_factor` stays 1, T9's cascades' and R11's clouds'. `shadows/litFraction.ts` is
    the formula's twin and `terrainHorizonFactor` the shader's composition. The plan's "the lit
    fraction's arccos x is `preciseAtan2(√(1 − x²), x)`" is therefore not built: R07's function,
    used unchanged as the task says, keeps WGSL's `acos` and `asin` (a deferred finding below). The
    star's elevation and grid azimuth do go through `preciseAtan2`.
  - _No hard step under 20″_ (main's ruling, 2026-10-16: accepted as built). Design note 10's hard
    step for stars under 20″ is not built, because R07's soft edge converges to it and the
    quantisation dominates there: under 20″ the edge is 2ρ, under four of the map's 9.9″ steps, a
    band of centimetres on the ground, and the quantisation moves the lit fraction by up to
    2 δ ÷ (π ρ), 16% at 20″, no more visible than a hard step's edge moving by δ. The condition's
    test: at ρ = 0.1″ and at ρ = 0 the lit fraction and the term are finite, in [0, 1], and the hard
    step away from the edge. At ρ = 0, H infinite, R07's term alone is ∞ × 0, NaN, so the shadow
    code takes H at most 10¹⁸ (`terrain_star_distance_radii`, `MAX_STAR_DISTANCE_RADII`; ρ of
    10⁻¹⁸ rad, H² finite in `f32`), R07 unchanged, and `litFraction` takes ρ = 0 as the step.
  - _The grid azimuth at a fragment._ The vertex stage passes each vertex's cell coordinates (s, t)
    from its slot record (s₀ + (x − 32) step, linear across a quad, a skirt's its edge vertex's) and
    its face (from the record's centre axis); the fragment forms p and the tangents on the patch's
    own face from them, never from its `f32` position. The zenith guard is |t|² ≤ 10⁻¹² |s|², about
    0.2″ from the zenith, where the azimuth is 0; `preciseAtan2`'s half-plane form with the sign of b
    gives [0, 2π] (2π, at a rounding, is azimuth 0's knot). The guard is f32's resolution
    (decision-r10-t8a §2), not the crate's and the twin's 10⁻¹² of the length, which are `f64`'s.
    `gridAzimuth.ts` is the crate's `grid_azimuth` in `f64`, held to its fixture.
  - _The blend_ (decision-r10-t8a §1), as specified: the instance record's spare words are
    `parentSlot` and `parentQuadrant` (bit 0 i & 1, bit 1 j & 1), `NO_PARENT_SLOT` 0xffffffff for a
    root, a parent not resident (`PatchCache.residentOver`), or one whose slot holds no map
    (`TerrainResources.holdsHorizon`); the morph factor rides to the fragment as `PatchVertex.morph`,
    and the parent is read at ((q_x 64 + x) ÷ 2, (q_y 64 + y) ÷ 2) only where k > 0, by WGSL's
    `mix`. `INSTANCE_RECORD_BYTES` stays 32, and R05's spike shading writes no parent. The fixture is
    a Moon-like rim patch at L + 3 and its parent baked within the reach: both far fields whole, the
    near fields at every eighth vertex of the child and four odd ones, and `elevation_at` at five
    azimuths (63.8 kB).
  - _The slot field._ `horizon-map` takes 68,896 B on a lit layout alone, `HORIZON_MAP_BYTES`; R05's
    spike layout keeps 0, so the descent spike keeps its slots. A lit slot whose bake has no map
    is written words of no horizon (0x80018001, −90° in every azimuth) and holds no map to blend to.
    The low lit slot is 154,708 B, 433 slots in 64 MiB (782 before), against Design note 15's rule of
    430; the high lit slot 306,796 B, 1,303 slots in 400 MB, whose offsets now fit WebGPU's default
    128 MiB binding (132.1 MB) where before the horizon map they did not.
  - _The worker._ `bake` takes the horizon layer as its own argument, `JsHorizonLayer`, rather than
    tying it to the weights inside the module, so that the module's arguments stay explicit; the
    client passes the weights' setting for both (`WASM_HORIZON_LAYER[source.weights]`): the lit
    style bakes the map, the wireframe skips both.
  - _`SurveyChange::is_empty` is renamed `surveys_nothing`_ (the review): a block that brings only
    margin surveys nothing yet may take a bake by rule 3, so a name that read as "changed nothing"
    was a trap; its callers were tests alone.
  - _The rebake rule_ (decision-r10-t8a §3), as ruled: the far field's refused cells are recorded
    where `FarGround` names a refused vertex's `NotSurveyed` cell; a block's change carries its
    newly held runs, every cell of its cover the field did not hold before it; rules 1–3 in
    `needs_rebake` and `needsRebake`; in flight, `staleInFlight` takes a bake as stale unless it is
    complete, its map complete and its mask whole. The worker golden gains each block's `held`
    runs and two lit bakes' `horizon` records (the words' digests, completeness and refused cells),
    which `fieldsWasm.test.ts` holds the module to. There, with the maps on, the second survey
    takes the patch surveyed whole by the third case alone (it brings cells its far field was
    refused for), and a block sent again, which brings nothing, passes over every complete bake,
    which bakes the same bytes, its map's words and refused cells too; the margin and surveyed
    cases are held in the crate, whose worker tests run
    on the browser target too, and on `needsRebake`, not on the module through blocks of its own.
    T8.a's `worker_rebake_rule_takes_an_incomplete_horizon_as_an_incomplete_bake` goes with the rule
    it held; rules 1–3 are held by `worker_rebake_rule_passes_over_a_block_elsewhere`,
    `…_takes_a_block_that_brings_a_refused_cell_as_margin`, `…_takes_a_block_that_surveys_a_refused_cell`
    and `…_misses_no_bake_the_second_survey_changes_with_the_map`.
  - _The smoke run._ The lit check also bakes the 3 × 3 block's four parents into the slots after
    it and draws the block at a morph of about ¼ to ½ (0 to 120 km), so that SwiftShader runs the
    blend's parent reads as well as the lookup.
  - _Tests._ Crate (`bake::horizon`, 21, three new; `worker::`, 26 with the golden's two, five new
    and T8.a's rule test replaced): the refused cells named, rising, not held,
    within the reach, never named again once held, the same from a cache warmed under the smaller
    view, none within the reach's view; the fixtures;
    the held runs exactly the block's cells not held; a block elsewhere passed over, the bakes the
    same bytes; a refused cell brought as margin alone taken; one surveyed taken; the second survey
    with the maps on, the passed-over bakes the same bytes. Client: `shadows/` (the lit fraction by
    hand; the term bit for bit on the smooth figure; against R07's oracle in the element's frame
    to 10⁻⁶ wholly up or down and to 2 × 10⁻³ cos φ + 10⁻⁶ across the cut, a consistency check of
    R07's own function kept as built (decision-r10-t8b §1); the cut at e★ = e_h on four slopes, the
    half-light point to ±0.05; against an independent integral within 1.5 × 10⁻³ of face-on over the
    cases of a grid of 324 (horizon, slope, bearing, elevation) whose slope's plane does not hide
    the whole disc; `gridAzimuth` against its fixture to 10⁻⁵ rad; the encoding against its fixture;
    the lookup against `elevation_at` to 10⁻¹² rad; the blend at k = 0, at k = 1 on the even
    vertices, continuous in k, and with no parent); `rebake.test.ts`; `pool.test.ts`;
    `uniforms.test.ts`; `slotLayout.test.ts`; `resources.test.ts`; `terrainPass.test.ts`;
    `terrainMaterial.test.ts`; `materials.test.ts`; `fieldsWasm.test.ts`.
  - _Acceptance, as built:_
    `cargo test -p hyperion-surface --features testing -- bake::horizon worker::`;
    `pnpm --filter hyperion exec vitest run src/renderer/src/view/terrain src/renderer/src/view/engine/webgpu`
    (the task's `…/terrain/shadows` is a part of it); the
    smoke run (`just test-render`, with and without f16); the wasm targets through smart-ci, whose
    gate log the lane's report names.
  - _By hand, pending._ The sunset over a generated range on the low setting, its three places
    (a coarse–fine edge, a same-level edge, a face edge near a cube corner) and the pass's GPU time
    with the blend, on the development machine and the owner's UHD 620: no view yet draws a
    generated world's lit terrain under a moving sun to run it in (T15's descent), so neither could
    be captured hidden here. The eight azimuths' missing peaks wait on the same run.
  - _Limb darkening_ (T8.a's deferred item): R07's term is a uniform disc's, so the 0.33% bound
    stands and there is nothing to recompute.
  - _Generator version._ 22, untouched: the map and its fixtures are presentation, and the worker
    golden's new `held` and `horizon` lines are; its payload bytes are unchanged.
- **Questions for "main" (R10.T8.b)** (each built to the lean and ruled: the first three in
  decision-r10-t8b, the fourth by main).
  - _The horizon in the element's frame._ Passing the map's e to R07's term unconverted would
    light sun-facing slopes under a set Sun. Lean: as built, e − (e★ − (π ÷ 2 − φ)), which is e on
    flat ground. _Ruled 2026-10-16 (decision-r10-t8b §1): the lean, exact in the disc's share; the
    irradiance errs by at most (2 ÷ 3π) ρ sin φ (1 − cos β) of face-on, β the angle at the star
    between the great circles to n and ν, measured at most 1.3 × 10⁻³._
  - _One lit fraction, R07's._ The plan names both R07's horizon argument and Design note 10's lit
    fraction; they are one formula, so the shader applies it once, in R07's function, and the
    plan's `preciseAtan2` for its arccos is not built. Lean: as built; T9, which combines the map
    with its cascades by minimum, takes the map's share at R07's binding cut,
    `disc_visible_fraction(h, φ + max(η, 0))`, and multiplies the term by min(1, c ÷ m), 1 where
    m = 0. _Ruled 2026-10-16 (decision-r10-t8b §2): the lean, R07's trigonometry deferred._
  - _The horizon layer as an argument._ Lean: as built, the client deciding it with the weights.
    _Ruled 2026-10-16 (§3): as built._
  - _The hard step under 20″_ (above). Lean: as built, R07's soft edge at every size, its edge
    under four of the map's steps there. _Ruled 2026-10-16 by main: accepted as built, with the
    test at ρ = 0.1″ and ρ = 0 (built), and the point star's NaN guarded in the shadow code._
- **Deferred findings (2026-10-13 rule) (R10.T8.b)** (recorded, not ruled; each leaves the
  algorithm and the contracts as built).
  - _R07's horizon term in WGSL's trigonometry._ Under T8.b's conversion φ cancels from the cut,
    and the arccos of the visible share costs at most 2.2 × 10⁻⁵ at WGSL's 6.77 × 10⁻⁵, and ρ =
    asin(1/H) at most 0.47% of the share at asin's 6.81 × 10⁻⁵ (2 × 10⁻¹⁰ on Mesa). The exposure is
    Howell's partial branch, whose terms (h² ÷ π) arctan z and x sin φ √(1 − y²) ÷ π cancel to
    1/68 of themselves at 1 au (H ÷ π), so `atan`'s relative error is amplified 68-fold. Mesa's
    `nir_atan` (ANV, the UHD 620's Linux driver; RADV; lavapipe; leading coefficient 0.99997931)
    takes 1.42 × 10⁻³ of face-on off: R07's smooth terminator at 90° reads −4.4 × 10⁻⁴ against
    9.87 × 10⁻⁴, and a flat element's shadow edge errs by 8.1%, 1.6% and 0.25% of its light at
    horizons of 1°, 5° and 30°. At `atan`'s 4,096 ULP it is 3.3 × 10⁻²; correctly rounded f32,
    1.2 × 10⁻⁵. SwiftShader is accurate here. The lever (R07's): h² atan z − x sin φ √(1 − y²) =
    z + h² (atan z − z), atan z − z by its series, on `preciseTrig.wgsl`, and the η = 0 return
    clamped at 0; before the owner's UHD 620 sunset (decision-r10-t8b §2).
  - _R07's horizon term is NaN for a point star._ `sphere_irradiance` and `sphereIrradianceFactor`
    form H² F with H infinite as ∞ × 0; R07's own disc shading takes no guard (T8.b's shadow code
    does, above). The lever is R07's, with its trigonometry.
  - _Planetshine is not shadowed by the terrain._ A neighbour's light reaches a crater floor behind
    its rim; the map would shadow it with one more lookup a source.
  - _The high setting's fallback holds the horizon field at its limit._ On `face-differences` the
    field is 1,947 × 68,896 = 134,140,512 B, 77 kB under WebGPU's default 128 MiB storage binding;
    for T14's budgets.
  - _"Same bytes" by `PartialEq`._ The rule tests compare bakes by the derived equality, as T6.b's
    do, which takes +0.0 and −0.0 as one; the map's words are `u32` and exact (the determinism
    review).
  - _The low setting's cache._ 433 lit slots in 64 MiB against the rule's 430, as Risks' "The low
    setting's cache is at its edge" has it; T14.
- **Deviations in T10.c, as built** (2026-10-16; lane r10-shader, on `rendering-and-planets` at
  `5da24214`).
  - _Files._ In `view/terrain/`: `shaders/terrainLit.wgsl` (the discard, the skirts' and fins'
    heights, their shading), `terrainMaterial.ts` (`surveyMask` at binding 17), `slotLayout.ts`
    (`survey-mask`, `SURVEY_MASK_BYTES`, `SURVEY_CELL_CODE_BYTE`), `gpu/resources.ts`
    (`litPatchMeshData`, `LIT_VERTEX_CODE`, `SURVEY_FIN_LINES`, `LIT_PATCH_INDICES`,
    `surveyCellCode`, `packSurveyMask`, `surveyedAt`; the field's upload; the lit layout's mesh and
    index count), `terrainPass.ts` (binds the field), `coverage.ts` (`surveyEdges`, `SurveyEdge`,
    `CoverageMask.survey`), a new `surveyEdge.ts` (the mark), and `workers/{messages,heightBake}.ts`
    (`TerrainLayers.fieldLevel`). R07's `shaders/smoothMesh.wgsl` and `bodies/draw.ts`
    (`SMOOTH_MESH_MATERIAL`'s `coverageMask` at binding 13 and `coverageAxes`,
    `LitBodyInput.surveyCoverage`, `MeshBodyPlan.coverage`, `MeshBodyCoverage`); `bodies/smoothMesh.ts`
    is unchanged, the discard being its shader's. R02's `view/wireframe/drawList.ts`
    (`DrawOptions.surveyEdges`, `WireframeDrawList.surveyEdges`, `DrawnSurveyEdge`). The smoke
    harness's new `smoke/surveyEdge.ts` in `page.ts`'s part 6, and `smoke/litTerrain.ts` (binds the
    field; shares its fixture). `view/engine/types.ts`, `webgpu/materials.ts` and `platform.ts` are
    unchanged (below).
  - _The storage split was T8.b's._ `StorageBufferSpec.stages`, `stageVisibility`,
    `storageBuffersPerStage`, `resourceLayoutEntries`' per-entry visibility, the `requiredLimits` doc
    (no binding-count limit raised), `src/test/wgslStages.ts` and the counts and stage match in
    `terrainPass.test.ts` are T8.b's ("Deviations in T8.b, as built"), checked against this task's
    text and kept. This task adds the `survey-mask` field's declaration and the counts with it: the
    lit material's vertex stage reads 6 (5 on `face-differences`) and its fragment stage 4; R05's
    spike shading 5 (4) and none.
  - _The mask binds to both stages._ The fragment stage discards by it; the vertex stage reads its
    cell code alone, to stand the fins on the coarse cells' edges and fold the rest flat, which would
    otherwise raster six fins of the patch's full relief in every patch, all discarded.
  - _The discard is the coarse cell's, not the mask's half._ Design note 12's half of the
    interpolated mask stands half a quad from the cell's edge: a vertex on a cell's edge holds either
    cell's state, as the rounding of its direction falls (`PatchKey::containing`), so the half lies
    inside the surveyed cell (neither surface draws) or inside the unsurveyed one (both draw), and
    the task's smoke check fails either way. As built, a fragment reads its coarse cell's state at
    that cell's centre vertex within the patch (`survey_surveyed`, its twin `surveyedAt`), on none
    of the cell's edges, from the cell's size in quads, 2^(P + 6 − L) for a patch of level P over a
    field of level L, carried as a code in the field's spare byte 4,225 (`surveyCellCode`, log₂
    clamped to 0–6). So the terrain's edge is the cell's exactly wherever a cell spans two quads or
    more, a patch at most five levels coarser than the field; coarser still (a sixth of a face or
    more), the half of the mask between the quad's vertices stands in, as Design note 12 has it. On
    the generated world (`surveyEdge.wasm.test.ts`), over the 14 patches of levels 2 to 7 baked
    across the Moon-like fixture's first survey (a 15th, the face's root, codes 0), every quad's
    centre and the points by two of its corners read the coverage's cell exactly.
  - _Each bake carries its field's level._ `TerrainLayers.fieldLevel`, the level its mask's and
    `unsurveyedCells`' cells are of, which the worker keeps from each block's answer
    (`WorkerFields`), so that the upload can code the cell's size. A client message alone; the
    module, its golden and the wire are unchanged.
  - _The skirts and fins_ are the lit material's own mesh (`litPatchMeshData`; R05's spike and R07's
    figure keep the shared one): R05's edge skirts as each edge's foot strip, its foot at
    min(h − skirt depth, 0), still R05's crack filler and now reaching the datum from ground above
    it; a head strip over each edge vertex up to max(h, 0), rising to the datum from ground below it;
    and fins along columns and rows 16, 32 and 48, from the ground to the datum on a coarse cell's
    edge, folded flat elsewhere. The side that stands higher draws each: a surveyed cell its foot
    strips, an unsurveyed cell its head strips, a fin where its line's two cells differ; a strip
    between cells of one state lies beneath their surfaces. So no skirt needs its neighbour patch's
    survey, which a vertex on the edge cannot tell. Each triangle's first vertex names its part, read
    flat (`kind`). They are drawn in the reference surface's appearance: law 0 alone under the
    datum's normal, |μ| so either face is lit, without decoration or the horizon map, as R07's disc
    draws the reference surface. A crack between levels, which R05 closes at morph 1, now shows law
    0's colour rather than the edge vertex's.
  - _The reference surface's inverse mask._ `smoothMesh.wgsl` discards a pixel whose ray (the
    disc's `hit_spheroid` at the pixel's centre) meets the spheroid in a cell the GPU's coverage
    mask holds surveyed, its direction taken into the body's own body-fixed axes (`coverageAxes`,
    the rotation's: the figure's patches are about its pole, not the body's faces) and read as the
    disc reads a class map (`class_map_texel`). `LitBodyInput.surveyCoverage` gives the mask, which
    is ignored without a `rotation`; a body without one binds a mask of no surveyed cell (one texel
    a face), and draws as before.
  - _The GPU mask follows the workers' blocks_ (main's ruling on R10.T6.b's question):
    `CoverageMask.survey(change)` adds a block's `newlySurveyed` runs as the pool's
    `onFieldChange` reports them, and `update(coverage)` now adds the store's cells to the cells it
    holds, rather than replacing them, so that an answer behind the workers unsurveys nothing; a
    release, or a field of another level, starts it afresh.
  - _`surveyEdges(coverage, face)`._ Provides' `surveyEdges(face, level)`: the level is the
    coverage's own. Each piece (`SurveyEdge`) is a side of a surveyed cell whose neighbour across it,
    on the same face or across a face's edge (`edgeNeighbour`), is not surveyed, margin cells
    unsurveyed; each listed once, by its surveyed cell's face.
  - _The mark_ (`surveyEdgeMark`). The pieces are joined through their corners into runs, the
    surveyed cell on the left seen from outside the body; each side is drawn on the spheroid in
    pieces of at most 8 px on the view, at the rim of the step, max(drawn height, 0) (the view's
    `heightAtM`; the datum where it holds none), as view-space segments, so that a depth test hides
    it behind nearer ground, and not beyond the spheroid's horizon or behind the camera; a tick 0.25
    rem long every 1 rem of each drawn run, the first half a rem in, at right angles on the view,
    towards the unsurveyed side, its far end at its foot's depth, none within 0.5 rem of
    `options.keepClear` (the line-of-sight mark's ticks); and the range to its nearest drawn point
    inside the view, from the own ship (`options.rangeFromM`) or the camera.
    `buildWireframeDrawList` draws it from `DrawOptions.surveyEdges` in both styles, the overlay
    casing the wireframe list's lines: a batch a body, `survey-edge:<body>`, `--text` at 1 px with
    the lines' casing, never dashed, its nearest range in `WireframeDrawList.surveyEdges`.
    `surveyEdgeNote` writes the list's note, `SURVEY EDGE 4.21 km`, with `FROM CAMERA` where there is
    no own ship (`rangeText`'s form).
  - _Not wired into a view._ No view draws a body's terrain yet. Its terrain pass, the store and the
    pool's `onFieldChange` feeding a `CoverageMask`, `litBodiesOf`'s `surveyCoverage` and the draw
    list's `surveyEdges` with heights from the resident bakes wait for the first view that draws it,
    R10.T10.e's hand-over or T11's wireframe; the list's note and the tripod's give-way
    (`keepClear`) are T12's, whose text lists the note. The edge hidden by nearer terrain needs that
    terrain's depth in the symbology's pass: T11's depth-only terrain in the wireframe; over the
    image, the overlay's depth holds none yet.
  - _Tests_ (`slotLayout.test.ts`, `gpu/resources.test.ts`, `terrainMaterial.test.ts`,
    `terrainPass.test.ts`, `coverage.test.ts`, `surveyEdge.test.ts`, `surveyEdge.wasm.test.ts`,
    `draw.test.ts`, `drawList.test.ts`, `submit.test.ts`): as the task lists them, `surveyEdges` on
    a fixture coverage of level 5 (an L of three cells beside a margin cell, a corner cell alone,
    a cell on a face's edge whose neighbour across it is surveyed) exactly the sides listed by hand,
    each once over the six faces; and in `gpu/resources.test.ts` and `terrainPass.test.ts` every
    terrain material on both paths, lit and spike, at most 8 storage buffers a stage by its declared
    stages, each binding's stages those of the WGSL walk, and (in `gpu/resources.test.ts`) each
    storage field its layout makes bound by its material. Beside them: the lit mesh's parts,
    windings and first vertices; the mask's packing, code and refusals; the centre vertex read
    against edge vertices that hold the neighbour's state; the discard against the coverage on the
    generated world; the GPU mask following blocks and unsurveying nothing on a lagging answer; the
    mark's closed line, tick count, length and side, its rim's height, the horizon, its ranges, its
    give-way and its note; the mesh draw's mask and axes, and the default.
  - _The smoke run_, `R10.T10.c the survey edge` (part 6): a 4 × 4 block of level-7 patches across
    the first survey's +i edge, two columns surveyed and two not, seen from 30 km over the survey's
    side about 60° below the horizon, the camera pitched so that the edge on the datum falls between
    two rows of pixels (`poseOver`: the reference surface finds a pixel's cell from an `f32` hit
    some tenths of a metre off, so that a pixel's centre within that of the edge could go to both
    or neither). With their heights, the reference surface then the terrain into one target: every
    texel finite and every pixel drawn, the skirts closing the step to the datum (the terrain 150 to
    830 m below it at the edge). On the datum, each surface alone: no pixel drawn by both. Run with
    `just test-render --part=6` (2026-10-16, both variants pass, 4.0 s and 6.0 s): with relief,
    finite and no pixel undrawn of 3,072; on the datum, the terrain 1,147 pixels, the reference
    surface 1,921, both 0; and T10.b's lit terrain unchanged (53.9% terrain, 1,655 lit pixels), its
    block inside the survey.
  - _Acceptance, as built:_
    `pnpm --filter hyperion exec vitest run src/renderer/src/view/terrain src/renderer/src/view/engine`
    (65 files, 841 tests), with `src/renderer/src/view/bodies`, `view/wireframe`, `smoke/` and
    `view/atmosphere/surfaceLighting.test.ts`, whose files it changes too (90 files, 2,419 tests);
    and `just test-render --part=6`, the part that holds both lit-terrain groups (part 1's
    catalogue and R07.T9's mesh bodies, whose material this task changes, are the gate's).
  - _By hand, run here._ The skirt heights seen at the edge on a flattened generated world: on the
    Moon-like fixture (flattening 0.0012), along the five sides of four surveyed patches of levels 6
    and 7 that meet unsurveyed cells, the skirts span 0 to 831 m (the terrain 831 m below the datum
    at most and 476 m above it), within each patch's own relief, 766 to 1,339 m
    (`surveyEdge.wasm.test.ts` holds each vertex's span to its patch's): the two surfaces share the
    spheroid's datum. The edge's look by eye, on the development machine and the owner's UHD 620,
    stays pending by hand until a view draws a generated world's terrain (T10.e, T15).
  - _Generator version._ Untouched (22): presentation alone, no golden moved.
  - _Drafted for the owner_ (the batch review's UX findings, 2026-10-19, for the sign-off queue;
    nothing new built to them, the note's text as built). Each for T12, which lists the note:
    (a) with no own ship the note reads `SURVEY EDGE 4.21 km FROM CAMERA`, where the guide shows
    only the own ship's form (`SURVEY EDGE 4.21 km`); (b) short ranges read `SURVEY EDGE 0.412 km`,
    and below 10 m `SURVEY EDGE 5.00E-3 km` (the list's range form, `lib/format.ts`'
    `formatBodyDistance`), while the readout's `SLANT RANGE` turns to m below 1 km; (c) the note is
    one a body, so two bodies' edges in view give two identical `SURVEY EDGE …` notes: name the
    body, or keep the nearest alone?; (d) the note's accessible name, which the guide gives for
    the line-of-sight note and not for `SURVEY EDGE`.
  - _The batch review (2026-10-19)._ The mark clips its pieces rather than dropping them
    (`surveyEdgeMark`): at the spheroid's horizon by bisection along the side to the crossing, at
    the near plane on the piece's straight view-space segment by its view depth, a piece in the
    view and longer than 8 px halved (at most 20 times), so that the line runs on to the view's
    edge beneath a ship low over its survey; the nearest range is the least over the drawn
    segments as clipped to the view's four side planes, where a segment leaves the view included,
    not over the vertices inside it; and the ticks are placed perspective-correctly along a
    segment, only within a tick's length of the view (tested: a camera 100 m over an edge looking
    along it 15° down, the line crossing the view's bottom edge and ending on the horizon, its range
    a centimetre-sampled minimum's to 5 × 10⁻⁴). The draw list builds the occluder sphere of a
    body whose survey edge it draws from its polar radius less the margin
    (`occluderRadius(min(a, c), d)`), so that the edge on its spheroid, up to a − c inside the
    equatorial sphere (21 km on Earth), passes the depth test: the one depth buffer every batch
    tests against, filled by one instanced draw of the spheres before any line, cannot leave one
    sphere out of one batch's test without a pass a surveyed body (and an order that fails with
    two in view), and the edge's own horizon cull already hides what of it lies behind its body
    (tested: Earth's figure, a survey at 47°, every point of the edge's batch in front of every
    occluder; the cost is deferred below). `CoverageMask.restore()` makes the texture again from the
    cells held, for the device loss its documentation names (`release()` now forgets them alone),
    and `survey` writes nothing for a block that surveys nothing once a texture of its level stands.
    `LitBodyInput.surveyCoverage` is a `MeshBodyCoverage`, the mask with the axes it is read in, and
    `MeshBodyPlan.coverage` is in its `role` (T10.e's entry). The moon_like.golden parser is one
    module, `src/test/moonLikeFixture.ts` (`moonLikeFixture`, `bytesOf`), shared by the two smoke
    checks and the two wasm tests, which keep their own module loading; `smoke/litTerrain.ts`'
    `MOON_LIKE` carries its TSDoc and is read-only. `smoothMesh.wgsl`'s comment names
    `class_map_texel` as the coverage mask's lookup, with no gutter. Tests split to one behaviour
    an `it` (`coverage.test.ts`, `drawList.test.ts`, `draw.test.ts`' mask test), and
    `terrainMaterial.test.ts` holds the WGSL's `SURVEY_MASK_WORDS`, `SURVEY_CELL_CODE_BYTE` and
    each `LIT_*` code to `slotLayout.ts` and `gpu/resources.ts`' `LIT_VERTEX_CODE`.
- **Questions for "main" (R10.T10.c)** (none touches the server's contracts; each built to its
  lean).
  - _The edge over the image, and the note's points, for R10.T16 and T12_ (the batch review,
    2026-10-19; nothing passes survey edges to a view yet). The photorealistic overlay's depth
    holds only the occluder spheres and the hulls' faces, so before survey edges reach it the
    overlay needs the terrain's depth, or nearer ground will not hide the edge over the image (nor,
    since the review, lines behind a surveyed body's equatorial limb; deferred below); and the note,
    and whether it stands, must count only the edge points the operator can see: a CPU march
    against `heightAtM`, or the depth-tested result. Lean: R10.T16's, as it wires the edges to the
    views.
  - _The discard's rule._ Design note 12's "interpolated mask below one half" draws both surfaces,
    or neither, along a half-quad strip at each coarse cell's edge; as built, the fragment's cell at
    its centre vertex (above), the half only where a cell spans one quad or less. Lean: as built,
    Design note 12 amended to "discards fragments outside a surveyed coarse cell, read from the
    survey mask at the cell's centre vertex".
  - _The field's code byte and stages._ The `survey-mask` field's 4,228 B carry the cell code in
    the last word's spare byte, and the field binds to both stages, where decision-r10-t10b-r09-t2e
    §5 foresaw one. Lean: as built; T11's wireframe reads the same field and code.
- **Deferred findings (2026-10-13 rule) (R10.T10.c)** (recorded, not ruled; each leaves the
  algorithm and the contracts as built).
  - _A hole while a rebake is due._ The GPU mask takes a block's cells at once and a patch's mask
    takes them at its rebake, so for a rebake's latency (a bake, 40 to 140 ms) a newly surveyed cell
    is discarded by both surfaces along the survey's advancing edge. The lever: the terrain's
    discard reading the GPU mask too, or the GPU mask taking a block's cells once its patches'
    rebakes land.
  - _Fins at every sixteenth line._ Patches three to five levels coarser than the field (cells of
    8, 4 or 2 quads) have no fin on a coarse cell's edge that lies off lines 16, 32 and 48; such
    patches are drawn from a distance at which a step of a kilometre subtends a pixel or so. The
    lever: a fin every eighth line (half again the fins' vertices), or a fin draw of its own.
  - _The reference surface's edge in `f32`._ The reference surface's cell comes from an `f32` hit on
    the analytic spheroid, a few tenths of a metre to a metre off at a body's radius, the terrain's
    from its patch's coordinates, exact: across that width a pixel may be drawn by both (the depth
    test picks one) or by neither (the surveyed side's foot strip shows below). Seen only with metre
    pixels at the edge. The lever: the reference surface's cell from patch-relative coordinates.
  - _The limb draw keeps no mask._ A mesh body's limb, the disc's second draw, is drawn whole, over
    any surveyed terrain at the limb. The lever: the same discard in `bodyDiscDraw.wgsl` for a mesh
    body's limb.
  - _A skirt seen edge-on_ is shaded under the datum's normal at |μ|, which is 0 exactly edge-on,
    where it reflects nothing: a line one pixel wide at most.
  - _The cache's slots._ The `survey-mask` field takes 4,228 B a lit slot: 158,936 B, 422 slots in
    64 MiB on the low setting, against Design note 15's rule of 430; 311,024 B, 1,286 slots in
    400 MB on the high one. T14.
  - _The mark sits at the ground's own depth_ (the batch review, 2026-10-19). Once the terrain's
    depth is in the symbology's pass, a tick towards the camera sinks into the nearer ground, and
    the line on the step's rim z-fights with the higher surface. The lever: the ticks' ends on the
    ground, or the occluders' slope push for the terrain. For T11's GPU smoke.
  - _Datum-deep skirts._ Every surveyed patch's foot strips hang from its ground to the datum on
    all four edges, under its neighbours' surfaces, in a shader that discards, so hidden fragments
    may be shaded before the depth test rejects them; the overdraw grows with the ground's height
    above the datum, unmeasured. The lever: R05's skirt depth where no survey edge meets the patch
    edge. T14.
  - _A surveyed body's occluder at its polar radius_ (the batch review's choice, above). Between
    its sphere of c less the margin and its equatorial limb, a band up to a − c deep at the
    equator (21 km on Earth, some 15 px at the limb seen from 400 km up at 1080p across 60°),
    lines behind the body
    (another body's graticule, an orbit, a hull) are not hidden by the sphere. In the wireframe the
    body's terrain, which T11 draws to depth wherever the list draws its edge, covers the band;
    over the image the overlay holds no terrain depth yet (the question above). The lever: the
    terrain's depth in the overlay, or an occluder of the spheroid's own shape.
- **Deviations in T10.d, as built** (2026-10-16; lane r10-shader, on `rendering-and-planets` at
  `a4dcf814`).
  - _Files._ The task's: a new `view/terrain/classMap.ts` (`CLASS_MAP_LEVEL`,
    `CLASS_MAP_FACE_TEXELS`, `classMapLawsOf`, `classMapTexelsOf`, `ClassMapSource`,
    `ClassMapPool`, `ClassMapStore`); R07's `bodies/discSurface.ts` (`classMapSide`,
    `classMapSharesAt`; `packClassMap`, `classWeightsAt`, `classMapTextureSpec` and
    `classMapSurface` on the new layout), `bodies/discShading.ts` (`sharesAt` reads the
    reconstruction) and `shaders/bodyDisc.wgsl` (`CubeFaceUv`, `cube_face_uv`, `TexelShares`,
    `texel_shares`, `surface_shares` rewritten; `class_map_texel` kept for T10.c's coverage mask);
    `displays/view/photorealFrame.ts`, `viewFrameDrawer.ts` and `ViewDisplay.tsx`. Beyond them:
    `displays/view/useInstruments.ts` (the instruments' frames); the data path's
    `crates/hyperion-surface/src/worker.rs` (`WorkerFields::class_map`, `ClassMap`,
    `ClassMapClass`, `ClassMapTexel`, `CLASS_MAP_LEVEL`, `CLASS_MAP_BAND`, `CLASS_MAP_CLASSES`) with
    five tests in `worker/tests.rs`, `src/wasm.rs` (the `classMap` export, `ClassMapAnswer`, the
    shared `photometry_values`), and the client's `workers/messages.ts` (`ClassMapAnswer`, the
    `class-map` request, the `class-map` and `class-map-failed` replies, `classMapTransferables`),
    `workers/heightBake.ts` (`WasmClassMap`, `WorkerFields.classMap`, its answer) and
    `workers/pool.ts` (`HeightWorkerPool.classMap`); `test/scriptedHeightWorker.ts`
    (`pendingClassMaps`; its field messages by their kinds); the smoke harness's `smoke/bodies.ts`,
    and `smoke/litTerrain.ts` and `smoke/surveyEdge.ts`, whose replies' switches take the new kinds.
    No wire format, payload or golden changes: the worker query is a client message alone.
  - _The map's level_ (main's provisional ruling on R10.T2's question, confirmed against Design
    note 8). Quadtree level 6, band level 0: 64 texels along a face's side, each the cell
    (f, 6, i, j), its weights `weights_at` at band level 0. It is inside the ruling's bound (a map
    at L reads band level L − 6, Design note 8's "level 4 or shallower" bounding L at 10), and it is
    the only level that keeps Design note 8's "a few hundred kilobytes": with the gutter 66 × 66
    texels a layer, 104,544 B at four classes or fewer and 418,176 B at sixteen (24 layers), where a
    map at L = 10 would take about 100 MB. `weights_at` evaluates there from what the worker holds:
    a surveyed cell's centre reads its margin's cells (tested on the Moon-like field, level 6, and
    the Mars-like, level 7). The level is a named constant on both sides (`CLASS_MAP_LEVEL`).
  - _The data path_ (the lane lead's decision). An additive worker query,
    `HeightWorkerPool.classMap(body, detailSeed)`, a `Promise` answered by one worker, the least
    loaded, after the blocks posted before it, with the module's `classMap(system, body, seed)`:
    the classes (two bytes each, ≤ 16), their photometry (T10.b's five values a class: c `A_N` in
    B, V and R, the law's code, L(0)), and each texel the survey touches, by its cell index at
    level 6, with its surveyed share and its weight bytes; `null` where no payload field is held (a
    stand-in's included); a module refusal rejects with its message; a failed worker's query is
    asked again of its replacement after the fields, or of another worker, and of none answers
    `null`, as a terminated pool does.
  - _Partly surveyed texels_ (the lane lead's decision, as built). A texel's surveyed share is its
    field cells surveyed over its cells (4^(level − 6), margin cells unsurveyed), 1 where the field
    is at level 6 or shallower. Its weights are `weights_at` at its centre where the field cell
    holding the centre is surveyed, and otherwise the mean of `weights_at` at its surveyed cells'
    centres, both at band level 0, as bytes summing to 255 by the largest remainder; the client
    multiplies them by the share, so `elsewhere` takes the rest. A texel whose reads lack a cell is
    left out and the map marked incomplete (the store asks again at a block that brings cells).
  - _The classes._ Every material the texels name, in `GroundMaterial`'s order; past 16, the
    heaviest by weight × share summed over the texels (ties to the earlier material), the rest's
    weight left to `elsewhere`, not spread over the kept classes (asked below).
  - _Laws_ (the lane lead's decision). One per class, a full `PhotometricLaw` of its own:
    `{ ...classRowLaw(code, L(0)), a: c A_N }` from the worker's photometry, so the disc and the
    terrain take one row law each (identity-keyed rows hold: the store builds the laws once a map).
    A class whose palette names no substance (law code 0) shades with `elsewhere`, as the lit
    terrain's law 0 takes such a slot, so that its texel still counts as surveyed in the
    reconstruction. `elsewhere` is the body's photometry's law, given each frame
    (`ClassMapSource.classMapOf(body, elsewhere)`). R07's disc keeps `oblateAlbedoScale` on every
    law, the classes' included, as R07 built it; T10.e decides the terrain's side (T10.b's
    deferred "The class laws carry no figure scale").
  - _The gutter_ (the lane lead's decision): (N + 2) × (N + 2) texels a layer, face texel (i, j) at
    (i + 1, j + 1); each side of the gutter the edge texels of the neighbouring face across that
    edge (found by a point 10⁻⁹ past the edge, tested against R05's `edgeNeighbour`), its corners
    empty; the shader's N is `textureDimensions − 2`. This changes R07's recorded N × N layout
    (a pointer in R07's "Deviations in T8.b, as built"). R07's one-texel `bodies:no class map` stays,
    never read.
  - _The reconstruction_ (`surface_shares`, its twin `classMapSharesAt`): with n s and n t the
    point's place in texels, `elsewhere` alone where the texel holding it, (⌊n s⌋, ⌊n t⌋), holds no
    weight; otherwise each of the four texels from (⌊n s − ½⌋, ⌊n t − ½⌋), the gutter's at a face's
    edge, by its bilinear weight, those holding weight alone, each its own share vector
    (`surfaceShares`: `elsewhere`'s 1 − Σ w and the classes'), summed and renormalised. The texel
    holding the point is always one of the four at a bilinear weight of at least ¼. A partly
    surveyed texel counts as surveyed. `classWeightsAt` now reads a texel's own weights through the
    gutter's layout.
  - _The live store_ (the lane lead's decision). `ClassMapStore(engine, pool)`: `want(body,
detailSeed)` asks the pool, and it asks again after each of the body's blocks the workers take
    (the pool's `onFieldChange`, which follows the workers' coverage as T10.c's mask does) that
    surveys a cell, starts the field afresh, or brings a cell while the map is incomplete or
    absent; one query out a body, a change meanwhile asking again after it; an answer for a body
    released or asked for with another seed meanwhile dropped. It owns each texture (`<name>
    <body>`, `coarse-field-gpu`), remakes it at `onRestored` (a map answered during a loss waits
    for the restore), releases it with the body and at `dispose`, and reports a refused query once
    a body. Plumbing: `litBodiesOf(scene, pose, discs, classMaps?)` sets each body's `surface` from
    `classMaps.classMapOf(id, photometry.law)`, `PhotorealInputs.classMaps`,
    `ViewFrameInputs.classMaps`, `InstrumentsFrame.classMaps` and `ViewStage`'s `classMaps` prop,
    which `ViewPanels` passes as `null`: no view creates a `CoarseFieldStore` or a pool, so every
    disc is uniform until one does (asked below). The rotation is as R07 built it: the scene body's
    at its drawn time, none where it is `null`, whose class map then shades with `elsewhere`.
  - _The cell pass._ `drawnThroughCells` still keeps class-map discs out of R07's `disc cells`
    pass, which binds `noClassMap`; Risks' "Class-map discs and R07's cell pass" stays open.
  - _Tests._ `discSurface.test.ts`: the layout with its gutter (the gutter's sides against
    `edgeNeighbour` on all six faces, its corners empty), the 66-texel upload under
    `coarse-field-gpu`; each disc pixel equals its laws' reconstructed shares of the uniform discs to
    10⁻¹² (the test's own hit taken by cross products, since the shares now vary across a texel and
    the old quadratic's 10⁻¹³ moved them 3.5 × 10⁻¹²); a surveyed texel's centre gives its own
    shares to 10⁻¹², a partly surveyed one's with `elsewhere`'s rest; no sample in an unsurveyed
    texel, 3,072 of them to within 10⁻³ of its edge, carries surveyed weight; a linear ramp across
    the +x/+y edge stays linear to 10⁻⁶; a map of one class equal to the uniform law surveyed whole
    draws the `lawFor` disc to 10⁻⁴ (and T8.b's half-surveyed one to 10⁻⁵); the half-surveyed
    two-class map's flux equals the area-weighted fluxes, its areas the reconstruction's over
    2 × 10⁵ directions, within T8.b's 0.3% (the task's 1%); the edges and corners refuse nothing.
    `classMap.test.ts`: no texel for an unsurveyed cell over all 24,576, a partly surveyed texel's
    weights times its share, the laws, the refusals, and the store's asking, re-asking, dropping,
    restore, release, disposal and reporting. `photorealFrame.test.ts`: `litBodiesOf` passes each
    body's class map with its own photometry's law, a rotation where the scene body has one and
    none where it is `null`, that body's record then `elsewhere`'s uniform surface, and no surface
    without a source. `pool.test.ts`, `heightBake.test.ts` and `fieldsWasm.test.ts` (the real
    module over the worker golden's blocks: a texel for each surveyed cell, shares 1, bytes summing
    to 255, the classes' photometry lawful, and packed no weight elsewhere). Rust: the map holds each
    surveyed cell of the Moon-like field with `weights_at`'s bytes at its centre and the classes'
    photometry; a Mars-like texel three-quarters surveyed takes the mean of its cells' within a byte
    and its share 0.75; the largest remainder; the heaviest sixteen of seventeen; no field.
  - _The smoke run_, `R07.T8.b the class-map hook` (part 1): T8.b's two checks on the new layout and
    reconstruction, and `R10.T10.d a disc under a half-surveyed class map at 64 texels a face equals
the CPU rasteriser's, every texel finite`: a 40 px, 10% oblate disc at 60° of phase, half of each
    face surveyed, the survey's edge column half surveyed, two classes in a pattern that changes
    texel to texel. No pixel needed excluding at the survey's edge. Run with
    `just test-render --part=1` (2026-10-16, both variants, 548 checks passing, none failing): each
    disc 1,357 pixels, worst 0.125 of R07.T8.a's texel tolerance (T8.b's map 0.926 under the nearest
    read), the one-class map's texels the uniform disc's exactly; and `--part=6` (both variants,
    74 checks passing, none failing; T10.b's lit terrain and T10.c's survey edge unchanged).
  - _Acceptance, as built:_ `just gen-surface` first (the `classMap` export), then
    `pnpm --filter hyperion exec vitest run src/renderer/src/view/terrain src/renderer/src/view/bodies
src/renderer/src/displays/view` (72 files, 1,454 tests; the client suite 423 files, 12,179 tests)
    and `just test-render --part=1` (the class-map, disc and mesh groups, whose shader this task
    changes) and `--part=6` (the lit terrain and the survey edge, which compose `bodyDisc.wgsl`
    through the mesh regime); and the Rust the task's commands do not reach,
    `cargo test -p hyperion-surface --features testing --lib worker` (32 tests since the batch
    review, six of them the class map's: it added the incomplete-map test), with the surface
    crate's Clippy on the native and browser targets.
  - _By hand._ None can run here: no view feeds a class map yet. The disc's look under a live
    survey, on the development machine and the owner's UHD 620, stays pending until the store is
    wired (T10.e's hand-over and T15).
  - _Generator version._ Untouched (22): presentation alone; no golden moved, the worker golden
    included (the map is tested natively and on the module without one).
  - _The batch review (2026-10-19)._ The class map joins the worker's order-independence test, and
    the incomplete map is tested (a texel whose margin is in a block not yet taken is left out,
    then held, the map a fresh worker's); band level 0 is pinned in the weights goldens (new lines
    at 22, no existing line moved; main's ruling on R10.T2's Q2); and the texel loop's allocations
    are hoisted. On the client, the tests that failed for more than one reason are split, one
    behaviour an `it` (`classMap.test.ts`' refusals and the store's restore, release, listener and
    disposal; `heightBake.test.ts`' and `pool.test.ts`' class-map queries).
- **Questions for "main" (R10.T10.d)** (none touches a contract of the server's; each built to its
  lean).
  - _Who wires the live store and pool._ No live view creates a `CoarseFieldStore` or a
    `HeightWorkerPool`, and no plan-10 task names wiring them. Lean: the task that first draws a
    body's terrain live (T10.e's hand-over, or T11's wireframe if it lands first) makes the store
    over the view's engine and the pool in `ViewStage`, beside T10.c's `CoverageMask`, and passes it
    as `classMaps`. _Answered 2026-10-16: R10.T16 "The live terrain path"
    (decision-r10-live-path.md)._
  - _The map's level in the text._ Lean: Design note 8's "T2's weights at level 4 or shallower"
    and T2's "the class map's level (level 4)" read "a texel a cell of quadtree level 6, its
    weights at band level 0" (main's provisional ruling, confirmed here).
  - _Past sixteen classes._ Lean: as built, the lightest classes' weight left to `elsewhere`; the
    alternative, T2's spreading over the kept classes, would shade unknown ground with known laws.
  - _A class of no named substance_ shades with `elsewhere` and counts as surveyed. Lean: as built,
    as the lit terrain's law 0.
  - _The cell pass_ (Risks, "Class-map discs and R07's cell pass"): still open, for the
    orchestrator; T10.d keeps class-map discs out of it as R07 built.
- **Deferred findings (2026-10-13 rule) (R10.T10.d)** (recorded, not ruled; each leaves the
  algorithm and the contracts as built).
  - _Band level 0's agreement is unmeasured._ T2 held the weights to counted classes at n − 1 to
    n − 6 and at band level 4; the map reads band level 0, a texel of 1/64 of a face's side, whose
    own agreement no test states. The lever: T2's slow agreement case at band level 0.
  - _A partly surveyed texel whose centre's cell is surveyed_ takes the centre's weights for its
    surveyed part, where the mean over its surveyed cells would weigh only them. The lever: always
    the mean (up to sixteen `weights_at` a texel at level 8).
  - _A whole map a block._ Each block that surveys cells asks for the whole map: 24,576 texels
    scanned and a `weights_at` (13 to 26 µs, T2's figures) for each surveyed texel, about 0.3 s on
    one worker for 10⁴ texels, beside its bakes. The lever: asking for the texels a block's cells
    touch alone.
  - _At a cube's corner_ three texels meet and the gutter's corner is empty, so within half a texel
    of a corner the reconstruction renormalises over three: continuous across each edge, not
    exactly at the corner.
  - _The centre's cell at field levels 7 and 8_ (the batch review's consider item, 2026-10-19).
    There a texel's centre is the corner where four of its 4 or 16 field cells meet, so which cell
    is "the centre's" is settled by `PatchKey::containing`'s rounding of the st → uv → xyz → st
    round trip: deterministic on every target, but an integer choice of the child,
    `(i << shift) + (side >> 1)`, would be exact.
- **Deviations in T10.e, as built** (2026-10-16; lane r10-shader, on `rendering-and-planets` at
  `05da4caf`).
  - _Files._ A new `view/terrain/handover.ts` (`pixelAngleRad`, `handoverRangeM`,
    `handoverReliefM`, `limbDistanceM`, `handoverTauPx`, `handsOver`, `handoverRangeRatio`,
    `HandoverSphere`, `handOverRegimes`, `HandoverReliefSource`, `storeReliefSource`) with
    `handover.test.ts` and `handover.wasm.test.ts`; `view/quality/qualitySetting.ts`
    (`WIREFRAME_TERRAIN_TAU_PX`); R07's `bodies/regime.ts` (`LitRegime`'s `terrain`, `drawnAsMesh`,
    the promotion's writers), `bodies/draw.ts` (`LitBodyInput.handoverReliefM`,
    `MeshBodyPlan.handedOver`, `planLitBodies`' hand-over, the coverage mask's reader),
    `bodies/discShading.ts` (`discReflectance`, `scaledLawOf`) and `photoreal/renderer.ts`
    (`FrameTerrain`, `PhotorealFrame.terrain`, its draws in the `bodies` pass);
    `displays/view/photorealFrame.ts` (`PhotorealInputs.reliefs` and `terrain`,
    `LitBodiesTerrain`, `litBodiesOf`'s fifth argument) and `viewFrameDrawer.ts`
    (`ViewFrameInputs.reliefs` and `terrain`); `view/terrain/terrainMaterial.ts` (`figureScaledA`,
    `packClassLaws`' and `classTexelLaw`'s c ÷ a, `terrainTexelLaws`), `gpu/resources.ts` (passes
    the figure's c ÷ a), `shaders/terrainLit.wgsl` (a comment alone) and `coarseFieldStore.ts`
    (`FieldCandidate.rangeRatio`'s documentation); the smoke harness's `smoke/surveyEdge.ts`, whose
    reference body is now handed over. Tests beside them in `terrainMaterial.test.ts`,
    `draw.test.ts`, `regime.test.ts`, `renderer.test.ts` and `photorealFrame.test.ts`.
  - _The range, pure_ (the lane lead's decision). `handoverRangeM(reliefM, tauPx, radPerPx)` is
    relief ÷ (τ · rad a pixel), refusing a relief negative or not finite and a τ or pixel not
    positive: at Design note 13's 5.454 × 10⁻⁴ rad a pixel, 1.833 × 10⁷ m for 10 km at 1 px and
    4.584 × 10⁶ m at 4 px (the task's 1.8 × 10⁷ and 4.6 × 10⁶ m). The view passes R07's own pixel,
    `pixelAngleRad` = 2 tan(fov ÷ 2) ÷ width, the inverse of `angularDiameterPx`'s centre-pixel
    scale that `litRegimes` measures its 3 px switch by, so that both switches count pixels alike:
    6.014 × 10⁻⁴ rad at 1080p across 60°, where 10 km hands over at 1.663 × 10⁷ m (4.157 × 10⁶ m at
    4 px), as R07's orchestrator ruling of 2026-10-03 took the centre pixel for its own figures.
  - _The distance is the limb's_ (beyond the decisions; asked below). `handsOver` sets the range
    against the camera's distance to the body's limb, √(D² − a²) for a camera D from the centre of
    a body of equatorial radius a (`limbDistanceM`, 0 from inside), not the centre's: there the
    relief stands against the sky across the line of sight. Far off the two agree (the centre is
    7% farther at an Earth's 1.663 × 10⁷ m, which a live 1080p view so reaches from 1.781 × 10⁷ m
    off its centre); near the ground the limb's goes to 0, so a body whose relief is small beside
    its radius still hands over as the camera nears it. From the centre it would not: a Venus of
    6,051.8 km and 13 km of relief has a wireframe range of 5.40 × 10⁶ m, inside its radius, and
    would never hand over there, landed or not; from its limb it hands over some 1,740 km up
    (`handover.test.ts`). `handoverRangeRatio(limbM, reliefM, tauPx, radPerPx)` gives
    `FieldCandidate.rangeRatio` by the same distance, `Infinity` with no relief (R10.T7's lean),
    and that field's documentation now says so (T7 computed none), so that a body's field is wanted
    before it hands over.
  - _τ per style_ (the lane lead's decision). `handoverTauPx(style, setting)`: the settings'
    `TerrainSettings.tauPx` in the photorealistic style (1 px high, 2 px low) and
    `WIREFRAME_TERRAIN_TAU_PX` = 4 in the wireframe on both settings, a named constant beside
    `TERRAIN_SETTINGS`.
  - _The relief_ (the lane lead's decision). `handoverReliefM(header, coverage)` is the header's
    `realised_relief_m`, as Design note 13 names R09's `realised_relief`, where a header and a
    coverage whose `surveyedFraction()` is above 0 are held, and `null` otherwise (and for a relief
    of 0 or not finite): `null` never hands over, at any distance. `storeReliefSource(store)` reads
    a `CoarseFieldStore`'s `header` and `coverage` through it. Design note 13's one-sided example is
    a deferred finding below.
  - _The hysteresis applies naturally, and is R07's_ (the lane lead's decision). `handsOver` takes
    `REGIME_HYSTERESIS` on the relief's subtense as R07's switch takes it on the diameter: a body
    hands over once its relief subtends τ (1 + h) px at its limb, the limb within range ÷ 1.1, and
    hands back once under τ (1 − h), past range ÷ 0.9. The previous frame's state is the regime
    map's own `terrain`, which the views already carry (`previousRegimes`); `litRegimes` keeps a
    `terrain` body on the disc side of its 3 px switch as it keeps a `mesh`.
  - _The decision, per view and style_ (the lane lead's decision). `LitRegime` gains `terrain`, the
    mesh regime whose ground R05's quadtree draws (Design note 13: "the `mesh` regime hands its
    geometry to R05's quadtree"), a fourth value rather than `mesh` with a flag, so that the
    hysteresis reads the previous frame from the map R07 already passes and a mesh by promotion
    is not taken for one handed over (asked below). `planLitBodies` runs `litRegimes`, then
    `handOverRegimes` (each disc with a relief, at the photorealistic style's τ on
    `options.setting` and `pixelAngleRad` of the frame's camera and viewport, the pixel the 3 px
    switch measures), then `promoteOverlapping`, whose writers now include a body handed over
    (`drawnAsMesh`), so that a disc overlapping it is promoted. A point never hands over: its flux
    is the point regime's, T10.f's. A body handed over is drawn as a mesh body is (its smooth figure
    in the `bodies` pass, its limb in the sequence), its `MeshBodyPlan.handedOver` set, its figure
    the reference surface beneath its terrain: T10.c's coverage mask is now read for a body handed
    over alone, so that a body promoted by its overlaps draws its figure whole (T10.c read any
    mesh body's; no view gave one). The terrain input is `PhotorealFrame.terrain?: FrameTerrain`,
    the view's: `surveyCoverage(body)`, which `litBodiesOf` passes as each body's
    `surveyCoverage`, and `draws(plan)`, drawn in the `bodies` pass after the figures; the reliefs
    are `PhotorealInputs.reliefs` (a `HandoverReliefSource`), which `litBodiesOf` passes as each
    body's `handoverReliefM`. `ViewFrameInputs` takes both, optional. Without a terrain input a body
    handed over is drawn as a mesh body is, its figure whole; without reliefs none hands over.
  - _Not built_ (the lane lead's decision; asked below). The live pipeline: no view makes a
    `CoarseFieldStore` over `surface_field`, a `HeightWorkerPool`, a `TerrainPass` per view and
    body handed over (a `FrameTerrain`), a `TerrainLighting` from the body's disc record, the
    `AtmosphereScene` draw of the terrain, a `CoverageMask` or the draw list's `surveyEdges`;
    `ViewStage` passes neither input, so no body hands over live. The wireframe: the decision at
    its τ is built and tested (`handoverTauPx("wireframe", …)`, `handsOver` at 4 px), its draw is
    T11's.
  - _One row, one figure scale_ (the lane lead's decision). The terrain's class laws now take the
    body's figure scale as the disc's laws do (`scaledLaws`): each class's A is c A_N times
    `oblateAlbedoScale` of its L(0) and the body's c ÷ a (`figureScaledA`, in `packClassLaws`, which
    `TerrainResources.upload` calls with its figure's c ÷ a, and in the twins `classTexelLaw` and
    `terrainTexelLaws`, the latter the twin of `packTerrainLighting`'s law 0 and `packClassLaws`'
    slots), retiring T10.b's deferred "The class laws carry no figure scale". The factor is 1 under
    Lommel–Seeliger's L(0) = 1, whose disc reaches p on any figure, 1.0062 for the Ice row at
    L(0) 0.628 and 1.0224 under Lambert's at c ÷ a 0.9, and 1 + 2.4 × 10⁻⁴ under Lambert's on the
    Moon-like world (c ÷ a 0.9988; the code's 1 + 2.85 × 10⁻⁴ carries R07's quadrature bias,
    deferred below). It is right as the disc's is: T10.a's c is solved on the
    sphere's p, and the factor takes the disc to that p equator-on, so the terrain under it draws
    the disc's light a class. T10.b's tests are unchanged but for the new argument (1, a sphere).
    `discShading.ts` exports `discReflectance(laws, c ÷ a, shares, μ₀, μ, α)`, the disc's own
    `surfaceReflectance` at h = μ₀ with no source radius, through the same scaling.
  - _The agreement_ (`handover.test.ts`). Per class law, Regolith (A 0.172, 0.142, 0.105 in r, g,
    b at L(0) 1), Ice (0.92, 0.90, 0.86 at L(0) 0.628) and Lambert (0.81, 0.80, 0.78), each from
    T10.d's `classMapLawsOf` on the disc's side and `terrainTexelLaws` on the terrain's, and law 0
    (Mercury's template at the Moon-like world's p): the disc's and the terrain's I/F at 16 (μ₀, μ)
    pairs at 30° and 90° of phase, on a sphere and at c ÷ a 0.9, agree exactly (the test holds
    10⁻⁶), and each law's row packed by `packPhaseFactorRows` from the disc's law and from
    `TerrainPhaseRows`' row is the same, bit for bit: one row read by both.
  - _Tests._ `handover.test.ts`: the range at Design note 13's pixel and at a live view's, τ per
    style, the refusals, the range ratio, the relief and a body with no survey, coverage or header
    never handing over at any distance, the store's source, the hysteresis both ways at the range's
    ends, the limb's distance and a Venus handing over near its ground, the wireframe's quarter
    range, a disc alone handed over by its own relief whatever the input order, the agreement
    above, and the classes' q (below). `draw.test.ts`: the setting's τ (1.5 px hands over on high
    and not on low), R07's 10% about τ, a body handed over drawn as a mesh body with its limb, and
    one promoted but not handed over drawing its figure whole; T10.c's mask test now hands its
    planet over. `regime.test.ts`: a body handed over promotes the disc it overlaps and keeps the
    disc side of the 3 px switch. `renderer.test.ts`: a body handed over drawn as a mesh without a
    terrain, and the terrain's draws after its figure in the `bodies` pass with one.
    `photorealFrame.test.ts`: each body's relief and coverage mask where the inputs hold them, and
    none without. `terrainMaterial.test.ts`: each class's packed A scaled for an oblate figure.
  - _By hand: what ran scripted_ (`handover.wasm.test.ts`, the real module). The Moon-like fixture's
    every block (eight surveyed cells of level 6 on face +y, `moon_like.golden`) held by a worker's
    fields; its class map (T10.d) for the disc's twin; and for the terrain's twin each patch over
    the survey baked at levels 0 (the root face, where Design note 13's quadtree takes the ground
    over), 2, 4, 6 (the field's) and 7 (44 patches). At 30° and 90° of phase, the sun and the camera
    far off in the plane of the survey's mean normal, each twin's I/F weighted by μ over a 16 × 16
    grid a texel on the spheroid's normal (both twins at the same points and normals, so the
    resolved slopes of the bake are not in it): the per-texel and the survey's mean brightness
    change by 0 stops at every level. The fixture's surveyed ground is one law, every named
    material there resolving to A 0.20 at L(0) 1 on the Regolith row (deferred below), so the case
    reduces to the reconstruction; as a sensitivity, the one material the bakes weigh but the map
    leaves out (palette pair (0, 0), up to 16% of a level-7 vertex's weight) given Lambert's law at
    half the albedo on both sides moves a texel by at most 1.27 × 10⁻³ stop at level 7 (30°; 1.26 ×
    10⁻³ at 90°), 4.33 × 10⁻⁴ at level 6, and 0 at levels 0 to 4, and the survey by at most 2.02 ×
    10⁻⁴: all under the task's 1/3 stop, which the test holds.
  - _By hand: pending, for the development machine and the owner's UHD 620_, until a view draws a
    generated world's terrain live (asked below). On a thin-aired generated world fully surveyed
    (a served Moon-like or Mars-like analogue, after P14.T54.a and the live pipeline), in `VIEW`
    at 1080p on the high setting and again on the low: (1) lock the exposure (the display's manual
    control) and set the operator's meter to `average`; (2) place the camera so that the phase at
    the body is 30°, at three times its hand-over range (1.663 × 10⁷ m from the limb for 10 km of
    relief on high, half that on low), and close along the line of sight at no more than 1% of
    the range a second until the regime map shows the body `terrain`, then back out past
    range ÷ 0.9 until it shows `disc`; (3) at the frame before and the frame after each switch,
    take the scene target's HDR read-back (hidden, as the smoke harness's `readBack`), sum the
    body's pixels' luminance, and per class-map texel the pixels whose rays meet that texel
    (`classMapSharesAt`'s texel, from `rasteriseDisc`'s hit), and state log₂ of each after ÷ before:
    each under 1/3 stop; (4) repeat at 90°. Also by eye there: the survey's edge at the hand-over
    level on a partly surveyed world (deferred below) and T10.b's "Back-facing shading normals draw
    black" at grazing views, whose floor on μ T10.e's look was to decide.
  - _The residuals of T10.a against plan 14's p and q._ p: T10.a's own record ("Deviations in
    T10.a, as built", the table): in V zero to rounding (at most 2 × 10⁻¹⁶ relative) on every
    served world, in B and R +35% and −17% on Mercury's colour and +93% and −41% on Mars's, the grey
    palettes'. q, computed now (`handover.test.ts`): a patterned sphere's orientation-averaged phase
    integral is Σ aᵢ pᵢ qᵢ ÷ Σ aᵢ pᵢ, as its p is the area average (T10.a's module documentation),
    so on a world whose ground takes one class law it is that law's whatever the albedos. The
    airless worlds of T10.a's table (the Moon- and Ceres-like and the reference Moon and Ceres, all
    regolith or bedrock of silicate families, the Regolith row by `PhaseLaw::for_family`) and the
    reference Mars's dust take the Regolith row, q 0.62600 under McEwen's L(α) (V), against plan
    14's 0.47980 on Mercury's template (+30.5%) and 1.08465 on Mars's at L 0.5 (−42.3%) (deferred
    below). The Mars-like world, whose glaciers take Lambert's row (q 3/2) on 1.5% of its area at
    their clamped albedos, waits on T10.f's point integral or those albedos; the Earth-like is not
    served a scale.
  - _Acceptance, as built:_ `pnpm --filter hyperion exec vitest run src/renderer/src/view/terrain`
    with `src/renderer/src/view/bodies`, `src/renderer/src/view/photoreal` and
    `src/renderer/src/displays/view`, whose tests it adds; `just test-render --part=6`
    (2026-10-16, both variants, 74 checks), and the whole harness in the batch's gate
    (`/home/quantum/gh/hyperion/.git/rm45-scratch/r10-shader/smart-ci-final.log`), over the final
    tree.
  - _Generator version._ Untouched (22): presentation alone, no golden moved.
  - _The batch review (2026-10-19)._ `MeshBodyPlan.handedOver` and its `coverage` are one
    discriminated `role` (`MeshBodyRole`: `{ kind: "promoted" }` or
    `{ kind: "handed-over"; coverage: MeshBodyCoverage | null }`), read by an exhaustive switch, so
    that a promoted body cannot carry a mask; `LitBodyInput.surveyCoverage` is the mask with the
    axes it is read in (`MeshBodyCoverage`), which `litBodiesOf` gives only with the body's
    rotation, so that it cannot arrive without them; `photoreal/renderer.ts`, its test and the
    smoke's reference body take both. New tests: `photorealFrame` passes the view's class maps,
    reliefs and terrain to the lit bodies and carries the terrain on the frame, a body without a
    rotation takes no mask, and `ViewFrameDrawer` passes all three on. Tests split to one
    behaviour an `it` (`regime.test.ts`, `draw.test.ts`' mask test); `handover.wasm.test.ts`
    reads the shared `moonLikeFixture`; `handover.test.ts` imports its types as types, takes
    Design note 13's pixel as 60° ÷ 1,920 (5.454 × 10⁻⁴ rad), pins 1.833 × 10⁷ and 4.584 × 10⁶ m to
    10⁻³ (the old 1.83 × 10⁷ and 4.59 × 10⁶ m at 3 × 10⁻³ held too), and puts the Venus's hand-over
    about 1,740 km up; `handover.ts`' a · 4 px is 14.6 km (the science check).
- **Questions for "main" (R10.T10.e)** (none touches a contract of the server's; each built to its
  lean).
  - _Who builds the live terrain pipeline._ No plan-10 task names it: the store over
    `surface_field` with each view's `FieldCandidate`s (`handoverRangeRatio`), the pool, a
    `TerrainPass` per view and body handed over as the frame's `FrameTerrain` (lit by a
    `TerrainLighting` from its disc record, drawn into the `AtmosphereScene`), T10.c's
    `CoverageMask` and the draw list's `surveyEdges`, and T10.d's `ClassMapStore`, fed through
    `ViewStage` (the lane lead has told main; the scoping agent's `decision-r10-live-path.md`).
    Lean: a task main assigns. _Answered 2026-10-16: R10.T16 "The live terrain path"
    (decision-r10-live-path.md)._
  - _The limb's distance._ Design note 13 says when the relief "subtends τ pixels" without the
    distance it is measured over; as built, the camera's to the limb, so that a body of little
    relief beside its radius hands over near it (above). Lean: as built, Design note 13 amended to
    "subtends τ pixels at the body's limb", and Design note 16's want range with it.
  - _The pixel it is measured in_ (the batch review's science check, 2026-10-19), asked with the
    limb's distance and Design note 16's want range. The live view measures its pixel at the
    centre pixel, 2 tan(fov ÷ 2) ÷ width (6.014 × 10⁻⁴ rad at 1080p across 60°, where 10 km hands
    over at 1.663 × 10⁷ m), as R07's 3 px switch does, where Design note 13 takes width ÷ field
    (5.454 × 10⁻⁴ rad, 1.833 × 10⁷ m). Lean: as built, Design note 13's figures restated at the
    centre pixel with its limb's distance.
  - _`terrain`, a fourth regime._ Lean: as built, R07's regime text ("a `mesh` … or R10 hands it to
    terrain") reading "`terrain`, the mesh regime handed to R05's quadtree".
  - _T10.c's mask on a body handed over alone._ Lean: as built (a promoted body's terrain is not
    drawn, so its figure is the ground).
- **Deferred findings (2026-10-13 rule) (R10.T10.e)** (recorded, not ruled; each leaves the
  algorithm and the contracts as built).
  - _Design note 13's example is one-sided; the header's relief is peak to trough._ Its "10 km of
    relief either side of the datum" is 20 km from peak to trough, while `realised_relief_m` is
    the highest elevation less the lowest, so a symmetric world of ±10 km takes 20 km and hands
    over twice as far as the one-sided reading (the range is linear in the relief); the example's
    1.8 × 10⁷ m is a peak-to-trough relief of 10 km. The lever: the largest deviation from the
    datum, half the relief on a symmetric world, which the header does not carry.
  - _The hand-over moves with the frame's internal scale._ The plan measures the frame's own
    viewport, the render resolution at the budget's scale (720 rows on the low setting, scales
    0.5 to 1), as the 3 px switch does, while `TerrainSettings.tauPx` is stated at the presented
    resolution and `TerrainPass` selects at it; so the range falls with the scale, to half at 0.5,
    and a body near its range may hand over and back as the resolution controller moves by more
    than the 10% hysteresis. The lever: both switches on the presented pixel.
  - _The classes' q is the Regolith row's, not plan 14's._ The airless rocks take Mercury's q
    (0.480) while their classes draw the Regolith row's (0.626, the Moon's curve), and the
    reference Mars's dust Mars's (1.085) against the same 0.626: +30.5% and −42.3%. So away from
    zero phase surveyed ground on the disc and the terrain alike differs from unsurveyed ground,
    `lawFor`'s, by the two curves' ratio, and T10.f's point under a class map will differ from the
    photometry's point by as much in phase-integrated light. For plan 14 and R10.T1.a's rows (the
    template a family's row draws, or the photometry's q from the classes).
  - _The scripted world is one law._ The Moon-like fixture's surveyed ground resolves every named
    material to one photometry (A 0.20, L(0) 1, the Regolith row), so the scripted check reduces to
    the reconstruction, and its sensitivity case stands in for a contrast. The lever: a fixture
    survey with contrasting classes (a Mars-like survey with its glaciers, or an Iapetus).
  - _The survey's edge at the hand-over level._ At the hand-over the quadtree takes the ground from
    the root faces (Design note 13), where a quad spans a cell of a level-6 field or more and
    T10.c's discard falls back on the interpolated mask's half (its code 0): along the edge of a
    partial survey both surfaces draw, or neither, over a strip up to a quad wide, which at the
    range spans π a τ ÷ (128 · relief) px, about 16 px on an Earth of 10 km and 3 px on a Moon-like
    of 15 km. The lever: the terrain's discard reading the GPU coverage mask at the fragment, as the
    reference surface does, where the patch is coarser than its code can carry.
  - _The figure's factor after the clamp_ (the batch review's science check, 2026-10-19). The
    per-class oblate factor applies after T10.a's clamp, so it shifts the Lambert to
    Lommel–Seeliger contrast by k_Lambert ÷ k_LS (2.2% at c ÷ a 0.9, 6.7 × 10⁻⁴ at Earth's
    0.99665), and can push a Lambert class past its ceiling A_N ≤ 1 (0.98 × 1.0224 = 1.002 at
    c ÷ a 0.9; the factor is 1.087 at c ÷ a 0.7 and 1.204 at 0.5), for fast rotators such as a
    Haumea. The lever: T10.a's c solved on the spheroid (m(0) for ⅔), or each scaled A clamped to
    its law's `normal_albedo_ceiling` (the server's).
  - _R07's quadrature bias in the figure's factor_ (the batch review's science check). The code's
    Lambert factor on the Moon-like world, 1 + 2.85 × 10⁻⁴ against the closed form's
    1 + 2.4 × 10⁻⁴, carries R07's 96 × 192 quadrature: m = 0.666637 against ⅔ as c ÷ a → 1, a
    bias of 4.5 × 10⁻⁵, which the disc and the terrain share, so that they agree. R07's lever: the
    quadrature's grid, or m's closed form.
- **Deviations in T10.f, as built** (2026-10-16; lane r10-shader, on `rendering-and-planets` at
  `05da4caf`; finished 2026-10-19 after a reboot).
  - _Files._ The task's: R07's `bodies/draw.ts` (`pointFlux` takes each light's reflection from
    `pointReflectionOf`: the class map integrated where `surface` is a class map and `rotation` is
    known, `bodyReflection` of the photometry's law otherwise) and `bodies/discSurface.ts`
    (`classMapSurface` returns its map as `texels`). Beyond them: a new `bodies/classMapPoint.ts`
    (`CLASS_MAP_POINT_FACE_ELEMENTS`, `classMapPointSubdivision`, `CLASS_MAP_POINT_CACHE_RAD`,
    `classMapPointCacheAngleRad`, `classMapReflectionRatio`, `classMapPointReflection`) with
    `classMapPoint.test.ts`, its own file since `discSurface.ts` cannot import `discShading.ts`,
    which imports it; `bodies/discShading.ts` (`DiscLawTerms`, `discLawTerms`: `discReflectance`'s
    arithmetic split by its two terms, per law); `appearance/bodyAppearance.ts`
    (`ClassMapDiscSurface.texels`); `terrain/classMap.ts` (the store's surface carries its held
    texels). Tests beside them in `discShading.test.ts`, `discSurface.test.ts` and
    `classMap.test.ts`; the surfaces `discCells.test.ts` and `photorealFrame.test.ts` build take
    texels. No shader, `view/engine/` or smoke file changes, and no smoke scene draws a point under
    a class map (the harness's 3 px switch is a uniform sphere's).
  - _The CPU copy_ (the lane lead's decision). `ClassMapDiscSurface` gains
    `texels: ClassMapTexels`, set by `classMapSurface` and by `ClassMapStore.classMapOf` (its held
    map's), so that the texture and its CPU copy travel as one value and `pointFlux` reads the map
    from `body.surface`. An additive change to R07's Provides shape, now `kind`, `weights`,
    `texels`, `laws` and `elsewhere` (asked below; a pointer in R07's "Deviations in T8.b, as
    built"). `rasteriseDisc` still takes the texels as its argument, as T10.d built it.
  - _The integral_ (the lane lead's decision). `classMapReflectionRatio(figure, surface, light,
observer)`, in the body-fixed axes (`classMapPointReflection` takes the galactic directions in
    by `rotateToBodyFixed`): over the lit (μ₀ > 0) and visible (μ > 0) elements of the spheroid
    about the body-fixed z axis, the pole the class-map disc draws about, each element's shares by
    T10.d's reconstruction (`classMapSharesAt`) times each law's I/F by the disc's arithmetic
    (`discLawTerms`: A × `oblateAlbedoScale` of L(0), f and L(α) from the law's row; equal to
    `discReflectance` to 10⁻¹⁵, tested) times μ and the element's area, in `f64`. Regrouped: each
    law's two sums over the elements, Σ share · 2μ₀μ ÷ (μ₀ + μ) dS and Σ share · μ₀μ dS, then each
    law's terms once. Normalised so that a uniform map gives the photometry's point exactly: the
    sum over the same sum under `elsewhere` alone, per channel, times `bodyReflection` of
    `elsewhere` on the figure about the rotation's pole, in the galactic axes, which is a uniform
    body's own call where its figure's pole is the rotation's. The elements' error at the limb and
    the terminator cancels in the ratio, a weighted mean of the classes' reflectances over
    `elsewhere`'s and bounded by them; it is 1 in a channel where `elsewhere` alone reflects nothing
    by the elements (none lit and visible, at the extreme of phase). Each star and each
    planetshine secondary goes through its own integral, from its own direction.
  - _The elements' rule._ A texel's face cell cut into n × n in (s, t), n = ⌈128 ÷ N⌉, 2 at R10's
    N = 64: 98,304 elements on the sphere, each at its centre in (s, t) (a texel's quarter points,
    inside whose quarters the reconstruction is bilinear between the four nearest texel centres),
    with its exact solid angle (the face rectangle's, F(u, v) = atan(u v ÷ √(1 + u² + v²)), through
    the quadratic warp) times the spheroid's factor c ÷ a · |(x, y, z a ÷ c)|. Against 2 × 10⁶
    Fibonacci directions on the Iapetus's three views at 0° to 150°: at most 3.8 × 10⁻⁴ at n = 2,
    1.33 × 10⁻³ at n = 1 (a texel's centre alone, its own shares); on a half-surveyed checkerboard
    of texels 2.0 × 10⁻⁴ (2.6 × 10⁻⁴ at n = 1), on a half-surveyed Iapetus 1.1 × 10⁻⁴
    (3.8 × 10⁻⁴). n = 2 keeps a factor of eight under the 0.3% for contrasts past the Iapetus's 14
    to 1. Cost (2026-10-16, load about 3, provisional): the elements' shares, found once a map and
    held with its texels, 300 ms (95 ms at n = 1); an evaluation, one pass over the elements, two
    dot products each and a multiply-add per share of the lit and visible ones, 1.65 ms a light
    (0.43 ms); the elements' directions and solid angles 3.1 MB once, each map's shares 1–2 MB
    (deferred below).
  - _The eclipse_ (the lane lead's decision, literal to the plan and R07.T10.b). Each star keeps
    its `discEclipseVisible` term, with the photometry's limb share (`elsewhere`'s), on its own
    integral: tested, the share an eclipse leaves under the map equals the uniform body's to 10⁻¹²
    (deferred below).
  - _The cache_ (the lane lead's decision; what it holds is asked below). Per surface, in a module
    `WeakMap` keyed by the `ClassMapDiscSurface`, at most eight entries a surface (two stars and two
    planetshine sources from two views), the latest kept; released with the surface by the
    collector once its owner drops it (the store makes a new one for a new map or a new
    `elsewhere`, and holds none once the body is released); the elements' shares likewise with the
    texels. The held value is each light's ratio, uneclipsed, not its integral: the uniform point
    (closed form on a sphere, R07's memoised spheroid integral otherwise) is fresh each call and
    carries the phase's steep dependence exactly, so δ bounds only the pattern's drift. An entry is
    taken again while the light's and the camera's body-fixed directions both lie within
    δ(α) = 10⁻³ rad × (π − α) ÷ π of its own, by chord, at its own c ÷ a. The derivation: the
    ratio's sensitivity S, |Δ ln R| per radian with both directions moved, measured on the
    Iapetus's three views at 0° to 165°, keeps S (π − α) at or below about 5.2 (worst at 60° to
    90°, 1.8 at 150° and 165°), a pattern's share of the light moving with the angle over the lit
    and visible lune's width π − α; so a cached ratio is within 5.2 δ₀ ÷ π, 1.7 × 10⁻³, of a fresh
    one, about half the 0.3%, the rest left for contrasts past the Iapetus's.
  - _Without a rotation, or with a uniform surface,_ today's path: `bodyReflection` of the
    photometry's law, tested equal bit for bit.
  - _The switch's test rasterises at 64 × 64 samples a pixel_ (the brief's allowance, "enough
    sub-pixel samples"), one centred placement: R07.T8.c's 8 × 8 at 3 px misses the Iapetus's
    bright rim, the disc's error and not the point's (asked and deferred below).
  - _Tests_ (`classMapPoint.test.ts`). The synthetic Iapetus: 64 texels a face through
    `packClassMap`, dark (the Regolith row at L(0) 1, A 0.04) where a texel's centre lies within 70°
    of the leading apex, bright (the Ice row at L(0) 1, A 0.55) elsewhere, `elsewhere` the Regolith
    row at A 0.38; the camera on galactic +z and a rotation turning each view onto it, the Sun at 1
    au. Measured (the brute force over 2 × 10⁵ directions; the half map surveyed where
    x + 0.3 z > 0.1, its edge across the cap and the bright side):

    | Facing   | Phase | Disc ÷ point − 1, 64² | At R07's 8² | Point ÷ brute force − 1, unsigned: whole, sphere and c ÷ a 0.9; half, the same |
    | -------- | ----- | --------------------- | ----------- | ------------------------------------------------------------------------------ |
    | leading  | 0°    | −0.28%                | −1.73%      | 5.7 × 10⁻⁵, 1.0 × 10⁻⁴; 6.1 × 10⁻⁶, 5.1 × 10⁻⁵                                 |
    | leading  | 60°   | −0.36%                | −1.99%      | 7.2 × 10⁻⁵, 1.9 × 10⁻⁴; 1.0 × 10⁻⁵, 1.2 × 10⁻⁴                                 |
    | leading  | 120°  | −0.56%                | −2.58%      | 3.1 × 10⁻⁵, 4.8 × 10⁻⁴; 3.7 × 10⁻⁵, 4.9 × 10⁻⁴                                 |
    | trailing | 0°    | −0.011%               | +0.050%     | 4 × 10⁻⁹, 4.5 × 10⁻⁵; 1.6 × 10⁻⁶, 4.6 × 10⁻⁵                                   |
    | trailing | 60°   | +0.010%               | +0.073%     | 2 × 10⁻⁸, 1.1 × 10⁻⁴; 5.6 × 10⁻⁶, 1.1 × 10⁻⁴                                   |
    | trailing | 120°  | −0.18%                | −0.38%      | 8 × 10⁻⁹, 4.5 × 10⁻⁴; 5.3 × 10⁻⁶, 4.6 × 10⁻⁴                                   |
    | a pole   | 0°    | +0.004%               | +0.031%     | 1.1 × 10⁻⁵, 1.7 × 10⁻⁴; 1.5 × 10⁻⁵, 1.9 × 10⁻⁴                                 |
    | a pole   | 60°   | +0.090%               | +0.083%     | 1.6 × 10⁻⁵, 1.7 × 10⁻⁴; 1.5 × 10⁻⁵, 2.1 × 10⁻⁴                                 |
    | a pole   | 120°  | −0.33%                | −1.73%      | 2.0 × 10⁻⁴, 5.7 × 10⁻⁴; 7.3 × 10⁻⁵, 2.9 × 10⁻⁴                                 |

    The tests hold the disc at 64² to the task's 1% (worst 0.56%) and the brute force to 0.3%
    (worst 5.7 × 10⁻⁴; about 4 s for its 36 cases); a one-class map equal to the uniform law,
    surveyed whole and in half, on a spheroid of c ÷ a 0.9 at 0° to 150°, by starlight and a
    planetshine source, gives the uniform point to 10⁻⁶ (exact to rounding by the normalisation);
    without a rotation, and under a uniform surface, the photometry's point exactly; an eclipse's
    share as the uniform body's; a cached value within 0.3% of a fresh one at 0.98 δ over the
    Iapetus's views at 0° to 150° (8 pairs of directions each; over 64 pairs the worst is
    1.60 × 10⁻³, facing the leading side at 60°), taken exactly from the cache within δ and afresh
    past it; and the refusal of laws not one per class. `discShading.test.ts`: `discLawTerms` by
    shares give `discReflectance` to 10⁻¹⁵ for three laws at four phases on a sphere and a
    spheroid; `discSurface.test.ts` and `classMap.test.ts`: the construction's and the store's
    surfaces carry their map's texels.

  - _The Iapetus's faces_, for R07's "The point keeps the photometry's law": against a uniform law
    of the area-mean p (0.382), the point at zero phase is 0.260× facing the leading side and
    1.439× facing the trailing, 1.86 mag apart, where decision-r07-t8b estimated 0.28× and 1.4×.
  - _Acceptance, as built._ `pnpm --filter hyperion exec vitest run src/renderer/src/view/bodies`
    (9 files, 391 tests), with `view/terrain`, `displays/view`, `view/appearance` and
    `view/lighting` (95 files, 2,037 tests), and the client suite (427 files, 12,315 tests, 2
    skipped), `pnpm typecheck` from a clean cache, and oxlint and Prettier on the files. No smoke
    scene draws a point under a class map; part 1's `T8.a at the 3 px switch` check calls
    `pointFlux` on a uniform body, whose path is tested equal bit for bit, and the batch's gate
    runs the whole harness (`/home/quantum/gh/hyperion/.git/rm45-scratch/r10-shader/smart-ci-final.log`).
  - _By hand._ None can run: no live view feeds a class map yet (T10.d's and T10.e's questions,
    now R10.T16's live path). Pending until then, on the development machine and the owner's
    UHD 620: on a surveyed patterned body in `VIEW`, its brightness across the 3 px switch both
    ways, facing its most contrasting face at 0°, 60° and 120° of phase, the meter on `average`.
  - _Generator version._ Untouched (22): presentation alone, no golden.
  - _The batch review (2026-10-19)._ `classMapPoint.test.ts`' tests that failed for more than one
    reason are split, one behaviour an `it`: the photometry's point without a rotation, under a
    uniform surface, and the oriented map dimming it; and the cache reused within δ, within 0.3% of
    a fresh value, and found afresh past δ. `classMapSurface`'s documentation names the map's CPU
    copy as the surface's `texels`.
- **Questions for "main" (R10.T10.f)** (one touches R07's Provides, as the lane lead decided; none
  the server's; each built to its lean).
  - _`texels` on `ClassMapDiscSurface`_ (R07's Provides, additive; the lane lead's decision 1).
    Lean: as built, R07's Provides entry reading `{ kind, weights, texels, laws, elsewhere }`.
  - _The cached value is the ratio._ Decision 4 names the uneclipsed per-light integral; built as
    the per-light ratio to the uniform law with the uniform point fresh, which lets δ bound the
    pattern's drift alone. Lean: as built.
  - _A file of its own._ Lean: as built, the task's Files reading "R07's `bodies/draw.ts` and
    `bodies/discSurface.ts`, and a new `bodies/classMapPoint.ts`".
  - _The switch's test at 64 × 64 samples._ The task's "equals the CPU rasteriser's summed disc
    flux at the switch to 1%" holds with the disc rasterised at the switch's geometry at 64 × 64
    samples a pixel, not at R07.T8.c's 8 × 8, under which the disc errs by up to 2.6% (deferred
    below). Lean: as built, the test's text so amended, and the disc's sampling R07's to revisit.
- **Deferred findings (2026-10-13 rule) (R10.T10.f)** (recorded, not ruled; each leaves the
  algorithm and the contracts as built).
  - _The eclipse's limb weighting is the uniform law's._ Under a patterned map each star's
    `discEclipseVisible` weights the disc by `elsewhere`'s L, not by the classes' A and L where the
    shadow falls. The error is first order in partial phases (the batch review's science check,
    2026-10-19): the product of the pattern's contrast and the eclipse's variation across the disc;
    a sharp shadow over exactly the bright half of a 14:1 body gives the point 7.5 times its light,
    over the dark half 0.54 times. The lever stays a per-element eclipse in the integral, which
    would be exact (the cache then holding the uncut ratio and the eclipse its own factor).
  - _R07's disc at 3 px misses a pattern's edge near the limb._ R07.T8.c's 8 × 8 cells integrate
    the two cells nearest the limb across their profile in √δ, as a uniform law's light needs; the
    Iapetus's bright rim, 0.06 a inside the limb facing the leading side, lies in that band, and its
    albedo step is not followed: the disc is 1.73%, 1.99% and 2.58% under the point facing the
    leading side at 0°, 60° and 120°, and 1.73% facing a pole at 120° (the point within 2 × 10⁻⁴
    of the brute force there; 4.1% at 150°, measured with one element a texel). At 32 × 32 it is
    still 1.1% to 1.7% off, the band about the rim's width; at 64 × 64 the band lies inside it. So
    the 3 px switch steps by up to 0.03 mag for such a body. The lever: R07.T8.c's limb cells
    under a class map (more of them, or each Gauss depth of the profile taking its own shares).
  - _Planetshine from a class-mapped neighbour_ keeps the neighbour's photometry law
    (`litNeighbours` through `bodyReflection`), so a patterned moon lights its planet by its mean,
    not by the face it turns to it. The lever: the neighbour's light through
    `classMapPointReflection` towards the lit body.
  - _A map's first point costs a frame._ The elements' shares, 98,304 `classMapSharesAt` calls,
    take about 300 ms on the render thread the first time a point is drawn under each map the store
    takes, and each light's evaluation 1.65 ms; under time acceleration a fast rotator misses the
    cache every frame (δ at α = 0 is 10⁻³ rad, which an Earth-like day turns through in 14 s of
    game time). The levers: the shares built off the frame (with the map in the worker, or as the
    store takes it), a reconstruction loop cheaper than `classMapSharesAt`'s allocations, and
    uniform texels' elements merged.
  - _Memory._ The elements' geometry, 3.1 MB once, and each map's shares 1–2 MB, against its
    texture's 104–418 KB. The lever: directions found per face from tabulated (u, v), or `f32`
    stores.
  - _T10.e's q for the Mars-like world is not closed by the point integral_ (T10.e's deferred "The
    classes' q is the Regolith row's, not plan 14's"). The integral is linear in the map, so
    averaged over orientations it gives T10.e's closed form Σ aᵢ pᵢ qᵢ ÷ Σ aᵢ pᵢ exactly and adds
    nothing to it, and no client fixture holds the Mars-like world's class map (its field is the
    surface crate's synthetic world; the worker golden is the Moon-like's). By the closed form, from
    T10.a's `albedo/worlds.golden` (dust 98.29%, two glaciers 0.770% each, sediment 0.166%,
    p_V 0.17) with the glaciers clamped low (T10.a's table) to their spans' low ends under Lambert's
    law, the H₂O glacier's `GLACIER_ICE_V` 0.69 and, taking CO₂ as a volatile ice,
    `VOLATILE_GLACIER_V`'s 0.80 (p 0.460 and 0.533), and the dust and sediment on the Regolith row
    (q 0.62600 in V) taking the rest of p: q_V ≈ 0.665, the glaciers 4.5% of p, against plan 14's
    1.08465 on Mars's template (−38.7%). Not checked against the worker's photometry export; T10.e's
    finding stands, for plan 14 and R10.T1.a's rows.
