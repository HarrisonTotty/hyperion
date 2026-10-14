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
pub fn material_at<F: FieldView>(synth: &Synthesiser<'_, F>, cache: &mut SynthCache,
    at: FaceUv) -> Result<Material, QueryHeightError>;
pub const CLIFF_SLOPE: Radians;            // 40°, Design note 1
pub const TRANSITION_HALF_WIDTH: Radians;  // 5°, the visual blend band either side
pub const SNOW_SHED_SLOPE: Radians;        // 60°, Schaerer 1977, p. 286 (T1.a, as built)
/// Presentation weights over a per-patch palette of up to eight materials, each a (form, entry)
/// pair (Design note 3).
pub struct ClassWeights { /* palette: [Option<Material>; 8], weights: [u8; 8] summing to 255 */ }
pub fn weights_at<F: FieldView>(synth: &Synthesiser<'_, F>, cache: &mut SynthCache,
    at: FaceUv, level: BandLevel) -> Result<ClassWeights, QueryHeightError>;
/// The per-body scale on class albedos that makes the body's geometric albedo p (Design note 8).
pub struct AlbedoScale { /* scale: f64, residual: f64 */ }
pub fn albedo_scale(field: &CoarseField, geometric_albedo: f64) -> AlbedoScale;
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

/// What a readout may quote at a point, given the survey resolution the ship holds there.
pub struct SurveyedReading {
    /* elevation: Metres, elevation_sigma: Metres, slope_rms: Radians, slope_sigma: Radians,
       slope_baseline: Metres, resolved: bool (sigma is zero: the survey reaches the band limit) */
}
pub fn surveyed_reading<F: FieldView>(synth: &Synthesiser<'_, F>, cache: &mut SynthCache,
    at: FaceUv, survey: Metres, slope_baseline: Metres)
    -> Result<Option<SurveyedReading>, QueryHeightError>;          // None: unsurveyed
pub struct SlantRange { /* range: Metres, sigma: Option<Metres> (None when grazing),
    grazing: bool, hit: [f64; 3] (body-fixed) */ }
pub fn slant_range<F: FieldView>(synth: &Synthesiser<'_, F>, cache: &mut SynthCache,
    origin: &[f64; 3], direction: &[f64; 3], max: Metres)
    -> Result<Option<SlantRange>, QueryHeightError>;
pub const SLOPE_BASELINES: [Metres; 3];   // 2 m (default), 5 m, 100 m, Design note 7
```

### `bake`

```rust
/// What this plan adds to R05's `PatchBake`.
pub struct TerrainLayers {
    /* palette: [Option<Material>; 8], weights: Vec<[u8; 8]> (65 × 65),
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
/// The face-wide far-field lattice, keyed by position alone (Design note 10).
pub fn far_field_point(face: Face, level: u8, i: u32, j: u32) -> FaceUv;
```

The worker's WebAssembly entry points, on R05's binding, gain `field_init`, `field_chunk`,
`field_release` and a `bake` that takes R05's `BakeOptions` with this plan's settings, and answer
`field_init` with the body's `LevelBounds` (below). They replace R05's single `postField(bytes)`
with these per-body calls. For the readouts they also gain `reading`, `slant_range` and `material`,
the wasm faces of `surveyed_reading`, `slant_range` and `material_at` with `properties` (T6.b).
As built, R05's module (`src/wasm.rs`) serves the test planet alone: `bakePatch`, `levelTable`,
`surfaceHeightM` and `omittedSigmaM`, each taking `Ridges`, which the spike keeps (R09 Design note
18); the per-body exports go beside them, under the same camel-case `js_name` rule.

### Client (`apps/hyperion/src/renderer/src/view/terrain/`)

- `coarseFieldStore.ts`: `CoarseFieldStore` with `want(body, reason)`, `release(body)`,
  `coverage(body): CoverageView`, `onRevision`; it drives R09's `surface_field` and posts to R05's
  `HeightWorkerPool`.
- `coverage.ts`: `CoverageView` (`resolutionAt(cell)`, `surveyedFraction()`, `bestResolution()`),
  `surveyEdges(face, level)` for the edge mark, and the GPU coverage mask per face.
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
against R05, R07, R08 and R09 as built (Risks, "Re-validated at `c6d77cac`"). What R09.T8, T9 and
T19.b provide is not built yet: those names are R09's Provides, **provisional until R09.T8/T9/T19.b
land**, and each is marked so below.

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
  `surface_revisions` (provisional until R09.T19.b lands: no `surface_revisions` exists yet). As
  built, `packages/protocol/src/requests.ts`'s `RequestClient.requestBulk`, taking
  `(body, manifestOf, options)`, hands over whole answers only, each `partial_response` with its
  chunks through `options.onPartial` and the last through `outcome`, so forwarding a block as its
  frames arrive
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
  (`None` for a cell the view does not hold); `height::HeightSample`, with R05's `BAND_LIMIT_M` and
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
  - _Provisional until R09.T8 lands_ (`synth/mod.rs` is `synth.rs`): the full `height_at` (until
    then the base elevation alone, `cache` and `level` unread), `Synthesiser::bake_patch` and
    `height_range_m`, `level_bound_m`, the whole `structure_function` and `unresolved_rms`, and
    `QueryHeightError::NonFinite`. `Synthesiser` implements no `HeightSource` until T3. Main has
    accepted three as-built changes for T8 (Risks, "Re-validated at `c6d77cac`"): the bound is hard
    in its octaves and channels and a 1 − 10⁻⁹ per-point Chernoff quantile in its crater and rim
    terms; the structure function holds within 5% on belt cells only, channelled plains running
    15–37% high; and the bench is provisional at 14 µs a point and 77–82 ms a patch at L + 4.
  - _Provisional until R09.T9 lands:_ the golden heights in `tests/golden/height/`, which this
    plan's material and ground goldens follow.
  - _Provisional until R09.T19.b lands:_ the `surface_field` payload (the handler answers
    `unsupported` until then), its revisions and deltas since `have_revision`, and
    `surface_revisions` on the scene topic.
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
   and the variance of the slope in the bands finer than n, from R09's per-cell closed forms
   (`unresolved_variance` and `structure_function`, per contribution); for the rules on altitude,
   the same with the height variance. At the finest level the weights are the class, blended
   linearly across `TRANSITION_HALF_WIDTH` either side of each threshold for the image only, and
   their largest weight is always the authoritative class, ties broken by discriminant. A test holds
   the weights at every coarser level to within ten percentage points of the fractions counted by
   brute force from the finest level. This is the plan's reading of "the shader blends the class's
   textures across a transition band but never re-decides the class": the worker decides everything,
   and the shader only mixes the photometric parameters it was handed.
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
   of the omitted bands, σ from R09's closed forms. Over a patch's 4,225 vertices, taken as
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
   by main for R09.T8 (provisional until it lands): `level_bound_m` is hard in its octaves and
   channels, and its crater and rim terms are a 1 − 10⁻⁹ per-point Chernoff quantile, so "the hard
   bound" here reads as that stated bound; the finding is in Risks ("Re-validated at
   `c6d77cac`")._
6. **Knowledge gates coverage for the image, and coverage and resolution for every number**
   (researched 2026-09-29). The client holds exact cells wherever it holds any, so the image of
   surveyed ground is the ground at whatever detail the camera resolves; the survey's resolution
   never changes a drawn height. What the resolution changes is what a readout may quote.
   `surveyed_reading` evaluates the height at the band level matching the survey resolution of the
   point's cell and states as its uncertainty R09's `unresolved_rms`, one standard deviation of
   every contribution finer than that. It is 1σ and not an interval such as LE90 (1.6449σ, a
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
   (Kreslavsky and Head 2000), and the adirectional one is what tilts a lander. Each unresolved band
   of variance σ_k² at wavenumber k adds per axis v = Σ 2 σ_k² (1 − J₀(k b)) ÷ b², the structure
   function at lag b (Shepard et al. 2001), which tends to 2σ_k² ÷ b² for bands much finer than b
   and to the band's gradient variance for bands much coarser; craters and channels take R09's
   `structure_function` at lag b, not only a variance. The slope magnitude is then Rician
   about the resolved slope |g₀|, biased upward when |g₀| is below about 2√v, so the readout quotes
   the RMS slope √(|g₀|² + 2v) with the Rice standard deviation as its uncertainty; quoting |g₀|
   alone would understate slopes from orbit, the unsafe direction. The baseline is shown with the
   value: 2 m by default, the band limit and a lander pad's scale, with 5 m and 100 m selectable,
   the ladder InSight's site selection used (lander-stability slopes at 2–5 m from HiRISE DEMs
   against a 15° limit, and 300 m scale from MOLA: Golombek et al. 2014, LPSC 45, 1499, verified).
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
   Φ_shape(α; L) = [L(α) Φ_LS(α) + ⅔(1 − L(α)) Φ_Lam(α)] ÷ [L(0) + ⅔(1 − L(0))], so p and q are
   unchanged and only the limb profile moves; `body_brdf` reads L from the row and `LunarLambert.l`
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
    shared edge are the same point with the same value and no seam can open. Steps grow by 1.15–1.25
    from one texel beyond the patch edge, the curvature drop d² ÷ 2R is included, so an obstacle at
    d stands at arctan((h − h₀ − d² ÷ 2R) ÷ d), and the march stops at √(2 R Δh), Δh the body's
    realised relief range: about 505 km for an Earth with 20 km of relief. The shader takes the
    larger of near and far. The lit fraction is the share of the star's disc of angular radius ρ
    above the horizon, 1 − (arccos x − x √(1 − x²)) ÷ π with x = (e_sun − e_horizon) ÷ ρ clamped to
    [−1, 1] (checked: 0, ½ and 1 at x = −1, 0, 1), without limb darkening, which moves the
    half-light point by under 0.1ρ. On terrain R07's horizon term (`sphere_irradiance`, its Design
    note 6) takes its local horizon per disc sample from this map, through the `horizon` argument of
    R07's `sphere_irradiance(h, phi, horizon)` (R07.T6.a, T6.c; 0 on the smooth figure), with R07's
    oracle as its test oracle; T8.b fills it. Where R08 draws the sun at its refracted elevation (on
    an Earth about 0.57° at the horizon, more than the Sun's disc, and 0.16° at 5°; Bennett 1982),
    the shadow test uses the same apparent elevation, or shadows and the drawn sunset disagree by
    more than a solar diameter (as built R08 draws no refraction, so the test takes the geometric
    elevation until it does, R08's Risks). Max, Sloan and Cohen, Stewart 1998, Timonen and
    Westerholm and Bennett are from memory in the research; T8.a's bench and the by-hand sunset
    settle what they support.
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
    min(horizon, cascade) where a cascade exists, so cascades add what the horizon map cannot,
    casters that are not terrain (craft, R11's rocks and scatter) and sub-texel detail near the
    camera, and there is no seam at the last cascade, which only fades to lit over its last 10%.
    Cascades switch off when cascade 1's texel is coarser than the horizon map's, as in orbit.
    Splits follow Zhang et al. 2006's practical scheme, Cᵢ = λ n (f ÷ n)^(i ÷ N) + (1 − λ)(n + (f −
    n) i ÷ N) with N = 4 and λ ≈ 0.75, over a shadowed range f = min(view far, max(2 km, 20 × height
    above ground)), continuous in that height. Each cascade is fitted to a bounding sphere of its
    frustum slice, whose radius does not change as the camera turns, snapped to whole texels
    (Valient 2008), with every light matrix built camera-relative in `f64` and narrowed to `f32`,
    since snapping at planetary coordinates would jitter. An orthographic light projection has
    linear depth, so reversed-Z buys nothing there: the maps are `depth32float`, whose worst step
    over a 50 km range is about 3 mm, and keep the project's greater-equal convention for uniformity
    only, which the code says so that no one tunes bias expecting precision. Casters in front of the
    near plane are pancaked through `primitive.unclippedDepth` where the adapter offers
    `depth-clip-control` (to be confirmed on the UHD 620 through R01's feature report), and
    otherwise the near plane is pulled towards the sun by Δh ÷ sin(e_sun), capped. Acne is held by
    slope-scaled bias (`depthBias`, `depthBiasSlopeScale`, `depthBiasClamp`) and a normal offset of
    1–1.5 texels of the cascade's world-space size; filtering is 2 × 2 hardware PCF through
    `textureSampleCompare` with a few Poisson taps, since the Sun's penumbra at 10 m from an
    occluder is about 9 cm and PCSS is unneeded. Four 2,048² `depth32float` layers are 64 MiB, the
    brainstorm's figure. Zhang et al.'s formula, Valient's snapping and the bias recipes are from
    memory in the research and are checked against the sources in T9.a.
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
    heights, the horizon map and the mask, all read by `textureLoad`, live in storage buffers
    addressed by slot index, tight and untiled (the default `maxStorageBufferBindingSize` is 128
    MiB); normals and class weights, which want filtering, live in 2D arrays (at most the adapter's
    `maxTextureArrayLayers`, 256 by default) or atlases with a one-texel gutter; and each style's
    terrain is one instanced draw over R05's shared 65 × 65 index buffer with a slot index per
    instance, which also keeps the CPU's per-draw cost down. R05 already packs its octahedral
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
    decodes one with `wire::decode_block` (R09's "Deviations in T3, as built")._

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
- **T4.b** builds `levelBounds.ts` and the selection rule after T6.b, on a fixture from
  `synthetic_worlds()`, and records the owner's counts after T4.a's `level-bounds.json`.
- **T6.a's bench** waits on R09's follow-up after T8, the fused interpolant pass that main has
  queued before this plan's quiet bake timing; meanwhile its figure is recorded provisional.
- **New T1.c** (decision-composition) follows T1.b; its cover term waits on P14.T51.d.
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
  ground. Files: `material/classify.rs`, both helper modules
  of Test helpers, `tests/material_golden.rs` with its `golden/material_*.golden` (a thousand points
  on each of `synthetic_worlds()`), and `crates/hyperion-sim/tests/surface_material_worlds.rs`.
  Tests: a `cliff_field` is `Bedrock` above 40° and not below; on `sample_worlds()`, a warm pole
  has no ice cap and a locked world's ice lies where R09's field puts it (the brainstorm's two
  examples under Materials); `Seabed` exactly below the water surface; an arid filled basin is
  `Evaporite`; the Pluto-like and lava-world `FieldBuilder` worlds of decision-composition take
  their frost and `Melt`; the order of rules is total; the goldens match on native,
  `wasm32-wasip1` and `wasm32-unknown-unknown` through R04's checks. The `sample_worlds()` cases
  follow R09.T16 (the order of tasks). Bumps `GENERATOR_VERSION` (see Generator version).
  Acceptance: `cargo test -p hyperion-surface material` and
  `cargo test -p hyperion-surface --features testing --test material_golden`,
  `cargo test -p hyperion-sim --features testing --test surface_material_worlds`, and
  `just test-wasm-fast` runs the goldens on both wasm targets (the orchestrator's `just ci` runs
  all three).
- **R10.T1.c Maturity and cover.** New at the re-validation (decision-composition §1.8 and
  §7.5). Regolith's A_N and colour are modulated by exposure (crater and surface ages, from R09's
  header and cells) and by plan 14's P14.T51.d cover. Files: `material/maturity.rs`. Tests: a
  fresh crater's ejecta is
  brighter than mature regolith by the Moon's measured contrast; zero cover gives the substance's
  own albedo. The cover term waits on P14.T51.d (the 23 → 24 batch); until then the exposure term
  lands alone. The contrast is checked against its primary before it is a constant. Acceptance:
  `cargo test -p hyperion-surface material::maturity`.

### R10.T2 Class weights for coarser levels

`ClassWeights`, `weights_at` (Design note 3), with the slope-exceedance probability from R09's
`unresolved_variance` and `structure_function`, and the palette of up to eight classes a patch
keeps. As built, `unresolved_variance` and the channels' and craters' own structure functions
exist, and the whole `structure_function` is R09.T8's (provisional until it lands); the closed
forms are held once per body, since R09's crater octaves cost about a millisecond to build and the
per-cell forms rebuild them (R09's "Deviations in T7.b, as built"). Files: `material/weights.rs`,
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
and `Synthesiser::height_range_m(patch)`, and `figure()` the header's (provisional until R09.T8
lands). R05's interpolant returns the height alone, so this task adds beside it, in
`patch::collision`, the query that also gives the triangle, with `finest_surface_height` its
height. Files:
`ground.rs`, R05's `patch` module, `tests/ground_golden.rs` with `golden/ground_*.golden`. Tests: at
a vertex `ground_at` equals R09's `height_at` there bit for bit; on an analytic plane field it is
exact; points on a shared edge get the same height from either triangle; points either side of a
cube-face edge and at a cube corner agree to 1 µm; the normal is the triangle's; the chord sagitta
of Design note 4 (L² ÷ 8R at 0.5 m on 6,371 km, about 5 nm) keeps face-coordinate and spatial
interpolants within 1 µm; `ground_at` is finite at every sampled point of `synthetic_worlds()` and
of `sample_worlds()` (the latter in `crates/hyperion-sim/tests/surface_ground_worlds.rs`), and
asserts finiteness before it returns (the brainstorm's NaN hazard). The goldens, on
`synthetic_worlds()`, are asserted on native and both wasm targets. No bump: the ground reads
heights that R09's goldens already pin (R09.T9's, provisional until it lands). Acceptance:
`cargo test -p hyperion-surface ground`,
`cargo test -p hyperion-surface --features testing --test ground_golden`, and
`cargo test -p hyperion-sim --features testing --test surface_ground_worlds`.

### R10.T4 The level bound on generated worlds

- **R10.T4.a The bound measured.** R09's `level_bound_m` on `sample_worlds()`, the real coarse
  pass's output rather than R09's synthetic fields, with σ per level from R09's closed forms (Design
  note 5). Files: `crates/hyperion-sim/tests/level_bound_worlds.rs`, which also writes each world's
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
  `LevelBounds` (hard and σ per level); `levelBounds.ts` holds them per body and `selectionBound`
  gives R05's `screenSpaceErrorPx` its ε_n under the rule in force (the hard bound until the owner's
  ruling), for each style's τ: 1 px high and 2 px low (the brainstorm's lean; CesiumJS's default
  `maximumScreenSpaceError` is 2 px, though its error is a spacing heuristic), and 4 px wireframe.
  As built, selection reads the bound column of the body's level table (`PlanetGeometry.levels`,
  stride 4), and its error is that bound plus R05's chord sagitta (`selectionErrorM`), so
  `selectionBound` fills that column and `field_init`'s answer carries the rest of the table, the
  lowest and highest height by level that culling reads before a patch is baked (lean: the union of
  the six root patches' `height_range_m`, R09.T8's). The test planet keeps its own `levelTable`.
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
6 and 7), over R09's `unresolved_rms`, its `structure_function` at lag b and the coverage
resolution per cell (`PartialField::resolution`), with J₀ through base's `math::j0` (R04.T4.c).
As built: `unresolved_rms` and the whole `structure_function` are R09.T8's (provisional until it
lands), held once per body as T2's are; `PartialField::resolution` is `Some(ResolutionCode::NONE)`
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
region before it hits gives `None`. Acceptance: `cargo test -p hyperion-surface reading`, and the
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
  `c6d77cac`: `Synthesiser::bake_patch` is R09.T8's (provisional until it lands), whose own bench
  is provisional at 77–82 ms a patch and 14 µs a point at L + 4, so this bench's quiet figure
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
holds R05's field today. The payload's bytes, its deltas and
`surface_revisions` are provisional until R09.T19.b lands (the server answers `surface_field`
`unsupported` until then): the tests drive them through the fake socket in R09's Provides' shapes.
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
  patches and across levels; the far field is independent of the cache's state, with bakes in any
  order; the march stops at √(2 R Δh). Bench `bake/horizon`: at most a quarter of the patch's 40 ms,
  recorded; over it, the near field moves to the line sweep. Presentation-only: no golden across
  targets, though the bake is deterministic. The far field's level-m heights cost R09.T8's
  per-point figure (provisional at 14 µs, against the 2–4 µs Design note 10's estimates assumed),
  which the bench records. Acceptance: `cargo test -p hyperion-surface bake::horizon` and the
  bench's figure recorded (`just bench -- bake/horizon`, in T6.a's `benches/bake.rs`).
- **R10.T8.b The shadow lookup.** `shadows/horizon.wgsl`: the sun's apparent direction in each
  patch's local frame from `body_fixed_at`, R07's star and R08's refraction where drawn; the compare
  against the interpolated horizon, passed as the `horizon` argument of R07's
  `sphere_irradiance(h, phi, horizon)` (the signature and its default of 0 on the smooth figure are
  R07.T6.a's and T6.c's, used unchanged); the soft lit fraction of Design note 10. The sun's
  elevation is π ÷ 2 less its zenith angle `preciseAtan2(|s × ν|, s·ν)`, since as built
  `preciseAtan2(y, x)` takes y ≥ 0 and returns [0, π], and the lit fraction's arccos x is
  `preciseAtan2(√(1 − x²), x)` (R08 Design note 18). R08 draws no refraction yet, so the sun's
  direction is the geometric one. R07's `sphere_irradiance` clamps a negative horizon (a peak's)
  to 0, so a peak's extended-source term is the flat ground's (Risks). WGSL's `asin` inherits
  atan2's 4,096 ULP, about 50″ at 30°. That is five of the map's steps, and 3.3% of the Sun's lit
  fraction at mid-transit, against this plan's 1/255. Files: `shadows/horizon.wgsl`,
  `shadows/litFraction.ts`, and the call
  sites in R07's `shaders/litBody.wgsl`. Tests: with a horizon of 0 the terrain's
  `sphere_irradiance` equals R07's smooth-figure value bit for bit, and with the horizon map's value
  it equals R07.T6.a's oracle for that local horizon to 10⁻⁶; the lit-fraction formula as a pure
  TypeScript twin against hand values (0 and 1 beyond ±ρ, one half at e_sun = e_horizon); R01's
  smoke harness compiles and runs the pass on SwiftShader with and without f16, every texel finite.
  By hand, recorded: a sunset over a generated range on the low setting, on the development machine
  and, by the owner, on the UHD 620, whose GPU time is the one held against the budget's 0.3–0.5 ms
  (Performance budget, Shadows row), and whether eight azimuths show missing peaks. Acceptance:
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
  puts under `GENERATOR_VERSION`, so this task bumps it (see Generator version). As built
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
  p is `not_modelled` (the `surface_field` answer's half provisional until R09.T19.b lands; until
  then the server test reads `SurfaceService::field`'s header; the `sample_worlds()` binary follows
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
  is `PhaseLaw::limb_parameter(c A_N)` of the class's law, and the class laws' rows are three:
  Regolith, Ice, and Lambert (f ≡ 1, alpha 0), from R07's `lambert` template, provisional and
  labelled as R07 has it; a consolidated class's row is Lambert's whatever its entry's code._ The
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
  changes no q, p or point flux. Files: `terrainMaterial.ts`, `shaders/terrainLit.wgsl`; R07's
  `appearance/law.ts`, `shapes.ts`, `phase.ts`, `brdf.ts` and `templates.ts`, `bodies/oblate.ts` and
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
  under it, the Moon template's p, q and point flux equal the constant-L law's to 10⁻⁴, and its
  CPU-rasterised disc flux equals its point's to 1% at 30°, 90° and 120°. Acceptance:
  `pnpm --filter hyperion test` (the client suite: the Files reach R07's `appearance/`, `bodies/`
  and `shaders/`, which the view's `lighting`, `photoreal`, `post` and `scenes` tests also read) and
  `just test-render` (the WGSL).
- **R10.T10.c The survey edge.** The survey mask's discard, the skirts at the survey edge, and the
  reference surface drawn beneath with the inverse mask from the GPU coverage mask (Design note 12),
  in `terrainLit.wgsl` and the `mesh`-regime body's material; the edge mark over the image from
  `surveyEdges`, the guide's `SURVEY EDGE`. Files: `shaders/terrainLit.wgsl`, `coverage.ts`, R07's
  `mesh`-regime material (`bodies/smoothMesh.ts`, `shaders/smoothMesh.wgsl`, `SMOOTH_MESH_MATERIAL`
  in `bodies/draw.ts`). Tests: `surveyEdges` returns exactly the boundary between surveyed and
  unsurveyed cells on a fixture coverage, margin cells counted unsurveyed; the smoke harness renders
  a half-surveyed patch set with no fragment where both terrain and the reference surface write. By
  hand, recorded: the skirt heights seen at the edge on a flattened generated world, which stay
  within the terrain's local relief. Acceptance:
  `pnpm --filter hyperion exec vitest run src/renderer/src/view/terrain` and the smoke run.
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
- **R10.T10.e The disc hands over.** R07's `disc` regime gives way to the quadtree at the range of
  Design note 13, per view and style. Tests: the handover range as a pure function (about
  1.8 × 10⁷ m for 10 km of relief at 1080p and 1 px, and 4.6 × 10⁶ m at the wireframe's 4 px), and a
  body with no survey never hands over; per class law, the disc's and the terrain's I/F at the same
  (μ₀, μ) agree to 10⁻⁶ at 30° and 90° of phase, one row read by both. By hand, recorded: at phase
  angles of about 30° and 90°, the body's mean brightness across the handover changes by less than
  1/3 stop and, averaged over each class-map texel, by less than 1/3 stop on a fully surveyed
  generated world; the residuals of T10.a against plan 14's p and q. Acceptance:
  `pnpm --filter hyperion exec vitest run src/renderer/src/view/terrain` and the recorded check.
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

### R10.T11 The wireframe's terrain

Depth-only terrain in the wireframe style at τ = 4 px from the same bounds and bakes (heights and
mask only), as one instanced draw, with the contours of Design note 14 evaluated in the same pass,
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
a substance not computed), and `RESOLUTION`. The material's properties (bearing capacity and pad
friction, with their footing width and temperature) and a mark at the line of sight, of a shape no
other mark uses, are drafted in this task's entry for a sign-off before either is built; the rest of
the task does not wait for them. The DOM list notes the survey edge while it is in view, as
`SURVEY EDGE` with its nearest range, a note that cannot be selected. The task also renames the
descent spike's `SITE HEIGHT` to `SITE ELEVATION`, in `ELEVATION`'s form, and makes the guide's row
edit in the same commit (signoff-9). Build to the guide's rows as signed off on 2026-10-13
(`signoff-9.md`). As built, the label block is `viewRun.ts`'s `labelStatements` and
`ViewLabelBlock.tsx`, and the DOM list `ViewMarkList.tsx` over `viewRun.ts`'s `MarkRow`; the
material is named by its form and its entry's substance through P14.T49.f's `substanceName`
(`lib/substances.ts`; the roadmap's Conventions), never by a registry key. Files: `readouts.ts`,
`SurveyLine.tsx`, `TerrainReadout.tsx` and their tests; `displays/view/viewRun.ts` and
`ViewMarkList.tsx`; `view/spike/DescentSpike.tsx` and its test. Tests (Vitest and Testing Library):
the formats of T13's drafts for each state, `~2140 m ± 180 m` among them; the PDG rounding at its
breakpoints (354, 355, 949, 950), and the round-up to one step where it would go finer than the
quantity's precision (0.12 m → `± 0.2 m`); no value is shown finer than its uncertainty; the field
keeps its fixed width, and the value's unit its place, from Estimated to resolved and stale;
`SITE ELEVATION` in `ELEVATION`'s form; stale on link loss; keyboard reach of the readout's target
through the DOM list; the console-ux skill's lint, contrast and glyph checks pass, with no `σ`
anywhere (B612 has none). Acceptance:
`pnpm --filter hyperion exec vitest run src/renderer/src/view/terrain` and the console-ux checks.

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
settings with its metrics harness, recording patches a second sustained against the brainstorm's
demand of (200 · v + 290 · |ḣ|) ÷ h, the level bound per level from T4.a, and pops or cracks seen;
every golden of this plan asserted on the three targets. Acceptance: the lane's `just smart-ci`
and `just test-wasm-fast`, this plan's slow binaries by name (`just test-slow -E 'binary(…)'`) and
`just bench -- bake/` complete, and the descent's figures are recorded; `just ci` and the slow
pass are the orchestrator's.

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
  order-independent, the horizon far field bit-identical at shared points (T6.a, T8.a).
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
generator-versioned too. The weights, the class map, the bounds' use in selection, the readings and
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
  whether sixteen are needed, at twice the horizon memory.
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
    (Design note 10) keeps the signed horizon. Small; R07's file.
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
    whole instead.
  - _Coverage lags the workers._ A payload's blocks reach the workers before its answer brings the
    coverage. A payload cut off by the link leaves blocks in the workers that the coverage never
    names, until the next answer covers them again. T10.c, which bakes the survey mask, should take
    it from the store's coverage, or from a cover T6.b reports, and say which.
  - _The fraction on a spheroid._ `surveyedFraction` is by solid angle from the centre; on a
    spheroid the share of its area differs by about its flattening (Earth's 1 ÷ 298). Small.
  - _The re-bake on a field's start._ Starting any body's field re-bakes every bake in flight, as
    `postField` did. Once bakes name their body (T6.b), the epoch belongs to the body.
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
