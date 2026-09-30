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
- The view's survey coverage readout, the survey edge, and the cursor readouts (`ELEVATION`,
  `SLOPE`, `SLANT RANGE`) with coverage- and resolution-gated uncertainty.
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
`BandLevel`, `DetailSeed`, `ResolutionCode`), R05's (`PatchKey`, `Face`, `FaceUv`, `HeightSample`,
`bake_patch`, `BakeOptions`, `PatchBake`, `finest_surface_height`, `NormalScale`) and R02's
(`BodyFixedPosition`, `BodyFixedVector`) are theirs.

### `material`

```rust
/// Authoritative ground classes. Discriminants are wire- and golden-stable `u8`s; a new class
/// appends, and any change to the set, the table or the classifier is a generator-version change.
#[repr(u8)]
pub enum MaterialClass {
    Bedrock = 0, Regolith = 1, Soil = 2, Sand = 3, Sediment = 4, VolcanicRock = 5, Melt = 6,
    WaterIce = 7, Snow = 8, SeaIce = 9, VolatileFrost = 10, OrganicSediment = 11, Seabed = 12,
    Evaporite = 13, Dust = 14,
}
pub enum FrostSpecies { CarbonDioxide, Nitrogen, Methane }   // carried beside VolatileFrost
pub struct Material { /* class: MaterialClass, frost: Option<FrostSpecies> */ }

pub struct MohrCoulomb { /* cohesion: Pascals, friction_angle: Radians */ }
pub enum FrictionModel { Constant(f64), ByTemperature(&'static [(Kelvin, f64)]) }  // tan δ, pad
pub struct MaterialProperties {
    /* normal_albedo: Option<(f64, f64)> (A_N range, visible), phase: PhaseLaw (f(α)),
       shear: Option<MohrCoulomb>, bulk_density: Option<KgPerCubicMetre>,
       interface_friction: Option<FrictionModel>, compressive_strength: Option<Pascals> (ice),
       sources: &'static [&'static str] */
}
pub fn properties(material: Material) -> &'static MaterialProperties;
/// L(0) of the lunar-Lambert law from the normal albedo (Design note 8).
pub fn limb_parameter(normal_albedo: f64) -> f64;
/// Ultimate bearing capacity under a footing (Design note 2); `None` where the table has no data.
pub fn bearing_capacity(p: &MaterialProperties, gravity: MetresPerSecondSquared,
    footing_width: Metres) -> Option<Pascals>;
pub fn interface_friction(p: &MaterialProperties, temperature: Kelvin) -> Option<f64>;

/// The authoritative class at a point, from the finest level's slope, altitude, R09's climate and
/// surface-state classes, and the ice field.
pub fn material_at<F: FieldView>(synth: &Synthesiser<'_, F>, cache: &mut SynthCache,
    at: FaceUv) -> Result<Material, QueryHeightError>;
pub const CLIFF_SLOPE: Radians;            // 40°, Design note 1
pub const TRANSITION_HALF_WIDTH: Radians;  // 5°, the visual blend band either side
pub const SNOW_SHED_SLOPE: Radians;        // provisional, T1.a finds a source or removes it
/// Presentation weights over a per-patch palette of up to eight classes (Design note 3).
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
/// triangle it came from. Collision reads this and nothing else.
pub fn ground_at<F: FieldView>(synth: &Synthesiser<'_, F>, cache: &mut SynthCache,
    p: &BodyFixedPosition) -> Result<GroundPoint, QueryHeightError>;
pub struct GroundPoint { /* height: Metres (above the reference radius), normal: BodyFixedVector,
    triangle: FinestTriangle */ }
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
    grazing: bool, hit: BodyFixedPosition */ }
pub fn slant_range<F: FieldView>(synth: &Synthesiser<'_, F>, cache: &mut SynthCache,
    origin: &BodyFixedPosition, direction: &BodyFixedVector, max: Metres)
    -> Result<Option<SlantRange>, QueryHeightError>;
pub const SLOPE_BASELINES: [Metres; 3];   // 2 m (default), 5 m, 100 m, Design note 7
```

### `bake`

```rust
/// What this plan adds to R05's `PatchBake`.
pub struct TerrainLayers {
    /* palette: [Option<Material>; 8], weights: Vec<[u8; 8]> (65 × 65),
       survey_mask: Vec<u8> (65 × 65: 255 surveyed, 0 not), horizon: HorizonMap,
       bound: Metres, sigma: Metres, extra: Vec<ExtraLayer> (R11's, through `BakeHook`) */
}
pub struct HorizonMap { /* azimuths: 8, near: 65 × 65 × 8 f16, far: 9 × 9 × 8 f16,
    as sin(elevation), Design note 10 */ }
/// R11 adds per-patch layers (the rock list) through this, in the same bake and cache.
pub trait BakeHook { fn bake(&self, key: PatchKey, heights: &[f32], out: &mut Vec<ExtraLayer>); }
/// The face-wide far-field lattice, keyed by position alone (Design note 10).
pub fn far_field_point(face: Face, level: u8, i: u32, j: u32) -> FaceUv;
```

The worker's WebAssembly entry points, on R05's binding, gain `field_init`, `field_chunk`,
`field_release` and a `bake` that takes R05's `BakeOptions` with this plan's settings, and answer
`field_init` with the body's `LevelBounds` (below).

### Client (`apps/hyperion/src/renderer/src/view/terrain/`)

- `coarseFieldStore.ts`: `CoarseFieldStore` with `want(body, reason)`, `release(body)`,
  `coverage(body): CoverageView`, `onRevision`; it drives R09's `surface_field` and posts to R05's
  `HeightWorkerPool`.
- `coverage.ts`: `CoverageView` (`resolutionAt(cell)`, `surveyedFraction()`, `bestResolution()`),
  `surveyEdges(face, level)` for the edge mark, and the GPU coverage mask per face.
- `levelBounds.ts`: `LevelBounds { hard: Float64Array; sigma: Float64Array }` per body, and
  `selectionBound(bounds, level, rule)`, which R05's `screenSpaceErrorPx` reads.
- `classMap.ts`: the per-body coarse class-weights map R07's disc samples.
- `shaders/terrainLit.wgsl` (the file R07 names) with `terrainMaterial.ts`: palette and weights to
  `body_brdf`'s per-class law, the survey mask, skirts at the survey edge, and the hooks
  `terrain_decoration` and `terrain_shadow_factor` that R11 fills.
- `shaders/terrainWire.wgsl` with `terrainWire.ts`: the depth-only pass and its contours;
  `contourInterval(aglM, previous)`, `contourPhase(patchOriginHeightM, intervalM)`.
- `shadows/horizon.wgsl`, `shadows/cascades.ts`: the two shadow paths and their combination.
- `readouts.ts`: `formatWithUncertainty`, `formatElevationReading`, `formatSlopeReading`,
  `formatSlantRange`, and the `SurveyLine` and `TerrainReadout` components in the view's label
  block.
- `cacheBudget.ts`: `PATCH_CACHE_BYTES` per setting, `bytesPerPatch(options)`, and
  `patchCountModel(camera, tau, viewport)`, the pure count model of Design note 15.

### Test helpers

`crates/hyperion-surface/tests/common/worlds.rs`: `sample_worlds()`, R09's reference worlds
(`hyperion_sim::planetary::surface::reference::{earth_like, mars_like, moon_like, ceres_like}`) plus
an icy moon and a locked world built with `CoarseInputs::builder()`, each run through the coarse
pass once per test binary; and `flat_field(height)`, `cliff_field(height, strike)` built with R09's
`FieldBuilder` for the horizon and classifier tests.

## Consumes

Names are the owning plans' as they stand on disk; the owner is authoritative, and where a name has
changed by the time this plan runs only the call sites here change.

- **R01:** the engine adapter, its WGSL-only material path, its adapter feature report
  (`GpuCapabilities`, with `depthClipControl` and `timestampQuery`) and the headless SwiftShader
  smoke harness, with its no-f16 and no-subgroup runs.
- **R02:** `coords::BodyFixedPosition` and `BodyFixedVector` and their client mirror; the view's
  camera, per-patch origin convention and rotation-only view matrix; the wireframe style's line
  tokens, casing and label block; the DOM list; the graticule; the nine drafted guide items, of
  which this plan extends items 1, 3, 5 and 7.
- **R03:** the binary frames the chunks ride, their backpressure, and the scene topic carrying R09's
  `surface_revisions`.
- **R04:** the `hyperion-surface` crate with its `clippy.toml` and relaxed-SIMD `compile_error!`;
  the base crate's `math` (a `j0` wrapper over `libm::j0` is asked for Design note 7), `rng` and
  `units`; both wasm targets' fast goldens in `just ci`; the testkit's `include_str!` arm.
- **R05:** `cube` and `geometry` (`Face`, `FaceUv`, `PatchKey`, `finest_level`, `MAX_LEVEL`,
  `FINEST_SPACING_M`);
  `patch::{bake_patch, BakeOptions, PatchBake, VertexPath, NormalScale, finest_surface_height}`,
  built for its test planet, which this plan points at R09's synthesis (Design note 4); the level
  bound's role in selection (its Design note 15) and `screenSpaceErrorPx`; `selectPatches`,
  `morphHold`, `GroundContact` and the grounded-body rule; `PatchCache` and `resolveDrawSet`;
  `HeightWorkerPool` with `postField`; `terrainPass.ts`; `terrainAnnunciation`; the scripted
  descent, `descentProfile.ts`, `metrics.ts` and the results files under
  `docs/measurements/descent-spike/`.
- **R07:** `body_brdf(law: PhotometricLaw, mu0, mu, alpha)`, `sphere_irradiance` and
  `eclipse_visible` in `shaders/litBody.wgsl`; `BodyAppearance`, `LitRegime`
  (`"point" | "disc" | "mesh"`) and `lawFor`; the star's angular radius per view.
- **R08:** aerial perspective applied to the terrain pass's output, and the sun's refracted apparent
  elevation where it draws one (Design note 10).
- **R09:** `field`'s `FieldView`, `PartialField`, `CoarseField`, `FieldHeader`, `SynthesisCell`,
  `ClimateCell`, `CoarseCrater`, `Cover` and `SYNTHESIS_MARGIN_CELLS`; `wire::decode_payload`;
  `synth`'s `Synthesiser`, `SynthCache`, `BandLevel`, `level_bound_m` and `unresolved_rms` (over
  R05's `HeightSample`, `BAND_LIMIT_M` and `FINEST_SPACING_M`); `SurfaceClass`; `DetailSeed`; the
  `surface_field` request with `SurfaceFieldDto`; `surface_revisions` on the scene topic;
  `testing::FieldBuilder` and the reference worlds. Its asks are listed under Risks.
- **R11:** nothing consumed; it fills this plan's `BakeHook` and shader hooks.
- **Galaxy plan 14:** radius and reference surface, ocean fraction, ice fraction, `SurfaceState`
  with `SurfaceMaterial`, surface age, tectonic regime and volcanism level, `body_fixed_at`
  (P14.T14.c), and the visual geometric albedo p that R07 asks of it.
- **Galaxy plan 12:** nothing directly; R09 gates coverage on P12.T7's store.

## Design notes

1. **The material classifier is `hyperion-surface` code and runs where the height runs** (researched
   2026-09-29). The brainstorm makes the class authoritative, "from slope, altitude, the coarse
   climate field and plan 14's ice fraction, from the same inputs as the height", and R09's Design
   note 17 leaves it to this plan over `FieldView` and `HeightSample`'s gradient. It sits beside the
   synthesis, under the same determinism discipline, and goldens pin it on native and both wasm
   targets. Its order of decision is fixed: `Seabed` below R09's water surface; the coarse pass's
   ice (`SeaIce` over the sea, `WaterIce` or `VolatileFrost` on land by the condensable plan 14
   names), since glaciers and ice caps stand in cliffs of their own; `Bedrock` above `CLIFF_SLOPE`;
   `Snow`, giving way to `Bedrock` above `SNOW_SHED_SLOPE`; `Melt` where the surface state is a
   magma ocean or a young volcanic province; `Evaporite` in arid basins whose base level the coarse
   pass filled (playas, among the brightest land surfaces); `Sediment` along channels and in the
   other filled basins; then by regime and R09's `SurfaceClass`: `Regolith` on an airless world,
   `Dust` on an arid thin-aired world where R09's wind field deposits, `Sand` and `Soil` by aridity,
   `OrganicSediment` for a Titan-like haze regime, `VolcanicRock` in older provinces. Loose material
   cannot rest above its static angle of repose, so steeper ground is exposed rock: `CLIFF_SLOPE` is
   40° with a 5° transition band either side, covering the 35–45° of the literature. Kleinhans et
   al. 2011 (JGR 116, E11004) measured about 40° for angular grains at 0.1, 0.38 and 1 g, with the
   static angle rising about 5° at low gravity, while Atwood-Stone and McEwen 2013 (GRL 40, 2929)
   found Martian dune lee faces at 30–34°, as on Earth; since the two disagree on gravity, the
   threshold does not scale with it. The Lunar Sourcebook (§9.1.10) gives dumped lunar soil nearly
   40° and compacted soil about 45°. `SNOW_SHED_SLOPE`, about 50–60°, is from memory; T1.a finds a
   source or removes the rule and records the gap. Nothing is drawn from a stream, so the classifier
   needs no domain tag.
2. **Physical properties are data with sources, and bearing capacity is a function** (researched
   2026-09-29). Ultimate bearing capacity is not a constant of the ground: it grows with footing
   width and gravity, q_ult = c N_c ξ_c + ½ ρ g B N_γq ξ_γq (Lunar Sourcebook §9.1.9, after
   Durgunoglu and Mitchell 1975), about 6,000 kPa for a 1 m footing on the Moon, where the Apollo
   lunar module's pads met 3,000–11,000 kPa against an applied 5 kPa. So the table holds the
   Mohr–Coulomb pair and the bulk density, and `bearing_capacity(props, g, width)` computes it. The
   figures to settle source by source in T1.a: lunar regolith c ≈ 0.44–0.62 kPa and φ ≈ 41–43° at
   0–15 cm, bulk density about 1.5 g/cm³ (Sourcebook Table 9.12 and §9.1.4); Martian soils from
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
   and the variance of the slope in the bands finer than n, from R09's per-cell closed forms (an
   ask); for the rules on altitude, the same with the height variance. At the finest level the
   weights are the class, blended linearly across `TRANSITION_HALF_WIDTH` either side of each
   threshold for the image only, and their largest weight is always the authoritative class, ties
   broken by discriminant. A test holds the weights at every coarser level to within ten percentage
   points of the fractions counted by brute force from the finest level. This is the plan's reading
   of "the shader blends the class's textures across a transition band but never re-decides the
   class": the worker decides everything, and the shader only mixes the photometric parameters it
   was handed.
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
   of the omitted bands, σ from R09's closed forms: the expected maximum of Gaussian deviation over
   a patch's 4,225 vertices is the 1 − 1 ÷ 4,225 quantile, about 3.5σ, and 4σ leaves margin. It is
   chunked level of detail's "maximum error within the chunk" taken statistically, it saves two to
   three times the patches, and what it lets through is a sub-τ pop that morphing hides; collision,
   which reads the finest level, and the readouts, which state 1σ, are untouched. Until the ruling,
   selection uses the hard bound. A per-patch bound, each bake computing its children's bounds from
   the amplitudes over their footprints as chunked level of detail stores δ per chunk, is adopted
   only if T4.a shows body-wide and per-patch bounds differing by more than 1.5 times.
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
   `~2140 m ± 180 m` and `~2100 m ± 500 m`. When the survey reaches the band limit the uncertainty
   is zero, the value is the ground itself at the ship-wide elevation precision of 0.1 m, and both
   the `~` and the `± …` go. Unsurveyed ground has no reading: the guide's em dash with
   `NOT SURVEYED`.
7. **Slope and slant range state their basis** (researched 2026-09-29). The slope is the
   adirectional slope, the gradient magnitude from central differences over a baseline b along the
   local east and north axes; a two-point profile slope is smaller by about √2 on isotropic terrain
   (Kreslavsky and Head 2000), and the adirectional one is what tilts a lander. Each unresolved band
   of variance σ_k² at wavenumber k adds per axis v = Σ 2 σ_k² (1 − J₀(k b)) ÷ b², the structure
   function at lag b (Shepard et al. 2001), which tends to 2σ_k² ÷ b² for bands much finer than b
   and to the band's gradient variance for bands much coarser; craters and channels need R09's
   structure function at lag b, not only a variance (an ask). The slope magnitude is then Rician
   about the resolved slope |g₀|, biased upward when |g₀| is below about 2√v, so the readout quotes
   the RMS slope √(|g₀|² + 2v) with the Rice standard deviation as its uncertainty; quoting |g₀|
   alone would understate slopes from orbit, the unsafe direction. The baseline is shown with the
   value: 2 m by default, the band limit and a lander pad's scale, with 5 m and 100 m selectable,
   the ladder InSight's site selection used (lander-stability slopes at 2–5 m from HiRISE DEMs
   against a 15° limit, and 300 m scale from MOLA: Golombek et al. 2014, LPSC 45, 1499, verified).
   The slant range to the ground under the view's reticle is a march of the ray against the surface
   at the survey's resolution. Linearising the hit of p(t) = o + t d on z = h + δ gives dt ÷ dδ =
   cos θ_s ÷ sin γ_s, with θ_s the resolved surface's slope and γ_s the grazing angle to the local
   surface plane, so σ_R = σ_h cos θ_s ÷ sin γ_s. The linearisation fails once unresolved relief can
   occlude the hit, so when sin γ_s < 2√v at b equal to the survey's cell size the readout drops its
   σ and says `GRAZING`: the true range is then biased short and its error one-sided. Consoles that
   draw no terrain would need a server request for the same numbers; none exists yet, and when one
   does it wraps these functions unchanged.
8. **One photometric law for every class, and the disc and terrain share it** (researched
   2026-09-29). Every class takes McEwen's lunar-Lambert law, r(i, e, α) = A_N f(α) [2 L(α) μ₀ ÷
   (μ₀ + μ) + (1 − L(α)) μ₀], with f(0) = 1, μ₀ = cos i, μ = cos e and A_N the normal albedo; L = 1
   is Lommel–Seeliger and L = 0 Lambert. For the Moon L(α) = 1 − 0.019 α + 2.42 × 10⁻⁴ α² − 1.46 ×
   10⁻⁶ α³ in degrees (McEwen 1991, Icarus 92, 298; the form and coefficients verified in USGS
   ISIS's `LunarLambertMcEwen.cpp`). Dark surfaces act like Lommel–Seeliger and bright ones like
   Lambert (Buratti and Veverka 1983), so L(0) is a function of A_N, not a free number per class.
   Hapke's full model is not used per texel: its roughness term counts relief the mesh, normals and
   horizon map already resolve, and what it adds lies below the band limit. This is one of R07's
   candidate families (its Design note 5), so `body_brdf` carries it and this plan passes per-class
   parameters. At zero phase a uniform body's geometric albedo is p = A_N [L(0) + ⅔ (1 − L(0))], and
   averaged over viewing directions a patterned sphere's p is the plain area average of each
   element's, so the scale c on the class albedos that makes the body's p is the root of Σ a_i c
   A_N,i [L_i + ⅔ (1 − L_i)] = p over the classes' area fractions a_i. Clamping to the classes'
   ranges makes it piecewise linear in c, solved exactly between sorted breakpoints. The server
   holds the whole field, so `albedo_scale` runs there, on every cell, and c ships with the field
   (an ask of R09): the terrain's brightness then never shifts as the survey grows, and one scalar
   reveals nothing the disc's brightness does not. Matching p at zero phase is not enough for a
   hand-over that changes geometry only, since the laws differ at other phases and the pattern
   differs everywhere (an Iapetus, a Mars albedo map). So R07's disc samples a per-body coarse
   class-weights map, T2's weights at level 4 or shallower, a few hundred kilobytes, through the
   same law, and disc and terrain agree at every phase by construction. The map covers surveyed
   cells only; unsurveyed texels take R07's uniform `lawFor(p, q)`, so the disc shows no pattern the
   ship has not seen. Plan 14's phase integral q then becomes a result of the classes, reported with
   its residual as the albedo's is. Starting parameters per class, all to be settled in T1.a (most
   from memory in the research): regolith A_N 0.07–0.20 with the lunar L(α); basaltic rock
   0.05–0.15, felsic 0.2–0.35; dry soil and sediment 0.10–0.30, halved to 0.6 when wet (Lekner and
   Dorf 1988); desert sand 0.25–0.45, Martian dune sand 0.10–0.15; fresh snow 0.80–0.95, old 0.5–0.7
   (Warren 1982); glacier ice 0.3–0.6 (Cuffey and Paterson 2010); sea ice 0.5–0.7 bare, 0.80–0.87
   snow-covered (Perovich et al. 2002); nitrogen and methane frost 0.8–1.0 (Buratti et al. 2017);
   carbon dioxide frost 0.4–0.8; organic sediment 0.05–0.2; evaporite 0.5–0.8. `Melt` is emissive,
   which is R07's or R11's, and `Seabed` lies under the ocean pass.
9. **No textures in this plan.** The Materials section lists triplanar projection of rock and detail
   textures and two-scale repetition hiding. Anything a texture adds below the band limit is detail
   the simulation did not compute, which the brainstorm defines as decoration, and R11 owns it,
   zero-mean about this plan's class albedo, through the `terrain_decoration` hook of
   `terrainLit.wgsl`. This plan's lit terrain is shaded from baked data alone: the height, the
   normals at twice the mesh resolution on the high setting (R05's `NormalScale`), the class weights
   and their photometric parameters. It is the undecorated ground R11's decoration switch turns off
   to.
10. **Horizon maps are sun-independent, half-float, and their far field is a shared lattice**
    (researched 2026-09-29). A horizon map stores, per texel and azimuth, the horizon's elevation
    (Max 1988; Sloan and Cohen 2000), and the test compares the sun's elevation in that azimuth
    against it; nothing depends on the light, so a patch never rebakes for the sun or the body's
    rotation (the brainstorm's "needs rebaking only as the sun moves" is reported in the notes).
    Eight azimuths, interpolated linearly in azimuth; sixteen would double the memory, and the
    by-hand sunset decides whether narrow distant peaks need them. The encoding is sin(e) in
    `rgba16float`, two layers: eight bits over [−1, 1] step 0.45° in elevation, while the Sun's
    whole penumbra at 1 au is 0.53°, so sunsets would band; half-float steps under 0.02° everywhere,
    is signed for horizons below zero on peaks, and is filterable in core WebGPU. The rule stated
    with it is a step at most half the smallest stellar angular radius drawn with a soft penumbra.
    The near field is marched from the patch's own 65 × 65 heights, about 2.2 M compares, an
    estimated 2–6 ms in WebAssembly (provisional; T8.a measures it), or by Timonen and Westerholm
    2010's convex-hull line sweep, O(n) per direction, if the march breaks the budget. The far field
    is not rays through the height function, which at 81 points × 8 azimuths × about 45 steps is
    some 29,000 evaluations, an estimated 60–120 ms: each far sample is the bilinear interpolation
    of the level-m vertex heights, m chosen so the vertex spacing is about the step length times the
    growth factor less one, memoised by (face, level, i, j) in the worker's `SynthCache`. The
    far-field points lie on a face-wide lattice at a patch edge's spacing ÷ 8, defined by position
    alone and not by the patch's level, so points on a shared edge are the same point with the same
    value and no seam can open. Steps grow by 1.15–1.25 from one texel beyond the patch edge, the
    curvature drop d² ÷ 2R is included, so an obstacle at d stands at arctan((h − h₀ − d² ÷ 2R) ÷
    d), and the march stops at √(2 R Δh), Δh the body's realised relief range: about 505 km for an
    Earth with 20 km of relief. The shader takes the larger of near and far. The lit fraction is the
    share of the star's disc of angular radius ρ above the horizon, 1 − (arccos x − x √(1 − x²)) ÷ π
    with x = (e_sun − e_horizon) ÷ ρ clamped to [−1, 1] (checked: 0, ½ and 1 at x = −1, 0, 1),
    without limb darkening, which moves the half-light point by under 0.1ρ. On terrain R07's horizon
    term (`sphere_irradiance`, its Design note 6) takes its local horizon per disc sample from this
    map (an ask of R07), with the closed form as its test oracle. Where R08 draws the sun at its
    refracted elevation (on an Earth about 0.57° at the horizon, more than the Sun's disc, and 0.16°
    at 5°; Bennett 1982), the shadow test uses the same apparent elevation, or shadows and the drawn
    sunset disagree by more than a solar diameter. Max, Sloan and Cohen, Stewart 1998, Timonen and
    Westerholm and Bennett are from memory in the research; T8.a's bench and the by-hand sunset
    settle what they support.
11. **Terrain shadows are this plan's on both settings, and combine by minimum** (researched
    2026-09-29). The brainstorm's ladder gives the high setting cascaded shadows "with cloud shadows
    on terrain" and the low one the horizon map, and its budget's third rule builds the low setting
    beside the high. No plan in the set names the cascades, and terrain self-shadowing is this
    plan's feature, so the reading closest to step 8 is that both are built here; R11's cloud
    shadows multiply into the result through `terrain_shadow_factor`. The horizon map is applied
    everywhere on both settings; on the high one the lit fraction is min(horizon, cascade) where a
    cascade exists, so cascades add what the horizon map cannot, casters that are not terrain
    (craft, R11's rocks and scatter) and sub-texel detail near the camera, and there is no seam at
    the last cascade, which only fades to lit over its last 10%. Cascades switch off when cascade
    1's texel is coarser than the horizon map's, as in orbit. Splits follow Zhang et al. 2006's
    practical scheme, Cᵢ = λ n (f ÷ n)^(i ÷ N) + (1 − λ)(n + (f − n) i ÷ N) with N = 4 and λ ≈ 0.75,
    over a shadowed range f = min(view far, max(2 km, 20 × height above ground)), continuous in that
    height. Each cascade is fitted to a bounding sphere of its frustum slice, whose radius does not
    change as the camera turns, snapped to whole texels (Valient 2008), with every light matrix
    built camera-relative in `f64` and narrowed to `f32`, since snapping at planetary coordinates
    would jitter. An orthographic light projection has linear depth, so reversed-Z buys nothing
    there: the maps are `depth32float`, whose worst step over a 50 km range is about 3 mm, and keep
    the project's greater-equal convention for uniformity only, which the code says so that no one
    tunes bias expecting precision. Casters in front of the near plane are pancaked through
    `primitive.unclippedDepth` where the adapter offers `depth-clip-control` (to be confirmed on the
    UHD 620 through R01's feature report), and otherwise the near plane is pulled towards the sun by
    Δh ÷ sin(e_sun), capped. Acne is held by slope-scaled bias (`depthBias`, `depthBiasSlopeScale`,
    `depthBiasClamp`) and a normal offset of 1–1.5 texels of the cascade's world-space size;
    filtering is 2 × 2 hardware PCF through `textureSampleCompare` with a few Poisson taps, since
    the Sun's penumbra at 10 m from an occluder is about 9 cm and PCSS is unneeded. Four 2,048²
    `depth32float` layers are 64 MiB, the brainstorm's figure. Zhang et al.'s formula, Valient's
    snapping and the bias recipes are from memory in the research and are checked against the
    sources in T9.a.
12. **Unsurveyed ground is the reference surface, and its edge is marked.** A patch's vertices whose
    coarse cell is not surveyed (R09's margin cells are held but not surveyed) carry a survey mask
    of zero, and the terrain shader discards fragments whose interpolated mask is below one half.
    Beneath them R07's `mesh`-regime body at plan 14's reference radius is drawn with its own
    fragments discarded where the GPU's coverage mask says surveyed, so no terrain is claimed there
    and no reference sphere shows through a surveyed valley. Where terrain and reference surface
    meet at different heights, a skirt along the survey edge joins them, drawn in the reference
    surface's appearance, so the edge reads as an edge rather than as a cliff in the ground. The
    edge itself is symbology: a mark over the image, cased under guide item 3, in both styles. The
    coverage mask is the brainstorm's "coarse field, GPU copy", one byte per coarse cell per face,
    at most 384 KiB at level 8.
13. **The disc hands over to terrain by the body's own relief.** R07 draws a body in its `disc`
    regime until R09's `realised_relief` subtends τ pixels; nearer, the `mesh` regime hands its
    geometry to R05's quadtree, from the root faces down. At an Earth's ±10 km at 1080p and 1 px
    that is about 2 × 10⁷ m, the range the brainstorm gives for the coarse field resolving the
    ground in the wireframe. The resolved slopes and shadows at that level are negligible, which is
    why the same smooth law holds on both sides of it (Design note 8). A body with no survey never
    hands over.
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
    colour alone may not carry meaning: minor 1 px and index 2 px in `--text-muted`, coastline 2 px
    in `--text`; never dashed, since the guide reserves dashes for predictions; no contour labels in
    the image, since the cursor's `ELEVATION` and the stated datum carry the values. The line is
    antialiased at constant width from u = (h_local + φ) ÷ CI: dist_px = |fract(u − 0.5) − 0.5| ÷
    length((dpdxFine(u), dpdyFine(u))), coverage = clamp(w ÷ 2 + 0.5 − dist_px, 0, 1), the
    gradient's length rather than `fwidth`, whose L1 norm varies widths by up to √2 with direction
    (after Golus 2021). The derivatives are taken in uniform control flow, before the survey mask's
    `discard`. Where a family's spacing falls below about 4 px it fades out over 3–6 px, minor
    contours first, and the guide draft says so, so that an absent line is not read as flat ground;
    this is the grazing case, where spacing collapses faster than any interval can follow. The
    height varying is patch-local, h − h_patch, with a per-patch phase φ = h_patch mod CI formed in
    `f64` on the CPU, because the `f32` height above the reference radius steps 2 mm at ±20 km and
    would quantise the derivative near the camera. The label block states
    `CONTOUR INTERVAL 200 m · INDEX 1 km · DATUM …`, the USGS margin form and guide item 5's
    substitute for a scale bar, scaling to km with hysteresis. Unsurveyed ground has no contours;
    R02's graticule is drawn over the terrain's height where terrain is surveyed. The datum is plan
    14's reference sphere, which is what every height in the patch is measured from (R05's Design
    note 5), stated as `DATUM REFERENCE SPHERE 6371.0 km`; the sea level is stated beside it on an
    ocean world. The h ÷ 20 figure has no published precedent for perspective views and is settled
    by T11's check on steep and gentle terrain; the index and margin conventions are from memory in
    the research.
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
    R05's heights and parent heights, 33.8 kB (65 × 65 × `rg32float`, read with `textureLoad` and
    bound as `unfilterable-float`, so `float32-filterable` is not needed); normals, 16.9 kB at mesh
    resolution or 66.6 kB at twice it (`rg16float` octahedral, core and filterable, angular error
    about 0.05°); class weights, 33.8 kB (two `rgba8unorm`); the horizon map, 67.6 kB (Design note
    10); and the survey mask, 4.2 kB (`r8unorm`): about 156 kB on the low setting and 206 kB on the
    high. R05's `BakedOffsets` vertex path adds about 101 kB a patch while it stays R05's default.
    The sizing rule is max(1.3 × the all-round peak, 3 × the frustum peak), summed over the views
    that stream, plus the forced region. On the low setting that is about 430 slots, which 64 MiB
    holds at 156 kB with nothing to spare, and not with `BakedOffsets`; on the high setting 3 × the
    frustum peak is about 800 slots, 165 MB, which 256 MiB holds, while 128 MB does not, and the
    all-round set at 100 m would need about 390 MB. The brainstorm's 128–256 MB therefore becomes
    256 MB at least on the high setting, and T14 measures which rule the resident peaks need.
    `writeTexture` has no 256-byte row rule (only encoder copies do), so 520-byte rows upload as
    they are, but tiled layouts on Intel may pad a 65 × 65 texture to 1.8–2.2 times its size, so T14
    measures driver-reported memory. Hence the layout: the cache is preallocated as fixed slots from
    the byte budget; heights, parent heights, the horizon map and the mask, all read by
    `textureLoad`, live in storage buffers addressed by slot index, tight and untiled (the default
    `maxStorageBufferBindingSize` is 128 MiB); normals and class weights, which want filtering, live
    in 2D arrays (at most the adapter's `maxTextureArrayLayers`, 256 by default) or atlases with a
    one-texel gutter; and each style's terrain is one instanced draw over R05's shared 65 × 65 index
    buffer with a slot index per instance, which also keeps the CPU's per-draw cost down. R05's
    octahedral `snorm16` normals need `texture-formats-tier1` for `rg16snorm`, so this plan asks R05
    to convert them to `rg16float` in the worker, or to require the feature with that fallback. The
    slot layout changes R05's `PatchCache` from bytes-bounded LRU to slots; the eviction rule stays
    R05's.

16. **The client fetches what the camera can resolve, and nothing more.** A body's field is wanted
    once it is the camera's local body or within the hand-over range of Design note 13 of any view's
    camera, and released when every view has left twice that range. The client sends R09's
    `surface_field` with `have_revision`, and a rise in the body's revision in `surface_revisions`
    on the scene topic sends it again for the new cells only. Chunks are decoded in the workers,
    each holding a `PartialField` per body: a worker receives the header at `field_init` and each
    chunk after it. Cells arrive exact and whole with their margins, so a new chunk never changes a
    baked height, and only patches whose survey mask it changes are rebaked. Patches waiting on a
    chunk of a surveyed region count toward R05's `TERRAIN: STREAMING`; unsurveyed ground does not,
    because it is not coming.

## Tasks

T1–T5 are `hyperion-surface` code and run in order after R09's synthesis lands; T6 moves R05's bake
onto it. T7 (the client's field store) needs only R09's chunks and R05's workers and can run beside
T1–T5. T8 (horizon maps) needs T6. T9 (cascades) is independent of T8 but lands after it, so the
high setting has its horizon map to combine with. T10 (lit terrain) needs T6 and T7. T11 (the
wireframe) needs T6 and T7. T12 (readouts) needs T5 and T7. T13 (guide drafts) can be written at any
time and lands before T11 and T12, which are built to the drafts. T14 (cache sizes) follows T10 and
T11. T15 closes. There is no interface-reconciliation task: this plan is re-validated against R05
and R09 as built when its turn comes.

Rust files are under `crates/hyperion-surface/src/` and client files under
`apps/hyperion/src/renderer/src/view/terrain/` unless a path says otherwise. Every GPU check is by
hand and recorded in the task's plan entry with the machine, setting and date, as the brainstorm's
Testing section requires; no golden images. Every timing, bench or by hand, is taken on a quiet
machine, with no other test suite or lane running, and recorded with the load average; a figure
taken under shared load is marked provisional and re-measured quiet before any decision rests on it.
The timings this plan quotes from its research (2–6 ms for the near-field horizon march, 60–120 ms
for ray-marched far fields, 3–6.5 ms for the station wireframe) are reasoned estimates, not
measurements, and are provisional until the tasks that own them measure them.

### R10.T1 Material classes

- **R10.T1.a The class set and its properties.** `MaterialClass` (with `Evaporite` and `Dust`),
  `FrostSpecies`, `MohrCoulomb`, `FrictionModel`, `MaterialProperties`, `properties`,
  `limb_parameter`, `bearing_capacity`, `interface_friction` (Design notes 2 and 8). Each figure is
  re-checked in its primary (the Lunar Sourcebook's Table 9.12 page, USGS PP 1389 Table 7, NAVFAC
  DM-7.02 Table 1, Kietzig et al. 2010's Fig. 2, the albedo sources of Design note 8) and cited in
  the doc comment; a figure without a source leaves its field `None`. `CLIFF_SLOPE` 40° and
  `TRANSITION_HALF_WIDTH` 5° with Kleinhans et al. 2011 and Atwood-Stone and McEwen 2013;
  `SNOW_SHED_SLOPE` with a source or removed, recorded either way. Files:
  `material/{mod,class,properties,bearing}.rs`. Tests: discriminants are stable (a table test on the
  `u8`s); every property is finite and in its physical range; every row with a value names its
  source; `bearing_capacity` for a 1 m footing on lunar regolith at the Moon's gravity from Table
  9.12's pair lies within the Sourcebook's 3,000–11,000 kPa and near its 6,000 kPa; it grows with
  width and gravity; `interface_friction` for ice interpolates between its points and is `None`
  outside them. Acceptance: `cargo test -p hyperion-surface material::properties`.
- **R10.T1.b The classifier.** `material_at` with the decision order of Design note 1, over R09's
  `Synthesiser` and the finest level's gradient. Files: `material/classify.rs`,
  `tests/material_golden.rs` with its `golden/material_*.json` (a thousand points on each of
  `sample_worlds()`). Tests: a `cliff_field` is `Bedrock` above 40° and not below; a warm pole has
  no ice cap and a locked world's ice lies where R09's field puts it (the brainstorm's two examples
  under Materials); `Seabed` exactly below the water surface; an arid filled basin is `Evaporite`;
  the order of rules is total; the goldens match on native, `wasm32-wasip1` and
  `wasm32-unknown-unknown` through R04's checks. Bumps `GENERATOR_VERSION` (see Generator version).
  Acceptance: `cargo test -p hyperion-surface material`, and `just ci` runs the goldens on both wasm
  targets.

### R10.T2 Class weights for coarser levels

`ClassWeights`, `weights_at` (Design note 3), with the slope-exceedance probability from R09's
per-cell gradient variance, and the palette of up to eight classes a patch keeps. Files:
`material/weights.rs`. Tests: at the finest level the largest weight is `material_at`'s class at
every sampled point, ties included; weights sum to 255; on `sample_worlds()` the weights at levels n
− 1 to n − 6 from the finest agree with brute-force class fractions over each texel's footprint
(sixty-four finest points a texel) to within ten percentage points in the mean over a thousand
texels a level (slow); a patch never needs more than eight classes on `sample_worlds()`, and if one
does the test names it. Acceptance: `cargo test -p hyperion-surface material::weights`, and the slow
case under `just test-slow`.

### R10.T3 The authoritative ground on real terrain

R05's `finest_surface_height`, moved from the test planet to R09's `Synthesiser` and wrapped as
`ground_at` with `GroundPoint` and `FinestTriangle` (Design note 4), on R05's diagonal and
`xyz_to_face_uv`. Files: `ground.rs`, R05's `patch` module, `tests/ground_golden.rs` with
`golden/ground_*.json`. Tests: at a vertex `ground_at` equals R09's `height_at` there bit for bit;
on an analytic plane field it is exact; points on a shared edge get the same height from either
triangle; points either side of a cube-face edge and at a cube corner agree to 1 µm; the normal is
the triangle's; the chord sagitta of Design note 4 (L² ÷ 8R at 0.5 m on 6,371 km, about 5 nm) keeps
face-coordinate and spatial interpolants within 1 µm; `ground_at` is finite for every point of
`sample_worlds()` and asserts finiteness before it returns (the brainstorm's NaN hazard). The
goldens are asserted on native and both wasm targets. No bump: the ground reads heights that R09's
goldens already pin. Acceptance: `cargo test -p hyperion-surface ground`.

### R10.T4 The level bound on generated worlds

- **R10.T4.a The bound measured.** R09's `level_bound_m` on `sample_worlds()`, the real coarse
  pass's output rather than R09's synthetic fields, with σ per level from R09's closed forms (Design
  note 5). Files: `tests/level_bound_worlds.rs`. Tests: for ten thousand positions a world and every
  pair (n, n + k), |h_n − h_(n+k)| stays under `level_bound_m(n)` (slow). Recorded per world and
  level in this task's plan entry: the observed 99.9th percentile over the bound (expected 0.3–0.5),
  the per-patch maximum over 4,225 vertices over the bound, and the spread between the body-wide and
  a per-patch bound; and on R05's scripted descent, the patches selected at τ = 1 px under the hard
  bound and under min(hard, 4σ), so the owner rules with numbers. Acceptance:
  `just test-slow -E 'test(level_bound_worlds)'` passes, and the figures are recorded.
- **R10.T4.b Selection by the real bound.** The worker answers `field_init` with the body's
  `LevelBounds` (hard and σ per level); `levelBounds.ts` holds them per body and `selectionBound`
  gives R05's `screenSpaceErrorPx` its ε_n under the rule in force (the hard bound until the owner's
  ruling), for each style's τ: 1 px high and 2 px low (the brainstorm's lean; CesiumJS's default
  `maximumScreenSpaceError` is 2 px, though its error is a spacing heuristic), and 4 px wireframe.
  Files: `levelBounds.ts`, R05's `select.ts`. Tests (Vitest, pure): a level is selected exactly
  where its bound subtends at most τ, under either rule, taken as a parameter; R05's
  history-independence test passes with real bounds; the grounded-body rule still yields the finest
  level. Acceptance: `pnpm --filter hyperion test view/terrain`.

### R10.T5 Surveyed readings

`SurveyedReading`, `surveyed_reading`, `SlantRange`, `slant_range`, `SLOPE_BASELINES` (Design notes
6 and 7), over R09's `unresolved_rms`, its structure function at lag b (an ask) and the coverage
resolution per cell, with J₀ through the base crate's `math`. Files: `reading.rs`. Tests: at a
band-limit survey the elevation is `ground_at`'s, the uncertainty zero and `resolved` set; at an
orbital survey, pooled over about 200 cells × 100 points a world on `sample_worlds()`, the RMS of
`ground_at` less the quoted value is within 10% of the pooled predicted σ, and the per-cell z-scores
have unit variance within 15% (slow); the slope uncertainty tends to the band's gradient σ for b
much larger than the finest wavelength and to √2 σ_h ÷ b for b much smaller; the RMS slope is never
below |g₀|; an unsurveyed cell gives `None`; the slant range to a plane is exact and its σ follows
cos θ_s ÷ sin γ_s; a ray under the `GRAZING` threshold has no σ; a ray that leaves the surveyed
region before it hits gives `None`. Acceptance: `cargo test -p hyperion-surface reading`, and the
pooled case under `just test-slow`.

### R10.T6 The bake on real terrain

R05's `bake_patch` moved onto R09's `Synthesiser::bake_patch` (heights, parent band and gradients)
and `SynthCache`, and extended with `TerrainLayers`: the class palette and weights (T2), the survey
mask from the `PartialField`'s `is_surveyed`, the patch's hard bound and σ, the horizon map once T8
lands, and R11's `BakeHook`; normals at R05's `NormalScale`, delivered as `rg16float` (Design note
15, an ask of R05). The worker gains `field_init`, `field_chunk` and `field_release`, holding one
`PartialField` per body, and R05's provisional height function leaves the client path (R09's Design
note 17). Files: `bake/{mod,layers}.rs`, R05's `patch` module and binding, R05's
`view/terrain/workers/*.ts`. Tests: the bake is the same bytes whatever order a patch and its
neighbours are baked in (`assert_order_independent`); heights equal R09's `height_at` narrowed to
`f32`; the survey mask is 0 exactly on vertices of unsurveyed or margin cells; a `field_chunk` that
adds cells changes no baked height of a patch already baked (Design note 16); in the worker test,
init, two chunks and a release leave no field held. Bench `bake/patch_65`: the brainstorm's 40 ms a
patch on two cores of the development machine (about 10 µs a point with its gradient, Performance
budget), recorded with and without the class weights. Acceptance:
`cargo test -p hyperion-surface bake`, `pnpm --filter hyperion test view/terrain/workers`, and the
bench's figure in this task's entry.

### R10.T7 The client's coarse-field store

`CoarseFieldStore` and `CoverageView` (Design note 16): wanting and releasing bodies from the views'
cameras with hysteresis, R09's `surface_field` with `have_revision`, re-requests on
`surface_revisions`, forwarding chunks to every worker, the coverage and its resolution per cell,
the GPU coverage mask per face (Design note 12), and the link's loss keeping what was received and
marking the coverage stale. Files: `coarseFieldStore.ts`, `coverage.ts`. Tests (Vitest with the fake
socket): a body within range is requested once and its chunks reach every worker in arrival order; a
revision rise fetches only new cells; leaving and returning inside the hysteresis band fetches
nothing; every worker gets `field_init` once and every chunk; releasing a body frees it in every
worker; the resident total of worker copies is reported, for T14. Acceptance:
`pnpm --filter hyperion test view/terrain`.

### R10.T8 Horizon-map shadows

- **R10.T8.a The bake.** `HorizonMap`, `far_field_point` and the bake of Design note 10 in
  `bake/horizon.rs`, called by T6's bake on both settings. Tests: a `flat_field` on a sphere has a
  horizon at the curvature drop's angle and never above zero; a `cliff_field` of height H at
  distance d gives arctan((H − d² ÷ 2R) ÷ d) within one quantisation step; no quantisation step
  exceeds half the Sun's angular radius at 1 au (959.6″ ÷ 2); azimuth interpolation is continuous;
  the far field is bit-identical at shared lattice points of adjacent patches and across levels; the
  far field is independent of the cache's state, with bakes in any order; the march stops at √(2 R
  Δh). Bench `bake/horizon`: at most a quarter of the patch's 40 ms, recorded; over it, the near
  field moves to the line sweep. Presentation-only: no golden across targets, though the bake is
  deterministic. Acceptance: `cargo test -p hyperion-surface bake::horizon` and the bench's figure
  recorded.
- **R10.T8.b The shadow lookup.** `shadows/horizon.wgsl`: the sun's apparent direction in each
  patch's local frame from `body_fixed_at`, R07's star and R08's refraction where drawn; the compare
  against the interpolated horizon as the per-sample local horizon of R07's horizon term
  (`sphere_irradiance`); the soft lit fraction of Design note 10. Tests: the lit-fraction formula as
  a pure TypeScript twin against hand values (0 and 1 beyond ±ρ, one half at e_sun = e_horizon);
  R01's smoke harness compiles and runs the pass on SwiftShader with and without f16, every texel
  finite. By hand, recorded: a sunset over a generated range on the UHD 620 low setting, with the
  pass's GPU time against the budget's 0.3–0.5 ms (Performance budget, Shadows row), and whether
  eight azimuths show missing peaks. Acceptance: `pnpm --filter hyperion test view/terrain/shadows`
  and the smoke run.

### R10.T9 Cascaded terrain shadows on the high setting

- **R10.T9.a Cascades.** `shadows/cascades.ts` (Design note 11): four 2,048² `depth32float` layers
  (the brainstorm's 64 MB), PSSM splits with λ ≈ 0.75 over the shadowed range f(h), bounding-sphere
  fitting, whole-texel snapping, light matrices camera-relative in `f64`, `unclippedDepth` where
  `depth-clip-control` is offered and the pulled-back near plane otherwise, slope-scaled bias and
  normal offset; the sources of Design note 11 re-checked. Tests (pure): the splits are monotonic
  and continuous in the height above ground; snapping moves a cascade by whole texels only; the
  fitted radius does not change as the camera turns; cascades are off where cascade 1's texel is
  coarser than the horizon map's. Acceptance: `pnpm --filter hyperion test view/terrain/shadows`.
- **R10.T9.b The high setting's shadow term.** The horizon map everywhere, cascades by `min`, each
  cascade fading to lit over its last 10%, 2 × 2 PCF with Poisson taps, exposed to R11 through
  `terrain_shadow_factor`. By hand, recorded: the discrete target at 1080p against the budget's
  1.5–3 ms, and no double-dark band at a cascade's edge in a sunset scene; R01's feature report on
  whether the UHD 620 offers `depth-clip-control`. Smoke run on SwiftShader. Acceptance: the smoke
  run passes and the figures are recorded.

### R10.T10 The lit style on generated terrain

- **R10.T10.a Class photometry.** `albedo_scale` in `material/albedo.rs` (Design note 8): the exact
  piecewise-linear solve of Σ a_i c A_N,i [L_i + ⅔ (1 − L_i)] = p over the whole field, with the
  residual; called on the server beside R09's field build so that c ships with the field (an ask of
  R09). Tests: on each of `sample_worlds()` the area-weighted p of the scaled classes equals the
  body's p to 1% where the ranges allow, and the residual is stated where they do not; the clamped
  solve equals a bisection reference; the solve depends on the class fractions only. Acceptance:
  `cargo test -p hyperion-surface material::albedo`.
- **R10.T10.b The terrain shader and the class map.** `terrainMaterial.ts` and
  `shaders/terrainLit.wgsl`: the palette and weights to `body_brdf`'s lunar-Lambert parameters (A_N
  × c, L(α) from A_N, f(α)), the normals at the setting's resolution, the survey mask's discard and
  the skirts at the survey edge, the reference surface drawn with the inverse mask (Design note 12),
  the shadow term from T8 or T9, the hooks `terrain_decoration` and `terrain_shadow_factor` for R11,
  and R08's aerial perspective on the output; `classMap.ts`, the per-body coarse class-weights map
  over surveyed cells that R07's disc samples (an ask of R07). Tests: a pure test that the
  parameters a texel gets are the weighted sum of its palette's, and that the largest weight's class
  is the one a readout names; the class map holds no texel for an unsurveyed cell; the smoke harness
  renders a generated world's patch set on SwiftShader with every texel finite. Acceptance:
  `pnpm --filter hyperion test view/terrain` and the smoke run.
- **R10.T10.c The disc hands over.** R07's `disc` regime gives way to the quadtree at the range of
  Design note 13, per view. Tests: the handover range as a pure function (about 2 × 10⁷ m for ±10 km
  at 1080p and 1 px, the brainstorm's figure under Two styles), and a body with no survey never
  hands over. By hand, recorded: at phase angles of about 30° and 90°, the body's mean brightness
  across the handover changes by less than 1/3 stop on a fully surveyed generated world; the
  residuals of T10.a against plan 14's p and q. Acceptance:
  `pnpm --filter hyperion test view/terrain` and the recorded check.

### R10.T11 The wireframe's terrain

Depth-only terrain in the wireframe style at τ = 4 px from the same bounds and bakes (heights and
mask only), as one instanced draw, with the contours of Design note 14 evaluated in the same pass,
the interval, index and datum stated in the label block through R02's label API, analytic line
antialiasing and no MSAA, and the patches under every grounded body in view at the finest level with
morph held at zero, as in the lit style (Performance budget, the second rule). Unsurveyed ground has
no contours, and the survey edge is drawn. Files: `terrainWire.ts`, `shaders/terrainWire.wgsl`.
Tests (pure): `contourInterval` walks the 1-2-5 sequence from the nadir's height above ground with
±20% hysteresis and never falls below 1 m; `contourPhase` with the patch-local height equals the
`f64` contour levels to 1 mm at ±20 km; the fade thresholds sit at 4 px and 3–6 px; the wireframe's
selection is within a factor of two of `patchCountModel`'s ratio to the lit style, about a fifth;
the grounded-body rule holds in this style. Smoke run on SwiftShader. By hand, recorded: a station
wireframe at 1080p on the UHD 620 during the low fast pass of the scripted descent with the height
workers busy, by `timestamp-query` with developer features, p50, p95 and p99 GPU time per pass and
the GPU clock from `intel_gpu_top`, with the patch and triangle counts, against the brainstorm's 60
fps and its 4–9 ms for the whole view (Two deployments, one scene; the research's provisional
estimate is 3–6.5 ms with analytic antialiasing); and legibility on steep and gentle terrain.
Acceptance: `pnpm --filter hyperion test view/terrain`, the smoke run, and the figures recorded.

### R10.T12 Coverage and terrain readouts on the view

`SurveyLine` in the view's label block: the survey's coverage of the body and its best resolution,
and the resolution beneath the view's centre (the brainstorm's "states the survey's resolution
beneath its centre", after the star chart's census line), stale with its `S` when the link is lost.
`TerrainReadout`: `ELEVATION`, `SLOPE` with its baseline part and `SLANT RANGE` from
`surveyed_reading` and `slant_range`, run in a worker at the guide's 4 Hz, in `output` elements on
their `--surface-0` plates, with `~` and `± …` until resolved, rounded by Design note 6's rule, the
em dash with `NOT SURVEYED` over unsurveyed ground, `GRAZING` where it applies, and the material
class's name with its properties as a reading; the datum stated once. The DOM list gains the survey
edge as an item when it is in view. Files: `readouts.ts`, `SurveyLine.tsx`, `TerrainReadout.tsx` and
their tests. Tests (Vitest and Testing Library): the formats of T13's drafts for each state,
`~2140 m ± 180 m` among them; the PDG rounding at its breakpoints (354, 355, 949, 950); no value is
shown finer than its uncertainty; the field keeps its fixed width; stale on link loss; keyboard
reach of the readout's target through the DOM list; the console-ux skill's lint, contrast and glyph
checks pass, with no `σ` anywhere (B612 has none). Acceptance:
`pnpm --filter hyperion test view/terrain` and the console-ux checks.

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
of elevation, altitude and height; the survey line's wording; the survey edge as a mark in both
styles; the contour widths and tokens, `CONTOUR INTERVAL`, `INDEX`, and the statement that minor
contours are omitted where closer than 4 px, as guide item 5's substitute for a scale bar. The
client is built to the drafts, as the galaxy plans' slice was. Files: a draft section in this task's
entry, not the guide. Acceptance: the drafts are written and **the owner signs off**, after which
the guide edit is made as the owner directs.

### R10.T14 Cache sizes

`patchCountModel` as a pure function of camera, τ and viewport (Design note 15, after the research
model's rule). Measure the all-round and frustum peaks per view on R05's scripted descent over a
generated world, on both settings and in both styles, with a second view open; set
`PATCH_CACHE_BYTES` per setting from max(1.3 × all-round peak, 3 × frustum peak) summed over the
streaming views plus the forced region, at `bytesPerPatch`, as fixed slots in the layout of Design
note 15 (storage buffers for `textureLoad` data, arrays or atlases for filtered data), recording
whether R05 chose `BakedOffsets`, and whether 64 MiB and 256 MB hold, or the finding; and record the
resident total (the cache, the coverage mask, the class map, the cascades, each worker's field copy)
against the ceilings of 1 GB low and 2–3 GB discrete, from driver-reported GPU memory as well as
computed bytes. Files: `cacheBudget.ts`, R05's `cache.ts`. Tests (pure): `bytesPerPatch` matches
Design note 15's formats for each setting and vertex path; `patchCountModel` reproduces the table of
Design note 15 within 10%; the cache never evicts a patch under a grounded body at the chosen size
during a replay of the descent's selections. By hand, recorded: the peaks and totals on both GPUs
where available. Acceptance: `pnpm --filter hyperion test view/terrain` and the figures recorded.

### R10.T15 Verification pass

The collision-agrees test end to end: a TypeScript test that forms the finest patch's vertices with
R05's vertex formation in `Math.fround`, from a bake of a real patch committed as a fixture, and
interpolates on R05's diagonal, agrees with `ground_at` from the same fixture to 2 mm plus the
formation's stated error (the height texture's `f32` step at ±20 km is 2⁻⁹ m, 1.95 mm: the
brainstorm's "about 2 mm", under The geometry); R05's descent run on a generated world at both
settings with its metrics harness, recording patches a second sustained against the brainstorm's
demand of (200 · v + 290 · |ḣ|) ÷ h, the level bound per level from T4.a, and pops or cracks seen;
every golden of this plan asserted on the three targets. Acceptance: `just ci`, `just test-slow` and
`just bench` complete, and the descent's figures are recorded.

## Verification

- **Collision agrees with what is drawn:** the finest mesh and `ground_at` to 2 mm (T3, T15).
- **Level-of-detail consistency:** every coarser level inside R09's bound on generated worlds, both
  ratios recorded (T4.a); selection by that bound, history-independent (T4.b).
- **The ground drawn and the ground quoted agree:** the largest weight is the authoritative class at
  the finest level; coarse weights within ten points of brute force (T2); the readout names the
  class the shader draws most of (T10.b).
- **Photometry:** the scaled classes reproduce the body's p (T10.a); the handover changes brightness
  by under 1/3 stop at two phases (T10.c).
- **Knowledge:** unsurveyed ground has no terrain, no contours, no class-map texel and no reading; a
  readout's uncertainty matches the pooled RMS of the unsurveyed bands (T5, T12); the image is the
  same at every survey resolution that covers it.
- **Determinism:** material and ground goldens on native and both wasm targets (T1.b, T3); bakes
  order-independent, the horizon far field bit-identical at shared points (T6, T8.a).
- **Benches:** `bake/patch_65` (40 ms), `bake/horizon` (a quarter of it), taken on a quiet machine
  and recorded with the load average; a run under shared load is provisional and repeated.
- **By hand, recorded:** the horizon-map pass on the UHD 620 (0.3–0.5 ms), the cascades on the
  discrete target (1.5–3 ms), the disc-to-terrain handover, the station wireframe at 60 fps at 1080p
  on the UHD 620 under load, the descent on a generated world, and the cache peaks.

## Generator version

T1.b bumps `GENERATOR_VERSION` once: the material class is new authoritative output, read by landing
and the consoles, and its goldens pin it. The class discriminants, the property table, the
classifier's thresholds and its order of rules are generator-versioned from then on; a new class
appends a discriminant. `ground_at` reads only heights R09's goldens pin, so T3 adds goldens without
a bump. The weights, the albedo scale, the class map, the bounds' use in selection, the readings and
the horizon maps are presentation and move nothing generated; a change to them is a client change.
No domain tag is added: nothing here draws from a stream. The plan adds no protocol message of its
own; it reads R09's `surface_field` and R03's frames, and asks R09 for one header field (the albedo
scale) and the coverage resolution per cell.

## Risks and open points

- **The selection bound's calibration is an owner ruling.** If T4.a's 99.9th-percentile ratios fall
  at the expected 0.3–0.5, the hard bound costs four to eleven times the patches of a perfect bound;
  selection by min(hard, 4σ) recovers two to three times. The owner rules with T4.a's recorded
  counts; until then selection uses the hard bound.
- **The low setting's cache is at its edge.** With half-float horizon maps a low-setting patch is
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
- **The datum.** This plan states heights above plan 14's reference sphere and sea level beside it;
  if R09 makes the ocean surface the datum on ocean worlds, the classifier's `Seabed` rule, the
  coastline contour and the `DATUM` wording follow it together.
- **Asks of other plans, which the owners may not yet carry** (listed in the notes): of R05, the
  normals as `rg16float` (its `snorm16` needs `texture-formats-tier1`), the effect of `BakedOffsets`
  on the low cache, the slot layout in `PatchCache`, its interpolant and bake kept independent of
  the test planet's height function; of R09, the per-cell variance and lag-b structure function of
  every contribution finer than a wavelength (craters and channels included), the coverage
  resolution per cell to the client (its `CoverDto` carries ranges only), and the albedo scale in
  the field header; of R07, `body_brdf` as lunar-Lambert with per-texel parameters, its disc
  sampling this plan's class map, and its horizon term (`sphere_irradiance`) taking its local
  horizon from the horizon map; of R08, the sun's refracted elevation for the shadow test; of R04, a
  `j0` wrapper in the base crate's `math`.
- **Shadows on the high setting** are taken here by the reading of Design note 11. If the roadmap
  gives them to R07 or R11 instead, T9 moves whole, and T8's horizon map stays here.
- **`Dust` is placed by a simple rule** (arid, thin-aired, where R09's wind deposits); if the coarse
  field cannot tell deposition from erosion well enough, `Dust` falls back into `Soil` with its
  lower bearing figures, and the class stays reserved.
- **Vegetation** is absent until a biosphere exists; a living world's land is drawn bare, which is
  wrong for it and says nothing false about what the generator computed.
