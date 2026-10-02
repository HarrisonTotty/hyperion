# Plan R11: Decoration, Scatter, Clouds, Oceans, Rings and Still Images

- **Milestone:** Rendering milestone RM6, with R12.
- **Depends on:** [R08 Atmospheres](08-atmospheres.md) and [R10 Terrain on generated
  worlds](10-terrain-on-generated-worlds.md), and through them R01–R07 and R09. Galaxy plan 14
  ([planetary systems](../galaxy-generation/14-planetary-systems.md)): its rings (P14.T20, built),
  rotation and poles (P14.T14), surface conditions and global figures (P14.T24), and a ring radial
  profile asked of it here. Sessions, ship state and a flight model, which have no plan yet, are not
  needed to build this plan; they are needed before anything collides with its rocks.
- **Brainstorm sections covered** (by heading, in
  [the brainstorm](../../brainstorming/rendering-and-planets.md)): "The line between truth and
  decoration" (decoration, scatter and the authoritative rocks); the scatter, triplanar and
  repetition bullets of "Materials, and what a surface looks like"; "Clouds"; "Oceans and ice";
  "Rings"; "Still images"; the transparent-layer and ocean-thickness rules of "Depth: reversed-Z,
  and no logarithmic depth"; the clouds, ocean, scatter and rings rows of "Performance budget" and
  of its ladder; item 2's honesty rule and flash threshold under "What the guide must gain"; the
  Gerstner, ring-table, extinction and stills items of "Testing"; open questions 7, 8, 11 and 18;
  step 9 of "Suggested order of attack".

## Goal

When this plan is done a generated world, drawn in the photorealistic style, has everything the
brainstorm's step 9 lists. The ground carries GPU decoration below the 2 m band limit, and the view
says when it is on. Rocks 0.4 m across or more, which covers every rock 0.2 m tall, are
authoritative: `hyperion-surface` places them by hash in cells per size octave, from Golombek and
Rapp's rock size–frequency law with an abundance set by the surface's type and age, with analytic
shapes that the same crate answers contact queries against, bit for bit on native and both
WebAssembly targets; they are drawn wherever they could touch a grounded or descending body, on
every setting and in both styles. Smaller scatter is decoration, drawn on the high setting only and
culled where it meets a body. Clouds are a raymarched volume on the high setting and a
two-dimensional layer on the low one, sharing one optical-depth field and one δ-Eddington optics,
built together, drawn over the whole body from a terrain-free climatology refined where the ground
is surveyed, and scaled once to plan 14's cloud fraction. Oceans are recognisable from orbit by a
sun glint computed from Cox–Munk wave-slope statistics that the climate field's wind sets, become
Gerstner waves nearer the surface under one slope budget, meet the shore with foam and a depth fade,
and give way to sea ice where the coarse pass put it. Rings are a vertically non-uniform particle
layer with a power-law phase function and an opposition surge, a baked shadowing factor between
particles, both analytic shadows and a radial profile, giving way to instanced particles once the
largest subtends a pixel. A console can ask for a still image, rendered tile by tile through the
offscreen pipeline beside the live view, labelled as data and saved with its label, and a soak at
the batch cap on the development machine and on the UHD 620 loses no device. Open questions 7, 8 and
11 are built to their leans, and open question 18 is ruled by this plan, pending the owner's
revision of the brainstorm: craters of 1–2 m stay decoration.

## Scope and non-goals

In scope:

- GPU decoration on terrain (normal detail, colour variation, sand ripples, crater scars, and the
  triplanar detail textures with their repetition hiding, which R10 hands over as decoration in its
  Design note 9), its setting and its `DECORATION ON` label.
- `hyperion_surface::scatter`: rock abundance, placement, shapes and the contact query; its goldens
  on three targets; its generator-version bump.
- Rocks in the client: baked with patches through R10's `BakeHook`, instanced, drawn under the rule
  of Design note 7, checked against the contact query.
- Decorative scatter below 0.4 m across: GPU placement, filtering, distance fall-off, the cull
  against bodies near the ground; high setting only.
- Open question 18's ruling, recorded.
- Clouds in both settings, with their direct shadows and diffuse light on terrain.
- Oceans: sea state, glint from space, the near-surface wave field, the shoreline, sea ice's
  hand-over to the terrain material.
- Rings: photometry, the baked shadowing table (an offline Monte Carlo), both analytic shadows, the
  radial profile's texture, the far annulus for R07's analytic-disc regime, and the instanced
  particles close to.
- Still images: the progressive offscreen job, its label, its file store through the main process,
  the console control, the soak.
- Each feature's own low setting and its own recorded benchmark, as the budget's third rule
  requires.

Non-goals:

- Vegetation. "Vegetation follows only where a later biosphere says it exists", and planet content
  is owed its own document. The instancing here takes a vegetation class later without change.
- Collision itself. The contact query is provided and tested; the flight model that calls it has no
  plan.
- Authoritative 1–2 m craters (open question 18's other branch, Design note 20), and an ejecta-block
  population keyed by coarse craters, larger than the background law's rocks (Risks).
- The ring radial profile's generator: it is plan 14's (asked below). This plan draws a provisional
  profile from what plan 14 already sends until then (Design note 13).
- Cloud decks: a body whose cloud fraction is 1 and whose deck's optical depth exceeds 10 is R08's
  split, with any thin layer above it. Gas giants' banded cloud tops (Design note 9).
- A spectral FFT ocean. The brainstorm calls it "the WebGPU upgrade" and "not where the first effort
  goes"; Design note 12 keeps its door open. Tides (Design note 11).
- Exporting stills beyond the client's own still folder (a file dialog, a shared gallery), and a
  linear half-float master of a still.
- The consolidated performance runs, the resident-memory run and the ladder's adjustment: R12's.

## Provides

Rust paths are under `hyperion_surface` unless a crate is named. Signatures are sketches; each is
fixed by its task and recorded as built. Positions in the surface crate are `BodyFixed`, an alias of
`[f64; 3]` in body-fixed metres, as R05's `origin` is: the crate depends on `hyperion-base` alone,
and `BodyFixedPosition` is the sim's (`hyperion_sim::coords`), so the sim and the client wrap the
alias as `BodyFixedPosition`. The queries take R09's `Synthesiser` and `SynthCache`, since a rock's
base stands on R10's `ground_at` and its site reads R10's `material_at`, and they return R10's
`QueryHeightError` where those can fail.

### `hyperion_surface::scatter`

```rust
/// The height at and above which a rigid instance must be authoritative: 0.2 m (brainstorm, open
/// question 11). A generator constant.
pub const AUTHORITATIVE_ROCK_HEIGHT: Metres;
/// The diameter from which every rock is authoritative: AUTHORITATIVE_ROCK_HEIGHT ÷ the largest
/// drawn height ratio, 0.2 m ÷ 0.5 = 0.4 m (Design note 4).
pub const AUTHORITATIVE_ROCK_DIAMETER: Metres;
/// The exposed height-to-diameter ratio's range, drawn per rock: 0.25–0.50.
pub const ROCK_HEIGHT_RATIO: (f64, f64);
/// Golombek and Rapp 1997's rock abundance k at a site, clamped to [0, 0.40].
pub struct RockAbundance { /* k: f64, the fraction of the area covered by all rocks */ }
impl RockAbundance {
    pub fn q(&self) -> f64;                                          // 1.79 + 0.152 ÷ k, per m
    pub fn cumulative_area(&self, diameter: Metres) -> f64;          // F_k(D) = k exp(−qD)
    pub fn cumulative_number_per_m2(&self, diameter: Metres) -> f64; // N(>D), closed form in E₁
}
pub fn exponential_integral_e1(x: f64) -> f64;   // series below 1, continued fraction above
/// Airless below plan 14's `AIRLESS_PRESSURE` (100 Pa, the sim's own threshold for
/// `SurfaceState::Airless`), otherwise with an atmosphere (Design note 4).
pub enum AtmosphereClass { Airless, Atmosphere }
pub fn atmosphere_class(surface_pressure: Pascals) -> AtmosphereClass;
/// What decides k at a point, from inputs both sides hold (Design note 4): the surface age and the
/// atmosphere class come from R09's `FieldHeader.surface_age` and `FieldHeader.surface_pressure`.
pub struct RockSite { /* material: Material (R10's), surface_age: Gyr, atmosphere: AtmosphereClass,
    nearest_crater: Option<(CoarseCrater, distance: Metres)> */ }
pub fn rock_site<F: FieldView>(synth: &Synthesiser<'_, F>, synth_cache: &mut SynthCache,
    at: BodyFixed) -> Result<RockSite, QueryHeightError>;
pub fn rock_abundance(site: &RockSite) -> RockAbundance;
/// A rock: an ellipsoid or low-order superquadric standing on the authoritative ground.
pub struct Rock { /* base_centre: BodyFixed, diameter: Metres, height_ratio: f64,
    semi_axes: [Metres; 3], yaw: f64 rad, exponent: f64 (2 = ellipsoid),
    burial: Metres (depth of the base below the ground) */ }
impl Rock { pub fn exposed_height(&self) -> Metres;
            pub fn top_above(&self, at: BodyFixed) -> Option<Metres>; } // radial
/// A placement cell: face, the octave's quadtree level (R09's `level` slot) and integer cell,
/// keyed through R09's fallible `ObjectKey::surface_cell` (the body is in the detail seed).
pub struct RockCellKey { /* face: u8, level: u8, i: u32, j: u32 */ }
impl RockCellKey {
    pub fn object_key(&self, instance: u32) -> Result<ObjectKey, SurfaceCellKeyError>;
}
pub const ROCK_OCTAVES: [(Metres, Metres); 4];  // 0.4–0.8, 0.8–1.6, 1.6–3.2, 3.2–6.4 m
pub const ROCK_OCTAVE_LEVELS: [u8; 4];            // one quadtree level per octave (Design note 5)
pub enum RockError { Query(QueryHeightError), Key(SurfaceCellKeyError) }
pub fn rocks_in_cell<F: FieldView>(synth: &Synthesiser<'_, F>, synth_cache: &mut SynthCache,
    cell: RockCellKey, out: &mut Vec<Rock>) -> Result<(), RockError>;
/// The caller's cache of cells, as R09's `SynthCache` is: order-independent.
pub struct RockCache;
pub fn rocks_near<F: FieldView>(synth: &Synthesiser<'_, F>, synth_cache: &mut SynthCache,
    cache: &mut RockCache, at: BodyFixed, radius: Metres, out: &mut Vec<Rock>)
    -> Result<(), RockError>;
/// The authoritative ground with its rocks: max(R10's `ground_at`, every rock's top) at a point.
/// A cell that is not surveyed on the client is `Err(Query(NotSurveyed))`, never "no rocks there".
pub fn surface_height_with_rocks<F: FieldView>(synth: &Synthesiser<'_, F>,
    synth_cache: &mut SynthCache, cache: &mut RockCache, at: BodyFixed)
    -> Result<Metres, RockError>;
/// The rock list a finest patch carries through R10's `BakeHook` (R10's `TerrainLayers::extra`).
pub enum ExtraLayer { Rocks(Vec<Rock>) }
/// R10's `BakeHook`, implemented here. It owns its `SynthCache` and borrows the worker's
/// `Synthesiser`, so R10's `bake(&self, key, heights, out)` needs no further argument.
pub struct RockBakeHook<'a, F: FieldView> { /* synth, synth_cache: RefCell<SynthCache> */ }
```

The detail seed is read from the `Synthesiser`, which holds the field and R09's `DetailSeed`, so it
is not a separate argument.

Domain tags, in R09's `SurfaceDetail` scope (opened only by `DetailSeed::stream`), under a
"Rendering plan R11" heading after R09's in the surface crate's registry, `hyperion_surface::tags`:
`surface.scatter` (count, position and diameter per cell; reserved by R09 for this plan) and
`surface.scatter.shape` (height ratio, axial ratios, yaw and exponent per rock, keyed with the
rock's index as the key's `instance`).

### Offline table (`hyperion-fit`)

A `ring-shadowing` subcommand whose Monte Carlo of a vertically non-uniform particle layer emits the
ring shadowing factor as a committed client module under 50 kB, with a header naming the tool, its
inputs, its version and its acceptance figures (Design note 15).

### Client (`apps/hyperion/src/renderer/src/view/`, and `src/main`, `src/preload`)

Pure modules are unit-tested without a GPU, as `spatial/` is; each WGSL shader beside them has the
pure module as its reference.

- `surface/decoration/`: `decorationSetting.ts` (the ladder entries and whether decoration is
  drawn), `decorationLattice.ts` (the integer lattice hash the WGSL mirrors), `decoration.wgsl` and
  `detailTextures.wgsl` (triplanar, or biplanar on the low end, with two-scale repetition hiding),
  both behind R10's `terrain_decoration` hook; `DECORATION_BAND_LIMIT_M`, R05's `BAND_LIMIT_M` read
  through R05's `bandLimitM()` in `view/terrain/planet.ts`, over the wasm export `band_limit_m()`
  (R05.T5, T7.a), and never redefined.
- `surface/scatter/`: `rockLayer.ts` (the `ExtraLayer` it fills through R10's `BakeHook`),
  `rockMesh.ts` (`tessellateRock(rock, level)`, `chordalBound(rock, level)`), `rockSelection.ts`
  (`rocksToDraw(patches, bodies, setting, style)`, `couldTouch(body, path)`,
  `extrapolationMargin(gravityMps2, tS)`, the widening of a sweep along an extrapolated path, and
  `couldTouchContacts(bodies, sweepS)`, the chain of `GroundContact`s it hands R05's selection),
  `rock.wgsl` (the instanced rock draw), `pebbles.wgsl` (decorative
  placement), `scatterCull.ts` (`intersectsBody(instance, bodies)`) and `scatterCull.wgsl`, its
  twin.
- `clouds/`: `cloudOptics.ts` (`deltaEddington({ tau, asymmetry, singleScatterAlbedo, mu0 })` →
  reflectance, direct and diffuse transmittance, absorptance), `condensates.ts` (stand-in optics
  until R08's `modeOptics` has a condensate mode to read), `coverage.ts`
  (`coverageField(zonal, surveyed, globalFraction, month)`, `calibrateOffset`,
  `flowMapPhase(t, periodS)`, `timeRateFade(rate)`), `cloudDensity.ts` (the shared sub-cell density
  and its column integral), `cloudLayer.wgsl`, `cloudVolume.wgsl`, `cloudShadow.wgsl` (behind R10's
  `terrain_shadow_factor` and its sky term).
- `ocean/`: `seaState.ts` (`seaState({ windMps, windHeightM, gravityMps2, liquid, air, fetchM })` →
  upwind and crosswind slope variances, significant height, spectrum; `equivalentWind`),
  `gerstner.ts` (`gerstnerSet(state, count)`, `summedSteepness(set)`, `resolvedSlopeVariance(set)`),
  `liquids.ts`, `oceanMask.ts`, `oceanFar.wgsl`, `oceanNear.wgsl`, `shoreline.wgsl`.
- `rings/`: `ringPhotometry.ts` (`ringReflectance`, `ringTransmitted`, `ringPhase`,
  `oppositionSurge`, `hapkeShadowHiding`), `ringProfile.ts` (`ringProfile(ring)`, provisional until
  plan 14's), `ringShadows.ts` (`planetShadowOnRing`, `ringShadowOnBody`), `ringSplit.ts`
  (`instancedShare(pixelFootprintM, radiusMinM, radiusMaxM)`, `handOverDistance(ring, pixelRad)`),
  `shadowingTable.ts` (generated), `ringSlab.wgsl`, `ringParticles.wgsl`, and `ringShadow.wgsl`,
  whose `ring_shadow_on_body` R07's `litBody.wgsl` and R10's `terrain_ring_shadow` hook call.
- `stills/`: `stillPlan.ts` (`planTiles`, `tileProjection`), `batchController.ts`
  (`nextBatch(measured)`), `stillJob.ts` (`StillJob`), `pngLabel.ts` (the `iTXt` chunk),
  `stillStore.ts`, `StillLabel`, `StillControl.tsx`, `StillList.tsx`; constants `STILL_TILE_MAX_PX`
  (2,048 low, 4,096 high), `STILL_STREAMING_TIMEOUT_S` (20).
- In `src/preload/api.ts`, additions to `HyperionApi`: `saveStill(png, label)`, `listStills()`,
  `readStill(id)`; in `src/main/stills.ts`, their handlers, writing under
  `app.getPath("userData")/stills/`.
- In R02's `ViewLabelBlock`: the `DECORATION ON` annunciation, whose nomenclature R02 drafts, and
  `STILLS SUSPENDED` in the still control; in R05's `ViewSettings` and `SETTINGS`: the entries of
  Design note 18.
- For R12: the pass labels of Design note 18, each for one of R12's `BudgetRow`s; R01
  `MemoryCategory` names on every allocation (`clouds`, `ocean`, `rings`, `scatter`, `stills`); and
  Design note 18's quality limits for R12.T7's audit.

## Consumes

Names are the owning plans' Provides as they stand when this plan is re-validated; the owner is
authoritative, and where a name has changed only the call sites here change. Where an owner's row is
ambiguous, the design note that reads it is named.

- **R01** (its Design notes 19 and 20): compute passes, WGSL only; offscreen render targets
  (`RenderEngine.createRenderTarget(spec: RenderTargetSpec)`); indirect draws and dispatches
  (`DrawItem.indirect` and `dispatch(kernel, bindings, IndirectArgs)`); asynchronous pipeline
  creation (`createMaterialAsync`, `createComputeAsync`); per-pass GPU time (`onPassTimes`,
  delivering `PassTimes` keyed by `FrameSubmission.label`, `bracketed` always `false` since R01
  Design note 24) where the
  adapter exposes `timestamp-query`, quantised to 65,536 ns in shipping launches and unquantised
  under `--hyperion-gpu-timing` (its Design note 4, found by probe), with
  `GraphicsStatus.timer: GpuTimer` (`"quantized" | "full" | "absent"`); CPU read-back
  (`readTexture(texture, level?, rect?)` and `readBuffer`, not harness-only); `MemoryCategory`, the
  union later plans extend, on every creation; the
  device-loss and GPU-process-loss fault and its three-loss rule; the preload's `HyperionApi`, to
  which this plan adds `ipcRenderer.invoke` handlers of its own, validating the sender itself (R01
  sets no invoke convention); the SwiftShader smoke harness, `just test-render`, into whose
  `WGSL_CATALOGUE` every shader here is registered for its no-f16 and no-subgroup runs.
- **R02:** the camera, camera-relative `f64` differencing and per-patch origins; `BodyFixedPosition`
  and its client mirror; the fixed reversed-Z infinite projection; `transparentLayerOrder`
  (R02.T7.b: opaque first, then per body back to front, within a body the shells above the camera by
  descending altitude, then the rings' plane, then the shells below by ascending altitude, as
  corrected in RM1 validation; the rings' place between the two groups is only a heuristic, since a
  ring plane can lie on either side of a shell along a ray; each
  testing and not writing depth); the photometric units and pre-exposure; AgX's `toneCurve` and its
  WGSL twin `agx`, which a still's PNG is encoded through; `ViewLabelBlock`; the nine guide drafts,
  of which item 2's decoration rule and flash threshold, item 7's annunciations and the
  `DECORATION ON` nomenclature bind here.
- **R03:** the scene topic: bodies (with plan 14's rings through `BodySummaryDto`), craft poses
  (`SceneCraftDto`, a draft) and their predicted paths through `lib/scene/craft.ts`'s
  `predictedPath(craft, untilS)`, which returns `SceneCraftDto.planned_path` when the sessions plan
  or the flight model fills it and a straight-line extrapolation of the pose otherwise, for which
  this plan widens its sweep (Design note 7); and every push's simulation time and rate
  (`SceneClockDto`).
- **R04:** the `hyperion-surface` crate, its `clippy.toml` and relaxed-SIMD `compile_error!`, the
  `.golden` files asserted equal on native, `wasm32-wasip1` and `wasm32-unknown-unknown` in
  `just ci`, the terrain hazards of the sim-determinism skill; and its test conventions (its Design
  note 12): every test module carries the `wasm_bindgen_test as test` import, a test that cannot
  run on `wasm32-unknown-unknown` sits in a `native_only` module, and every `should_panic` test
  states `expected` and lives in the crate's own `tests/panics.rs`.
- **R05:** the height-worker pool and its per-patch bake; `QualitySetting` (`"high" | "low"`),
  `ViewSettings` and `SETTINGS`, the one list whose `ViewSettings` gains this plan's fields with
  their values in `SETTINGS` (R05's Design note 26); `AllocationTally`, which counts this plan's
  allocations by `MemoryCategory`; the finest-level set under grounded and descending bodies
  (`SelectionInput.grounded: readonly GroundContact[]`, its Design note 9), with morph held at zero,
  to which this plan adds a swept path as a chain of `GroundContact`s spaced at most one radius
  apart; `FORCED_REGION_RESIDENCY_S` in `view/terrain/grounded.ts`, the forced region's p99
  residency, provisional until R05.T18 sets it (Design note 7); streaming priority; `BAND_LIMIT_M`
  and `FINEST_SPACING_M`, with the band limit exported to TypeScript as the wasm `band_limit_m()`
  and the client's `bandLimitM()` (R05.T5, T7.a); the `TERRAIN: STREAMING` and
  `TERRAIN: DETAIL LIMITED` annunciations; `hyperion_surface::num`'s `min`, `max` and
  `assert_finite`; the scripted, seeded descent and its metrics harness, which this plan's by-hand
  runs reuse.
- **R06:** each host's `HostDisc`: `angular_radius`, `mean_luminance` (cd/m², per channel) and its
  limb-darkening `PowerTwo`, for the planet's penumbra on a ring. The star's direction comes from
  R03's scene positions and its illuminance from R07's `starIlluminance(disc, distanceM)` (lux).
- **R07:** the photorealistic style's pass list, with the slots it leaves for this plan's passes;
  `shaders/litBody.wgsl`'s `body_brdf`, `sphere_irradiance` and `eclipse_visible`, which the ring
  and ocean passes call; `METER_CLASS`, which each pass writes in the HDR target's alpha, keeping
  the destination alpha wherever it blends, as every transparent pass here does (its Design note
  10), through R01's `blend: "premultiplied"` or `"additive"`, both of which keep it (R01 Design
  note 21, R01.T8.i); `GlareSource` for glint above the half-float range, evaluated in closed form
  in R07's tone-mapping pass (its Design note 12); `post/tonemap.wgsl`, which applies R02's AgX;
  bloom, run once on a still's assembled image; `starIlluminance(disc, distanceM)`; the
  analytic-disc and point regimes, whose far ring annulus this plan supplies in place of R02's cased
  ellipse (Design note 14; R07 expects it in its Scope and its Consumes of R11); several views
  sharing one `QualitySetting` with per-view budgets (`viewBudgets(views, setting, …)`), one
  photorealistic at a time on the low setting (`photorealisticAllowed`); and `litBody.wgsl`'s call
  of this plan's `ring_shadow_on_body(p, sun) -> vec3f` for a ringed body, through a stub returning
  1 until `ringShadow.wgsl` replaces it, skipped where `ringShadowOnBody` is off (Design note 14).
- **R08:** `HillaireAtmosphere`, whose `aerialPerspective(view)` exposes the volume and the
  transmittance table, for lighting clouds (a 2D array texture, one layer a curvature slice, read
  through `oblate.wgsl`'s slice lookup at the ray's latitude and azimuth, never as one texture;
  R08 Design note 17), and the medium the clouds and the ocean are drawn inside;
  `skyView(view): SkyViewTable` (R08.T9.a), the view's sky radiance by local azimuth and elevation
  summed over the drawn suns, with the per-sun fallback in the same type, for reflecting the sky
  in the ocean; `surfaceLighting.wgsl`'s
  `atmosphere_sun_transmittance(altitude_m, mu_sun, latitude_rad, sun_azimuth_rad) -> vec3f` and
  `atmosphere_sky_irradiance(altitude_m, mu_sun, latitude_rad) -> vec3f` (R08.T9.b), through
  which lit clouds, oceans and rings take each sun's light and the sky's, each call at the lit
  point's geodetic height and latitude and the sun's azimuth from local north there (R08 Design
  note 17); `modeOptics(mode, wavelengthNm)`, the per-wavelength asymmetry and single-scattering
  albedo of an inventory mode, with its column optical depth from R08's `aerosolTerm`, and so a
  condensate mode's cloud optics once one exists;
  `CLOUD_DECK_SPLIT_OPTICAL_DEPTH` and `classifyRegime(medium, cover: BodyCover)`, whose
  `cover.cloudFraction` is P14.T24.b's and whose per-body rule gives `cloudDeck` exactly where
  Design note 9 does; and R08's reference tracer beside which the ring subcommand sits in
  `hyperion-fit` (its Design note 10, whose output is JSON fixtures; this plan's is a shipped
  TypeScript module, a further kind of `hyperion-fit` output, Design note 15).
- **R09:** `FieldView`, with `CoarseField` on the server and `PartialField` on the client
  (`is_surveyed`); `FieldHeader` (`sea_level`, `craters: CraterParams`, and `surface_age: Gyr` and
  `surface_pressure: Pascals`, plan 14's figures carried for this plan's `RockSite`);
  `SynthesisCell` (`water_surface_mm`, `ice`, `class`); `ClimateCell` at level L − 1 (monthly
  anomaly and precipitation, the labelled heuristic, and `wind`); `CoarseCrater` through
  `craters_reaching`; `craters::cumulative_density`; `DetailSeed` and its `stream`, the
  `SurfaceDetail` scope and `ObjectKey::surface_cell(face, level, i, j, instance)`; R05's
  `BAND_LIMIT_M` and `FINEST_SPACING_M`, which R09 uses; `Synthesiser` (the field and the detail
  seed); the fallible `ObjectKey::surface_cell` (`Result<Self, SurfaceCellKeyError>`); the
  caller-owned, order-independent cache convention of `SynthCache`; `hyperion_surface::tags`, the
  surface crate's registry; and `surface.scatter`, which it reserves for this plan. **Asked of
  R09:** the terrain-independent zonal part of its precipitation heuristic (the rain band on the
  energy-flux equator, the dry belts and storm tracks, by latitude and month) as a function the
  client can call from plan 14's global figures without the coarse field (Design note 8).
- **R10:** `MaterialClass`, `Material` and `material_at` (the rock site's class, the scatter filter,
  `SeaIce` and `Seabed` for the ocean); `ground_at`, the authoritative ground the rocks stand on;
  `ClassWeights`; `TerrainLayers` and `BakeHook`, whose `extra: Vec<ExtraLayer>` carries this
  plan's rock list in the same bake and cache; the shader hooks `terrain_decoration` and
  `terrain_shadow_factor(p) -> f32` (the direct sun's lit fraction, into which cloud shadows
  multiply, R10's Design note 11); `terrain_sky_factor(p) -> vec3f`, 1 by
  default, a factor on R08's `atmosphere_sky_irradiance` at the ground, into which the clouds'
  diffuse transmittance goes; and `terrain_ring_shadow(p, sun) -> vec3f`, 1 by default, which this
  plan fills for a ringed body with `ring_shadow_on_body`'s full `vec3f`, the per-channel
  transmittance that R07's `litBody.wgsl` also reads; the
  survey mask; the collision-agrees test (R10.T15, in TypeScript over a committed bake fixture) and
  the per-level-bound test, which this plan extends to rocks; the wireframe's depth-only terrain
  pass.
- **R12:** nothing is consumed. R12 folds this plan's recorded benchmarks into its runs, reads its
  ladder entries into its `SettingSnapshot`, maps its pass labels to `BudgetRow`s, counts its
  allocations by `MemoryCategory` and audits Design note 18's quality limits.
- **Galaxy plan 14:** `RingDto` (`ring_kind`, `material`, `inner_edge_m`, `outer_edge_m`,
  `optical_depth`, `gaps` with `radius_m`) and the sim's `planetary::rings::RING_PARTICLE_RADII`;
  P14.T14's poles and `body_fixed_at`; P14.T24.b's cloud, ocean and ice fractions, surface age and
  crater density, and P14.T13.c's surface state, once the surface section carries them; the rocks
  read the surface age and pressure through R09's `FieldHeader`, not from plan 14 directly, and
  the `AIRLESS_PRESSURE` constant. **Asked of plan 14**,
  as named asks: a radial profile of optical depth, albedo and spectral slope per ring built from
  processes (the brainstorm's list under Rings, with the density-wave formula checked against Shu
  1984); the ring's particle size distribution and vertical thickness on the wire; and a cloud
  fraction that depends on the condensable's availability rather than on the surface state alone
  (Design note 9's finding). Until they land, the provisional forms of Design notes 9 and 13 stand,
  each with a _Slice:_-style note naming the task that removes it.

## Design notes

1. **Order and settings.** The tasks follow step 9's order of value — decoration, scatter, clouds,
   ocean, rings — then stills, which need every pass to exist. Each feature's high and low settings
   are built in the same task, as the budget's third rule requires ("a two-dimensional cloud layer
   written after the volumetric one exists will never be tested and will rot"), and each records its
   own benchmark on the development machine's RTX 3080, the discrete reference, and, by the owner,
   on the UHD 620: in doc comments and this plan's "as built" notes, with the machine, driver,
   setting, resolution, whether the timer was quantised (R01's `GraphicsStatus.timer`), whether the
   machine was otherwise idle, the load average at the start (under 1 for the run to count) and the
   CPU governor, as the roadmap's measurement convention requires, for R12's first task to fold into
   its results record. Timings taken during this plan's research were measured under shared load and
   are provisional until re-measured on a quiet machine.

2. **What "decoration" labels.** Decoration is what only the GPU draws on the ground below what the
   height function computes: high-frequency normal detail, colour variation, sand ripples, small
   crater scars, the detail textures, and scatter below 0.4 m across. The view's label block carries
   `DECORATION ON` beside its setting and its survey coverage while any of it is drawn, and every
   still records whether it was. The low setting draws none of it (Design note 18), so the label
   means exactly what the ladder's "decoration off" means. Clouds and waves are drawn on both
   settings, and their small-scale shapes are no more computed than a ripple is, but they are
   presentations of quantities the simulation did compute (plan 14's cloud fraction and the climate;
   the sea state the wind implies), and they mislead neither a touchdown nor a readout, which is the
   brainstorm's reason for the label ("safety in the fiction depends on it"). This is this plan's
   reading of guide item 2, which R02 drafts and the owner signs off; if the owner reads clouds and
   waves as decoration too, the annunciation names its parts
   (`DECORATION ON: TERRAIN, CLOUDS, WAVES`) and nothing else changes. Researched 2026-09-29
   (advisory).

3. **Decoration is stable, zero-mean, never geometry, and held to the flash limit.** It is evaluated
   in each patch's own coordinates from an integer lattice hash of (face, level, cell, lattice
   point), never from world-space `f32` positions, so it neither swims as the camera moves nor
   differs between two views of one patch; it is not bit-identical across GPUs and need not be,
   since nothing reads it back. It perturbs the baked analytic normal and the class albedo only,
   each with zero mean over a patch, so the shading's large-scale mean is the authoritative
   surface's. It never moves a vertex: a smoke test renders a patch's depth with decoration on and
   off and asserts the two read-backs equal bit for bit. Every octave fades out across one octave of
   pixel footprint, and glints anywhere in this plan (waves, pebbles, ring particles) go through a
   per-pixel filtered BRDF: these are the mechanisms by which the image meets guide item 2's flash
   threshold (no region above the guide's area limit alternating more than three times a second),
   and R11.T11 checks it on recorded sequences.

4. **Rocks: the law, the height and the site.** Researched 2026-09-29. The abundance is Golombek and
   Rapp 1997's cumulative fractional area F_k(D) = k exp(−q(k) D), with q(k) = 1.79 + 0.152 ÷ k per
   metre (their eqs. 1–2, quoted in Golombek et al. 2012, _Mars_ 7:1–22). The cumulative number per
   unit area has a closed form, N(>D) = (4kq ÷ π) [e^(−qD) ÷ D − q E₁(qD)], which reproduces
   Golombek et al. 2012's 0.08% and 0.75% chance of meeting a rock over 1.1 m in 4 m² at k = 5% and
   10%; E₁ is computed in `f64` by series below 1 and continued fraction above, with no platform
   `libm`. k is clamped to [0, 0.40], the model family's range. A rock's exposed height is drawn per
   rock as h/D uniform on [0.25, 0.50], around the 0.29–0.41 measured at the MSL sites and capped at
   the hazard convention of 0.5; every rock of D ≥ 0.4 m is therefore authoritative whatever its
   drawn height, a superset of "0.2 m tall or more" that keeps the split a diameter threshold, and
   decorative scatter owns only D < 0.4 m, so no rock is in both. At 32 m square this gives about 58
   authoritative rocks at Viking 1's k of 0.07 and about 210 at Viking 2's and Pathfinder's 0.18,
   not the brainstorm's "some tens" (a finding in the notes). k is `RockSite`'s function,
   `clamp(k_base(material, atmosphere) · A(age) + k_ejecta, 0, 0.40)`: k_base 0.15–0.25 on young
   `Bedrock` and `VolcanicRock`, 0.05–0.08 on `Regolith`, 0.01–0.03 where dust mantles it, 0.02–0.05
   on ice classes (about 0.01 on Titan-like fluvial ice plains), and zero on `Seabed` (under any
   sea, R10 having no liquid class), `SeaIce` and `Sand`; A(age) non-increasing, falling faster on
   airless bodies (rock breakdown, Ghent et al. 2014) and slower under an atmosphere; k_ejecta = 0.3
   exp(−(r − R) ÷ R) outside the rim of a coarse crater within plan 14's surface age, truncated at
   3R and aged by the same A. On airless worlds the one-parameter law cannot fit the flatter lunar
   size distribution, so k is set for the authoritative sizes (mature mare 0.03–0.05) and small
   rocks are knowingly wrong there. The mapping is a ruling of medium to low confidence. Every input
   is one both sides hold: the material and the coarse craters come through R09's `Synthesiser` and
   R10's `material_at`, and the body's surface age and surface pressure come from R09's
   `FieldHeader` (`surface_age`, `surface_pressure`, plan 14's figures, which both sides' fields
   carry), the pressure classed as `Airless` below plan 14's own `AIRLESS_PRESSURE` of 100 Pa, so
   the client places the rocks the server does. A cell whose inputs are not
   held is an error (`NotSurveyed`) on the client, which draws no rocks there, and no hull reaches
   it there, since the server sends the cells under and ahead of a descent before it could touch
   them.

5. **Rocks: placement and arithmetic.** Four diameter octaves, 0.4–0.8, 0.8–1.6, 1.6–3.2 and 3.2–6.4
   m: at k ≤ 0.40 the law's N(>6.4 m) is at most 1.0 × 10⁻⁸ per m² (one per 100 km²; 2 × 10⁻¹⁰ at
   Viking 2's k, and about 10⁻⁵ rocks per 32 m patch at k = 0.40), so nothing measurable lies beyond
   them (researched 2026-09-29). Each octave has its own cube-sphere quadtree level, with cells at
   least as large as its largest rock's reach, as R09's crater octaves are. A cell's count is a
   Poisson draw of the expected number, N(>D_low) − N(>D_high) times the cell's true area, through
   the integer thresholds on `surface.scatter` keyed by `ObjectKey::surface_cell` of face, octave
   level and cell; diameters invert the law within the octave, and positions are jittered in the
   cell. Each rock's shape comes from `surface.scatter.shape` with the rock's index as the key's
   `instance`: its height ratio, axial ratios near the fragment ratios a : b : c ≈ 2 : √2 : 1 with
   b/a drawn from [0.55, 0.9] and the vertical axis following from the drawn exposed height, a yaw
   and a superquadric exponent. The drawn diameter is the visible one, as Golombek's are, and the
   base centre is buried below R10's `ground_at` so that the exposed height is the drawn one. A
   query searches the 3 × 3 neighbourhood of cells at each octave's level, across face edges, where
   a cube corner has seven neighbours. Everything is `f64`; keys are `u64`, never `usize`; min and
   max are `hyperion_surface::num`'s `min` and `max` (R05's); heights are asserted finite before
   they are compared or emitted.

6. **Drawn rocks and the contact query agree to a stated bound.** The contact query is the analytic
   shape. The drawn mesh is a tessellation whose vertices lie on the analytic surface, so it lies
   inside the shape by at most its chordal error, and the level is chosen per rock so that the error
   is at most 5 mm or 2% of the smallest semi-axis, whichever is smaller. That is the rocks'
   counterpart of the terrain's "collision and the picture agree to the `f32` step of the height
   texture", and R11.T4.c tests it the same way.

7. **Where rocks are drawn, and "could touch".** Researched 2026-09-29 (advisory). "Could touch" is
   defined by what must be streamed: for a grounded body, its contact footprint widened by the
   largest octave's reach; for a descending body, that footprint swept along its predicted path to
   the ground over a horizon of at least the forced region's residency time, so that a fast descent
   never reaches a rock that is not yet drawn. The residency time is R05's
   `FORCED_REGION_RESIDENCY_S` (`view/terrain/grounded.ts`), the p99 time from a forced region's
   request to its residency, provisional until R05.T18 sets it. The path is R03's
   `predictedPath(craft, untilS)`: the sessions plan's or the flight model's `planned_path` where
   one is sent, and otherwise a straight-line extrapolation of the pose at its velocity. A straight
   line misses gravity's pull, so along an extrapolated path the footprint is widened by
   `extrapolationMargin`, ½ g t² at each time t of the horizon, which bounds an unpowered arc's
   departure from it; a powered manoeuvre is not bounded (Risks). Within that region every
   authoritative rock is drawn, on every setting and in both styles — in the wireframe as
   depth-writing geometry stroked at its silhouette, so that the helm's picture at touchdown agrees
   with what the hull meets. R05's finest-level set takes only discs
   (`GroundContact { position, radius_m }`), so `couldTouchContacts` covers the swept region with
   discs along the path, spaced at most one disc radius apart, and hands them to R05's
   `SelectionInput.grounded` beside the bodies' own; T4.d checks that the union covers the region.
   Elsewhere, on the high setting, rocks are drawn out to where they subtend a pixel; on the low
   setting they are drawn nowhere else, and wherever a finest-level patch is in view whose rocks
   would subtend a pixel the view annunciates `TERRAIN: DETAIL LIMITED`. Decorative scatter is
   culled on the GPU against every body whose bounding volume comes within 0.2 m of the ground, each
   box widened by the instance's radius; pebbles against authoritative rocks are cosmetic and not
   culled.

8. **Clouds everywhere, refined by the survey.** Researched 2026-09-29.
   Only the orographic part of a cloud pattern reveals the ground, since R09's precipitation carries
   rain shadows; the rest is weather a ship sees from orbit without a survey, and a flat haze that
   turns structured at the survey edge would give the edge away. So coverage is built over the whole
   body from a terrain-free zonal climatology — the latitude-and-month part of R09's heuristic from
   plan 14's global figures, asked of R09 — and refined, over surveyed cells, by their own
   `ClimateCell`s, blended across two or three coarse cells at the survey edge. It is a labelled
   heuristic: the month's standardised log precipitation through a logistic to a pseudo-humidity and
   then Sundqvist et al. 1989's closure, C = 1 − √((1 − RH) ÷ (1 − RH_c)), with a low marine term
   that grows with a sea cell's cold anomaly against its latitude band (after Klein and Hartmann
   1993), combined as 1 − (1 − C_conv)(1 − C_strat). One offset in the logistic is solved by
   bisection once per body so that the area-weighted mean over the whole body equals plan 14's cloud
   fraction, and it is never re-solved over the surveyed subset, so surveying new ground never
   changes clouds already drawn. Motion is a stateless two-phase flow map along the wind (Vlachos
   2010; Neyret 2003), so the field is a pure function of the detail seed, the climate and the
   scene's time, with no accumulated state that a client joining late could miss; noise is hashed
   from body-fixed integer lattices, as decoration's is. Two clients therefore draw the same weather
   to the eye, and bit for bit on the CPU reference; the GPU's `f32` is not bit-identical and need
   not be, because no readout quotes a cloud and nothing collides with one. Under time warp, above
   the rate at which a feature would cross a coverage cell in a third of a second, the structured
   part fades to the calibrated mean (`timeRateFade`), so clouds average out instead of strobing
   past guide item 2's flash limit.

9. **Which clouds are this plan's.** Researched 2026-09-29 (advisory). The boundary with
   R08 is drawn per body, never per column, since a broken cumulonimbus field has cells of τ ≫ 10
   that `CLOUD_DECK_SPLIT_OPTICAL_DEPTH` must not reclassify: R08 owns a body whose cloud fraction
   is 1 and whose deck exceeds τ 10, with any thin layer above it; this plan owns every body with a
   cloud fraction strictly between 0 and 1, whatever the local τ. A gas giant's banded cloud tops
   belong to neither: R07 draws a uniform giant and no plan generates bands, which the roadmap is
   asked to assign (the advice is R07, a band texture driven by zonal jets over R08's deck
   photometry). **Finding for plan 14:** `SurfaceState::cloud_fraction`
   (`crates/hyperion-sim/src/planetary/derive/atmosphere.rs:769`) gives 0.67 to every `Temperate`
   world, Mars included, and 0 to `Snowball`, which it names Titan's state, so scaling to it would
   draw Earth's cover on a Mars-like world and no methane clouds on a Titan-like one; plan 14 is
   asked for a fraction that depends on the condensable's availability. Until then this plan scales
   to plan 14's figure as it stands, marked provisional.

10. **Clouds: one model, two renderings.** Researched 2026-09-29. Both settings share the coverage
    field, one sub-cell density field (log-normal in τ with an inhomogeneity factor χ = exp⟨ln τ⟩ ÷
    ⟨τ⟩ ≈ 0.75, after Oreopoulos and Cahalan 2005), the condensate's optics, R08's transmittance to
    the layer inside R08's medium and aerial perspective, and the layer's altitude. The layer's
    per-texel τ is the column integral of the same density the volume marches, so the
    two-dimensional layer is the independent-pixel approximation of the volume; without that,
    reflectance's concavity in τ would put 7–22% between them (the plane-parallel albedo bias,
    Cahalan et al. 1994). The layer's reflectance and transmittance are δ-Eddington's for a direct
    beam at μ₀ (Joseph, Wiscombe and Weinman 1976; Meador and Weaver 1980), not the diffuse-only
    two-stream the first draft named, which ignores μ₀ and falls about 20% short at τ = 10; the
    direct part of the transmission is e^(−τ ÷ μ₀) with the unscaled τ. Shadows on terrain take only
    that direct part, multiplied into R10's `terrain_shadow_factor`; the diffuse transmission lights
    the ground below through R10's `terrain_sky_factor` on R08's `atmosphere_sky_irradiance`, since
    multiplying the total into the sun would draw a sharp half-bright sun under overcast (Frostbite
    bakes cloud shadow the same way, Hillaire 2016 §5.9). The brainstorm's ladder gives cloud
    shadows on terrain to the high setting only (its Shadows row: "terrain self-shadowing only" on
    the low one), so the low setting draws the shell and casts no cloud shadow by default. The
    layer's own shadow, one tap of the τ texture per shaded pixel where the sun ray crosses the
    shell (a ray–sphere intersection, well under 0.1 ms, with the diffuse part into
    `terrain_sky_factor`), is built and tested beside it behind the `cloudShadows` ladder entry, off
    on the low setting, and a correction proposing to turn it on there goes to the README's
    brainstorm corrections for the owner. The high setting marches the volume at quarter resolution
    with temporal reprojection (Nubis 2015: a 128³ base noise, a 32³ detail noise and a 128² curl
    noise, single-channel `r8unorm` as Frostbite did, about 2 MiB in all), Hillaire 2016's
    energy-conserving integration, a two-lobe Henyey–Greenstein phase whose weighted asymmetry
    equals the condensate's g, and Wrenninge et al. 2013's multiple-scattering octaves (a ≤ b)
    fitted to δ-Eddington for a homogeneous slab over τ 1–50 and μ₀ 0.5–1, with the fit error in the
    doc comment; its shadows come from a transmittance map of the volume in the sun's frame over the
    camera's footprint, sampled and multiplied into R10's `terrain_shadow_factor` (R10's Design note
    11), with its diffuse part into `terrain_sky_factor`. Either way the clouds test depth and write
    none, and a deck at least a kilometre above the terrain needs no depth bias out to 10⁹ m. The
    budget's 1.5–3 ms on the discrete target is consistent with Frostbite's 1.6 ms at 1080p on an
    Xbox One and Nubis's 2 ms on a PS4; an orbital shell's longer marches justify its upper end.

11. **The ocean's authoritative surface.** The water surface the simulation knows is the sea-level
    sphere of R09's coarse field (`FieldHeader::sea_level`, the quantile that leaves plan 14's ocean
    fraction below it), and for a closed basin its own level (`SynthesisCell`'s `water_surface_mm`),
    drawn as a flat lake. Waves are presentation about it: their statistics are physics, from the
    climate field's wind, and their phases are the GPU's. Gerstner surfaces have zero mean vertical
    displacement over their Lagrangian grid, and an Eulerian mean that differs at second order, O(Σ
    kA²); the test states which. Nothing collides with water yet; when a flight model does, it meets
    the mean surface first, and waves join the authoritative surface only if a later ruling asks.
    Tides are absent, and a later tide model moves the authoritative level. That keeps "an ocean on
    the terrain quadtree consistent across levels of detail under the collision rules" true: the
    mean surface is analytic and the same at every level, and the mask is decided per coarse cell (a
    water surface above the cell's coarse elevation and no ice), never re-decided by a shader;
    within a cell the shoreline comes from R10's height texture against that level. Where the coarse
    field's ice covers the sea, the ocean gives way to R10's `SeaIce`, "with no glint and no waves".
    Over unsurveyed ground no sea is drawn, as no coastline is claimed there; the research suggests
    a disc-statistical glint on R07's reference body (the ocean fraction times the glint at the
    body's mean wind), which is left to the owner (Risks).

12. **Ocean by distance, with one slope budget.** Researched 2026-09-29. The total slope
    variance is Cox and Munk's, anisotropic along the wind: σ_u² = 3.16 × 10⁻³ U and σ_c² = 0.003 +
    1.92 × 10⁻³ U (total 0.003 + 5.12 × 10⁻³ U), with U at 12.5 m converted from the climate field's
    wind by a neutral log profile, U(z) = U₁₀ [1 + (√C_D ÷ κ) ln(z ÷ 10)], C_D ≈ 1.3 × 10⁻³, κ =
    0.4, and clamped to 0–20 m/s with the extrapolation above 14 m/s stated. Above 20 km the ocean
    is the sphere with that distribution filtered per pixel, as Bruneton, Neyret and Holzschuch 2010
    draw it (their switch "for altitudes below 20,000 m" confirmed); below 20 km it is a projected
    grid in the frame tangent to the sphere under the camera, displaced by Gerstner components
    sampled geometrically from about twice the peak wavelength down to the grid's cut, within
    ±30–45° of the wind, from a Pierson–Moskowitz spectrum (ω_p = 0.877 g ÷ U₁₉.₅, H_s ≈ 0.21 U₁₉.₅²
    ÷ g), or JONSWAP's fetch-limited form over a basin, g H_s ÷ U₁₀² = 1.6 × 10⁻³ (g X ÷ U₁₀²)^½
    capped at full development. Each component's resolved slope variance is 1 − √(1 − k²A²) per axis
    (Bruneton's eq. 4), and the BRDF keeps max(σ²_CM − Σ resolved, 10⁻⁴) per axis, the floor keeping
    the sun's disc resolved; if the resolved variance would exceed the total, the amplitudes are
    scaled down. Every set keeps Σ Qᵢ wᵢ Aᵢ ≤ 1 (Finch; with w = 2π ÷ L) and each component kᵢAᵢ ≤
    0.44, the Stokes limiting steepness; dispersion is ω² = gk + (γ ÷ ρ) k³, so that capillary
    components and other liquids are right. Other worlds' seas scale through an equivalent Earth
    wind, U_eq = U (c_m,⊕ ÷ c_m) √[(ρ_a ÷ ρ_l) ÷ (ρ_a ÷ ρ_l)_⊕], with c_m = (4gγ ÷ ρ_l)^¼ the
    minimum phase speed (0.231 m/s for water), below a generation threshold the sea is glassy at the
    floor (about 1 m/s on Earth, 0.4–0.7 m/s on Titan): a heuristic of low confidence, labelled so,
    that is the identity at Earth's parameters. The water's colour over the shallows is Beer–Lambert
    through pure water (Pope and Fry 1997: 0.00635, 0.0565 and 0.624 per metre at 440, 550 and 700
    nm), its thickness from the depth buffer within 10⁵ m and from the bathymetry beyond, and a
    lower layer is culled once its separation falls below 10⁻⁶ of the camera distance. Whitecaps, W
    = 3.84 × 10⁻⁶ U₁₀^3.41 (Monahan and O'Muircheartaigh 1980), are on the high setting only.
    Refractive indices: water 1.333, seawater 1.339, liquid methane about 1.29, ethane about 1.37
    (the last three re-checked when written). The shoreline is foam driven by depth and the
    terrain's slope, amplitude attenuated by depth, and a depth fade, never a depth bias. Refraction
    is on the high setting only; the low setting keeps about eight components and the glint. A
    spectral FFT field would replace only the component sum, behind the same budget.

13. **Rings: the provisional profile.** Researched 2026-09-29. Until plan 14 sends a
    radial profile, the ring's profile is its one `optical_depth` across the annulus, and each of
    its `gaps` is a band that starts at the gap's `radius_m` — the resonance is the gap's inner
    edge, as Mimas's 2:1 is the Cassini Division's — and runs outward by
    `PROVISIONAL_GAP_WIDTH_FRACTION` = 0.039 of that radius (the Division's 4,590 km at the B ring's
    117,580 km edge), with τ of 0.1 × the ring's (floored at 0.05; the Division holds τ ≈ 0.05–0.12,
    not zero) and darker, less red material. Albedo and spectral slope come from the ring's
    `material`. The constant is named provisional in its doc comment and its test, and plan 14's
    profile removes it. The client samples whichever profile it has as a one-dimensional texture at
    the mip for the pixel's footprint.

14. **Ring photometry in every regime.** Researched 2026-09-29. With μ = |sin B|, μ₀ =
    |sin B′| and P normalised over 4π, the lit face is I/F = ϖ₀ P(α) μ₀ ÷ (4(μ + μ₀)) × [1 −
    exp(−τ(1/μ + 1/μ₀))] and the unlit face I/F = ϖ₀ P(α) μ₀ ÷ (4(μ − μ₀)) × [exp(−τ/μ) −
    exp(−τ/μ₀)], with its limit ϖ₀ P τ exp(−τ/μ) ÷ 4μ as μ → μ₀; the unlit face is the transmitted
    beam single-scattered, and multiply scattered through the table, so a dense B ring is dark from
    below while the Cassini Division and the C ring are bright. The lit face is multiplied by the
    shadowing factor of Design note 15. The main ring's particles take Salo and French 2010's
    power-law phase, P(α) = c_n (π − α)^n with n = 3.09 (Callisto's, after Dones et al. 1993), times
    an opposition surge multiplier of half-width 0.20° for A- and B-like rings, 0.26° for C-like and
    0.28° for the Division, amplitude about 1.45 in sparse rings and falling with τ (Déau et al.
    2009); dusty rings take a forward lobe (two-term Henyey–Greenstein with g₁ ≈ 0.7, low
    confidence, tested only for being brighter back-lit). The planet's shadow on the ring is a ray
    against the sphere with its penumbra from R07's `eclipse_visible` across R06's disc; the ring's
    shadow on the planet is a ray against the ring plane reading the profile's transmittance,
    supplied as the WGSL function `ring_shadow_on_body`, which R07's `litBody.wgsl` calls for a
    ringed body's disc and mesh regimes (R07's stub, which this replaces) and R10's
    `terrain_ring_shadow` hook calls on terrain; on the low setting it is not called
    (`ringShadowOnBody` off). The same functions serve R07's far annulus beyond 10⁹ m, so a ring is
    one model from its annulus to its particles.

15. **The shadowing table.** Researched 2026-09-29. Salo and French 2010 found the B ring's tilt
    brightening (about 20% from shadowing, at most 10% from multiple scattering; 1.25–1.35 at B =
    26° against 4.5°, τ_dyn = 1.5, α = 6°) in dynamical particle fields whose central filling factor
    is 0.32–0.38, and note that a vertically uniform ring would show no elevation dependence; the
    brainstorm's "a particle slab at a filling factor near 0.05" cannot reproduce the test it is
    accepted against (a finding in the notes). The Monte Carlo therefore traces photons, in `f64`
    and with multiple scattering included and tallied by scattering order, through a layer whose
    number density is Gaussian in height, n(z) = n₀ exp(−z² ÷ 2σ²). Its central filling factor is D₀
    = τ_dyn (4/3)⟨R³⟩ ÷ (⟨R²⟩ H_eff), exact for that layer when H_eff ≡ √(2π) σ and τ_dyn = ∫ n
    π⟨R²⟩ dz is the dynamical (geometric) optical depth, with ⟨R³⟩ ÷ ⟨R²⟩ = (R_max − R_min) ÷
    ln(R_max ÷ R_min) for q = 3. Researched 2026-09-29 against Salo and French 2010 (Icarus 210,
    §3.3–3.4, eq. 13): their layers run R⁻³ with R_max = 5 m and R_min ÷ R_max of 0.02–0.2, never 1
    cm, and the width of the shadowing peak scales with R_max ÷ R_min (their eqs. 18–19), so a 1
    cm–5 m layer would put α = 6° in the peak's wing. The shadowing layer therefore takes R 0.5–5 m
    (W = 10, which their opposition fits favour), with 0.1–5 m as a cross-check, deliberately not
    plan 14's 1 cm–5 m, which stays the slab's and the split's optical-depth budget (Design note
    16); D₀ = 0.35, the middle of their 0.32–0.38, at τ_dyn = 1.5 gives H_eff = 11.2 m (σ = 4.5 m;
    7.2 m for 0.1–5 m), and D₀ is proportional to τ_dyn at fixed H_eff. The first draft's "about 7
    m" fitted 0.1–5 m, and gave D₀ = 0.23 over 1 cm–5 m. The Monte Carlo measures τ_phot =
    −ln(normal transmission), about τ_dyn (1 + D) in dense layers, and indexes the table by τ_phot,
    which is what the slab's extinction uses. It emits the shadowing factor over phase angle,
    elevation and optical depth as a table under 50 kB. Acceptance, from Salo and French's §5.2: at
    τ_dyn = 1.5, W = 10 and α = 6°, the single-scattering ratio f_e(26°) ÷ f_e(4.5°), with f_e =
    I_ss ÷ I_ss(D = 0), is 1.30 ± 0.05, their modelled 1.25–1.35 and the brainstorm's five
    percentage points (their abstract's "20%" and their body's "about 30%" disagree; the band
    follows the body's models); at α = 0.5° the same ratio is about 1.15 ± 0.05, a second check.
    Their photometric match used τ_dyn = 2.0, W = 10, the n = 3.09 phase and Bond albedos of 0.57 at
    814 nm and 0.21 at 336 nm, against the observed I/F of 0.44 and 0.16 at B = 4.5°, α = 6°; the
    test reproduces that configuration and accepts 0.44 ± 0.05 and 0.16 ± 0.03 at B = 4.5°, labelled
    as a consequence of their fit, not a fit of this plan's. The cap of D₀ at 0.40 at high τ is a
    lean of low confidence (Risks). Plan 15's emitter writes Rust into the sim's tables, and this
    table is the client's, so it follows R08's precedent (its Design note 10): an `hyperion-fit`
    subcommand outside the table manifest, run by hand, whose committed output is a client module.
    The low setting samples the same table, one trilinear fetch; Hapke's shadow-hiding term, B(α) =
    B₀ ÷ (1 + tan(α/2) ÷ h) with h from Salo and French's eq. 18 and D_eff = D₀ sin|B|, remains a
    measured fallback with its error against the table recorded.

16. **Rings close to.** The criterion is an angle: the ring is a slab while its largest particle
    subtends less than a pixel. At 1080p across 60° a pixel is about 0.55 mrad, so a body of
    diameter D fills one at about 1,830 × D, 1,220 × D at 720p and 3,670 × D at 4K. Plan 14's
    largest particle has a _radius_ of 5 m (`RING_PARTICLE_RADII`, after Zebker et al. 1985), so its
    diameter is 10 m and the hand-over lies at about 18 km at 1080p, not the brainstorm's 9 km (a
    finding in the notes). Nearer, a particle is instanced when its diameter 2s exceeds the pixel's
    footprint p = d θ_pix, so the threshold radius is s_pix = p ÷ 2, half the footprint, and with s
    a radius throughout the instanced share of the optical depth is ln(s_max ÷ s_pix) ÷ ln(s_max ÷
    s_min), clamped to [0, 1], for an s⁻³ distribution, whose τ per ln s is flat: over plan 14's
    radii of 1 cm–5 m it is 0 at the hand-over (p = 10 m, 18.3 km at 1080p), 0.371 at p = 1 m (0.5 m
    particles fill a pixel) and 1 once p is below 2 cm (researched 2026-09-29, high confidence).
    That is a share that grows with approach, not the brainstorm's fixed "top size decade" (a
    correction proposed to the README); that share is removed from the slab so
    that the photometric extinction is conserved, which a test checks. Within a few layer
    thicknesses the view becomes a local particle field inside volumetric extinction. Instances are
    placed by a GPU hash in cells of the ring plane that turn at the local Keplerian rate from the
    scene's time, so the field shears as a ring does; they are presentation, not authoritative.
    Particles are drawn on the discrete target only: the UHD 620's low setting keeps the textured
    annulus (open question 8), and a 4K main screen renders below native.

17. **Still images.** Researched 2026-09-29 (the probe's round-trip timings were
    taken under shared load and are provisional). A still is a job, one per client at a time, that
    renders the view's scene frozen at its simulation time, from the requesting view's camera or one
    the console sets, through the photorealistic pipeline with every pass the high setting runs, at
    the exposure the view had at the request, never metered per tile. Tiles are the smaller of the
    adapter's `maxTextureDimension2D` (16,384 on the UHD 620 by probe, 8,192 the core guarantee, so
    it is read and never assumed) and `STILL_TILE_MAX_PX`, 2,048 on the low setting and 4,096 on the
    target, chosen by memory (for context, the development machine's RTX 3080 has 10 GiB of VRAM,
    and the UHD 620 shares its laptop's system memory), with widths a multiple of 64 px so that
    `bytesPerRow` meets the 256-byte rule; each tile's projection is M · P, M touching only clip x
    and y, so R02's
    reversed-Z projection keeps its depth. Every screen-space footprint (terrain error, mips,
    decoration fade, the ring profile's mip and hand-over) is taken from the full still's pixel,
    frustum culling is per tile, bloom and glare run once on the assembled image, and sub-pixel
    jitter accumulates at full resolution instead of temporal reprojection or quarter-resolution
    clouds. Every pipeline the still needs is created asynchronously before the first batch, because
    a synchronous compile of the high setting's pipelines on the GPU process's main thread is the
    realistic way to trip Chromium's 15 s watchdog. The job is progressive: one batch of whole
    passes per animation frame (timestamps exist only at pass boundaries), submitted only after the
    previous one's `onSubmittedWorkDone()`. The batch is sized from `timestamp-query` to about 6 ms
    of GPU time on the UHD 620's low setting and a quarter of the frame on the target, with a margin
    of two 65,536 ns quanta; without timestamps, from the submit round trip less the empty-submit
    round trip measured at job start (median of five), growing by at most 1.5 times a step and
    halving after an overrun, which over-estimates by 1.5–5 ms and so cannot undershoot the cap. No
    single dispatch or draw may exceed about 50 ms, by construction from the measured cost per
    workgroup; i915's 640 ms preemption timeout and 2,500 ms heartbeat are confirmed on the UHD 620
    laptop. The first device or GPU-process loss while a still is in flight suspends stills for the
    session on that client (`STILLS SUSPENDED` in the control), allowing only an explicit re-request
    at the smallest batch with growth capped at half the batch that was in flight; a second disables
    them, since three losses disable WebGPU. The still asks the server for nothing the view could
    not, and uses only cells surveyed at the request: its patches stream at bulk priority through
    R05, and if its frustum has not reached the level its pixels warrant within
    `STILL_STREAMING_TIMEOUT_S` (20 s, measured in T10.d) it is labelled `TERRAIN: STREAMING`. It is
    the image alone: symbology is not burned in, since its data states would freeze outside the
    guide's semantics. Its label is a record — its time, camera frame, position, field of view and
    exposure, setting, whether decoration was on, survey coverage and any annunciation that held —
    written as a JSON sidecar and embedded in the PNG as an `iTXt` chunk (`hyperion-still`). The
    image is an 8-bit sRGB PNG tone-mapped through R02's AgX in R07's tone-mapping pass at the
    recorded exposure, each tile read back through R01's `readTexture`, encoded
    with `OffscreenCanvas.convertToBlob`, and saved through the preload's `saveStill` under
    `userData/stills/` by the main process, which validates the sender, the size and the label's
    schema and writes atomically. A file, rather than IndexedDB, because the renderer's origin
    differs between development and production and a still is an artefact a player keeps. It is not
    ship state.

18. **The low settings, together.** Decoration: off, decorative scatter included, so the low setting
    draws no decoration at all. Scatter: authoritative rocks in the "could touch" region only.
    Clouds: the two-dimensional layer, about 1 ms and under 16 MB, with the time-rate fade, and no
    cloud shadow on terrain (Design note 10). Ocean: about eight Gerstner components, no refraction,
    no whitecaps, glint retained, 2–3 ms. Rings: the textured annulus sampling the same shadowing
    table, with the planet's analytic shadow and not the ring's on the planet (the ladder's Rings
    row), under 0.5 ms, and no particles. Stills: the same passes as on the high setting, slower,
    within the tile limit. These are the budget's estimates, which this plan's benchmarks record
    against; R12 audits them together. A wireframe view draws none of this plan's passes except the
    rocks of Design note 7; R02 draws rings there as ellipses.

    The ladder entries, added as fields of R05's `ViewSettings` with their values in `SETTINGS`
    (high; low): `decoration` (on; off),
    `scatterDensity` (1; 0, the fraction of the decorative instance density drawn, which R12 may
    tune, with 0 creating no scatter pipeline), `clouds` (`volume`; `layer`), `cloudShadows` (on;
    off), `oceanComponents` (32; 8), `oceanRefraction` and `whitecaps` (on; off), `rings`
    (`particles`; `annulus`) and `ringShadowOnBody` (on; off). The pass labels, each mapped to one
    of R12's `BudgetRow`s: `decoration` (inside R10's terrain pass, timed as its increment),
    `rocks`, `scatter`, `clouds`, `cloudShadow`, `ocean`, `rings` and `still`. Every GPU allocation
    names an R01 `MemoryCategory`: `clouds` (noise volumes, the τ texture, the transmittance map),
    `ocean`, `rings` (the profile and the shadowing table), `scatter` and `stills` (tiles and the
    accumulation target). The quality limits for R12.T7's audit: the budget's rows above for each
    pass on each GPU, 16 MB for the low clouds and 10–20 MB for the volume, and the tile sizes of
    Design note 17; R12 records a miss as a finding.

19. **Motion.** Clouds move with the simulation's wind and waves with its time: that is the world
    moving, which guide item 8's reduced-motion rule does not stop ("the world cannot be frozen").
    Both hold still when the scene's time rate is zero, and clouds fade to their mean under fast
    time warp (Design note 8). Decoration and scatter do not move.

20. **Craters between 1 and 2 m stay decoration (open question 18).** Researched 2026-09-29.
    Brand-new Martian craters of metre scale average d/D = 0.23 (Daubar et al. 2014), but lunar
    "random fresh" ones 0.10 and mature ones about 0.05 or less (Stopar et al. 2017), so on a
    typical mature surface a 1–2 m crater is 0.05–0.2 m deep, not the brainstorm's 0.2–0.4 m. At
    equilibrium about four lie in a lunar module's footprint and some 0.5–1 of them reach 0.2 m; a
    depression cannot strike an engine skirt or a belly, which is what makes 0.2 m of positive
    relief authoritative; a 0.2 m drop under one pad tilts a 9.4 m-diagonal lander by 1.2–1.7°; and
    hazard practice folds such bowls into slope and residual relief over the footprint (ALHAT's 5°
    and 0.3 m), which the authoritative 2 m surface already carries. They are therefore scars in the
    decoration, and no dip is built. The ruling is revisited if a vehicle with pads or wheels under
    about 1 m on legs spaced under 2 m arrives, a hazard console must show the ground below 2 m, or
    a fresh-crater class is placed at landing sites.

## Tasks

T1 comes first. T2 needs T1. T3's subtasks run in order; T4 and T6 follow T3. T5 is a record and can
land at any time. T7, T8 and T9 are independent of the rocks and of each other and can run beside
them after T1. T10 follows T2, T4, T6 and T7–T9, since a still wants every pass; T10.a can start at
once except for its decoration-fade test, which waits for T2. T11 closes. Before T1 starts, the plan
is re-validated against the code as built (`revalidate-plan`): R01's, R02's and R05's for T1, T2 and
T10, and R09's and R10's before T3, since their names are what T3 and T4 call. Every task that adds
or changes a WGSL shader registers it in R01's `WGSL_CATALOGUE` and runs `just test-render` as part
of its acceptance, as the roadmap's conventions require. Every photorealistic pass writes R07's
`METER_CLASS` in the HDR target's alpha and keeps the destination alpha where it blends, and every
lit pass takes its sun and sky light through R08's `surfaceLighting.wgsl`. Every figure below that
turns into a constant is re-checked against its cited source when it does, per the galaxy README's
Figures rule; the figures the research marked "from memory" (the notes list them) are re-checked by
a research agent in the task that writes them.

Rust files are under `crates/hyperion-surface/src/` and client files under
`apps/hyperion/src/renderer/src/view/` unless a path says otherwise.

### R11.T1 The decoration setting and label

Add the ladder entries of Design note 18 as fields of R05's `ViewSettings`, with their high and low
values in `SETTINGS` (`view/quality/qualitySetting.ts`) (`decoration`, `scatterDensity`,
`clouds`, `cloudShadows`, `oceanComponents`, `oceanRefraction`, `whitecaps`, `rings`,
`ringShadowOnBody`), and the `DECORATION ON` annunciation to
R02's `ViewLabelBlock` beside the setting and the survey coverage, in `--text` on its plate, steady,
shown while any decoration is drawn. Nothing is drawn yet, so it never shows.

Files: R05's `view/quality/qualitySetting.ts`, R02's `ViewLabelBlock`,
`surface/decoration/decorationSetting.ts`, tests. Tests: the annunciation follows what is drawn, not
the setting alone; the low setting's defaults are Design note 18's, with no decoration of any kind;
the label is text in the DOM, not canvas. Acceptance: `pnpm test`, `just ci`.

### R11.T2 GPU decoration

- **R11.T2.a The decoration library.** `decorationLattice.ts` and `decoration.wgsl`: an integer
  lattice hash of (face, level, cell, lattice point) in `u32`, band-limited gradient noise for
  normal perturbation at wavelengths below the 2 m band limit, sand ripples keyed by material class
  and oriented by the coarse wind, colour variation, and crater scars below 2 m (Design note 20) at
  a density from R09's `craters::cumulative_density` extended below the band limit, each scar's
  depth from the d/D ratios of Design note 20; every octave fades across one octave of pixel
  footprint (Design note 3); and `detailTextures.wgsl`, the Materials section's triplanar projection
  of rock and detail textures blended by the normal, with its biplanar variant for the UHD 620 if
  three samples cost too much there, and two-scale repetition hiding with a per-patch rotation and
  offset, each texture's contribution zero-mean about R10's class albedo. Tests: the TypeScript
  reference's hash equals the WGSL's on SwiftShader through R01's smoke harness for 10⁴ lattice
  points; each perturbation has zero mean over a patch to 10⁻³; no octave above the band limit is
  ever summed.
- **R11.T2.b On the terrain.** Wire the library into R10's `terrain_decoration` hook, compiled out
  when `decoration` is off; `DECORATION ON` shows while it is drawn. Tests: R01's smoke harness
  renders one patch's depth with decoration on and off and asserts the read-backs equal bit for bit;
  every texel finite; the low setting's pipeline contains no decoration code. Bench: GPU time added
  to the terrain pass on the high setting, at 1080p on the development machine and, by the owner,
  at 720p on the UHD 620.

Files: `surface/decoration/*`, R10's terrain material shader, R01's `WGSL_CATALOGUE`, tests.
Acceptance: `pnpm test`, `just ci`, `just test-render`, and by hand: from 2 m above a sandy slope,
ripples and scars read as ground, and turn off with the setting.

### R11.T3 Authoritative rocks in `hyperion-surface`

- **R11.T3.a The law and the site.** `RockAbundance`, `exponential_integral_e1`, `RockSite`,
  `rock_site` (from R09's `FieldHeader.surface_age` and `surface_pressure`), `AtmosphereClass`,
  `atmosphere_class`, `rock_abundance`, `AUTHORITATIVE_ROCK_HEIGHT`, `AUTHORITATIVE_ROCK_DIAMETER`
  and `ROCK_HEIGHT_RATIO` in `scatter/{mod,law,site}.rs` (Design note 4), each constant's doc
  comment citing its source; the site values marked "from memory" in the notes (Ghent et al. 2014,
  Bandfield et al. 2011, Christensen 1986) are re-checked by a research agent first. Tests: N(>1.1
  m) over 4 m² at k = 0.05 and 0.10 is 0.078% and 0.75% (Golombek et al. 2012, a published oracle);
  E₁ against tabulated values to 10⁻¹² relative across the range used; N(>0.4 m) over 1,024 m² is
  2.6, 32, 58, 101, 212 and 377 at k = 0.02, 0.05, 0.069 (the Viking 1 lander's value), 0.10, 0.18
  and 0.30, to 1%; k lies in 0.06–0.09 at Viking 1 conditions, 0.15–0.20 at Viking 2's and 0.16–0.20
  at Pathfinder's (researched 2026-09-29: Golombek et al. 2012 give Viking 2 and Pathfinder about
  20% lander-measured and 18% by IRTM; the Viking 1 and near-field Pathfinder values are from
  memory, of medium confidence, and are re-checked against Golombek and Rapp 1997 and Golombek et
  al. 2003 first); k ≤ 0.40 everywhere; k is non-increasing in surface age at fixed material and
  falls faster on an `Airless` body than on one with an `Atmosphere`, classed from
  `FieldHeader.surface_pressure` against `AIRLESS_PRESSURE`; k is zero on `Seabed`, `SeaIce` and
  `Sand`; `AUTHORITATIVE_ROCK_DIAMETER` equals the height over the ratio's upper bound.
- **R11.T3.b Placement.** `RockCellKey`, `ROCK_OCTAVES`, `rocks_in_cell`, `Rock` and its shapes, and
  the tags `surface.scatter` and `surface.scatter.shape` (Design note 5). Tests: the same seed gives
  the same rocks twice; the counts over 10⁵ cells at three k match the law's expectation per octave
  (chi-square at α = 10⁻³, slow); no rock is under 0.4 m across and every rock of 0.2 m exposed
  height or more is present; drawn height ratios lie in [0.25, 0.5] and b/a in [0.55, 0.9]; every
  key is built from integers; a golden of the rocks of twelve pinned cells, one per face and at a
  cube corner.
- **R11.T3.c Contact.** `Rock::top_above`, `Rock::exposed_height`, `rocks_near`, `RockCache` and
  `surface_height_with_rocks`. Tests: an ellipsoid's and a superquadric's top heights against closed
  forms; the exposed height above R10's `ground_at` equals the drawn one; the result is the ground's
  height where no rock stands and never below it anywhere; a rock whose cell lies across a face edge
  or at a cube corner is found from the neighbouring face; a query over a cell that is not surveyed
  on a `PartialField` returns `Err(RockError::Query(NotSurveyed))`, never a height without rocks;
  `hyperion_testkit::order::assert_order_independent` over the cache.
- **R11.T3.d Goldens on three targets and the bump.** `.golden` files, under
  `crates/hyperion-surface/tests/golden/`, of `rocks_in_cell` and
  `surface_height_with_rocks` at pinned points, asserted equal on native, `wasm32-wasip1` and
  `wasm32-unknown-unknown` under R04's machinery; `GENERATOR_VERSION` bumped once, with every golden
  that moves accounted for by `golden_diff.py`. Criterion bench `rocks_per_finest_patch` under
  `just bench`: the rocks of one 32 m finest patch at k = 0.18 and their base heights, against the
  budget's 40 ms patch bake; more than 4 ms (a tenth of it) is a finding.

Files: `crates/hyperion-surface/src/scatter/{mod,law,site,place,shape,contact}.rs`,
`hyperion_surface::tags`, `crates/hyperion-surface/tests/{scatter_golden,scatter_slow}.rs` (every
module with R04's `wasm_bindgen_test as test` import, file-reading tests in `native_only`, and any
`should_panic` test, with its `expected`, in the crate's `tests/panics.rs`), the `.golden` files.
Acceptance: `cargo test -p hyperion-surface scatter`, `just ci` (both wasm targets' fast goldens),
`just test-slow`, `just bench` (recorded with Design note 1's fields).

### R11.T4 Rocks in the client

- **R11.T4.a Baked with the patch.** `rockLayer.ts` and `RockBakeHook`, the worker's side of R10's
  `BakeHook`, holding its own `SynthCache` and the worker's `Synthesiser`: each
  finest-level patch's rocks and their base heights, computed in the worker beside its heights as an
  `ExtraLayer` and transferred and cached with the patch; `rockMesh.ts` tessellates each rock to the
  level Design note 6 picks; instances are placed relative to the patch's `f64` origin, as every
  patch vertex is. Tests: a patch's list equals `rocks_near` over its footprint (the wasm build in a
  worker against the same build on the main thread, in Vitest); every tessellated vertex lies on its
  analytic surface to the `f32` step; the chordal bound holds for 10³ random rocks.
- **R11.T4.b Draw rules.** `rockSelection.ts`: `couldTouch` (Design note 7) from the scene's
  grounded and descending bodies, their footprints and a given path; which rocks are drawn for a
  camera, a setting and a style; and when `TERRAIN: DETAIL LIMITED` is raised for them. The
  wireframe draws them as depth-writing geometry stroked at the silhouette. Tests: R05's
  grounded-body rule test extended to rocks, on both settings and in both styles; the set is a
  function of pose, field of view, viewport, setting and bodies alone; the low setting raises the
  annunciation exactly when a finest-level patch with pixel-sized rocks is in view outside the
  region.
- **R11.T4.c Collision agrees, with rocks.** R10's collision-agrees test (R10.T15) gains rocks. In
  Rust, on native and both wasm targets under R04's machinery, goldens of
  `surface_height_with_rocks` at 10⁴ sample points on four sampled worlds; in Vitest, against the
  wasm build of those same points, the drawn surface (R05's finest mesh formed in `Math.fround`
  from a committed bake fixture, and T4.a's rock meshes) agrees with them within the chordal bound
  where a rock stands and the `f32` step of the height texture elsewhere.
- **R11.T4.d The swept region, into R05's selection.** In `rockSelection.ts`,
  `couldTouchContacts(bodies, sweepS)`, which sweeps each descending body's footprint along R03's
  `predictedPath` over `sweepS` = R05's `FORCED_REGION_RESIDENCY_S`, widened by
  `extrapolationMargin` where the path is an extrapolation, and covers it with a chain of
  `GroundContact` discs spaced at most one radius apart in R05's `SelectionInput.grounded` (R05's
  Design note 9). Tests: the union of R05's forced region over those contacts covers every
  `couldTouch` region, for grounded bodies and for descents at 2, 20 and 300 m/s; a scripted fast
  descent in R05's harness never reaches a rock before its patch is drawn; an unpowered ballistic
  arc stays inside the widened sweep of its straight-line extrapolation over the horizon; a craft
  with a `planned_path` is swept along it without widening.

Files: `surface/scatter/{rockLayer,rockMesh,rockSelection}.ts`,
`surface/scatter/rock.wgsl`, `crates/hyperion-surface/src/scatter/bake.rs` (`RockBakeHook`), R05's
worker and selection input, R01's `WGSL_CATALOGUE`, tests. Acceptance: `pnpm test`, `just ci`,
`just test-render`; by hand, recorded: in R05's
scripted descent onto a rocky plain, the rocks under the lander are drawn on the low setting and in
the wireframe, and the frame time is recorded with them.

### R11.T5 Craters between 1 and 2 m (open question 18)

Record Design note 20's ruling with its sources and revisit triggers, and propose to the owner the
brainstorm's open question 18 status ("ruled decoration") and its corrected depths; the brainstorm
itself is not edited here. No dip is built, no tag reserved and no version bumped. Files: this plan
and the notes. Acceptance: the ruling is recorded and `pnpm format:check` passes.

### R11.T6 Decorative scatter

- **R11.T6.a Placement on the GPU.** `pebbles.wgsl`: a compute pass per visible patch places
  instances below 0.4 m across from an integer lattice hash, from the same size–frequency law below
  the threshold, filtered by R10's baked class weights and slope, falling off with distance by pixel
  footprint, and drawn indirectly; `scatterCull.ts` and its WGSL twin cull instances against every
  body whose bounding volume comes within 0.2 m of the ground, each box widened by the instance's
  radius (Design note 7). High setting only.
- **R11.T6.b Tests and bench.** No decorative instance is 0.4 m across or more (CPU reference over
  10⁵ draws); the number per size bin is continuous across 0.4 m with T3's authoritative rocks, to
  within the Poisson interval; the cull predicate against boxes, including an instance straddling a
  box face and a body hovering 0.1 m up; the low setting creates no scatter pipeline; the smoke
  harness runs the pass; `scatterDensity` scales the instance count linearly, and 0 creates no
  pipeline. Bench: scatter instances on both GPUs against the target's 1–2 ms.

Files: `surface/scatter/{pebbles.wgsl,scatterCull.ts,scatterCull.wgsl}`, R01's `WGSL_CATALOGUE`,
tests. Acceptance: `pnpm test`, `just ci`, `just test-render`, by hand at the surface.

### R11.T7 Clouds

- **R11.T7.a Optics and coverage.** `cloudOptics.ts` (δ-Eddington, Design note 10), `condensates.ts`
  and `coverage.ts` (Design note 8). The stand-in optics, at 0.55 µm and each with its citation
  re-checked first where the notes mark it "from memory": liquid water r_eff 10 µm, g 0.85–0.87, ϖ₀
  ≈ 1 − 10⁻⁵, median column τ about 8 (Hansen and Travis 1974); terrestrial ice r_eff 30 µm, g about
  0.75, τ 0.1–3 (Yang et al. 2013); Martian water ice r_eff 1–4 µm, g 0.7–0.8, τ 0.1–1 (Clancy et
  al. 2003); Martian CO₂ ice r_eff 0.5–3 µm, g 0.6–0.8, τ 0.01–0.6 (Montmessin et al. 2007); liquid
  methane g 0.85–0.9, τ a few (Griffith et al. 1998); the phase chosen by temperature, liquid above
  260 K, as ISCCP's switch. The coverage uses the zonal climatology asked of R09, with a provisional
  stand-in (a rain band on the subsolar latitude, dry belts and storm tracks, labelled provisional)
  until R09 provides it. Tests: δ-Eddington's reflectance against the twelve cases below to 10⁻⁴,
  and against their Monte Carlo column within the stated error (2.5% for τ ≥ 10, 10% at τ = 1 with
  an oblique sun). The table was computed on 2026-09-29 from the conservative δ-Eddington solution
  (ϖ₀ = 1; f = g², τ′ = (1 − g²) τ, g′ = g ÷ (1 + g); γ₁ = 3(1 − g′) ÷ 4, γ₃ = (2 − 3g′μ₀) ÷ 4;
  R = [γ₁τ′ + (γ₃ − γ₁μ₀)(1 − e^(−τ′ ÷ μ₀))] ÷ (1 + γ₁τ′); Joseph, Wiscombe and Weinman 1976,
  Meador and Weaver 1980) and from a plane-parallel Henyey–Greenstein Monte Carlo of statistical
  error about 0.002, which the task re-runs in its Vitest reference at a smoke size:

  | τ   | g    | μ₀  | Monte Carlo | δ-Eddington |
  | --- | ---- | --- | ----------- | ----------- |
  | 1   | 0.85 | 1.0 | 0.0429      | 0.0467      |
  | 1   | 0.85 | 0.5 | 0.1649      | 0.1490      |
  | 10  | 0.85 | 1.0 | 0.4216      | 0.4191      |
  | 10  | 0.85 | 0.5 | 0.6033      | 0.5880      |
  | 50  | 0.85 | 1.0 | 0.8078      | 0.8113      |
  | 50  | 0.85 | 0.5 | 0.8702      | 0.8679      |
  | 1   | 0.75 | 1.0 | 0.0787      | 0.0833      |
  | 1   | 0.75 | 0.5 | 0.2410      | 0.2193      |
  | 10  | 0.75 | 1.0 | 0.5676      | 0.5663      |
  | 10  | 0.75 | 0.5 | 0.7059      | 0.6956      |
  | 50  | 0.75 | 1.0 | 0.8774      | 0.8795      |
  | 50  | 0.75 | 0.5 | 0.9161      | 0.9157      |

  Further tests: plain Eddington is not used (it gives R < 0 at τ = 1, g = 0.85, μ₀ = 1); R + T + A
  = 1 with A ≥ 0 for ϖ₀ 0.99, 0.999 and 1, R rising monotonically in τ, and the direct part e^(−τ ÷
  μ₀) with the unscaled τ; the coverage's area-weighted mean equals the global fraction to 10⁻⁴ for
  a fully surveyed field, with cells weighted by their true solid angle; surveying further cells
  changes no coverage outside the blend band; within one flow-map phase advection by a uniform wind
  is a rotation, and the field is continuous in t across phase boundaries; two runs begun at
  different times give the same coverage at the same t, bit for bit; the time-rate fade reaches the
  mean above its named rate.

- **R11.T7.b The two-dimensional layer.** `cloudDensity.ts`, `cloudLayer.wgsl` and
  `cloudShadow.wgsl`: a shell at the condensate's altitude, its per-texel τ the column integral of
  the shared density, lit through R08's `atmosphere_sun_transmittance` at the shell's geodetic
  height and latitude and the sun's azimuth, shaded by δ-Eddington, entered in R02's
  transparent-layer order; its direct transmittance along the sun, from one tap at the sun ray's
  crossing of the shell, into R10's `terrain_shadow_factor`, and its diffuse transmittance into
  R10's `terrain_sky_factor`, behind the `cloudShadows` entry (off on the low setting, Design note
  10); the low setting's clouds. Tests: under a τ = 10 column at μ₀ = 1 the direct factor is below
  10⁻⁴ and the ground's total irradiance is within 2% of T_total times the unclouded total; with
  `cloudShadows` off neither hook is written; the smoke harness; every texel finite. Bench on the
  UHD 620 at 720p, by the owner, against about 1 ms and under 16 MB, with the development machine's
  figure beside it.
- **R11.T7.c The volume.** `cloudVolume.wgsl` with the noises, march, integration, phase and octaves
  of Design note 10, and the transmittance map for its shadows; high setting only. The noise
  textures are generated into a storage-capable format and copied, or generated on the CPU once,
  after checking the core format table. Tests: the two lobes' weighted asymmetry equals the
  condensate's g; the octaves' fit to δ-Eddington over τ 1–50 and μ₀ 0.5–1, with its error in the
  doc comment; per column, the TypeScript reference march matches δ-Eddington for that column's τ to
  3–5% at μ₀ ≥ 0.5; per coverage cell, the mean of per-texel δ-Eddington equals the march's cell
  mean to 2%; the smoke harness. Bench on the discrete target (the development machine's RTX 3080)
  against 1.5–3 ms and 10–20 MB, and by the owner on the UHD 620 for the record.
- **R11.T7.d The switch.** Switching the setting, or the style, leaves the coverage and density
  fields identical and the cell-mean brightness within the T7.c tolerance. By hand, recorded: an
  Earth-like world from orbit, from within the layer and from below, on both settings, and its zonal
  mean cover against ISCCP's or MODIS's pattern (high at the ITCZ, minima near ±20–30°, high near
  ±55°).

Files: `clouds/*`, R10's hooks, R01's `WGSL_CATALOGUE`, tests. Acceptance: `pnpm test`, `just ci`,
`just test-render`, and the recorded runs.

### R11.T8 Oceans and glint

- **R11.T8.a Sea state.** `seaState.ts`, `gerstner.ts` and `liquids.ts` (Design note 12); the
  figures the notes mark "from memory" (Pierson–Moskowitz and JONSWAP constants, Monahan and
  O'Muircheartaigh, the liquids' indices, the Titan threshold) are re-checked first. Tests: σ_u² and
  σ_c² at three wind speeds against Cox and Munk; the log-profile conversion gives U₁₂.₅ ≈ 1.02 U₁₀
  and U₁₉.₅ ≈ 1.06 U₁₀; every `gerstnerSet` for winds of 0–30 m/s, gravities of 0.1–3 g and 8 and 32
  components has Σ Qᵢ wᵢ Aᵢ ≤ 1 and every kᵢAᵢ ≤ 0.44; per axis, resolved (1 − √(1 − k²A²)) plus
  unresolved variance equals the Cox–Munk total to 10⁻⁶, and the unresolved never falls below the
  10⁻⁴ floor; the Lagrangian mean vertical displacement is zero and the Eulerian mean within O(Σ
  kA²); the fetch limit caps H_s for a basin and reaches Pierson–Moskowitz at full development;
  `equivalentWind` is the identity at Earth's parameters; a Titan-like sea at 0.3 m/s is at the
  floor and glints as a mirror.
- **R11.T8.b From space.** `oceanMask.ts` and `oceanFar.wgsl`: the sea-level sphere where the mask
  says ocean, the anisotropic Cox–Munk BRDF filtered per pixel, Fresnel at the liquid's index, the
  sky reflected from R08's per-view sky-view table (`skyView`), the upwelling colour by Pope and
  Fry's absorption over the water's thickness, whitecaps on the high setting, and the glint's energy
  above the half-float range handed to R07's tone-mapping pass as a `GlareSource`. Tests: the mask
  is a water surface above coarse elevation and never an ice-covered cell; a closed basin is drawn
  at its own level; no sea over unsurveyed cells; the specular point on the sphere against a CPU
  reflection solution for three geometries; the smoke harness.
- **R11.T8.c Near the surface.** `oceanNear.wgsl`: below 20 km (Bruneton et al. 2010's switch) the
  projected grid under the camera, displaced by the Gerstner set, 32 components on the high setting
  and about 8 on the low, refraction on the high setting only, the unresolved variance kept in the
  BRDF, the water's thickness from depth within 10⁵ m and from the bathymetry beyond. Tests: the
  glint's integrated radiance over a patch of sea is continuous across the 20 km switch to 2% in the
  reference; the lower-layer cull at 10⁻⁶ of the camera distance.
- **R11.T8.d The shoreline.** `shoreline.wgsl`: amplitude attenuated by depth, foam from depth and
  the terrain's slope, the depth fade. By hand, recorded: a coast from 1 km, 100 m and 2 m.
- **R11.T8.e Benches.** Both GPUs (the development machine's RTX 3080, and the UHD 620 by the
  owner), against 1–2 ms on the target and 2–3 ms on the UHD 620's low
  setting, and by eye: a world with oceans is recognisable from orbit by its glint alone, and a
  frozen sea reads as ice.

Files: `ocean/*`, R01's `WGSL_CATALOGUE`, tests. Acceptance: `pnpm test`, `just ci`,
`just test-render`, and the recorded runs.

### R11.T9 Rings

- **R11.T9.a Photometry.** `ringPhotometry.ts`, `ringProfile.ts` (provisional, Design note 13) and
  `ringShadows.ts` (Design note 14), with `ringShadow.wgsl` and its `ring_shadow_on_body`, which
  R07's `litBody.wgsl` calls through its stub and R10's `terrain_ring_shadow` hook calls; the dusty
  lobe's g, marked "from memory", is re-checked first. Tests: the lit and unlit forms against
  hand-computed values at three geometries and two τ, the factor ¼ included, and the unlit form's
  limit at μ → μ₀; the power-law phase normalises over 4π with n = 3.09; the surge's half-width is
  0.20°, 0.26° and 0.28° for the three ring kinds and its amplitude falls with τ; a dense ring is
  darker from its unlit side than a sparse one, and a dusty ring is brighter back-lit than
  front-lit; the planet's shadow on the ring against a ray–sphere oracle, with a penumbra of the
  stellar disc's angular diameter times the distance; the ring's shadow on the planet is exp(−τ(r) ÷
  |μ₀|) at the plane crossing; each gap of a `RingDto` fixture is a band starting at its radius,
  0.039 of the radius wide, at 0.1 of the ring's τ.
- **R11.T9.b The shadowing table.** A `ring-shadowing` subcommand of `hyperion-fit`, beside R08's
  reference tracer and outside the table manifest (Design note 15): photon tracing with multiple
  scattering, tallied by order, through a vertically Gaussian layer (H_eff ≡ √(2π) σ) of R⁻³
  particles of 0.5–5 m radius, D₀ tied to τ_dyn, indexed by τ_phot (Design note 15), in `f64`,
  seeded, writing the table as a generated TypeScript module of unsigned 16-bit entries under 50 kB,
  its header in the module's doc comment. Tests: a slow Rust test reruns the Monte Carlo at a smoke
  size and finds D₀ = 0.35 at τ_dyn = 1.5 with H_eff = 11.2 m; the single-scattering ratio f_e(26°)
  ÷ f_e(4.5°) at α = 6°, τ_dyn = 1.5 within 1.30 ± 0.05, and at α = 0.5° about 1.15 ± 0.05; with
  τ_dyn = 2.0, the n = 3.09 phase and Bond albedos of 0.57 and 0.21, the I/F at B = 4.5°, α = 6° is
  0.44 ± 0.05 at 814 nm and 0.16 ± 0.03 at 336 nm; the 0.1–5 m cross-check is recorded beside them;
  a uniform layer at D = 0.05 fails the ratio (so the test has teeth); a fast Vitest reads the
  committed module and checks its header, size and bounds. Hapke's fallback is measured against the
  table and its error recorded.
- **R11.T9.c The slab.** `ringSlab.wgsl`: the annulus in the ring plane with the profile texture at
  the pixel's footprint, both faces, the planet's shadow on the ring on both settings and the ring's
  on the planet where `ringShadowOnBody` is on (the high setting, Design note 18), the table on both
  settings, R02's transparent-layer order and no depth bias; and the same photometry for R07's far
  annulus. Tests: the smoke harness;
  the ring projects to the same pixels in both styles as R02's wireframe ellipse to half a pixel.
  Bench against 0.5–1 ms and under 0.5 ms.
- **R11.T9.d Close to.** `ringSplit.ts` and `ringParticles.wgsl` (Design note 16): the hand-over by
  angle, the instanced share ln(s_max ÷ s_pix) ÷ ln(s_max ÷ s_min) with s radii and s_pix half the
  pixel footprint, clamped to [0, 1], removed from the slab, the shearing cells, the local particle
  field; discrete target only. `instancedShare(pixelFootprintM, radiusMinM, radiusMaxM)` takes the
  footprint and halves it itself. Tests: the hand-over distance is 1,830 × D at 1080p, 1,220 × D at
  720p and 3,670 × D at 4K to 1%, with D the particle's diameter; over radii of 1 cm–5 m the share
  is 0 at a 10 m footprint, 0.371 at a 1 m footprint and 1 below 2 cm, and continuous; along 10⁴
  random rays through a split ring the expected photometric transmittance of slab and instances
  together equals the unsplit slab's to 1%; instances never appear on the low setting. Bench on the
  discrete target, with SpaceEngine's published figures as the comparison.

Files: `rings/*`, `crates/hyperion-fit/src/rings/{mod,layer,trace}.rs` and its subcommand in
`crates/hyperion-fit/src/cli.rs`, `rings/shadowingTable.ts` (generated), R01's `WGSL_CATALOGUE`,
tests. Acceptance: `pnpm test`, `just ci`, `just test-slow`, `just test-render`, and by hand,
recorded: a Saturn-like ring front-lit and back-lit, with its shadow on the planet and the planet's
on it.

### R11.T10 Still images

- **R11.T10.a The job's arithmetic.** `stillPlan.ts` and `batchController.ts` (Design note 17).
  Tests: tiles cover the image exactly once, each no larger than the adapter's limit or
  `STILL_TILE_MAX_PX`, with widths a multiple of 64 px and `bytesPerRow` a multiple of 256; the
  tiles' projections compose to the full view's, so a body lands on the same pixel of the still as
  of an untiled render of the same size; a decoration fade and a terrain level chosen for a tile
  equal those for the same pixel untiled; with timestamps, no proposed batch's predicted time
  exceeds the cap less two quanta; with none, a controller fed round trips of GPU time plus a 3 ms
  overhead never proposes a batch whose GPU time exceeds the cap, grows by at most 1.5 times and
  halves after an overrun; no planned dispatch exceeds 50 ms at the measured cost per workgroup.
- **R11.T10.b Rendering.** `stillJob.ts`: the scene snapshot with the cells surveyed at the request,
  the offscreen target through R01's `createRenderTarget`, every high-setting pipeline created
  asynchronously (`createMaterialAsync`, `createComputeAsync`) before the first batch, each tile
  read back through R01's `readTexture`, batch times from R01's `onPassTimes` where
  `GraphicsStatus.timer` is not `"absent"`, one batch of whole passes per animation frame
  gated by `onSubmittedWorkDone()`, jittered accumulation at full resolution, fixed exposure, bloom
  on the assembled image, the streaming wait and its timeout, the suspension rule on device loss
  with R01's fault. Tests: with a fake device, only one batch is ever in flight and no synchronous
  pipeline creation is called during a job; cells surveyed while the job runs are not used; a loss
  mid-job abandons it, reports once and suspends stills, a second disables them; the smoke harness
  completes a small still on SwiftShader with every texel finite.
- **R11.T10.c Label, store and control.** `StillLabel`, `pngLabel.ts`, `stillStore.ts` over the
  preload's `saveStill`, `listStills` and `readStill`, their handlers in `src/main/stills.ts` (the
  sender frame validated, arguments `unknown` until the size and the label's schema are checked, the
  write atomic by temporary file and rename), `StillControl` on the view (keyboard-operable, its
  progress a `progress` element, `STILLS SUSPENDED` when suspended) and `StillList` (the saved
  stills with their labels in B612 Mono). Tests: the label carries every field of Design note 17;
  the PNG's `iTXt` label parses back equal to the sidecar; a still whose frustum had not streamed is
  labelled `TERRAIN: STREAMING`; the handlers refuse a foreign sender, an oversized image and a
  malformed label; the control and list by keyboard, through `getByRole`.
- **R11.T10.d The soak, by hand, recorded.** On the otherwise idle development machine (RTX 3080)
  and, by the owner, on an otherwise idle UHD 620, whose i915 preemption limit is the one the cap
  guards, at the batch cap: repeated
  stills of a cloudy, ringed and oceanic scene for 30 minutes beside the live view, recording the
  live view's frame time, lost devices and GPU-process restarts (none allowed), the batch sizes the
  controller settled on with and without timestamps, the first still's pipeline-compile time, the
  streaming wait against the 20 s timeout, and whether 2,048² tiles fit the memory ceiling.

Files: `stills/*`, `apps/hyperion/src/main/stills.ts`, `apps/hyperion/src/preload/api.ts`, tests.
Acceptance: `pnpm test`, `just ci`, `just test-render` (T10.b's small still), and the recorded soak.

### R11.T11 Verification pass

Run every slow test and bench above on a quiet machine, record the figures in the doc comments and
this plan's "as built" notes with the fields of Design note 1 for R12's first task, confirm that
every shader of this plan runs in R01's smoke harness on its no-f16 and no-subgroup paths, check the
flash limit on 10-second recorded sequences of glint, pebbles and ring particles (no region above
the guide's area limit alternating more than three times a second), and check by eye the scenes of
Verification. Acceptance: `just ci`, `just ci-slow` and the recorded runs complete.

## Verification

- **Truth and decoration:** decoration never moves a vertex (T2.b's depth read-back); no decorative
  instance reaches 0.4 m across, and the size distribution is continuous across it (T6.b);
  `DECORATION ON` shows exactly while decoration is drawn, never on the low setting, and every still
  records it.
- **Rocks:** goldens equal on native and both wasm targets; N(>D) meets Golombek et al. 2012's
  oracle and counts match the law (slow); the drawn surface and `surface_height_with_rocks` agree
  within the chordal bound (T4.c); every rock in a "could touch" region is drawn on the low setting
  and in the wireframe, and a fast descent meets none undrawn (T4.d).
- **Clouds:** δ-Eddington meets its fixtures; the two settings agree on coverage, on density and on
  cell-mean reflectance to 2%; the global cloud fraction is met, and surveying moves no cloud
  already drawn.
- **Oceans:** every Gerstner set's summed steepness is at most 1 and each component's at most 0.44;
  the slope budget holds across the 20 km switch; sea ice replaces the ocean wherever the coarse
  field has it.
- **Rings:** the shadowing table meets Salo and French's tilt ratio and I/F, and a uniform layer
  fails them; extinction is conserved across the split; the shadows match their ray oracles.
- **Stills:** one batch in flight, none over the cap, no synchronous compile, and the soaks on the
  development machine and the UHD 620 lose no device.
- **Flash limit:** the recorded sequences of T11 pass.
- **Benches:** decoration, scatter, clouds, ocean and rings on both GPUs against the budget's rows,
  and `rocks_per_finest_patch`, all recorded on a quiet machine; a miss is a finding for R12, not a
  failure.
- **By eye:** an Earth-like world from orbit to 2 m above a rocky shore on both settings; a
  Mars-like plain with its rocks; a Saturn-like ring front-lit, back-lit and from inside its
  hand-over range.

## Generator version

T3 bumps `GENERATOR_VERSION` once, because the authoritative rocks are local synthesis, which the
brainstorm's open question 4 puts under the generator version with the coarse pass and its wire
quantisation. Nothing else here moves generated output: decoration, decorative scatter, clouds,
oceans, rings and stills are presentation, and open question 18's ruling builds nothing. The plan
reserves the tags `surface.scatter` and `surface.scatter.shape`, and the four octave levels of
`ROCK_OCTAVES` in `ObjectKey::surface_cell`'s level field, so that a later rock class (an
ejecta-block population), shape parameter or authoritative dip opens a new tag and moves no rock.

## Risks and open points

- **The rock site mapping** (Design note 4) is a ruling of medium to low confidence: no published
  law covers generated worlds, and on airless worlds the exponential law cannot fit the lunar size
  distribution, so small rocks there are knowingly wrong. A lunar boulder size distribution (for
  example Li et al. 2017) would calibrate an airless k_base or motivate a power-law option. Ejecta
  blocks larger than 6.4 m, about 0.29 D^0.66 for a crater of diameter D (Bart and Melosh 2010, from
  memory), belong to a crater-keyed population that no task builds.
- **Sea state off Earth** (Design note 12) is a labelled heuristic of low confidence; Hayes et al.
  2013 or an Elfouhaily spectrum with g, γ ÷ ρ and u★ substituted would settle it.
- **Plan 14's cloud fraction** (Design note 9's finding) gives a Mars-like world Earth's 67% and a
  Titan-like world none, until plan 14 makes it depend on the condensable. Until then this plan's
  clouds on those worlds are wrong in amount, though right in optics.
- **The zonal climatology** is an ask of R09; until it exists the clouds over unsurveyed ground use
  T7.a's provisional stand-in.
- **The sea over unsurveyed ground** is not drawn (Design note 11), which removes the glint that
  makes an ocean world recognisable before a survey. A disc-statistical glint on R07's reference
  body is suggested and left to the owner.
- **The decoration label's scope** (Design note 2) is this plan's reading of guide item 2; the owner
  signs off the guide, and a wider reading only lengthens the annunciation.
- **Rings wait on plan 14** for a radial profile, a size distribution and a thickness on the wire,
  and for poles (P14.T14). Until then Design note 13's provisional profile draws plain rings with
  Division-like gaps, and the layer and split read the sim's constants.
- **Gas giants' cloud bands** are owned by no plan (Design note 9). R07's Risks say R11 leaves them
  to R08 and R07; this plan leaves them to neither R08 nor itself and advises R07. The roadmap
  should assign them.
- **The shadowing layer's size range** (Design note 15) is Salo and French's 0.5–5 m, not plan 14's
  1 cm–5 m, which the brainstorm's lean names; the choice of W = 10 is of medium confidence, the
  cap of D₀ at 0.40 at high τ of low confidence (researched 2026-09-29). A run at 1 cm–5 m, recorded
  in T9.b, shows what the difference costs.
- **Paths and residency are stand-ins** (Design note 7): until a `planned_path` is sent, R03's
  `predictedPath` is a straight line and the sweep's ½ g t² widening bounds only an unpowered arc,
  and `FORCED_REGION_RESIDENCY_S` is provisional until R05.T18. A powered descent that manoeuvres
  off its extrapolation inside the horizon could reach ground whose rocks are not yet drawn; the
  sessions plan's or the flight model's `planned_path` removes the risk.
- **Cloud shadows on the low setting** are off, as the brainstorm's ladder says, though they cost
  under 0.1 ms; whether to turn them on is proposed to the owner as a brainstorm correction.
- **Stills' timings** come from one probe under shared load; the tile size, the streaming timeout
  and the controller's settled batches are measured in T10.d, and the tile constants may move.
- **The ocean on the quadtree.** Open question 7's risk is reduced by drawing near water on a
  projected grid and keeping waves out of the authoritative surface; a later ruling that waves are
  collidable, or a tide model, reopens it.
- **Rocks collide with nothing yet.** The contact query is tested against the drawn meshes and
  across targets, but no flight model calls it; the plan that builds one inherits it.
